import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { chmod, copyFile, link, lstat, mkdir, readFile, readdir, rm, stat, symlink, writeFile } from 'node:fs/promises';
import { arch, platform, release, tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { CODEX_SENTINEL, createFixture, stopGroup } from './private-fixture.mjs';
import { NativeDriver, waitFor } from './webdriver.mjs';

const harness = dirname(fileURLToPath(import.meta.url));
const repository = resolve(process.env.OPENUSAGE_NATIVE_SOURCE_ROOT ?? join(harness, '../..'));
const application = resolve(process.env.OPENUSAGE_NATIVE_BINARY ?? join(repository, 'target/debug/openusage-desktop'));
const output = resolve(process.env.OPENUSAGE_NATIVE_EVIDENCE ?? join(tmpdir(), `openusage-backup-inspection-evidence-${randomUUID()}`));
const password = 'OpenUsage synthetic inspection password 2026';
const passphrase = 'OpenUsage synthetic inspection workspace 2026';
const missingName = 'openusage-backup-0000000000000000.oubackup';
const evidence = {
  requirement: 'NATIVE-BACKUP-INSPECTION-001', startedAt: new Date().toISOString(), result: 'failed',
  scope: 'Actual native archive listing and authentication from an exported synthetic workspace. GUI wrong-password and successful inspection; direct production IPC fault checks. Database records remain not checked. No restore, provider login, or cross-platform acceptance.',
  platform: { os: platform(), release: release(), architecture: arch() }, steps: [], screenshots: [],
};
const hash = async path => createHash('sha256').update(await readFile(path)).digest('hex');
let fixture;
let driver;

async function rejected(command, args, error) {
  assert.deepEqual(await driver.invoke(command, args), { ok: false, error });
}

async function clickButton(text) {
  const element = await driver.execute('return [...document.querySelectorAll("button")].find(node=>node.textContent.trim()===arguments[0]);', [text]);
  assert.ok(element, `Missing native button: ${text}`);
  await driver.request('POST', `/session/${driver.session}/element/${element['element-6066-11e4-a52e-4f735466cecf']}/click`, {});
}

async function list() {
  const result = await driver.invoke('list_backups');
  assert.equal(result.ok, true);
  assert.deepEqual(Object.keys(result.value).sort(), ['backups', 'truncated']);
  assert.equal(typeof result.value.truncated, 'boolean');
  for (const item of result.value.backups) {
    assert.deepEqual(Object.keys(item).sort(), ['fileBytes', 'fileName']);
    assert.match(item.fileName, /^openusage-backup-[A-Za-z0-9]{16}\.oubackup$/);
    assert.match(item.fileBytes, /^(0|[1-9][0-9]*)$/);
  }
  return result.value;
}

async function workspaceHashes() {
  const result = {};
  for (const path of [fixture.database, `${fixture.database}-wal`, join(dirname(fixture.database), 'database-key.v1')]) result[path] = await hash(path);
  return result;
}

const newName = () => `openusage-backup-${randomUUID().replaceAll('-', '').slice(0, 16)}.oubackup`;

async function setTheme(theme, mode = 'pointer') {
  if ((await driver.execute('return document.querySelector("#theme").value;')) === theme) return;
  const keys = async values => driver.request('POST', `/session/${driver.session}/actions`, {
    actions: [{ type: 'key', id: 'inspection-keyboard', actions: values.flatMap(value => [{ type: 'keyDown', value }, { type: 'keyUp', value }]) }],
  });
  await driver.click('#theme');
  await keys(['\uE00C']);
  assert.equal(await driver.execute('return document.activeElement?.id;'), 'theme');
  if (mode === 'keyboard') {
    assert.equal(theme, 'light');
    assert.equal(await driver.execute('return document.querySelector("#theme").value;'), 'dark');
    await keys(['\uE013', '\uE007']);
  } else await driver.click(`#theme option[value="${theme}"]`);
  await waitFor(async () => (await driver.execute('return !document.querySelector("#theme").disabled&&document.documentElement.dataset.theme===arguments[0];', [theme])) === true, `${mode} appearance save`);
  await driver.request('POST', `/session/${driver.session}/execute/async`, { script: 'const done=arguments[arguments.length-1];requestAnimationFrame(()=>requestAnimationFrame(()=>done(true)));', args: [] });
  assert.equal(await driver.execute('return document.activeElement?.id;'), 'theme', 'Appearance must retain focus after its native save.');
  assert.equal((await driver.invoke('bootstrap')).value?.theme, theme);
  (evidence.themeFocus ??= []).push({ mode, theme, focusAfterSave: 'theme', nativeSaved: true });
}

async function captureReceipt(name, theme, width, height) {
  await setTheme(theme, theme === 'light' ? 'keyboard' : 'pointer');
  for (let attempt = 0; attempt < 3; attempt++) {
    const viewport = await driver.execute('return {width:innerWidth,height:innerHeight};');
    if (viewport.width === width && viewport.height === height) break;
    const rect = await driver.request('GET', `/session/${driver.session}/window/rect`);
    await driver.request('POST', `/session/${driver.session}/window/rect`, { width: rect.width + width - viewport.width, height: rect.height + height - viewport.height });
    await driver.request('POST', `/session/${driver.session}/execute/async`, { script: 'const done=arguments[arguments.length-1];requestAnimationFrame(()=>requestAnimationFrame(()=>done(true)));', args: [] });
  }
  await driver.execute('document.querySelector(".backup-inspection-receipt").scrollIntoView({block:"center"});return true;');
  const view = await driver.execute('return {width:innerWidth,height:innerHeight,scrollWidth:document.scrollingElement.scrollWidth,overflows:[...document.querySelectorAll(".backup-inspection-form *, .backup-inspection-receipt *")].filter(node=>{const rect=node.getBoundingClientRect();return rect.width>1&&rect.height>1&&(rect.left < -1||rect.right>innerWidth+1);}).map(node=>({tag:node.tagName,className:node.className}))};');
  assert.deepEqual({ width: view.width, height: view.height }, { width, height });
  assert.ok(view.scrollWidth <= width + 1);
  assert.deepEqual(view.overflows, []);
  await driver.screenshot(join(output, name));
  evidence.screenshots.push(name);
  (evidence.views ??= []).push({ screenshot: name, theme, ...view });
}

await mkdir(output, { recursive: true, mode: 0o700 });
process.once('exit', () => { driver?.kill('SIGKILL'); stopGroup(fixture?.bus, 'SIGKILL'); });
try {
  assert.equal(platform(), 'linux');
  evidence.application = { path: application, sha256: await hash(application), commit: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: repository, encoding: 'utf8' }).trim() };
  evidence.sourceDirtyPaths = execFileSync('git', ['status', '--porcelain'], { cwd: repository, encoding: 'utf8' }).trim().split('\n').filter(Boolean);
  evidence.sourceHashes = {};
  const sources = execFileSync('rg', ['--files', 'apps/desktop/src', 'apps/desktop/src-tauri/src', 'crates/openusage-storage/src', 'crates/openusage-import/src'], { cwd: repository, encoding: 'utf8' }).trim().split('\n');
  for (const path of [...sources, 'Cargo.lock', 'apps/desktop/src-tauri/Cargo.toml', 'apps/desktop/src-tauri/build.rs', 'apps/desktop/src-tauri/capabilities/main-window.json']) evidence.sourceHashes[path] = await hash(join(repository, path));
  evidence.harnessHashes = {};
  for (const path of ['backup-inspection.mjs', 'private-fixture.mjs', 'webdriver.mjs']) evidence.harnessHashes[path] = await hash(join(harness, path));
  fixture = await createFixture('backup-inspection');
  driver = new NativeDriver(application, fixture.environment, join(output, 'driver.log'));
  await driver.start();
  await waitFor(async () => Boolean(await driver.execute('return document.querySelector("#workspace-passphrase");')), 'fallback setup form');
  await rejected('list_backups', {}, 'workspace_locked');
  await rejected('inspect_backup', { fileName: missingName, password }, 'workspace_locked');
  for (const command of ['list_backups', 'inspect_backup']) await rejected(command, [], 'invalid_arguments');
  await rejected('list_backups', { unexpected: true }, 'invalid_arguments');
  for (const args of [{ fileName: '../unsafe.oubackup', password }, { fileName: missingName, password: [] }, { fileName: missingName, password, directory: '../unsafe' }]) await rejected('inspect_backup', args, 'invalid_arguments');
  assert.deepEqual(await readdir(fixture.dirs.downloads), []);
  await driver.type('#workspace-passphrase', passphrase);
  await driver.type('#workspace-confirmation', passphrase);
  await driver.click('.unlock-form button[type="submit"]');
  await waitFor(async () => (await driver.invoke('bootstrap')).value?.storage === 'ready', 'private workspace setup');
  assert.deepEqual(await list(), { backups: [], truncated: false });
  await rejected('inspect_backup', { fileName: missingName, password: 'x' }, 'backup_not_found');
  await rejected('inspect_backup', { fileName: missingName, password: '' }, 'invalid_passphrase');
  await rejected('inspect_backup', { fileName: missingName, password: 'é'.repeat(600) }, 'passphrase_too_long');
  assert.deepEqual(await readdir(fixture.dirs.downloads), [], 'Listing or inspecting a missing archive must not create Downloads/OpenUsage.');
  evidence.steps.push({ lockedAndMalformedCommandsRejected: true, missingCatalogDoesNotCreateDirectory: true, singleCharacterPasswordReachesMissingFileCheck: true });

  await setTheme('dark');
  await clickButton('Inspect a backup');
  await waitFor(async () => Boolean(await driver.execute('return document.querySelector(".backup-inspection-empty");')), 'native empty backup catalog');
  assert.equal(await driver.execute('return Boolean(document.querySelector(".backup-inspection-form"));'), false);
  assert.deepEqual(await readdir(fixture.dirs.downloads), []);
  await driver.execute('document.querySelector(".backup-inspection-empty").scrollIntoView({block:"center"});return true;');
  await driver.screenshot(join(output, 'inspection-empty-catalog-dark.png'));
  evidence.screenshots.push('inspection-empty-catalog-dark.png');
  await clickButton('Create a backup');
  await waitFor(async () => (await driver.execute('return !document.querySelector(".backup-inspection-empty")&&Boolean(document.querySelector(".backup-form"));')) === true, 'exclusive backup creation form');
  await driver.type('#backup-password', password);
  await driver.type('#backup-confirmation', password);
  await driver.click('#backup-records');
  await driver.click('.backup-form button[type="submit"]');
  await waitFor(async () => Boolean(await driver.execute('return document.querySelector(".backup-receipt");')), 'native encrypted export', 30000);
  const directory = join(fixture.dirs.downloads, 'OpenUsage');
  const names = await readdir(directory);
  assert.equal(names.length, 1);
  const fileName = names[0];
  const archive = join(directory, fileName);
  const archiveHash = await hash(archive);
  const catalog = await list();
  assert.deepEqual(catalog, { backups: [{ fileName, fileBytes: (await stat(archive, { bigint: true })).size.toString() }], truncated: false });
  const before = await workspaceHashes();
  await rejected('inspect_backup', { fileName, password: 'x' }, 'backup_authentication_failed');

  await clickButton('Inspect a backup');
  await waitFor(async () => Boolean(await driver.execute('return document.querySelector("#backup-file option[value="+JSON.stringify(arguments[0])+"]");', [fileName])), 'export listed in the native inspection form');
  await driver.click(`#backup-file option[value="${fileName}"]`);
  await driver.type('#backup-inspection-password', 'x');
  await driver.click('.backup-inspection-form button[type="submit"]');
  await waitFor(async () => Boolean(await driver.execute('return document.querySelector("#backup-inspection-error");')), 'wrong-password inspection guidance', 30000);
  await waitFor(async () => (await driver.execute('return document.activeElement?.id;')) === 'backup-inspection-password', 'wrong inspection password focus');
  const failedView = await driver.execute('const field=document.querySelector("#backup-inspection-password"),error=document.querySelector("#backup-inspection-error");return {empty:field.value==="",invalid:field.getAttribute("aria-invalid"),fileInvalid:document.querySelector("#backup-file").getAttribute("aria-invalid"),describedBy:field.getAttribute("aria-describedby"),errorId:error.id,message:error.textContent,errorColor:getComputedStyle(error).color,invalidBorderColor:getComputedStyle(field).borderTopColor};');
  assert.equal(failedView.empty, true);
  assert.equal(failedView.invalid, 'true');
  assert.notEqual(failedView.fileInvalid, 'true');
  assert.ok(failedView.describedBy?.split(' ').includes(failedView.errorId));
  assert.match(failedView.message, /password|authenticate/i);
  assert.equal(failedView.errorColor, failedView.invalidBorderColor);
  await driver.execute('document.querySelector("#backup-inspection-error").scrollIntoView({block:"center"});return true;');
  await driver.screenshot(join(output, 'inspection-wrong-password-dark.png'));
  evidence.screenshots.push('inspection-wrong-password-dark.png');
  await driver.type('#backup-inspection-password', password);
  await driver.click('.backup-inspection-form button[type="submit"]');
  await waitFor(async () => Boolean(await driver.execute('return document.querySelector(".backup-inspection-receipt");')), 'native authenticated inspection receipt', 30000);
  await waitFor(async () => Boolean(await driver.execute('return document.activeElement?.matches(".backup-inspection-receipt");')), 'focus after successful archive inspection');
  const inspected = await driver.invoke('inspect_backup', { fileName, password });
  assert.equal(inspected.ok, true);
  const metadata = inspected.value;
  assert.deepEqual(Object.keys(metadata).sort(), ['archiveVersion', 'databaseValidation', 'entries', 'entryCount', 'fileName', 'manifestSchemaVersion', 'restoreAvailable', 'totalBytes']);
  assert.equal(metadata.fileName, fileName);
  assert.equal(metadata.archiveVersion, 1);
  assert.equal(metadata.manifestSchemaVersion, 1);
  assert.equal(metadata.entryCount, 1);
  assert.equal(metadata.databaseValidation, 'not_checked');
  assert.equal(metadata.restoreAvailable, false);
  assert.deepEqual(metadata.entries.map(entry => Object.keys(entry).sort()), [['bytes', 'name']]);
  assert.equal(metadata.entries[0].name, 'records.sqlite');
  assert.match(metadata.entries[0].bytes, /^[1-9][0-9]*$/);
  assert.equal(metadata.totalBytes, metadata.entries[0].bytes);
  const receipt = await driver.execute('return document.querySelector(".backup-inspection-receipt").textContent;');
  assert.ok(receipt.includes(fileName));
  assert.match(receipt, /archive authenticated/i);
  assert.match(receipt, /not (?:been )?checked/i);
  assert.match(receipt, /restore.*not available/i);
  assert.equal(await driver.execute('return [...document.querySelectorAll("#backup-inspection button, #backup-inspection a")].some(node=>/restore|replace/i.test(node.textContent));'), false);
  assert.equal(await driver.execute('return document.querySelector("#backup-inspection-password")?.value??"";'), '');
  evidence.inspection = metadata;
  evidence.wrongPasswordView = failedView;
  await driver.execute('document.querySelector(".backup-inspection-receipt").scrollIntoView({block:"center"});return true;');
  await driver.screenshot(join(output, 'inspection-authenticated-dark.png'));
  evidence.screenshots.push('inspection-authenticated-dark.png');
  await driver.type('#backup-inspection-password', 'Synthetic disclosure switch only');
  await clickButton('Create a backup');
  await waitFor(async () => (await driver.execute('return !document.querySelector("#backup-inspection-password")&&Boolean(document.querySelector(".backup-form"));')) === true, 'inspection password removed when switching backup action');
  assert.deepEqual(await driver.execute('return [...document.querySelectorAll(".backup-form input[type=password]")].map(node=>node.value);'), ['', '']);
  await clickButton('Inspect a backup');
  await waitFor(async () => Boolean(await driver.execute('return document.querySelector("#backup-inspection-password");')), 'inspection form after disclosure switch');
  assert.equal(await driver.execute('return document.querySelector("#backup-inspection-password").value;'), '');
  evidence.steps.push({ backupDisclosureSwitchClearsPassword: true, inspectionOffersNoRestoreAction: true });

  const bytes = await readFile(archive);
  const fixtures = [];
  async function badArchive(name, contents, error) {
    const path = join(directory, name);
    await writeFile(path, contents, { mode: 0o600, flag: 'wx' });
    const digest = await hash(path);
    await rejected('inspect_backup', { fileName: name, password }, error);
    assert.equal(await hash(path), digest);
    fixtures.push({ kind: error, sourcePreserved: true });
  }
  const tampered = Buffer.from(bytes); tampered[tampered.length - 1] ^= 1;
  await badArchive(newName(), tampered, 'backup_authentication_failed');
  const truncatedName = newName();
  await badArchive(truncatedName, bytes.subarray(0, 32), 'invalid_backup');
  await badArchive(newName(), Buffer.concat([bytes, Buffer.from([0])]), 'invalid_backup');
  const external = join(fixture.root, 'external-synthetic-archive');
  await writeFile(external, bytes, { mode: 0o600, flag: 'wx' });
  const externalHash = await hash(external);
  const linked = newName(); await symlink(external, join(directory, linked));
  await rejected('inspect_backup', { fileName: linked, password }, 'backup_unavailable');
  const hardlinked = newName(); await link(external, join(directory, hardlinked));
  await rejected('inspect_backup', { fileName: hardlinked, password }, 'backup_unavailable');
  const shared = newName(); await writeFile(join(directory, shared), bytes, { mode: 0o600, flag: 'wx' });
  await chmod(join(directory, shared), 0o644);
  await rejected('inspect_backup', { fileName: shared, password }, 'backup_unavailable');
  assert.equal((await lstat(join(directory, shared))).mode & 0o777, 0o644);
  const safeCatalog = await list();
  assert.equal(safeCatalog.backups.some(item => [linked, hardlinked, shared].includes(item.fileName)), false);
  assert.ok(safeCatalog.backups.some(item => item.fileName === fileName));
  await clickButton('Refresh backups');
  await waitFor(async () => Boolean(await driver.execute('return [...document.querySelectorAll("#backup-file option")].some(option=>option.value===arguments[0]);', [truncatedName])), 'unverified truncated file in refreshed catalog');
  await driver.click(`#backup-file option[value="${truncatedName}"]`);
  await driver.type('#backup-inspection-password', password);
  await driver.click('.backup-inspection-form button[type="submit"]');
  await waitFor(async () => (await driver.execute('return document.activeElement?.id;')) === 'backup-inspection-error', 'structural archive error focus');
  const structuralView = await driver.execute('const field=document.querySelector("#backup-inspection-password"),error=document.querySelector("#backup-inspection-error");return {empty:field.value==="",invalid:field.getAttribute("aria-invalid"),message:error?.textContent};');
  assert.equal(structuralView.empty, true);
  assert.notEqual(structuralView.invalid, 'true');
  assert.match(structuralView.message, /archive|backup/i);
  evidence.structuralFailureView = structuralView;
  await driver.execute('document.querySelector("#backup-inspection-error").scrollIntoView({block:"center"});return true;');
  await driver.screenshot(join(output, 'inspection-structural-error-dark.png'));
  evidence.screenshots.push('inspection-structural-error-dark.png');
  await driver.click(`#backup-file option[value="${fileName}"]`);
  await driver.type('#backup-inspection-password', password);
  await driver.click('.backup-inspection-form button[type="submit"]');
  await waitFor(async () => Boolean(await driver.execute('return document.querySelector(".backup-inspection-receipt")?.textContent.includes(arguments[0]);', [fileName])), 'healthy archive authenticated after structural failure', 30000);
  assert.equal(await hash(external), externalHash);
  assert.equal(await hash(archive), archiveHash);
  assert.deepEqual(await workspaceHashes(), before, 'Read-only inspection must preserve the encrypted workspace and wrapped key.');
  const staging = join(dirname(fixture.database), 'backup-inspection');
  assert.equal((await lstat(staging)).mode & 0o777, 0o700);
  assert.deepEqual(await readdir(staging), []);
  evidence.steps.push({ guiEmptyCatalog: true, guiWrongPasswordRejectedAndCleared: true, guiArchiveAuthenticated: true, guiStructuralFailureFocusWithoutPasswordInvalid: true, databaseValidationNotChecked: true, restoreAvailable: false, corruptedArchiveFixtures: fixtures, unsafeFileFixtures: ['Synthetic symlink.', 'Synthetic hardlink.', 'Synthetic file with group/other read permission.'], unsafeFilesExcludedFromCatalog: true, sourceArchiveAndWorkspacePreserved: true, workspacePreservationScope: 'Workspace hashes match across inspection and fault checks. Later theme selection is a separate GUI write.', noRetainedInspectionStaging: true });
  await copyFile(archive, join(output, fileName)); await chmod(join(output, fileName), 0o600);
  evidence.archive = { fileName, sha256: archiveHash, fileBytes: (await stat(archive, { bigint: true })).size.toString() };
  assert.deepEqual(await readdir(fixture.dirs.codex), ['synthetic-fixture.txt']);
  assert.equal(await readFile(join(fixture.dirs.codex, 'synthetic-fixture.txt'), 'utf8'), CODEX_SENTINEL);
  evidence.steps.push({ syntheticCodexHomeUnchanged: true });
  await captureReceipt('inspection-authenticated-dark-360.png', 'dark', 360, 800);
  await captureReceipt('inspection-authenticated-light-360.png', 'light', 360, 800);
  assert.equal(await hash(application), evidence.application.sha256);
  for (const [path, digest] of Object.entries(evidence.sourceHashes)) assert.equal(await hash(join(repository, path)), digest, `Source changed during inspection: ${path}`);
  for (const [path, digest] of Object.entries(evidence.harnessHashes)) assert.equal(await hash(join(harness, path)), digest, `Harness changed during inspection: ${path}`);
  evidence.result = 'passed';
} catch (error) {
  evidence.error = error.message;
  process.exitCode = 1;
  if (driver?.session) { try { await driver.screenshot(join(output, 'failure.png')); evidence.screenshots.push('failure.png'); } catch { /* Preserve the first failure. */ } }
} finally {
  try { await driver?.stop(); } catch (error) { evidence.cleanupError = error.message; evidence.result = 'failed'; process.exitCode = 1; }
  if (fixture) { stopGroup(fixture.bus, 'SIGKILL'); await rm(fixture.root, { recursive: true, force: true }); }
  evidence.finishedAt = new Date().toISOString();
  await writeFile(join(output, 'result.json'), `${JSON.stringify(evidence, null, 2)}\n`, { mode: 0o600 });
  console.log(`${evidence.requirement}: ${evidence.result}. Evidence: ${output}`);
  if (evidence.error) console.error(evidence.error);
}
