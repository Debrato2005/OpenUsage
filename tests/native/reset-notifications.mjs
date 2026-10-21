import assert from 'node:assert/strict';
import { execFileSync, spawn } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { chmod, copyFile, mkdir, readFile, readlink, rm, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { setTimeout as delay } from 'node:timers/promises';
import { CODEX_SENTINEL, createFixture, stopGroup } from './private-fixture.mjs';
import { NativeDriver, waitFor } from './webdriver.mjs';
const repository = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const production = process.argv.includes('--production');
assert.ok(process.env.OPENUSAGE_NATIVE_BINARY && process.env.OPENUSAGE_NATIVE_EVIDENCE && process.env.OPENUSAGE_NATIVE_ALERT_FIXTURE);
const application = resolve(process.env.OPENUSAGE_NATIVE_BINARY), output = resolve(process.env.OPENUSAGE_NATIVE_EVIDENCE), seeder = resolve(process.env.OPENUSAGE_NATIVE_ALERT_FIXTURE);
const password = 'OpenUsage synthetic reset notification workspace 2026', backupPassword = 'OpenUsage synthetic reset notification backup 2026';
const hash = async path => createHash('sha256').update(await readFile(path)).digest('hex');
const evidence = { requirement: production ? 'NATIVE-QUOTA-NOTIFICATION-PRODUCTION-REFUSAL-001' : 'NATIVE-QUOTA-NOTIFICATIONS-001', result: 'failed', scope: 'Actual Linux Tauri GUI, private SQLCipher4/WAL workspace, synthetic profiles and explicitly synthetic quota proofs through public storage APIs. Threshold acceptance uses an explicit native-test live context. A private Freedesktop D-Bus service acknowledges submissions; no real OS notifications or provider sessions. Production must suppress existing fixture queues for reset and threshold events. The native-test binary alone permits fixture enqueue/dispatch, retaining fixture provenance. Restore replay checks retain the same controlled profile association; they do not establish deduplication across a newly created association. No live-provider, installed Windows/macOS, notification visibility, click routing or crash/power-loss acceptance.', closes: [], screenshots: [] };
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
  assert.equal((await call('bootstrap')).schemaVersion, 7);
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
    assert.equal(result.schemaVersion, 7); assert.equal(result.verified, true); assert.equal(result.readOnly, true); assert.equal(result.plaintextDiskStaging, false);
    assert.equal(Object.keys(result.state.tables).length, 24);
    assert.equal(await hash(fixture.database), before.database); assert.equal(await hash(keyPath), before.key);
    if (walHash !== null) assert.equal(await hash(`${fixture.database}-wal`), walHash);
    const rows = Object.fromEntries(['alert_events', 'threshold_alert_events', 'reset_notification_outbox', 'reset_notification_receipts'].map(name => [name, result.state.tables[name]]));
    (evidence.oracleChecks ??= []).push({ label, allNamedTables: 24, databaseWalKeyUnchanged: true, rows: Object.fromEntries(Object.entries(rows).map(([name, value]) => [name, { count: value.length, sha256: createHash('sha256').update(JSON.stringify(value)).digest('hex') }])) });
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
async function seedThreshold(profile, mode, retainedReset) {
  assert.equal(driver, undefined, 'Close the application before synthetic storage acceptance.');
  const marker = join(fixture.dirs.control, 'alert-fixture.json');
  await writeFile(marker, JSON.stringify({ purpose: 'openusage-native-reset-notifications-v1', profileId: profile.id }), { mode: 0o600, flag: 'wx' });
  try {
    const now = Date.now(), times = {
      observations: [-120000, -6000, -5000, -4000, -3000, -2000, -1000].map(offset => new Date(now + offset).toISOString()),
      runIds: Array.from({ length: 7 }, () => randomUUID()), acceptedAt: new Date(now).toISOString(),
      priorResetUnix: retainedReset?.priorResetUnix ?? Math.floor(now / 1000) - 60,
      nextResetUnix: retainedReset?.nextResetUnix ?? Math.floor(now / 1000) + 18000,
    };
    const result = JSON.parse(execFileSync(seeder, [fixture.root, profile.id, mode], { input: JSON.stringify({ passphrase: password, notificationTimes: times }), encoding: 'utf8', timeout: 30000, env: { PATH: '/usr/bin:/bin' } }));
    assert.equal(result.synthetic, true); assert.equal(result.explicitNativeTestLiveContext, true); assert.equal(result.schemaVersion, 7);
    return { result, priorResetUnix: times.priorResetUnix, nextResetUnix: times.nextResetUnix };
  } finally { await rm(marker); }
}
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
  for (const label of ['Synthetic notification account', 'Synthetic threshold account']) {
    await driver.openProfileCreate(); await driver.type('#profile-label', label); await driver.click('.profile-create-form button[type=submit]');
    await waitFor(async () => (await call('list_profiles')).profiles.some(profile => profile.label === label), 'GUI-created account');
    await waitFor(() => driver.execute('const n=document.querySelector("#profile-label");return n&&!n.disabled&&n.value==="";'), 'settled account form');
    await driver.click('[data-account-panel-close]');
  }
  const profiles = await call('list_profiles'); selected = profiles.profiles.find(profile => profile.label === 'Synthetic notification account'); assert.ok(selected);
  const stepped = profiles.profiles.find(profile => profile.label === 'Synthetic threshold account'); assert.ok(stepped);
  const initialPreferences = await call('load_reset_notification_preferences');
  const initialCombined = await call('load_notification_preferences'); assert.equal(initialCombined.usage.enabled, false);
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
  await driver.click('#reset-notifications-enabled'); await driver.click('#usage-notifications-enabled');
  // Select a real fixed-offset IANA zone whose current local hour lies in
  // the default22:00–08:00 quiet period. Do not change the OS clock.
  const offset = (2 - new Date().getUTCHours() + 36) % 24 - 12;
  const zone = offset === 0 ? 'Etc/UTC' : `Etc/GMT${offset > 0 ? '-' : '+'}${Math.abs(offset)}`;
  await driver.replaceText('#reset-notifications-zone', zone); await save();
  const quietPreferences = await call('load_notification_preferences'); assert.equal(quietPreferences.reset.timeZone, zone); assert.equal(quietPreferences.reset.quietEnabled, true); assert.equal(quietPreferences.usage.enabled, true);
  // Keep the second account's reset event in history without delivering it.
  await call('save_notification_preferences', { preferences: { ...quietPreferences, reset: { ...quietPreferences.reset, profileOverrides: [{ profileId: stepped.id, enabled: false }] } } });
  const baselineArchive = production ? null : await call('create_backup', { password: backupPassword });
  const baselineArchivePath = baselineArchive && join(baselineArchive.directory, baselineArchive.fileName);
  const baselineArchiveHash = baselineArchivePath && await hash(baselineArchivePath);
  await close();
  const highSeed = await seedThreshold(selected, 'threshold-high-seed');
  const steppedSeed = await seedThreshold(stepped, 'threshold-steps-seed');
  assert.deepEqual(highSeed.result.thresholds, [[80, 'covered'], [90, 'queued']]);
  assert.deepEqual(steppedSeed.result.thresholds, [[80, 'queued'], [90, 'queued'], [100, 'queued']]);
  evidence.syntheticSeed = { firstConfirmed95: highSeed.result, subsequent80Then90Then100: steppedSeed.result };
  await open();
  const eventsFor = (history, profile) => history.events.filter(event => event.profileId === profile.id);
  const thresholdsFor = (history, profile) => eventsFor(history, profile).filter(event => event.kind === 'quota_threshold').map(event => [event.thresholdPercent, event.deliveryState]).sort((a, b) => a[0] - b[0]);
  let history = await call('load_alert_history'); assert.equal(history.events.length, 7);
  assert.equal(history.events.every(event => event.sourceKind === 'test_fixture'), true);
  if (!production) {
    assert.deepEqual(thresholdsFor(history, selected), [[80, 'covered'], [90, 'queued']]);
    assert.deepEqual(thresholdsFor(history, stepped), [[80, 'queued'], [90, 'queued'], [100, 'queued']]);
  }
  assert.equal(eventsFor(history, stepped).find(event => event.kind === 'quota_reset').deliveryState, 'suppressed');
  if (production) {
    await waitFor(async () => (await call('load_alert_history')).events.every(event => ['suppressed', 'covered'].includes(event.deliveryState)), 'production refuses preexisting reset and threshold fixture queues', 80000);
    history = await call('load_alert_history');
    assert.deepEqual(thresholdsFor(history, selected), [[80, 'covered'], [90, 'suppressed']]);
    assert.deepEqual(thresholdsFor(history, stepped), [[80, 'suppressed'], [90, 'suppressed'], [100, 'suppressed']]);
    assert.deepEqual(await submissions(), []); evidence.productionFixtureDispatchRefused = { reset: true, threshold: true };
  } else {
    // Wait across one timer interval: neither kind may reach the OS in quiet hours.
    await delay(11000);
    history = await call('load_alert_history'); assert.equal(history.events.filter(event => event.deliveryState === 'queued').length, 5); assert.deepEqual(await submissions(), []);
    const archive = await call('create_backup', { password: backupPassword }), archivePath = join(archive.directory, archive.fileName), archiveHash = await hash(archivePath);
    const keyPath = join(dirname(fixture.database), 'database-key.v1'), keyHash = await hash(keyPath);
    await preferences(); await driver.click('#reset-notifications-quiet'); await save();
    await waitFor(async () => (await call('load_alert_history')).events.filter(event => event.deliveryState === 'submitted').length === 5, 'five actual desktop service acknowledgements', 80000);
    history = await call('load_alert_history');
    assert.deepEqual(thresholdsFor(history, selected), [[80, 'covered'], [90, 'submitted']]);
    assert.deepEqual(thresholdsFor(history, stepped), [[80, 'submitted'], [90, 'submitted'], [100, 'submitted']]);
    const acceptedHistory = history, acceptedRows = await notificationRows('five accepted OS submissions and one covered lower level');
    assert.equal(acceptedRows.alert_events.length, 7); assert.equal(acceptedRows.threshold_alert_events.length, 5);
    assert.equal(acceptedRows.reset_notification_receipts.length, 6);
    assert.equal(acceptedRows.reset_notification_receipts.filter(row => row.state === 'covered').length, 1);
    assert.equal(acceptedRows.reset_notification_receipts.filter(row => row.state === 'submitted').length, 5);
    const sent = await submissions(); assert.equal(sent.length, 5);
    const highSent = sent.filter(row => row.title.startsWith(selected.label)), steppedSent = sent.filter(row => row.title.startsWith(stepped.label));
    assert.equal(highSent.length, 2); assert.equal(steppedSent.length, 3);
    assert.equal(highSent.filter(row => /reset confirmed/.test(row.title)).length, 1);
    assert.equal(highSent.filter(row => /90%/.test(row.title + row.body)).length, 1);
    assert.equal(highSent.some(row => /80%/.test(row.title + row.body)), false, 'A first95 reading must not send the covered80 alert.');
    for (const threshold of [80, 90, 100]) assert.equal(steppedSent.filter(row => new RegExp(`${threshold}%`).test(row.title + row.body)).length, 1);
    for (const row of sent) assert.match(row.body, /Open OpenUsage to view usage/);
    for (const privateValue of [password, backupPassword, selected.id, stepped.id, fixture.root, 'alert-synthetic@example.invalid']) assert.equal(JSON.stringify(sent).includes(privateValue), false);
    await driver.click('[data-alert-history] > button');
    await waitFor(() => driver.execute('return document.querySelectorAll("[data-alert-history-event]").length===7;'), 'visible reset and threshold history');
    const submittedEvent = history.events.find(event => event.deliveryState === 'submitted'); assert.ok(submittedEvent);
    const submittedSelector = `[data-alert-history-event="${submittedEvent.id}"]`;
    await driver.click(`${submittedSelector} .alert-history-source > button`);
    assert.equal(await driver.execute('return document.querySelector(arguments[0]).textContent.includes("Accepted by the desktop notification service.");', [submittedSelector]), true);
    await driver.screenshot(join(output, 'quota-notifications-settings.png')); evidence.screenshots.push('quota-notifications-settings.png');
    await close(); await open(); assert.deepEqual(await call('load_alert_history'), acceptedHistory); assert.equal((await submissions()).length, 5);
    assert.deepEqual(await call('list_profiles'), profiles);
    const preview = await call('prepare_restore_preview', { fileName: archive.fileName, password: backupPassword });
    await call('apply_restore', { previewId: preview.previewId, confirmed: true });
    assert.deepEqual(await call('load_alert_history'), acceptedHistory);
    assert.deepEqual(await notificationRows('pending archive restored without replay or lost covered80'), acceptedRows);
    const undo = await call('prepare_checkpoint_restore_preview', { checkpointId: preview.checkpoint.id });
    await call('apply_restore', { previewId: undo.previewId, confirmed: true });
    assert.deepEqual(await call('load_alert_history'), acceptedHistory);
    assert.deepEqual(await notificationRows('checkpoint Undo retains all consumed and covered levels'), acceptedRows);
    assert.equal(await hash(archivePath), archiveHash); assert.equal(await hash(keyPath), keyHash);
    await close();
    await seedThreshold(selected, 'threshold-high-followup', highSeed);
    await seedThreshold(stepped, 'threshold-steps-followup', steppedSeed);
    await open();
    await delay(11000);
    assert.equal((await submissions()).length, 5); assert.deepEqual(await call('load_alert_history'), acceptedHistory);
    assert.deepEqual(await notificationRows('genuine restart and restored re-cross retain consumption'), acceptedRows);
    const baselinePreview = await call('prepare_restore_preview', { fileName: baselineArchive.fileName, password: backupPassword });
    await call('apply_restore', { previewId: baselinePreview.previewId, confirmed: true });
    assert.deepEqual((await call('load_alert_history')).events, []);
    const beforeProofRows = await notificationRows('restore before any confirmed reset proof retains orphan consumption');
    assert.deepEqual(beforeProofRows.reset_notification_receipts, acceptedRows.reset_notification_receipts);
    const restoredPreferences = await call('load_notification_preferences');
    await call('save_notification_preferences', { preferences: { ...restoredPreferences, reset: { ...restoredPreferences.reset, quietEnabled: false } } });
    await close();
    const highReseed = await seedThreshold(selected, 'threshold-high-reseed', highSeed);
    const steppedReseed = await seedThreshold(stepped, 'threshold-steps-reseed', steppedSeed);
    assert.notEqual(highReseed.result.generationRunId, highSeed.result.generationRunId);
    assert.notEqual(steppedReseed.result.generationRunId, steppedSeed.result.generationRunId);
    assert.deepEqual(highReseed.result.thresholds, []); assert.deepEqual(steppedReseed.result.thresholds, []);
    await open();
    await delay(11000);
    assert.equal((await submissions()).length, 5);
    const reconfirmedHistory = await call('load_alert_history'); assert.equal(reconfirmedHistory.events.length, 2);
    assert.equal(reconfirmedHistory.events.every(event => event.kind === 'quota_reset' && event.deliveryState === 'suppressed'), true);
    const reconfirmedRows = await notificationRows('new local generations cannot replay consumed provider-window levels');
    assert.equal(reconfirmedRows.threshold_alert_events.length, 0);
    assert.deepEqual(reconfirmedRows.reset_notification_receipts, acceptedRows.reset_notification_receipts);
    assert.equal(await hash(baselineArchivePath), baselineArchiveHash); assert.equal(await hash(keyPath), keyHash);
    evidence.first95HighestOnlyPassed = true; evidence.subsequent80Then90Then100Passed = true;
    evidence.quietQueuePassed = true; evidence.osAcknowledgementPassed = true; evidence.restoreUndoRestartDidNotReplay = true; evidence.preGenerationRestoreDidNotReplay = true;
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
