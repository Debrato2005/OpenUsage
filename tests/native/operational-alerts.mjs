import assert from 'node:assert/strict';
import { execFileSync, spawn } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { chmod, copyFile, mkdir, readFile, readlink, rename, rm, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { setTimeout as delay } from 'node:timers/promises';
import { CODEX_SENTINEL, createFixture, stopGroup } from './private-fixture.mjs';
import { NativeDriver, waitFor } from './webdriver.mjs';

const repository = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
assert.ok(process.env.OPENUSAGE_NATIVE_BINARY && process.env.OPENUSAGE_NATIVE_EVIDENCE, 'Select the current explicit native-test binary and private evidence directory.');
const application = resolve(process.env.OPENUSAGE_NATIVE_BINARY), output = resolve(process.env.OPENUSAGE_NATIVE_EVIDENCE);
const providerSource = join(repository, 'crates/openusage-codex/tests/fixtures/synthetic-collection-provider.py');
const providerDigest = '53a853b737ceca311649e9f1d9ebd1341154a5f782efd8e915f7f834c819a6dc';
const passphrase = 'OpenUsage synthetic operational alert workspace 2026';
const backupPassword = 'OpenUsage synthetic operational alert backup 2026';
const hash = async path => createHash('sha256').update(await readFile(path)).digest('hex');
const evidence = { requirement: 'NATIVE-OPERATIONAL-ALERTS-001', result: 'failed', startedAt: new Date().toISOString(), scope: 'Actual Linux Tauri GUI and typed IPC in a private SQLCipher4/WAL workspace. Explicit native-test companion fixture, real supervised child exits and real refresh backoff; private Freedesktop acknowledgements. No fabricated failure records, real provider sessions, user-visible OS notifications, production-source qualification, other-platform or crash/power-loss acceptance.', steps: [], closes: [] };
let fixture, driver, service, executable, workspace, approvalId, sequence = 0;
const children = new Map();
const call = async (command, args) => { const response = await driver.invoke(command, args); assert.equal(response.ok, true, `${command}: ${JSON.stringify(response)}`); return response.value; };
const home = profile => join(workspace, 'profiles', profile.id);
const history = () => call('load_alert_history');
const serviceKinds = new Set(['authentication_required', 'collection_failure']);
const operational = value => value.events.filter(event => serviceKinds.has(event.kind));
const cached = async profile => ({ quota: await call('load_profile_quota', { profileId: profile.id }), activity: await call('load_profile_activity', { profileId: profile.id }) });
function metrics(value) { const copy = structuredClone(value); for (const row of Object.values(copy)) if (row) delete row.freshness; return copy; }
async function mode(profile, selected) { const path = join(home(profile), 'synthetic-collection-config.json'); await writeFile(`${path}.next`, JSON.stringify({ mode: selected }) + '\n', { mode: 0o600 }); await rename(`${path}.next`, path); }
async function observations(profile) { try { return (await readFile(join(home(profile), 'synthetic-collection-observations.jsonl'), 'utf8')).trim().split('\n').filter(Boolean).map(JSON.parse); } catch (error) { if (error.code === 'ENOENT') return []; throw error; } }
async function submissions() { try { return (await readFile(join(fixture.dirs.control, 'notification-submissions.jsonl'), 'utf8')).trim().split('\n').filter(Boolean).map(JSON.parse); } catch (error) { if (error.code === 'ENOENT') return []; throw error; } }
async function open(setup = false) {
  driver = new NativeDriver(application, fixture.environment, join(output, `driver-${++sequence}.log`)); await driver.start();
  await waitFor(() => driver.execute('const a=document.querySelector("#workspace-passphrase"),b=document.querySelector("#workspace-confirmation");return Boolean(a&&!a.disabled&&(!arguments[0]||b&&!b.disabled));', [setup]), 'native password fields');
  await driver.type('#workspace-passphrase', passphrase); if (setup) await driver.type('#workspace-confirmation', passphrase);
  await driver.click('.unlock-form button[type=submit]');
  await waitFor(async () => (await call('bootstrap')).storage === 'ready', 'unlocked operational alert workspace');
  assert.equal((await call('bootstrap')).schemaVersion, 8);
}
async function close() {
  const queue = [driver.child.pid]; let owned;
  for (let count = 0; queue.length && count < 64; count++) {
    const pid = queue.shift();
    try { if (await readlink(`/proc/${pid}/exe`) === application) { owned = pid; break; } queue.push(...(await readFile(`/proc/${pid}/task/${pid}/children`, 'utf8')).trim().split(/\s+/).filter(Boolean).map(Number)); }
    catch (error) { if (error.code !== 'ENOENT') throw error; }
  }
  assert.ok(owned, 'Only the owned native process may receive a window close.');
  evidence.closes.push(JSON.parse(execFileSync('/usr/bin/python3', [join(repository, 'tests/native/x11-close.py'), String(owned)], { env: fixture.environment, encoding: 'utf8', timeout: 10000 })));
  await waitFor(async () => { try { await readlink(`/proc/${owned}/exe`); return false; } catch (error) { if (error.code === 'ENOENT') return true; throw error; } }, 'genuine native process exit');
  driver.session = undefined; await driver.stop(); driver = undefined;
}
async function create(label) {
  await driver.openProfileCreate(); await driver.type('#profile-label', label); await driver.click('.profile-create-form button[type=submit]');
  let profile; await waitFor(async () => { profile = (await call('list_profiles')).profiles.find(value => value.label === label); return Boolean(profile); }, 'GUI-created isolated profile');
  await waitFor(() => driver.execute('const n=document.querySelector("#profile-label");return Boolean(n&&!n.disabled&&n.value==="");'), 'settled account form');
  await driver.click('[data-account-panel-close]'); return profile;
}
async function approve(profile) {
  await driver.navigate('settings');
  if (!await driver.execute('return document.querySelector("[data-provider-settings]").closest("details").open;')) await driver.click('[data-provider-settings]');
  if (await driver.execute('const n=document.querySelector("[data-provider-manual]");return Boolean(n&&!n.closest("details").open);')) await driver.click('[data-provider-manual]');
  await driver.type('#provider-executable', executable); await driver.click('.provider-inspection-form button[type=submit]');
  await waitFor(() => driver.execute('const n=document.querySelector("#provider-executable-approved");return Boolean(n&&!n.disabled&&n.getAttribute("aria-checked")==="false");'), 'unapproved reviewed fixture');
  assert.equal(await driver.execute('return document.querySelector(".provider-approval-form button[type=submit]").disabled;'), true);
  await driver.click('#provider-executable-approved');
  await waitFor(() => driver.execute('return document.querySelector("#provider-executable-approved")?.getAttribute("aria-checked")==="true"&&!document.querySelector(".provider-approval-form button[type=submit]").disabled;'), 'explicit fixture consent');
  await driver.click('.provider-approval-form button[type=submit]');
  await waitFor(() => driver.execute('return Boolean(document.activeElement?.matches(".provider-approval-result"));'), 'completed companion approval');
  const probe = await call('check_profile_connection', { profileId: profile.id });
  assert.equal(probe.sourceKind, 'test_fixture'); assert.equal(probe.persisted, false); approvalId = probe.approvalId;
}
const verifier = resolve(process.env.OPENUSAGE_LOGIN_VERIFIER ?? join(repository, '.tools/build-target/debug/openusage-native-activity-verifier'));
const verifierDigest = 'a2e4a64707081a9943f3c04f8237c2b16bea673e974e96393d6a8a7d9357f975';
async function oracle(label) {
  assert.equal(await hash(verifier), verifierDigest);
  const key = join(workspace, 'database-key.v1');
  const before = { database: await hash(fixture.database), key: await hash(key) };
  let wal = null; try { wal = await hash(`${fixture.database}-wal`); } catch (error) { if (error.code !== 'ENOENT') throw error; }
  const directory = join(fixture.root, `readonly-oracle-${randomUUID()}`); await mkdir(directory, { mode: 0o700 });
  try {
    const database = join(directory, 'database.db'); await copyFile(fixture.database, database); await chmod(database, 0o600);
    if (wal !== null) { await copyFile(`${fixture.database}-wal`, `${database}-wal`); await chmod(`${database}-wal`, 0o600); }
    const result = JSON.parse(execFileSync(verifier, ['database', database, key, '--profile-state'], { input: JSON.stringify({ passphrase }), encoding: 'utf8', timeout: 30000, env: fixture.environment, maxBuffer: 4 * 1024 * 1024 }));
    assert.equal(result.schemaVersion, 8); assert.equal(result.verified, true); assert.equal(result.readOnly, true); assert.equal(result.plaintextDiskStaging, false);
    assert.equal(Object.keys(result.state.tables).length, 30);
    assert.equal(await hash(fixture.database), before.database); assert.equal(await hash(key), before.key);
    if (wal !== null) assert.equal(await hash(`${fixture.database}-wal`), wal);
    const selected = Object.fromEntries(['notification_event_ids', 'operational_alert_outcomes', 'operational_alert_events', 'operational_alert_state', 'reset_notification_outbox', 'reset_notification_receipts'].map(name => [name, result.state.tables[name]]));
    (evidence.oracles ??= []).push({ label, allNamedTables: 30, databaseWalKeyUnchanged: true, tables: Object.fromEntries(Object.entries(result.state.tables).map(([name, rows]) => [name, { count: rows.length, sha256: createHash('sha256').update(JSON.stringify(rows)).digest('hex') }])) });
    return selected;
  } finally { await rm(directory, { recursive: true, force: true }); }
}
async function preferences() {
  await driver.navigate('settings');
  if (await driver.execute('return document.querySelector("[data-reset-notifications] > button").getAttribute("aria-expanded")==="false";')) await driver.click('[data-reset-notifications] > button');
  await waitFor(() => driver.execute('const n=document.querySelector("#authentication-notifications-enabled");return Boolean(n&&!n.disabled);'), 'loaded notification preferences');
  if (!await driver.execute('return Boolean(document.querySelector("#reset-notifications-zone")?.getClientRects().length);')) {
    const trigger = await driver.execute('return [...document.querySelectorAll("[data-reset-notifications] button")].find(n=>n.textContent.trim()==="Quiet hours");');
    await driver.request('POST', `/session/${driver.session}/element/${trigger['element-6066-11e4-a52e-4f735466cecf']}/click`, {});
  }
}
async function savePreferences() { await driver.click('.reset-notifications-form button[type=submit]'); await waitFor(() => driver.execute('return document.querySelector(".reset-notifications-form")?.textContent.includes("Notification preferences saved.");'), 'saved GUI notification preferences'); }
async function read(profile, selected, expectedError) {
  await driver.waitForProfileRefresh(profile.id, 180000); await mode(profile, selected);
  const request = await call('begin_profile_collection_read', { profileId: profile.id, approvalId });
  assert.equal(request.sourceKind, 'test_fixture'); assert.equal(request.accepted, false);
  const result = await driver.invoke('complete_profile_collection_read', { profileId: profile.id, requestId: request.requestId });
  if (expectedError) assert.deepEqual(result, { ok: false, error: expectedError });
  else { assert.equal(result.ok, true, JSON.stringify(result)); assert.equal(result.value.profileId, profile.id); assert.equal(result.value.accepted, true); assert.equal(result.value.sourceKind, 'test_fixture'); }
  return result;
}
async function childIdentity(pid) {
  const stat = await readFile(`/proc/${pid}/stat`, 'utf8'), fields = stat.slice(stat.lastIndexOf(')') + 2).trim().split(/\s+/);
  return { pid, group: Number(fields[2]), start: fields[19], command: (await readFile(`/proc/${pid}/cmdline`)).toString().split('\0').filter(Boolean) };
}
async function killOwned(child) {
  const current = await childIdentity(child.pid);
  assert.equal(current.start, child.start, 'PID reuse must never target another process.');
  assert.equal(current.group, child.pid); assert.equal(current.group, child.group);
  assert.deepEqual(current.command, child.command); assert.ok(current.command.includes(executable));
  process.kill(-current.group, 'SIGKILL');
}
async function transientFailure(profile, index) {
  const started = performance.now(); await driver.waitForProfileRefresh(profile.id, 180000);
  const waited = Math.round(performance.now() - started), offset = (await observations(profile)).length;
  await mode(profile, 'quota_hang');
  const request = await call('begin_profile_collection_read', { profileId: profile.id, approvalId });
  assert.equal(request.sourceKind, 'test_fixture');
  const pending = driver.invoke('complete_profile_collection_read', { profileId: profile.id, requestId: request.requestId });
  void pending.catch(() => {});
  let observed; await waitFor(async () => { observed = (await observations(profile)).slice(offset).find(row => row.kind === 'quota-pending'); return Boolean(observed); }, 'actual supervised quota wait');
  assert.ok(Number.isSafeInteger(observed.pid) && observed.pid > 1 && observed.pid !== process.pid); assert.equal(observed.processGroup, observed.pid);
  const child = await childIdentity(observed.pid); assert.equal(child.group, observed.processGroup); assert.ok(child.start && child.command.includes(executable)); children.set(child.pid, child);
  await killOwned(child);
  assert.deepEqual(await pending, { ok: false, error: 'provider_unavailable' });
  await waitFor(async () => { try { const current = await childIdentity(child.pid); return current.start !== child.start; } catch (error) { if (error.code === 'ENOENT') return true; throw error; } }, 'owned transient process reaped');
  assert.throws(() => process.kill(-child.group, 0), error => error.code === 'ESRCH', 'The owned fixture process group must be reaped.');
  children.delete(child.pid);
  const remaining = await call('load_profile_refresh_status', { profileId: profile.id });
  const base = [30, 60, 120][index - 1]; assert.ok(remaining.retryAfterSeconds >= base - Math.floor(base / 5) - 2 && remaining.retryAfterSeconds <= base + Math.floor(base / 5));
  evidence.steps.push({ transientFailure: index, actualOwnedExit: true, profileId: profile.id, processGroup: child.group, procStartTimeTicks: child.start, waitedForRealEligibilityMilliseconds: waited, retryAfterSeconds: remaining.retryAfterSeconds });
}

await mkdir(output, { recursive: true, mode: 0o700 });
process.once('exit', () => { driver?.kill('SIGKILL'); stopGroup(service, 'SIGKILL'); stopGroup(fixture?.bus, 'SIGKILL'); });
try {
  evidence.application = { path: application, sha256: await hash(application) }; assert.equal(await hash(providerSource), providerDigest);
  evidence.providerFixture = { sha256: providerDigest, source: 'crates/openusage-codex/tests/fixtures/synthetic-collection-provider.py', unchanged: true };
  evidence.harnessSha256 = await hash(fileURLToPath(import.meta.url));
  evidence.sourceCommit = execFileSync('git', ['rev-parse', 'HEAD'], { cwd: repository, encoding: 'utf8', timeout: 10000 }).trim();
  fixture = await createFixture('operational-alerts'); workspace = dirname(fixture.database);
  fixture.environment.HOME = join(fixture.root, 'synthetic-home'); await mkdir(fixture.environment.HOME, { mode: 0o700 });
  executable = join(fixture.root, 'synthetic-collection-provider.py'); await copyFile(providerSource, executable); await chmod(executable, 0o700); assert.equal(await hash(executable), providerDigest);
  service = spawn('/usr/bin/python3', [join(repository, 'tests/native/notification-service.py'), fixture.root], { env: fixture.environment, detached: true, stdio: ['ignore', 'pipe', 'pipe'] });
  let ready = '', failure = ''; service.stdout.on('data', chunk => { ready += chunk; }); service.stderr.on('data', chunk => { failure += chunk; }); service.once('error', error => { failure += error.message; });
  await waitFor(() => { if (failure) throw new Error(failure); return ready.includes('private-notifications-ready'); }, 'private notification service');
  await open(true);
  const control = await create('Synthetic unaffected account'), failing = await create('Synthetic interrupted readings'), signedOut = await create('Synthetic expired session'), initial = await create('Synthetic first sign-in'), unknown = await create('Synthetic missing identity');
  await approve(control);
  const defaults = await call('load_notification_preferences');
  for (const kind of ['reset', 'usage', 'authentication', 'collectionFailure']) assert.equal(defaults[kind].enabled, false);
  assert.deepEqual(await history(), { events: [], truncated: false }); assert.deepEqual(await submissions(), []);
  await preferences(); await driver.click('#authentication-notifications-enabled'); await driver.click('#collection-failure-notifications-enabled');
  const offset = (2 - new Date().getUTCHours() + 36) % 24 - 12;
  const zone = offset === 0 ? 'Etc/UTC' : `Etc/GMT${offset > 0 ? '-' : '+'}${Math.abs(offset)}`;
  await driver.replaceText('#reset-notifications-zone', zone); await savePreferences();
  const selected = await call('load_notification_preferences'); assert.equal(selected.reset.quietEnabled, true); assert.equal(selected.reset.timeZone, zone);
  assert.equal(selected.reset.enabled, false); assert.equal(selected.usage.enabled, false); assert.equal(selected.authentication.enabled, true); assert.equal(selected.collectionFailure.enabled, true);
  // A disconnected account exposes Sign in, followed by an automatic first read.
  await writeFile(join(home(control), 'synthetic-collection-config.json'), JSON.stringify({ mode: 'success', requireLogin: true }) + '\n', { mode: 0o600 });
  await driver.openProfileReadings(control.id);
  await waitFor(() => driver.execute('const node=document.querySelector(arguments[0]);return Boolean(node&&!node.disabled&&node.getClientRects().length);', [`[data-login-start="${control.id}"]`]), 'visible selected account Sign in');
  await driver.click(`[data-login-start="${control.id}"]`);
  await waitFor(async () => Boolean((await cached(control)).quota && (await cached(control)).activity), 'GUI accepted quota and activity');
  for (const profile of [signedOut, unknown, failing]) await read(profile, 'success');
  const snapshots = new Map(); for (const profile of [control, signedOut, unknown, failing]) { const value = await cached(profile); assert.equal(value.quota.sourceKind, 'test_fixture'); assert.equal(value.activity.sourceKind, 'test_fixture'); assert.equal(value.quota.limits[0].primary.usedPercent, 25); assert.equal(value.activity.summary.lifetimeTokens, '9007199254740993'); snapshots.set(profile.id, metrics(value)); }
  await read(initial, 'signed_out', 'provider_authentication_required');
  await read(unknown, 'unknown', 'provider_account_identity_unavailable');
  assert.deepEqual(operational(await history()), [], 'Initial signed-out and missing identity cannot assert an expired authorized session.');
  await transientFailure(failing, 1); assert.deepEqual(operational(await history()), []);
  await read(signedOut, 'signed_out', 'provider_authentication_required');
  let events = operational(await history()); assert.equal(events.length, 1); assert.equal(events[0].kind, 'authentication_required'); assert.equal(events[0].profileId, signedOut.id); assert.equal(events[0].deliveryState, 'queued');
  await transientFailure(failing, 2); assert.equal(operational(await history()).length, 1);
  await transientFailure(failing, 3);
  events = operational(await history()); assert.equal(events.length, 2);
  assert.equal(events.filter(event => event.kind === 'authentication_required' && event.profileId === signedOut.id).length, 1);
  assert.equal(events.filter(event => event.kind === 'collection_failure' && event.profileId === failing.id).length, 1);
  for (const event of events) { assert.equal(event.sourceKind, 'test_fixture'); assert.equal(event.deliveryState, 'queued'); assert.deepEqual(Object.keys(event).sort(), ['deliveryState', 'evidenceAtUtc', 'id', 'kind', 'profileId', 'profileLabel', 'sourceKind']); }
  for (const profile of [control, signedOut, unknown, failing]) assert.deepEqual(metrics(await cached(profile)), snapshots.get(profile.id), 'Operational failures must preserve every cached measurement.');
  assert.equal((await call('list_profiles')).profiles.find(profile => profile.id === signedOut.id).connectionState, 'authentication_required');
  await delay(11000); assert.deepEqual(await submissions(), []); assert.equal(operational(await history()).every(event => event.deliveryState === 'queued'), true);
  const queuedRecords = await oracle('queued-failure-evidence'); assert.equal(queuedRecords.operational_alert_events.length, 2);
  const archive = await call('create_backup', { password: backupPassword }), archivePath = join(archive.directory, archive.fileName), archiveHash = await hash(archivePath);
  const protectedKey = join(workspace, 'database-key.v1'), keyHash = await hash(protectedKey);
  await preferences(); await driver.click('#reset-notifications-quiet'); await savePreferences();
  await waitFor(async () => operational(await history()).filter(event => event.deliveryState === 'submitted').length === 2, 'two actual private desktop acknowledgements', 45000);
  const submittedRecords = await oracle('acknowledged-failure-evidence');
  const submitted = await history(), sent = await submissions(); assert.equal(sent.length, 2);
  assert.equal(sent.filter(row => row.title.startsWith(signedOut.label) && /sign-in required/.test(row.title)).length, 1);
  assert.equal(sent.filter(row => row.title.startsWith(failing.label) && /readings unavailable/.test(row.title) && /Three readings failed in a row/.test(row.body)).length, 1);
  for (const row of sent) assert.match(row.body, /Open OpenUsage/);
  for (const privateValue of [passphrase, backupPassword, fixture.root, ...snapshots.keys(), 'synthetic-collection@example.invalid', 'synthetic-routing-account']) assert.equal(JSON.stringify(sent).includes(privateValue), false);
  await driver.navigate('settings'); await driver.click('[data-alert-history] > button');
  await waitFor(() => driver.execute('return document.querySelectorAll("[data-alert-history-event]").length===2;'), 'operational alert history');
  const authEvent = submitted.events.find(event => event.kind === 'authentication_required');
  await driver.click(`[data-alert-history-event="${authEvent.id}"] .alert-history-event button`);
  await waitFor(() => driver.execute('const n=document.querySelector("[data-account-panel]");return n?.open&&n.querySelector("#account-panel-heading")?.textContent===arguments[0];', [signedOut.label]), 'exact account recovery controls');
  await driver.click('[data-account-panel-close]');
  await close(); await open(); assert.deepEqual(await history(), submitted); assert.deepEqual(await oracle('genuine-restart'), submittedRecords); assert.equal((await submissions()).length, 2);
  for (const profile of [control, signedOut, unknown, failing]) assert.deepEqual(metrics(await cached(profile)), snapshots.get(profile.id));
  const preview = await call('prepare_restore_preview', { fileName: archive.fileName, password: backupPassword }); await call('apply_restore', { previewId: preview.previewId, confirmed: true });
  assert.deepEqual(await history(), submitted); assert.deepEqual(await oracle('restore-or-undo'), submittedRecords); await delay(11000); assert.equal((await submissions()).length, 2);
  const undo = await call('prepare_checkpoint_restore_preview', { checkpointId: preview.checkpoint.id }); await call('apply_restore', { previewId: undo.previewId, confirmed: true });
  assert.deepEqual(await history(), submitted); assert.deepEqual(await oracle('restore-or-undo'), submittedRecords); await delay(11000); assert.equal((await submissions()).length, 2);
  for (const profile of [control, signedOut, unknown, failing]) assert.deepEqual(metrics(await cached(profile)), snapshots.get(profile.id));
  assert.equal(await hash(archivePath), archiveHash); assert.equal(await hash(protectedKey), keyHash);
  evidence.steps.push({ defaultOptOutAndExplicitGuiConsent: true, initialSignedOutRefused: true, missingIdentityRefused: true, firstTwoTransientFailuresDidNotAlert: true, exactlyOneThirdFailureAndOnePreviouslyAuthorizedSignedOutAlert: true, realQuietQueueAndTwoDesktopAcknowledgements: true, unchangedOtherProfileAndSavedMetrics: true, exactAccountHistoryRecovery: true, genuineRestartRetainedHistory: true, pendingArchiveRestoreAndUndoDidNotReplay: true, archiveAndProtectedKeyUnchanged: true });
  await close(); assert.equal(await readFile(join(fixture.dirs.codex, 'synthetic-fixture.txt'), 'utf8'), CODEX_SENTINEL); assert.equal(await hash(application), evidence.application.sha256); assert.equal(await hash(providerSource), providerDigest); evidence.result = 'passed';
} catch (error) {
  evidence.error = String(error?.stack ?? error); throw error;
} finally {
  for (const child of children.values()) try { await killOwned(child); } catch (error) { if (error.code !== 'ENOENT' && error.code !== 'ESRCH') evidence.cleanupFailure = String(error.message); }
  try { await driver?.stop(); }
  finally { stopGroup(service, 'SIGKILL'); stopGroup(fixture?.bus, 'SIGKILL'); if (fixture) await rm(fixture.root, { recursive: true, force: true }); evidence.finishedAt = new Date().toISOString(); await writeFile(join(output, 'result.json'), JSON.stringify(evidence, null, 2) + '\n', { mode: 0o600 }); }
}
