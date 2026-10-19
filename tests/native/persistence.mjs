import assert from 'node:assert/strict';
import { execFile, execFileSync } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { lstat, mkdir, readFile, readdir, rename, rm, stat, writeFile } from 'node:fs/promises';
import { arch, platform, release, tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { promisify } from 'node:util';
import { fileURLToPath } from 'node:url';
import { NativeDriver, waitFor } from './webdriver.mjs';
import { CODEX_SENTINEL, createFixture, stopGroup } from './private-fixture.mjs';
import { startSecretService, stopSecretService } from './secret-service.mjs';

const run = promisify(execFile);
const repository = resolve(process.env.OPENUSAGE_NATIVE_SOURCE_ROOT ?? join(dirname(fileURLToPath(import.meta.url)), '../..'));
const application = resolve(process.env.OPENUSAGE_NATIVE_BINARY ?? join(repository, 'target/debug/openusage-desktop'));
const output = resolve(process.env.OPENUSAGE_NATIVE_EVIDENCE ?? join(tmpdir(), `openusage-persistence-evidence-${randomUUID()}`));
const sdk = process.env.OPENUSAGE_SECRET_SERVICE_SDK;
const secretTool = sdk ? join(resolve(sdk), 'runtime/usr/bin/secret-tool') : 'secret-tool';
const testService = 'app.openusage.desktop.native-test';
const keyAccount = 'primary-database-key-v1';
const evidence = {
  requirement: 'NATIVE-STORAGE-001', startedAt: new Date().toISOString(), result: 'failed',
  scope: 'Native Tauri application with real isolated GNOME Secret Service and synthetic local data. No provider or release-platform qualification.',
  platform: { os: platform(), release: release(), architecture: arch() },
  expectedResult: ['OS-protected SQLCipher workspace survives application and daemon restart.', 'Fallback requires the passphrase after restart and preserves data after a wrong passphrase.', 'Missing key or database shows recovery without reset.'],
  steps: [], screenshots: [],
};
let fixture;
let driver;
let driverNumber = 0;

async function openApp() {
  driver = new NativeDriver(application, fixture.environment, join(output, `driver-${++driverNumber}.log`));
  await driver.start();
  await waitFor(async () => (await driver.execute('return document.querySelector("#startup-heading")?.textContent;')) !== 'Checking local storage…', 'native startup state');
}

async function closeApp() {
  if (!driver) return;
  try { await driver.stop(); } finally { driver = undefined; }
}

async function disposeFixture() {
  if (!fixture) return;
  const errors = [];
  try { await closeApp(); } catch (error) { errors.push(error); }
  try { await stopSecretService(fixture); } catch (error) { errors.push(error); }
  stopGroup(fixture.bus, 'SIGKILL');
  try {
    await writeFile(join(output, `${fixture.name}-service.log`), Buffer.concat(fixture.logs), { mode: 0o600 });
    assert.deepEqual(await readdir(fixture.dirs.codex), ['synthetic-fixture.txt']);
    assert.ok((await readFile(join(fixture.dirs.codex, 'synthetic-fixture.txt'), 'utf8')) === CODEX_SENTINEL, 'The synthetic Codex sentinel must stay unchanged.');
  } catch (error) { errors.push(error); }
  try { await rm(fixture.root, { recursive: true, force: true }); } catch (error) { errors.push(error); }
  fixture = undefined;
  if (errors.length) throw errors[0];
}

async function state(expected) {
  const response = await driver.invoke('bootstrap');
  assert.equal(response.ok, true, 'Native bootstrap must succeed.');
  const value = response.value;
  const allowed = ['storage', 'theme', 'schemaVersion', 'activeProfiles', 'monitoring', 'provider', 'keyProtection', 'storageIssue'];
  assert.deepEqual(Object.keys(value).sort(), allowed.sort(), 'Bootstrap must return the exact approved metadata fields.');
  evidence.steps.push({ ...value });
  for (const [key, expectedValue] of Object.entries(expected)) assert.equal(value[key] ?? null, expectedValue, `Unexpected bootstrap ${key}.`);
  assert.equal(value.monitoring, 'stopped');
  assert.equal(value.provider, 'not_configured');
  return value;
}

async function heading(text) {
  await waitFor(async () => (await driver.execute('return document.querySelector("#startup-heading")?.textContent;')) === text, text);
}

async function ready(theme, protection) {
  await heading('Workspace ready');
  await state({ storage: 'ready', theme, keyProtection: protection, schemaVersion: 4, activeProfiles: 0, storageIssue: null });
  await waitFor(async () => driver.execute('const count=document.querySelector(".accounts-section > .section-heading .muted-tag");return Boolean(count&&count.textContent.trim()==="0 of 10 active"&&count.getBoundingClientRect().width>0&&count.getBoundingClientRect().height>0);'), 'visible Accounts active count');
  const summary = await driver.execute('const root=document.querySelector(".workspace-summary"),visible=node=>Boolean(node&&node.getBoundingClientRect().width>0&&node.getBoundingClientRect().height>0&&getComputedStyle(node).visibility!=="hidden");return {metrics:[...root.querySelectorAll(".metric")].map(node=>({label:node.querySelector(".metric-label").textContent.trim(),value:node.querySelector(".metric-value").textContent.trim(),visible:visible(node.querySelector(".metric-label"))&&visible(node.querySelector(".metric-value"))})),reason:root.querySelector(".workspace-summary-note").textContent.trim(),reasonVisible:visible(root.querySelector(".workspace-summary-note")),activeCount:document.querySelector(".accounts-section > .section-heading .muted-tag").textContent.trim()};');
  assert.deepEqual(summary, { metrics: [{ label: 'Quota remaining', value: 'Not available', visible: true }, { label: 'Token activity', value: 'Not available', visible: true }], reason: 'No accepted provider reading', reasonVisible: true, activeCount: '0 of 10 active' });
  evidence.steps.push({ readyWorkspaceSummary: summary });
}

async function darkTheme() {
  await driver.selectVisibleOption('#theme', 'Dark');
  await waitFor(async () => (await driver.execute('return document.documentElement.dataset.theme;')) === 'dark', 'persisted dark appearance');
}

async function screenshot(name) {
  await driver.execute('window.scrollTo(0,0); return true;');
  await driver.screenshot(join(output, name));
  evidence.screenshots.push(name);
}

const hash = async path => createHash('sha256').update(await readFile(path)).digest('hex');

async function encryptedFiles(protection) {
  const database = await readFile(fixture.database);
  const wal = await readFile(`${fixture.database}-wal`);
  assert.ok(database.length >= 4096 && wal.length > 32, 'The native database and active WAL must exist.');
  for (const bytes of [database, wal.subarray(32)]) {
    assert.equal(bytes.includes(Buffer.from('SQLite format 3\0')), false);
    assert.equal(bytes.includes(Buffer.from('CREATE TABLE settings')), false);
  }
  const mode = path => stat(path).then(value => value.mode & 0o777);
  assert.equal(await mode(dirname(fixture.database)), 0o700);
  assert.equal(await mode(fixture.database), 0o600);
  assert.equal(await mode(`${fixture.database}-wal`), 0o600);
  await run('python3', ['-c', 'import sqlite3,sys\ntry:\n c=sqlite3.connect("file:"+sys.argv[1]+"?mode=ro",uri=True); c.execute("SELECT count(*) FROM sqlite_schema").fetchone()\nexcept sqlite3.DatabaseError:\n sys.exit(0)\nsys.exit(1)', fixture.database], { timeout: 5000 });
  if (protection === 'passphrase') {
    const wrapped = join(dirname(fixture.database), 'database-key.v1');
    assert.equal((await stat(wrapped)).size, 112);
    assert.equal(await mode(wrapped), 0o600);
  }
  evidence.steps.push({ encryptedDatabaseAndWal: true, unkeyedSqliteRejected: true, privateModes: true, protection });
}

async function keyEntryExists(service) {
  const { stdout } = await run('gdbus', ['call', '--session', '--dest', 'org.freedesktop.secrets', '--object-path', '/org/freedesktop/secrets', '--method', 'org.freedesktop.Secret.Service.SearchItems', `{'service': '${service}', 'username': '${keyAccount}'}`], { env: fixture.environment, timeout: 5000 });
  return stdout.includes('/org/freedesktop/secrets/collection/');
}

await mkdir(output, { recursive: true, mode: 0o700 });
process.once('SIGINT', () => { driver?.kill('SIGTERM'); fixture?.daemon?.kill(); stopGroup(fixture?.bus); });
process.once('SIGTERM', () => { driver?.kill('SIGTERM'); fixture?.daemon?.kill(); stopGroup(fixture?.bus); });
process.once('exit', () => { driver?.kill('SIGKILL'); fixture?.daemon?.kill('SIGKILL'); stopGroup(fixture?.bus, 'SIGKILL'); });
try {
  assert.equal(platform(), 'linux');
  evidence.application = { path: application, sha256: await hash(application), commit: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: repository, encoding: 'utf8' }).trim() };
  evidence.sourceDirtyPaths = execFileSync('git', ['status', '--porcelain'], { cwd: repository, encoding: 'utf8' }).trim().split('\n').filter(Boolean);
  evidence.sourceHashes = {};
  for (const path of ['Cargo.toml', 'Cargo.lock', 'apps/desktop/src-tauri/Cargo.toml', 'apps/desktop/src-tauri/tauri.conf.json', 'apps/desktop/src-tauri/build.rs', 'apps/desktop/src-tauri/src/main.rs', 'apps/desktop/src-tauri/src/workspace.rs', 'apps/desktop/src-tauri/src/keyvault.rs', 'apps/desktop/src/contracts.ts', 'apps/desktop/src/main.tsx', 'crates/openusage-storage/src/lib.rs', 'crates/openusage-storage/src/keywrap.rs', 'crates/openusage-storage/src/schema.sql']) {
    try { evidence.sourceHashes[path] = await hash(join(repository, path)); } catch (error) { if (error.code !== 'ENOENT') throw error; }
  }
  fixture = await createFixture('os-store');
  await startSecretService(fixture);
  await openApp();
  await state({ storage: 'setup_required', keyProtection: 'os_store', activeProfiles: null });
  for (const [command, args] of [
    ['setup_workspace', {}], ['setup_workspace', { passphrase: null, unexpected: true }],
    ['unlock_workspace', { passphrase: [] }], ['set_theme', { theme: 'ultraviolet' }],
  ]) {
    assert.deepEqual(await driver.invoke(command, args), { ok: false, error: 'invalid_arguments' });
  }
  await assert.rejects(stat(fixture.database), { code: 'ENOENT' });
  assert.equal(await keyEntryExists(testService), false);
  evidence.steps.push({ malformedNativeCommandsRejectedBeforeDatabaseOrKeyCreation: true });
  await driver.click('.startup-content .primary-button');
  await ready('system', 'os_store');
  assert.equal(await keyEntryExists(testService), true, 'The native-test feature must use its test credential namespace.');
  assert.equal(await keyEntryExists('app.openusage.desktop'), false, 'The private test must not use the production credential namespace.');
  await darkTheme();
  await ready('dark', 'os_store');
  await encryptedFiles('os_store');
  await screenshot('os-created-dark.png');
  await closeApp();
  await stopSecretService(fixture);
  await startSecretService(fixture);
  await openApp();
  await ready('dark', 'os_store');
  await screenshot('os-restarted-dark.png');
  await closeApp();
  const savedHash = await hash(fixture.database);
  await stopSecretService(fixture);
  await openApp();
  await state({ storage: 'locked', storageIssue: 'key_store_unavailable' });
  assert.equal(await hash(fixture.database), savedHash);
  await closeApp();
  await startSecretService(fixture);
  const preserved = `${fixture.database}.preserved`;
  await rename(fixture.database, preserved);
  await openApp();
  await state({ storage: 'recovery_required', storageIssue: 'missing_database' });
  await heading('Your local data needs recovery');
  const recoveryActions = await driver.execute('return [...document.querySelectorAll(".startup-content button")].map(button=>button.textContent.trim());');
  assert.ok(recoveryActions.includes('Check workspace'));
  assert.equal(recoveryActions.some(text => /reset|delete|create|initialize|set up/i.test(text)), false);
  assert.equal(await hash(preserved), savedHash);
  await assert.rejects(stat(fixture.database), { code: 'ENOENT' });
  await closeApp();
  await rename(preserved, fixture.database);
  const toolEnvironment = { ...fixture.environment };
  if (sdk) toolEnvironment.LD_LIBRARY_PATH = join(resolve(sdk), 'runtime/usr/lib/x86_64-linux-gnu');
  await run(secretTool, ['clear', 'service', testService, 'username', keyAccount], { env: toolEnvironment, timeout: 5000 });
  await openApp();
  await state({ storage: 'recovery_required', storageIssue: 'missing_key' });
  await heading('Your local data needs recovery');
  assert.equal(await hash(fixture.database), savedHash);
  await screenshot('os-missing-key-recovery.png');
  await disposeFixture();

  fixture = await createFixture('passphrase');
  await openApp();
  await state({ storage: 'setup_required', keyProtection: 'passphrase_required', storageIssue: 'key_store_unavailable' });
  const passphrase = 'OpenUsage synthetic passphrase 2026';
  await driver.type('#workspace-passphrase', passphrase);
  await driver.type('#workspace-confirmation', passphrase);
  await driver.click('.unlock-form button[type="submit"]');
  await ready('system', 'passphrase');
  await darkTheme();
  await ready('dark', 'passphrase');
  await encryptedFiles('passphrase');
  await closeApp();
  const lockedHash = await hash(fixture.database);
  const wrappedPath = join(dirname(fixture.database), 'database-key.v1');
  const wrappedHash = await hash(wrappedPath);
  await openApp();
  await state({ storage: 'locked', keyProtection: 'passphrase' });
  await heading('Unlock your local workspace');
  await driver.type('#workspace-passphrase', 'Wrong synthetic passphrase');
  await driver.click('.unlock-form button[type="submit"]');
  await waitFor(async () => Boolean(await driver.execute('return document.querySelector("[role=alert]")?.textContent.includes("passphrase could not unlock");')), 'wrong passphrase guidance');
  await state({ storage: 'locked', keyProtection: 'passphrase' });
  assert.equal(await hash(fixture.database), lockedHash);
  assert.equal(await hash(wrappedPath), wrappedHash);
  assert.equal(await driver.execute('return document.querySelector("#workspace-passphrase").value;'), '');
  await driver.type('#workspace-passphrase', passphrase);
  await driver.click('.unlock-form button[type="submit"]');
  await ready('dark', 'passphrase');
  await screenshot('passphrase-unlocked-dark.png');
  await disposeFixture();
  evidence.result = 'passed';
} catch (error) {
  evidence.error = error.message;
  if (fixture) {
    evidence.failedFixture = fixture.root;
    evidence.dataModes = [];
    const inspect = async (path, depth) => {
      const metadata = await lstat(path);
      evidence.dataModes.push({ path, mode: (metadata.mode & 0o777).toString(8), uid: metadata.uid, directory: metadata.isDirectory(), symlink: metadata.isSymbolicLink() });
      if (depth > 0 && metadata.isDirectory()) for (const name of await readdir(path)) await inspect(join(path, name), depth - 1);
    };
    try { await inspect(fixture.dirs.data, 2); } catch (inspectionError) { evidence.inspectionError = inspectionError.message; }
    try { evidence.filesystem = (await run('stat', ['-f', '-c', '%T %t', fixture.dirs.data])).stdout.trim(); } catch {}
  }
  process.exitCode = 1;
} finally {
  try { await disposeFixture(); } catch (error) { evidence.cleanupError = error.message; evidence.result = 'failed'; process.exitCode = 1; }
  evidence.finishedAt = new Date().toISOString();
  await writeFile(join(output, 'result.json'), `${JSON.stringify(evidence, null, 2)}\n`, { mode: 0o600 });
  console.log(`${evidence.requirement}: ${evidence.result}. Evidence: ${output}`);
  if (evidence.error) console.error(evidence.error);
}
