import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { chmod, copyFile, lstat, mkdir, readFile, readdir, readlink, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { fileURLToPath } from 'node:url';
import { CODEX_SENTINEL, createFixture, stopGroup } from './private-fixture.mjs';
import { NativeDriver, waitFor } from './webdriver.mjs';

const harness = dirname(fileURLToPath(import.meta.url));
const repository = resolve(process.env.OPENUSAGE_NATIVE_SOURCE_ROOT ?? join(harness, '../..'));
const application = resolve(process.env.OPENUSAGE_NATIVE_BINARY ?? join(repository, 'target/debug/openusage-desktop'));
const output = resolve(process.env.OPENUSAGE_NATIVE_EVIDENCE ?? join(tmpdir(), `openusage-login-timeout-${randomUUID()}`));
const fixtureDigest = '25476e3759cbdc1cc74bcd9e46f4a01d1d85ea8ccc34425ddb45a38df27b9859';
const password = 'OpenUsage synthetic native timeout workspace 2026';
const hash = async path => createHash('sha256').update(await readFile(path)).digest('hex');
const monotonic = () => Number(process.hrtime.bigint()) / 1e9;
const evidence = { requirement: 'NATIVE-DEVICE-LOGIN-TIMEOUT-001', result: 'failed', startedAt: new Date().toISOString(), scope: 'Actual 900-second native application wait limit with the existing reviewed synthetic pending-login provider. No real OAuth, browser opening, network operation, existing credential home, provider-code expiry, verified identity, accepted history, monitoring, release-performance or cross-platform qualification.', screenshots: [], views: [], heartbeats: [] };
let fixture, driver, workspace, executable, executionLog, first, other, archived;
const startSelector = id => `[data-login-start="${id}"]`;
const consentSelector = id => `form[data-login-consent="${id}"]`;
const promptSelector = id => `.login-prompt[data-login-profile="${id}"]`;
const cancelSelector = id => `[data-login-cancel="${id}"]`;
const resultSelector = id => `.login-result[data-login-profile="${id}"]`;

async function observations() {
  try { return (await readFile(executionLog, 'utf8')).trim().split('\n').filter(Boolean).map(JSON.parse); }
  catch (error) { if (error.code === 'ENOENT') return []; throw error; }
}
async function liveHashes() {
  const result = {};
  for (const path of [fixture.database, `${fixture.database}-wal`, join(workspace, 'database-key.v1')]) {
    try { result[path] = await hash(path); } catch (error) { if (error.code !== 'ENOENT') throw error; }
  }
  return result;
}
async function files(home) {
  const result = {};
  async function visit(path, relative) {
    for (const item of await readdir(path, { withFileTypes: true })) {
      const name = join(relative, item.name), full = join(path, item.name);
      assert.equal(item.isSymbolicLink(), false);
      if (item.isDirectory()) { result[name] = { directory: true, mode: (await lstat(full)).mode & 0o777 }; await visit(full, name); }
      else { assert.equal(item.isFile(), true); result[name] = { sha256: await hash(full), mode: (await lstat(full)).mode & 0o777 }; }
    }
  }
  await visit(home, ''); return result;
}
async function profiles() { const value = await driver.invoke('list_profiles'); assert.equal(value.ok, true); return value.value.profiles; }
async function create(label) { await driver.openProfileCreate();
  await waitFor(async () => driver.execute('return Boolean(document.querySelector("#profile-label")&&!document.querySelector("#profile-label").disabled);'), 'enabled profile form');
  await driver.type('#profile-label', label); await driver.click('.profile-create-form button[type=submit]');
  await waitFor(async () => driver.execute('const field=document.querySelector("#profile-label");return !field.disabled&&field.value==="";'), 'created profile');
  const value = (await profiles()).find(profile => profile.label === label); assert.ok(value); return value;
}
async function stage(kind, home, offset) {
  let found;
  await waitFor(async () => { const rows = await observations(), starts = new Map(rows.filter(row => row.kind === 'start').map(row => [row.pid, row])); found = rows.slice(offset).find(row => row.kind === kind && starts.get(row.pid)?.environment.CODEX_HOME === home); return Boolean(found); }, `selected-home ${kind}`);
  return found;
}
async function reaped(row) {
  await waitFor(() => { for (const pid of [row.pid, -row.processGroup]) try { process.kill(pid, 0); return false; } catch (error) { if (error.code !== 'ESRCH') throw error; } return true; }, 'owned login PID and group reaped');
}
async function approveGui() { await driver.navigate('settings');
  if (await driver.execute('return !document.querySelector("[data-provider-settings]").closest("details").open;')) await driver.click('[data-provider-settings]');
  if (await driver.execute('const node=document.querySelector("[data-provider-manual]");return Boolean(node&&!node.closest("details").open);')) await driver.click('[data-provider-manual]');
  await driver.type('#provider-executable', executable); await driver.click('.provider-inspection-form button[type=submit]');
  await waitFor(async () => driver.execute('const node=document.querySelector("#provider-executable-approved");return Boolean(node&&!node.disabled);'), 'reviewed fixture source');
  assert.deepEqual(await driver.execute('const node=document.querySelector("#provider-executable-approved");return {tag:node.tagName,role:node.getAttribute("role"),checked:node.getAttribute("aria-checked"),disabled:document.querySelector(".provider-approval-form button[type=submit]").disabled};'), { tag: 'BUTTON', role: 'checkbox', checked: 'false', disabled: true });
  assert.deepEqual(await observations(), [], 'Inspection must not execute the fixture.');
  await driver.click('#provider-executable-approved');
  await waitFor(async () => driver.execute('return document.querySelector("#provider-executable-approved").getAttribute("aria-checked")==="true"&&!document.querySelector(".provider-approval-form button[type=submit]").disabled;'), 'explicit source approval');
  await driver.click('.provider-approval-form button[type=submit]');
  await waitFor(async () => driver.execute('return document.activeElement?.matches(".provider-approval-result");'), 'source approval focus');
  assert.deepEqual(await observations(), [], 'Approval must not execute the fixture.');
}
async function consentGui() {
  await driver.openProfileReadings(first.id);
  await waitFor(async () => driver.execute('const node=document.querySelector(arguments[0]);return Boolean(node&&!node.disabled);', [startSelector(first.id)]), 'enabled Sign in');
  const before = (await observations()).length; await driver.click(startSelector(first.id));
  await waitFor(async () => driver.execute('const form=document.querySelector(arguments[0]);return Boolean(form&&document.activeElement===form.querySelector("button[type=submit]"));', [consentSelector(first.id)]), 'focused explicit consent');
  const text = await driver.execute('return document.querySelector(arguments[0]).textContent;', [consentSelector(first.id)]);
  assert.ok(text.includes(first.label)); assert.match(text, /OpenAI/); assert.match(text, /sign-in information/); assert.match(text, /Credentials already saved/);
  assert.equal((await observations()).length, before, 'Opening consent must not start the provider.');
  const submittedAt = monotonic(); await driver.click(`${consentSelector(first.id)} button[type=submit]`); return submittedAt;
}
async function promptGui() {
  await waitFor(async () => driver.execute('const field=document.querySelector(arguments[0]);return Boolean(field&&document.activeElement===field&&field.selectionStart===0&&field.selectionEnd===field.value.length);', [`#login-code-${first.id}`]), 'focused selected synthetic code');
  const value = await driver.execute('const panel=document.querySelector(arguments[0]),code=document.querySelector(arguments[1]),url=document.querySelector(arguments[2]),cancel=document.querySelector(arguments[3]);return {code:code.value,url:url.value,codeReadonly:code.readOnly,urlReadonly:url.readOnly,links:panel.querySelectorAll("a").length,cancelEnabled:Boolean(cancel&&!cancel.disabled),fixture:Boolean(panel.closest(".login-panel").querySelector("[data-provider-fixture]")),wait:panel.textContent};', [promptSelector(first.id), `#login-code-${first.id}`, `#login-address-${first.id}`, cancelSelector(first.id)]);
  assert.equal(value.code, 'SYNTH-1234'); assert.equal(value.url, 'https://auth.openai.com/codex/device'); assert.equal(value.codeReadonly, true); assert.equal(value.urlReadonly, true); assert.equal(value.links, 0); assert.equal(value.cancelEnabled, true); assert.equal(value.fixture, true); assert.match(value.wait, /up to 15 minutes/); assert.match(value.wait, /did not provide a code expiry/);
  return monotonic();
}
async function terminal(pattern) {
  await waitFor(async () => driver.execute('return document.activeElement?.matches(arguments[0])&&Boolean(document.querySelector(arguments[1])&&!document.querySelector(arguments[1]).disabled);', [resultSelector(first.id), startSelector(first.id)]), 'terminal result focus and reusable Sign in');
  const value = await driver.execute('const root=document.querySelector(arguments[0]);return {role:root.getAttribute("role"),text:root.textContent,prompts:document.querySelectorAll(arguments[1]).length,fields:document.querySelectorAll(arguments[2]).length,cancel:document.querySelectorAll(arguments[3]).length};', [resultSelector(first.id), promptSelector(first.id), `#login-code-${first.id},#login-address-${first.id}`, cancelSelector(first.id)]);
  assert.match(value.text, pattern); assert.equal(value.prompts, 0); assert.equal(value.fields, 0); assert.equal(value.cancel, 0); assert.doesNotMatch(value.text, /SYNTH-1234|auth\.openai\.com|synthetic-device-attempt/); return value;
}
async function capture(name, selector) {
  await driver.execute('document.querySelector(arguments[0]).scrollIntoView({block:"center"});return true;', [selector]);
  await driver.request('POST', `/session/${driver.session}/execute/async`, { script: 'const done=arguments[arguments.length-1];document.fonts.ready.then(()=>requestAnimationFrame(()=>requestAnimationFrame(()=>done(true))));', args: [] });
  const value = await driver.execute('const root=document.querySelector(arguments[0]);return {width:innerWidth,scrollWidth:document.scrollingElement.scrollWidth,theme:document.documentElement.dataset.theme,font:getComputedStyle(root).fontFamily,overflow:[root,...root.querySelectorAll("*")].filter(node=>{const r=node.getBoundingClientRect();return r.width>1&&r.height>1&&(r.left< -1||r.right>innerWidth+1);}).map(node=>node.tagName)};', [selector]);
  assert.ok(value.scrollWidth <= value.width + 1); assert.deepEqual(value.overflow, []); assert.match(value.font, /DM Sans/);
  await driver.screenshot(join(output, name)); evidence.screenshots.push(name); evidence.views.push({ name, ...value });
}
async function closeNormally() {
  const queue = [driver.child.pid]; let owned;
  for (let count = 0; queue.length && count < 64; count++) { const pid = queue.shift(); try { if (await readlink(`/proc/${pid}/exe`) === application) { owned = pid; break; } queue.push(...(await readFile(`/proc/${pid}/task/${pid}/children`, 'utf8')).trim().split(/\s+/).filter(Boolean).map(Number)); } catch (error) { if (error.code !== 'ENOENT') throw error; } }
  assert.ok(owned); evidence.nativeClose = JSON.parse(execFileSync('/usr/bin/python3', [join(harness, 'x11-close.py'), String(owned)], { env: fixture.environment, encoding: 'utf8', timeout: 10000 }));
  await waitFor(async () => { try { await readlink(`/proc/${owned}/exe`); return false; } catch (error) { if (error.code === 'ENOENT') return true; throw error; } }, 'genuine native close'); driver.session = undefined;
}

await mkdir(output, { recursive: true, mode: 0o700 });
process.once('exit', () => { driver?.kill('SIGKILL'); stopGroup(fixture?.bus, 'SIGKILL'); });
try {
  evidence.application = { path: application, sha256: await hash(application) };
  evidence.sourceCommit = execFileSync('git', ['rev-parse', 'HEAD'], { cwd: repository, encoding: 'utf8' }).trim();
  evidence.sourceHashes = {};
  const sources = execFileSync('rg', ['--files', 'apps/desktop/src', 'apps/desktop/src-tauri/src', 'apps/desktop/src-tauri/permissions', 'crates/openusage-codex/src', 'crates/openusage-storage/src', 'crates/openusage-import/src'], { cwd: repository, encoding: 'utf8' }).trim().split('\n');
  for (const path of [...sources, 'Cargo.toml', 'Cargo.lock', 'package.json', 'package-lock.json', 'crates/openusage-codex/Cargo.toml', 'crates/openusage-codex/provider-sources/codex-0.163.0-linux-x86_64.json', 'apps/desktop/src-tauri/Cargo.toml', 'apps/desktop/src-tauri/build.rs', 'apps/desktop/src-tauri/tauri.conf.json', 'apps/desktop/src-tauri/capabilities/main-window.json', 'apps/desktop/public/fonts/DMSans-Variable.ttf', 'apps/desktop/public/fonts/DM-Sans-OFL.txt']) evidence.sourceHashes[path] = await hash(join(repository, path));
  assert.match(await readFile(join(repository, 'crates/openusage-codex/src/session.rs'), 'utf8'), /pub const DEVICE_LOGIN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs\(15 \* 60\);/);
  evidence.harnessHashes = {}; for (const path of ['device-login-timeout.mjs', 'fixtures/synthetic-login-provider.py', 'private-fixture.mjs', 'webdriver.mjs', 'x11-close.py']) evidence.harnessHashes[path] = await hash(join(harness, path));
  fixture = await createFixture('device-login-timeout'); fixture.environment.HOME = join(fixture.root, 'synthetic-user-home'); await mkdir(fixture.environment.HOME, { mode: 0o700 });
  fixture.environment.OPENAI_API_KEY = 'synthetic-parent-key-must-not-reach-child'; fixture.environment.OPENAI_ACCESS_TOKEN = 'synthetic-parent-token-must-not-reach-child';
  workspace = dirname(fixture.database); executable = join(fixture.root, 'synthetic-login-provider'); executionLog = join(fixture.root, 'synthetic-login-observations.jsonl');
  await copyFile(join(harness, 'fixtures/synthetic-login-provider.py'), executable); await chmod(executable, 0o700); assert.equal(await hash(executable), fixtureDigest);
  await writeFile(join(fixture.root, 'synthetic-login-provider-config.json'), JSON.stringify({ defaultMode: 'pending' }) + '\n', { mode: 0o600 });
  driver = new NativeDriver(application, fixture.environment, join(output, 'driver.log')); await driver.start();
  await waitFor(async () => driver.execute('return ["#workspace-passphrase","#workspace-confirmation"].every(selector=>{const field=document.querySelector(selector);return Boolean(field&&!field.disabled);});'), 'enabled setup fields');
  await driver.type('#workspace-passphrase', password); await driver.type('#workspace-confirmation', password); await driver.click('.unlock-form button[type=submit]');
  first = await create('Synthetic timeout profile'); other = await create('Other preserved profile'); archived = await create('Archived preserved profile');
  await driver.openProfileOptions(archived.id); await driver.click(`[data-profile-id="${archived.id}"] [data-profile-action="archive"]`); await waitFor(async () => (await profiles()).find(profile => profile.id === archived.id)?.archived, 'archived preserved profile');
  const home = join(workspace, 'profiles', first.id), otherHome = join(workspace, 'profiles', other.id), archivedHome = join(workspace, 'profiles', archived.id);
  for (const path of [home, otherHome, archivedHome]) await writeFile(join(path, 'synthetic-preserved-marker.txt'), 'Owned timeout test marker. No credentials.\n', { mode: 0o600 });
  const beforeProfiles = await profiles(), beforeHomes = [await files(home), await files(otherHome), await files(archivedHome)];
  await approveGui(); const baseline = await liveHashes(); assert.equal(Object.keys(baseline).length, 3);
  // Observe the real IPC promises without delaying, replacing or changing their results or clocks.
  assert.equal(await driver.execute('const descriptor=Object.getOwnPropertyDescriptor(window.__TAURI__,"core");return descriptor.writable;'), true);
  await driver.execute('const tauri=window.__TAURI__,original=tauri.core;window.__timeoutOriginalCore=original;window.__timeoutTrace=[];tauri.core={...original,invoke(command,...args){const at=performance.now(),pending=Reflect.apply(original.invoke,original,[command,...args]);if(command!=="complete_profile_login")return pending;pending.then(()=>window.__timeoutTrace.push({command,ok:true,startedAtMs:at,finishedAtMs:performance.now(),profileId:args[0]?.profileId,attemptId:args[0]?.attemptId}),error=>window.__timeoutTrace.push({command,ok:false,startedAtMs:at,finishedAtMs:performance.now(),profileId:args[0]?.profileId,attemptId:args[0]?.attemptId,error}));return pending;}};return true;');
  const offset = (await observations()).length, submittedAt = await consentGui(), promptObservedAt = await promptGui();
  assert.ok(promptObservedAt - submittedAt <= 5, 'Prompt observation must stay within the documented five-second timing allowance.');
  const child = await stage('challenge-emitted', home, offset); await capture('device-login-timeout-pending.png', promptSelector(first.id));
  let timedOutAt, nextHeartbeat = promptObservedAt;
  while (monotonic() - submittedAt <= 960) {
    const state = await driver.execute('return {nowMs:performance.now(),trace:window.__timeoutTrace,code:document.querySelector(arguments[0])?.value??null,url:document.querySelector(arguments[1])?.value??null,cancelEnabled:Boolean(document.querySelector(arguments[2])&&!document.querySelector(arguments[2]).disabled),heartbeat:window.__timeoutHeartbeat=(window.__timeoutHeartbeat??0)+1};', [`#login-code-${first.id}`, `#login-address-${first.id}`, cancelSelector(first.id)]);
    const completion = state.trace.find(item => item.command === 'complete_profile_login');
    if (completion) { assert.equal(completion.ok, false); assert.equal(completion.error, 'provider_login_timed_out'); timedOutAt = monotonic(); break; }
    assert.equal(state.code, 'SYNTH-1234'); assert.equal(state.url, 'https://auth.openai.com/codex/device'); assert.equal(state.cancelEnabled, true); process.kill(child.pid, 0);
    if (monotonic() >= nextHeartbeat) {
      assert.deepEqual(await liveHashes(), baseline); assert.deepEqual(await files(otherHome), beforeHomes[1]); assert.deepEqual(await files(archivedHome), beforeHomes[2]);
      evidence.heartbeats.push({ elapsedSinceSubmitSeconds: monotonic() - submittedAt, browserMonotonicMs: state.nowMs, successfulShortExecute: true, heartbeat: state.heartbeat, pendingCancelEnabled: true, storageAndOtherHomesPreserved: true });
      await writeFile(join(output, 'progress.json'), JSON.stringify({ result: 'running', elapsedSeconds: monotonic() - submittedAt, targetSeconds: 900, heartbeatCount: evidence.heartbeats.length }) + '\n', { mode: 0o600 }); nextHeartbeat = monotonic() + 30;
    }
    await delay(500);
  }
  assert.ok(timedOutAt, 'The real application must report its timeout within the bounded 960-second observation window.');
  const elapsedSinceSubmitSeconds = timedOutAt - submittedAt, elapsedSinceObservedPromptSeconds = timedOutAt - promptObservedAt;
  assert.ok(elapsedSinceSubmitSeconds >= 900, 'A premature application timeout must fail the native journey.');
  assert.ok(elapsedSinceObservedPromptSeconds >= 895, 'The observed prompt must remain pending for approximately the full production wait.');
  evidence.timing = { configuredNativeWaitSeconds: 900, elapsedSinceSubmitSeconds, elapsedSinceObservedPromptSeconds, promptObservationLagSeconds: promptObservedAt - submittedAt, monotonicClock: 'Node process.hrtime.bigint; browser performance.now recorded independently.', origin: 'Immediately before the real GUI consent submit, and immediately after the validated visible readonly prompt.', bounds: 'At least 900 seconds from submit, at least 895 from observed prompt; prompt observation within five seconds of submit; terminal within 960 seconds. No clock, timer, native constant or response delay was changed.' };
  const timeoutResult = await terminal(/OpenUsage reached its sign-in wait limit/); assert.equal(timeoutResult.role, 'alert'); evidence.timeoutResult = timeoutResult;
  await reaped(child); const timeoutRows = (await observations()).filter(row => row.pid === child.pid);
  assert.deepEqual(timeoutRows.filter(row => row.kind === 'request').map(row => ({ method: row.request.method, id: row.request.id ?? null })), [{ method: 'initialize', id: 1 }, { method: 'initialized', id: null }, { method: 'account/login/start', id: 3 }, { method: 'account/login/cancel', id: 5 }]);
  assert.equal(timeoutRows.some(row => row.kind === 'completion-emitted' || row.kind === 'post-login-account-read'), false);
  assert.deepEqual(timeoutRows.find(row => row.kind === 'request' && row.request.id === 5).request.params, { loginId: 'synthetic-device-attempt' });
  await capture('device-login-timeout-result.png', resultSelector(first.id)); assert.deepEqual(await liveHashes(), baseline);
  const firstTrace = await driver.execute('return window.__timeoutTrace;'); assert.equal(firstTrace.length, 1); assert.equal(firstTrace[0].profileId, first.id); assert.match(firstTrace[0].attemptId, /^[0-9a-f-]{36}$/);
  const reuseOffset = (await observations()).length; await consentGui(); await promptGui(); const successor = await stage('challenge-emitted', home, reuseOffset); assert.notEqual(successor.pid, child.pid);
  await driver.click(cancelSelector(first.id)); const cancelled = await terminal(/Credentials already saved by Codex may remain/); assert.equal(cancelled.role, 'status'); await reaped(successor);
  const trace = await driver.execute('return window.__timeoutTrace;'); assert.equal(trace.length, 2); assert.notEqual(trace[0].attemptId, trace[1].attemptId); assert.equal(trace[1].profileId, first.id); assert.equal(trace[1].ok, false); assert.equal(trace[1].error, 'provider_login_cancelled');
  const successorRequests = (await observations()).filter(row => row.kind === 'request' && row.pid === successor.pid);
  assert.deepEqual(successorRequests.map(row => ({ method: row.request.method, id: row.request.id ?? null })), [{ method: 'initialize', id: 1 }, { method: 'initialized', id: null }, { method: 'account/login/start', id: 3 }, { method: 'account/login/cancel', id: 5 }]);
  await capture('device-login-timeout-slot-reused-cancelled.png', resultSelector(first.id)); evidence.ipcTrace = trace;
  await driver.execute('window.__TAURI__.core=window.__timeoutOriginalCore;delete window.__timeoutOriginalCore;delete window.__timeoutTrace;delete window.__timeoutHeartbeat;return true;');
  assert.deepEqual(await profiles(), beforeProfiles); assert.deepEqual(await liveHashes(), baseline);
  assert.deepEqual([await files(home), await files(otherHome), await files(archivedHome)], beforeHomes);
  const rows = await observations(); evidence.observations = rows;
  for (const row of rows.filter(row => row.kind === 'start')) {
    assert.equal(row.environment.CODEX_HOME, home); assert.deepEqual(row.unexpectedEnvironmentKeys, []); assert.equal(row.environment.PATH, '/usr/bin:/bin'); assert.equal(row.environment.LANG, 'C.UTF-8');
    const runtime = dirname(row.environment.HOME); assert.ok(runtime.startsWith(home + '/openusage-provider-'));
    for (const key of ['XDG_CONFIG_HOME', 'XDG_DATA_HOME', 'XDG_CACHE_HOME', 'XDG_STATE_HOME', 'XDG_RUNTIME_DIR', 'TMPDIR']) assert.ok(row.environment[key].startsWith(runtime + '/'));
    await reaped(row); await assert.rejects(lstat(runtime), { code: 'ENOENT' });
  }
  for (const row of rows.filter(row => row.kind === 'request')) assert.ok(['initialize', 'initialized', 'account/login/start', 'account/login/cancel'].includes(row.request.method), 'No account read, login completion, inference or extra operation is allowed.');
  assert.equal(await readFile(join(fixture.dirs.codex, 'synthetic-fixture.txt'), 'utf8'), CODEX_SENTINEL); assert.deepEqual(await readdir(fixture.dirs.codex), ['synthetic-fixture.txt']);
  evidence.storagePreservation = { dbWalAndWrappedKeyHashes: baseline, unchangedAfterTimeoutAndSuccessorCancel: true, selectedOtherAndArchivedHomesPreserved: true };
  assert.equal(await hash(executable), fixtureDigest); assert.equal(await hash(application), evidence.application.sha256);
  for (const [path, digest] of Object.entries(evidence.sourceHashes)) assert.equal(await hash(join(repository, path)), digest, `Source drift: ${path}`);
  for (const [path, digest] of Object.entries(evidence.harnessHashes)) assert.equal(await hash(join(harness, path)), digest, `Harness drift: ${path}`);
  await closeNormally(); evidence.result = 'passed';
} catch (error) {
  evidence.error = error.stack ?? String(error); process.exitCode = 1;
  try { evidence.observations = await observations(); } catch {}
  if (driver?.session) try { await driver.screenshot(join(output, 'failure.png')); evidence.screenshots.push('failure.png'); } catch {}
} finally {
  if (evidence.result !== 'passed' && driver?.session) try { await driver.click(cancelSelector(first.id)); } catch {}
  try { await driver?.stop(); } catch (error) { evidence.cleanupError = error.message; evidence.result = 'failed'; process.exitCode = 1; }
  if (fixture) {
    if (evidence.result !== 'passed') for (const row of evidence.observations ?? []) if (row.kind === 'start') try { const command = await readFile(`/proc/${row.pid}/cmdline`); if (command.includes(Buffer.from(executable + '\0')) && row.pid === row.processGroup) process.kill(-row.processGroup, 'SIGTERM'); } catch (error) { if (!['ENOENT', 'ESRCH'].includes(error.code)) throw error; }
    stopGroup(fixture.bus, 'SIGKILL'); await rm(fixture.root, { recursive: true, force: true });
  }
  evidence.finishedAt = new Date().toISOString(); await writeFile(join(output, 'result.json'), JSON.stringify(evidence, null, 2) + '\n', { mode: 0o600 }); console.log(`${evidence.requirement}: ${evidence.result}. Evidence: ${output}`); if (evidence.error) console.error(evidence.error);
}
