import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdir, readFile, readlink, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { CODEX_SENTINEL, createFixture, stopGroup } from './private-fixture.mjs';
import { NativeDriver, waitFor } from './webdriver.mjs';

const repository = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
assert.ok(process.env.OPENUSAGE_NATIVE_BINARY && process.env.OPENUSAGE_NATIVE_EVIDENCE);
const application = resolve(process.env.OPENUSAGE_NATIVE_BINARY), output = resolve(process.env.OPENUSAGE_NATIVE_EVIDENCE);
const password = 'OpenUsage synthetic navigation workspace 2026';
const hash = async path => createHash('sha256').update(await readFile(path)).digest('hex');
const evidence = { requirement: 'NATIVE-TOP-NAVIGATION-001', result: 'failed', scope: 'Production-configuration Linux Tauri app, private SQLCipher4/WAL workspace and GUI-created synthetic profiles. Real navigation, keyboard actions, responsive layout, themes, saved native zoom and genuine close/reopen. No provider session, token metrics, OS notifications or other-platform acceptance.', layouts: [], screenshots: [], closes: [] };
let fixture, driver, sequence = 0;
async function call(command, args) { const response = await driver.invoke(command, args); assert.equal(response.ok, true, command); return response.value; }
async function open(setup = false) {
  driver = new NativeDriver(application, fixture.environment, join(output, `driver-${++sequence}.log`)); await driver.start();
  await waitFor(() => driver.execute('return Boolean(document.querySelector("#workspace-passphrase")&&!document.querySelector("#workspace-passphrase").disabled);'), 'workspace password');
  await driver.type('#workspace-passphrase', password);
  if (setup) await driver.type('#workspace-confirmation', password);
  await driver.execute('window.__navUnlock={clicks:0,submits:0};document.querySelector(".unlock-form").addEventListener("submit",()=>window.__navUnlock.submits++);document.querySelector(".unlock-form button[type=submit]").addEventListener("click",()=>window.__navUnlock.clicks++);return true;');
  await key('\uE007');
  try { await waitFor(async () => (await call('bootstrap')).storage === 'ready', 'ready workspace'); }
  catch (error) {
    evidence.unlockFailure = await driver.execute('const form=document.querySelector(".unlock-form"),input=document.querySelector("#workspace-passphrase"),button=form?.querySelector("button[type=submit]");return {trace:window.__navUnlock,heading:document.getElementById("startup-heading")?.textContent,error:document.getElementById("workspace-error")?.textContent,inputLength:input?.value.length,valid:form?.checkValidity(),buttonDisabled:button?.disabled,focus:document.activeElement?.tagName,viewport:{width:innerWidth,height:innerHeight},storage:document.querySelector("[data-storage]")?.dataset.storage};');
    throw error;
  }
}
async function pid() {
  const queue = [driver.child.pid];
  for (let i = 0; queue.length && i < 64; i++) {
    const next = queue.shift();
    try { if (await readlink(`/proc/${next}/exe`) === application) return next; queue.push(...(await readFile(`/proc/${next}/task/${next}/children`, 'utf8')).trim().split(/\s+/).filter(Boolean).map(Number)); }
    catch (error) { if (error.code !== 'ENOENT') throw error; }
  }
  throw new Error('The owned app process was not found.');
}
async function close() {
  const owned = await pid();
  evidence.closes.push(JSON.parse(execFileSync('/usr/bin/python3', [join(repository, 'tests/native/x11-close.py'), String(owned)], { env: fixture.environment, encoding: 'utf8', timeout: 10000 })));
  await waitFor(async () => { try { await readlink(`/proc/${owned}/exe`); return false; } catch (error) { if (error.code === 'ENOENT') return true; throw error; } }, 'genuine native exit');
  driver.session = undefined; await driver.stop(); driver = undefined;
}
async function resize(width) {
  for (let i = 0; i < 4; i++) {
    const actual = await driver.execute('return {width:innerWidth,height:innerHeight};');
    if (actual.width === width && actual.height === 850) return;
    const rect = await driver.request('GET', `/session/${driver.session}/window/rect`);
    await driver.request('POST', `/session/${driver.session}/window/rect`, { width: rect.width + width - actual.width, height: rect.height + 850 - actual.height });
  }
  assert.equal(await driver.execute('return innerWidth;'), width);
}
async function layout() {
  const value = await driver.execute('const header=document.querySelector(".workspace-header").getBoundingClientRect(),main=document.querySelector("main").getBoundingClientRect(),links=[...document.querySelectorAll("[data-workspace-nav]")];return {width:innerWidth,headerBottom:header.bottom,mainTop:main.top,mainLeft:main.left,mainWidth:main.width,overflow:document.scrollingElement.scrollWidth>innerWidth+1,links:links.map(node=>{const r=node.getBoundingClientRect();return {name:node.textContent,top:Math.round(r.top),width:r.width,height:r.height};})};');
  evidence.layouts.push(value);
  assert.equal(value.mainLeft, 0); assert.equal(value.mainWidth, value.width); assert.ok(value.mainTop >= value.headerBottom);
  assert.equal(value.overflow, false); assert.deepEqual(value.links.map(item => item.name), ['Overview', 'Usage', 'Settings']);
  assert.equal(new Set(value.links.map(item => item.top)).size, 1);
  assert.ok(value.links.every(item => item.width >= 44 && item.height >= 44));
}
async function key(value) {
  await driver.request('POST', `/session/${driver.session}/actions`, { actions: [{ type: 'key', id: 'navigation-keyboard', actions: [{ type: 'keyDown', value }, { type: 'keyUp', value }] }] });
  await driver.request('DELETE', `/session/${driver.session}/actions`);
}
async function capture(name) { await driver.screenshot(join(output, name)); evidence.screenshots.push(name); }
await mkdir(output, { recursive: true, mode: 0o700 });
process.once('exit', () => { driver?.kill('SIGKILL'); stopGroup(fixture?.bus, 'SIGKILL'); });
try {
  evidence.application = { path: application, sha256: await hash(application) };
  fixture = await createFixture('navigation');
  fixture.environment.HOME = join(fixture.root, 'synthetic-home'); await mkdir(fixture.environment.HOME, { mode: 0o700 });
  await open(true);
  for (const label of ['Synthetic account A', 'Synthetic account B']) {
    await driver.openProfileCreate(); await driver.type('#profile-label', label); await driver.click('.profile-create-form button[type=submit]');
    await waitFor(async () => (await call('list_profiles')).profiles.some(profile => profile.label === label), 'GUI-created profile');
    await waitFor(() => driver.execute('const n=document.querySelector("#profile-label");return n&&!n.disabled&&n.value==="";'), 'profile form settled');
    await driver.click('[data-account-panel-close]');
  }
  const profiles = await call('list_profiles'); assert.equal(profiles.activeProfiles, 2);
  await driver.navigate('overview'); await driver.setAppearance('light');
  for (const width of [1280, 1024, 768, 360]) {
    await resize(width); await layout();
    await driver.navigate('settings'); await driver.click('.settings-privacy');
    assert.equal(await driver.execute('return Boolean(document.getElementById("privacy-details")?.getClientRects().length);'), true);
    await driver.navigate('usage');
    assert.equal(await driver.execute('return document.querySelector(".settings-privacy")===null&&document.getElementById("privacy-details")===null;'), true);
    await driver.navigate('overview');
    await driver.navigate('settings'); await driver.click('.settings-privacy'); await driver.navigate('overview');
    if (width === 1280) await capture('top-navigation-light.png');
    if (width === 360) await capture('top-navigation-narrow.png');
  }
  await resize(1280); await driver.setAppearance('dark'); await capture('top-navigation-dark.png');
  execFileSync('/usr/bin/python3', [join(repository, 'tests/native/x11-close.py'), String(await pid()), 'focus'], { env: fixture.environment, timeout: 10000 });
  await driver.execute('document.querySelector("[data-workspace-nav=overview]").focus();return true;');
  await key('\uE004');
  assert.equal(await driver.execute('return document.activeElement?.dataset.workspaceNav;'), 'usage');
  await key('\uE007');
  await waitFor(() => driver.execute('return document.querySelector("[data-workspace-nav=usage]")?.getAttribute("aria-current")==="page";'), 'keyboard Usage navigation');
  await driver.selectVisibleOption('#display-scale', '125%');
  await waitFor(async () => (await call('get_display_scale')).percent === 125, 'saved native scale');
  await close(); await open();
  assert.deepEqual(await call('list_profiles'), profiles); assert.equal((await call('get_display_scale')).percent, 125);
  assert.equal(await driver.execute('return document.documentElement.dataset.theme;'), 'dark');
  await layout();
  for (const view of ['settings', 'usage', 'overview']) {
    await driver.execute('document.querySelector(arguments[0]).focus();return true;', [`[data-workspace-nav="${view}"]`]);
    await key('\uE007');
    await waitFor(() => driver.execute('return document.querySelector(arguments[0])?.getAttribute("aria-current")==="page";', [`[data-workspace-nav="${view}"]`]), `keyboard ${view} after native zoom restart`);
  }
  await close(); assert.equal(await readFile(join(fixture.dirs.codex, 'synthetic-fixture.txt'), 'utf8'), CODEX_SENTINEL);
  assert.equal(await hash(application), evidence.application.sha256);
  evidence.result = 'passed'; evidence.savedProfilesUnchanged = true; evidence.realKeyboardNavigation = true; evidence.nativeScaleRestored = true;
} catch (error) { evidence.error = String(error?.stack ?? error); throw error; }
finally {
  try { await driver?.stop(); }
  finally {
    stopGroup(fixture?.bus, 'SIGKILL');
    const { rm } = await import('node:fs/promises');
    if (fixture) await rm(fixture.root, { recursive: true, force: true });
    evidence.finishedAt = new Date().toISOString();
    await writeFile(join(output, 'result.json'), JSON.stringify(evidence, null, 2) + '\n', { mode: 0o600 });
  }
}
