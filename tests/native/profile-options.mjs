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
const output = resolve(process.env.OPENUSAGE_NATIVE_EVIDENCE ?? join(tmpdir(), `openusage-profile-options-${randomUUID()}`));
const password = 'OpenUsage synthetic management workspace 2026';
const hash = async path => createHash('sha256').update(await readFile(path)).digest('hex');
const providers = [
  { name: 'check', file: 'synthetic-provider.py', config: 'synthetic-provider-config.json', log: 'synthetic-execution-observations.jsonl', digest: '91c56f02d6d020bcb9a29c69c00bb108c306f169849971b5c42dbcd085398080', mode: 'hang', stage: 'account-read-pending', cancel: 'data-provider-cancel', result: '.provider-diagnostic', message: 'Connection check cancelled.' },
  { name: 'quota', file: 'synthetic-quota-provider.py', config: 'synthetic-quota-provider-config.json', log: 'synthetic-quota-observations.jsonl', digest: 'cae93994fd4e64bc63f5c6ac82e973273ad7efbaf20f4055749d99c8c6cbf570', mode: 'pending', stage: 'quota-read-pending', cancel: 'data-quota-cancel', result: '.quota-result', message: 'Quota read stopped.' },
  { name: 'login', file: 'synthetic-login-provider.py', config: 'synthetic-login-provider-config.json', log: 'synthetic-login-observations.jsonl', digest: '25476e3759cbdc1cc74bcd9e46f4a01d1d85ea8ccc34425ddb45a38df27b9859', mode: 'pending', stage: 'challenge-emitted', cancel: 'data-login-cancel', result: '.login-result', message: 'Sign-in stopped' },
];
const evidence = { requirement: 'NATIVE-PROFILE-OPTIONS-001', result: 'failed', startedAt: new Date().toISOString(), scope: 'Actual native disclosure, keyboard and operation-continuity checks using three reviewed static synthetic providers. No real OAuth, existing credential homes, inference, accepted readings, release performance, screen reader or cross-platform qualification.', steps: [], screenshots: [], views: [], cases: [] };
let fixture, driver, workspace, profile, other, ownedPid;
const row = () => `[data-profile-id="${profile.id}"]`;
const options = () => `${row()} [data-profile-action="options"]`;
async function observations(provider) {
  try { return (await readFile(join(fixture.root, provider.log), 'utf8')).trim().split('\n').filter(Boolean).map(JSON.parse); }
  catch (error) { if (error.code === 'ENOENT') return []; throw error; }
}
async function liveHashes() {
  const value = {};
  for (const path of [fixture.database, `${fixture.database}-wal`, join(workspace, 'database-key.v1')]) {
    try { value[path] = await hash(path); } catch (error) { if (error.code !== 'ENOENT') throw error; }
  }
  return value;
}
async function key(value, shift = false) {
  await driver.request('POST', `/session/${driver.session}/actions`, { actions: [{ type: 'key', id: 'management-keyboard', actions: [...(shift ? [{ type: 'keyDown', value: '\uE008' }] : []), { type: 'keyDown', value }, { type: 'keyUp', value }, ...(shift ? [{ type: 'keyUp', value: '\uE008' }] : [])] }] });
  await driver.request('DELETE', `/session/${driver.session}/actions`);
}
async function optionsState() {
  return driver.execute('const trigger=document.querySelector(arguments[0]),panel=trigger?.closest("[data-profile-id]").querySelector(".profile-management[data-slot=collapsible-panel]");return {controlsMatch:trigger?.getAttribute("aria-expanded")!=="true"||trigger?.getAttribute("aria-controls")===panel?.id,expanded:trigger?.getAttribute("aria-expanded"),panelPresent:Boolean(panel),panelVisible:Boolean(panel&&panel.getClientRects().length&&getComputedStyle(panel).display!=="none"),actions:panel?[...panel.querySelectorAll("button")].map(node=>({text:node.textContent.trim(),visible:Boolean(node.getClientRects().length)})):[]};', [options()]);
}
async function manage(open) {
  assert.equal((await optionsState()).controlsMatch, true, 'An expanded trigger must control its selected profile panel.');
  if ((await optionsState()).expanded !== String(open)) await driver.click(options());
  await waitFor(async () => { const state = await optionsState(); return state.expanded === String(open) && state.panelVisible === open; }, `${open ? 'open' : 'closed'} Manage profile`);
}
async function create(label) {
  await waitFor(async () => driver.execute('return Boolean(document.querySelector("#profile-label")&&!document.querySelector("#profile-label").disabled);'), 'profile creation control');
  await driver.type('#profile-label', label);
  await driver.click('.profile-create-form button[type=submit]');
  await waitFor(async () => driver.execute('const field=document.querySelector("#profile-label");return field&&!field.disabled&&field.value==="";'), 'created profile');
  const result = await driver.invoke('list_profiles'); assert.equal(result.ok, true);
  const added = result.value.profiles.find(value => value.label === label); assert.ok(added); return added;
}
async function approve(provider) {
  const executable = join(fixture.root, provider.name + '-synthetic-provider');
  await copyFile(join(harness, 'fixtures', provider.file), executable); await chmod(executable, 0o700);
  assert.equal(await hash(executable), provider.digest);
  await writeFile(join(fixture.root, provider.config), JSON.stringify({ defaultMode: provider.mode, modesByHome: {} }) + '\n', { mode: 0o600 });
  for (const selector of ['[data-provider-settings]', '[data-provider-manual]']) {
    if (await driver.execute('return !document.querySelector(arguments[0]).closest("details").open;', [selector])) await driver.click(selector);
  }
  await driver.type('#provider-executable', executable);
  await driver.click('.provider-inspection-form button[type=submit]');
  await waitFor(async () => driver.execute('const node=document.querySelector("#provider-executable-approved");return Boolean(node&&node.getAttribute("role")==="checkbox"&&node.getAttribute("aria-disabled")!=="true"&&!node.disabled&&node.getClientRects().length);'), 'visible reviewed-source approval checkbox');
  const before = await driver.execute('const node=document.querySelector("#provider-executable-approved");return {role:node.getAttribute("role"),checked:node.getAttribute("aria-checked"),visible:Boolean(node.getClientRects().length),submitDisabled:document.querySelector(".provider-approval-form button[type=submit]").disabled,review:document.querySelector(".provider-executable-review").textContent};');
  assert.equal(before.role, 'checkbox'); assert.equal(before.checked, 'false'); assert.equal(before.visible, true); assert.equal(before.submitDisabled, true); assert.match(before.review, /fixture/i);
  await driver.click('#provider-executable-approved');
  await waitFor(async () => driver.execute('return document.querySelector("#provider-executable-approved")?.getAttribute("aria-checked")==="true"&&!document.querySelector(".provider-approval-form button[type=submit]").disabled;'), 'explicit checked source consent');
  await driver.click('.provider-approval-form button[type=submit]');
  await waitFor(async () => driver.execute('return document.activeElement?.matches(".provider-approval-result");'), 'approved-source result focus');
  assert.deepEqual(await observations(provider), [], 'Inspection and explicit approval must execute no provider.');
  await manage(false);
  return executable;
}
async function pending(provider, offset) {
  let selected;
  await waitFor(async () => {
    const rows = await observations(provider), starts = new Map(rows.filter(value => value.kind === 'start').map(value => [value.pid, value]));
    selected = rows.slice(offset).find(value => value.kind === provider.stage && starts.get(value.pid)?.environment.CODEX_HOME === join(workspace, 'profiles', profile.id));
    return Boolean(selected);
  }, `${provider.name} pending in selected synthetic profile`);
  return selected;
}
async function reaped(processRow) {
  await waitFor(() => { for (const id of [processRow.pid, -processRow.processGroup]) { try { process.kill(id, 0); return false; } catch (error) { if (error.code !== 'ESRCH') throw error; } } return true; }, 'owned provider and process group reaped');
}
async function visible(selector) {
  return driver.execute('const node=document.querySelector(arguments[0]);return Boolean(node&&!node.disabled&&node.getClientRects().length&&getComputedStyle(node).visibility!=="hidden");', [selector]);
}
async function capture(name, width = 360, scale = 1) {
  for (let attempt = 0; attempt < 4; attempt++) {
    const actual = await driver.execute('return {width:innerWidth,height:innerHeight};');
    if (actual.width === width && actual.height === 850) break;
    const rect = await driver.request('GET', `/session/${driver.session}/window/rect`);
    await driver.request('POST', `/session/${driver.session}/window/rect`, { width: rect.width + width - actual.width, height: rect.height + 850 - actual.height });
  }
  if (scale !== 1) await driver.execute('window.__managementFontStyles=[...document.querySelectorAll("body *")].map(node=>({node,style:node.getAttribute("style"),size:getComputedStyle(node).fontSize}));for(const item of window.__managementFontStyles)item.node.style.fontSize=`${parseFloat(item.size)*arguments[0]}px`;return true;', [scale]);
  try {
    await driver.execute('document.querySelector(arguments[0]).scrollIntoView({block:"center"});return true;', [row()]);
    const view = await driver.execute('const panel=document.querySelector(arguments[0]);return {width:innerWidth,height:innerHeight,scrollWidth:document.scrollingElement.scrollWidth,theme:document.documentElement.dataset.theme,bodyFont:getComputedStyle(document.body).fontFamily,overflow:[panel,...panel.querySelectorAll("*")].filter(node=>{const rect=node.getBoundingClientRect();return rect.width>1&&rect.height>1&&(rect.left< -1||rect.right>innerWidth+1);}).map(node=>({tag:node.tagName,text:node.textContent.slice(0,80)}))};', [row()]);
    assert.equal(view.width, width); assert.ok(view.scrollWidth <= width + 1); assert.deepEqual(view.overflow, []);
    await driver.screenshot(join(output, name)); evidence.screenshots.push(name); evidence.views.push({ name, textScale: scale, scalingScope: 'Test-side CSS enlargement, not OS zoom or full accessibility acceptance.', ...view });
  } finally {
    if (scale !== 1) await driver.execute('for(const item of window.__managementFontStyles){if(item.style===null)item.node.removeAttribute("style");else item.node.setAttribute("style",item.style);}delete window.__managementFontStyles;return true;');
  }
}
async function focusWindow() {
  const queue = [driver.child.pid];
  for (let count = 0; queue.length && count < 64; count++) {
    const candidate = queue.shift();
    try {
      if (await readlink(`/proc/${candidate}/exe`) === application) { ownedPid = candidate; break; }
      queue.push(...(await readFile(`/proc/${candidate}/task/${candidate}/children`, 'utf8')).trim().split(/\s+/).filter(Boolean).map(Number));
    } catch (error) { if (error.code !== 'ENOENT') throw error; }
  }
  assert.ok(ownedPid);
  evidence.nativeWindowFocus = JSON.parse(execFileSync('/usr/bin/python3', [join(harness, 'x11-close.py'), String(ownedPid), 'focus'], { env: fixture.environment, encoding: 'utf8', timeout: 10000 }));
  await waitFor(async () => driver.execute('return document.hasFocus();'), 'owned native keyboard focus');
}

await mkdir(output, { recursive: true, mode: 0o700 });
process.once('exit', () => { driver?.kill('SIGKILL'); stopGroup(fixture?.bus, 'SIGKILL'); });
try {
  evidence.application = { path: application, sha256: await hash(application) };
  evidence.sourceCommit = execFileSync('git', ['rev-parse', 'HEAD'], { cwd: repository, encoding: 'utf8' }).trim();
  evidence.sourceHashes = {};
  const sources = execFileSync('rg', ['--files', 'apps/desktop/src', 'apps/desktop/src-tauri/src', 'apps/desktop/src-tauri/permissions', 'apps/desktop/public/fonts', 'crates/openusage-codex/src', 'crates/openusage-storage/src'], { cwd: repository, encoding: 'utf8' }).trim().split('\n');
  for (const path of [...sources, 'Cargo.toml', 'Cargo.lock', 'package.json', 'package-lock.json', 'apps/desktop/index.html', 'apps/desktop/src-tauri/Cargo.toml', 'apps/desktop/src-tauri/build.rs', 'apps/desktop/src-tauri/tauri.conf.json', 'apps/desktop/src-tauri/capabilities/main-window.json']) evidence.sourceHashes[path] = await hash(join(repository, path));
  evidence.harnessHashes = {};
  for (const path of ['profile-options.mjs', 'private-fixture.mjs', 'webdriver.mjs', 'x11-close.py', ...providers.map(value => 'fixtures/' + value.file)]) evidence.harnessHashes[path] = await hash(join(harness, path));
  fixture = await createFixture('profile-options'); workspace = dirname(fixture.database);
  driver = new NativeDriver(application, fixture.environment, join(output, 'driver.log')); await driver.start();
  await waitFor(async () => driver.execute('return ["workspace-passphrase","workspace-confirmation"].every(id=>{const node=document.getElementById(id);return Boolean(node&&!node.disabled);});'), 'enabled setup fields');
  await driver.type('#workspace-passphrase', password); await driver.type('#workspace-confirmation', password); await driver.click('.unlock-form button[type=submit]');
  await waitFor(async () => (await driver.invoke('bootstrap')).value?.storage === 'ready', 'synthetic workspace setup');
  profile = await create('Selected synthetic management profile'); other = await create('Other synthetic management profile');
  await focusWindow();
  await driver.selectVisibleOption('#theme', 'Light');
  await waitFor(async () => driver.execute('return document.documentElement.dataset.theme==="light"&&!document.querySelector("#theme").disabled;'), 'saved light appearance');
  for (const provider of providers) {
    provider.executable = await approve(provider);
    const idle = await driver.execute('const row=document.querySelector(arguments[0]);return [...row.querySelectorAll("button")].filter(node=>node.getClientRects().length&&!node.disabled).map(node=>node.textContent.trim());', [row()]);
    assert.deepEqual(idle, ['Sign in', 'Read quotas', 'Manage profile']);
    const state = await optionsState(); assert.equal(state.panelPresent, true); assert.equal(state.panelVisible, false); assert.ok(state.actions.length >= 3 && state.actions.every(value => !value.visible));
    const before = await liveHashes(), offset = (await observations(provider)).length;
    if (provider.name === 'check') { await manage(true); await driver.click(`${row()} [data-provider-check]`); }
    if (provider.name === 'quota') await driver.click(`${row()} [data-quota-read]`);
    if (provider.name === 'login') {
      await driver.click(`${row()} [data-login-start]`);
      await waitFor(async () => visible(`${row()} [data-login-consent] button[type=submit]`), 'explicit profile consent');
      assert.equal((await observations(provider)).length, offset, 'Sign-in consent alone must execute nothing.');
      await driver.click(`${row()} [data-login-consent] button[type=submit]`);
    }
    const processRow = await pending(provider, offset), cancel = `${row()} [${provider.cancel}="${profile.id}"]`;
    await waitFor(async () => visible(cancel), `${provider.name} visible Cancel`);
    await manage(true); await manage(false);
    await key(' '); await waitFor(async () => (await optionsState()).panelVisible, 'keyboard reopen during pending operation');
    await key('\uE007'); await waitFor(async () => !(await optionsState()).panelVisible, 'keyboard close during pending operation');
    assert.equal(await visible(cancel), true, 'Closing Manage must keep active Cancel visible.');
    process.kill(processRow.pid, 0);
    const beforeCancel = await observations(provider);
    assert.equal(beforeCancel.slice(offset).filter(value => value.kind === 'start' && value.arguments[0] === 'app-server').length, 1, 'Disclosure must not replace the active provider process.');
    assert.equal(beforeCancel.filter(value => value.kind === 'request' && value.request.method === 'account/login/cancel').length, 0, 'Disclosure must not cancel a sign-in.');
    assert.equal(await driver.execute('return Boolean(document.querySelector(arguments[0]));', [`[data-profile-id="${other.id}"] [${provider.cancel}]`]), false, 'Cancel must belong only to the selected profile.');
    await capture(`management-${provider.name}-pending-light-360.png`);
    const focusSnapshot = () => driver.execute('const node=document.activeElement;return {documentFocused:document.hasFocus(),tag:node?.tagName,text:node?.textContent.trim().slice(0,100),disabled:node?.disabled??null,ariaDisabled:node?.getAttribute("aria-disabled"),manage:node?.matches(arguments[0]),cancel:node?.matches(arguments[1])};', [options(), cancel]);
    const keyboardFocus = { operation: provider.name, afterResize: await focusSnapshot() };
    await focusWindow();
    keyboardFocus.afterOwnedWindowFocus = await focusSnapshot();
    await driver.execute('window.__managementKeyTrace=[];window.__managementKeyListener=event=>{const item={type:event.type,key:event.key,shift:event.shiftKey,target:event.target?.textContent.trim().slice(0,60)};queueMicrotask(()=>window.__managementKeyTrace.push({...item,prevented:event.defaultPrevented,active:document.activeElement?.textContent.trim().slice(0,60)}));};for(const name of ["keydown","keyup","focusin"])window.addEventListener(name,window.__managementKeyListener,true);return true;');
    await key('\uE004', true);
    keyboardFocus.immediatelyAfterReverseTab = await focusSnapshot();
    try { await waitFor(async () => driver.execute('return document.activeElement?.matches(arguments[0]);', [cancel]), 'settled keyboard focus on active Cancel', 1000); }
    catch (error) { keyboardFocus.focusWaitError = error.message; }
    keyboardFocus.afterReverseTab = await focusSnapshot();
    keyboardFocus.events = await driver.execute('const events=window.__managementKeyTrace;for(const name of ["keydown","keyup","focusin"])window.removeEventListener(name,window.__managementKeyListener,true);delete window.__managementKeyTrace;delete window.__managementKeyListener;return events;');
    evidence.keyboardFocusChecks ??= []; evidence.keyboardFocusChecks.push(keyboardFocus);
    assert.equal(keyboardFocus.afterReverseTab.cancel, true, 'Reverse Tab must reach the active Cancel while Manage is closed.');
    await key('\uE007');
    await waitFor(async () => driver.execute('const result=document.querySelector(arguments[0]);return Boolean(result&&result.getClientRects().length&&result.textContent.includes(arguments[1]));', [`${row()} ${provider.result}`, provider.message]), 'visible terminal result');
    await reaped(processRow);
    assert.deepEqual(await liveHashes(), before, 'Provider read/cancel and disclosure must preserve database, WAL, and key bytes.');
    evidence.cases.push({ operation: provider.name, selectedProfileId: profile.id, processId: processRow.pid, processGroup: processRow.processGroup, disclosurePreservedOwnedProcess: true, closedControlsHidden: true, actualKeyboardToggle: true, reverseTabReachesActiveCancel: true, cancelVisibleOutsidePanel: true, keyboardCancelReapedProcess: true, storageHashesPreserved: true });
  }
  await capture('management-idle-light-desktop.png', 1280); await capture('management-idle-light-360.png'); await manage(true); await capture('management-expanded-light-360.png'); await capture('management-expanded-light-text200-360.png', 360, 2); await manage(false);
  assert.equal(await readFile(join(fixture.dirs.codex, 'synthetic-fixture.txt'), 'utf8'), CODEX_SENTINEL); assert.deepEqual(await readdir(fixture.dirs.codex), ['synthetic-fixture.txt']);
  for (const provider of providers) { assert.equal(await hash(provider.executable), provider.digest); for (const start of (await observations(provider)).filter(value => value.kind === 'start')) { assert.equal(start.environment.CODEX_HOME, join(workspace, 'profiles', profile.id)); assert.deepEqual(start.unexpectedEnvironmentKeys, []); await reaped(start); } }
  evidence.steps.push({ threeIdleActions: true, explicitSourceConsentThroughVisibleRoleCheckbox: true, inspectionExecutesNothing: true, maintenanceDisclosurePreservesAllThreeOperationKinds: true, noExistingCredentialHomes: true });
  assert.equal(await hash(application), evidence.application.sha256);
  for (const [path, digest] of Object.entries(evidence.sourceHashes)) assert.equal(await hash(join(repository, path)), digest, `Source drift: ${path}`);
  for (const [path, digest] of Object.entries(evidence.harnessHashes)) assert.equal(await hash(join(harness, path)), digest, `Harness drift: ${path}`);
  evidence.nativeClose = JSON.parse(execFileSync('/usr/bin/python3', [join(harness, 'x11-close.py'), String(ownedPid)], { env: fixture.environment, encoding: 'utf8', timeout: 10000 }));
  await waitFor(async () => { try { await readlink(`/proc/${ownedPid}/exe`); return false; } catch (error) { if (error.code === 'ENOENT') return true; throw error; } }, 'genuine native exit');
  driver.session = undefined; await driver.stop(); driver = undefined; evidence.result = 'passed';
} catch (error) {
  evidence.error = error.stack ?? String(error); process.exitCode = 1;
  if (driver?.session) try { await driver.screenshot(join(output, 'failure.png')); evidence.screenshots.push('failure.png'); } catch {}
} finally {
  try { await driver?.stop(); } catch (error) { evidence.cleanupError = error.message; evidence.result = 'failed'; process.exitCode = 1; }
  if (fixture) {
    evidence.observations = {};
    for (const provider of providers) {
      const rows = await observations(provider); evidence.observations[provider.name] = rows;
      if (evidence.result !== 'passed' && provider.executable) for (const start of rows.filter(value => value.kind === 'start')) {
        try { const command = await readFile(`/proc/${start.pid}/cmdline`); if (start.pid === start.processGroup && command.includes(Buffer.from(provider.executable + '\0'))) process.kill(-start.processGroup, 'SIGTERM'); }
        catch (error) { if (!['ENOENT', 'ESRCH'].includes(error.code)) throw error; }
      }
    }
    stopGroup(fixture.bus, 'SIGKILL'); await rm(fixture.root, { recursive: true, force: true });
  }
  evidence.finishedAt = new Date().toISOString(); await writeFile(join(output, 'result.json'), JSON.stringify(evidence, null, 2) + '\n', { mode: 0o600 });
  console.log(`${evidence.requirement}: ${evidence.result}. Evidence: ${output}`); if (evidence.error) console.error(evidence.error);
}
