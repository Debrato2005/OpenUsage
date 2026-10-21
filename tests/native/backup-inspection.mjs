import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { chmod, copyFile, link, lstat, mkdir, readFile, readdir, readlink, rm, stat, symlink, writeFile } from 'node:fs/promises';
import { arch, platform, release, tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { CODEX_SENTINEL, createFixture, stopGroup } from './private-fixture.mjs';
import { NativeDriver, waitFor } from './webdriver.mjs';

const harness = dirname(fileURLToPath(import.meta.url));
const repository = resolve(process.env.OPENUSAGE_NATIVE_SOURCE_ROOT ?? join(harness, '../..'));
const application = resolve(process.env.OPENUSAGE_NATIVE_BINARY ?? join(repository, 'target/debug/openusage-desktop'));
const generator = resolve(process.env.OPENUSAGE_BACKUP_FIXTURE_GENERATOR ?? join(harness, 'backup-verifier/target/debug/openusage-native-backup-fixtures'));
const output = resolve(process.env.OPENUSAGE_NATIVE_EVIDENCE ?? join(tmpdir(), `openusage-backup-inspection-evidence-${randomUUID()}`));
const password = 'OpenUsage synthetic inspection password 2026';
const passphrase = 'OpenUsage synthetic inspection workspace 2026';
const missingName = 'openusage-backup-0000000000000000.oubackup';
const expectedSchemaVersion = 8;
const evidence = {
  requirement: 'NATIVE-BACKUP-CONTENT-001', startedAt: new Date().toISOString(), result: 'failed',
  scope: 'Actual native archive authentication and read-only content checks from a GUI-exported synthetic workspace. GUI profiles, wrong-password handling, valid counts, and authenticated invalid/unsupported fixtures. Direct production IPC fault checks. No restore, provider login, or cross-platform acceptance.',
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

const detailsSelector = '.backup-inspection-receipt [data-slot="collapsible-trigger"]';
const advancedFields = ['Archive version', 'Schema version', 'Files', 'Total size'];
async function receiptView() {
  return driver.execute('const receipt=document.querySelector(".backup-inspection-receipt"),password=document.querySelector("#backup-inspection-password"),status=receipt.querySelector(".backup-content-status"),trigger=receipt.querySelector("[data-slot=collapsible-trigger]"),panel=receipt.querySelector("[data-slot=collapsible-panel]");return {text:receipt.textContent,status:status.textContent,statusVisible:Boolean(status.getClientRects().length),statusFirst:receipt.firstElementChild===status,metadata:Object.fromEntries([...receipt.querySelectorAll("dt")].map(node=>[node.textContent,node.nextElementSibling?.textContent])),password:password.value,passwordInvalid:password.getAttribute("aria-invalid"),details:{name:trigger.textContent.trim(),enabled:!trigger.disabled,visible:Boolean(trigger.getClientRects().length),expanded:trigger.getAttribute("aria-expanded"),controlsMatch:!panel||trigger.getAttribute("aria-controls")===panel.id,panelPresent:Boolean(panel),panelVisible:Boolean(panel?.getClientRects().length),passwordFields:panel?.querySelectorAll("input,form").length??0},previewVisible:[...document.querySelectorAll(".restore-preparation button")].some(node=>node.textContent.trim()==="Preview restore"&&node.getClientRects().length)};');
}
async function waitDetails(open) {
  await waitFor(async () => { const view = await receiptView(); return view.details.expanded === String(open) && view.details.panelPresent === open && view.details.panelVisible === open; }, `${open ? 'expanded' : 'closed'} archive details`);
}
async function readAdvancedDetails(expectedPreview, keyboard = false) {
  const before = await receiptView(), stored = await workspaceHashes();
  assert.deepEqual(before.details, { name: 'Details', enabled: true, visible: true, expanded: 'false', controlsMatch: true, panelPresent: false, panelVisible: false, passwordFields: 0 });
  assert.equal(before.statusVisible, true); assert.equal(before.statusFirst, true);
  assert.equal(before.previewVisible, expectedPreview); assert.equal(before.password, '');
  for (const field of advancedFields) assert.equal(Object.hasOwn(before.metadata, field), false);
  await driver.click(detailsSelector); await waitDetails(true);
  if (keyboard) {
    const key = value => driver.request('POST', `/session/${driver.session}/actions`, { actions: [{ type: 'key', id: 'archive-details-keyboard', actions: [{ type: 'keyDown', value }, { type: 'keyUp', value }] }] });
    await focusOwnedWindow();
    assert.equal(await driver.execute('return document.activeElement?.matches(arguments[0]);', [detailsSelector]), true);
    await key('\uE007'); await waitDetails(false);
    await key(' '); await waitDetails(true);
    await driver.request('DELETE', `/session/${driver.session}/actions`);
  }
  const expanded = await receiptView();
  assert.equal(expanded.details.controlsMatch, true); assert.equal(expanded.details.passwordFields, 0);
  assert.equal(expanded.status, before.status); assert.equal(expanded.statusVisible, true);
  assert.equal(expanded.previewVisible, expectedPreview); assert.equal(expanded.password, '');
  assert.deepEqual(await workspaceHashes(), stored, 'Read-only details must preserve database, WAL and key bytes.');
  (evidence.archiveDetails ??= []).push({ status: before.status, mainMetadata: before.metadata, collapsedHasNoAdvancedFields: true, advancedMetadata: Object.fromEntries(advancedFields.map(field => [field, expanded.metadata[field]])), warningRemainsVisible: true, previewVisible: expectedPreview, noFormsOrPasswords: true, keyboardEnterAndSpace: keyboard, storageHashesPreserved: true });
  return expanded;
}
async function closeDetails() {
  const before = await receiptView();
  await driver.click(detailsSelector); await waitDetails(false);
  const after = await receiptView();
  assert.equal(after.status, before.status); assert.equal(after.statusVisible, true);
  assert.equal(after.password, ''); assert.equal(after.previewVisible, before.previewVisible);
}

async function inspectGui(fileName, status) {
  await driver.selectVisibleOption('#backup-file', fileName);
  await driver.type('#backup-inspection-password', password);
  await driver.click('.backup-inspection-form button[type="submit"]');
  await waitFor(async () => Boolean(await driver.execute('return document.querySelector(".backup-inspection-receipt")?.textContent.includes(arguments[0]);', [fileName])), 'native authenticated content receipt', 30000);
  await waitFor(async () => Boolean(await driver.execute('return document.activeElement?.matches(".backup-inspection-receipt");')), 'focus after content inspection');
  const view = await readAdvancedDetails(false);
  assert.match(view.text, /archive authenticated/i);
  assert.equal(view.status, status);
  assert.equal(view.metadata.File, fileName);
  assert.equal(view.password, '');
  assert.notEqual(view.passwordInvalid, 'true');
  await closeDetails();
  return view;
}

async function focusOwnedWindow() {
  const queue = [driver.child.pid];
  for (let count = 0; queue.length && count < 64; count++) {
    const pid = queue.shift();
    try {
      if (await readlink(`/proc/${pid}/exe`) === application) {
        const focus = JSON.parse(execFileSync('/usr/bin/python3', [join(harness, 'x11-close.py'), String(pid), 'focus'], { env: fixture.environment, encoding: 'utf8', timeout: 10000 }));
        (evidence.focus ??= []).push(focus);
        await waitFor(async () => await driver.execute('return document.hasFocus();'), 'owned native document focus');
        return;
      }
      queue.push(...(await readFile(`/proc/${pid}/task/${pid}/children`, 'utf8')).trim().split(/\s+/).filter(Boolean).map(Number));
    } catch (error) { if (error.code !== 'ENOENT') throw error; }
  }
  assert.fail('The owned native application was not found.');
}

async function setTheme(theme, mode = 'pointer') {
  await driver.navigate('settings');
  if ((await driver.execute('return document.querySelector("#theme").textContent.trim();')) === { system: 'System', light: 'Light', dark: 'Dark' }[theme]) return;
  await focusOwnedWindow();
  const currentLabel = await driver.execute('return document.querySelector("#theme").textContent.trim();');
  const optionFocused = label => driver.execute('const trigger=document.querySelector("#theme"),list=document.getElementById(trigger.getAttribute("aria-controls")),option=document.activeElement;return trigger.getAttribute("aria-expanded")==="true"&&Boolean(list?.contains(option))&&option?.getAttribute("role")==="option"&&option.textContent.trim()===arguments[0];', [label]);
  const keys = async values => {
    assert.equal(await driver.execute('return document.hasFocus();'), true);
    return driver.request('POST', `/session/${driver.session}/actions`, {
      actions: [{ type: 'key', id: 'inspection-keyboard', actions: values.flatMap(value => [{ type: 'keyDown', value }, { type: 'keyUp', value }]) }],
    });
  };
  await driver.click('#theme');
  await waitFor(async () => optionFocused(currentLabel), 'focused current appearance option before Escape');
  await keys(['\uE00C']);
  await waitFor(async () => await driver.execute('return document.querySelector("#theme").getAttribute("aria-expanded")==="false"&&document.activeElement?.id==="theme";'), 'appearance focus after Escape');
  assert.equal(await driver.execute('return document.activeElement?.id;'), 'theme');
  if (mode === 'keyboard') {
    assert.equal(theme, 'light');
    assert.equal(await driver.execute('return document.querySelector("#theme").textContent.trim();'), 'Dark');
    await driver.click('#theme');
    await waitFor(async () => await driver.execute('return document.querySelector("#theme").getAttribute("aria-expanded")==="true";'), 'appearance options open');
    await waitFor(async () => optionFocused('Dark'), 'focused current Dark appearance option');
    await keys(['\uE013']);
    await waitFor(async () => optionFocused('Light'), 'ArrowUp focuses the Light appearance option');
    await keys(['\uE007']);
  } else await driver.selectVisibleOption('#theme', { system: 'System', light: 'Light', dark: 'Dark' }[theme]);
  await waitFor(async () => (await driver.execute('return !document.querySelector("#theme").disabled&&document.documentElement.dataset.theme===arguments[0];', [theme])) === true, `${mode} appearance save`);
  await driver.request('POST', `/session/${driver.session}/execute/async`, { script: 'const done=arguments[arguments.length-1];requestAnimationFrame(()=>requestAnimationFrame(()=>done(true)));', args: [] });
  assert.equal(await driver.execute('return document.activeElement?.id;'), 'theme', 'Appearance must retain focus after its native save.');
  assert.equal((await driver.invoke('bootstrap')).value?.theme, theme);
  (evidence.themeFocus ??= []).push({ mode, theme, focusAfterSave: 'theme', nativeSaved: true });
}

async function resizeViewport(width, height) {
  for (let attempt = 0; attempt < 3; attempt++) {
    const viewport = await driver.execute('return {width:innerWidth,height:innerHeight};');
    if (viewport.width === width && viewport.height === height) break;
    const rect = await driver.request('GET', `/session/${driver.session}/window/rect`);
    await driver.request('POST', `/session/${driver.session}/window/rect`, { width: rect.width + width - viewport.width, height: rect.height + height - viewport.height });
    await driver.request('POST', `/session/${driver.session}/execute/async`, { script: 'const done=arguments[arguments.length-1];requestAnimationFrame(()=>requestAnimationFrame(()=>done(true)));', args: [] });
  }
  assert.deepEqual(await driver.execute('return {width:innerWidth,height:innerHeight};'), { width, height });
}

async function verifyDetailsKeyboardClearance() {
  const original = await driver.execute('return {width:innerWidth,height:innerHeight};');
  assert.equal(await driver.execute('return document.activeElement?.matches(".backup-inspection-receipt");'), true, 'Successful inspection must focus its actual receipt.');
  await resizeViewport(360, 800);
  await driver.execute('document.querySelector(".backup-inspection-receipt").scrollIntoView({block:"center"});return true;');
  await focusOwnedWindow();
  assert.equal(await driver.execute('return document.activeElement?.matches(".backup-inspection-receipt");'), true, 'Viewport changes must preserve the inspection result focus.');
  await driver.request('POST', `/session/${driver.session}/actions`, { actions: [{ type: 'key', id: 'archive-details-forward-tab', actions: [{ type: 'keyDown', value: '\uE004' }, { type: 'keyUp', value: '\uE004' }] }] });
  await driver.request('DELETE', `/session/${driver.session}/actions`);
  await waitFor(async () => driver.execute('return document.activeElement?.matches(arguments[0]);', [detailsSelector]), 'actual Tab from inspection result to Details');
  const view = await driver.execute('const receipt=document.querySelector(".backup-inspection-receipt"),trigger=receipt.querySelector("[data-slot=collapsible-trigger]"),counts=[...receipt.querySelectorAll(".backup-inspection-metadata > dd")].filter(node=>node.getClientRects().length),count=counts.at(-1),style=getComputedStyle(trigger),rect=node=>{const r=node.getBoundingClientRect();return {left:r.left,top:r.top,right:r.right,bottom:r.bottom,width:r.width,height:r.height};},triggerRect=rect(trigger),countRect=rect(count),outlineWidth=parseFloat(style.outlineWidth),outlineOffset=parseFloat(style.outlineOffset),outset=outlineWidth+outlineOffset,outlineRect={left:triggerRect.left-outset,top:triggerRect.top-outset,right:triggerRect.right+outset,bottom:triggerRect.bottom+outset};return {width:innerWidth,height:innerHeight,focused:document.activeElement===trigger,focusVisible:trigger.matches(":focus-visible"),expanded:trigger.getAttribute("aria-expanded"),lastCount:{label:count.previousElementSibling.textContent,value:count.textContent,rect:countRect},triggerRect,outline:{width:outlineWidth,offset:outlineOffset,style:style.outlineStyle,rect:outlineRect},gap:outlineRect.top-countRect.bottom,fontSize:style.fontSize,fontFamily:style.fontFamily};');
  assert.deepEqual({ width: view.width, height: view.height }, { width: 360, height: 800 });
  assert.equal(view.focused, true); assert.equal(view.focusVisible, true); assert.equal(view.expanded, 'false');
  assert.equal(view.lastCount.label, 'Accounts'); assert.equal(view.lastCount.value, '0');
  assert.equal(view.outline.style, 'solid'); assert.ok(view.outline.width >= 2); assert.ok(view.outline.offset >= 0);
  evidence.detailsKeyboardClearance = { ...view, method: 'Actual Tab from the application-focused successful inspection receipt. Normal CSS size; no DOM focus assignment or text scaling.' };
  assert.ok(view.gap > 0, 'The visible Details focus outline must have positive clearance below the last count.');
  await captureReceipt('inspection-details-keyboard-clearance-dark-360.png', 'dark', 360, 800);
  assert.equal(await driver.execute('return document.activeElement?.matches(arguments[0]);', [detailsSelector]), true);
  await resizeViewport(original.width, original.height);
}

async function captureReceipt(name, theme, width, height) {
  await setTheme(theme, theme === 'light' ? 'keyboard' : 'pointer');
  await resizeViewport(width, height);
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
  evidence.fixtureGenerator = { path: generator, sha256: await hash(generator) };
  evidence.sourceDirtyPaths = execFileSync('git', ['status', '--porcelain'], { cwd: repository, encoding: 'utf8' }).trim().split('\n').filter(Boolean);
  evidence.sourceHashes = {};
  const sources = execFileSync('rg', ['--files', 'apps/desktop/src', 'apps/desktop/src-tauri/src', 'crates/openusage-storage/src', 'crates/openusage-import/src'], { cwd: repository, encoding: 'utf8' }).trim().split('\n');
  for (const path of [...sources, 'Cargo.toml', 'Cargo.lock', 'crates/openusage-storage/Cargo.toml', 'crates/openusage-import/Cargo.toml', 'apps/desktop/src-tauri/Cargo.toml', 'apps/desktop/src-tauri/tauri.conf.json', 'apps/desktop/src-tauri/build.rs', 'apps/desktop/src-tauri/capabilities/main-window.json']) evidence.sourceHashes[path] = await hash(join(repository, path));
  evidence.harnessHashes = {};
  for (const path of ['backup-inspection.mjs', 'private-fixture.mjs', 'webdriver.mjs', 'x11-close.py', 'backup-verifier/Cargo.toml', 'backup-verifier/Cargo.lock', 'backup-verifier/src/bin/openusage-native-backup-fixtures.rs']) evidence.harnessHashes[path] = await hash(join(harness, path));
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

  for (const [index, label] of ['Content synthetic active one', 'Content synthetic active two', 'Content synthetic archived'].entries()) {
    await driver.openProfileCreate();
    await driver.type('#profile-label', label);
    await driver.click('.profile-create-form button[type="submit"]');
    await waitFor(async () => (await driver.invoke('list_profiles')).value?.profiles.length === index + 1, 'GUI-created backup fixture profile');
    await waitFor(async () => (await driver.execute('const input=document.querySelector("#profile-label");return !input.disabled&&input.value==="";')) === true, 'profile creation form ready');
  }
  const profiles = (await driver.invoke('list_profiles')).value;
  assert.equal(profiles.activeProfiles, 3);
  const archived = profiles.profiles.find(profile => profile.label === 'Content synthetic archived');
  assert.ok(archived);
  await driver.openProfileOptions(archived.id);
  await driver.click(`[data-profile-id="${archived.id}"] [data-profile-action="archive"]`);
  await waitFor(async () => (await driver.invoke('list_profiles')).value?.activeProfiles === 2, 'GUI-archived backup fixture profile');
  await waitFor(async () => (await driver.execute('return !document.querySelector("#theme").disabled;')) === true, 'archive save completed');
  evidence.expectedContentSummary = { activeProfiles: 2, archivedProfiles: '1', accounts: '0' };
  evidence.steps.push({ contentFixtureProfilesCreatedThroughGui: 3, archivedThroughGui: 1, noProviderLogin: true });

  await setTheme('dark');
  await driver.navigate('settings'); await clickButton('Inspect a backup');
  await waitFor(async () => Boolean(await driver.execute('return document.querySelector(".backup-inspection-empty");')), 'native empty backup catalog');
  assert.equal(await driver.execute('return Boolean(document.querySelector(".backup-inspection-form"));'), false);
  assert.deepEqual(await readdir(fixture.dirs.downloads), []);
  await driver.execute('document.querySelector(".backup-inspection-empty").scrollIntoView({block:"center"});return true;');
  await driver.screenshot(join(output, 'inspection-empty-catalog-dark.png'));
  evidence.screenshots.push('inspection-empty-catalog-dark.png');
  await driver.navigate('settings'); await clickButton('Create a backup');
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

  await driver.navigate('settings'); await clickButton('Inspect a backup');
  await waitFor(async () => Boolean(await driver.execute('const trigger=document.querySelector("#backup-file");return trigger?.getAttribute("role")==="combobox"&&!trigger.disabled;', [fileName])), 'enabled backup selector before choosing the exported file');
  await driver.selectVisibleOption('#backup-file', fileName);
  await driver.type('#backup-inspection-password', 'x');
  await driver.click('.backup-inspection-form button[type="submit"]');
  await waitFor(async () => Boolean(await driver.execute('return document.querySelector("#backup-inspection-error");')), 'wrong-password inspection guidance', 30000);
  await waitFor(async () => (await driver.execute('return document.activeElement?.id;')) === 'backup-inspection-password', 'wrong inspection password focus');
  const failedView = await driver.execute('const field=document.querySelector("#backup-inspection-password"),error=document.querySelector("#backup-inspection-error");return {empty:field.value==="",invalid:field.getAttribute("aria-invalid"),fileInvalid:document.querySelector("#backup-file").getAttribute("aria-invalid"),describedBy:field.getAttribute("aria-describedby"),errorId:error.id,message:error.textContent,errorColor:getComputedStyle(error).color,invalidBorderColor:getComputedStyle(field.closest("[data-slot=input-control]")??field).borderTopColor,invalidBorderWidth:getComputedStyle(field.closest("[data-slot=input-control]")??field).borderTopWidth};');
  assert.equal(failedView.empty, true);
  assert.equal(failedView.invalid, 'true');
  assert.notEqual(failedView.fileInvalid, 'true');
  assert.ok(failedView.describedBy?.split(' ').includes(failedView.errorId));
  assert.match(failedView.message, /password|authenticate/i);
  assert.equal(failedView.errorColor, failedView.invalidBorderColor);
  assert.ok(parseFloat(failedView.invalidBorderWidth) >= 1, "The actual invalid field control must have a visible error border.");
  await driver.execute('document.querySelector("#backup-inspection-error").scrollIntoView({block:"center"});return true;');
  await driver.screenshot(join(output, 'inspection-wrong-password-dark.png'));
  evidence.screenshots.push('inspection-wrong-password-dark.png');
  await driver.type('#backup-inspection-password', password);
  await driver.click('.backup-inspection-form button[type="submit"]');
  await waitFor(async () => Boolean(await driver.execute('return document.querySelector(".backup-inspection-receipt");')), 'native authenticated inspection receipt', 30000);
  await waitFor(async () => Boolean(await driver.execute('return document.activeElement?.matches(".backup-inspection-receipt");')), 'focus after successful archive inspection');
  await verifyDetailsKeyboardClearance();
  const inspected = await driver.invoke('inspect_backup', { fileName, password });
  assert.equal(inspected.ok, true);
  const metadata = inspected.value;
  assert.deepEqual(Object.keys(metadata).sort(), ['archiveVersion', 'contentSummary', 'databaseValidation', 'entries', 'entryCount', 'fileName', 'manifestSchemaVersion', 'restoreAvailable', 'totalBytes']);
  assert.equal(metadata.fileName, fileName);
  assert.equal(metadata.archiveVersion, 1);
  assert.equal(metadata.manifestSchemaVersion, expectedSchemaVersion);
  assert.equal(metadata.entryCount, 1);
  assert.equal(metadata.databaseValidation, 'valid');
  assert.deepEqual(metadata.contentSummary, evidence.expectedContentSummary);
  assert.equal(metadata.restoreAvailable, false);
  assert.deepEqual(metadata.entries.map(entry => Object.keys(entry).sort()), [['bytes', 'name']]);
  assert.equal(metadata.entries[0].name, 'records.sqlite');
  assert.match(metadata.entries[0].bytes, /^[1-9][0-9]*$/);
  assert.equal(metadata.totalBytes, metadata.entries[0].bytes);
  const receipt = await driver.execute('return document.querySelector(".backup-inspection-receipt").textContent;');
  assert.ok(receipt.includes(fileName));
  assert.match(receipt, /archive authenticated/i);
  assert.match(receipt, /backup contents checked/i);
  assert.ok(receipt.includes('Backup contents checked. Preview the records before restoring.'));
  assert.equal(await driver.execute('return [...document.querySelectorAll("#backup-inspection button, #backup-inspection a")].some(node=>/^(apply|replace|undo|restore)\\b/i.test(node.textContent.trim()));'), false);
  assert.equal(await driver.execute('return document.querySelector("#backup-inspection-password")?.value??"";'), '');
  const counts = await driver.execute('return Object.fromEntries([...document.querySelectorAll(".backup-inspection-metadata dt")].map(node=>[node.textContent,node.nextElementSibling?.textContent]));');
  assert.deepEqual([counts['Active profiles'], counts['Archived profiles'], counts.Accounts], ['2', '1', '0']);
  const advanced = await readAdvancedDetails(true, true);
  assert.deepEqual(advancedFields.map(field => advanced.metadata[field]), ['1', String(expectedSchemaVersion), '1', `${metadata.totalBytes} bytes`]);
  await closeDetails();
  evidence.inspection = metadata;
  evidence.wrongPasswordView = failedView;
  await driver.execute('document.querySelector(".backup-inspection-receipt").scrollIntoView({block:"center"});return true;');
  await driver.screenshot(join(output, 'inspection-authenticated-dark.png'));
  evidence.screenshots.push('inspection-authenticated-dark.png');
  await driver.type('#backup-inspection-password', 'Synthetic disclosure switch only');
  await driver.navigate('settings'); await clickButton('Create a backup');
  await waitFor(async () => (await driver.execute('return !document.querySelector("#backup-inspection-password")&&Boolean(document.querySelector(".backup-form"));')) === true, 'inspection password removed when switching backup action');
  assert.deepEqual(await driver.execute('return [...document.querySelectorAll(".backup-form input[type=password]")].map(node=>node.value);'), ['', '']);
  await driver.navigate('settings'); await clickButton('Inspect a backup');
  await waitFor(async () => Boolean(await driver.execute('return document.querySelector("#backup-inspection-password");')), 'inspection form after disclosure switch');
  assert.equal(await driver.execute('return document.querySelector("#backup-inspection-password").value;'), '');
  evidence.steps.push({ backupDisclosureSwitchClearsPassword: true, inspectionOffersNoApplyAction: true });

  const generatorStaging = join(fixture.root, 'fixture-generator-staging');
  await mkdir(generatorStaging, { mode: 0o700 });
  const generated = JSON.parse(execFileSync(generator, [archive, generatorStaging, directory], { input: password, encoding: 'utf8', env: fixture.environment, cwd: fixture.root, timeout: 60000, maxBuffer: 1024 * 1024 }));
  assert.equal(generated.authenticatedFixtures, 4);
  assert.equal(generated.plaintextDiskStaging, false);
  assert.equal(generated.files.length, 4);
  assert.deepEqual(await readdir(generatorStaging), []);
  evidence.contentFixtures = [];
  for (const [fixtureName, state, schema, screenshot] of [
    ['openusage-backup-FixtureNonSqlite.oubackup', 'invalid', expectedSchemaVersion, 'inspection-invalid-database-dark.png'],
    ['openusage-backup-FixtureManifest2.oubackup', 'unsupported', 77, 'inspection-unsupported-manifest-dark.png'],
    ['openusage-backup-FixtureFutureDB1.oubackup', 'unsupported', expectedSchemaVersion, null],
    ['openusage-backup-FixtureBadRows01.oubackup', 'invalid', expectedSchemaVersion, 'inspection-invalid-relation-dark.png'],
  ]) {
    assert.ok(generated.files.includes(fixtureName));
    const path = join(directory, fixtureName);
    assert.equal((await lstat(path)).mode & 0o777, 0o600);
    const digest = await hash(path);
    const result = await driver.invoke('inspect_backup', { fileName: fixtureName, password });
    assert.equal(result.ok, true);
    assert.equal(result.value.fileName, fixtureName);
    assert.equal(result.value.manifestSchemaVersion, schema);
    assert.equal(result.value.databaseValidation, state);
    assert.equal(result.value.contentSummary, null);
    assert.equal(result.value.restoreAvailable, false);
    await clickButton('Refresh backups');
    await waitFor(async () => Boolean(await driver.execute('const select=document.querySelector("#backup-file");return select?.getAttribute("role")==="combobox"&&!select.disabled;', [fixtureName])), 'enabled backup selector before choosing the content fixture');
    const view = await inspectGui(fixtureName, state === 'invalid' ? 'These records cannot be restored. Keep the backup file.' : 'This version of OpenUsage cannot check these records. Restore is unavailable.');
    assert.equal(view.metadata['Schema version'], String(schema));
    assert.deepEqual([view.metadata['Archive version'], view.metadata.Files, view.metadata['Total size']], ['1', '1', `${result.value.totalBytes} bytes`]);
    for (const field of ['Active profiles', 'Archived profiles', 'Accounts']) assert.equal(Object.hasOwn(view.metadata, field), false);
    assert.equal(await hash(path), digest);
    if (screenshot) await captureReceipt(screenshot, 'dark', 360, 800);
    await copyFile(path, join(output, fixtureName));
    await chmod(join(output, fixtureName), 0o600);
    evidence.contentFixtures.push({ fileName: fixtureName, sha256: digest, manifestSchemaVersion: schema, databaseValidation: state, contentSummary: null, sourcePreserved: true, guiAuthenticatedMetadataVisible: true });
  }

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
  await waitFor(async () => Boolean(await driver.execute('const trigger=document.querySelector("#backup-file");return trigger?.getAttribute("role")==="combobox"&&!trigger.disabled;', [truncatedName])), 'enabled refreshed backup selector before choosing the truncated file');
  await driver.selectVisibleOption('#backup-file', truncatedName);
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
  await driver.selectVisibleOption('#backup-file', fileName);
  await driver.type('#backup-inspection-password', password);
  await driver.click('.backup-inspection-form button[type="submit"]');
  await waitFor(async () => Boolean(await driver.execute('return document.querySelector(".backup-inspection-receipt")?.textContent.includes(arguments[0]);', [fileName])), 'healthy archive authenticated after structural failure', 30000);
  assert.equal(await hash(external), externalHash);
  assert.equal(await hash(archive), archiveHash);
  assert.deepEqual(await workspaceHashes(), before, 'Read-only inspection must preserve the encrypted workspace and wrapped key.');
  const staging = join(dirname(fixture.database), 'backup-inspection');
  assert.equal((await lstat(staging)).mode & 0o777, 0o700);
  assert.deepEqual(await readdir(staging), []);
  evidence.steps.push({ guiEmptyCatalog: true, guiWrongPasswordRejectedAndCleared: true, guiArchiveAuthenticated: true, guiStructuralFailureFocusWithoutPasswordInvalid: true, validContentCountsMatchGuiProfiles: true, authenticatedInvalidAndUnsupportedContentRetainsMetadata: true, restoreAvailable: false, corruptedArchiveFixtures: fixtures, unsafeFileFixtures: ['Synthetic symlink.', 'Synthetic hardlink.', 'Synthetic file with group/other read permission.'], unsafeFilesExcludedFromCatalog: true, sourceArchiveAndWorkspacePreserved: true, workspacePreservationScope: 'Workspace hashes match across inspection and fault checks. Later theme selection is a separate GUI write.', noRetainedInspectionStaging: true });
  await copyFile(archive, join(output, fileName)); await chmod(join(output, fileName), 0o600);
  evidence.archive = { fileName, sha256: archiveHash, fileBytes: (await stat(archive, { bigint: true })).size.toString() };
  assert.deepEqual(await readdir(fixture.dirs.codex), ['synthetic-fixture.txt']);
  assert.equal(await readFile(join(fixture.dirs.codex, 'synthetic-fixture.txt'), 'utf8'), CODEX_SENTINEL);
  evidence.steps.push({ syntheticCodexHomeUnchanged: true });
  await captureReceipt('inspection-authenticated-dark-360.png', 'dark', 360, 800);
  await captureReceipt('inspection-authenticated-light-360.png', 'light', 360, 800);
  await readAdvancedDetails(true);
  await captureReceipt('inspection-details-expanded-light-360.png', 'light', 360, 800);
  await closeDetails();
  assert.equal(await hash(application), evidence.application.sha256);
  assert.equal(await hash(generator), evidence.fixtureGenerator.sha256);
  for (const [path, digest] of Object.entries(evidence.sourceHashes)) assert.equal(await hash(join(repository, path)), digest, `Source changed during inspection: ${path}`);
  for (const [path, digest] of Object.entries(evidence.harnessHashes)) assert.equal(await hash(join(harness, path)), digest, `Harness changed during inspection: ${path}`);
  evidence.result = 'passed';
} catch (error) {
  evidence.error = error.stack ?? error.message;
  process.exitCode = 1;
  if (driver?.session) try { evidence.failureAppearance = await driver.execute('const trigger=document.querySelector("#theme"),active=document.activeElement;return {theme:document.documentElement.dataset.theme,trigger:trigger?.textContent.trim(),expanded:trigger?.getAttribute("aria-expanded"),disabled:trigger?.disabled,documentFocused:document.hasFocus(),activeTag:active?.tagName,activeRole:active?.getAttribute("role"),activeId:active?.id,activeText:active?.textContent.slice(0,160)};'); } catch { /* Preserve the first failure. */ }
  if (driver?.session) { try { await driver.screenshot(join(output, 'failure.png')); evidence.screenshots.push('failure.png'); } catch { /* Preserve the first failure. */ } }
} finally {
  try { await driver?.stop(); } catch (error) { evidence.cleanupError = error.message; evidence.result = 'failed'; process.exitCode = 1; }
  if (fixture) { stopGroup(fixture.bus, 'SIGKILL'); await rm(fixture.root, { recursive: true, force: true }); }
  evidence.finishedAt = new Date().toISOString();
  await writeFile(join(output, 'result.json'), `${JSON.stringify(evidence, null, 2)}\n`, { mode: 0o600 });
  console.log(`${evidence.requirement}: ${evidence.result}. Evidence: ${output}`);
  if (evidence.error) console.error(evidence.error);
}
