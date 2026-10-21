import assert from 'node:assert/strict';
import { execFileSync, spawn } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { chmod, copyFile, mkdir, readFile, readlink, rm, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { CODEX_SENTINEL, createFixture, stopGroup } from './private-fixture.mjs';
import { NativeDriver, waitFor } from './webdriver.mjs';
const repository = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const production = process.argv.includes('--production');
assert.ok(process.env.OPENUSAGE_NATIVE_BINARY && process.env.OPENUSAGE_NATIVE_EVIDENCE && process.env.OPENUSAGE_NATIVE_ALERT_FIXTURE);
const application = resolve(process.env.OPENUSAGE_NATIVE_BINARY), output = resolve(process.env.OPENUSAGE_NATIVE_EVIDENCE), seeder = resolve(process.env.OPENUSAGE_NATIVE_ALERT_FIXTURE);
const password = 'OpenUsage synthetic reset notification workspace 2026', backupPassword = 'OpenUsage synthetic reset notification backup 2026';
const hash = async path => createHash('sha256').update(await readFile(path)).digest('hex');
const evidence = { requirement: production ? 'NATIVE-RESET-NOTIFICATION-PRODUCTION-REFUSAL-001' : 'NATIVE-RESET-NOTIFICATIONS-001', result: 'failed', scope: 'Actual Linux Tauri GUI, private SQLCipher4/WAL workspace, synthetic profiles and explicitly synthetic quota proofs through public storage APIs. A private Freedesktop D-Bus service acknowledges submissions; no real OS notifications or provider sessions. Production must suppress existing fixture queues. The native-test binary alone permits fixture enqueue/dispatch, retaining fixture provenance. No live-provider, installed Windows/macOS, notification visibility, click routing or crash/power-loss acceptance.', closes: [], screenshots: [] };
let fixture, driver, service, sequence = 0, selected;
const call = async (command, args) => { const response = await driver.invoke(command, args); assert.equal(response.ok, true, `${command}: ${JSON.stringify(response)}`); return response.value; };
const submissions = async () => { try { return (await readFile(join(fixture.dirs.control, 'notification-submissions.jsonl'), 'utf8')).trim().split('\n').filter(Boolean).map(JSON.parse); } catch (error) { if (error.code === 'ENOENT') return []; throw error; } };
async function open(setup = false) {
  driver = new NativeDriver(application, fixture.environment, join(output, `driver-${++sequence}.log`)); await driver.start();
  await waitFor(() => driver.execute('const n=document.querySelector("#workspace-passphrase");return n&&!n.disabled;'), 'enabled workspace password');
  await driver.type('#workspace-passphrase', password); if (setup) await driver.type('#workspace-confirmation', password);
  await driver.request('POST', `/session/${driver.session}/actions`, { actions: [{ type: 'key', id: 'unlock', actions: [{ type: 'keyDown', value: '\uE007' }, { type: 'keyUp', value: '\uE007' }] }] });
  await driver.request('DELETE', `/session/${driver.session}/actions`);
  await waitFor(async () => (await call('bootstrap')).storage === 'ready', 'ready workspace');
  assert.equal((await call('bootstrap')).schemaVersion, 6);
}
async function close() {
  const queue = [driver.child.pid]; let owned;
  for (let n = 0; queue.length && n < 64; n++) {
    const pid = queue.shift();
    try { if (await readlink(`/proc/${pid}/exe`) === application) { owned = pid; break; } queue.push(...(await readFile(`/proc/${pid}/task/${pid}/children`, 'utf8')).trim().split(/\s+/).filter(Boolean).map(Number)); }
    catch (error) { if (error.code !== 'ENOENT') throw error; }
  }
  assert.ok(owned);
  evidence.closes.push(JSON.parse(execFileSync('/usr/bin/python3', [join(repository, 'tests/native/x11-close.py'), String(owned)], { env: fixture.environment, encoding: 'utf8', timeout: 10000 })));
  await waitFor(async () => { try { await readlink(`/proc/${owned}/exe`); return false; } catch (error) { if (error.code === 'ENOENT') return true; throw error; } }, 'genuine native close');
  driver.session = undefined; await driver.stop(); driver = undefined;
}
async function notificationRows(label) {
  const verifier = process.env.OPENUSAGE_LOGIN_VERIFIER; assert.ok(verifier);
  const keyPath = join(dirname(fixture.database), 'database-key.v1');
  const before = { database: await hash(fixture.database), key: await hash(keyPath) };
  let walHash = null; try { walHash = await hash(`${fixture.database}-wal`); } catch (error) { if (error.code !== 'ENOENT') throw error; }
  const temporary = join(fixture.root, `notification-oracle-${randomUUID()}`); await mkdir(temporary, { mode: 0o700 });
  try {
    const copy = join(temporary, 'database.db'); await copyFile(fixture.database, copy); await chmod(copy, 0o600);
    if (walHash !== null) { await copyFile(`${fixture.database}-wal`, `${copy}-wal`); await chmod(`${copy}-wal`, 0o600); }
    const result = JSON.parse(execFileSync(verifier, ['database', copy, keyPath, '--profile-state'], { input: JSON.stringify({ passphrase: password }), encoding: 'utf8', timeout: 30000, env: fixture.environment, maxBuffer: 4 * 1024 * 1024 }));
    assert.equal(result.schemaVersion, 6); assert.equal(result.verified, true); assert.equal(result.readOnly, true); assert.equal(result.plaintextDiskStaging, false);
    assert.equal(Object.keys(result.state.tables).length, 21);
    assert.equal(await hash(fixture.database), before.database); assert.equal(await hash(keyPath), before.key);
    if (walHash !== null) assert.equal(await hash(`${fixture.database}-wal`), walHash);
    const rows = Object.fromEntries(['alert_events', 'reset_notification_outbox', 'reset_notification_receipts'].map(name => [name, result.state.tables[name]]));
    (evidence.oracleChecks ??= []).push({ label, allNamedTables: 21, databaseWalKeyUnchanged: true, rows: Object.fromEntries(Object.entries(rows).map(([name, value]) => [name, { count: value.length, sha256: createHash('sha256').update(JSON.stringify(value)).digest('hex') }])) });
    return rows;
  } finally { await rm(temporary, { recursive: true, force: true }); }
}
async function preferences() {
  await driver.navigate('settings');
  if (await driver.execute('return document.querySelector("[data-reset-notifications] > button").getAttribute("aria-expanded")==="false";')) await driver.click('[data-reset-notifications] > button');
  await waitFor(() => driver.execute('return Boolean(document.querySelector("#reset-notifications-enabled")&&!document.querySelector("#reset-notifications-enabled").disabled);'), 'loaded reset preferences');
  if (!await driver.execute('return Boolean(document.querySelector("#reset-notifications-zone")?.getClientRects().length);')) {
    const trigger = await driver.execute('return [...document.querySelectorAll("[data-reset-notifications] button")].find(n=>n.textContent.trim()==="Quiet hours");');
    await driver.request('POST', `/session/${driver.session}/element/${trigger['element-6066-11e4-a52e-4f735466cecf']}/click`, {});
  }
}
async function save() { await driver.click('.reset-notifications-form button[type=submit]'); await waitFor(() => driver.execute('return document.querySelector(".reset-notifications-form")?.textContent.includes("Notification preferences saved.");'), 'saved GUI preferences'); }
await mkdir(output, { recursive: true, mode: 0o700 });
process.once('exit', () => { driver?.kill('SIGKILL'); stopGroup(service, 'SIGKILL'); stopGroup(fixture?.bus, 'SIGKILL'); });
try {
  evidence.application = { path: application, sha256: await hash(application) }; evidence.seederSha256 = await hash(seeder);
  fixture = await createFixture('reset-notifications'); fixture.environment.HOME = join(fixture.root, 'synthetic-home'); await mkdir(fixture.environment.HOME, { mode: 0o700 });
  service = spawn('/usr/bin/python3', [join(repository, 'tests/native/notification-service.py'), fixture.root], { env: fixture.environment, detached: true, stdio: ['ignore', 'pipe', 'pipe'] });
  let ready = '', serviceFailure = ''; service.stdout.on('data', chunk => { ready += chunk; }); service.stderr.on('data', chunk => { serviceFailure += chunk; });
  service.once('error', error => { serviceFailure += error.message; });
  await waitFor(() => { if (serviceFailure) throw new Error(serviceFailure); return ready.includes('private-notifications-ready'); }, 'private notification service');
  await open(true);
  for (const label of ['Synthetic notification account', 'Synthetic unrelated account']) {
    await driver.openProfileCreate(); await driver.type('#profile-label', label); await driver.click('.profile-create-form button[type=submit]');
    await waitFor(async () => (await call('list_profiles')).profiles.some(profile => profile.label === label), 'GUI-created account');
    await waitFor(() => driver.execute('const n=document.querySelector("#profile-label");return n&&!n.disabled&&n.value==="";'), 'settled account form');
    await driver.click('[data-account-panel-close]');
  }
  const profiles = await call('list_profiles'); selected = profiles.profiles.find(profile => profile.label === 'Synthetic notification account'); assert.ok(selected);
  const initialPreferences = await call('load_reset_notification_preferences');
  for (const [command, args] of [
    ['load_reset_notification_preferences', { extra: true }],
    ['save_reset_notification_preferences', { preferences: { ...initialPreferences, quietStart: '24:00' } }],
    ['save_reset_notification_preferences', { preferences: { ...initialPreferences, timeZone: '../private' } }],
    ['save_reset_notification_preferences', { preferences: { ...initialPreferences, enabled: 'true' } }],
    ['save_reset_notification_preferences', { preferences: initialPreferences, path: '/external' }],
  ]) assert.deepEqual(await driver.invoke(command, args), { ok: false, error: 'invalid_arguments' });
  assert.deepEqual(await call('load_reset_notification_preferences'), initialPreferences);
  assert.equal((await driver.invoke('claim_reset_notification')).ok, false);
  assert.deepEqual(await submissions(), []);
  await preferences(); assert.equal(initialPreferences.enabled, false);
  await driver.click('#reset-notifications-enabled');
  // Select a real fixed-offset IANA zone whose current local hour lies in
  // the default22:00–08:00 quiet period. Do not change the OS clock.
  const offset = (2 - new Date().getUTCHours() + 36) % 24 - 12;
  const zone = offset === 0 ? 'Etc/UTC' : `Etc/GMT${offset > 0 ? '-' : '+'}${Math.abs(offset)}`;
  await driver.replaceText('#reset-notifications-zone', zone); await save();
  const quietPreferences = await call('load_reset_notification_preferences'); assert.equal(quietPreferences.timeZone, zone); assert.equal(quietPreferences.quietEnabled, true);
  await close();
  await writeFile(join(fixture.dirs.control, 'alert-fixture.json'), JSON.stringify({ purpose: 'openusage-native-reset-notifications-v1', profileId: selected.id }), { mode: 0o600, flag: 'wx' });
  const now = Date.now(), times = { observations: [-120000, -6000, -5000, -4000].map(offset => new Date(now + offset).toISOString()), priorResetUnix: Math.floor(now / 1000) - 60, nextResetUnix: Math.floor(now / 1000) + 18000 };
  const seeded = JSON.parse(execFileSync(seeder, [fixture.root, selected.id, 'notification-seed'], { input: JSON.stringify({ passphrase: password, notificationTimes: times }), encoding: 'utf8', timeout: 30000, env: { PATH: '/usr/bin:/bin' } }));
  assert.equal(seeded.deliveryState, 'queued'); evidence.syntheticSeed = seeded;
  await open();
  let history = await call('load_alert_history'); assert.equal(history.events[0].sourceKind, 'test_fixture');
  if (production) {
    await waitFor(async () => (await call('load_alert_history')).events[0].deliveryState === 'suppressed', 'production refuses preexisting fixture queue', 22000);
    assert.deepEqual(await submissions(), []); evidence.productionFixtureDispatchRefused = true;
  } else {
    // Wait across one timer interval: the queue must not reach the OS in quiet hours.
    const beforeQuiet = Date.now();
    await waitFor(() => Date.now() - beforeQuiet >= 11000, 'one local timer interval in quiet hours', 13000);
    assert.equal((await call('load_alert_history')).events[0].deliveryState, 'queued'); assert.deepEqual(await submissions(), []);
    const archive = await call('create_backup', { password: backupPassword }), archivePath = join(archive.directory, archive.fileName), archiveHash = await hash(archivePath);
    const keyPath = join(dirname(fixture.database), 'database-key.v1'), keyHash = await hash(keyPath);
    await preferences(); await driver.click('#reset-notifications-quiet'); await save();
    await waitFor(async () => (await call('load_alert_history')).events[0].deliveryState === 'submitted', 'real OS service acknowledgement', 22000);
    const acceptedRows = await notificationRows('accepted OS submission');
    assert.equal(acceptedRows.reset_notification_receipts.length, 1);
    const sent = await submissions(); assert.equal(sent.length, 1); assert.match(sent[0].title, /Synthetic notification account: reset confirmed/);
    assert.match(sent[0].body, /5-hour quota window reset/); assert.match(sent[0].body, /Open OpenUsage to view usage/);
    for (const privateValue of [password, backupPassword, selected.id, fixture.root, 'alert-synthetic@example.invalid']) assert.equal(JSON.stringify(sent).includes(privateValue), false);
    await driver.click('[data-alert-history] > button');
    await waitFor(() => driver.execute('return Boolean(document.querySelector("[data-alert-history-event]"));'), 'visible delivery history');
    await driver.click('[data-alert-history-event] .alert-history-source > button');
    assert.equal(await driver.execute('return document.querySelector("[data-alert-history-event]").textContent.includes("Accepted by the desktop notification service.");'), true);
    await driver.screenshot(join(output, 'reset-notifications-settings.png')); evidence.screenshots.push('reset-notifications-settings.png');
    await close(); await open(); assert.equal((await call('load_alert_history')).events[0].deliveryState, 'submitted'); assert.equal((await submissions()).length, 1);
    assert.deepEqual(await call('list_profiles'), profiles);
    const preview = await call('prepare_restore_preview', { fileName: archive.fileName, password: backupPassword });
    await call('apply_restore', { previewId: preview.previewId, confirmed: true });
    assert.equal((await call('load_alert_history')).events[0].deliveryState, 'submitted');
    assert.deepEqual(await notificationRows('pending archive restored without replay'), acceptedRows);
    const undo = await call('prepare_checkpoint_restore_preview', { checkpointId: preview.checkpoint.id });
    await call('apply_restore', { previewId: undo.previewId, confirmed: true });
    assert.equal((await call('load_alert_history')).events[0].deliveryState, 'submitted');
    assert.deepEqual(await notificationRows('checkpoint Undo retains consumed notification'), acceptedRows);
    assert.equal(await hash(archivePath), archiveHash); assert.equal(await hash(keyPath), keyHash);
    await close(); await open();
    const restarted = Date.now(); await waitFor(() => Date.now() - restarted >= 11000, 'one timer interval after restored restart', 13000);
    assert.equal((await submissions()).length, 1); assert.equal((await call('load_alert_history')).events[0].deliveryState, 'submitted');
    assert.deepEqual(await notificationRows('genuine restart after archive and Undo'), acceptedRows);
    evidence.quietQueuePassed = true; evidence.osAcknowledgementPassed = true; evidence.restoreUndoRestartDidNotReplay = true;
  }
  await close(); assert.equal(await readFile(join(fixture.dirs.codex, 'synthetic-fixture.txt'), 'utf8'), CODEX_SENTINEL);
  assert.equal(await hash(application), evidence.application.sha256); evidence.result = 'passed';
} catch (error) {
  evidence.error = String(error?.stack ?? error);
  if (driver?.session) try {
    evidence.failureView = await driver.execute('return {view:document.querySelector("nav[aria-label=Main] [aria-current=page]")?.dataset.workspaceNav,storage:document.querySelector("[data-storage]")?.dataset.storage,preferences:document.querySelector("[data-reset-notifications]")?.textContent,alerts:[...document.querySelectorAll("[role=alert]")].filter(n=>n.getClientRects().length).map(n=>n.textContent)};');
  } catch {}
  throw error;
}
finally {
  try { await driver?.stop(); }
  finally { stopGroup(service, 'SIGKILL'); stopGroup(fixture?.bus, 'SIGKILL'); if (fixture) await rm(fixture.root, { recursive: true, force: true }); evidence.finishedAt = new Date().toISOString(); await writeFile(join(output, 'result.json'), JSON.stringify(evidence, null, 2) + '\n', { mode: 0o600 }); }
}
