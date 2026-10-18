import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { chmod, copyFile, mkdir, readFile, readdir, readlink, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { CODEX_SENTINEL, createFixture, stopGroup } from './private-fixture.mjs';
import { NativeDriver, waitFor } from './webdriver.mjs';

const harness = dirname(fileURLToPath(import.meta.url));
const repository = resolve(process.env.OPENUSAGE_NATIVE_SOURCE_ROOT ?? join(harness, '../..'));
const application = resolve(process.env.OPENUSAGE_NATIVE_BINARY ?? join(repository, 'target/debug/openusage-desktop'));
const output = resolve(process.env.OPENUSAGE_NATIVE_EVIDENCE ?? join(tmpdir(), `openusage-browser-login-${randomUUID()}`));
const expectedUrl = 'https://auth.openai.com/codex/device';
const fixtureDigest = '25476e3759cbdc1cc74bcd9e46f4a01d1d85ea8ccc34425ddb45a38df27b9859';
const password = 'OpenUsage synthetic browser workspace 2026';
const hash = async path => createHash('sha256').update(await readFile(path)).digest('hex');
const evidence = { requirement: 'NATIVE-DEVICE-BROWSER-001', result: 'failed', startedAt: new Date().toISOString(), scope: 'Actual native device-login browser request through the narrow Rust command and a private XDG synthetic HTTPS handler. No real browser, external network, OAuth, provider credentials, configured browser profile, 900-second timeout or cross-platform acceptance.', cases: [], screenshots: [] };
let fixture, driver, workspace, executable, bin, launcher, browserLog, executionLog, selected, other, ownedPid;
const button = () => `[data-login-browser="${selected.id}"]`;
const cancel = () => `[data-login-cancel="${selected.id}"]`;
async function records(path) { try { return (await readFile(path, 'utf8')).trim().split('\n').filter(Boolean).map(JSON.parse); } catch (error) { if (error.code === 'ENOENT') return []; throw error; } }
async function hashes() {
  const result = {};
  for (const path of [fixture.database, `${fixture.database}-wal`, join(workspace, 'database-key.v1')]) try { result[path] = await hash(path); } catch (error) { if (error.code !== 'ENOENT') throw error; }
  return result;
}
async function create(label) {
  await driver.type('#profile-label', label); await driver.click('.profile-create-form button[type=submit]');
  await waitFor(async () => driver.execute('const n=document.getElementById("profile-label");return Boolean(n&&!n.disabled&&!n.value);'), 'profile creation settled');
  const value = await driver.invoke('list_profiles'); assert.equal(value.ok, true); return value.value.profiles.find(profile => profile.label === label);
}
async function rejected(args, error) { assert.deepEqual(await driver.invoke('open_profile_login_page', args), { ok: false, error }); }
async function focusWindow() {
  const queue = [driver.child.pid];
  for (let count = 0; queue.length && count < 64; count++) {
    const pid = queue.shift();
    try { if (await readlink(`/proc/${pid}/exe`) === application) { ownedPid = pid; break; } queue.push(...(await readFile(`/proc/${pid}/task/${pid}/children`, 'utf8')).trim().split(/\s+/).filter(Boolean).map(Number)); }
    catch (error) { if (error.code !== 'ENOENT') throw error; }
  }
  assert.ok(ownedPid);
  evidence.nativeWindowFocus = JSON.parse(execFileSync('/usr/bin/python3', [join(harness, 'x11-close.py'), String(ownedPid), 'focus'], { env: fixture.environment, encoding: 'utf8', timeout: 10000 }));
  await waitFor(async () => driver.execute('return document.hasFocus();'), 'owned browser test focus');
}
async function reaped(pid) {
  await waitFor(() => { for (const id of [pid, -pid]) try { process.kill(id, 0); return false; } catch (error) { if (error.code !== 'ESRCH') throw error; } return true; }, 'owned synthetic provider reaped');
}
async function installLauncher() {
  // Only this owned launcher is on the application's PATH; no Windows or system fallback can run.
  await writeFile(launcher, '#!/bin/sh\nexec /usr/bin/gio open "$@"\n', { mode: 0o700 }); await chmod(launcher, 0o700);
}
async function capture(name, width) {
  for (let count = 0; count < 4; count++) {
    const actual = await driver.execute('return {width:innerWidth,height:innerHeight};'); if (actual.width === width && actual.height === 850) break;
    const rect = await driver.request('GET', `/session/${driver.session}/window/rect`); await driver.request('POST', `/session/${driver.session}/window/rect`, { width: rect.width + width - actual.width, height: rect.height + 850 - actual.height });
  }
  await driver.execute('document.querySelector(arguments[0]).scrollIntoView({block:"center"});return true;', [button()]);
  const view = await driver.execute('const n=document.querySelector(arguments[0]),r=n.getBoundingClientRect();return {width:innerWidth,height:innerHeight,scrollWidth:document.scrollingElement.scrollWidth,button:{left:r.left,right:r.right,top:r.top,bottom:r.bottom},bodyFont:getComputedStyle(document.body).fontFamily};', [button()]);
  assert.equal(view.width, width); assert.equal(view.height, 850); assert.ok(view.scrollWidth <= width + 1); assert.ok(view.button.left >= 0 && view.button.right <= width);
  await driver.screenshot(join(output, name)); evidence.screenshots.push(name); (evidence.views ??= []).push({ name, ...view });
}
await mkdir(output, { recursive: true, mode: 0o700 });
process.once('exit', () => { driver?.kill('SIGKILL'); stopGroup(fixture?.bus, 'SIGKILL'); });
try {
  assert.equal(process.env.OPENUSAGE_NATIVE_PRIVATE_XVFB, '1', 'Run only in an isolated owned native Xvfb session.');
  evidence.application = { path: application, sha256: await hash(application) }; evidence.sourceCommit = execFileSync('git', ['rev-parse', 'HEAD'], { cwd: repository, encoding: 'utf8' }).trim();
  evidence.sourceHashes = {};
  const sources = execFileSync('rg', ['--files', 'apps/desktop/src', 'apps/desktop/src-tauri/src', 'apps/desktop/src-tauri/permissions', 'crates/openusage-codex/src', 'crates/openusage-storage/src'], { cwd: repository, encoding: 'utf8' }).trim().split('\n');
  for (const path of [...sources, 'Cargo.lock', 'apps/desktop/src-tauri/Cargo.toml', 'apps/desktop/src-tauri/build.rs', 'apps/desktop/src-tauri/capabilities/main-window.json', 'apps/desktop/src-tauri/tauri.conf.json']) evidence.sourceHashes[path] = await hash(join(repository, path));
  evidence.harnessHashes = {};
  for (const path of ['browser-login.mjs', 'fixtures/synthetic-login-provider.py', 'private-fixture.mjs', 'webdriver.mjs', 'x11-close.py']) evidence.harnessHashes[path] = await hash(join(harness, path));
  fixture = await createFixture('browser-login'); workspace = dirname(fixture.database); bin = join(fixture.root, 'browser-bin'); launcher = join(bin, 'xdg-open'); browserLog = join(fixture.root, 'synthetic-browser-observations.jsonl'); executionLog = join(fixture.root, 'synthetic-login-observations.jsonl');
  await mkdir(bin, { mode: 0o700 });
  const applications = join(fixture.dirs.data, 'applications'); await mkdir(applications, { mode: 0o700 });
  const handler = join(fixture.root, 'synthetic-browser.py');
  await writeFile(handler, '#!/usr/bin/python3\nimport json,os,sys\nfrom pathlib import Path\np=Path(__file__).with_name("synthetic-browser-observations.jsonl")\nwith p.open("a") as f: f.write(json.dumps({"pid":os.getpid(),"arguments":sys.argv[1:]})+"\\n")\n', { mode: 0o700 });
  await writeFile(join(applications, 'openusage-synthetic-browser.desktop'), `[Desktop Entry]\nType=Application\nName=OpenUsage synthetic browser\nExec=/usr/bin/python3 ${handler} %u\nTerminal=false\nNoDisplay=true\nMimeType=x-scheme-handler/https;\n`, { mode: 0o600 });
  await writeFile(join(fixture.dirs.config, 'mimeapps.list'), '[Default Applications]\nx-scheme-handler/https=openusage-synthetic-browser.desktop;\n', { mode: 0o600 });
  await installLauncher();
  Object.assign(fixture.environment, { HOME: fixture.root, PATH: bin, XDG_DATA_DIRS: fixture.dirs.data, XDG_CURRENT_DESKTOP: 'OpenUsageSynthetic' }); delete fixture.environment.BROWSER;
  assert.deepEqual(await readdir(bin), ['xdg-open']); evidence.osHandlerScope = { privatePathOnly: true, windowsAndSystemLaunchersAbsent: true, privateXdgAssociationOnly: true, launcher: 'Owned xdg-open shim invokes absolute /usr/bin/gio with the private HTTPS association. No actual browser or network.' };
  executable = join(fixture.root, 'synthetic-login-provider'); await copyFile(join(harness, 'fixtures/synthetic-login-provider.py'), executable); await chmod(executable, 0o700); assert.equal(await hash(executable), fixtureDigest);
  await writeFile(join(fixture.root, 'synthetic-login-provider-config.json'), JSON.stringify({ defaultMode: 'pending', modesByHome: {} }) + '\n', { mode: 0o600 });
  driver = new NativeDriver(application, fixture.environment, join(output, 'driver.log')); await driver.start(); await focusWindow();
  const genericOpener = await driver.invoke('plugin:opener|open_url', { url: expectedUrl }); assert.equal(genericOpener.ok, false); assert.deepEqual(await records(browserLog), []); evidence.genericOpenerCommandRefused = true;
  await rejected({ profileId: randomUUID(), attemptId: randomUUID() }, 'workspace_locked');
  await driver.type('#workspace-passphrase', password); await driver.type('#workspace-confirmation', password); await driver.click('.unlock-form button[type=submit]'); await waitFor(async () => (await driver.invoke('bootstrap')).value?.storage === 'ready', 'synthetic workspace setup');
  selected = await create('Selected synthetic browser profile'); other = await create('Other synthetic browser profile');
  // Observe only public attempt/approval UUIDs; all actions still use real IPC.
  await driver.execute('window.__browserOriginalInvoke=window.__TAURI__.core.invoke;window.__browserObservedAttempt=null;window.__browserObservedApproval=null;window.__TAURI__.core.invoke=function(command,args){const pending=window.__browserOriginalInvoke.call(this,command,args);if(command==="begin_profile_login")pending.then(value=>{window.__browserObservedAttempt={profileId:value.profileId,attemptId:value.attemptId};});if(command==="approve_provider_executable")pending.then(value=>{window.__browserObservedApproval=value.approvalId;});return pending;};return true;');
  for(const selector of ['[data-provider-settings]','[data-provider-manual]'])if(await driver.execute('return !document.querySelector(arguments[0]).closest("details").open;',[selector]))await driver.click(selector);
  await driver.type('#provider-executable', executable);await driver.click('.provider-inspection-form button[type=submit]');
  await waitFor(async()=>driver.execute('const n=document.getElementById("provider-executable-approved");return Boolean(n&&!n.disabled&&n.getAttribute("role")==="checkbox"&&n.getAttribute("aria-checked")==="false"&&n.getClientRects().length);'),'reviewed synthetic source consent');
  await driver.click('#provider-executable-approved');await driver.click('.provider-approval-form button[type=submit]');
  await waitFor(async()=>driver.execute('return Boolean(window.__browserObservedApproval&&document.querySelector(arguments[0]));',[`[data-login-start="${selected.id}"]`]),'approved synthetic source and sign-in action');
  const approvalId=await driver.execute('return window.__browserObservedApproval;');
  const before = await hashes(); const otherFiles = await readdir(join(workspace, 'profiles', other.id));
  assert.deepEqual(await records(browserLog), []);
  for (const args of [[], {}, { profileId: selected.id, attemptId: randomUUID(), url: expectedUrl }, { profileId: selected.id, attemptId: randomUUID(), program: 'browser' }]) await rejected(args, 'invalid_arguments');
  const early = await driver.invoke('begin_profile_login', { profileId: selected.id, approvalId, flow: 'device_code', confirmed: true }); assert.equal(early.ok, true);
  await rejected({ profileId: selected.id, attemptId: early.value.attemptId }, 'provider_login_prompt_pending');
  await driver.invoke('cancel_profile_login', { profileId: selected.id, attemptId: early.value.attemptId });
  assert.deepEqual(await records(browserLog), []); assert.deepEqual(await hashes(), before);
  await driver.click(`[data-login-start="${selected.id}"]`); assert.deepEqual(await records(browserLog), []); await driver.click(`form[data-login-consent="${selected.id}"] button[type=submit]`);
  await waitFor(async () => driver.execute('const n=document.querySelector(arguments[0]);return Boolean(n&&!n.disabled&&window.__browserObservedAttempt);', [button()]), 'active published browser action');
  const attempt = await driver.execute('return window.__browserObservedAttempt;'); assert.equal(attempt.profileId, selected.id);
  await waitFor(async () => (await records(executionLog)).some(row => row.kind === 'challenge-emitted'), 'synthetic challenge marker');
  const starts = (await records(executionLog)).filter(row => row.kind === 'start' && row.arguments[0] === 'app-server'); const providerProcess = starts.at(-1); assert.ok(providerProcess); assert.equal(providerProcess.environment.CODEX_HOME, join(workspace, 'profiles', selected.id));
  await rejected({ profileId: other.id, attemptId: attempt.attemptId }, 'provider_login_not_found'); assert.deepEqual(await records(browserLog), []);
  await rm(launcher); assert.deepEqual(await readdir(bin), []); await driver.click(button());
  await waitFor(async () => driver.execute('const n=document.getElementById(arguments[0]);return Boolean(n&&n.textContent.includes("could not be started"));', [`login-browser-error-${selected.id}`]), 'redacted launcher-spawn failure');
  assert.deepEqual(await records(browserLog), []); assert.equal(await driver.execute('return document.querySelector(arguments[0]).disabled;', [cancel()]), false); assert.equal(await driver.execute('return document.getElementById(arguments[0]).value;', [`login-code-${selected.id}`]), 'SYNTH-1234');
  await capture('browser-launch-failure-light-360.png', 360);
  await installLauncher(); await focusWindow(); await driver.click(button());
  await waitFor(async () => (await records(browserLog)).length === 1, 'private synthetic HTTPS handler dispatch');
  const dispatches = await records(browserLog); assert.deepEqual(dispatches.map(row => row.arguments), [[expectedUrl]]); for(const dispatch of dispatches)await reaped(dispatch.pid); assert.equal(await driver.execute('return Boolean(document.getElementById(arguments[0]));', [`login-browser-error-${selected.id}`]), false);
  assert.equal(await driver.execute('return document.querySelector(arguments[0]).textContent.trim();', [button()]), 'Open sign-in page'); assert.equal(await driver.execute('return document.querySelector(arguments[0]).disabled;', [cancel()]), false);
  process.kill(providerProcess.pid, 0); assert.deepEqual(await hashes(), before); assert.deepEqual(await readdir(join(workspace, 'profiles', other.id)), otherFiles);
  await capture('browser-request-light-360.png', 360); await capture('browser-request-light-desktop.png', 1280);
  await driver.click(cancel()); await waitFor(async () => driver.execute('return !document.querySelector(arguments[0])&&!document.getElementById(arguments[1]);', [button(), `login-code-${selected.id}`]), 'cancel clears browser action and sensitive prompt'); await reaped(providerProcess.pid);
  await rejected(attempt, 'provider_login_not_found'); assert.equal((await records(browserLog)).length, 1); assert.deepEqual(await hashes(), before);
  await driver.execute('window.__TAURI__.core.invoke=window.__browserOriginalInvoke;delete window.__browserOriginalInvoke;delete window.__browserObservedAttempt;delete window.__browserObservedApproval;return true;');
  assert.equal(await readFile(join(fixture.dirs.codex, 'synthetic-fixture.txt'), 'utf8'), CODEX_SENTINEL); assert.deepEqual(await readdir(fixture.dirs.codex), ['synthetic-fixture.txt']);
  for (const row of (await records(executionLog)).filter(row => row.kind === 'start')) { assert.equal(row.environment.CODEX_HOME, join(workspace, 'profiles', selected.id)); assert.deepEqual(row.unexpectedEnvironmentKeys, []); await reaped(row.pid); }
  evidence.cases.push({ noDispatchBeforeConsentAndPublishedPrompt: true, extraUrlAndProgramArgumentsRefused: true, selectedAttemptOnly: true, launcherFailureKeepsManualCopyAndCancel: true, sameAttemptRetryDispatchesExactFixedUrl: true, onlyOneExternalHandoff: true, cancellationClearsPromptAndRefusesStaleDispatch: true, applicationRecordsAndOtherHomePreserved: true, ownedProvidersReaped: true }); evidence.dispatches = dispatches;
  assert.equal(await hash(application), evidence.application.sha256); for (const [path, digest] of Object.entries(evidence.sourceHashes)) assert.equal(await hash(join(repository, path)), digest); for (const [path, digest] of Object.entries(evidence.harnessHashes)) assert.equal(await hash(join(harness, path)), digest);
  evidence.nativeClose = JSON.parse(execFileSync('/usr/bin/python3', [join(harness, 'x11-close.py'), String(ownedPid)], { env: fixture.environment, encoding: 'utf8', timeout: 10000 }));
  await waitFor(async () => { try { await readlink(`/proc/${ownedPid}/exe`); return false; } catch (error) { if (error.code === 'ENOENT') return true; throw error; } }, 'owned native close'); driver.session = undefined; evidence.result = 'passed';
} catch (error) { evidence.error = error.stack; process.exitCode = 1; if (driver?.session) try { await driver.screenshot(join(output, 'failure.png')); evidence.screenshots.push('failure.png'); } catch {} }
finally { try { await driver?.stop(); } catch(error) {evidence.cleanupError=error.message;evidence.result="failed";process.exitCode=1;} if (fixture) { for (const row of (await records(executionLog)).filter(row => row.kind === 'start')) try { const cmd = await readFile(`/proc/${row.pid}/cmdline`); if (row.pid === row.processGroup && cmd.includes(Buffer.from(executable + '\0'))) process.kill(-row.processGroup, 'SIGTERM'); } catch (error) { if (!['ENOENT', 'ESRCH'].includes(error.code)) throw error; } stopGroup(fixture.bus, 'SIGKILL'); await rm(fixture.root, { recursive: true, force: true }); } evidence.finishedAt = new Date().toISOString(); await writeFile(join(output, 'result.json'), JSON.stringify(evidence, null, 2) + '\n', { mode: 0o600 }); console.log(`${evidence.requirement}: ${evidence.result}. Evidence: ${output}`); }
