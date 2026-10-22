import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { chmod, copyFile, lstat, mkdir, readFile, readlink, rm, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { CODEX_SENTINEL, createFixture, stopGroup } from './private-fixture.mjs';
import { NativeDriver, waitFor } from './webdriver.mjs';

for (const name of ['OPENUSAGE_NATIVE_BINARY', 'OPENUSAGE_NATIVE_EVIDENCE', 'OPENUSAGE_LOGIN_VERIFIER']) assert.ok(process.env[name], name);
const repository = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const application = resolve(process.env.OPENUSAGE_NATIVE_BINARY), output = resolve(process.env.OPENUSAGE_NATIVE_EVIDENCE), verifier = resolve(process.env.OPENUSAGE_LOGIN_VERIFIER);
const password = 'OpenUsage synthetic account signout workspace 2026';
const collectionDigest = '53a853b737ceca311649e9f1d9ebd1341154a5f782efd8e915f7f834c819a6dc';
const logoutDigest = '3f3f22fb2617d79e73bde3a1c3386f28302215f5f997326782d3d35a7f952b88';
const verifierDigest = 'a2e4a64707081a9943f3c04f8237c2b16bea673e974e96393d6a8a7d9357f975';
const hash = async path => createHash('sha256').update(await readFile(path)).digest('hex');
const evidence = { requirement: 'NATIVE-ACCOUNT-SIGNOUT-001', result: 'failed', scope: 'Actual Linux Tauri selected-profile sign-out with two explicitly authorized synthetic fixture accounts and saved SQLCipher4/WAL schema8 readings. Official-shaped account/logout is exercised only by the reviewed synthetic provider. Independent30table comparisons preserve saved history and the other account. Explicit GUI consent/cancel, invalid and locked refusals, genuine restart, other-account refresh and selected-account reconnect are checked. No real OAuth revocation, provider keyring or other-platform acceptance.', oracles: [], closes: [] };
let fixture, driver, sequence = 0, collection, logout, approvalId;
const profiles = [];
const call = async (command, args) => { const response = await driver.invoke(command, args); assert.equal(response.ok, true, `${command}: ${JSON.stringify(response)}`); return response.value; };
const home = profile => join(dirname(fixture.database), 'profiles', profile.id);
const marker = profile => join(home(profile), 'synthetic-monitored-login-completed');
const normalized = value => { const copy = structuredClone(value); if (copy) { delete copy.freshness; delete copy.monitoring; } return copy; };
async function key(value) { await driver.request('POST', `/session/${driver.session}/actions`, { actions: [{ type: 'key', id: 'account-signout-keyboard', actions: [{ type: 'keyDown', value }, { type: 'keyUp', value }] }] }); await driver.request('DELETE', `/session/${driver.session}/actions`); }
async function activate(selector, value = '\uE007') {
  await waitFor(() => driver.execute('const n=document.querySelector(arguments[0]);return Boolean(n&&!n.disabled&&n.getClientRects().length);', [selector]), `available ${selector}`);
  assert.equal(await driver.execute('const n=document.querySelector(arguments[0]);n.scrollIntoView({block:"nearest",inline:"nearest"});n.focus({preventScroll:true});return document.activeElement===n;', [selector]), true); await key(value);
}
async function navigate(view) {
  if (await driver.execute('return Boolean(document.querySelector("[data-account-panel]")?.open);')) { await activate('[data-account-panel-close]'); await waitFor(() => driver.execute('return !document.querySelector("[data-account-panel]")?.open;'), 'account dialog closed'); }
  await activate(`[data-workspace-nav="${view}"]`);
  await waitFor(() => driver.execute('const n=document.querySelector(`[data-workspace-nav="${arguments[0]}"]`),p=document.querySelector(`[data-workspace-view="${arguments[0]}"]`);return n?.getAttribute("aria-current")==="page"&&Boolean(p&&!p.hidden&&p.getClientRects().length);', [view]), `native ${view}`);
}
async function open(setup = false) {
  driver = new NativeDriver(application, fixture.environment, join(output, `driver-${++sequence}.log`)); await driver.start();
  await waitFor(() => driver.execute('const a=document.querySelector("#workspace-passphrase"),b=document.querySelector("#workspace-confirmation");return Boolean(a&&!a.disabled&&(!arguments[0]||b&&!b.disabled));', [setup]), 'workspace password fields');
  assert.deepEqual(await driver.invoke('sign_out_profile', { profileId: randomUUID(), approvalId: randomUUID(), confirmed: true }), { ok: false, error: 'workspace_locked' });
  await driver.type('#workspace-passphrase', password); if (setup) await driver.type('#workspace-confirmation', password); await key('\uE007');
  await waitFor(async () => (await call('bootstrap')).storage === 'ready', 'ready signout workspace'); assert.equal((await call('bootstrap')).schemaVersion, 8);
}
async function close() {
  const queue = [driver.child.pid]; let owned;
  for (let count = 0; queue.length && count < 64; count++) { const pid = queue.shift(); try { if (await readlink(`/proc/${pid}/exe`) === application) { owned = pid; break; } queue.push(...(await readFile(`/proc/${pid}/task/${pid}/children`, 'utf8')).trim().split(/\s+/).filter(Boolean).map(Number)); } catch (error) { if (error.code !== 'ENOENT') throw error; } }
  assert.ok(owned); evidence.closes.push(JSON.parse(execFileSync('/usr/bin/python3', [join(repository, 'tests/native/x11-close.py'), String(owned)], { env: fixture.environment, encoding: 'utf8', timeout: 10000 })));
  await waitFor(async () => { try { await readlink(`/proc/${owned}/exe`); return false; } catch (error) { if (error.code === 'ENOENT') return true; throw error; } }, 'genuine native close'); driver.session = undefined; await driver.stop(); driver = undefined;
}
async function approve(executable) {
  await navigate('settings');
  for (const selector of ['[data-provider-settings]', '[data-provider-manual]']) if (!await driver.execute('return document.querySelector(arguments[0]).closest("details").open;', [selector])) await activate(selector);
  await driver.type('#provider-executable', executable); await activate('.provider-inspection-form button[type=submit]');
  await waitFor(() => driver.execute('const n=document.querySelector("#provider-executable-approved");return Boolean(n&&!n.disabled&&n.getAttribute("aria-checked")==="false");'), 'explicit source consent');
  await activate('#provider-executable-approved', ' '); await activate('.provider-approval-form button[type=submit]');
  await waitFor(() => driver.execute('return Boolean(document.querySelector(".provider-approval-result"))&&!document.querySelector("#provider-executable-approved");'), 'approved source');
}
async function saved(profile) { return { quota: normalized(await call('load_profile_quota', { profileId: profile.id })), activity: normalized(await call('load_profile_activity', { profileId: profile.id })) }; }
async function originalMode(profile) { await writeFile(join(home(profile), 'synthetic-collection-config.json'), JSON.stringify({ mode: 'startup_snapshot', requireLogin: true }) + '\n', { mode: 0o600 }); }
async function addAndSignIn() {
  await navigate('overview'); await activate('[data-overview-add-account]'); let profile;
  await waitFor(async () => { profile = (await call('list_profiles')).profiles.find(row => !profiles.some(previous => previous.id === row.id)); return Boolean(profile); }, 'new isolated account'); profiles.push(profile);
  await originalMode(profile); await navigate('overview'); await activate(`[data-overview-account="${profile.id}"] td:last-child button:first-child`);
  await waitFor(async () => Boolean((await saved(profile)).activity), 'automatic accepted readings', 45000);
  const value = await saved(profile); assert.equal(value.activity.sourceKind, 'test_fixture'); assert.equal(value.activity.summary.lifetimeTokens, '9007199254740993'); assert.ok(value.quota); return profile;
}
async function options(profile) {
  await navigate('settings'); await activate('[data-account-manager-open]');
  for (const action of ['readings', 'options']) {
    const selector = `[data-profile-id="${profile.id}"] [data-profile-action="${action}"]`;
    await waitFor(() => driver.execute('const n=document.querySelector(arguments[0]);return Boolean(n&&!n.disabled&&n.getClientRects().length);', [selector]), `profile ${action}`);
    if (await driver.execute('return document.querySelector(arguments[0]).getAttribute("aria-expanded")==="false";', [selector])) await activate(selector);
    await waitFor(() => driver.execute('return document.querySelector(arguments[0])?.getAttribute("aria-expanded")==="true";', [selector]), `expanded ${action}`);
  }
}
async function liveHashes() {
  const result = {}; for (const path of [fixture.database, `${fixture.database}-wal`, join(dirname(fixture.database), 'database-key.v1')]) { try { result[path] = await hash(path); } catch (error) { if (error.code !== 'ENOENT') throw error; result[path] = null; } } return result;
}
async function oracle(label) {
  assert.equal(await hash(verifier), verifierDigest); const before = await liveHashes(), temporary = join(fixture.root, `signout-oracle-${randomUUID()}`); await mkdir(temporary, { mode: 0o700 });
  try {
    const database = join(temporary, 'database.db'); await copyFile(fixture.database, database); await chmod(database, 0o600); if (before[`${fixture.database}-wal`] !== null) { await copyFile(`${fixture.database}-wal`, `${database}-wal`); await chmod(`${database}-wal`, 0o600); }
    const raw = execFileSync(verifier, ['database', database, join(dirname(fixture.database), 'database-key.v1'), '--profile-state'], { input: JSON.stringify({ passphrase: password }), encoding: 'utf8', env: fixture.environment, timeout: 30000, maxBuffer: 8 * 1024 * 1024 });
    const report = JSON.parse(execFileSync('/usr/bin/python3', ['-c', 'import json,sys; print(json.dumps(json.load(sys.stdin,parse_int=str)))'], { input: raw, encoding: 'utf8', timeout: 10000, maxBuffer: 8 * 1024 * 1024, env: { PATH: '/usr/bin:/bin' } }));
    for (const flag of ['verified', 'readOnly', 'inputUnchanged', 'keyEnvelopeUnchanged', 'walRecordsUnchanged']) assert.equal(report[flag], true); assert.equal(report.schemaVersion, '8'); assert.equal(Object.keys(report.state.tables).length, 30); assert.deepEqual(await liveHashes(), before);
    evidence.oracles.push({ label, tables: Object.fromEntries(Object.entries(report.state.tables).map(([name, rows]) => [name, { rows: rows.length, sha256: createHash('sha256').update(JSON.stringify(rows)).digest('hex') }])) }); return report.state.tables;
  } finally { await rm(temporary, { recursive: true, force: true }); }
}
function onlySelectedConnectionChanged(before, after, profile) {
  const adjusted = structuredClone(after); const original = before.profiles.find(row => row.id === profile.id), current = adjusted.profiles.find(row => row.id === profile.id);
  assert.equal(current.connection_state, 'authentication_required'); current.connection_state = original.connection_state; current.updated_at = original.updated_at;
  assert.deepEqual(adjusted, before, 'Only the selected profile connection state and update timestamp may change; all saved readings/history remain exact.');
}
async function observations(profile, file = 'synthetic-logout-observations.jsonl') { try { return (await readFile(join(home(profile), file), 'utf8')).trim().split('\n').filter(Boolean).map(JSON.parse); } catch (error) { if (error.code !== 'ENOENT') throw error; return []; } }
const logoutRequests = rows => rows.filter(row => row.kind === 'request' && row.request?.method === 'account/logout');

await mkdir(output, { recursive: true, mode: 0o700 });
process.once('exit', () => { driver?.kill('SIGKILL'); stopGroup(fixture?.bus, 'SIGKILL'); });
try {
  evidence.applicationSha256 = await hash(application); assert.equal(await hash(verifier), verifierDigest);
  fixture = await createFixture('account-signout'); fixture.environment.HOME = join(fixture.root, 'synthetic-home'); await mkdir(fixture.environment.HOME, { mode: 0o700 });
  collection = join(fixture.root, 'synthetic-collection-provider.py'); logout = join(fixture.root, 'synthetic-logout-provider.py');
  for (const [source, destination, digest] of [['synthetic-collection-provider.py', collection, collectionDigest], ['synthetic-logout-provider.py', logout, logoutDigest]]) { await copyFile(join(repository, 'crates/openusage-codex/tests/fixtures', source), destination); await chmod(destination, 0o700); assert.equal(await hash(destination), digest); }
  await open(true); await approve(collection); const first = await addAndSignIn(), second = await addAndSignIn();
  const firstSaved = await saved(first), secondSaved = await saved(second), secondMarker = await hash(marker(second)), firstMarker = await hash(marker(first)), keyHash = await hash(join(dirname(fixture.database), 'database-key.v1'));
  await approve(logout); approvalId = (await call('check_profile_connection', { profileId: first.id })).approvalId;
  const before = await oracle('two connected profiles before selected signout'), refusedLogs = await observations(first);
  for (const args of [{ profileId: first.id, approvalId, confirmed: false }, { profileId: first.id, approvalId }, { profileId: first.id, approvalId, confirmed: 'true' }, { profileId: first.id, approvalId, confirmed: true, extra: true }, { profileId: '../outside', approvalId, confirmed: true }, { profileId: first.id, approvalId: randomUUID(), confirmed: true }]) assert.equal((await driver.invoke('sign_out_profile', args)).ok, false);
  assert.deepEqual(await observations(first), refusedLogs); assert.equal(await hash(marker(first)), firstMarker); assert.deepEqual(await oracle('invalid and unconfirmed signout preserve all30tables'), before);
  await writeFile(join(home(first), 'synthetic-logout-config.json'), JSON.stringify({ mode: 'rpc_error' }) + '\n', { mode: 0o600 });
  const failed = await driver.invoke('sign_out_profile', { profileId: first.id, approvalId, confirmed: true }); assert.equal(failed.ok, false);
  assert.equal(await hash(marker(first)), firstMarker); assert.equal(await hash(marker(second)), secondMarker); assert.deepEqual(await saved(first), firstSaved); assert.deepEqual(await saved(second), secondSaved); assert.deepEqual(await oracle('provider RPC refusal preserves both accounts and all saved fields'), before);
  assert.equal(logoutRequests(await observations(first)).length, 1);
  await writeFile(join(home(first), 'synthetic-logout-config.json'), JSON.stringify({ mode: 'success' }) + '\n', { mode: 0o600 });
  const beforeConsentLogs = await observations(first);
  await options(first); await activate(`[data-account-signout-open="${first.id}"]`);
  const dialog = `[data-account-signout-dialog="${first.id}"]`;
  await waitFor(() => driver.execute('return Boolean(document.querySelector(arguments[0])?.open);', [dialog]), 'selected signout confirmation');
  assert.equal(await driver.execute('return document.activeElement.matches(arguments[0]);', [`${dialog} [data-account-signout-cancel]`]), true);
  assert.equal(await driver.execute('return document.querySelector(arguments[0]).textContent.includes(arguments[1]);', [dialog, first.label]), true);
  await driver.screenshot(join(output, 'account-signout-confirmation.png'));
  await activate(`${dialog} [data-account-signout-cancel]`); await waitFor(() => driver.execute('return !document.querySelector(arguments[0])?.open;', [dialog]), 'signout cancellation');
  assert.deepEqual(await observations(first), beforeConsentLogs); assert.equal(await hash(marker(first)), firstMarker);
  await activate(`[data-account-signout-open="${first.id}"]`); await waitFor(() => driver.execute('return Boolean(document.querySelector(arguments[0])?.open);', [dialog]), 'renewed explicit signout confirmation'); await activate(`${dialog} [data-account-signout-submit]`);
  await waitFor(async () => (await call('list_profiles')).profiles.find(row => row.id === first.id)?.connectionState === 'authentication_required', 'selected account signed out', 45000);
  await assert.rejects(lstat(marker(first)), { code: 'ENOENT' }); assert.equal(await hash(marker(second)), secondMarker); assert.deepEqual(await saved(first), firstSaved); assert.deepEqual(await saved(second), secondSaved); assert.equal(await hash(join(dirname(fixture.database), 'database-key.v1')), keyHash);
  const after = await oracle('successful signout changes only selected connection'); onlySelectedConnectionChanged(before, after, first);
  assert.equal(logoutRequests(await observations(first)).length, 2); assert.equal(logoutRequests(await observations(second)).length, 0);
  assert.equal((await observations(first)).filter(row => row.kind === 'logout-completed').length, 1);
  await navigate('overview'); const row = `[data-overview-account="${first.id}"]`; assert.equal(await driver.execute('return document.querySelector(arguments[0]).textContent.trim();', [`${row} td:last-child button:first-child`]), 'Sign in');
  await close(); await open(); assert.deepEqual(await saved(first), firstSaved); assert.deepEqual(await saved(second), secondSaved);
  assert.equal((await call('list_profiles')).profiles.find(row => row.id === first.id).connectionState, 'authentication_required'); assert.equal((await call('list_profiles')).profiles.find(row => row.id === second.id).connectionState, 'connected'); assert.deepEqual(await oracle('genuine restart retains both histories and selected signedout state'), after);
  await approve(collection); await driver.waitForProfileRefresh(second.id, 45000); await navigate('overview'); await activate(`[data-overview-row-refresh="${second.id}"]`);
  await waitFor(async () => (await saved(second)).activity.collectionRunId !== secondSaved.activity.collectionRunId, 'other account refreshes using retained synthetic authorization', 45000); assert.deepEqual(await saved(first), firstSaved); assert.equal(await hash(marker(second)), secondMarker);
  await navigate('overview'); await activate(`${row} td:last-child button:first-child`);
  await waitFor(async () => (await call('list_profiles')).profiles.find(item => item.id === first.id)?.connectionState === 'connected' && (await saved(first)).activity.collectionRunId !== firstSaved.activity.collectionRunId, 'selected account reconnect automatically saves readings', 45000);
  assert.ok((await lstat(marker(first))).isFile()); const final = await oracle('reconnected selected account and independently refreshed second account');
  for (const profile of profiles) assert.equal(final.profiles.find(row => row.id === profile.id).connection_state, 'connected');
  assert.equal(await readFile(join(fixture.dirs.codex, 'synthetic-fixture.txt'), 'utf8'), CODEX_SENTINEL); assert.equal(await hash(application), evidence.applicationSha256); assert.equal(await hash(collection), collectionDigest); assert.equal(await hash(logout), logoutDigest);
  evidence.acceptance = { syntheticAccounts: 2, explicitSelectedSignout: true, cancellationAndInvalidRequestsPreserveState: true, failedProviderLogout: 1, successfulProviderLogout: 1, selectedLogoutRequests: 2, otherLogoutRequests: 0, all30TableReadingsPreserved: true, otherSyntheticAuthorizationUnchanged: true, wrappedDatabaseKeyUnchanged: true, genuineRestart: true, otherAccountRefresh: true, selectedReconnectAutocollection: true, providerKeyringOrRealRevocation: 'not_tested' };
  await close(); evidence.result = 'passed';
} catch (error) { evidence.error = error.stack ?? String(error); process.exitCode = 1; }
finally { try { await driver?.stop(); } finally { stopGroup(fixture?.bus, 'SIGKILL'); if (fixture) await rm(fixture.root, { recursive: true, force: true }); await writeFile(join(output, 'result.json'), JSON.stringify(evidence, null, 2) + '\n', { mode: 0o600 }); } }
