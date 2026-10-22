import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { chmod, copyFile, lstat, mkdir, readFile, readdir, readlink, rm, symlink, unlink, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { setTimeout as delay } from 'node:timers/promises';
import { CODEX_SENTINEL, createFixture, stopGroup } from './private-fixture.mjs';
import { NativeDriver, waitFor } from './webdriver.mjs';
import { startSecretService, stopSecretService } from './secret-service.mjs';

for (const name of ['OPENUSAGE_NATIVE_BINARY', 'OPENUSAGE_NATIVE_EVIDENCE']) assert.ok(process.env[name], name);
const repository = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const application = resolve(process.env.OPENUSAGE_NATIVE_BINARY), output = resolve(process.env.OPENUSAGE_NATIVE_EVIDENCE);
const passphrase = 'OpenUsage synthetic local deletion workspace 2026';
const backupPassword = 'OpenUsage synthetic retained backup 2026';
const providerDigest = '53a853b737ceca311649e9f1d9ebd1341154a5f782efd8e915f7f834c819a6dc';
const service = 'app.openusage.desktop.native-test', account = 'primary-database-key-v1';
const sdk = process.env.OPENUSAGE_SECRET_SERVICE_SDK;
const secretTool = sdk ? join(resolve(sdk), 'runtime/usr/bin/secret-tool') : 'secret-tool';
const hash = async path => createHash('sha256').update(await readFile(path)).digest('hex');
const evidence = { requirement: 'NATIVE-LOCAL-DATA-DELETION-001', result: 'failed', scope: 'Actual Linux Tauri application with private synthetic profiles, SQLCipher4/WAL, fallback key wrapping and a real isolated Secret Service. No existing provider credential homes. Seeded durable intent proves restart guards and renewed consent, not crash or power-loss behavior. External exports/backups and accessibility calibration remain. No OpenAI-side deletion or physical secure-erasure claim.', fixtures: [], closes: [], screenshots: [] };
let fixture, driver, sequence = 0, record;
const call = async (command, args) => { const response = await driver.invoke(command, args); assert.equal(response.ok, true, `${command}: ${JSON.stringify(response)}`); return response.value; };
const rejected = async (command, args, error) => assert.deepEqual(await driver.invoke(command, args), { ok: false, error });
const workspace = () => dirname(fixture.database);
const marker = () => join(workspace(), '.local-deletion.v1');
const scaleFile = () => join(fixture.dirs.config, 'app.openusage.desktop', 'display-scale.v1');
async function absent(path) { await assert.rejects(lstat(path), { code: 'ENOENT' }, path); }
async function open() {
  driver = new NativeDriver(application, fixture.environment, join(output, `driver-${++sequence}.log`));
  await driver.start();
  await waitFor(() => driver.execute('return document.querySelector("#startup-heading")?.textContent!=="Checking local storage…";'), 'native startup result');
}
async function close() {
  const queue = [driver.child.pid]; let owned;
  for (let count = 0; queue.length && count < 64; count++) {
    const pid = queue.shift();
    try { if (await readlink(`/proc/${pid}/exe`) === application) { owned = pid; break; } queue.push(...(await readFile(`/proc/${pid}/task/${pid}/children`, 'utf8')).trim().split(/\s+/).filter(Boolean).map(Number)); }
    catch (error) { if (error.code !== 'ENOENT') throw error; }
  }
  assert.ok(owned, 'The native application must be an owned child.');
  evidence.closes.push(JSON.parse(execFileSync('/usr/bin/python3', [join(repository, 'tests/native/x11-close.py'), String(owned)], { env: fixture.environment, encoding: 'utf8', timeout: 10000 })));
  await waitFor(async () => { try { await readlink(`/proc/${owned}/exe`); return false; } catch (error) { if (error.code === 'ENOENT') return true; throw error; } }, 'genuine native exit');
  driver.session = undefined; await driver.stop(); driver = undefined;
}
async function ready(protection) {
  try { await waitFor(async () => (await call('bootstrap')).storage === 'ready', 'ready private workspace'); }
  catch (error) { await unlockDiagnostic('ready-timeout'); throw error; }
  assert.equal((await call('bootstrap')).keyProtection, protection);
  assert.equal((await call('bootstrap')).schemaVersion, 8);
}
async function nativeKey(value) {
  await driver.request('POST', `/session/${driver.session}/actions`, { actions: [{ type: 'key', id: 'local-deletion-keyboard', actions: [{ type: 'keyDown', value }, { type: 'keyUp', value }] }] });
  await driver.request('DELETE', `/session/${driver.session}/actions`);
}
async function nativeActivate(selector, value = '\uE007') {
  await waitFor(() => driver.execute('const node=document.querySelector(arguments[0]);return Boolean(node&&!node.disabled&&node.getClientRects().length);', [selector]), `Available keyboard control: ${selector}`);
  assert.equal(await driver.execute('const node=document.querySelector(arguments[0]);if(!node||node.disabled||!node.getClientRects().length)return false;node.scrollIntoView({block:"nearest",inline:"nearest"});node.focus({preventScroll:true});return document.activeElement===node;', [selector]), true, `Keyboard focus: ${selector}`);
  await nativeKey(value);
}
async function navigate(view) {
  assert.ok(['overview', 'usage', 'settings'].includes(view));
  if (await driver.execute('return Boolean(document.querySelector("[data-account-panel]")?.open);')) {
    await nativeActivate('[data-account-panel-close]');
    await waitFor(() => driver.execute('return !document.querySelector("[data-account-panel]")?.open;'), 'account dialog closed');
  }
  const selector = `[data-workspace-nav="${view}"]`;
  await nativeActivate(selector);
  await waitFor(() => driver.execute('const link=document.querySelector(arguments[0]),section=document.querySelector(`[data-workspace-view="${arguments[1]}"]`);return link?.getAttribute("aria-current")==="page"&&Boolean(section&&!section.hidden&&section.getClientRects().length);', [selector, view]), `keyboard navigation to ${view}`);
}

async function setup(protection) {
  assert.equal((await call('bootstrap')).storage, 'setup_required');
  if (protection === 'passphrase') {
    await waitFor(() => driver.execute('return Boolean(document.querySelector("#workspace-confirmation")&&!document.querySelector("#workspace-confirmation").disabled);'), 'setup password confirmation');
    await driver.type('#workspace-passphrase', passphrase); await driver.type('#workspace-confirmation', passphrase);
    assert.deepEqual(await driver.execute('return ["workspace-passphrase","workspace-confirmation"].map(id=>{const node=document.getElementById(id);return{length:node.value.length,valid:node.validity.valid,disabled:node.disabled};});'), Array.from({ length: 2 }, () => ({ length: passphrase.length, valid: true, disabled: false })));
    assert.equal(await driver.execute('return document.activeElement?.id;'), 'workspace-confirmation');
    await nativeKey('\uE007');
  } else await nativeActivate('.startup-content .primary-button');
  await ready(protection);
}
async function unlock() {
  await waitFor(() => driver.execute('return Boolean(document.querySelector("#workspace-passphrase")&&!document.querySelector("#workspace-passphrase").disabled);'), 'unlock field');
  await driver.execute('window.__localDeletionUnlock={buttonClicks:0,submits:0,invalid:0,clicks:[]};document.addEventListener("click",event=>{const trace=window.__localDeletionUnlock,node=event.target;trace.clicks.push({tag:node.tagName,id:node.id,x:event.clientX,y:event.clientY});if(trace.clicks.length>8)trace.clicks.shift();if(node.closest(".unlock-form button[type=submit]"))trace.buttonClicks++;},true);document.addEventListener("submit",event=>{if(event.target.matches(".unlock-form"))window.__localDeletionUnlock.submits++;},true);document.addEventListener("invalid",event=>{if(event.target.closest(".unlock-form"))window.__localDeletionUnlock.invalid++;},true);return true;');
  await driver.type('#workspace-passphrase', passphrase);
  const before = await unlockDiagnostic('before-mouse-click');
  assert.equal(before.dom.inputLength, passphrase.length); assert.equal(before.dom.inputValid, true); assert.equal(before.dom.inputDisabled, false); assert.equal(before.dom.submitDisabled, false);
  await driver.click('.unlock-form button[type=submit]'); await delay(500);
  const clicked = await unlockDiagnostic('after-mouse-click');
  if (clicked.dom.trace.submits === 0 && clicked.bootstrap.storage === 'locked') {
    // Retain the missed mouse submission evidence before the existing native keyboard path.
    assert.equal(clicked.dom.inputLength, passphrase.length); assert.equal(clicked.dom.inputValid, true); assert.equal(clicked.dom.submitDisabled, false); assert.equal(clicked.dom.trace.invalid, 0);
    await driver.type('#workspace-passphrase', passphrase);
    const keyboard = await unlockDiagnostic('before-native-enter');
    assert.equal(keyboard.dom.inputLength, passphrase.length); assert.equal(keyboard.dom.focus.id, 'workspace-passphrase');
    await nativeKey('\uE007');
    record.unlockMouseAutomation = { observedMouseSubmitEvents: 0, recovery: 'real_native_keyboard_enter', scope: 'WebDriver mouse click did not submit the valid enabled form at persisted native zoom; this does not establish that an actual user mouse fails.' };
  }
  await ready('passphrase'); await unlockDiagnostic('unlock-ready');
}
async function unlockDiagnostic(phase) {
  const bootstrap = await driver.invoke('bootstrap');
  const dom = await driver.execute('const form=document.querySelector(".unlock-form"),input=document.querySelector("#workspace-passphrase"),button=form?.querySelector("button[type=submit]"),rect=button?.getBoundingClientRect(),hit=rect?document.elementFromPoint(rect.left+rect.width/2,rect.top+rect.height/2):null;return {trace:window.__localDeletionUnlock??null,heading:document.getElementById("startup-heading")?.textContent??null,error:document.getElementById("workspace-error")?.textContent??null,inputLength:input?.value.length??null,inputValid:input?.validity.valid??null,inputDisabled:input?.disabled??null,submitDisabled:button?.disabled??null,submitRect:rect?{left:rect.left,top:rect.top,width:rect.width,height:rect.height}:null,centerHit:hit?{tag:hit.tagName,id:hit.id,insideSubmit:button===hit||button?.contains(hit)}:null,focus:{tag:document.activeElement?.tagName??null,id:document.activeElement?.id??null},viewport:{width:innerWidth,height:innerHeight,devicePixelRatio,visualScale:visualViewport?.scale??null},documentFocused:document.hasFocus(),storage:document.querySelector("[data-storage]")?.dataset.storage??null};');
  const diagnostic = { phase, bootstrap: bootstrap.ok ? bootstrap.value : { error: bootstrap.error }, dom };
  (record.unlockDiagnostics ??= []).push(diagnostic);
  return diagnostic;
}
async function liveHashes() {
  const result = {};
  for (const path of [fixture.database, `${fixture.database}-wal`, join(workspace(), 'database-key.v1')]) {
    try { result[path] = await hash(path); } catch (error) { if (error.code !== 'ENOENT') throw error; result[path] = null; }
  }
  return result;
}
async function addProfile() {
  const previous = (await call('list_profiles')).profiles.map(profile => profile.id);
  await navigate('overview'); await nativeActivate('[data-overview-add-account]');
  let profile;
  await waitFor(async () => { profile = (await call('list_profiles')).profiles.find(profile => !previous.includes(profile.id)); return Boolean(profile); }, 'new isolated account');
  await navigate('overview');
  return profile;
}
async function savedSyntheticReading() {
  const executable = join(fixture.root, 'synthetic-collection-provider.py');
  await copyFile(join(repository, 'crates/openusage-codex/tests/fixtures/synthetic-collection-provider.py'), executable); await chmod(executable, 0o700); assert.equal(await hash(executable), providerDigest);
  await navigate('settings');
  if (!await driver.execute('return document.querySelector("[data-provider-settings]").closest("details").open;')) await nativeActivate('[data-provider-settings]');
  if (!await driver.execute('return document.querySelector("[data-provider-manual]").closest("details").open;')) await nativeActivate('[data-provider-manual]');
  await driver.type('#provider-executable', executable); await nativeActivate('.provider-inspection-form button[type=submit]');
  await waitFor(() => driver.execute('const n=document.querySelector("#provider-executable-approved");return Boolean(n&&!n.disabled&&n.getAttribute("aria-checked")==="false");'), 'explicit fixture approval');
  await nativeActivate('#provider-executable-approved', ' ');
  await waitFor(() => driver.execute('return document.querySelector("#provider-executable-approved")?.getAttribute("aria-checked")==="true";'), 'keyboard fixture approval consent');
  await nativeActivate('.provider-approval-form button[type=submit]');
  await waitFor(() => driver.execute('return Boolean(document.querySelector(".provider-approval-result"));'), 'approved synthetic provider');
  const profile = await addProfile();
  await writeFile(join(workspace(), 'profiles', profile.id, 'synthetic-collection-config.json'), JSON.stringify({ mode: 'startup_snapshot', requireLogin: true }) + '\n', { mode: 0o600 });
  await nativeActivate(`[data-overview-account="${profile.id}"] td:last-child button:first-child`);
  await waitFor(async () => Boolean(await call('load_profile_activity', { profileId: profile.id })), 'automatic accepted synthetic collection', 45000);
  const activity = await call('load_profile_activity', { profileId: profile.id });
  assert.equal(activity.sourceKind, 'test_fixture'); assert.equal(activity.summary.lifetimeTokens, '9007199254740993');
  assert.ok(await call('load_profile_quota', { profileId: profile.id }));
  record.syntheticSavedReading = { profileId: profile.id, sourceKind: activity.sourceKind, collectionRunId: activity.collectionRunId };
  return profile;
}
async function retainedFiles() {
  const archive = await call('create_backup', { password: backupPassword });
  assert.equal(resolve(archive.directory), join(fixture.dirs.downloads, 'OpenUsage'));
  const archivePath = join(archive.directory, archive.fileName);
  const preview = await call('prepare_restore_preview', { fileName: archive.fileName, password: backupPassword });
  assert.equal(preview.restoreAvailable, true);
  assert.ok((await readdir(join(workspace(), 'restore-checkpoints'))).length > 0, 'A real managed encrypted checkpoint must exist before deletion.');
  const exported = join(fixture.dirs.downloads, 'OpenUsage', 'synthetic-retained-export.csv');
  await writeFile(exported, 'Synthetic user export; retain this file.\n', { mode: 0o600 });
  const external = join(fixture.dirs.codex, 'synthetic-fixture.txt');
  return new Map(await Promise.all([archivePath, exported, external, scaleFile()].map(async path => [path, await hash(path)])));
}
async function preserved(expected) { for (const [path, digest] of expected) assert.equal(await hash(path), digest, `Retain ${path}`); assert.equal(await readFile(join(fixture.dirs.codex, 'synthetic-fixture.txt'), 'utf8'), CODEX_SENTINEL); }
async function confirmGui() {
  const availableStarted = Date.now();
  try {
    await waitFor(() => driver.execute('const n=document.querySelector("[data-local-data-deletion-open]");return Boolean(n&&!n.disabled&&n.getClientRects().length);'), 'visible local deletion action', 60000);
  } catch (error) {
    record.deletionActionFailure = await driver.execute('const n=document.querySelector("[data-local-data-deletion-open]");return {exists:!!n,disabled:n?.disabled??null,visible:!!n?.getClientRects().length,page:document.querySelector("[data-workspace-nav][aria-current=page]")?.getAttribute("data-workspace-nav")??null,heading:document.querySelector("#startup-heading")?.textContent??null,preparing:[...document.querySelectorAll("[role=status]")].some(node=>node.textContent.includes("Preparing OpenAI sign-in"))};');
    throw error;
  }
  (record.deletionActionWaitMs ??= []).push(Date.now() - availableStarted);
  await nativeActivate('[data-local-data-deletion-open]');
  await waitFor(() => driver.execute('return Boolean(document.querySelector("[data-local-data-deletion-dialog]")?.open);'), 'native deletion confirmation dialog');
  assert.deepEqual(await driver.execute('const c=document.querySelector("[data-local-data-deletion-confirm]"),s=document.querySelector("[data-local-data-deletion-submit]");return {checked:c.getAttribute("aria-checked"),disabled:s.disabled};'), { checked: 'false', disabled: true });
  const disclosure = await driver.execute('return document.querySelector("[data-local-data-deletion-dialog]").textContent;');
  for (const text of ['Display size', 'OpenAI records', 'exported files', 'external backups', 'other Codex installations']) assert.equal(disclosure.includes(text), true);
  await nativeActivate('[data-local-data-deletion-confirm]', ' ');
  await waitFor(() => driver.execute('return document.querySelector("[data-local-data-deletion-confirm]").getAttribute("aria-checked")==="true"&&!document.querySelector("[data-local-data-deletion-submit]").disabled;'), 'renewed explicit deletion consent');
  if (!evidence.screenshots.length) { await driver.screenshot(join(output, 'local-deletion-confirmation.png')); evidence.screenshots.push('local-deletion-confirmation.png'); }
  await nativeActivate('[data-local-data-deletion-submit]');
  await waitFor(async () => (await call('bootstrap')).storage === 'setup_required', 'native deletion complete', 30000);
}
async function deleted(lock, expected) {
  for (const name of ['openusage.db', 'openusage.db-wal', 'openusage.db-shm', 'database-key.v1', 'profiles', 'restore-checkpoints', 'restore-staging', 'backup-inspection', '.local-deletion.v1']) await absent(join(workspace(), name));
  const current = await lstat(join(workspace(), '.instance.lock'));
  assert.equal(current.ino, lock.ino); assert.equal(current.dev, lock.dev); assert.equal(current.isSymbolicLink(), false);
  assert.deepEqual((await readdir(workspace())).sort(), ['.instance.lock']);
  await preserved(expected);
  const state = await call('bootstrap'); assert.equal(state.storage, 'setup_required'); assert.equal(state.activeProfiles, null); assert.equal(state.provider, 'not_configured'); assert.equal(state.monitoring, 'stopped');
  await rejected('list_profiles', {}, 'workspace_locked');
  record.deleted = { databaseWalShmWrappedKeyManagedHomesCheckpointsRemoved: true, instanceLockInodeRetained: true, exportedBackupAndExternalCodexHomePreserved: true, calibrationPreserved: true, bootstrapProviderUnconfigured: true };
}
function toolEnvironment() { const environment = { ...fixture.environment }; if (sdk) environment.LD_LIBRARY_PATH = join(resolve(sdk), 'runtime/usr/lib/x86_64-linux-gnu'); return environment; }
function searchEntry(selectedService, username) {
  const stdout = execFileSync('gdbus', ['call', '--session', '--dest', 'org.freedesktop.secrets', '--object-path', '/org/freedesktop/secrets', '--method', 'org.freedesktop.Secret.Service.SearchItems', `{'service': '${selectedService}', 'username': '${username}'}`], { env: fixture.environment, encoding: 'utf8', timeout: 5000 });
  return stdout.includes('/org/freedesktop/secrets/collection/');
}
function putUnrelatedEntry(selectedService, username) { execFileSync(secretTool, ['store', '--label=OpenUsage synthetic unrelated entry', 'service', selectedService, 'username', username], { input: 'Synthetic unrelated secret\n', env: toolEnvironment(), timeout: 5000 }); }
function unrelatedEntryUnchanged(selectedService, username) { assert.equal(execFileSync(secretTool, ['lookup', 'service', selectedService, 'username', username], { env: toolEnvironment(), encoding: 'utf8', timeout: 5000 }).trim(), 'Synthetic unrelated secret'); }
async function cleanup() {
  if (!fixture) return;
  try { if (driver) { await driver.stop(); driver = undefined; } }
  finally { try { await stopSecretService(fixture); } finally { stopGroup(fixture.bus, 'SIGKILL'); await rm(fixture.root, { recursive: true, force: true }); fixture = undefined; } }
}

await mkdir(output, { recursive: true, mode: 0o700 });
process.once('exit', () => { driver?.kill('SIGKILL'); fixture?.daemon?.kill('SIGKILL'); stopGroup(fixture?.bus, 'SIGKILL'); });
try {
  evidence.applicationSha256 = await hash(application);
  for (const protection of ['passphrase', 'os_store']) {
    record = { protection }; evidence.fixtures.push(record);
    fixture = await createFixture(`local-deletion-${protection}`); fixture.environment.HOME = join(fixture.root, 'synthetic-home'); await mkdir(fixture.environment.HOME, { mode: 0o700 });
    if (protection === 'os_store') {
      await startSecretService(fixture);
      putUnrelatedEntry('app.openusage.synthetic-unrelated', account); putUnrelatedEntry(service, 'synthetic-unrelated-account');
      assert.equal(searchEntry(service, account), false); assert.equal(searchEntry('app.openusage.desktop', account), false);
    }
    await open();
    await rejected('delete_all_local_data', { confirmed: true }, 'workspace_locked');
    for (const args of [{}, { confirmed: false }, { confirmed: 'yes' }, { confirmed: true, unexpected: true }]) await rejected('delete_all_local_data', args, 'invalid_arguments');
    await setup(protection);
    if (protection === 'passphrase') await savedSyntheticReading();
    await addProfile();
    assert.equal((await call('list_profiles')).profiles.length, protection === 'passphrase' ? 2 : 1);
    await call('set_display_scale', { percent: 125 });
    const orphan = join(workspace(), 'profiles', randomUUID()); await mkdir(orphan, { mode: 0o700 }); await writeFile(join(orphan, 'synthetic-orphan-session.txt'), 'Synthetic old restore home; remove on confirmed deletion.\n', { mode: 0o600 });
    const expected = await retainedFiles(), lock = await lstat(join(workspace(), '.instance.lock'));
    assert.ok((await lstat(`${fixture.database}-wal`)).size > 32); assert.equal((await readFile(fixture.database)).includes(Buffer.from('SQLite format 3\0')), false);
    if (protection === 'passphrase') assert.ok((await lstat(join(workspace(), 'database-key.v1'))).size > 0);
    else { await absent(join(workspace(), 'database-key.v1')); assert.equal(searchEntry(service, account), true); }
    const beforeUnsafe = await liveHashes(), unknown = join(workspace(), 'synthetic-unknown-preserve.txt');
    await writeFile(unknown, 'Unknown private data must not be deleted.\n', { mode: 0o600 });
    await rejected('delete_all_local_data', { confirmed: true }, 'local_deletion_unsafe');
    assert.deepEqual(await liveHashes(), beforeUnsafe); await absent(marker()); assert.equal((await call('bootstrap')).storage, 'ready');
    assert.equal(await readFile(unknown, 'utf8'), 'Unknown private data must not be deleted.\n'); await unlink(unknown);
    const escape = join(orphan, 'synthetic-outside-link'); await symlink(fixture.dirs.codex, escape);
    await rejected('delete_all_local_data', { confirmed: true }, 'local_deletion_unsafe');
    assert.deepEqual(await liveHashes(), beforeUnsafe); await absent(marker()); await preserved(expected); await unlink(escape);
    record.unsafeRefusal = { unknownRootAndSymlinkRefusedBeforeIntent: true, databaseWalKeyAndExternalFilesUnchanged: true };
    if (protection === 'passphrase') {
      await close(); await open(); assert.equal((await call('bootstrap')).storage, 'locked');
      const locked = await liveHashes(); await rejected('delete_all_local_data', { confirmed: true }, 'workspace_locked'); assert.deepEqual(await liveHashes(), locked);
      await unlock(); await close();
      const database = await lstat(fixture.database), wrapped = await lstat(join(workspace(), 'database-key.v1'));
      for (const value of [database.dev, database.ino, wrapped.dev, wrapped.ino]) assert.equal(Number.isSafeInteger(value), true, 'The fixture must preserve exact filesystem identities.');
      await writeFile(marker(), JSON.stringify({ version: 1, mode: 'passphrase', os_key_fingerprint: null, database_identity: [database.dev, database.ino], wrapped_identity: [wrapped.dev, wrapped.ino] }) + '\n', { mode: 0o600, flag: 'wx' });
      const pending = await liveHashes(), pendingMarker = await hash(marker()); await open();
      const state = await call('bootstrap'); assert.equal(state.storage, 'recovery_required'); assert.equal(state.storageIssue, 'local_deletion_incomplete');
      for (const [command, args] of [['setup_workspace', { passphrase }], ['unlock_workspace', { passphrase }]]) await rejected(command, args, 'local_deletion_failed');
      await rejected('delete_all_local_data', { confirmed: false }, 'invalid_arguments');
      assert.deepEqual(await liveHashes(), pending); assert.equal(await hash(marker()), pendingMarker); await preserved(expected);
      record.pendingRestart = { syntheticIntentSeedAfterGenuineClose: true, recoveryRequired: true, noAutomaticDeletion: true, setupAndUnlockRefused: true, renewedConsentRequired: true };
    } else await navigate('settings');
    await confirmGui(); await deleted(lock, expected);
    if (protection === 'os_store') {
      assert.equal(searchEntry(service, account), false); assert.equal(searchEntry('app.openusage.desktop', account), false);
      unrelatedEntryUnchanged('app.openusage.synthetic-unrelated', account); unrelatedEntryUnchanged(service, 'synthetic-unrelated-account');
      record.osStore = { realPrivateSecretService: true, exactApplicationEntryRemoved: true, otherServiceAndUsernameEntriesUnchanged: true, productionNamespaceUntouched: true };
    }
    await close(); await open();
    assert.equal((await call('bootstrap')).storage, 'setup_required'); await rejected('list_profiles', {}, 'workspace_locked'); await preserved(expected);
    assert.equal((await call('get_display_scale')).percent, 125);
    await setup(protection); assert.deepEqual((await call('list_profiles')).profiles, []);
    assert.equal((await call('bootstrap')).provider, 'not_configured');
    record.restartAndRecreate = { genuineRestartSetupRequired: true, noSavedProfiles: true, newSetupSucceeded: true, calibrationPercent: 125 };
    if (protection === 'passphrase') {
      await addProfile(); await navigate('settings');
      await absent(marker()); await confirmGui(); await deleted(lock, expected);
      record.normalFallbackDeletion = { confirmedReadyWorkspaceDeletedWithoutSeededIntent: true };
    }
    await close(); await cleanup();
  }
  assert.equal(await hash(application), evidence.applicationSha256); evidence.result = 'passed';
} catch (error) { evidence.error = error.stack ?? String(error); process.exitCode = 1; }
finally { try { await cleanup(); } catch (error) { evidence.cleanupError = String(error); evidence.result = 'failed'; process.exitCode = 1; } await writeFile(join(output, 'result.json'), JSON.stringify(evidence, null, 2) + '\n', { mode: 0o600 }); }
