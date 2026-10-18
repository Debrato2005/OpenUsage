import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { lstat, mkdir, readFile, readdir, readlink, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { CODEX_SENTINEL, createFixture, stopGroup } from './private-fixture.mjs';
import { NativeDriver, waitFor } from './webdriver.mjs';

const harness = dirname(fileURLToPath(import.meta.url));
const repository = resolve(process.env.OPENUSAGE_NATIVE_SOURCE_ROOT ?? join(harness, '../..'));
const application = resolve(process.env.OPENUSAGE_NATIVE_BINARY ?? join(repository, 'target/debug/openusage-desktop'));
const output = resolve(process.env.OPENUSAGE_NATIVE_EVIDENCE ?? join(tmpdir(), `openusage-theme-select-${randomUUID()}`));
const password = 'OpenUsage synthetic theme selection workspace 2026';
const hash = async path => createHash('sha256').update(await readFile(path)).digest('hex');
const evidence = { requirement: 'NATIVE-COSS-THEME-SELECT-001', result: 'failed', startedAt: new Date().toISOString(), scope: 'Actual native theme selection, keyboard cancellation and confirmation, focus return, SQLCipher-backed preference persistence and genuine window restart. Fresh synthetic workspace and private bus only. No provider process, credential-home inspection, OAuth, release performance, or cross-platform qualification. Enlarged text is test-side CSS scaling, not OS zoom.', screenshots: [], views: [], focus: [], closes: [], steps: [] };
let fixture, driver, sequence = 0;

async function desktopPid() {
  const queue = [driver.child.pid];
  for (let count = 0; queue.length && count < 64; count++) {
    const pid = queue.shift();
    try { if (await readlink(`/proc/${pid}/exe`) === application) return pid; queue.push(...(await readFile(`/proc/${pid}/task/${pid}/children`, 'utf8')).trim().split(/\s+/).filter(Boolean).map(Number)); }
    catch (error) { if (error.code !== 'ENOENT') throw error; }
  }
  assert.fail('The owned native application was not found.');
}
async function open() {
  driver = new NativeDriver(application, fixture.environment, join(output, `driver-${++sequence}.log`)); await driver.start();
  const pid = await desktopPid(); evidence.focus.push(JSON.parse(execFileSync('/usr/bin/python3', [join(harness, 'x11-close.py'), String(pid), 'focus'], { env: fixture.environment, encoding: 'utf8', timeout: 10000 })));
  await waitFor(async () => await driver.execute('return document.hasFocus();'), 'owned native document focus');
}
async function closeNormally() {
  const pid = await desktopPid(); evidence.closes.push(JSON.parse(execFileSync('/usr/bin/python3', [join(harness, 'x11-close.py'), String(pid)], { env: fixture.environment, encoding: 'utf8', timeout: 10000 })));
  await waitFor(async () => { try { await readlink(`/proc/${pid}/exe`); return false; } catch (error) { if (error.code === 'ENOENT') return true; throw error; } }, 'native window exit'); driver.session = undefined; await driver.stop(); driver = undefined;
}
async function key(value) {
  assert.equal(await driver.execute('return document.hasFocus();'), true);
  await driver.request('POST', `/session/${driver.session}/actions`, { actions: [{ type: 'key', id: 'theme-keyboard', actions: [{ type: 'keyDown', value }, { type: 'keyUp', value }] }] });
}
async function liveHashes() {
  const result = {}; for (const path of [fixture.database, `${fixture.database}-wal`, join(dirname(fixture.database), 'database-key.v1')]) try { result[path] = await hash(path); } catch (error) { if (error.code !== 'ENOENT') throw error; } return result;
}
async function saved(value) {
  try {
  await waitFor(async () => (await driver.invoke('bootstrap')).value?.theme === value && await driver.execute('const trigger=document.querySelector("#theme");return !trigger.disabled&&trigger.textContent.trim()===arguments[0]&&document.activeElement===trigger&&trigger.getAttribute("aria-expanded")==="false";', [{ system: 'System', light: 'Light', dark: 'Dark' }[value]]), `saved ${value} preference and trigger focus`);
  } catch (error) { evidence.themeWaitFailure = { expected: value, bootstrap: await driver.invoke('bootstrap'), view: await driver.execute('const trigger=document.querySelector("#theme"),active=document.activeElement;return {triggerText:trigger.textContent.trim(),disabled:trigger.disabled,expanded:trigger.getAttribute("aria-expanded"),triggerFocused:active===trigger,documentFocused:document.hasFocus(),activeId:active?.id,activeTag:active?.tagName,activeRole:active?.getAttribute("role")};') }; throw error; }
  const expected = value === 'system' ? await driver.execute('return matchMedia("(prefers-color-scheme: dark)").matches?"dark":"light";') : value;
  assert.equal(await driver.execute('return document.documentElement.dataset.theme;'), expected);
}
async function openOptions() {
  await driver.click('#theme'); await waitFor(async () => await driver.execute('return document.querySelector("#theme").getAttribute("aria-expanded")==="true";'), 'theme popup open');
  const options = await driver.execute('const trigger=document.querySelector("#theme");const roots=(trigger.getAttribute("aria-controls")??"").split(/\\s+/).filter(Boolean).map(id=>document.getElementById(id)).filter(Boolean);return roots.flatMap(root=>[...root.querySelectorAll("[role=option]")]).filter(node=>{const r=node.getBoundingClientRect();return r.width>0&&r.height>0&&getComputedStyle(node).visibility!=="hidden";}).map(node=>({text:node.textContent.trim(),selected:node.getAttribute("aria-selected")}));');
  assert.deepEqual(options.map(option => option.text), ['System', 'Light', 'Dark']); return options;
}
async function settledFrames() { await driver.request('POST', `/session/${driver.session}/execute/async`, { script: 'const done=arguments[arguments.length-1];requestAnimationFrame(()=>requestAnimationFrame(()=>done(true)));', args: [] }); }
async function capture(name, width = 1280, scale = 1) {
  for (let attempt = 0; attempt < 3; attempt++) { const size = await driver.execute('return {width:innerWidth,height:innerHeight};'); if (size.width === width && size.height === 850) break; const rect = await driver.request('GET', `/session/${driver.session}/window/rect`); await driver.request('POST', `/session/${driver.session}/window/rect`, { width: rect.width + width - size.width, height: rect.height + 850 - size.height }); }
  if (scale !== 1) await driver.execute('window.__themeFonts=[...document.querySelectorAll("body *")].map(node=>({node,style:node.getAttribute("style"),size:getComputedStyle(node).fontSize}));for(const item of window.__themeFonts)item.node.style.fontSize=`${parseFloat(item.size)*arguments[0]}px`;return true;', [scale]);
  try {
    await driver.execute('document.querySelector("#theme").scrollIntoView({block:"center"});return true;'); await settledFrames();
    const view = await driver.execute('const trigger=document.querySelector("#theme"),roots=(trigger.getAttribute("aria-controls")??"").split(/\\s+/).filter(Boolean).map(id=>document.getElementById(id)).filter(Boolean),targets=[document.querySelector(".theme-control"),...roots];const visible=targets.flatMap(node=>[node,...node.querySelectorAll("*")]).filter(node=>{const r=node.getBoundingClientRect();return r.width>1&&r.height>1&&getComputedStyle(node).visibility!=="hidden";});return {width:innerWidth,height:innerHeight,theme:document.documentElement.dataset.theme,trigger:trigger.textContent.trim(),expanded:trigger.getAttribute("aria-expanded"),scrollWidth:document.scrollingElement.scrollWidth,overflow:visible.filter(node=>{const r=node.getBoundingClientRect();return r.left< -1||r.right>innerWidth+1;}).map(node=>node.tagName),optionWords:visible.filter(node=>node.getAttribute("role")==="option").map(node=>{const walker=document.createTreeWalker(node,NodeFilter.SHOW_TEXT),words=[];while(walker.nextNode()){const text=walker.currentNode;for(const match of text.textContent.matchAll(/[A-Za-z]+/g)){const range=document.createRange();range.setStart(text,match.index);range.setEnd(text,match.index+match[0].length);words.push({word:match[0],lines:new Set([...range.getClientRects()].filter(rect=>rect.width>0&&rect.height>0).map(rect=>Math.round(rect.top))).size});}}return words;}).flat()};');
    assert.equal(view.width, width); assert.ok(view.scrollWidth <= width + 1); assert.deepEqual(view.overflow, []); for (const word of view.optionWords) assert.equal(word.lines, 1, 'Theme option words must remain whole.');
    await driver.screenshot(join(output, name)); evidence.screenshots.push(name); evidence.views.push({ name, textScale: scale, ...view });
  } finally { if (scale !== 1) { await driver.execute('for(const item of window.__themeFonts){if(item.style===null)item.node.removeAttribute("style");else item.node.setAttribute("style",item.style);}delete window.__themeFonts;return true;'); await settledFrames(); } }
}

await mkdir(output, { recursive: true, mode: 0o700 }); process.once('exit', () => { driver?.kill('SIGKILL'); stopGroup(fixture?.bus, 'SIGKILL'); });
try {
  evidence.application = { path: application, sha256: await hash(application) }; evidence.sourceCommit = execFileSync('git', ['rev-parse', 'HEAD'], { cwd: repository, encoding: 'utf8' }).trim(); evidence.sourceHashes = {};
  const sources = execFileSync('rg', ['--files', 'apps/desktop/src', 'apps/desktop/src-tauri/src', 'apps/desktop/src-tauri/permissions', 'crates/openusage-storage/src'], { cwd: repository, encoding: 'utf8' }).trim().split('\n'); for (const path of [...sources, 'package.json', 'package-lock.json', 'Cargo.toml', 'Cargo.lock', 'apps/desktop/src-tauri/Cargo.toml', 'apps/desktop/src-tauri/tauri.conf.json', 'apps/desktop/src-tauri/build.rs', 'apps/desktop/src-tauri/capabilities/main-window.json']) evidence.sourceHashes[path] = await hash(join(repository, path));
  evidence.harnessHashes = {}; for (const path of ['theme-select.mjs', 'webdriver.mjs', 'private-fixture.mjs', 'x11-close.py']) evidence.harnessHashes[path] = await hash(join(harness, path));
  fixture = await createFixture('theme-select'); const syntheticHome = join(fixture.root, 'synthetic-home'); await mkdir(syntheticHome, { mode: 0o700 }); fixture.environment.HOME = syntheticHome;
  await open(); await waitFor(async () => await driver.execute('return ["#workspace-passphrase","#workspace-confirmation"].every(selector=>{const field=document.querySelector(selector);return Boolean(field&&!field.disabled);});'), 'enabled setup fields');
  assert.deepEqual(await driver.execute('const control=document.querySelector("#theme");return {tag:control.tagName,role:control.getAttribute("role"),labelledByAppearance:(control.getAttribute("aria-labelledby")??"").split(/\\s+/).includes("theme-label")};'), { tag: 'BUTTON', role: 'combobox', labelledByAppearance: true });
  await driver.type('#workspace-passphrase', password); await driver.type('#workspace-confirmation', password); await driver.click('.unlock-form button[type=submit]'); await waitFor(async () => (await driver.invoke('bootstrap')).value?.storage === 'ready', 'fresh SQLCipher workspace');
  const keyHash = await hash(join(dirname(fixture.database), 'database-key.v1'));
  await driver.selectVisibleOption('#theme', 'Dark'); await saved('dark'); const beforeCancel = await liveHashes();
  const options = await openOptions(); assert.equal(options.find(option => option.text === 'Dark').selected, 'true'); await capture('theme-select-dark-desktop.png');
  await key('\uE013'); assert.equal((await driver.invoke('bootstrap')).value.theme, 'dark'); await key('\uE00C'); await saved('dark'); assert.deepEqual(await liveHashes(), beforeCancel, 'Arrow navigation followed by Escape must not save a new preference.');
  await key('\uE007'); await waitFor(async () => await driver.execute('return document.querySelector("#theme").getAttribute("aria-expanded")==="true";'), 'keyboard-opened theme popup'); await key('\uE013'); await key('\uE007'); await saved('light');
  await openOptions(); await capture('theme-select-light-360.png', 360); await capture('theme-select-light-text200-360.png', 360, 2); await key('\uE00C'); await saved('light'); await capture('theme-select-light-closed-360.png', 360);
  await driver.selectVisibleOption('#theme', 'System'); await saved('system'); await driver.selectVisibleOption('#theme', 'Dark'); await saved('dark');
  const beforeInvalid = await liveHashes(); assert.deepEqual(await driver.invoke('set_theme', { theme: 'unsupported' }), { ok: false, error: 'invalid_arguments' }); assert.deepEqual(await liveHashes(), beforeInvalid); assert.equal(await hash(join(dirname(fixture.database), 'database-key.v1')), keyHash);
  await closeNormally(); await open(); await waitFor(async () => await driver.execute('return Boolean(document.querySelector("#workspace-passphrase")&&!document.querySelector("#workspace-passphrase").disabled);'), 'unlock field after genuine restart'); assert.equal((await driver.invoke('bootstrap')).value.storage, 'locked'); await driver.type('#workspace-passphrase', password); await driver.click('.unlock-form button[type=submit]'); await waitFor(async () => (await driver.invoke('bootstrap')).value?.storage === 'ready', 'reopened encrypted workspace');
  await waitFor(async () => await driver.execute('return document.querySelector("#theme").textContent.trim()==="Dark"&&!document.querySelector("#theme").disabled;'), 'restored saved dark selection'); assert.equal((await driver.invoke('bootstrap')).value.theme, 'dark'); assert.equal(await driver.execute('return document.documentElement.dataset.theme;'), 'dark'); await capture('theme-select-restored-dark-desktop.png');
  assert.equal(await hash(join(dirname(fixture.database), 'database-key.v1')), keyHash); assert.equal(await readFile(join(fixture.dirs.codex, 'synthetic-fixture.txt'), 'utf8'), CODEX_SENTINEL); assert.deepEqual(await readdir(fixture.dirs.codex), ['synthetic-fixture.txt']);
  evidence.steps = [{ actualCompositeAndControlledVisibleOptions: true, pointerSelectionSavedInRust: true, arrowEscapePreservesPreferenceAndDatabase: true, enterArrowEnterConfirmsNewPreference: true, focusReturnsAfterEscapeAndNativeSave: true, systemPreferenceResolvesWithoutInventedOsTheme: true, enlargedOptionWordsRemainWhole: true, savedThemeSurvivesGenuineWindowRestart: true, databaseKeyAndExternalSyntheticHomePreserved: true }];
  await closeNormally(); assert.equal(await hash(application), evidence.application.sha256); for (const [path, digest] of Object.entries(evidence.sourceHashes)) assert.equal(await hash(join(repository, path)), digest); for (const [path, digest] of Object.entries(evidence.harnessHashes)) assert.equal(await hash(join(harness, path)), digest); evidence.result = 'passed';
} catch (error) { evidence.error = error.stack ?? String(error); process.exitCode = 1; if (driver?.session) try { await driver.screenshot(join(output, 'failure.png')); evidence.screenshots.push('failure.png'); } catch {} }
finally { try { await driver?.stop(); } catch (error) { evidence.cleanupError = error.message; evidence.result = 'failed'; process.exitCode = 1; } if (fixture) { stopGroup(fixture.bus, 'SIGKILL'); await rm(fixture.root, { recursive: true, force: true }); } evidence.finishedAt = new Date().toISOString(); await writeFile(join(output, 'result.json'), JSON.stringify(evidence, null, 2) + '\n', { mode: 0o600 }); console.log(`${evidence.requirement}: ${evidence.result}. Evidence: ${output}`); if (evidence.error) console.error(evidence.error); }
