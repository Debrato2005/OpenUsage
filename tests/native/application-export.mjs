import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { chmod, copyFile, lstat, mkdir, readFile, readdir, readlink, rm, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { CODEX_SENTINEL, createFixture, stopGroup } from './private-fixture.mjs';
import { NativeDriver, waitFor } from './webdriver.mjs';

for (const name of ['OPENUSAGE_NATIVE_BINARY', 'OPENUSAGE_NATIVE_ACTIVITY_HISTORY_FIXTURE', 'OPENUSAGE_NATIVE_EVIDENCE', 'OPENUSAGE_LOGIN_VERIFIER']) assert.ok(process.env[name], name);
const repository = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const application = resolve(process.env.OPENUSAGE_NATIVE_BINARY), seeder = resolve(process.env.OPENUSAGE_NATIVE_ACTIVITY_HISTORY_FIXTURE), output = resolve(process.env.OPENUSAGE_NATIVE_EVIDENCE), verifier = resolve(process.env.OPENUSAGE_LOGIN_VERIFIER);
const password = 'OpenUsage synthetic activity history workspace 2026';
const providerDigest = '53a853b737ceca311649e9f1d9ebd1341154a5f782efd8e915f7f834c819a6dc';
const verifierDigest = 'a2e4a64707081a9943f3c04f8237c2b16bea673e974e96393d6a8a7d9357f975';
const secretSentinel = 'Synthetic provider-only credential sentinel; never export this file.';
const hash = async path => createHash('sha256').update(await readFile(path)).digest('hex');
const evidence = { requirement: 'NATIVE-APPLICATION-DATA-EXPORT-001', result: 'failed', scope: 'Actual Linux Tauri full JSON application-record export from a private SQLCipher4/WAL schema8 workspace. Two explicit synthetic sign-ins and automatic readings,21strictly marked additional history snapshots, archived profile and genuine restart. An independent read-only encrypted-copy oracle establishes all30table rowsets and every populated field with exact decimal integers. Empty-table columns receive structural checks; storage tests qualify the trusted schema. Explicit identifiable plaintext consent, private files, no credential-home export, locked/invalid refusal and source preservation are checked. No real-provider or other-platform acceptance.', exports: [], oracles: [], closes: [], screenshots: [] };
let fixture, driver, sequence = 0;
const profiles = [];
const call = async (command, args) => { const response = await driver.invoke(command, args); assert.equal(response.ok, true, `${command}: ${JSON.stringify(response)}`); return response.value; };
const exportDirectory = () => join(fixture.dirs.downloads, 'OpenUsage');
async function exportNames() { try { return (await readdir(exportDirectory())).sort(); } catch (error) { if (error.code !== 'ENOENT') throw error; return []; } }
async function key(value) {
  await driver.request('POST', `/session/${driver.session}/actions`, { actions: [{ type: 'key', id: 'application-export-keyboard', actions: [{ type: 'keyDown', value }, { type: 'keyUp', value }] }] });
  await driver.request('DELETE', `/session/${driver.session}/actions`);
}
async function activate(selector, value = '\uE007') {
  await waitFor(() => driver.execute('const n=document.querySelector(arguments[0]);return Boolean(n&&!n.disabled&&n.getClientRects().length);', [selector]), `available ${selector}`);
  assert.equal(await driver.execute('const n=document.querySelector(arguments[0]);n.scrollIntoView({block:"nearest",inline:"nearest"});n.focus({preventScroll:true});return document.activeElement===n;', [selector]), true);
  await key(value);
}
async function navigate(view) {
  assert.ok(['overview', 'usage', 'settings'].includes(view));
  if (await driver.execute('return Boolean(document.querySelector("[data-account-panel]")?.open);')) {
    await activate('[data-account-panel-close]'); await waitFor(() => driver.execute('return !document.querySelector("[data-account-panel]")?.open;'), 'account dialog closed');
  }
  await activate(`[data-workspace-nav="${view}"]`);
  await waitFor(() => driver.execute('const link=document.querySelector(`[data-workspace-nav="${arguments[0]}"]`),panel=document.querySelector(`[data-workspace-view="${arguments[0]}"]`);return link?.getAttribute("aria-current")==="page"&&Boolean(panel&&!panel.hidden&&panel.getClientRects().length);', [view]), `native ${view} view`);
}
async function open(setup = false) {
  driver = new NativeDriver(application, fixture.environment, join(output, `driver-${++sequence}.log`)); await driver.start();
  await waitFor(() => driver.execute('const a=document.querySelector("#workspace-passphrase"),b=document.querySelector("#workspace-confirmation");return Boolean(a&&!a.disabled&&(!arguments[0]||b&&!b.disabled));', [setup]), 'native password fields');
  const before = await exportNames();
  assert.deepEqual(await driver.invoke('export_application_data', { confirmed: true }), { ok: false, error: 'workspace_locked' });
  assert.deepEqual(await exportNames(), before);
  await driver.type('#workspace-passphrase', password); if (setup) await driver.type('#workspace-confirmation', password);
  assert.equal(await driver.execute('return document.activeElement.value.length;'), password.length); await key('\uE007');
  await waitFor(async () => (await call('bootstrap')).storage === 'ready', 'ready application export workspace');
  assert.equal((await call('bootstrap')).schemaVersion, 8);
}
async function close() {
  const queue = [driver.child.pid]; let owned;
  for (let count = 0; queue.length && count < 64; count++) {
    const pid = queue.shift();
    try { if (await readlink(`/proc/${pid}/exe`) === application) { owned = pid; break; } queue.push(...(await readFile(`/proc/${pid}/task/${pid}/children`, 'utf8')).trim().split(/\s+/).filter(Boolean).map(Number)); }
    catch (error) { if (error.code !== 'ENOENT') throw error; }
  }
  assert.ok(owned, 'The native app must be an owned process.');
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
  const before = await liveHashes(), temporary = join(fixture.root, `application-oracle-${randomUUID()}`); await mkdir(temporary, { mode: 0o700 });
  try {
    const database = join(temporary, 'database.db'); await copyFile(fixture.database, database); await chmod(database, 0o600);
    if (before[`${fixture.database}-wal`] !== null) { await copyFile(`${fixture.database}-wal`, `${database}-wal`); await chmod(`${database}-wal`, 0o600); }
    const raw = execFileSync(verifier, ['database', database, join(dirname(fixture.database), 'database-key.v1'), '--profile-state'], { input: JSON.stringify({ passphrase: password }), encoding: 'utf8', env: fixture.environment, timeout: 30000, maxBuffer: 8 * 1024 * 1024 });
    // Preserve every SQLite INTEGER token before JavaScript could round an i64.
    const report = JSON.parse(execFileSync('/usr/bin/python3', ['-c', 'import json,sys; print(json.dumps(json.load(sys.stdin,parse_int=str),separators=(",",":")))'], { input: raw, encoding: 'utf8', timeout: 10000, maxBuffer: 8 * 1024 * 1024, env: { PATH: '/usr/bin:/bin' } }));
    for (const field of ['verified', 'readOnly', 'inputUnchanged', 'keyEnvelopeUnchanged', 'walRecordsUnchanged']) assert.equal(report[field], true);
    assert.equal(report.schemaVersion, '8'); assert.equal(report.plaintextDiskStaging, false); assert.equal(Object.keys(report.state.tables).length, 30); assert.deepEqual(await liveHashes(), before);
    evidence.oracles.push({ label, tables: Object.fromEntries(Object.entries(report.state.tables).map(([name, rows]) => [name, { rows: rows.length, sha256: createHash('sha256').update(JSON.stringify(rows)).digest('hex') }])) });
    return report.state.tables;
  } finally { await rm(temporary, { recursive: true, force: true }); }
}
function tableRows(payload) {
  const tables = {};
  assert.equal(payload.tables.length, 30);
  for (const table of payload.tables) {
    assert.deepEqual(Object.keys(table).sort(), ['columns', 'name', 'rows']); assert.equal(typeof table.name, 'string'); assert.equal(Object.hasOwn(tables, table.name), false);
    assert.ok(table.columns.length > 0); const columns = table.columns.map(column => { assert.deepEqual(Object.keys(column).sort(), ['name', 'type']); assert.equal(typeof column.name, 'string'); assert.ok(['TEXT', 'INTEGER'].includes(column.type)); return column.name; });
    assert.equal(new Set(columns).size, columns.length);
    tables[table.name] = table.rows.map(row => { assert.equal(row.length, columns.length); row.forEach((value, index) => { assert.ok(value === null || typeof value === 'string'); if (value !== null && table.columns[index].type === 'INTEGER') assert.match(value, /^-?(0|[1-9][0-9]*)$/); }); return Object.fromEntries(columns.map((name, index) => [name, row[index]])); });
  }
  return tables;
}
const rowSignatures = rows => rows.map(row => JSON.stringify(Object.keys(row).sort().map(name => [name, row[name]]))).sort();
function matchesOracle(actual, expected) {
  assert.deepEqual(Object.keys(actual).sort(), Object.keys(expected).sort());
  for (const [name, rows] of Object.entries(expected)) assert.deepEqual(rowSignatures(actual[name]), rowSignatures(rows), `Every ${name} field must match the independent encrypted-copy oracle.`);
}
async function readExport(fileName, expected, receipt) {
  assert.equal(fileName, fileName.split('/').at(-1)); assert.match(fileName, /\.json$/);
  const path = join(exportDirectory(), fileName), metadata = await lstat(path); assert.equal(metadata.isSymbolicLink(), false); assert.equal(metadata.mode & 0o777, 0o600); assert.equal(metadata.nlink, 1); assert.equal((await lstat(exportDirectory())).mode & 0o777, 0o700);
  const text = await readFile(path, 'utf8'), payload = JSON.parse(text);
  assert.deepEqual(Object.keys(payload).sort(), ['format', 'formatVersion', 'schemaVersion', 'exportedAt', 'integerEncoding', 'tables'].sort());
  assert.equal(payload.format, 'openusage.application-records'); assert.equal(payload.formatVersion, 1); assert.equal(payload.schemaVersion, 8); assert.equal(payload.integerEncoding, 'decimal-string'); assert.ok(Number.isFinite(Date.parse(payload.exportedAt)));
  const tables = tableRows(payload); matchesOracle(tables, expected);
  const recordCount = payload.tables.reduce((sum, table) => sum + BigInt(table.rows.length), 0n).toString();
  if (receipt) {
    assert.deepEqual(Object.keys(receipt).sort(), ['fileName', 'directory', 'format', 'schemaVersion', 'tableCount', 'recordCount', 'byteCount', 'exportedAt'].sort());
    assert.equal(receipt.fileName, fileName); assert.equal(receipt.directory, exportDirectory()); assert.equal(receipt.format, 'json'); assert.equal(receipt.schemaVersion, 8); assert.equal(receipt.tableCount, 30); assert.equal(receipt.recordCount, recordCount); assert.equal(receipt.byteCount, String(Buffer.byteLength(text))); assert.equal(receipt.exportedAt, payload.exportedAt);
  }
  for (const forbidden of [password, secretSentinel, fixture.root, 'database-key.v1', CODEX_SENTINEL.trim()]) assert.equal(text.includes(forbidden), false, 'Provider files, keys and machine paths must stay outside the export.');
  assert.equal((await exportNames()).some(name => name.endsWith('.pending')), false);
  evidence.exports.push({ fileName, tableCount: 30, recordCount, byteCount: String(Buffer.byteLength(text)), sha256: await hash(path), privatePlaintextFile: true });
  return tables;
}
async function guiExport(expected, capture = false) {
  await navigate('settings');
  if (await driver.execute('return document.querySelector("[data-application-export-toggle]")?.getAttribute("aria-expanded")!=="true";')) await activate('[data-application-export-toggle]');
  await waitFor(() => driver.execute('return Boolean(document.querySelector("[data-application-export-consent]")?.getClientRects().length);'), 'full data export consent');
  assert.equal(await driver.execute('return document.querySelector("[data-application-export-save]").disabled;'), true);
  assert.equal(await driver.execute('return document.querySelector("[data-application-export-consent]").getAttribute("aria-checked");'), 'false');
  const before = await exportNames(); await activate('[data-application-export-consent]', ' '); await activate('[data-application-export-save]');
  await waitFor(() => driver.execute('return document.querySelector("[data-application-export-result]")?.textContent.includes("Application data exported. This file is not encrypted.");'), 'full application export receipt');
  const added = (await exportNames()).filter(name => !before.includes(name)); assert.equal(added.length, 1);
  const result = await readExport(added[0], expected);
  const receiptText = await driver.execute('return document.querySelector("[data-application-export-result]").textContent;'); assert.equal(receiptText.includes(added[0]), true);
  assert.equal(await driver.execute('return document.querySelector("[data-application-export-consent]").getAttribute("aria-checked");'), 'false'); assert.equal(await driver.execute('return document.querySelector("[data-application-export-save]").disabled;'), true);
  if (capture) { await driver.execute('document.querySelector("[data-application-export-result]").scrollIntoView({block:"center"});return true;'); await driver.screenshot(join(output, 'application-export-receipt.png')); evidence.screenshots.push('application-export-receipt.png'); }
  return result;
}
async function sessions() { return Promise.all(profiles.map(async profile => (await readFile(join(dirname(fixture.database), 'profiles', profile.id, 'synthetic-collection-observations.jsonl'), 'utf8')).trim().split('\n').map(JSON.parse).filter(row => row.kind === 'start' && row.arguments[0] === 'app-server').length)); }
async function archive(profile) {
  await navigate('settings'); await activate('[data-account-manager-open]');
  const scope = `[data-profile-id="${profile.id}"]`;
  for (const action of ['readings', 'options']) {
    const selector = `${scope} [data-profile-action="${action}"]`;
    await waitFor(() => driver.execute('const n=document.querySelector(arguments[0]);return Boolean(n&&!n.disabled&&n.getClientRects().length);', [selector]), `profile ${action} action`);
    if (await driver.execute('return document.querySelector(arguments[0]).getAttribute("aria-expanded")==="false";', [selector])) await activate(selector);
    await waitFor(() => driver.execute('return document.querySelector(arguments[0])?.getAttribute("aria-expanded")==="true";', [selector]), `profile ${action} expanded`);
  }
  await activate(`${scope} [data-profile-action="archive"]`);
  await waitFor(async () => (await call('list_profiles')).profiles.find(row => row.id === profile.id)?.archived, 'archived synthetic profile');
  await navigate('settings');
}

await mkdir(output, { recursive: true, mode: 0o700 });
process.once('exit', () => { driver?.kill('SIGKILL'); stopGroup(fixture?.bus, 'SIGKILL'); });
try {
  evidence.applicationSha256 = await hash(application); evidence.seederSha256 = await hash(seeder); evidence.verifierSha256 = await hash(verifier);
  fixture = await createFixture('application-export'); fixture.environment.HOME = join(fixture.root, 'synthetic-home'); await mkdir(fixture.environment.HOME, { mode: 0o700 });
  await open(true);
  const emptyBefore = await liveHashes(), empty = await oracle('empty application workspace'); await guiExport(empty);
  assert.equal(empty.profiles.length, 0); assert.deepEqual(await liveHashes(), emptyBefore);
  const executable = join(fixture.root, 'synthetic-collection-provider.py'); await copyFile(join(repository, 'crates/openusage-codex/tests/fixtures/synthetic-collection-provider.py'), executable); await chmod(executable, 0o700); assert.equal(await hash(executable), providerDigest);
  if (!await driver.execute('return document.querySelector("[data-provider-settings]").closest("details").open;')) await activate('[data-provider-settings]');
  if (!await driver.execute('return document.querySelector("[data-provider-manual]").closest("details").open;')) await activate('[data-provider-manual]');
  await driver.type('#provider-executable', executable); await activate('.provider-inspection-form button[type=submit]');
  await waitFor(() => driver.execute('const n=document.querySelector("#provider-executable-approved");return Boolean(n&&!n.disabled&&n.getAttribute("aria-checked")==="false");'), 'fixture consent');
  await activate('#provider-executable-approved', ' '); await activate('.provider-approval-form button[type=submit]');
  await waitFor(() => driver.execute('return Boolean(document.querySelector(".provider-approval-result"));'), 'approved synthetic provider');
  for (let index = 0; index < 2; index++) {
    await navigate('overview'); await activate('[data-overview-add-account]'); let profile;
    await waitFor(async () => { profile = (await call('list_profiles')).profiles.find(row => !profiles.some(previous => previous.id === row.id)); return Boolean(profile); }, 'new isolated profile'); profiles.push(profile);
    const home = join(dirname(fixture.database), 'profiles', profile.id); await writeFile(join(home, 'synthetic-collection-config.json'), JSON.stringify({ mode: 'startup_snapshot', requireLogin: true }) + '\n', { mode: 0o600 }); await writeFile(join(home, 'synthetic-private-credential-sentinel.txt'), secretSentinel, { mode: 0o600 });
    await navigate('overview'); await activate(`[data-overview-account="${profile.id}"] td:last-child button:first-child`);
    await waitFor(async () => Boolean(await call('load_profile_activity', { profileId: profile.id })), 'automatic saved synthetic activity', 45000);
    const activity = await call('load_profile_activity', { profileId: profile.id }); assert.equal(activity.sourceKind, 'test_fixture'); assert.equal(activity.summary.lifetimeTokens, '9007199254740993'); assert.ok(await call('load_profile_quota', { profileId: profile.id }));
  }
  const initial = await call('load_profile_activity', { profileId: profiles[0].id }); await close();
  await writeFile(join(fixture.dirs.control, 'activity-history-fixture.json'), JSON.stringify({ purpose: 'openusage-native-activity-history-v1', profileId: profiles[0].id, initialRunId: initial.collectionRunId }), { mode: 0o600, flag: 'wx' });
  evidence.seed = JSON.parse(execFileSync(seeder, [fixture.root, profiles[0].id], { input: JSON.stringify({ passphrase: password }), encoding: 'utf8', timeout: 30000, env: { PATH: '/usr/bin:/bin' } })); assert.equal(evidence.seed.syntheticAcceptedSnapshotsAdded, 21);
  await open(); await archive(profiles[0]);
  const before = await liveHashes(), source = await oracle('archived profiles and all saved application fields'), sessionsBefore = await sessions();
  assert.equal(source.activity_summaries.filter(row => row.profile_id === profiles[0].id).length, 22); assert.equal(source.profiles.find(row => row.id === profiles[0].id).archived, '1');
  const names = await exportNames();
  for (const args of [{}, { confirmed: false }, { confirmed: 'true' }, { confirmed: true, format: 'csv' }, { confirmed: true, profileIds: [profiles[0].id] }, { confirmed: true, path: '../outside' }]) assert.equal((await driver.invoke('export_application_data', args)).ok, false);
  assert.deepEqual(await exportNames(), names);
  const exported = await guiExport(source, true);
  const values = Object.values(exported).flatMap(rows => rows.flatMap(row => Object.values(row))); assert.ok(values.includes('9223372036854775807')); assert.ok(values.includes('9007199254740993')); assert.ok(values.includes('0')); assert.ok(values.includes(null));
  // One saved-reading route checks the shared atomic publisher without repeating history qualification.
  const current = await call('export_saved_readings', { profileIds: [profiles[1].id], format: 'json', includeIdentifiers: true, confirmed: true });
  assert.equal(current.directory, exportDirectory()); const latest = JSON.parse(await readFile(join(current.directory, current.fileName), 'utf8'));
  assert.equal(latest.records.every(row => row.profile_id === profiles[1].id), true); assert.ok(latest.records.some(row => row.record_kind === 'quota_summary')); assert.ok(latest.records.some(row => row.record_kind === 'activity_summary')); assert.equal(JSON.stringify(latest).includes(secretSentinel), false);
  assert.deepEqual(await liveHashes(), before); assert.deepEqual(await oracle('exports and invalid requests preserve all30tables'), source); assert.deepEqual(await sessions(), sessionsBefore);
  await close(); await open();
  const restartBefore = await liveHashes(), restart = await oracle('genuine restart before complete export'); matchesOracle(restart, source);
  const receipt = await call('export_application_data', { confirmed: true }); await readExport(receipt.fileName, restart, receipt);
  assert.deepEqual(await liveHashes(), restartBefore); assert.deepEqual(await oracle('restarted full export remains read-only'), restart); assert.deepEqual(await sessions(), sessionsBefore);
  assert.equal(await readFile(join(fixture.dirs.codex, 'synthetic-fixture.txt'), 'utf8'), CODEX_SENTINEL);
  for (const profile of profiles) assert.equal(await readFile(join(dirname(fixture.database), 'profiles', profile.id, 'synthetic-private-credential-sentinel.txt'), 'utf8'), secretSentinel);
  evidence.acceptance = { emptyWorkspace: true, actualSyntheticSignIns: 2, allTables: 30, everyPopulatedFieldComparedExactly: true, emptyTableColumns: 'structural_checks_and_storage_schema_tests', archivedAndPriorConnections: true, exactI64NullZero: true, explicitIdentifiablePlaintextConsent: true, providerFilesExcluded: true, invalidAndLockedRequestsCreateNoFiles: true, sourceDatabaseWalWrappedKeyUnchanged: true, genuineRestart: true, providerSessions: sessionsBefore, newProviderSessionsDuringExport: 0, latestSharedPublisherRoute: true };
  assert.equal(await hash(application), evidence.applicationSha256); assert.equal(await hash(seeder), evidence.seederSha256); assert.equal(await hash(executable), providerDigest); await close(); evidence.result = 'passed';
} catch (error) { evidence.error = error.stack ?? String(error); process.exitCode = 1; }
finally { try { await driver?.stop(); } finally { stopGroup(fixture?.bus, 'SIGKILL'); if (fixture) await rm(fixture.root, { recursive: true, force: true }); await writeFile(join(output, 'result.json'), JSON.stringify(evidence, null, 2) + '\n', { mode: 0o600 }); } }
