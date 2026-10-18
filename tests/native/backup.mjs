import assert from 'node:assert/strict';
import { spawn, execFileSync } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { access, chmod, copyFile, lstat, mkdir, readFile, readdir, rm, writeFile } from 'node:fs/promises';
import { constants } from 'node:fs';
import { arch, platform, release, tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { NativeDriver, waitFor } from './webdriver.mjs';
import { CODEX_SENTINEL, createFixture, stopGroup } from './private-fixture.mjs';

const repository = resolve(process.env.OPENUSAGE_NATIVE_SOURCE_ROOT ?? join(dirname(fileURLToPath(import.meta.url)), '../..'));
const application = resolve(process.env.OPENUSAGE_NATIVE_BINARY ?? join(repository, 'target/debug/openusage-desktop'));
const verifier = process.env.OPENUSAGE_BACKUP_VERIFIER;
const output = resolve(process.env.OPENUSAGE_NATIVE_EVIDENCE ?? join(tmpdir(), `openusage-backup-evidence-${randomUUID()}`));
const backupPassword = 'OpenUsage synthetic backup password 2026';
const workspacePassphrase = 'OpenUsage synthetic workspace password 2026';
const evidence = {
  requirement: 'NATIVE-BACKUP-001', startedAt: new Date().toISOString(), result: 'failed',
  scope: 'Actual native backup export from synthetic records. Real Rust codec authentication and memory-only SQLite verification. No restore or live provider acceptance.',
  platform: { os: platform(), release: release(), architecture: arch() },
  expectedResult: ['Explicit records consent and matching backup password create one private encrypted archive.', 'Production codec authenticates the exported file and rejects wrong passwords and tampering.', 'Authenticated records include the current schema and appearance, without provider credentials.'],
  steps: [],
};
let fixture;
let driver;

const hash = async path => createHash('sha256').update(await readFile(path)).digest('hex');

async function heading(text) {
  await waitFor(async () => (await driver.execute('return document.querySelector("#startup-heading")?.textContent;')) === text, text);
}

async function invokeError(command, args, error) {
  assert.deepEqual(await driver.invoke(command, args), { ok: false, error });
}

async function clickButton(text) {
  const element = await driver.execute('return [...document.querySelectorAll("button")].find(button=>button.textContent.trim()===arguments[0]);', [text]);
  assert.ok(element, `Missing button: ${text}`);
  const id = element['element-6066-11e4-a52e-4f735466cecf'];
  await driver.request('POST', `/session/${driver.session}/element/${id}/click`, {});
}

async function authenticate(archive, staging) {
  const child = spawn(verifier, [archive, staging, 'dark'], { stdio: ['pipe', 'pipe', 'pipe'] });
  const stdout = [];
  const stderr = [];
  child.stdout.on('data', chunk => stdout.push(chunk));
  child.stderr.on('data', chunk => stderr.push(chunk));
  const exit = new Promise((done, reject) => {
    child.once('error', reject);
    child.once('exit', code => done(code));
  });
  child.stdin.end(backupPassword);
  const timer = setTimeout(() => child.kill('SIGKILL'), 30000);
  let code;
  try { code = await exit; } finally { clearTimeout(timer); }
  assert.equal(code, 0, `Rust backup verification failed: ${Buffer.concat(stderr).toString()}`);
  const result = JSON.parse(Buffer.concat(stdout).toString());
  assert.deepEqual(result, { authenticated: true, wrongPasswordRejected: true, tamperRejected: true, schemaVersion: 1, profiles: 0, plaintextDiskStaging: false });
  assert.deepEqual(await readdir(staging), []);
  return result;
}

await mkdir(output, { recursive: true, mode: 0o700 });
process.once('exit', () => { driver?.kill('SIGKILL'); stopGroup(fixture?.bus, 'SIGKILL'); });
try {
  assert.equal(platform(), 'linux');
  assert.ok(verifier, 'Set OPENUSAGE_BACKUP_VERIFIER to the test-side Rust verifier.');
  await access(verifier, constants.X_OK);
  evidence.application = { path: application, sha256: await hash(application), commit: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: repository, encoding: 'utf8' }).trim() };
  evidence.verifier = { path: verifier, sha256: await hash(verifier) };
  evidence.sourceDirtyPaths = execFileSync('git', ['status', '--porcelain'], { cwd: repository, encoding: 'utf8' }).trim().split('\n').filter(Boolean);
  evidence.sourceHashes = {};
  for (const path of ['Cargo.lock', 'apps/desktop/src-tauri/src/main.rs', 'apps/desktop/src-tauri/src/workspace.rs', 'apps/desktop/src-tauri/src/keyvault.rs', 'apps/desktop/src-tauri/src/backup.rs', 'apps/desktop/src/main.tsx', 'apps/desktop/src/contracts.ts', 'crates/openusage-storage/src/lib.rs', 'crates/openusage-import/src/lib.rs']) evidence.sourceHashes[path] = await hash(join(repository, path));
  fixture = await createFixture('backup');
  driver = new NativeDriver(application, fixture.environment, join(output, 'driver.log'));
  await driver.start();
  await heading('Set up your private workspace');
  await invokeError('create_backup', { password: backupPassword }, 'workspace_locked');
  for (const args of [{ password: backupPassword, path: '../unapproved' }, { password: [] }, {}]) await invokeError('create_backup', args, 'invalid_arguments');
  assert.deepEqual(await readdir(fixture.dirs.downloads), []);
  evidence.steps.push({ lockedWorkspaceAndMalformedArgumentsRejected: true, noExportBeforeWorkspaceUnlock: true });

  await driver.type('#workspace-passphrase', workspacePassphrase);
  await driver.type('#workspace-confirmation', workspacePassphrase);
  await driver.click('.unlock-form button[type="submit"]');
  await heading('Your workspace is ready');
  await driver.selectVisibleOption('#theme', 'Dark');
  await waitFor(async () => (await driver.execute('return document.documentElement.dataset.theme;')) === 'dark', 'current dark appearance');
  await invokeError('create_backup', { password: 'short' }, 'passphrase_too_short');
  await invokeError('create_backup', { password: 'é'.repeat(600) }, 'passphrase_too_long');
  assert.deepEqual(await readdir(fixture.dirs.downloads), []);

  await clickButton('Create a backup');
  await waitFor(async () => Boolean(await driver.execute('return document.querySelector(".backup-form");')), 'backup disclosure and consent form');
  assert.equal(await driver.execute('return document.querySelector("#backup-records").checked;'), false);
  await driver.type('#backup-password', backupPassword);
  await driver.type('#backup-confirmation', backupPassword);
  const consentRequired = await driver.execute('return document.querySelector(".backup-form button[type=submit]").disabled || document.querySelector("#backup-records").required;');
  assert.equal(consentRequired, true, 'Records consent must gate the user action.');
  assert.deepEqual(await readdir(fixture.dirs.downloads), []);
  await driver.click('#backup-records');
  assert.equal(await driver.execute('return document.querySelector("#backup-records").checked;'), true);
  await driver.click('.backup-form button[type="submit"]');
  await waitFor(async () => Boolean(await driver.execute('return document.querySelector(".backup-receipt")?.textContent.includes(".oubackup");')), 'completed native backup receipt', 30000);
  const destination = join(fixture.dirs.downloads, 'OpenUsage');
  const directoryMetadata = await lstat(destination);
  assert.ok(directoryMetadata.isDirectory() && !directoryMetadata.isSymbolicLink());
  assert.equal(directoryMetadata.mode & 0o777, 0o700);
  const entries = await readdir(destination);
  assert.equal(entries.length, 1);
  const fileName = entries[0];
  assert.match(fileName, /^openusage-backup-[A-Za-z0-9]{16}\.oubackup$/);
  const archive = join(destination, fileName);
  const archiveMetadata = await lstat(archive);
  assert.ok(archiveMetadata.isFile() && !archiveMetadata.isSymbolicLink());
  assert.equal(archiveMetadata.nlink, 1);
  assert.equal(archiveMetadata.mode & 0o777, 0o600);
  const receipt = await driver.execute('return document.querySelector(".backup-receipt").textContent;');
  assert.ok(receipt.includes(fileName));
  assert.ok(receipt.includes(destination));
  const bytes = await readFile(archive);
  assert.ok(bytes.length > 100);
  for (const marker of ['SQLite format 3\0', 'CREATE TABLE settings', CODEX_SENTINEL, backupPassword, workspacePassphrase]) assert.equal(bytes.includes(Buffer.from(marker)), false);
  const archiveHash = await hash(archive);
  const staging = join(fixture.root, 'verify-staging');
  await mkdir(staging, { mode: 0o700 });
  evidence.authentication = await authenticate(archive, staging);
  assert.equal(await hash(archive), archiveHash);
  await invokeError('create_backup', { password: backupPassword, directory: destination }, 'invalid_arguments');
  assert.deepEqual(await readdir(destination), [fileName]);
  assert.equal(await hash(archive), archiveHash);
  assert.equal(await driver.execute('return document.querySelector("#backup-password")?.value ?? "";'), '');
  assert.equal(await driver.execute('return document.querySelector("#backup-confirmation")?.value ?? "";'), '');
  evidence.steps.push({ explicitConsentAndMatchingPassword: true, privateArchiveAndDirectory: true, noPendingExport: true, latestWalThemePreserved: 'dark', manifestContainsOnlyRecordsSnapshot: true, malformedArgumentsPreserveExport: true, passwordInputsEmptyOrClosed: true });
  await copyFile(archive, join(output, fileName));
  await chmod(join(output, fileName), 0o600);
  evidence.archive = { fileName, sha256: archiveHash, bytes: bytes.length, directory: 'Synthetic XDG Downloads/OpenUsage' };
  await driver.execute('document.querySelector(".backup-receipt").scrollIntoView({block:"center"}); return true;');
  await driver.screenshot(join(output, 'backup-created-dark.png'));
  evidence.screenshot = 'backup-created-dark.png';
  assert.deepEqual(await readdir(fixture.dirs.codex), ['synthetic-fixture.txt']);
  assert.ok((await readFile(join(fixture.dirs.codex, 'synthetic-fixture.txt'), 'utf8')) === CODEX_SENTINEL);
  evidence.steps.push({ syntheticCodexHomeUnchanged: true });
  evidence.result = 'passed';
} catch (error) {
  evidence.error = error.message;
  process.exitCode = 1;
} finally {
  try { await driver?.stop(); } catch (error) { evidence.cleanupError = error.message; evidence.result = 'failed'; process.exitCode = 1; }
  if (fixture) {
    stopGroup(fixture.bus, 'SIGKILL');
    await rm(fixture.root, { recursive: true, force: true });
  }
  evidence.finishedAt = new Date().toISOString();
  await writeFile(join(output, 'result.json'), `${JSON.stringify(evidence, null, 2)}\n`, { mode: 0o600 });
  console.log(`${evidence.requirement}: ${evidence.result}. Evidence: ${output}`);
  if (evidence.error) console.error(evidence.error);
}
