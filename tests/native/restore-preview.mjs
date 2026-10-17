import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { chmod, copyFile, lstat, mkdir, readFile, readdir, readlink, rm, writeFile } from 'node:fs/promises';
import { platform, release, tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { CODEX_SENTINEL, createFixture, stopGroup } from './private-fixture.mjs';
import { NativeDriver, waitFor } from './webdriver.mjs';
const harness = dirname(fileURLToPath(import.meta.url));
const repository = resolve(process.env.OPENUSAGE_NATIVE_SOURCE_ROOT ?? join(harness, '../..'));
const application = resolve(process.env.OPENUSAGE_NATIVE_BINARY ?? join(repository, 'target/debug/openusage-desktop'));
const verifier = resolve(process.env.OPENUSAGE_CHECKPOINT_VERIFIER ?? join(harness, 'backup-verifier/target/debug/openusage-native-checkpoint-verifier'));
const generator = resolve(process.env.OPENUSAGE_BACKUP_FIXTURE_GENERATOR ?? join(harness, 'backup-verifier/target/debug/openusage-native-backup-fixtures'));
const output = resolve(process.env.OPENUSAGE_NATIVE_EVIDENCE ?? join(tmpdir(), `openusage-restore-preview-evidence-${randomUUID()}`));
const passphrase = 'OpenUsage synthetic preview workspace 2026';
const password = 'OpenUsage synthetic preview archive 2026';
const hash = async (path) => createHash('sha256').update(await readFile(path)).digest('hex');
const evidence = { requirement: 'NATIVE-RESTORE-PREVIEW-001', startedAt: new Date().toISOString(), result: 'failed', scope: 'Actual GUI state A export, state B changes, authenticated preview only after encrypted checkpoint, read-only Rust checkpoint verification, cancel and normal window-close restart. Synthetic homes only. No apply, undo, crash-durability, forced-exit zeroization, provider login, or cross-platform acceptance.', platform: { os: platform(), release: release() }, screenshots: [], steps: [], checkpoints: [], views: [] };
let fixture, driver, workspace, checkpointDirectory, archive, fileName, sequence = 0;
const markers = new Map();
async function click(text) { const node = await driver.execute('return [...document.querySelectorAll("button")].find(node=>node.textContent.trim()===arguments[0]);', [text]); assert.ok(node, `Missing button: ${text}`); await driver.request('POST', `/session/${driver.session}/element/${node['element-6066-11e4-a52e-4f735466cecf']}/click`, {}); }
async function state() { const bootstrap = await driver.invoke('bootstrap'), profiles = await driver.invoke('list_profiles'); assert.equal(profiles.ok, true); return { theme: bootstrap.value.theme, profiles: profiles.value.profiles.sort((a, b) => a.id.localeCompare(b.id)), accounts: '0' }; }
async function checkpoints() { try {
    return (await readdir(checkpointDirectory)).sort();
}
catch (error) {
    if (error.code === 'ENOENT')
        return [];
    throw error;
} }
async function unchanged(expected) { assert.deepEqual(await state(), expected); for (const [path, digest] of markers)
    assert.equal(await hash(path), digest); assert.equal(await readFile(join(fixture.dirs.codex, 'synthetic-fixture.txt'), 'utf8'), CODEX_SENTINEL); }
async function liveHashes() { const result = {}; for (const path of [fixture.database, `${fixture.database}-wal`, join(workspace, 'database-key.v1')]) {
    try {
        result[path] = await hash(path);
    }
    catch (error) {
        if (error.code !== 'ENOENT')
            throw error;
    }
} return result; }
async function rejected(command, args, error) { assert.deepEqual(await driver.invoke(command, args), { ok: false, error }); }
async function theme(value) { await driver.click(`#theme option[value="${value}"]`); await waitFor(async () => await driver.execute('return !document.querySelector("#theme").disabled&&document.documentElement.dataset.theme===arguments[0];', [value]), 'appearance saved'); }
async function create(label) { await driver.type('#profile-label', label); await driver.click('.profile-create-form button[type=submit]'); await waitFor(async () => await driver.execute('const input=document.querySelector("#profile-label");return !input.disabled&&input.value==="";'), 'profile form ready'); return (await state()).profiles.find(profile => profile.label === label); }
async function open() { driver = new NativeDriver(application, fixture.environment, join(output, `driver-${++sequence}.log`)); await driver.start(); }
async function unlock() { await driver.type('#workspace-passphrase', passphrase); await driver.click('.unlock-form button[type=submit]'); await waitFor(async () => (await driver.invoke('bootstrap')).value?.storage === 'ready', 'workspace unlocked'); }
async function inspect() { await click('Inspect a backup'); await waitFor(async () => Boolean(await driver.execute('return document.querySelector("#backup-file")&&!document.querySelector("#backup-file").disabled;')), 'inspection catalog ready'); await driver.click(`#backup-file option[value="${fileName}"]`); await driver.type('#backup-inspection-password', password); await driver.click('.backup-inspection-form button[type=submit]'); await waitFor(async () => Boolean(await driver.execute('return document.querySelector(".backup-inspection-receipt");')), 'authenticated inspection'); }
async function prepareGui() { await click('Preview restore'); await waitFor(async () => Boolean(await driver.execute('return document.querySelector("#restore-preview-password");')), 'preview password form'); await driver.type('#restore-preview-password', password); await driver.click('.restore-preview-form button[type=submit]'); await waitFor(async () => Boolean(await driver.execute('return document.querySelector(".restore-preview");')), 'checkpoint-backed restore preview', 30000); await waitFor(async () => Boolean(await driver.execute('return document.activeElement?.matches(".restore-preview");')), 'preview focus'); const ids = await driver.execute('const preview=document.querySelector(".restore-preview");return{previewId:preview.dataset.previewId,checkpointId:preview.dataset.checkpointId};'); assert.match(ids.previewId, /^[0-9a-f-]{36}$/); assert.match(ids.checkpointId, /^[0-9a-f-]{36}$/); return ids; }
async function verifyCheckpoint(id, expected) { const path = join(checkpointDirectory, `${id}.sqlite`); const metadata = await lstat(path); assert.equal(metadata.mode & 0o777, 0o600); assert.equal((await lstat(checkpointDirectory)).mode & 0o777, 0o700); const before = await hash(path); const result = JSON.parse(execFileSync(verifier, [path, join(workspace, 'database-key.v1')], { input: JSON.stringify({ passphrase, expected }), encoding: 'utf8', env: fixture.environment, cwd: fixture.root, timeout: 30000, maxBuffer: 1024 * 1024 })); assert.equal(result.verified, true); assert.equal(result.readOnly, true); assert.equal(result.exactStateMatched, true); assert.equal(await hash(path), before); await copyFile(path, join(output, `${id}.sqlite`)); await chmod(join(output, `${id}.sqlite`), 0o600); evidence.checkpoints.push({ id, sha256: before, fileBytes: String(metadata.size), verification: result }); }
async function capture(name, appearance, scale = 1, width = 360, height = 800) {
    for (let attempt = 0; attempt < 3; attempt++) {
        const view = await driver.execute('return{width:innerWidth,height:innerHeight};');
        if (view.width === width && view.height === height)
            break;
        const rect = await driver.request('GET', `/session/${driver.session}/window/rect`);
        await driver.request('POST', `/session/${driver.session}/window/rect`, { width: rect.width + width - view.width, height: rect.height + height - view.height });
    }
    if (scale !== 1)
        await driver.execute('window.__previewFontRestore=[...document.querySelectorAll("body *")].map(node=>({node,style:node.getAttribute("style"),size:getComputedStyle(node).fontSize}));for(const item of window.__previewFontRestore)item.node.style.fontSize=`${parseFloat(item.size)*arguments[0]}px`;return true;', [scale]);
    await driver.execute('document.querySelector(".restore-preview").scrollIntoView({block:"center"});return true;');
    const view = await driver.execute('const panel=document.querySelector(".restore-preview");return{width:innerWidth,height:innerHeight,scrollWidth:document.scrollingElement.scrollWidth,rows:[...panel.querySelectorAll("tbody tr")].map(row=>[...row.children].map(cell=>cell.querySelector("[data-restore-count]")?.textContent??cell.textContent)),overflow:[panel,...panel.querySelectorAll("*")].filter(node=>{const hiddenHeader=node.closest("thead");if(hiddenHeader&&getComputedStyle(hiddenHeader).clipPath!=="none")return false;const r=node.getBoundingClientRect();return r.width>1&&r.height>1&&(r.left< -1||r.right>innerWidth+1||((getComputedStyle(node).overflowX==="hidden"||getComputedStyle(node).overflowX==="clip")&&node.scrollWidth>node.clientWidth+1));}).map(node=>node.tagName),text:panel.textContent,destructive:[...panel.querySelectorAll("button,a")].some(node=>/^(apply|replace|undo|restore now)/i.test(node.textContent.trim()))};');
    assert.equal(view.width, width);
    assert.equal(view.height, height);
    assert.ok(view.scrollWidth <= width + 1);
    assert.deepEqual(view.overflow, []);
    const headerWords = await driver.execute('const words=[];for(const header of document.querySelectorAll(".restore-comparison thead th,.restore-comparison tbody th,.restore-comparison .comparison-label")){const hiddenHeader=header.closest("thead");if(header.getBoundingClientRect().width<2||(hiddenHeader&&getComputedStyle(hiddenHeader).clipPath!=="none"))continue;const walker=document.createTreeWalker(header,NodeFilter.SHOW_TEXT);while(walker.nextNode()){const node=walker.currentNode;for(const match of node.textContent.matchAll(/[A-Za-z]+/g)){const range=document.createRange();range.setStart(node,match.index);range.setEnd(node,match.index+match[0].length);words.push({word:match[0],lines:[...new Set([...range.getClientRects()].filter(rect=>rect.width>0).map(rect=>Math.round(rect.top)))]});}}}return words;');
    assert.deepEqual(headerWords.filter(item => item.lines.length > 1), [], 'Column header words must remain whole.');
    const semantics = await driver.execute('const table=document.querySelector(".restore-comparison");return{role:table.getAttribute("role"),caption:Boolean(table.caption?.textContent.trim()),headers:[...table.querySelectorAll("thead th")].map(node=>[node.textContent,node.getAttribute("role"),node.getAttribute("scope")]),rows:[...table.querySelectorAll("tbody tr")].map(row=>({role:row.getAttribute("role"),cells:[...row.children].map(node=>node.getAttribute("role")),rowScope:row.firstElementChild.getAttribute("scope")}))};');
    assert.equal(semantics.role, 'table');
    assert.equal(semantics.caption, true);
    assert.deepEqual(semantics.headers, [['Records', 'columnheader', 'col'], ['Current', 'columnheader', 'col'], ['Backup', 'columnheader', 'col']]);
    assert.deepEqual(semantics.rows, Array.from({length:3}, () => ({role:'row',cells:['rowheader','cell','cell'],rowScope:'row'})));
    assert.deepEqual(view.rows, [['Active profiles', '2', '2'], ['Archived profiles', '1', '0'], ['Accounts', '0', '0']]);
    assert.match(view.text, /Recovery checkpoint saved on this device with your workspace key/);
    assert.match(view.text, /Applying this backup is not available/);
    assert.equal(view.destructive, false);
    await driver.screenshot(join(output, name));
    evidence.screenshots.push(name);
    evidence.views.push({ name, appearance, textScale: scale, scalingScope: 'Test-side CSS font sizes; not OS or display zoom.', accessibilityScope: 'Native DOM roles and header associations; no screen-reader or platform accessibility-tree qualification.', semantics, headerWords, ...view });
    await driver.execute('document.querySelector(".restore-preview button").scrollIntoView({block:"center"});return true;');
    const reachable = await driver.execute('const button=document.querySelector(".restore-preview button"),rect=button.getBoundingClientRect();return !button.disabled&&rect.top>=0&&rect.bottom<=innerHeight&&rect.left>=0&&rect.right<=innerWidth;');
    assert.equal(reachable, true);
    if (scale !== 1) {
        await driver.execute('document.querySelector(".restore-preview").focus();return true;');
        await driver.request('POST', `/session/${driver.session}/actions`, { actions: [{ type: 'key', id: 'preview-layout-keyboard', actions: [{ type: 'keyDown', value: '\uE004' }, { type: 'keyUp', value: '\uE004' }] }] });
        assert.equal(await driver.execute('return document.activeElement===document.querySelector(".restore-preview button");'), true, 'The close control must be reachable by keyboard with enlarged text.');
    }
    if (scale !== 1)
        await driver.execute('for(const item of window.__previewFontRestore){if(item.style===null)item.node.removeAttribute("style");else item.node.setAttribute("style",item.style);}delete window.__previewFontRestore;return true;');
}
async function appPid() { const queue = [driver.child.pid]; for (let count = 0; queue.length && count < 64; count++) {
    const pid = queue.shift();
    try {
        if (await readlink(`/proc/${pid}/exe`) === application)
            return pid;
        const children = (await readFile(`/proc/${pid}/task/${pid}/children`, 'utf8')).trim().split(/\s+/).filter(Boolean).map(Number);
        queue.push(...children);
    }
    catch (error) {
        if (error.code !== 'ENOENT')
            throw error;
    }
} throw new Error('Cannot identify the owned desktop child.'); }
await mkdir(output, { recursive: true, mode: 0o700 });
process.once('exit', () => { driver?.kill('SIGKILL'); stopGroup(fixture?.bus, 'SIGKILL'); });
try {
    assert.equal(platform(), 'linux');
    evidence.application = { path: application, sha256: await hash(application) };
    evidence.verifier = { path: verifier, sha256: await hash(verifier) };
    evidence.generator = { path: generator, sha256: await hash(generator) };
    evidence.sourceCommit = execFileSync('git', ['rev-parse', 'HEAD'], { cwd: repository, encoding: 'utf8' }).trim();
    evidence.sourceHashes = {};
    const sources = execFileSync('rg', ['--files', 'apps/desktop/src', 'apps/desktop/src-tauri/src', 'crates/openusage-storage/src', 'crates/openusage-import/src'], { cwd: repository, encoding: 'utf8' }).trim().split('\n');
    for (const path of [...sources, 'Cargo.toml', 'Cargo.lock', 'crates/openusage-storage/Cargo.toml', 'crates/openusage-import/Cargo.toml', 'apps/desktop/src-tauri/Cargo.toml', 'apps/desktop/src-tauri/tauri.conf.json', 'apps/desktop/src-tauri/build.rs', 'apps/desktop/src-tauri/capabilities/main-window.json'])
        evidence.sourceHashes[path] = await hash(join(repository, path));
    evidence.harnessHashes = {};
    for (const path of ['restore-preview.mjs', 'private-fixture.mjs', 'webdriver.mjs', 'backup-verifier/Cargo.toml', 'backup-verifier/Cargo.lock', 'backup-verifier/src/bin/openusage-native-checkpoint-verifier.rs', 'backup-verifier/src/bin/openusage-native-backup-fixtures.rs'])
        evidence.harnessHashes[path] = await hash(join(harness, path));
    fixture = await createFixture('restore-preview');
    workspace = dirname(fixture.database);
    checkpointDirectory = join(workspace, 'restore-checkpoints');
    await open();
    await rejected('prepare_restore_preview', { fileName: 'openusage-backup-0000000000000000.oubackup', password }, 'workspace_locked');
    for (const command of ['prepare_restore_preview', 'cancel_restore_preview'])
        await rejected(command, [], 'invalid_arguments');
    await rejected('cancel_restore_preview', { previewId: '../unsafe' }, 'invalid_arguments');
    await driver.type('#workspace-passphrase', passphrase);
    await driver.type('#workspace-confirmation', passphrase);
    await driver.click('.unlock-form button[type=submit]');
    await waitFor(async () => (await driver.invoke('bootstrap')).value?.storage === 'ready', 'workspace created');
    await theme('light');
    const first = await create('Preview original first'), second = await create('Preview original second');
    const stateA = await state();
    evidence.stateA = stateA;
    await click('Create a backup');
    await driver.type('#backup-password', password);
    await driver.type('#backup-confirmation', password);
    await driver.click('#backup-records');
    await driver.click('.backup-form button[type=submit]');
    await waitFor(async () => Boolean(await driver.execute('return document.querySelector(".backup-receipt");')), 'state A archive');
    const directory = join(fixture.dirs.downloads, 'OpenUsage');
    [fileName] = await readdir(directory);
    archive = join(directory, fileName);
    const archiveHash = await hash(archive);
    await driver.click(`[data-profile-id="${first.id}"] [data-profile-action="rename"]`);
    await driver.type('.profile-rename-form input', 'Preview renamed first');
    await driver.click('.profile-rename-form button[type=submit]');
    await waitFor(async () => !(await driver.execute('return Boolean(document.querySelector(".profile-rename-form"));')), 'profile renamed');
    await driver.click(`[data-profile-id="${second.id}"] [data-profile-action="archive"]`);
    await waitFor(async () => (await state()).profiles.find(profile => profile.id === second.id)?.archived === true, 'profile archived');
    await waitFor(async () => await driver.execute('return !document.querySelector("#profile-label").disabled;'), 'profile form enabled');
    await create('Preview third live only');
    await theme('dark');
    const stateB = await state();
    evidence.stateB = stateB;
    for (const profile of stateB.profiles) {
        const path = join(workspace, 'profiles', profile.id, 'preview-synthetic-marker.txt');
        await writeFile(path, 'Explicit synthetic home marker. Never restore provider credentials.\n', { mode: 0o600, flag: 'wx' });
        markers.set(path, await hash(path));
    }
    const keyHash = await hash(join(workspace, 'database-key.v1'));
    const before = await liveHashes();
    await inspect();
    assert.deepEqual(await checkpoints(), []);
    await click('Preview restore');
    await driver.type('#restore-preview-password', 'wrong synthetic password');
    await driver.click('.restore-preview-form button[type=submit]');
    await waitFor(async () => Boolean(await driver.execute('return document.querySelector("#restore-preview-error");')), 'wrong preview password');
    await waitFor(async () => (await driver.execute('return document.activeElement?.id;')) === 'restore-preview-password', 'wrong-password focus');
    assert.equal(await driver.execute('return document.querySelector("#restore-preview-password").value;'), '');
    assert.equal(await driver.execute('return document.querySelector("#restore-preview-password").getAttribute("aria-invalid");'), 'true');
    assert.equal(await driver.execute('return document.querySelector("#restore-preview-password").getAttribute("aria-describedby").split(" ").includes("restore-preview-error");'), true);
    assert.deepEqual(await checkpoints(), []);
    await driver.execute('document.querySelector("#restore-preview-error").scrollIntoView({block:"center"});return true;');
    await driver.screenshot(join(output, 'restore-preview-wrong-password.png'));
    evidence.screenshots.push('restore-preview-wrong-password.png');
    await click('Cancel');
    const generatorStaging = join(fixture.root, 'generator-staging');
    await mkdir(generatorStaging, { mode: 0o700 });
    const generated = JSON.parse(execFileSync(generator, [archive, generatorStaging, directory], { input: password, encoding: 'utf8', env: fixture.environment, timeout: 60000 }));
    assert.equal(generated.authenticatedFixtures, 4);
    const tampered = join(directory, 'openusage-backup-TamperPreview001.oubackup');
    const bytes = await readFile(archive);
    bytes[bytes.length - 1] ^= 1;
    await writeFile(tampered, bytes, { mode: 0o600 });
    for (const [name, error] of [['openusage-backup-FixtureNonSqlite.oubackup', 'invalid_backup_contents'], ['openusage-backup-FixtureManifest2.oubackup', 'unsupported_backup_contents'], ['openusage-backup-TamperPreview001.oubackup', 'backup_authentication_failed']]) {
        await rejected('prepare_restore_preview', { fileName: name, password }, error);
        assert.deepEqual(await checkpoints(), []);
        await unchanged(stateB);
    }
    await rejected('prepare_restore_preview', { fileName, password, unexpected: true }, 'invalid_arguments');
    assert.deepEqual(await liveHashes(), before);
    evidence.steps.push({ preAuthenticationAndInvalidContentCreateNoCheckpoint: true, wrongPasswordClearedAndAssociated: true });
    const dark = await prepareGui();
    await verifyCheckpoint(dark.checkpointId, stateB);
    await capture('restore-preview-dark-desktop.png', 'dark', 1, 1280, 850);
    await capture('restore-preview-dark-360.png', 'dark');
    await click('Close preview');
    await waitFor(async () => !await driver.execute('return Boolean(document.querySelector(".restore-preview"));'), 'preview closed');
    await waitFor(async () => (await driver.execute('return document.activeElement?.textContent.trim();')) === 'Preview restore', 'preview toggle focus after close');
    assert.deepEqual(await driver.invoke('cancel_restore_preview', { previewId: dark.previewId }), { ok: true, value: { cancelled: false } });
    await unchanged(stateB);
    assert.deepEqual(await liveHashes(), before);
    assert.equal(await hash(join(workspace, 'database-key.v1')), keyHash);
    await theme('light');
    const lightState = await state();
    const light = await prepareGui();
    await verifyCheckpoint(light.checkpointId, lightState);
    await capture('restore-preview-light-360.png', 'light');
    await click('Close preview');
    await waitFor(async () => !await driver.execute('return Boolean(document.querySelector(".restore-preview"));'), 'light preview closed');
    await theme('dark');
    const scaled = await prepareGui();
    await verifyCheckpoint(scaled.checkpointId, stateB);
    await capture('restore-preview-dark-text-200-360.png', 'dark', 2);
    await click('Close preview');
    await waitFor(async () => !await driver.execute('return Boolean(document.querySelector(".restore-preview"));'), 'scaled preview closed');
    const stale = await prepareGui();
    await theme('light');
    assert.deepEqual(await driver.invoke('cancel_restore_preview', { previewId: stale.previewId }), { ok: true, value: { cancelled: false } });
    await click('Close preview');
    await waitFor(async () => !await driver.execute('return Boolean(document.querySelector(".restore-preview"));'), 'stale preview closed');
    await theme('dark');
    const retained = await driver.invoke('prepare_restore_preview', { fileName, password });
    assert.equal(retained.ok, true);
    assert.deepEqual(Object.keys(retained.value).sort(), ['checkpoint', 'current', 'fileName', 'incoming', 'previewId', 'restoreAvailable']);
    assert.equal(retained.value.restoreAvailable, false);
    assert.deepEqual(retained.value.current, { activeProfiles: 2, archivedProfiles: '1', accounts: '0' });
    assert.deepEqual(retained.value.incoming, { activeProfiles: 2, archivedProfiles: '0', accounts: '0' });
    const count = (await checkpoints()).length;
    await rejected('prepare_restore_preview', { fileName, password: 'wrong synthetic password' }, 'backup_authentication_failed');
    assert.equal((await checkpoints()).length, count);
    assert.deepEqual(await driver.invoke('cancel_restore_preview', { previewId: retained.value.previewId }), { ok: true, value: { cancelled: false } });
    await unchanged(stateB);
    const exitPreview = await prepareGui();
    await verifyCheckpoint(exitPreview.checkpointId, stateB);
    const pid = await appPid();
    const remaining = await driver.request('DELETE', `/session/${driver.session}/window`);
    assert.deepEqual(remaining, []);
    await waitFor(async () => { try {
        await readlink(`/proc/${pid}/exe`);
        return false;
    }
    catch (error) {
        if (error.code === 'ENOENT')
            return true;
        throw error;
    } }, 'desktop exited after normal window close');
    driver.session = undefined;
    await driver.stop();
    driver = undefined;
    evidence.steps.push({ normalWindowClose: true, ownedDesktopExitedBeforeDriverCleanup: true, noForcedExitZeroizationClaim: true });
    await open();
    await unlock();
    await unchanged(stateB);
    assert.deepEqual(await driver.invoke('cancel_restore_preview', { previewId: exitPreview.previewId }), { ok: true, value: { cancelled: false } });
    await verifyCheckpoint(exitPreview.checkpointId, stateB);
    assert.equal(await hash(join(workspace, 'database-key.v1')), keyHash);
    assert.equal(await hash(archive), archiveHash);
    for (const name of ['restore-staging', 'backup-inspection']) {
        try {
            assert.deepEqual(await readdir(join(workspace, name)), []);
        }
        catch (error) {
            if (error.code !== 'ENOENT')
                throw error;
        }
    }
    for (const item of evidence.checkpoints)
        assert.equal(await hash(join(checkpointDirectory, `${item.id}.sqlite`)), item.sha256);
    assert.deepEqual(await readdir(fixture.dirs.codex), ['synthetic-fixture.txt']);
    evidence.steps.push({ cancelPreservesLiveStateAndKey: true, liveThemeWriteInvalidatesCache: true, failedReplacementClearsPriorCache: true, restartPreservesExactStateB: true, encryptedCheckpointsPersist: true, retainedPreviewAbsentAfterRestart: true, noRetainedStaging: true, syntheticHomesUnchanged: true });
    for (const item of [evidence.application, evidence.verifier, evidence.generator])
        assert.equal(await hash(item.path), item.sha256);
    for (const [path, digest] of Object.entries(evidence.sourceHashes))
        assert.equal(await hash(join(repository, path)), digest, `Source drift: ${path}`);
    for (const [path, digest] of Object.entries(evidence.harnessHashes))
        assert.equal(await hash(join(harness, path)), digest, `Harness drift: ${path}`);
    evidence.result = 'passed';
}
catch (error) {
    evidence.error = error.stack ?? String(error);
    process.exitCode = 1;
    if (driver?.session)
        try {
            await driver.screenshot(join(output, 'failure.png'));
            evidence.screenshots.push('failure.png');
        }
        catch { }
}
finally {
    try {
        await driver?.stop();
    }
    catch (error) {
        evidence.cleanupError = error.message;
        evidence.result = 'failed';
        process.exitCode = 1;
    }
    if (fixture) {
        stopGroup(fixture.bus, 'SIGKILL');
        await rm(fixture.root, { recursive: true, force: true });
    }
    evidence.finishedAt = new Date().toISOString();
    await writeFile(join(output, 'result.json'), JSON.stringify(evidence, null, 2) + '\n', { mode: 0o600 });
    console.log(`${evidence.requirement}: ${evidence.result}. Evidence: ${output}`);
    if (evidence.error)
        console.error(evidence.error);
}
