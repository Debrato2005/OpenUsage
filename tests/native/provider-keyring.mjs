import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { chmod, lstat, mkdir, readFile, readdir, readlink, rm, stat, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { CODEX_SENTINEL, createFixture, stopGroup } from './private-fixture.mjs';
import { startSecretService, stopSecretService } from './secret-service.mjs';
import { NativeDriver, waitFor } from './webdriver.mjs';

const repository = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const application = resolve(process.env.OPENUSAGE_NATIVE_BINARY);
const output = resolve(process.env.OPENUSAGE_NATIVE_EVIDENCE);
const sdk = resolve(process.env.OPENUSAGE_SECRET_SERVICE_SDK);
const password = 'OpenUsage synthetic protected provider workspace 2026';
const marker = '.openusage-credentials-v1', markerText = 'openusage-provider-keyring-direct-v1\n';
const sha = value => createHash('sha256').update(value).digest('hex');
const hash = async path => sha(await readFile(path));
const account = home => `cli|${sha(home).slice(0, 16)}`;
const evidence = { requirement: 'NATIVE-PROVIDER-KEYRING-001', result: 'failed', scope: 'Default Linux native Tauri application and pinned official companion. Fresh private Secret Service with nonfunctional synthetic API-key credentials proves selected-home protected retrieval, restart, official logout and exact managed cleanup. This is not ChatGPT OAuth login, token renewal, revocation or real usage acceptance. No existing credential home, inference request or borrowed credentials.', closes: [] };
let fixture, driver, sequence = 0;
const call = async (command, args) => { const response = await driver.invoke(command, args); assert.equal(response.ok, true, command); return response.value; };
function secret(action, home, value, extra = []) {
  const result = execFileSync(join(sdk, 'runtime/usr/bin/secret-tool'), [action, ...(action === 'store' ? ['--label=OpenUsage nonfunctional provider test'] : []), 'service', 'Codex Auth', 'username', account(home), ...extra], { env: { ...fixture.environment, LD_LIBRARY_PATH: join(sdk, 'runtime/usr/lib/x86_64-linux-gnu') }, input: value, encoding: 'utf8', timeout: 5000, stdio: ['pipe', 'pipe', 'pipe'] });
  return result;
}
function present(home, expected) {
  let value;
  try { value = secret('lookup', home); } catch (error) { if (error.status === 1 && expected === null) return; throw new Error('The private synthetic credential lookup failed.'); }
  assert.notEqual(expected, null, 'The selected provider entry must be absent.');
  assert.equal(sha(value.trim()), sha(expected));
}
async function open(setup = false) {
  driver = new NativeDriver(application, fixture.environment, join(output, `driver-${++sequence}.log`)); await driver.start();
  await waitFor(() => driver.execute('const a=document.querySelector("#workspace-passphrase"),b=document.querySelector("#workspace-confirmation");return Boolean(a&&!a.disabled&&(!arguments[0]||b&&!b.disabled));', [setup]), 'private workspace password fields');
  await driver.type('#workspace-passphrase', password); if (setup) await driver.type('#workspace-confirmation', password);
  await activate('.unlock-form button[type=submit]'); await waitFor(async () => (await call('bootstrap')).storage === 'ready', 'ready workspace');
}
async function close() {
  const queue = [driver.child.pid]; let owned;
  for (let count = 0; queue.length && count < 64; count++) { const pid = queue.shift(); try { if (await readlink(`/proc/${pid}/exe`) === application) { owned = pid; break; } queue.push(...(await readFile(`/proc/${pid}/task/${pid}/children`, 'utf8')).trim().split(/\s+/).filter(Boolean).map(Number)); } catch (error) { if (error.code !== 'ENOENT') throw error; } }
  assert.ok(owned); evidence.closes.push(JSON.parse(execFileSync('/usr/bin/python3', [join(repository, 'tests/native/x11-close.py'), String(owned)], { env: fixture.environment, encoding: 'utf8', timeout: 10000 })));
  await waitFor(async () => { try { await readlink(`/proc/${owned}/exe`); return false; } catch (error) { if (error.code === 'ENOENT') return true; throw error; } }, 'genuine native close'); driver.session = undefined; await driver.stop(); driver = undefined;
}
async function activate(selector, key = '\uE007') {
  await waitFor(() => driver.execute('const n=document.querySelector(arguments[0]);return Boolean(n&&!n.disabled&&n.getClientRects().length);', [selector]), 'available native action');
  await driver.execute('document.querySelector(arguments[0]).focus();', [selector]);
  await driver.request('POST', `/session/${driver.session}/actions`, { actions: [{ type: 'key', id: 'keyring-keyboard', actions: [{ type: 'keyDown', value: key }, { type: 'keyUp', value: key }] }] });
}
async function add() {
  const before = new Set((await call('list_profiles')).profiles.map(profile => profile.id));
  await driver.navigate('overview'); await activate('[data-overview-add-account]'); let profile;
  await waitFor(async () => { profile = (await call('list_profiles')).profiles.find(profile => !before.has(profile.id)); return Boolean(profile); }, 'new isolated account');
  return profile;
}
function syntheticCredential() { return JSON.stringify({ auth_mode: 'apikey', OPENAI_API_KEY: `ou-nonfunctional-keyring-${randomUUID()}`, tokens: null, last_refresh: null }); }

await mkdir(output, { recursive: true, mode: 0o700 });
process.once('exit', () => { driver?.kill('SIGKILL'); fixture?.daemon?.kill('SIGKILL'); stopGroup(fixture?.bus, 'SIGKILL'); });
try {
  evidence.applicationSha256 = await hash(application); const companion = join(dirname(application), 'openusage-codex');
  evidence.companionSha256 = await hash(companion); assert.equal(evidence.companionSha256, '399ff69c431c066e40de318fba45c2b07f45a76b19939d247ee4828fc70c4215');
  fixture = await createFixture('provider-keyring'); fixture.environment.HOME = join(fixture.root, 'synthetic-home'); await mkdir(fixture.environment.HOME, { mode: 0o700 });
  await open(true); await startSecretService(fixture);
  const workspace = dirname(fixture.database), profilesRoot = join(workspace, 'profiles');
  const home = profile => join(profilesRoot, profile.id);
  const first = await add(), second = await add(); let approval = await call('prepare_provider_connection'); assert.equal(approval.sourceKind, 'official_artifact');
  for (const profile of [first, second]) {
    const result = await call('check_profile_connection', { profileId: profile.id }); assert.equal(result.authentication, 'authentication_required');
    assert.equal(await readFile(join(home(profile), marker), 'utf8'), markerText); assert.equal((await stat(join(home(profile), marker))).mode & 0o777, 0o600);
  }
  const firstValue = syntheticCredential(), secondValue = syntheticCredential(); secret('store', home(first), firstValue); secret('store', home(second), secondValue);
  for (const profile of [first, second]) { const result = await call('check_profile_connection', { profileId: profile.id }); assert.equal(result.authentication, 'unsupported'); assert.equal(result.identityConfidence, 'unknown'); assert.ok(!JSON.stringify(result).includes('ou-nonfunctional')); }
  const legacy = await add(), legacyValue = syntheticCredential();
  await writeFile(join(home(legacy), 'auth.json'), legacyValue, { mode: 0o600 });
  assert.equal((await call('check_profile_connection', { profileId: legacy.id })).authentication, 'unsupported');
  assert.equal((await readdir(home(legacy))).includes(marker), false);
  assert.equal(await hash(join(home(legacy), 'auth.json')), sha(legacyValue));
  present(home(first), firstValue); present(home(second), secondValue);
  for (const profile of [first, second]) assert.equal((await readdir(home(profile))).includes('auth.json'), false);
  await close(); await open(); approval = await call('prepare_provider_connection');
  for (const profile of [first, second]) assert.equal((await call('check_profile_connection', { profileId: profile.id })).authentication, 'unsupported');
  await call('sign_out_profile', { profileId: first.id, approvalId: approval.approvalId, confirmed: true }); present(home(first), null); present(home(second), secondValue);
  assert.equal((await call('check_profile_connection', { profileId: second.id })).authentication, 'unsupported');
  await call('set_profile_archived', { id: second.id, archived: true });
  // A restore can retain old managed homes without current profile mappings. Cleanup
  // must include their marker-bound entries, without reading any credential content.
  const orphan = join(profilesRoot, randomUUID()); await mkdir(orphan, { mode: 0o700 }); await writeFile(join(orphan, marker), markerText, { mode: 0o600 }); const orphanValue = syntheticCredential(); secret('store', orphan, orphanValue);
  const unrelatedHome = join(fixture.root, 'unrelated-home'); await mkdir(unrelatedHome, { mode: 0o700 }); const unrelatedValue = syntheticCredential(); secret('store', unrelatedHome, unrelatedValue);
  const duplicateValue = syntheticCredential();
  for (const tag of ['one', 'two']) secret('store', home(first), duplicateValue, ['openusage-fixture-tag', tag]);
  evidence.phase = 'before-unavailable-store';
  evidence.fixtureMetadata = [];
  async function inventory(path, relative = '') {
    if (evidence.fixtureMetadata.length >= 300) return;
    const info = await lstat(path);
    evidence.fixtureMetadata.push({ path: relative, mode: info.mode.toString(8), links: info.nlink, kind: info.isSymbolicLink() ? 'link' : info.isDirectory() ? 'directory' : info.isFile() ? 'file' : 'special', ...(info.isSymbolicLink() ? { target: await readlink(path) } : {}) });
    if (info.isDirectory()) for (const entry of await readdir(path)) await inventory(join(path, entry), `${relative}/${entry}`);
  }
  await inventory(workspace);
  const before = await call('list_profiles'), wrappedBefore = await hash(join(workspace, 'database-key.v1'));
  await stopSecretService(fixture);
  assert.deepEqual(await driver.invoke('check_profile_connection', { profileId: first.id }), { ok: false, error: 'provider_credential_store_unavailable' }); assert.deepEqual(await call('list_profiles'), before);
  evidence.phase = 'unavailable-store-deletion';
  assert.deepEqual(await driver.invoke('delete_all_local_data', { confirmed: true }), { ok: false, error: 'provider_credential_store_unavailable' });
  assert.equal((await call('bootstrap')).storageIssue, 'local_deletion_incomplete'); assert.equal(await hash(join(workspace, 'database-key.v1')), wrappedBefore);
  for (const path of [home(first), home(second), orphan]) assert.equal(await readFile(join(path, marker), 'utf8'), markerText);
  await close(); await startSecretService(fixture); present(home(second), secondValue); present(orphan, orphanValue); present(unrelatedHome, unrelatedValue);
  evidence.phase = 'cleanup-restart';
  // Restart must retain the deletion blocker and require renewed explicit consent.
  driver = new NativeDriver(application, fixture.environment, join(output, `driver-${++sequence}.log`)); await driver.start();
  await waitFor(async () => (await call('bootstrap')).storageIssue === 'local_deletion_incomplete', 'pending cleanup restart');
  assert.deepEqual(await driver.invoke('delete_all_local_data', { confirmed: false }), { ok: false, error: 'invalid_arguments' });
  assert.deepEqual(await driver.invoke('delete_all_local_data', { confirmed: true }), { ok: false, error: 'provider_credential_store_unavailable' });
  for (const path of [home(first), home(second), orphan]) assert.equal(await readFile(join(path, marker), 'utf8'), markerText);
  secret('clear', home(first), undefined, ['openusage-fixture-tag', 'one']);
  evidence.phase = 'confirmed-final-cleanup';
  const fresh = await call('delete_all_local_data', { confirmed: true }); assert.equal(fresh.storage, 'setup_required'); assert.equal(fresh.keyProtection, 'os_store');
  present(home(first), null); present(home(second), null); present(orphan, null); present(unrelatedHome, unrelatedValue);
  assert.equal((await readdir(workspace)).includes('profiles'), false); assert.deepEqual(await readdir(fixture.dirs.codex), ['synthetic-fixture.txt']); assert.equal(await readFile(join(fixture.dirs.codex, 'synthetic-fixture.txt'), 'utf8'), CODEX_SENTINEL);
  assert.equal(await hash(application), evidence.applicationSha256); assert.equal(await hash(companion), evidence.companionSha256);
  evidence.checks = { officialSource: true, twoProtectedHomes: true, existingFileSessionPreserved: true, ambiguousCredentialRefusal: true, noAuthJson: true, localKeyringRetrieval: true, genuineRestart: true, selectedOfficialLogout: true, secondCredentialPreserved: true, archivedAndUnmappedHomes: true, noRawCredentialIPC: true, unavailableStoreRefusesDowngrade: true, savedProfilesAndWrappedKeyPreservedOnFailure: true, deletionBlockerSurvivesRestart: true, renewedConsent: true, exactCleanup: true, unrelatedCredentialPreserved: true, existingCredentialHomeUntouched: true };
  await close(); evidence.result = 'passed';
} catch (error) { evidence.error = error instanceof assert.AssertionError ? `${error.message}\n${error.stack?.split('\n').filter(line => line.includes('provider-keyring.mjs')).join('\n')}` : String(error?.message ?? 'Private provider keyring verification failed'); process.exitCode = 1; }
finally { try { try { await driver?.stop(); } finally { if (fixture) { try { await stopSecretService(fixture); } finally { stopGroup(fixture.bus, 'SIGKILL'); await rm(fixture.root, { recursive: true, force: true }); } } } } catch { evidence.cleanupError = 'Private fixture cleanup failed'; evidence.result = 'failed'; process.exitCode = 1; } await writeFile(join(output, 'result.json'), JSON.stringify(evidence, null, 2) + '\n', { mode: 0o600 }); }
console.log(`NATIVE-PROVIDER-KEYRING-001: ${evidence.result}. Evidence: ${output}`);
