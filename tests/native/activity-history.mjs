import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { chmod, copyFile, mkdir, readFile, readlink, rm, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createFixture, stopGroup } from './private-fixture.mjs';
import { NativeDriver, waitFor } from './webdriver.mjs';

for (const name of ['OPENUSAGE_NATIVE_BINARY', 'OPENUSAGE_NATIVE_ACTIVITY_HISTORY_FIXTURE', 'OPENUSAGE_NATIVE_EVIDENCE', 'OPENUSAGE_LOGIN_VERIFIER']) assert.ok(process.env[name], name);
const repository = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const application = resolve(process.env.OPENUSAGE_NATIVE_BINARY), seeder = resolve(process.env.OPENUSAGE_NATIVE_ACTIVITY_HISTORY_FIXTURE), output = resolve(process.env.OPENUSAGE_NATIVE_EVIDENCE), verifier = resolve(process.env.OPENUSAGE_LOGIN_VERIFIER);
const password = 'OpenUsage synthetic activity history workspace 2026';
const providerDigest = '53a853b737ceca311649e9f1d9ebd1341154a5f782efd8e915f7f834c819a6dc';
const verifierDigest = 'a2e4a64707081a9943f3c04f8237c2b16bea673e974e96393d6a8a7d9357f975';
const hash = async path => createHash('sha256').update(await readFile(path)).digest('hex');
const evidence = { requirement: 'NATIVE-ACTIVITY-SUMMARY-HISTORY-001', result: 'failed', scope: 'Actual Linux Tauri/WebKitGTK browsing in a private SQLCipher4/WAL schema8 workspace. Two explicit synthetic sign-ins automatically save activity. A strictly marked closed-app Rust helper adds21summary observations through production storage APIs, preserving synthetic provenance, null/zero/exact counts, tied timestamps and a separate prior association. The native journey checks20/2paging, source details, wrong cursors, profile isolation, archived restart and independent read-only checks of all30tables. No live-provider, history export, verified identity, combined total or other-platform acceptance.', screenshots: [], closes: [], oracles: [] };
let fixture, driver, sequence = 0;
const profiles = [];
const call = async (command, args) => { const response = await driver.invoke(command, args); assert.equal(response.ok, true, `${command}: ${JSON.stringify(response)}`); return response.value; };
const metrics = value => { const copy = structuredClone(value); if (copy) delete copy.freshness; return copy; };
async function open(setup = false) {
  driver = new NativeDriver(application, fixture.environment, join(output, `driver-${++sequence}.log`)); await driver.start();
  await waitFor(() => driver.execute('const a=document.querySelector("#workspace-passphrase"),b=document.querySelector("#workspace-confirmation");return Boolean(a&&!a.disabled&&(!arguments[0]||b&&!b.disabled));', [setup]), 'native password fields');
  await driver.type('#workspace-passphrase', password); if (setup) await driver.type('#workspace-confirmation', password);
  await driver.click('.unlock-form button[type=submit]');
  await waitFor(async () => (await call('bootstrap')).storage === 'ready', 'ready activity history workspace');
  assert.equal((await call('bootstrap')).schemaVersion, 8);
}
async function close() {
  const queue = [driver.child.pid]; let owned;
  for (let count = 0; queue.length && count < 64; count++) {
    const pid = queue.shift();
    try { if (await readlink(`/proc/${pid}/exe`) === application) { owned = pid; break; } queue.push(...(await readFile(`/proc/${pid}/task/${pid}/children`, 'utf8')).trim().split(/\s+/).filter(Boolean).map(Number)); }
    catch (error) { if (error.code !== 'ENOENT') throw error; }
  }
  assert.ok(owned);
  evidence.closes.push(JSON.parse(execFileSync('/usr/bin/python3', [join(repository, 'tests/native/x11-close.py'), String(owned)], { env: fixture.environment, encoding: 'utf8', timeout: 10000 })));
  await waitFor(async () => { try { await readlink(`/proc/${owned}/exe`); return false; } catch (error) { if (error.code === 'ENOENT') return true; throw error; } }, 'genuine native close');
  driver.session = undefined; await driver.stop(); driver = undefined;
}
async function liveHashes() {
  const result = {};
  for (const path of [fixture.database, `${fixture.database}-wal`, join(dirname(fixture.database), 'database-key.v1')]) {
    try { result[path] = await hash(path); } catch (error) { if (error.code !== 'ENOENT') throw error; result[path] = null; }
  }
  return result;
}
async function oracle(label) {
  assert.equal(await hash(verifier), verifierDigest);
  const before = await liveHashes(), temporary = join(fixture.root, `history-oracle-${randomUUID()}`); await mkdir(temporary, { mode: 0o700 });
  try {
    const database = join(temporary, 'database.db'); await copyFile(fixture.database, database); await chmod(database, 0o600);
    if (before[`${fixture.database}-wal`] !== null) { await copyFile(`${fixture.database}-wal`, `${database}-wal`); await chmod(`${database}-wal`, 0o600); }
    const report = JSON.parse(execFileSync(verifier, ['database', database, join(dirname(fixture.database), 'database-key.v1'), '--profile-state'], { input: JSON.stringify({ passphrase: password }), encoding: 'utf8', env: fixture.environment, timeout: 30000, maxBuffer: 8 * 1024 * 1024 }));
    for (const field of ['verified', 'readOnly', 'inputUnchanged', 'keyEnvelopeUnchanged', 'walRecordsUnchanged']) assert.equal(report[field], true, field);
    assert.equal(report.schemaVersion, 8); assert.equal(report.plaintextDiskStaging, false); assert.equal(Object.keys(report.state.tables).length, 30);
    assert.deepEqual(await liveHashes(), before);
    evidence.oracles.push({ label, allNamedTables: 30, databaseWalKeyUnchanged: true, tables: Object.fromEntries(Object.entries(report.state.tables).map(([name, records]) => [name, { rows: records.length, sha256: createHash('sha256').update(JSON.stringify(records)).digest('hex') }])) });
    return report.state.tables;
  } finally { await rm(temporary, { recursive: true, force: true }); }
}
async function showHistory(profile) {
  await driver.navigate('usage');
  const current = (await call('list_profiles')).profiles.find(item => item.id === profile.id);
  await driver.selectVisibleOption('[aria-label="Usage profile"]', `${profile.label}${current.archived ? ' (archived)' : ''}`);
  const scope = `[data-activity-history="${profile.id}"]`, trigger = `${scope} [data-activity-history-open]`;
  if (!await driver.execute('return document.querySelector(arguments[0])?.getAttribute("aria-expanded")==="true";', [trigger])) await driver.click(trigger);
  await waitFor(() => driver.execute('return document.querySelectorAll(arguments[0]+" [data-activity-history-run]").length>0;', [scope]), 'visible activity summary history');
}
const rows = () => driver.execute('return [...document.querySelectorAll("[data-activity-history-run]")].map(node=>node.getAttribute("data-activity-history-run"));');
async function clickButton(text, scope = '[data-activity-history]') {
  const element = await driver.execute('return [...document.querySelectorAll(arguments[0]+" button")].find(node=>node.textContent.trim()===arguments[1]);', [scope, text]); assert.ok(element);
  await driver.request('POST', `/session/${driver.session}/element/${element['element-6066-11e4-a52e-4f735466cecf']}/click`, {});
}
function validateEntry(entry, profile) {
  assert.deepEqual(Object.keys(entry).sort(), ['profileId', 'bindingId', 'collectionRunId', 'observedAtUtc', 'sourceKind', 'providerVersion', 'sourceScope', 'coverage', 'summary', 'dailyBucketsState'].sort());
  assert.equal(entry.profileId, profile.id); assert.equal(entry.sourceKind, 'test_fixture'); assert.equal(entry.providerVersion, '0.163.0');
  assert.equal(entry.sourceScope, 'account_activity_with_unknown_application_scope'); assert.equal(entry.coverage, 'unknown');
  assert.deepEqual(Object.keys(entry.summary).sort(), ['lifetimeTokens', 'peakDailyTokens', 'longestRunningTurnSec', 'currentStreakDays', 'longestStreakDays'].sort());
  for (const value of Object.values(entry.summary)) assert.ok(value === null || typeof value === 'string' && /^(0|[1-9][0-9]*)$/.test(value));
}
await mkdir(output, { recursive: true, mode: 0o700 });
process.once('exit', () => { driver?.kill('SIGKILL'); stopGroup(fixture?.bus, 'SIGKILL'); });
try {
  evidence.applicationSha256 = await hash(application); evidence.seederSha256 = await hash(seeder); evidence.verifierSha256 = await hash(verifier);
  fixture = await createFixture('activity-history'); fixture.environment.HOME = join(fixture.root, 'synthetic-home'); await mkdir(fixture.environment.HOME, { mode: 0o700 });
  await open(true);
  const executable = join(fixture.root, 'synthetic-collection-provider.py'); await copyFile(join(repository, 'crates/openusage-codex/tests/fixtures/synthetic-collection-provider.py'), executable); await chmod(executable, 0o700); assert.equal(await hash(executable), providerDigest);
  await driver.navigate('settings');
  if (!await driver.execute('return document.querySelector("[data-provider-settings]").closest("details").open;')) await driver.click('[data-provider-settings]');
  if (await driver.execute('return !document.querySelector("[data-provider-manual]").closest("details").open;')) await driver.click('[data-provider-manual]');
  await driver.type('#provider-executable', executable); await driver.click('.provider-inspection-form button[type=submit]');
  await waitFor(() => driver.execute('const n=document.querySelector("#provider-executable-approved");return Boolean(n&&!n.disabled&&n.getAttribute("aria-checked")==="false");'), 'reviewed fixture without implicit consent');
  await driver.click('#provider-executable-approved'); await driver.click('.provider-approval-form button[type=submit]');
  await waitFor(() => driver.execute('return Boolean(document.querySelector(".provider-approval-result"));'), 'explicit fixture approval');
  for (let index = 0; index < 2; index++) {
    await driver.navigate('overview'); await driver.click('[data-overview-add-account]');
    let profile; await waitFor(async () => { profile = (await call('list_profiles')).profiles.find(item => !profiles.some(previous => previous.id === item.id)); return Boolean(profile); }, 'new isolated profile'); profiles.push(profile);
    await writeFile(join(dirname(fixture.database), 'profiles', profile.id, 'synthetic-collection-config.json'), JSON.stringify({ mode: 'startup_snapshot', requireLogin: true }) + '\n', { mode: 0o600 });
    await driver.navigate('overview'); await driver.click(`[data-overview-account="${profile.id}"] td:last-child button:first-child`);
    await waitFor(async () => Boolean(await call('load_profile_activity', { profileId: profile.id })), 'automatic first activity COMMIT', 45000);
    const initial = await call('load_profile_activity', { profileId: profile.id });
    assert.equal(initial.sourceKind, 'test_fixture'); assert.equal(initial.summary.lifetimeTokens, '9007199254740993');
  }
  const initial = await call('load_profile_activity', { profileId: profiles[0].id }), otherBefore = metrics(await call('load_profile_activity', { profileId: profiles[1].id }));
  const initialQuota = await call('load_profile_quota', { profileId: profiles[0].id });
  await close();
  await writeFile(join(fixture.dirs.control, 'activity-history-fixture.json'), JSON.stringify({ purpose: 'openusage-native-activity-history-v1', profileId: profiles[0].id, initialRunId: initial.collectionRunId }), { mode: 0o600, flag: 'wx' });
  evidence.seed = JSON.parse(execFileSync(seeder, [fixture.root, profiles[0].id], { input: JSON.stringify({ passphrase: password }), encoding: 'utf8', timeout: 30000, env: { PATH: '/usr/bin:/bin' } }));
  assert.equal(evidence.seed.syntheticAcceptedSnapshotsAdded, 21); assert.equal(evidence.seed.sourceKind, 'test_fixture');
  await open();
  const before = await liveHashes(), beforeTables = await oracle('before history browsing');
  const firstPage = await call('load_profile_activity_history', { profileId: profiles[0].id, beforeRunId: null });
  assert.equal(firstPage.entries.length, 20); assert.ok(firstPage.nextBeforeRunId);
  const secondPage = await call('load_profile_activity_history', { profileId: profiles[0].id, beforeRunId: firstPage.nextBeforeRunId });
  assert.equal(secondPage.entries.length, 2); assert.equal(secondPage.nextBeforeRunId, null);
  const all = [...firstPage.entries, ...secondPage.entries]; all.forEach(entry => validateEntry(entry, profiles[0]));
  assert.equal(new Set(all.map(entry => entry.collectionRunId)).size, 22); assert.equal(new Set(all.map(entry => entry.observedAtUtc)).size, 1);
  const summaryRows = new Map(beforeTables.activity_summaries.filter(row => row.profile_id === profiles[0].id).map(row => [row.id, row]));
  const orderedRuns = beforeTables.collection_runs.filter(row => row.profile_id === profiles[0].id && row.source_method === 'account/usage/read' && summaryRows.has(row.id)).sort((a, b) => a.observed_at_utc === b.observed_at_utc ? a.id < b.id ? 1 : a.id > b.id ? -1 : 0 : a.observed_at_utc < b.observed_at_utc ? 1 : -1);
  assert.deepEqual(all.map(row => row.collectionRunId), orderedRuns.map(row => row.id), 'Independent encrypted-copy records establish the tie-break order.');
  const names = { lifetimeTokens: 'lifetime_tokens', peakDailyTokens: 'peak_daily_tokens', longestRunningTurnSec: 'longest_running_turn_sec', currentStreakDays: 'current_streak_days', longestStreakDays: 'longest_streak_days' };
  for (const entry of all) {
    const raw = summaryRows.get(entry.collectionRunId); assert.equal(raw.binding_id, entry.bindingId);
    for (const [name, column] of Object.entries(names)) assert.equal(entry.summary[name], raw[column]);
  }
  assert.deepEqual(new Set(all.map(entry => entry.bindingId)), new Set([evidence.seed.originalBindingId, evidence.seed.currentBindingId]));
  assert.deepEqual(new Set(all.map(entry => entry.dailyBucketsState)), new Set(['unavailable', 'empty', 'returned']));
  for (const request of [
    { profileId: profiles[1].id, beforeRunId: firstPage.nextBeforeRunId }, { profileId: profiles[0].id, beforeRunId: initialQuota.collectionRunId },
    { profileId: profiles[0].id, beforeRunId: '99999999-9999-4999-8999-999999999999' }, { profileId: '../outside', beforeRunId: null },
    { profileId: profiles[0].id, beforeRunId: null, extra: true },
  ]) assert.equal((await driver.invoke('load_profile_activity_history', request)).ok, false);
  await showHistory(profiles[0]); assert.deepEqual(await rows(), firstPage.entries.map(entry => entry.collectionRunId));
  await clickButton('Older readings'); await waitFor(async () => (await rows()).length === 2, 'older activity page'); assert.deepEqual(await rows(), secondPage.entries.map(entry => entry.collectionRunId));
  await clickButton('Newer readings'); await waitFor(async () => (await rows()).length === 20, 'newer activity page'); assert.deepEqual(await rows(), firstPage.entries.map(entry => entry.collectionRunId));
  const detailed = firstPage.entries.find(entry => entry.summary.lifetimeTokens === '9223372036854775807'); assert.ok(detailed);
  const scope = `[data-activity-history-run="${detailed.collectionRunId}"]`;
  await driver.click(`${scope} > button`);
  for (const [metric, value] of Object.entries(detailed.summary)) assert.equal(await driver.execute('return document.querySelector(arguments[0]).textContent;', [`${scope} [data-activity-history-metric="${metric}"]`]), value ?? 'Not available');
  assert.equal(await driver.execute('return document.querySelector(arguments[0]).getAttribute("data-activity-history-daily-state");', [`${scope} [data-activity-history-daily-state]`]), detailed.dailyBucketsState);
  await clickButton('Source details', scope);
  const details = await driver.execute('return document.querySelector(arguments[0]).textContent;', [scope]);
  for (const text of [detailed.collectionRunId, detailed.bindingId, detailed.observedAtUtc, '0.163.0', 'Synthetic provider', 'Unknown']) assert.equal(details.includes(text), true);
  assert.equal(await driver.execute('return document.querySelector(arguments[0]).textContent.includes("Fresh");', [`[data-activity-history="${profiles[0].id}"]`]), false);
  await driver.screenshot(join(output, 'activity-summary-history.png')); evidence.screenshots.push('activity-summary-history.png');
  const previous = firstPage.entries.find(entry => entry.bindingId === evidence.seed.originalBindingId && entry.summary.lifetimeTokens === '0'); assert.ok(previous);
  const previousScope = `[data-activity-history-run="${previous.collectionRunId}"]`;
  await driver.click(`${previousScope} > button`); await clickButton('Source details', previousScope);
  assert.equal(await driver.execute('return document.querySelector(arguments[0]).textContent;', [`${previousScope} [data-activity-history-metric="lifetimeTokens"]`]), '0');
  assert.equal(await driver.execute('return document.querySelector(arguments[0]).textContent;', [`${previousScope} [data-activity-history-metric="peakDailyTokens"]`]), 'Not available');
  assert.equal(await driver.execute('return document.querySelector(arguments[0]).textContent.includes(arguments[1]);', [previousScope, previous.bindingId]), true);
  await showHistory(profiles[1]); assert.deepEqual(await rows(), [otherBefore.collectionRunId]); assert.deepEqual(metrics(await call('load_profile_activity', { profileId: profiles[1].id })), otherBefore);
  assert.deepEqual(await liveHashes(), before); assert.deepEqual(await oracle('paging, details and refused cursors preserve all records'), beforeTables);
  await driver.openProfileOptions(profiles[0].id); await driver.click(`[data-profile-id="${profiles[0].id}"] [data-profile-action="archive"]`);
  await waitFor(async () => (await call('list_profiles')).profiles.find(profile => profile.id === profiles[0].id)?.archived, 'archived profile retains activity history');
  await close(); await open();
  const archivedBefore = await liveHashes(), archivedTables = await oracle('archived restart before local history reads');
  assert.deepEqual(await driver.invoke('check_profile_connection', { profileId: profiles[1].id }), { ok: false, error: 'provider_executable_not_approved' });
  await showHistory(profiles[0]); assert.deepEqual(await rows(), firstPage.entries.map(entry => entry.collectionRunId));
  assert.deepEqual(await call('load_profile_activity_history', { profileId: profiles[0].id, beforeRunId: null }), firstPage);
  assert.deepEqual(await liveHashes(), archivedBefore); assert.deepEqual(await oracle('archived history after genuine restart stays read-only'), archivedTables);
  const sessions = await Promise.all(profiles.map(async profile => (await readFile(join(dirname(fixture.database), 'profiles', profile.id, 'synthetic-collection-observations.jsonl'), 'utf8')).trim().split('\n').map(JSON.parse).filter(row => row.kind === 'start' && row.arguments[0] === 'app-server').length));
  assert.deepEqual(sessions, [2, 2], 'History and restart must not launch another provider session.');
  evidence.paging = { first: 20, older: 2, unique: 22, tiedObservationOrdering: true, priorAssociationPreserved: true, profileCursorIsolation: true, nullZeroAndExactStrings: true, archivedAfterGenuineRestart: true, providerSessions: sessions };
  assert.equal(await hash(application), evidence.applicationSha256); assert.equal(await hash(seeder), evidence.seederSha256); assert.equal(await hash(executable), providerDigest);
  await close(); evidence.result = 'passed';
} catch (error) { evidence.error = error.stack ?? String(error); process.exitCode = 1; }
finally {
  try { await driver?.stop(); } finally { stopGroup(fixture?.bus, 'SIGKILL'); if (fixture) await rm(fixture.root, { recursive: true, force: true }); await writeFile(join(output, 'result.json'), JSON.stringify(evidence, null, 2) + '\n', { mode: 0o600 }); }
}
