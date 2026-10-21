import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { chmod, copyFile, mkdir, readFile, readdir, readlink, rm, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { CODEX_SENTINEL, createFixture, stopGroup } from './private-fixture.mjs';
import { NativeDriver, waitFor } from './webdriver.mjs';

const repository = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
for (const name of ['OPENUSAGE_NATIVE_BINARY', 'OPENUSAGE_NATIVE_ALERT_FIXTURE', 'OPENUSAGE_LOGIN_VERIFIER', 'OPENUSAGE_NATIVE_EVIDENCE']) assert.ok(process.env[name], `Set ${name}.`);
const application = resolve(process.env.OPENUSAGE_NATIVE_BINARY), seeder = resolve(process.env.OPENUSAGE_NATIVE_ALERT_FIXTURE), verifier = resolve(process.env.OPENUSAGE_LOGIN_VERIFIER);
const output = resolve(process.env.OPENUSAGE_NATIVE_EVIDENCE);
const passphrase = 'OpenUsage synthetic alert history workspace 2026';
const backupPassword = 'OpenUsage synthetic alert history backup 2026';
const hash = async path => createHash('sha256').update(await readFile(path)).digest('hex');
const evidence = { requirement: 'NATIVE-ALERT-HISTORY-001', result: 'failed', startedAt: new Date().toISOString(), scope: 'Actual native GUI-created synthetic profiles in a private SQLCipher4/WAL application-schema7 workspace. An owned closed-app test helper uses production Storage::accept_quota to seed controlled reset proof and follow-ups; no provider execution or clock bypass. Read-only alert listing, explicit Settings disclosure, exact-profile Usage navigation, genuine restart and follow-up dedupe, archive visibility, and complete alert table fields in authenticated archive/checkpoint copies and actual typed-IPC archive restore/Undo are checked. The native path uses synthetic quota records, not a live provider or desktop notification delivery. Archive restore and Undo use explicit confirmed=true typed native IPC; GUI confirmation is qualified by the separate restore journey. No desktop delivery, crash durability or cross-platform acceptance.', screenshots: [], refusals: [], readOnly: [], oracles: [], closes: [] };
let fixture, driver, sequence = 0, workspace, selected, event;
async function call(command, args) { const response = await driver.invoke(command, args); assert.equal(response.ok, true, `${command}: ${JSON.stringify(response)}`); return response.value; }
async function liveHashes() {
  const result = {};
  for (const path of [fixture.database, `${fixture.database}-wal`, join(workspace, 'database-key.v1')]) {
    try { result[path] = await hash(path); } catch (error) { if (error.code !== 'ENOENT') throw error; result[path] = null; }
  }
  return result;
}
async function refused(args, expected) {
  const before = await liveHashes();
  assert.deepEqual(await driver.invoke('load_alert_history', args), { ok: false, error: expected });
  assert.deepEqual(await liveHashes(), before);
  evidence.refusals.push({ arguments: args, error: expected, databaseWalKeyUnchanged: true });
}
async function open(setup = false) {
  driver = new NativeDriver(application, fixture.environment, join(output, `driver-${++sequence}.log`)); await driver.start();
  await waitFor(() => driver.execute('const field=document.querySelector("#workspace-passphrase");return Boolean(field&&!field.disabled);'), 'enabled synthetic workspace password');
  await refused({}, 'workspace_locked');
  await driver.type('#workspace-passphrase', passphrase);
  if (setup) await driver.type('#workspace-confirmation', passphrase);
  await driver.click('.unlock-form button[type=submit]');
  await waitFor(async () => (await call('bootstrap')).storage === 'ready', 'ready synthetic workspace');
  assert.equal((await call('bootstrap')).schemaVersion, 7);
}
async function close() {
  const queue = [driver.child.pid]; let pid;
  for (let count = 0; queue.length && count < 64; count++) {
    const next = queue.shift();
    try { if (await readlink(`/proc/${next}/exe`) === application) { pid = next; break; } queue.push(...(await readFile(`/proc/${next}/task/${next}/children`, 'utf8')).trim().split(/\s+/).filter(Boolean).map(Number)); }
    catch (error) { if (error.code !== 'ENOENT') throw error; }
  }
  assert.ok(pid, 'The exact owned native application must be found.');
  evidence.closes.push(JSON.parse(execFileSync('/usr/bin/python3', [join(repository, 'tests/native/x11-close.py'), String(pid)], { env: fixture.environment, encoding: 'utf8', timeout: 10000 })));
  await waitFor(async () => { try { await readlink(`/proc/${pid}/exe`); return false; } catch (error) { if (error.code === 'ENOENT') return true; throw error; } }, 'genuine native exit');
  driver.session = undefined; await driver.stop(); driver = undefined;
}
async function createProfile(label) {
  await driver.openProfileCreate();
  await driver.type('#profile-label', label); await driver.click('.profile-create-form button[type=submit]');
  await waitFor(async () => (await call('list_profiles')).profiles.some(profile => profile.label === label), 'GUI-created named profile');
  await waitFor(() => driver.execute('const field=document.querySelector("#profile-label");return Boolean(field&&!field.disabled&&field.value==="");'), 'profile form ready after save');
  const profile = (await call('list_profiles')).profiles.find(profile => profile.label === label);
  await driver.click('[data-account-panel-close]'); return profile;
}
function seed(mode) {
  assert.equal(driver, undefined, 'Seeding requires the exact owned app and driver to be closed.');
  return JSON.parse(execFileSync(seeder, [fixture.root, selected.id, mode], { input: JSON.stringify({ passphrase }), encoding: 'utf8', timeout: 30000, maxBuffer: 1024 * 1024, env: { PATH: '/usr/bin:/bin' } }));
}
async function history(label) {
  const before = await liveHashes();
  const first = await call('load_alert_history'), second = await call('load_alert_history');
  assert.deepEqual(first, second); assert.deepEqual(await liveHashes(), before);
  assert.deepEqual(Object.keys(first).sort(), ['events', 'truncated']); assert.equal(first.truncated, false); assert.equal(first.events.length, 1);
  const item = first.events[0];
  assert.deepEqual(Object.keys(item).sort(), ['deliveryState', 'evidenceAtUtc', 'evidenceRunId', 'id', 'kind', 'limitId', 'profileId', 'profileLabel', 'sourceKind', 'thresholdPercent', 'windowDurationMins', 'windowRole'].sort());
  assert.deepEqual(item, { id: event.eventId, profileId: selected.id, profileLabel: selected.label, kind: 'quota_reset', thresholdPercent: null, evidenceRunId: event.generationRunId, limitId: 'synthetic-reset-window', windowRole: 'primary', windowDurationMins: 300, evidenceAtUtc: '2026-10-22T12:01:00.000Z', sourceKind: 'test_fixture', deliveryState: event.deliveryState });
  evidence.readOnly.push({ label, databaseWalKeyUnchanged: true, eventId: item.id, eventCount: 1 }); return first;
}
async function oracle(mode, input, label, full = false) {
  const before = await liveHashes(), original = await hash(input), copy = join(fixture.root, `alert-oracle-${randomUUID()}`); await mkdir(copy, { mode: 0o700 });
  try {
    let source = input;
    if (mode === 'database') {
      source = join(copy, 'database.db'); await copyFile(input, source); await chmod(source, 0o600);
      try { await copyFile(`${input}-wal`, `${source}-wal`); await chmod(`${source}-wal`, 0o600); } catch (error) { if (error.code !== 'ENOENT') throw error; }
    }
    const report = JSON.parse(execFileSync(verifier, [mode, source, join(workspace, 'database-key.v1'), ...(full ? ['--profile-state'] : [])], { input: JSON.stringify({ passphrase, backupPassword }), encoding: 'utf8', timeout: 60000, maxBuffer: 16 * 1024 * 1024, env: fixture.environment }));
    for (const field of ['verified', 'readOnly', 'inputUnchanged', 'keyEnvelopeUnchanged', 'walRecordsUnchanged']) assert.equal(report[field], true, field);
    assert.equal(report.schemaVersion, 7); assert.equal(report.plaintextDiskStaging, false);
    assert.equal(Object.keys(report.state.tables).length, full ? 24 : 21);
    const rows = report.state.tables.alert_events; assert.equal(rows.length, 1); assert.equal(Object.keys(rows[0]).length, 12);
    assert.equal(rows[0].id, event.eventId); assert.equal(rows[0].generation_run_id, event.generationRunId);
    assert.deepEqual(await liveHashes(), before); assert.equal(await hash(input), original);
    evidence.oracles.push({ label, mode, completeNamedTables: Object.keys(report.state.tables).length, inputSha256: original, alertRows: 1, allAlertColumns: 12, tableHashes: Object.fromEntries(Object.entries(report.state.tables).map(([name, records]) => [name, { rows: records.length, sha256: createHash('sha256').update(JSON.stringify(records)).digest('hex') }])) });
    return rows;
  } finally { await rm(copy, { recursive: true, force: true }); }
}
await mkdir(output, { recursive: true, mode: 0o700 });
process.once('exit', () => { driver?.kill('SIGKILL'); stopGroup(fixture?.bus, 'SIGKILL'); });
try {
  evidence.application = { path: application, sha256: await hash(application) }; evidence.seeder = { path: seeder, sha256: await hash(seeder) }; evidence.verifier = { path: verifier, sha256: await hash(verifier) };
  evidence.sourceCommit = execFileSync('git', ['rev-parse', 'HEAD'], { cwd: repository, encoding: 'utf8' }).trim();
  fixture = await createFixture('alert-history'); workspace = dirname(fixture.database);
  fixture.environment.HOME = join(fixture.root, 'synthetic-home'); await mkdir(fixture.environment.HOME, { mode: 0o700 });
  await open(true);
  assert.deepEqual(await call('load_alert_history'), { events: [], truncated: false });
  const untouched = await createProfile('Synthetic unrelated account'); selected = await createProfile('Synthetic reset account');
  await writeFile(join(fixture.dirs.control, 'alert-fixture.json'), JSON.stringify({ purpose: 'openusage-native-alert-history-v1', profileId: selected.id }), { mode: 0o600, flag: 'wx' });
  await close(); event = seed('seed'); assert.equal(event.eventCount, 1); assert.equal(event.productionStorageApi, true);
  await open(); const expected = await history('initial reopen');
  for (const args of [{ extra: true }, { profileId: selected.id }, { limit: 1 }, []]) await refused(args, 'invalid_arguments');
  const originalAlertRows = await oracle('database', fixture.database, 'all columns after controlled reset and follow-up', true);
  await driver.navigate('settings'); await driver.click('[data-alert-history] > button');
  await waitFor(() => driver.execute('return document.querySelectorAll("[data-alert-history-event]").length===1;'), 'visible saved reset history');
  await driver.click('[data-alert-history-event] .alert-history-source > button');
  const visible = await driver.execute('const row=document.querySelector("[data-alert-history-event]");return {text:row.textContent,profile:row.querySelector("bdi").textContent,time:row.querySelector("time").dateTime};');
  assert.equal(visible.profile, selected.label); assert.equal(visible.time, expected.events[0].evidenceAtUtc);
  assert.match(visible.text, /Synthetic test data/); assert.match(visible.text, event.deliveryState === 'suppressed' ? /Not sent because this reset is no longer eligible/ : /No desktop notification sent/);
  await driver.execute('document.querySelector("[data-alert-history-event]").scrollIntoView({block:"center"});return true;');
  await driver.screenshot(join(output, 'saved-reset-alert-history.png'));  evidence.screenshots.push('saved-reset-alert-history.png');
  await driver.execute('window.__alertNavigationClicks=[];document.addEventListener("click",event=>{const button=event.target.closest?.(".alert-history-event > button");if(button)window.__alertNavigationClicks.push({target:event.target.tagName,trusted:event.isTrusted,label:button.textContent});},true);return true;');
  await driver.click('[data-alert-history-event] .alert-history-event > button');
  await waitFor(() => driver.execute('const usage=document.querySelector("[data-workspace-view=usage]");return usage?.getClientRects().length&&usage.querySelector("[data-token-heatmap]")?.getAttribute("data-profile-id")===arguments[0];', [selected.id]), 'View usage selects exact event profile');
  assert.equal(await call('load_profile_quota', { profileId: untouched.id }), null);
  evidence.gui = { explicitDisclosure: true, oneRecordedEvent: true, sourceDetailsRead: true, deliveryNotClaimed: true, viewUsageExactProfile: true, unrelatedProfileWithoutReadings: true };
  await close(); const followup = seed('followup'); assert.equal(followup.eventId, event.eventId); await open();
  assert.deepEqual(await history('genuine restart after another accepted snapshot'), expected);
  assert.deepEqual(await oracle('database', fixture.database, 'after restart dedupe', true), originalAlertRows);
  const archive = await call('create_backup', { password: backupPassword });
  assert.match(archive.fileName, /^openusage-backup-[A-Za-z0-9_-]+\.oubackup$/);
  const archivePath = join(fixture.dirs.downloads, 'OpenUsage', archive.fileName);
  assert.deepEqual(await oracle('backup', archivePath, 'authenticated backup preserves every alert column'), originalAlertRows);
  const preview = await call('prepare_restore_preview', { fileName: archive.fileName, password: backupPassword });
  const checkpoint = join(workspace, 'restore-checkpoints', `${preview.checkpoint.id}.sqlite`);
  assert.deepEqual(await oracle('checkpoint', checkpoint, 'encrypted pre-restore checkpoint preserves every alert column'), originalAlertRows);
  assert.equal((await call('cancel_restore_preview', { previewId: preview.previewId })).cancelled, true);
  await call('set_profile_archived', { id: selected.id, archived: true });
  assert.deepEqual(await history('archived profile event remains visible'), expected);
  await close(); await open(); assert.deepEqual(await history('archival survives genuine restart'), expected);
  assert.equal((await call('list_profiles')).profiles.find(profile => profile.id === selected.id).archived, true);
  // Populated-ledger restore/Undo facet uses production typed IPC, not a GUI-consent claim.
  const restorePreview = await call('prepare_restore_preview', { fileName: archive.fileName, password: backupPassword });
  assert.equal(restorePreview.restoreAvailable, true);
  const undoSource = join(workspace, 'restore-checkpoints', `${restorePreview.checkpoint.id}.sqlite`);
  assert.deepEqual(await oracle('checkpoint', undoSource, 'archived-state checkpoint before actual archive restore'), originalAlertRows);
  const archiveHash = await hash(archivePath), checkpointHash = await hash(undoSource);
  const protectedKeyHash = await hash(join(workspace, 'database-key.v1'));
  const restored = await call('apply_restore', { previewId: restorePreview.previewId, confirmed: true });
  assert.equal(restored.applied, true); assert.equal(restored.monitoring, 'stopped'); assert.equal(restored.authenticationRequired, true);
  assert.equal(restored.checkpoint.id, restorePreview.checkpoint.id);
  assert.equal((await call('list_profiles')).profiles.find(profile => profile.id === selected.id).archived, false);
  assert.deepEqual(await history('actual authenticated archive application'), expected);
  assert.deepEqual(await oracle('database', fixture.database, 'every alert column after actual archive restore', true), originalAlertRows);
  assert.equal(await hash(join(workspace, 'database-key.v1')), protectedKeyHash);
  const undoPreview = await call('prepare_checkpoint_restore_preview', { checkpointId: restored.checkpoint.id });
  assert.equal(undoPreview.checkpointId, restored.checkpoint.id); assert.equal(undoPreview.restoreAvailable, true);
  const undone = await call('apply_restore', { previewId: undoPreview.previewId, confirmed: true });
  assert.equal(undone.applied, true); assert.equal(undone.monitoring, 'stopped'); assert.equal(undone.authenticationRequired, true);
  assert.equal((await call('list_profiles')).profiles.find(profile => profile.id === selected.id).archived, true);
  assert.deepEqual(await history('actual pre-restore checkpoint application'), expected);
  assert.deepEqual(await oracle('database', fixture.database, 'every alert column after actual checkpoint Undo', true), originalAlertRows);
  assert.equal(await hash(archivePath), archiveHash); assert.equal(await hash(undoSource), checkpointHash);
  assert.equal(await hash(join(workspace, 'database-key.v1')), protectedKeyHash);
  await close(); await open();
  assert.deepEqual(await history('actual restore and Undo survive genuine restart'), expected);
  assert.deepEqual(await oracle('database', fixture.database, 'every alert column after restore Undo and restart', true), originalAlertRows);
  assert.equal((await call('list_profiles')).profiles.find(profile => profile.id === selected.id).archived, true);
  evidence.populatedLedgerRestore = { consentScope: 'Typed production IPC with confirmed=true; separate native GUI journey covers checkbox consent.', authenticatedArchiveApplied: true, checkpointUndoApplied: true, allTwelveAlertColumnsUnchanged: true, eventIdsUnchanged: true, archivedStateRestoredByUndo: true, genuineRestartRetainsLedger: true, archiveCheckpointAndProtectedKeyUnchanged: true };
  assert.equal(await readFile(join(fixture.dirs.codex, 'synthetic-fixture.txt'), 'utf8'), CODEX_SENTINEL);
  for (const profile of [selected, untouched]) assert.deepEqual(await readdir(join(workspace, 'profiles', profile.id)), []);
  assert.equal(await hash(application), evidence.application.sha256); assert.equal(await hash(seeder), evidence.seeder.sha256); assert.equal(await hash(verifier), evidence.verifier.sha256);
  evidence.result = 'passed';
} catch (error) {
  evidence.error = error.stack ?? String(error);
  if(driver?.session)try{evidence.failureView=await driver.execute('return {view:document.querySelector("nav[aria-label=Main] [aria-current=page]")?.dataset.workspaceNav,heatmaps:[...document.querySelectorAll("[data-token-heatmap]")].map(node=>({profileId:node.dataset.profileId,visible:Boolean(node.getClientRects().length)})),clicks:window.__alertNavigationClicks??[]};');}catch{}
  throw error;
}
finally {
  try { await driver?.stop(); } catch {} stopGroup(fixture?.bus, 'SIGKILL');
  if (fixture) await rm(fixture.root, { recursive: true, force: true });
  evidence.finishedAt = new Date().toISOString(); await writeFile(join(output, 'result.json'), JSON.stringify(evidence, null, 2) + '\n', { mode: 0o600 });
}
