import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdir, readFile, readlink, rm, stat, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createFixture, stopGroup } from './private-fixture.mjs';
import { NativeDriver, waitFor } from './webdriver.mjs';
import { startSecretService, stopSecretService } from './secret-service.mjs';

const repository = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const application = resolve(process.env.OPENUSAGE_NATIVE_BINARY);
const output = resolve(process.env.OPENUSAGE_NATIVE_EVIDENCE);
const sdk = resolve(process.env.OPENUSAGE_SECRET_SERVICE_SDK);
const password = 'OpenUsage synthetic fallback transition 2026';
const evidence = { result: 'failed', scope: 'Default native Linux application, fresh synthetic records and a real private Secret Service. A fallback workspace is deleted after the OS store becomes available. Fresh setup must need no app password. No provider sign-in, existing credential home or real account deletion.' };
const hash = async path => createHash('sha256').update(await readFile(path)).digest('hex');
let fixture, driver, sequence = 0;
const call = async (command, args) => { const response = await driver.invoke(command, args); assert.equal(response.ok, true, command); return response.value; };
async function open() {
  driver = new NativeDriver(application, fixture.environment, join(output, `driver-${++sequence}.log`));
  await driver.start();
  await waitFor(() => driver.execute('return document.querySelector("#startup-heading")?.textContent!=="Checking local storage…";'), 'native startup');
}
async function close() {
  const queue = [driver.child.pid]; let selected;
  for (let count = 0; queue.length && count < 64; count++) {
    const pid = queue.shift();
    try { if (await readlink(`/proc/${pid}/exe`) === application) { selected = pid; break; } queue.push(...(await readFile(`/proc/${pid}/task/${pid}/children`, 'utf8')).trim().split(/\s+/).filter(Boolean).map(Number)); }
    catch (error) { if (error.code !== 'ENOENT') throw error; }
  }
  assert.ok(selected);
  execFileSync('/usr/bin/python3', [join(repository, 'tests/native/x11-close.py'), String(selected)], { env: fixture.environment, timeout: 10000 });
  await waitFor(async () => { try { await readlink(`/proc/${selected}/exe`); return false; } catch (error) { if (error.code === 'ENOENT') return true; throw error; } }, 'genuine native exit');
  driver.session = undefined; await driver.stop(); driver = undefined;
}
function secretEnvironment() { return { ...fixture.environment, LD_LIBRARY_PATH: join(sdk, 'runtime/usr/lib/x86_64-linux-gnu') }; }
function unrelated(action) {
  return execFileSync(join(sdk, 'runtime/usr/bin/secret-tool'), [action, ...(action === 'store' ? ['--label=OpenUsage synthetic unrelated key'] : []), 'service', 'app.openusage.synthetic-unrelated', 'username', 'transition-test'], { env: secretEnvironment(), input: action === 'store' ? 'Synthetic unrelated key\n' : undefined, encoding: 'utf8', timeout: 5000 });
}
await mkdir(output, { recursive: true, mode: 0o700 });
process.once('exit', () => { driver?.kill('SIGKILL'); fixture?.daemon?.kill('SIGKILL'); stopGroup(fixture?.bus, 'SIGKILL'); });
try {
  evidence.applicationSha256 = await hash(application);
  fixture = await createFixture('deletion-key-store-recovery');
  fixture.environment.HOME = join(fixture.root, 'synthetic-home'); await mkdir(fixture.environment.HOME, { mode: 0o700 });
  await open();
  assert.equal((await call('bootstrap')).keyProtection, 'passphrase_required');
  await waitFor(() => driver.execute('return Boolean(document.querySelector("#workspace-confirmation")&&!document.querySelector("#workspace-confirmation").disabled);'), 'fallback setup');
  await driver.type('#workspace-passphrase', password); await driver.type('#workspace-confirmation', password);
  await driver.click('.unlock-form button[type=submit]');
  await waitFor(async () => (await call('bootstrap')).storage === 'ready', 'fallback ready');
  assert.equal((await call('bootstrap')).keyProtection, 'passphrase');
  await driver.navigate('overview'); await driver.click('[data-overview-add-account]');
  await waitFor(async () => (await call('list_profiles')).profiles.length === 1, 'synthetic account record');
  await driver.navigate('settings');
  await startSecretService(fixture); unrelated('store');
  assert.equal(unrelated('lookup').trim(), 'Synthetic unrelated key');
  await waitFor(() => driver.execute('const n=document.querySelector("[data-local-data-deletion-open]");return Boolean(n&&!n.disabled&&n.getClientRects().length);'), 'deletion action');
  await driver.click('[data-local-data-deletion-open]');
  await waitFor(() => driver.execute('return Boolean(document.querySelector("[data-local-data-deletion-dialog]")?.open);'), 'explicit deletion dialog');
  await driver.click('[data-local-data-deletion-confirm]');
  await waitFor(() => driver.execute('return !document.querySelector("[data-local-data-deletion-submit]").disabled;'), 'confirmed deletion');
  await driver.click('[data-local-data-deletion-submit]');
  await waitFor(async () => (await call('bootstrap')).storage === 'setup_required', 'deleted workspace');
  const fresh = await call('bootstrap'); assert.equal(fresh.keyProtection, 'os_store'); assert.equal(fresh.storageIssue, null);
  await waitFor(() => driver.execute('return !document.querySelector("#workspace-passphrase")&&Boolean(document.querySelector(".startup-content .primary-button"));'), 'password-free fresh onboarding');
  assert.equal(unrelated('lookup').trim(), 'Synthetic unrelated key');
  await driver.click('.startup-content .primary-button');
  await waitFor(async () => (await call('bootstrap')).storage === 'ready', 'new OS-protected workspace');
  assert.equal((await call('bootstrap')).keyProtection, 'os_store'); assert.deepEqual((await call('list_profiles')).profiles, []);
  await close(); await open();
  await waitFor(async () => (await call('bootstrap')).storage === 'ready', 'OS-backed restart without app password');
  assert.equal((await call('bootstrap')).keyProtection, 'os_store'); assert.deepEqual((await call('list_profiles')).profiles, []);
  assert.equal(unrelated('lookup').trim(), 'Synthetic unrelated key');
  const metadata = await stat(fixture.database); assert.equal(metadata.mode & 0o777, 0o600);
  assert.equal((await readFile(fixture.database)).includes(Buffer.from('SQLite format 3\0')), false);
  assert.equal(await hash(application), evidence.applicationSha256);
  evidence.steps = { fallbackSetup: true, syntheticProfileDeleted: true, osStoreBecameAvailable: true, explicitGuiConsent: true, freshOnboardingWithoutAppPassword: true, unrelatedKeyPreserved: true, newOsProtectedSetup: true, genuineRestartWithoutAppPassword: true, noSavedProfiles: true, encryptedPrivateDatabase: true };
  await close(); evidence.result = 'passed';
} catch (error) { evidence.error = error.stack ?? String(error); process.exitCode = 1; }
finally {
  try {
    try { if (driver) await driver.stop(); }
    finally { if (fixture) { try { await stopSecretService(fixture); } finally { stopGroup(fixture.bus, 'SIGKILL'); await rm(fixture.root, { recursive: true, force: true }); } } }
  } catch (error) { evidence.cleanupError = String(error); evidence.result = 'failed'; process.exitCode = 1; }
  await writeFile(join(output, 'result.json'), JSON.stringify(evidence, null, 2) + '\n', { mode: 0o600 });
}
