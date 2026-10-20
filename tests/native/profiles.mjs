import assert from 'node:assert/strict';
import { execFile, execFileSync } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { lstat, mkdir, readFile, readdir, readlink, rm, rmdir, symlink, unlink, writeFile } from 'node:fs/promises';
import { arch, platform, release, tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { promisify } from 'node:util';
import { CODEX_SENTINEL, createFixture, stopGroup } from './private-fixture.mjs';
import { pasteNative } from './clipboard.mjs';
import { NativeDriver, waitFor } from './webdriver.mjs';

const harness = dirname(fileURLToPath(import.meta.url));
const repository = resolve(process.env.OPENUSAGE_NATIVE_SOURCE_ROOT ?? join(harness, '../..'));
const application = resolve(process.env.OPENUSAGE_NATIVE_BINARY ?? join(repository, 'target/debug/openusage-desktop'));
const output = resolve(process.env.OPENUSAGE_NATIVE_EVIDENCE ?? join(tmpdir(), `openusage-profiles-evidence-${randomUUID()}`));
const passphrase = 'OpenUsage synthetic profiles passphrase 2026';
const run = promisify(execFile);
const hash = async path => createHash('sha256').update(await readFile(path)).digest('hex');
const evidence = {
  requirement: 'NATIVE-PROFILES-001', startedAt: new Date().toISOString(), result: 'failed',
  scope: 'Actual native profile creation, rename, archive, restart, and slot reuse with synthetic data. GUI actions create and change records; direct production IPC checks state and rejects invalid requests. No provider login, collection, remote history, or cross-platform acceptance.',
  platform: { os: platform(), release: release(), architecture: arch() },
  steps: [], screenshots: [], views: [],
};
let fixture;
let driver;
let sequence = 0;
let homes;
const homeMarkers = new Map();

async function openApp() {
  driver = new NativeDriver(application, fixture.environment, join(output, `driver-${++sequence}.log`));
  await driver.start();
  await waitFor(async () => !['Checking local storage…', undefined].includes(await driver.execute('return document.querySelector("#startup-heading")?.textContent;')), 'native storage state');
}

async function closeApp() {
  if (!driver) return;
  try { await driver.stop(); } finally { driver = undefined; }
}

async function rejected(command, args, error) {
  assert.deepEqual(await driver.invoke(command, args), { ok: false, error });
}

async function list() {
  const response = await driver.invoke('list_profiles');
  assert.equal(response.ok, true, 'Native profile metadata must be available after unlock.');
  assert.deepEqual(Object.keys(response.value).sort(), ['activeProfiles', 'profiles']);
  const { profiles, activeProfiles } = response.value;
  assert.equal(activeProfiles, profiles.filter(profile => !profile.archived).length);
  assert.ok(activeProfiles <= 10);
  for (const profile of profiles) {
    assert.deepEqual(Object.keys(profile).sort(), ['archived', 'connectionState', 'environment', 'id', 'label']);
    assert.match(profile.id, /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/);
    assert.equal(profile.environment, 'native');
    assert.equal(profile.connectionState, 'authentication_required');
  }
  return { profiles, activeProfiles };
}

const sorted = state => ({ activeProfiles: state.activeProfiles, profiles: [...state.profiles].sort((left, right) => left.id.localeCompare(right.id)) });

async function homeNames() {
  try { return (await readdir(homes)).sort(); } catch (error) { if (error.code === 'ENOENT') return []; throw error; }
}

async function privateHomes(state) {
  const root = await lstat(homes);
  assert.ok(root.isDirectory() && !root.isSymbolicLink());
  assert.equal(root.mode & 0o777, 0o700);
  assert.deepEqual(await homeNames(), state.profiles.map(profile => profile.id).sort());
  for (const profile of state.profiles) {
    const home = join(homes, profile.id);
    const metadata = await lstat(home);
    assert.ok(metadata.isDirectory() && !metadata.isSymbolicLink());
    assert.equal(metadata.mode & 0o777, 0o700);
    const marker = homeMarkers.get(profile.id);
    assert.deepEqual(await readdir(home), marker ? ['synthetic-profile-marker.txt'] : [], 'Profile homes must contain only explicit synthetic fixture data.');
    if (marker) {
      assert.equal(await hash(join(home, 'synthetic-profile-marker.txt')), marker);
      assert.equal((await lstat(join(home, 'synthetic-profile-marker.txt'))).mode & 0o777, 0o600);
    }
  }
}

async function replaceValue(selector, value) {
  await driver.click(selector);
  await driver.request('POST', `/session/${driver.session}/actions`, { actions: [{ type: 'key', id: 'profile-replace', actions: [{ type: 'keyDown', value: '\uE009' }, { type: 'keyDown', value: 'a' }, { type: 'keyUp', value: 'a' }, { type: 'keyUp', value: '\uE009' }, { type: 'keyDown', value: '\uE003' }, { type: 'keyUp', value: '\uE003' }] }] });
  await driver.request('DELETE', `/session/${driver.session}/actions`);
  await waitFor(async () => driver.execute('return document.querySelector(arguments[0])?.value==="";', [selector]), 'controlled profile field cleared through keyboard input');
  const element = await driver.element(selector);
  await driver.request('POST', `/session/${driver.session}/element/${element}/value`, { text: value });
  await waitFor(async () => driver.execute('return document.querySelector(arguments[0])?.value===arguments[1];', [selector, value]), 'exact replacement profile text');
}

const rowSelector = id => `[data-profile-id="${id}"]`;
const optionsSelector = id => `${rowSelector(id)} [data-profile-action="options"]`;
async function optionsState(id) {
  return driver.execute('const trigger=document.querySelector(arguments[0]);if(!trigger)return null;const panel=trigger.closest("[data-profile-id]").querySelector(".profile-management[data-slot=collapsible-panel]");return {controlsMatch:trigger.getAttribute("aria-expanded")!=="true"||trigger.getAttribute("aria-controls")===panel?.id,expanded:trigger.getAttribute("aria-expanded"),panelPresent:Boolean(panel),panelVisible:Boolean(panel&&panel.getClientRects().length&&getComputedStyle(panel).display!=="none"),hiddenActions:panel?[...panel.querySelectorAll("button")].map(node=>({label:node.textContent.trim(),visible:Boolean(node.getClientRects().length)})):[]};', [optionsSelector(id)]);
}
async function manage(id, open = true) {
  const state = await optionsState(id);
  assert.ok(state?.panelPresent, 'Manage must retain its hidden panel.');
  assert.equal(state.controlsMatch, true, 'An expanded trigger must control its own profile panel.');
  if (state.expanded !== String(open)) await driver.click(optionsSelector(id));
  await waitFor(async () => { const next = await optionsState(id); return next?.expanded === String(open) && next.panelVisible === open; }, `${open ? 'expanded' : 'collapsed'} management controls`);
}
async function key(value, shift = false) {
  await driver.request('POST', `/session/${driver.session}/actions`, { actions: [{ type: 'key', id: 'profile-keyboard', actions: [...(shift ? [{ type: 'keyDown', value: '\uE008' }] : []), { type: 'keyDown', value }, { type: 'keyUp', value }, ...(shift ? [{ type: 'keyUp', value: '\uE008' }] : [])] }] });
  await driver.request('DELETE', `/session/${driver.session}/actions`);
}
async function focusOwnedWindow() {
  const queue = [driver.child.pid]; let pid;
  for (let count = 0; queue.length && count < 64; count++) {
    const candidate = queue.shift();
    try {
      if (await readlink(`/proc/${candidate}/exe`) === application) { pid = candidate; break; }
      queue.push(...(await readFile(`/proc/${candidate}/task/${candidate}/children`, 'utf8')).trim().split(/\s+/).filter(Boolean).map(Number));
    } catch (error) { if (error.code !== 'ENOENT') throw error; }
  }
  assert.ok(pid, 'Keyboard setup must target an owned application descendant.');
  evidence.nativeWindowFocus = JSON.parse(execFileSync('/usr/bin/python3', [join(harness, 'x11-close.py'), String(pid), 'focus'], { env: fixture.environment, encoding: 'utf8', timeout: 10000 }));
  await waitFor(async () => driver.execute('return document.hasFocus();'), 'owned native keyboard focus');
}
async function storageHashes() {
  const value = {};
  for (const path of [fixture.database, `${fixture.database}-wal`, join(dirname(fixture.database), 'database-key.v1')]) {
    try { value[path] = await hash(path); } catch (error) { if (error.code !== 'ENOENT') throw error; }
  }
  return value;
}

async function create(label, unicode = false) { await driver.openProfileCreate();
  const before = await list();
  if (unicode) {
    assert.ok(Number(await driver.execute('return document.querySelector("#profile-label").maxLength;')) >= label.length);
    await pasteNative(driver, fixture, '#profile-label', label);
  } else {
    await replaceValue('#profile-label', label);
  }
  await driver.click('.profile-create-form button[type="submit"]');
  await waitFor(async () => (await list()).profiles.length === before.profiles.length + 1, 'profile created through the native GUI');
  const state = await list();
  const added = state.profiles.find(profile => !before.profiles.some(previous => previous.id === profile.id));
  assert.equal(added?.label, label);
  assert.equal(added.archived, false);
  await waitFor(async () => Boolean(await driver.execute('return document.querySelector(arguments[0]);', [`[data-profile-id="${added.id}"]`])), 'created profile row');
  await waitFor(async () => (await driver.execute('return document.querySelector("#profile-label").disabled;')) === false, 'profile creation controls');
  await privateHomes(state);
  return added;
}

async function unlock() {
  await waitFor(async () => Boolean(await driver.execute('return document.querySelector("#workspace-passphrase");')), 'workspace passphrase input');
  await driver.type('#workspace-passphrase', passphrase);
  await driver.click('.unlock-form button[type="submit"]');
  await waitFor(async () => (await driver.invoke('bootstrap')).value?.storage === 'ready', 'native workspace unlock');
  await waitFor(async () => Boolean(await driver.execute('return document.querySelector(".profile-create-form");')), 'unlocked profile controls');
}

async function screenshot(name, narrow = false, theme = 'dark') {
  await driver.setAppearance(theme);
  await waitFor(async () => (await driver.execute('return document.documentElement.dataset.theme;')) === theme, `${theme} appearance`);
  const target = narrow ? { width: 360, height: 800 } : { width: 1280, height: 850 };
  for (let attempt = 0; attempt < 3; attempt++) {
    const viewport = await driver.execute('return {width:innerWidth,height:innerHeight};');
    if (viewport.width === target.width && viewport.height === target.height) break;
    const rect = await driver.request('GET', `/session/${driver.session}/window/rect`);
    await driver.request('POST', `/session/${driver.session}/window/rect`, { width: rect.width + target.width - viewport.width, height: rect.height + target.height - viewport.height });
    await driver.request('POST', `/session/${driver.session}/execute/async`, { script: 'const done=arguments[arguments.length-1];requestAnimationFrame(()=>requestAnimationFrame(()=>done(true)));', args: [] });
  }
  await driver.execute('document.querySelector(".accounts-section").scrollIntoView({block:"start"});return true;');
  const view = await driver.execute('const nodes=[...document.querySelectorAll(".accounts-section *")].filter(node=>{const rect=node.getBoundingClientRect();return rect.width>1&&rect.height>1&&getComputedStyle(node).visibility!=="hidden";});return {width:innerWidth,height:innerHeight,scrollWidth:document.scrollingElement.scrollWidth,bodyFont:getComputedStyle(document.body).fontFamily,overflowing:nodes.filter(node=>{const rect=node.getBoundingClientRect();return rect.right>innerWidth+1||rect.left < -1;}).map(node=>({tag:node.tagName,className:node.className})),clipped:nodes.filter(node=>{const style=getComputedStyle(node);return node.textContent.trim()&&(( ["hidden","clip"].includes(style.overflowX)&&node.scrollWidth>node.clientWidth+2)||(["hidden","clip"].includes(style.overflowY)&&node.scrollHeight>node.clientHeight+2));}).map(node=>({tag:node.tagName,className:node.className})),metrics:[...document.querySelectorAll(".metric")].map(card=>({label:card.querySelector(".metric-label")?.textContent.trim(),value:card.querySelector(".metric-value")?.textContent.trim()}))};');
  assert.equal(view.width, target.width);
  assert.equal(view.height, target.height);
  assert.ok(view.scrollWidth <= view.width + 1, 'Profile controls must not cause horizontal overflow.');
  assert.deepEqual(view.overflowing, [], 'Profile controls must remain inside the viewport.');
  assert.deepEqual(view.clipped, [], 'Profile containers must not clip text.');
  for (const label of ['Quota remaining', 'Token activity']) assert.equal(view.metrics.find(metric => metric.label?.includes(label))?.value, 'Not available');
  await driver.screenshot(join(output, name));
  evidence.screenshots.push(name);
  evidence.views.push({ screenshot: name, theme, ...view });
}

await mkdir(output, { recursive: true, mode: 0o700 });
process.once('exit', () => { driver?.kill('SIGKILL'); stopGroup(fixture?.bus, 'SIGKILL'); });
try {
  assert.equal(platform(), 'linux');
  evidence.application = { path: application, sha256: await hash(application), commit: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: repository, encoding: 'utf8' }).trim() };
  evidence.sourceDirtyPaths = execFileSync('git', ['status', '--porcelain'], { cwd: repository, encoding: 'utf8' }).trim().split('\n').filter(Boolean);
  evidence.harnessHashes = {};
  for (const path of ['profiles.mjs', 'clipboard.mjs', 'private-fixture.mjs', 'webdriver.mjs', 'x11-close.py']) evidence.harnessHashes[path] = await hash(join(harness, path));
  evidence.sourceHashes = {};
  for (const path of ['Cargo.lock', 'apps/desktop/src-tauri/Cargo.toml', 'apps/desktop/src-tauri/tauri.conf.json', 'apps/desktop/src-tauri/build.rs', 'apps/desktop/src-tauri/capabilities/main-window.json', 'apps/desktop/src-tauri/src/main.rs', 'apps/desktop/src-tauri/src/workspace.rs', 'apps/desktop/src/main.tsx', 'apps/desktop/src/profiles.tsx', 'apps/desktop/src/profile-actions.css', 'apps/desktop/src/components/ui/collapsible.tsx', 'apps/desktop/src/components/ui/collapsible.css', 'apps/desktop/src/components/ui/field.tsx', 'apps/desktop/src/components/ui/input.tsx', 'apps/desktop/src/components/ui/button.tsx', 'apps/desktop/src/components/ui/selection.css', 'apps/desktop/index.html', 'apps/desktop/public/fonts/JetBrainsMono-Regular.woff2', 'apps/desktop/public/fonts/JetBrainsMono-Medium.woff2', 'apps/desktop/src/styles.css', 'apps/desktop/src/contracts.ts', 'crates/openusage-storage/src/lib.rs', 'crates/openusage-storage/src/profiles.rs']) evidence.sourceHashes[path] = await hash(join(repository, path));
  fixture = await createFixture('profiles');
  homes = join(dirname(fixture.database), 'profiles');
  await openApp();
  for (const [command, args] of [['list_profiles', {}], ['create_profile', { label: 'Locked synthetic profile' }], ['rename_profile', { id: randomUUID(), label: 'Locked rename' }], ['set_profile_archived', { id: randomUUID(), archived: true }]]) await rejected(command, args, 'workspace_locked');
  assert.deepEqual(await homeNames(), []);
  await waitFor(async () => driver.execute('return ["workspace-passphrase","workspace-confirmation"].every(id=>{const field=document.getElementById(id);return Boolean(field&&!field.disabled);});'), 'enabled setup passphrase fields');
  await driver.type('#workspace-passphrase', passphrase);
  await driver.type('#workspace-confirmation', passphrase);
  await driver.click('.unlock-form button[type="submit"]');
  await waitFor(async () => (await driver.invoke('bootstrap')).value?.storage === 'ready', 'private workspace creation');
  assert.deepEqual(await list(), { profiles: [], activeProfiles: 0 });
  evidence.steps.push({ lockedProfileCommandsRejected: true, setupThroughGui: true });
  await focusOwnedWindow();

  for (const [command, args, error] of [
    ['create_profile', { label: ' '.repeat(1024) + 'x' }, 'invalid_profile_label'],
    ['create_profile', { label: 'x'.repeat(101) }, 'invalid_profile_label'],
    ['create_profile', { label: 'Synthetic profile', path: '../unsafe' }, 'invalid_arguments'],
    ['rename_profile', { id: '../unsafe', label: 'Synthetic rename' }, 'invalid_profile_id'],
    ['set_profile_archived', { id: randomUUID(), archived: 'true' }, 'invalid_arguments'],
  ]) await rejected(command, args, error);
  assert.deepEqual(await list(), { profiles: [], activeProfiles: 0 });
  assert.deepEqual(await homeNames(), []);

  try { assert.deepEqual(await readdir(homes), []); await rmdir(homes); } catch (error) { if (error.code !== 'ENOENT') throw error; }
  await writeFile(homes, 'Synthetic profile-path marker. Preserve this file.\n', { mode: 0o600, flag: 'wx' });
  const markerHash = await hash(homes);
  await rejected('create_profile', { label: 'File fault synthetic profile' }, 'profile_unavailable');
  assert.equal(await hash(homes), markerHash);
  assert.deepEqual(await list(), { profiles: [], activeProfiles: 0 });
  await unlink(homes);
  const outside = join(fixture.root, 'external-synthetic-profile-root');
  await mkdir(outside, { mode: 0o700 });
  const sentinel = join(outside, 'preserve-synthetic-marker.txt');
  await writeFile(sentinel, 'External synthetic marker. No provider data.\n', { mode: 0o600 });
  const sentinelHash = await hash(sentinel);
  await symlink(outside, homes);
  await rejected('create_profile', { label: 'Symlink fault synthetic profile' }, 'profile_unavailable');
  assert.equal((await lstat(homes)).isSymbolicLink(), true);
  assert.equal(await hash(sentinel), sentinelHash);
  assert.deepEqual(await readdir(outside), ['preserve-synthetic-marker.txt']);
  assert.deepEqual(await list(), { profiles: [], activeProfiles: 0 });
  await unlink(homes);
  evidence.steps.push({ rawLabelAndMalformedArgumentsRejected: true, faultInjections: ['Synthetic profiles path is a regular file.', 'Synthetic profiles path is a symlink to another synthetic directory.'], markerAndExternalSentinelPreserved: true, rejectedFaultsCreatedNoRecords: true });

  const first = await create('😀'.repeat(100), true);
  evidence.unicodeInput = { unicodeCharacters: 100, utf16Units: 200, utf8Bytes: 400, route: 'Real GTK clipboard ownership and native Control+V on the explicitly private Xvfb display.' };
  for (let number = 2; number <= 10; number++) await create(`Synthetic profile ${number}`);
  const full = await list();
  assert.equal(full.activeProfiles, 10);
  const marker = join(homes, first.id, 'synthetic-profile-marker.txt');
  await writeFile(marker, 'Explicit synthetic profile-home marker for archive and restore retention.\n', { mode: 0o600, flag: 'wx' });
  homeMarkers.set(first.id, await hash(marker));
  evidence.steps.push({ freshProfileHomesEmpty: true, syntheticHomeMarkerAddedForArchiveRetention: true });
  const namesBefore = await homeNames();
  await replaceValue('#profile-label', 'Eleventh GUI synthetic profile');
  await driver.click('.profile-create-form button[type="submit"]');
  await waitFor(async () => Boolean(await driver.execute('return document.querySelector(".profile-create-form #profiles-error")?.textContent.includes("All 10 active profile slots");')), 'visible eleventh-profile capacity error');
  await waitFor(async () => (await driver.execute('return document.activeElement?.id;')) === 'profiles-error', 'focus after eleventh-profile refusal');
  assert.notEqual(await driver.execute('return document.querySelector("#profile-label").getAttribute("aria-invalid");'), 'true');
  assert.deepEqual(sorted(await list()), sorted(full));
  assert.deepEqual(await homeNames(), namesBefore);
  await rejected('create_profile', { label: 'Eleventh synthetic profile' }, 'profile_capacity_reached');
  assert.deepEqual(sorted(await list()), sorted(full));
  assert.deepEqual(await homeNames(), namesBefore);
  const keyboardProfile = full.profiles[0], nextProfile = full.profiles[1];
  const collapsed = await optionsState(keyboardProfile.id);
  assert.equal(collapsed.expanded, 'false');
  assert.equal(collapsed.panelVisible, false);
  assert.ok(collapsed.hiddenActions.length >= 2 && collapsed.hiddenActions.every(action => !action.visible));
  const beforeDisclosure = await storageHashes();
  await manage(keyboardProfile.id);
  await focusOwnedWindow();
  await key('\uE007');
  await waitFor(async () => (await optionsState(keyboardProfile.id)).expanded === 'false', 'Enter closes Manage');
  await key('\uE004');
  assert.equal(await driver.execute('return document.activeElement?.matches(arguments[0]);', [optionsSelector(nextProfile.id)]), true, 'Tab must skip hidden Rename and Archive and reach the next profile.');
  await key('\uE004', true);
  assert.equal(await driver.execute('return document.activeElement?.matches(arguments[0]);', [optionsSelector(keyboardProfile.id)]), true);
  await key(' ');
  await waitFor(async () => (await optionsState(keyboardProfile.id)).panelVisible, 'Space opens Manage');
  await manage(keyboardProfile.id, false);
  assert.deepEqual(await storageHashes(), beforeDisclosure, 'Disclosure and keyboard traversal must not write database, WAL, or key.');
  await manage(keyboardProfile.id);
  await screenshot('profile-management-light-360.png', true, 'light');
  await manage(keyboardProfile.id, false);
  evidence.steps.push({ namedManageControlRetainsHiddenPanel: true, collapsedActionsNotVisible: true, enterAndSpaceToggleActualDisclosure: true, tabSkipsClosedMaintenanceActions: true, disclosurePreservesStorageHashes: true });
  await screenshot('ten-profiles-dark.png');
  await screenshot('ten-profiles-light-360.png', true, 'light');
  await screenshot('ten-profiles-dark-360.png', true);

  const row = `[data-profile-id="${first.id}"]`;
  await manage(first.id);
  await driver.click(`${row} button[aria-label="Rename profile"]`);
  await replaceValue(`${row} .profile-rename-form input[name="profile-label"]`, 'Renamed synthetic profile');
  await driver.click(`${row} .profile-rename-form button[type="submit"]`);
  await waitFor(async () => (await list()).profiles.find(profile => profile.id === first.id)?.label === 'Renamed synthetic profile', 'profile renamed through GUI');
  await waitFor(async () => Boolean(await driver.execute('return document.activeElement?.matches(arguments[0]);', [`${row} [data-profile-action="options"]`])), 'focus after profile rename');
  await manage(first.id);
  await driver.click(`${row} button[aria-label="Archive profile"]`);
  await waitFor(async () => (await list()).profiles.find(profile => profile.id === first.id)?.archived === true, 'profile archived through GUI');
  const archived = await list();
  assert.equal(archived.activeProfiles, 9);
  await privateHomes(archived);
  await closeApp();
  await openApp();
  await rejected('list_profiles', {}, 'workspace_locked');
  await unlock();
  assert.deepEqual(sorted(await list()), sorted(archived));
  const reused = await create('Reused synthetic profile slot');
  assert.equal(namesBefore.includes(reused.id), false);
  const reusedState = await list();
  assert.equal(reusedState.activeProfiles, 10);
  assert.equal(reusedState.profiles.length, 11);
  await rejected('set_profile_archived', { id: first.id, archived: false }, 'profile_capacity_reached');
  assert.deepEqual(sorted(await list()), sorted(reusedState));
  const another = reusedState.profiles.find(profile => !profile.archived && profile.id !== reused.id);
  await manage(another.id);
  await driver.click(`[data-profile-id="${another.id}"] [data-profile-action="archive"]`);
  await waitFor(async () => (await list()).activeProfiles === 9, 'another slot archived before GUI restore');
  if (!await driver.execute('return document.querySelector(".archived-profiles").open;')) await driver.click('.archived-profiles > summary');
  await driver.click(`${row} [data-profile-action="restore"]`);
  await waitFor(async () => (await list()).profiles.find(profile => profile.id === first.id)?.archived === false, 'profile restored through GUI');
  await waitFor(async () => Boolean(await driver.execute('return document.activeElement?.matches(arguments[0]);', [`${row} [data-profile-action="options"]`])), 'focus after profile restore');
  assert.match(await driver.execute('return document.querySelector(".profile-status").textContent;'), /Profile restored\. 10 of 10 active\./);
  const final = await list();
  assert.equal(final.activeProfiles, 10);
  assert.equal(final.profiles.length, 11);
  assert.equal(final.profiles.find(profile => profile.id === another.id)?.archived, true);
  assert.deepEqual(final.profiles.map(profile => profile.id).sort(), reusedState.profiles.map(profile => profile.id).sort());
  await privateHomes(final);
  await screenshot('reused-profile-slot-dark-360.png', true);
  await screenshot('restored-profiles-light.png', false, 'light');
  await closeApp();
  await openApp();
  await unlock();
  assert.deepEqual(sorted(await list()), sorted(final));
  await privateHomes(final);
  const native = await driver.invoke('bootstrap');
  assert.equal(native.value.activeProfiles, 10);
  evidence.finalProfileView = final;
  const database = await readFile(fixture.database);
  assert.equal(database.includes(Buffer.from('SQLite format 3\0')), false);
  assert.equal(database.includes(Buffer.from('Renamed synthetic profile')), false);
  assert.equal((await lstat(fixture.database)).mode & 0o777, 0o600);
  await run('python3', ['-I', '-c', 'import sqlite3,sys\ntry:\n c=sqlite3.connect("file:"+sys.argv[1]+"?mode=ro",uri=True);c.execute("SELECT count(*) FROM sqlite_schema").fetchone()\nexcept sqlite3.DatabaseError:\n sys.exit(0)\nsys.exit(1)', fixture.database], { timeout: 5000 });
  assert.deepEqual(await readdir(fixture.dirs.codex), ['synthetic-fixture.txt']);
  assert.equal(await readFile(join(fixture.dirs.codex, 'synthetic-fixture.txt'), 'utf8'), CODEX_SENTINEL);
  evidence.steps.push({ tenProfilesCreatedThroughGui: true, eleventhRejectedThroughGuiAndNativeIpc: true, renameAndArchiveThroughGui: true, appRestartPreservedProfiles: true, archivedSlotReusedThroughGui: true, restoreAtCapacityRejectedThroughNativeIpc: true, successfulRestoreThroughGui: true, renameAndRestoreFocusAndStatus: true, encryptedSqliteAndPrivateHomes: true, freshProfilesRequireExplicitAuthentication: true, managedHomesContainOnlyExplicitSyntheticMarker: true, syntheticHomeMarkerPreservedAcrossArchiveAndRestore: true, syntheticCodexHomeUnchanged: true });
  assert.equal(await hash(application), evidence.application.sha256);
  for (const [path, digest] of Object.entries(evidence.sourceHashes)) assert.equal(await hash(join(repository, path)), digest, `Source changed during native profile run: ${path}`);
  for (const [path, digest] of Object.entries(evidence.harnessHashes)) assert.equal(await hash(join(harness, path)), digest, `The native profile harness changed during the run: ${path}`);
  evidence.result = 'passed';
} catch (error) {
  evidence.error = error.message;
  process.exitCode = 1;
  if (driver?.session) { try { await driver.screenshot(join(output, 'failure.png')); evidence.screenshots.push('failure.png'); } catch { /* Preserve the first failure. */ } }
} finally {
  try { await closeApp(); } catch (error) { evidence.cleanupError = error.message; evidence.result = 'failed'; process.exitCode = 1; }
  if (fixture) { stopGroup(fixture.bus, 'SIGKILL'); await rm(fixture.root, { recursive: true, force: true }); }
  evidence.finishedAt = new Date().toISOString();
  await writeFile(join(output, 'result.json'), `${JSON.stringify(evidence, null, 2)}\n`, { mode: 0o600 });
  console.log(`${evidence.requirement}: ${evidence.result}. Evidence: ${output}`);
  if (evidence.error) console.error(evidence.error);
}
