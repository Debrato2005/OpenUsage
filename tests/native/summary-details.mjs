import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { chmod, copyFile, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { createFixture, stopGroup } from './private-fixture.mjs';
import { NativeDriver, waitFor } from './webdriver.mjs';
import { settleViewport } from './viewport.mjs';

const repository = resolve('.');
const application = resolve(process.env.OPENUSAGE_NATIVE_BINARY);
const output = resolve(process.env.OPENUSAGE_NATIVE_EVIDENCE);
const evidence = { requirement: 'NATIVE-SUMMARY-DETAILS-001', result: 'failed', scope: 'Linux native-test Tauri/WebKitGTK, private homes and D-Bus, approved synthetic provider. Summary disclosure, actual keyboard input, saved-reading preservation, both themes and narrow text layout. No real account, screen reader, OS scaling, Windows/macOS or full AT-85 acceptance.', screenshots: [] };
const hash = async path => createHash('sha256').update(await readFile(path)).digest('hex');
const tile = id => `[data-overview-summary="${id}"]`;
const trigger = id => `${tile(id)} .overview-glance-trigger`;
const panel = '[data-detail-panel][open]';
let fixture, driver;
async function call(command, args = {}) {
  const reply = await driver.invoke(command, args);
  assert.equal(reply.ok, true, `${command}: ${JSON.stringify(reply)}`);
  return reply.value;
}
async function key(value) {
  await driver.request('POST', `/session/${driver.session}/actions`, { actions: [{ type: 'key', id: 'summary-keyboard', actions: [{ type: 'keyDown', value }, { type: 'keyUp', value }] }] });
  await driver.request('DELETE', `/session/${driver.session}/actions`);
}
// Each summary opens its details in the one detail panel.
async function opened(name) {
  await waitFor(() => driver.execute('return document.querySelectorAll(arguments[0]).length===1&&document.querySelector(arguments[0]).getAttribute("aria-label")===arguments[1];', [panel, `${name} details`]), `${name} details open`);
}
async function closed(id) {
  await waitFor(() => driver.execute('return !document.querySelector(arguments[0]);', [panel]), 'details closed');
  assert.equal(await driver.execute('return document.activeElement===document.querySelector(arguments[0]);', [trigger(id)]), true, 'Focus returns to the summary.');
}
async function openQuota() { await driver.click(trigger('quota')); await opened('Lowest quota left'); }
async function capture(name, width, scale = 1) {
  for (let adjustment = 0; adjustment < 3; adjustment++) {
    const view = await driver.execute('return {width:innerWidth,height:innerHeight};');
    if (view.width === width && view.height === 850) break;
    const rect = await driver.request('GET', `/session/${driver.session}/window/rect`);
    await driver.request('POST', `/session/${driver.session}/window/rect`, { width: rect.width + width - view.width, height: rect.height + 850 - view.height });
  }
  await driver.execute('window.__summaryFonts=[...document.querySelectorAll("body *")].map(node=>({node,style:node.getAttribute("style"),size:getComputedStyle(node).fontSize}));for(const item of window.__summaryFonts)item.node.style.fontSize=`${parseFloat(item.size)*arguments[0]}px`;scrollTo(0,0);return true;', [scale]);
  try {
    const viewport = await settleViewport(driver, width, 850);
    if (!viewport.ok) {
      evidence.layoutFailure = await driver.execute('return [...document.querySelectorAll("body *")].filter(node=>{const r=node.getBoundingClientRect();return r.width>0&&r.right>innerWidth+1&&!node.closest(".overview-table-scroll");}).slice(0,25).map(node=>({tag:node.tagName,class:node.className,width:node.getBoundingClientRect().width,text:node.textContent.slice(0,100)}));');
      await driver.screenshot(join(output, 'scaled-layout-failure.png'));
    }
    assert.equal(viewport.ok, true, JSON.stringify({ viewport, overflow: evidence.layoutFailure }));
    const panels = await driver.execute('return [...document.querySelectorAll(arguments[0])].filter(node=>node.getClientRects().length).map(node=>({left:node.getBoundingClientRect().left,right:node.getBoundingClientRect().right}));', [panel]);
    assert.equal(panels.length, 1);
    assert.ok(panels[0].left >= 0 && panels[0].right <= width + 1);
    if (width < 620) await driver.execute('document.querySelector(arguments[0]).scrollIntoView({block:"start",behavior:"instant"});return true;', [trigger('quota')]);
    await driver.screenshot(join(output, name));
    evidence.screenshots.push({ name, width, textScale: scale });
  } finally {
    await driver.execute('for(const item of window.__summaryFonts){if(item.style===null)item.node.removeAttribute("style");else item.node.setAttribute("style",item.style);}delete window.__summaryFonts;return true;');
  }
}
await mkdir(output, { recursive: true, mode: 0o700 });
process.once('exit', () => { driver?.kill('SIGKILL'); stopGroup(fixture?.bus, 'SIGKILL'); });
try {
  evidence.applicationSha256 = await hash(application);
  evidence.sourceCommit = execFileSync('git', ['rev-parse', 'HEAD'], { encoding: 'utf8' }).trim();
  evidence.sourceDirty = Boolean(execFileSync('git', ['status', '--porcelain'], { encoding: 'utf8' }).trim());
  evidence.sourceHashes = {};
  const sources = execFileSync('rg', ['--files', 'apps/desktop/src', 'apps/desktop/src-tauri/src', 'crates'], { encoding: 'utf8' }).trim().split('\n');
  for (const path of [...sources, 'Cargo.lock', 'bun.lock', 'tests/native/summary-details.mjs', 'tests/native/webdriver.mjs', 'tests/native/private-fixture.mjs', 'tests/native/viewport.mjs']) evidence.sourceHashes[path] = await hash(join(repository, path));
  fixture = await createFixture('summary-details');
  fixture.environment.HOME = join(fixture.root, 'synthetic-home');
  await mkdir(fixture.environment.HOME, { mode: 0o700 });
  driver = new NativeDriver(application, fixture.environment, join(output, 'driver.log'));
  await driver.start();
  await waitFor(() => driver.execute('return Boolean(document.querySelector("#workspace-passphrase")&&!document.querySelector("#workspace-passphrase").disabled);'), 'private setup');
  const password = 'OpenUsage synthetic summary details 2026';
  await driver.type('#workspace-passphrase', password);
  await driver.type('#workspace-confirmation', password);
  await driver.click('.unlock-form button[type=submit]');
  await waitFor(async () => (await call('bootstrap')).storage === 'ready', 'ready workspace');
  const profiles = [];
  for (let index = 0; index < 3; index++) {
    await driver.navigate('overview');
    await driver.click('[data-overview-add-account]');
    await waitFor(async () => (await call('list_profiles')).profiles.length === index + 1, 'Add account creates isolated profile');
    profiles.splice(0, profiles.length, ...(await call('list_profiles')).profiles);
  }
  await driver.navigate('overview');
  await waitFor(() => driver.execute('return document.querySelector(arguments[0])?.textContent==="Lowest quota leftNot available";', [trigger('quota')]), 'missing quota stays unavailable');
  const executable = join(fixture.root, 'synthetic-collection-provider.py');
  await copyFile(join(repository, 'crates/openusage-codex/tests/fixtures/synthetic-collection-provider.py'), executable);
  await chmod(executable, 0o700);
  await driver.navigate('settings');
  await driver.click('[data-provider-settings]'); await driver.click('[data-provider-manual]');
  await driver.type('#provider-executable', executable); await driver.click('.provider-inspection-form button[type=submit]');
  await waitFor(() => driver.execute('return Boolean(document.querySelector("#provider-executable-approved")&&!document.querySelector("#provider-executable-approved").disabled);'), 'synthetic source review');
  await driver.click('#provider-executable-approved'); await driver.click('.provider-approval-form button[type=submit]');
  await waitFor(() => driver.execute('return Boolean(document.querySelector(".provider-approval-result"));'), 'synthetic source approved');
  const home = join(dirname(fixture.database), 'profiles', profiles[0].id);
  await writeFile(join(home, 'synthetic-collection-config.json'), JSON.stringify({ mode: 'dashboard_success', requireLogin: true }), { mode: 0o600 });
  await driver.navigate('overview');
  await driver.click(`[data-overview-account="${profiles[0].id}"] td:last-child button:first-child`);
  await waitFor(async () => Boolean(await call('load_profile_quota', { profileId: profiles[0].id })), 'synthetic sign-in saves quota', 45000);
  await driver.navigate('overview');
  await waitFor(() => driver.execute('return document.querySelector(arguments[0])?.textContent==="Lowest quota left60%";', [trigger('quota')]), 'single saved window summary');
  assert.equal(await driver.execute('return document.querySelector(arguments[0]).textContent;', [trigger('accounts')]), 'Connected accounts1');
  const before = await call('load_profile_quota', { profileId: profiles[0].id });
  const providerLog = join(home, 'synthetic-collection-observations.jsonl');
  const providerBefore = await hash(providerLog);
  const keyBefore = await hash(join(dirname(fixture.database), 'database-key.v1'));
  await driver.execute('document.querySelector(arguments[0]).focus();return true;', [trigger('accounts')]);
  await key('\uE007'); await opened('Connected accounts');
  await key('\uE00C'); await closed('accounts');
  await key('\uE004');
  assert.equal(await driver.execute('return document.activeElement===document.querySelector(arguments[0]);', [trigger('quota')]), true);
  assert.notEqual(await driver.execute('return getComputedStyle(document.activeElement).outlineStyle;'), 'none');
  await key(' '); await opened('Lowest quota left');
  const details = await driver.execute('return document.querySelector(arguments[0]).textContent;', [`${panel} .detail-rows`]);
  assert.ok(details.includes(profiles[0].label) && details.includes('Seven-day') && details.includes('Synthetic test data') && details.includes('Calculation100% − 40% used'));
  await capture('summary-light-desktop.png', 1280);
  await capture('summary-light-narrow.png', 360);
  await driver.setAppearance('dark'); await openQuota();
  await capture('summary-dark-narrow-text.png', 360, 2);
  await driver.setAppearance('light'); await openQuota();
  await capture('summary-light-narrow-text.png', 360, 2);
  await key('\uE00C');
  await waitFor(() => driver.execute('return !document.querySelector(arguments[0]);', [panel]), 'details closed');
  await driver.execute('document.querySelector(arguments[0]).focus();return true;', [trigger('quota')]);
  await key('\uE007'); await opened('Lowest quota left');
  await key('\uE00C'); await closed('quota');
  const after = await call('load_profile_quota', { profileId: profiles[0].id });
  assert.deepEqual(after, before);
  assert.equal(await hash(providerLog), providerBefore);
  assert.equal(await hash(join(dirname(fixture.database), 'database-key.v1')), keyBefore);
  assert.equal(await hash(application), evidence.applicationSha256);
  for (const [path, digest] of Object.entries(evidence.sourceHashes)) assert.equal(await hash(join(repository, path)), digest, `Source drift: ${path}`);
  evidence.checks = { missingUnavailable: true, connectedProfilesOnly: true, keyboardDisclosure: true, singleOpenPanel: true, provenance: true, focusPreserved: true, noProviderRequests: true, savedQuotaUnchanged: true, protectedKeyUnchanged: true };
  evidence.result = 'passed';
} catch (error) {
  evidence.error = error.stack ?? String(error); process.exitCode = 1;
  if (driver?.session) try { await driver.screenshot(join(output, 'failure.png')); } catch {}
} finally {
  try { await driver?.stop(); } catch (error) { evidence.cleanupError = error.message; evidence.result = 'failed'; process.exitCode = 1; }
  if (fixture) { stopGroup(fixture.bus, 'SIGKILL'); await rm(fixture.root, { recursive: true, force: true }); }
  await writeFile(join(output, 'result.json'), JSON.stringify(evidence, null, 2) + '\n', { mode: 0o600 });
  console.log(`${evidence.requirement}: ${evidence.result}. Evidence: ${output}`);
  if (evidence.error) console.error(evidence.error);
}
