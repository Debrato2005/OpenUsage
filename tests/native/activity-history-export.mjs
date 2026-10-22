import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { chmod, copyFile, mkdir, readFile, readdir, readlink, rm, stat, writeFile } from 'node:fs/promises';
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
const evidence = { requirement: 'NATIVE-ACTIVITY-HISTORY-EXPORT-001', result: 'failed', scope: 'Actual Linux Tauri/WebKitGTK CSV/JSON export from a private SQLCipher4/WAL schema8 workspace. Two GUI synthetic sign-ins save activity; a strictly marked closed-app helper adds21synthetic summaries and retained daily observations through production storage APIs. Independent30table encrypted-copy records establish exact all-page/prior-association values, nulls, zeroes, large integers, dates and provenance. GUI plaintext consent, aliases, explicit local identifiers with email always omitted from activity history, refusal cleanup, routing isolation, archived restart and unchanged database/WAL/key are checked. No live-provider, combined-total or other-platform acceptance.', screenshots: [], closes: [], oracles: [], exports: [] };
let fixture, driver, sequence = 0;
const profiles = [];
const call = async (command, args) => { const response = await driver.invoke(command, args); assert.equal(response.ok, true, `${command}: ${JSON.stringify(response)}`); return response.value; };
const metrics = value => { const copy = structuredClone(value); if (copy) delete copy.freshness; return copy; };
async function open(setup = false) {
  driver = new NativeDriver(application, fixture.environment, join(output, `driver-${++sequence}.log`)); await driver.start();
  await waitFor(() => driver.execute('const a=document.querySelector("#workspace-passphrase"),b=document.querySelector("#workspace-confirmation");return Boolean(a&&!a.disabled&&(!arguments[0]||b&&!b.disabled));', [setup]), 'native password fields');
  if (setup) {
    const names = await exportNames();
    assert.deepEqual(await driver.invoke('export_activity_history', { profileId: '11111111-1111-4111-8111-111111111111', format: 'json', includeIdentifiers: false, confirmed: true }), { ok: false, error: 'workspace_locked' });
    assert.deepEqual(await exportNames(), names); evidence.lockedRefusedWithoutFiles = true;
  }
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
const columns = ['profile', 'profile_id', 'profile_label', 'email', 'record_kind', 'metric', 'value', 'unit', 'provider_date', 'observed_at_utc', 'source_method', 'source_kind', 'provider_version', 'source_scope', 'raw_scope', 'identity_confidence', 'coverage', 'status', 'inclusion_rule', 'limit_id', 'window_role', 'collection_run_id', 'binding_id'];
const exportDirectory = () => join(fixture.dirs.downloads, 'OpenUsage');
async function exportNames() { try { return (await readdir(exportDirectory())).sort(); } catch (error) { if (error.code !== 'ENOENT') throw error; return []; } }
const chronological = (a, b) => a.observed_at_utc === b.observed_at_utc ? a.id < b.id ? -1 : a.id > b.id ? 1 : 0 : a.observed_at_utc < b.observed_at_utc ? -1 : 1;
function expectedRecords(tables, profile, identifiers) {
  const summaries = new Map(tables.activity_summaries.filter(row => row.profile_id === profile.id).map(row => [row.collection_run_id, row]));
  const runs = tables.collection_runs.filter(row => row.profile_id === profile.id && row.source_method === 'account/usage/read' && row.outcome === 'accepted' && summaries.has(row.id)).sort(chronological);
  const bindings = new Map(), expected = [];
  for (const [index, run] of runs.entries()) {
    if (!bindings.has(run.binding_id)) bindings.set(run.binding_id, `association-${bindings.size + 1}`);
    const base = { profile: 'profile-1', profile_id: identifiers ? profile.id : null, profile_label: identifiers ? profile.label : null, email: null,
      observed_at_utc: run.observed_at_utc, source_method: run.source_method, source_kind: run.source_kind, provider_version: run.provider_version, source_scope: run.source_scope,
      raw_scope: run.raw_scope, identity_confidence: run.identity_confidence, coverage: run.coverage, inclusion_rule: 'separate_profile_no_aggregation', limit_id: null, window_role: null,
      collection_run_id: identifiers ? run.id : `reading-${index + 1}`, binding_id: identifiers ? run.binding_id : bindings.get(run.binding_id) };
    const row = (kind, metric, value, unit, date = null) => ({ ...base, record_kind: kind, metric, value, unit, provider_date: date, status: value === null ? 'unavailable' : 'available' });
    const summary = summaries.get(run.id);
    for (const [metric, field, unit] of [['lifetime_tokens', 'lifetime_tokens', 'tokens'], ['peak_daily_tokens', 'peak_daily_tokens', 'tokens'], ['longest_running_turn_sec', 'longest_running_turn_sec', 'seconds'], ['current_streak_days', 'current_streak_days', 'days'], ['longest_streak_days', 'longest_streak_days', 'days'], ['daily_buckets_state', 'daily_state', 'text']]) expected.push(row('activity_summary', metric, summary[field], unit));
    for (const day of tables.activity_day_history.filter(day => day.profile_id === profile.id && day.collection_run_id === run.id)) {
      assert.equal(day.binding_id, run.binding_id);
      expected.push(row('activity_daily_history', 'accepted_tokens', day.accepted_tokens, 'tokens', day.provider_date));
      expected.push(row('activity_daily_history', 'prior_tokens', day.prior_tokens, 'tokens', day.provider_date));
    }
  }
  return { rows: expected, runCount: runs.length, bindingCount: bindings.size, retainedDayCount: tables.activity_day_history.filter(row => row.profile_id === profile.id && summaries.has(row.collection_run_id)).length };
}
const signatures = records => records.map(row => JSON.stringify(columns.map(name => row[name]))).sort();
function assertExpected(records, expected) {
  assert.equal(records.length, expected.rows.length);
  assert.deepEqual(signatures(records), signatures(expected.rows), 'Independent encrypted-copy summaries and retained day observations establish every exported field.');
  assert.equal(new Set(records.map(row => row.collection_run_id)).size, expected.runCount);
  assert.equal(new Set(records.map(row => row.binding_id)).size, expected.bindingCount);
}
async function readExport(receipt, kind = 'activity') {
  assert.equal(receipt.directory, exportDirectory()); assert.equal(receipt.profileCount, 1);
  assert.match(receipt.fileName, new RegExp(`^openusage-readings-[A-Za-z0-9_-]{16,64}\\.${receipt.format}$`));
  const path = join(receipt.directory, receipt.fileName), metadata = await stat(path);
  assert.equal(metadata.mode & 0o777, 0o600); assert.equal(metadata.nlink, 1); assert.equal((await stat(exportDirectory())).mode & 0o777, 0o700);
  assert.equal((await exportNames()).some(name => name.endsWith('.pending')), false);
  let records;
  if (receipt.format === 'json') {
    const value = JSON.parse(await readFile(path, 'utf8'));
    assert.deepEqual(Object.keys(value).sort(), ['schemaVersion', 'exportedAt', 'selection', 'records'].sort());
    assert.equal(value.schemaVersion, 1); assert.equal(value.exportedAt, receipt.exportedAt);
    assert.deepEqual(value.selection, { profileCount: 1, identifiersIncluded: receipt.identifiersIncluded }); records = value.records;
  } else records = JSON.parse(execFileSync('/usr/bin/python3', ['-c', 'import csv,json,sys; f=open(sys.argv[1],newline="",encoding="utf-8"); print(json.dumps(list(csv.DictReader(f))))', path], { encoding: 'utf8', timeout: 10000, env: { PATH: '/usr/bin:/bin' } }));
  assert.equal(receipt.rowCount, records.length); assert.ok(records.length > 0 && records.length <= 50000);
  for (const row of records) {
    assert.deepEqual(Object.keys(row).sort(), (kind === 'latest' ? columns.slice(0, 21) : columns).slice().sort());
    assert.equal(row.source_kind, 'test_fixture'); assert.equal(row.identity_confidence, 'provisional'); assert.equal(row.coverage, 'unknown'); assert.equal(row.inclusion_rule, 'separate_profile_no_aggregation');
    for (const value of Object.values(row)) assert.ok(value === null || typeof value === 'string');
    if (kind !== 'latest') assert.equal(row.email || null, null);
    if (kind === 'activity') { assert.equal(row.source_method, 'account/usage/read'); assert.equal(row.source_scope, 'account_activity_with_unknown_application_scope'); }
  }
  evidence.exports.push({ kind, format: receipt.format, rowCount: records.length, localIdentifiers: receipt.identifiersIncluded, privateFile: true, sha256: await hash(path) });
  return records;
}
async function exported(profile, identifiers, command = 'export_activity_history') {
  const args = { format: 'json', includeIdentifiers: identifiers, confirmed: true, ...(command === 'export_saved_readings' ? { profileIds: [profile.id] } : { profileId: profile.id }) };
  const receipt = await call(command, args); assert.equal(receipt.format, 'json'); assert.equal(receipt.identifiersIncluded, identifiers);
  return readExport(receipt, command === 'export_saved_readings' ? 'latest' : command === 'export_quota_history' ? 'quota' : 'activity');
}
async function guiExport(profile, format, identifiers = false) {
  await showHistory(profile);
  const scope = `[data-activity-history="${profile.id}"]`, consent = `${scope} #activity-history-export-consent`;
  if (!await driver.execute('return document.querySelector(arguments[0])?.getAttribute("aria-expanded")==="true";', [`${scope} [data-export-toggle]`])) await driver.click(`${scope} [data-export-toggle]`);
  await waitFor(() => driver.execute('const n=document.querySelector(arguments[0]);return Boolean(n&&!n.disabled&&n.getClientRects().length);', [consent]), 'activity export plaintext consent');
  assert.equal(await driver.execute('return document.querySelector(arguments[0]).disabled;', [`${scope} [data-export-save]`]), true);
  assert.equal(await driver.execute('return document.querySelector(arguments[0]).getAttribute("aria-checked");', [consent]), 'false');
  assert.equal(await driver.execute('return document.querySelector(arguments[0]).getAttribute("aria-checked");', [`${scope} #activity-history-export-identifiers`]), 'false');
  if (format === 'json') await driver.selectVisibleOption(`${scope} #activity-history-export-format`, 'JSON');
  else assert.equal(await driver.execute('return document.querySelector(arguments[0]).textContent.trim();', [`${scope} #activity-history-export-format`]), 'CSV');
  if (identifiers) await driver.click(`${scope} #activity-history-export-identifiers`);
  await driver.click(consent); const before = await exportNames();
  await driver.click(`${scope} [data-export-save]`);
  await waitFor(() => driver.execute('return document.querySelector(arguments[0])?.textContent.includes("Activity history exported.");', [`${scope} [data-export-result]`]), 'activity history export receipt');
  const added = (await exportNames()).filter(name => !before.includes(name)); assert.equal(added.length, 1);
  assert.equal(await driver.execute('return document.querySelector(arguments[0]).getAttribute("aria-checked");', [consent]), 'false');
  assert.equal(await driver.execute('return document.querySelector(arguments[0]).disabled;', [`${scope} [data-export-save]`]), true);
  const payload = format === 'json' ? JSON.parse(await readFile(join(exportDirectory(), added[0]), 'utf8')) : null;
  const summary = await driver.execute('return document.querySelector(arguments[0]).children[2].textContent;', [`${scope} [data-export-result]`]);
  const count = summary.match(/^(CSV|JSON) · 1 profiles · ([0-9]+) rows$/);
  assert.ok(count, `Unexpected native export summary: ${JSON.stringify(summary)}`);
  assert.equal(count[1], format.toUpperCase());
  const receipt = { directory: exportDirectory(), fileName: added[0], format, profileCount: 1, identifiersIncluded: identifiers,
    exportedAt: payload?.exportedAt ?? await driver.execute('return document.querySelector(arguments[0]).dateTime;', [`${scope} [data-export-result] time`]),
    rowCount: Number(count[2]) };
  return readExport(receipt);
}
await mkdir(output, { recursive: true, mode: 0o700 });
process.once('exit', () => { driver?.kill('SIGKILL'); stopGroup(fixture?.bus, 'SIGKILL'); });
try {
  evidence.applicationSha256 = await hash(application); evidence.seederSha256 = await hash(seeder); evidence.verifierSha256 = await hash(verifier);
  fixture = await createFixture('activity-history-export'); fixture.environment.HOME = join(fixture.root, 'synthetic-home'); await mkdir(fixture.environment.HOME, { mode: 0o700 });
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
  const before = await liveHashes(), tables = await oracle('all retained source observations before export');
  const expectedDefault = expectedRecords(tables, profiles[0], false), expectedIdentified = expectedRecords(tables, profiles[0], true);
  assert.equal(expectedDefault.runCount, 22); assert.equal(expectedDefault.bindingCount, 2); assert.ok(expectedDefault.retainedDayCount > 0);
  const visible = await call('load_profile_activity_history', { profileId: profiles[0].id, beforeRunId: null });
  assert.equal(visible.entries.length, 20); assert.ok(visible.nextBeforeRunId);
  const namesBefore = await exportNames(), selection = { profileId: profiles[0].id, format: 'csv', includeIdentifiers: false, confirmed: true };
  for (const request of [
    { ...selection, confirmed: false }, { ...selection, confirmed: 'true' }, { ...selection, includeIdentifiers: 'true' },
    { ...selection, format: 'xml' }, { ...selection, profileId: '../outside' },
    { ...selection, profileId: '99999999-9999-4999-8999-999999999999' },
    { ...selection, beforeRunId: visible.nextBeforeRunId }, { ...selection, path: '/external' },
  ]) assert.equal((await driver.invoke('export_activity_history', request)).ok, false);
  assert.deepEqual(await exportNames(), namesBefore);
  const csv = await guiExport(profiles[0], 'csv');
  const redacted = await exported(profiles[0], false); assertExpected(redacted, expectedDefault);
  assert.deepEqual(csv, redacted.map(row => Object.fromEntries(Object.entries(row).map(([name, value]) => [name, value ?? '']))), 'CSV and JSON preserve identical rows and exact nullable values.');
  const text = JSON.stringify(redacted);
  for (const profile of profiles) { assert.equal(text.includes(profile.id), false); assert.equal(text.includes(profile.label), false); }
  for (const run of tables.collection_runs) assert.equal(text.includes(run.id), false);
  for (const binding of tables.profile_identity_events) assert.equal(text.includes(binding.id), false);
  assert.equal(text.includes(fixture.root), false);
  const identified = await exported(profiles[0], true); assertExpected(identified, expectedIdentified);
  for (const email of tables.profile_identity_events.map(row => row.returned_email)) {
    assert.equal(text.includes(email), false); assert.equal(JSON.stringify(identified).includes(email), false);
  }
  assert.ok(identified.some(row => row.record_kind === 'activity_summary' && row.metric === 'lifetime_tokens' && row.value === '9223372036854775807'));
  assert.ok(identified.some(row => row.metric === 'peak_daily_tokens' && row.value === '9007199254740993'));
  assert.ok(identified.some(row => row.value === '0' && row.status === 'available'));
  assert.ok(identified.some(row => row.value === null && row.status === 'unavailable'));
  assert.equal(identified.filter(row => row.record_kind === 'activity_summary').length, 22 * 6);
  assert.equal(identified.filter(row => row.record_kind === 'activity_daily_history').length, expectedDefault.retainedDayCount * 2);
  const other = await exported(profiles[1], true); assertExpected(other, expectedRecords(tables, profiles[1], true));
  assert.equal(JSON.stringify(other).includes(profiles[0].id), false);
  assert.deepEqual(metrics(await call('load_profile_activity', { profileId: profiles[1].id })), otherBefore);
  // These two calls check the shared native file writer routes without repeating
  // the existing current-reading and quota-history export qualification.
  const latest = await exported(profiles[1], true, 'export_saved_readings');
  assert.ok(latest.some(row => row.record_kind === 'activity_summary')); assert.ok(latest.some(row => row.record_kind === 'quota_summary'));
  assert.equal(latest.every(row => row.profile_id === profiles[1].id && ['account/usage/read', 'account/rateLimits/read'].includes(row.source_method)), true);
  assert.equal(JSON.stringify(latest).includes(profiles[0].id), false);
  const quota = await exported(profiles[0], true, 'export_quota_history');
  assert.equal(quota.every(row => row.profile_id === profiles[0].id && row.record_kind.startsWith('quota_') && row.source_method === 'account/rateLimits/read' && row.collection_run_id === initialQuota.collectionRunId), true);
  assert.equal(JSON.stringify(quota).includes(profiles[1].id), false);
  assert.deepEqual(await liveHashes(), before); assert.deepEqual(await oracle('all three export routes and refusals preserve source records'), tables);
  await driver.openProfileOptions(profiles[0].id); await driver.click(`[data-profile-id="${profiles[0].id}"] [data-profile-action="archive"]`);
  await waitFor(async () => (await call('list_profiles')).profiles.find(profile => profile.id === profiles[0].id)?.archived, 'archived profile retains activity export');
  await close(); await open();
  const archivedBefore = await liveHashes(), archivedTables = await oracle('archived genuine restart before export');
  assert.deepEqual(await driver.invoke('check_profile_connection', { profileId: profiles[1].id }), { ok: false, error: 'provider_executable_not_approved' });
  const archived = await guiExport(profiles[0], 'json', true); assertExpected(archived, expectedIdentified);
  assert.deepEqual(await liveHashes(), archivedBefore); assert.deepEqual(await oracle('archived GUI JSON export preserves all source fields'), archivedTables);
  await driver.screenshot(join(output, 'activity-history-export.png')); evidence.screenshots.push('activity-history-export.png');
  const sessions = await Promise.all(profiles.map(async profile => (await readFile(join(dirname(fixture.database), 'profiles', profile.id, 'synthetic-collection-observations.jsonl'), 'utf8')).trim().split('\n').map(JSON.parse).filter(row => row.kind === 'start' && row.arguments[0] === 'app-server').length));
  assert.deepEqual(sessions, [2, 2]);
  evidence.acceptance = { summarySnapshots: expectedDefault.runCount, priorAndCurrentAssociations: expectedDefault.bindingCount, retainedDayObservations: expectedDefault.retainedDayCount,
    exactExpectedRows: expectedDefault.rows.length, allPagesIncludedBeyondVisible20: true, csvJsonParity: true, explicitPlaintextConsent: true, consentClearedAfterSave: true,
    defaultNamesAndLocalIdsOmitted: true, optedInLocalReferencesExact: true, historyEmailAlwaysOmitted: true, nullZeroLargeIntegerAndDatePreserved: true,
    invalidRequestsSaveNoFiles: true, latestAndQuotaSharedWriterRoutesRemainSeparate: true, archivedAfterGenuineRestart: true, providerSessions: sessions };
  assert.equal(await hash(application), evidence.applicationSha256); assert.equal(await hash(seeder), evidence.seederSha256); assert.equal(await hash(executable), providerDigest);
  await close(); evidence.result = 'passed';
} catch (error) { evidence.error = error.stack ?? String(error); process.exitCode = 1; }
finally {
  try { await driver?.stop(); } finally { stopGroup(fixture?.bus, 'SIGKILL'); if (fixture) await rm(fixture.root, { recursive: true, force: true }); await writeFile(join(output, 'result.json'), JSON.stringify(evidence, null, 2) + '\n', { mode: 0o600 }); }
}
