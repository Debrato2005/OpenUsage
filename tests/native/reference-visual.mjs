import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { chmod, copyFile, lstat, mkdir, readFile, readdir, rename, rm, writeFile } from 'node:fs/promises';
import { arch, platform, release, tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const repository = resolve(process.env.OPENUSAGE_NATIVE_SOURCE_ROOT ?? '/example/home/Projects/OpenUsage/.recovery/integration');
const harness = join(repository, 'tests/native');
const { NativeDriver, waitFor } = await import(pathToFileURL(join(harness, 'webdriver.mjs')));
const { CODEX_SENTINEL, createFixture, stopGroup } = await import(pathToFileURL(join(harness, 'private-fixture.mjs')));
const application = resolve(process.env.OPENUSAGE_NATIVE_BINARY ?? join(repository, 'target/debug/openusage-desktop'));
const output = resolve(process.env.OPENUSAGE_NATIVE_EVIDENCE ?? join(tmpdir(), `openusage-reference-visual-${randomUUID()}`));
const hash = async path => createHash('sha256').update(await readFile(path)).digest('hex');
const passphrase = 'OpenUsage synthetic reference visual workspace 2026';
const fixtures = {
  quota: { file: 'synthetic-quota-provider.py', digest: 'ffceb4436e3ddbaf93548e36aa057232fc62bbda7138e00f90f250480d17e25c' },
  activity: { file: 'synthetic-activity-provider.py', digest: '91e780f49518a25ffa75963b2856f5951c5fe6f4d1e1c9040b41a4231d0750ae' },
};
const evidence = {
  requirement: 'NATIVE-REFERENCE-WORKSPACE-VISUAL-001', result: 'failed', startedAt: new Date().toISOString(),
  scope: 'Actual native WebKit GUI with three synthetic profiles, explicit reviewed-fixture approval, saved quota and activity on separate provisional profiles, real navigation, mounted background read completion, both themes, 360px viewport, and test-side 200% text. Fixture data is not live provider usage. Appearance changes are explicit storage writes. No fake metrics, identity verification, combined totals, operating-system text zoom, scheduled monitoring, platform release, or production-provider acceptance.',
  platform: { os: platform(), release: release(), architecture: arch() }, screenshots: [], views: [], steps: [], violations: [],
};
let fixture, driver, workspace;
const readSelector = (profile, kind) => `[data-${kind}-read="${profile.id}"]`;
const savedSelector = (profile, kind) => `[data-${kind}-saved="${profile.id}"]`;
const disclosureSelector = profile => `[data-profile-id="${profile.id}"] [data-profile-action="readings"]`;

async function load(profile, kind) {
  const result = await driver.invoke(`load_profile_${kind}`, { profileId: profile.id }); assert.equal(result.ok, true); return result.value;
}
async function liveHashes() {
  const result = {};
  for (const path of [fixture.database, `${fixture.database}-wal`, join(workspace, 'database-key.v1')]) result[path] = await hash(path);
  return result;
}
async function create(label) {
  await driver.openProfileCreate(); await driver.type('#profile-label', label); await driver.click('.profile-create-form button[type=submit]');
  await waitFor(async () => driver.execute('const node=document.querySelector("#profile-label");return Boolean(node&&!node.disabled&&node.value==="");'), 'GUI profile creation completed');
  const result = await driver.invoke('list_profiles'); assert.equal(result.ok, true);
  const profile = result.value.profiles.find(value => value.label === label); assert.ok(profile); return profile;
}
async function closeCreation() {
  if (await driver.execute('return document.querySelector("[data-profile-add]")?.getAttribute("aria-expanded")==="true";')) await driver.click('[data-profile-add]');
  await waitFor(async () => driver.execute('return document.querySelector("[data-profile-add]")?.getAttribute("aria-expanded")==="false";'), 'closed creation panel');
}
async function approve(kind) {
  await driver.navigate('settings');
  if (await driver.execute('return !document.querySelector("[data-provider-settings]").closest("details").open;')) await driver.click('[data-provider-settings]');
  if (await driver.execute('const node=document.querySelector("[data-provider-manual]");return Boolean(node&&!node.closest("details").open);')) await driver.click('[data-provider-manual]');
  await driver.type('#provider-executable', fixtures[kind].path); await driver.click('.provider-inspection-form button[type=submit]');
  await waitFor(async () => driver.execute('const node=document.querySelector("#provider-executable-approved");return Boolean(node&&!node.disabled);'), 'reviewed synthetic executable');
  await driver.click('#provider-executable-approved');
  await waitFor(async () => driver.execute('return document.querySelector("#provider-executable-approved").getAttribute("aria-checked")==="true"&&!document.querySelector(".provider-approval-form button[type=submit]").disabled;'), 'explicit source consent');
  await driver.click('.provider-approval-form button[type=submit]');
  await waitFor(async () => driver.execute('return document.activeElement?.matches(".provider-approval-result");'), 'native source approval result');
  evidence.steps.push({ guiApprovedFixture: kind, sha256: fixtures[kind].digest });
}
async function read(profile, kind) {
  await driver.openProfileReadings(profile.id);
  await driver.openProfileOptions(profile.id);
  const before = await load(profile, kind);
  await waitFor(async () => driver.execute('const node=document.querySelector(arguments[0]);return Boolean(node&&!node.disabled&&node.getClientRects().length);', [readSelector(profile, kind)]), `visible enabled ${kind} read`);
  await driver.click(readSelector(profile, kind));
  await waitFor(async () => { const value = await load(profile, kind); return value && value.collectionRunId !== before?.collectionRunId; }, `GUI ${kind} saved`);
  await waitFor(async () => driver.execute('return document.activeElement?.matches(arguments[0]);', [savedSelector(profile, kind)]), `focused saved ${kind} result`);
  const value = await load(profile, kind); assert.equal(value.sourceKind, 'test_fixture'); assert.equal(value.accepted, true);
  evidence.steps.push({ guiSavedReading: kind, profileId: profile.id, collectionRunId: value.collectionRunId, observedAtUtc: value.observedAtUtc }); return value;
}
async function quotaConfig(profile, mode) {
  const path = join(fixture.root, 'synthetic-quota-provider-config.json');
  await writeFile(`${path}.next`, `${JSON.stringify({ defaultMode: 'multi', modesByHome: { [join(workspace, 'profiles', profile.id)]: mode } })}\n`, { mode: 0o600 });
  await rename(`${path}.next`, path);
}
async function backgroundReading(profile) {
  await driver.openProfileOptions(profile.id); await quotaConfig(profile, 'pending');
  await driver.execute('window.__referenceProviderNode=document.querySelector(arguments[0]);return true;', [`[data-profile-id="${profile.id}"] .profile-provider-check`]);
  await driver.click(readSelector(profile, 'quota'));
  await waitFor(async () => driver.execute('return Boolean(document.querySelector(arguments[0])&&!document.querySelector(arguments[0]).disabled);', [`[data-quota-cancel="${profile.id}"]`]), 'pending native quota read');
  let pending;
  await waitFor(async () => {
    let rows; try { rows = (await readFile(join(fixture.root, 'synthetic-quota-observations.jsonl'), 'utf8')).trim().split('\n').filter(Boolean).map(JSON.parse); } catch (error) { if (error.code === 'ENOENT') return false; throw error; }
    const starts = new Map(rows.filter(row => row.kind === 'start').map(row => [row.pid, row]));
    pending = rows.find(row => row.kind === 'quota-read-pending' && row.mode === 'pending' && starts.get(row.pid)?.environment.CODEX_HOME === join(workspace, 'profiles', profile.id));
    return Boolean(pending);
  }, 'actual selected-home fixture waits before collapsing the panel');
  await driver.click(disclosureSelector(profile));
  await waitFor(async () => driver.execute('return document.querySelector(arguments[0]).getAttribute("aria-expanded")==="false";', [disclosureSelector(profile)]), 'collapsed pending reading');
  await driver.navigate('overview');
  await driver.execute('window.__referenceNavigationFocus=document.activeElement;return true;');
  await quotaConfig(profile, 'multi');
  await waitFor(async () => Boolean(await load(profile, 'quota')), 'background quota read completed after navigation');
  assert.deepEqual(await driver.execute('return {sameProvider:window.__referenceProviderNode===document.querySelector(arguments[0]),sameFocus:window.__referenceNavigationFocus===document.activeElement,currentView:document.querySelector("[data-workspace-nav][aria-current=page]")?.getAttribute("data-workspace-nav"),closed:document.querySelector(arguments[1]).getAttribute("aria-expanded")==="false"};', [`[data-profile-id="${profile.id}"] .profile-provider-check`, disclosureSelector(profile)]), { sameProvider: true, sameFocus: true, currentView: 'overview', closed: true });
  evidence.steps.push({ pendingReadSurvivedCollapsedPanelAndNavigation: true, selectedHomeFixturePendingObserved: true, processId: pending.pid, sameProviderDomMounted: true, hiddenCompletionDidNotStealNavigationFocus: true });
}
async function resize(width, height = 900) {
  for (let attempt = 0; attempt < 3; attempt++) {
    const view = await driver.execute('return {width:innerWidth,height:innerHeight};');
    if (view.width === width && view.height === height) return;
    const rect = await driver.request('GET', `/session/${driver.session}/window/rect`);
    await driver.request('POST', `/session/${driver.session}/window/rect`, { width: rect.width + width - view.width, height: rect.height + height - view.height });
  }
  assert.deepEqual(await driver.execute('return {width:innerWidth,height:innerHeight};'), { width, height });
}
async function scale(multiplier) {
  const result = await driver.execute(`
    let restored=0;
    if(window.__referenceTextSizes){for(const item of window.__referenceTextSizes){if(item.value)item.node.style.setProperty('font-size',item.value,item.priority);else item.node.style.removeProperty('font-size');if(!item.hadStyle&&!item.node.style.length)item.node.removeAttribute('style');if(item.node.isConnected)restored++;}delete window.__referenceTextSizes;}
    const before=parseFloat(getComputedStyle(document.body).fontSize);
    if(arguments[0]!==1){window.__referenceTextSizes=[...document.querySelectorAll('body,body *')].map(node=>({node,value:node.style.getPropertyValue('font-size'),priority:node.style.getPropertyPriority('font-size'),hadStyle:node.hasAttribute('style'),computed:parseFloat(getComputedStyle(node).fontSize)}));for(const item of window.__referenceTextSizes)if(Number.isFinite(item.computed))item.node.style.setProperty('font-size',item.computed*arguments[0]+'px','important');}
    return {before,after:parseFloat(getComputedStyle(document.body).fontSize),restored};`, [multiplier]);
  if (multiplier === 2) assert.equal(result.after, result.before * 2);
  return result;
}
async function capture(name, view, theme, width = 1280, text = 1, target) {
  await scale(1); await resize(width); await driver.setAppearance(theme); await driver.navigate(view); await scale(text);
  await driver.execute('window.scrollTo(0,0);if(arguments[0])document.querySelector(arguments[0])?.scrollIntoView({block:"center"});return true;', [target ?? null]);
  await driver.request('POST', `/session/${driver.session}/execute/async`, { script: 'const done=arguments[arguments.length-1];document.fonts.ready.then(()=>requestAnimationFrame(()=>requestAnimationFrame(()=>done(true))));', args: [] });
  const data = await driver.execute(`
    const visible=node=>Boolean(node.getClientRects().length&&getComputedStyle(node).visibility!=='hidden');
    const scrollAncestor=node=>{for(let parent=node.parentElement;parent&&parent!==document.body;parent=parent.parentElement)if(['auto','scroll'].includes(getComputedStyle(parent).overflowX)&&parent.scrollWidth>parent.clientWidth+1)return true;return false;};
    const overflowing=[...document.querySelectorAll('body *')].filter(visible).filter(node=>!scrollAncestor(node)&&!node.closest('[aria-hidden=true],.skip-link')).filter(node=>{const rect=node.getBoundingClientRect();return rect.width>0&&(rect.left < -1||rect.right>innerWidth+1);}).slice(0,15).map(node=>({tag:node.tagName,id:node.id,className:typeof node.className==='string'?node.className:''}));
    const number = [...document.querySelectorAll('[data-overview-lifetime] bdi,.overview-meter-heading strong,.activity-count')].find(visible);
    const heading = document.querySelector('h1');
    const lifetime = [...document.querySelectorAll('[data-overview-lifetime]')].find(visible);
    const lifetimeExact = lifetime?.getAttribute('data-exact-value') ?? null;
    const lifetimeTitle = lifetime?.querySelector('bdi')?.getAttribute('title') ?? null;
    let numberDigitWidths = null;
    if (number) {
      const style = getComputedStyle(number), probe = document.createElement('span');
      probe.style.cssText = 'position:absolute;visibility:hidden;font-family:'+style.fontFamily+';font-size:'+style.fontSize+';font-weight:'+style.fontWeight+';font-variant-numeric:tabular-nums;white-space:pre';
      document.body.append(probe);
      numberDigitWidths = ['0000000000','1111111111','9999999999'].map(text=>{probe.textContent=text;return probe.getBoundingClientRect().width;});
      probe.remove();
    }
    return {lifetimeExact,lifetimeTitle,numberDigitWidths,numberFont:number?getComputedStyle(number).fontFamily:null,numberVariant:number?getComputedStyle(number).fontVariantNumeric:null,numberFontLoaded:number?document.fonts.check(getComputedStyle(number).fontWeight+' '+getComputedStyle(number).fontSize+' "DM Sans"'):null,uiFontLoaded:document.fonts.check('400 14px "DM Sans"'),heading:heading?{font:getComputedStyle(heading).fontFamily,size:getComputedStyle(heading).fontSize,weight:getComputedStyle(heading).fontWeight,lineHeight:getComputedStyle(heading).lineHeight}:null,viewport:{width:innerWidth,height:innerHeight},pageWidth:document.scrollingElement.scrollWidth,bodyFont:getComputedStyle(document.body).fontFamily,fontStatus:document.fonts.status,fonts:[...document.fonts].filter(face=>face.status==='loaded').map(face=>({family:face.family,weight:face.weight})),currentView:document.querySelector('[data-workspace-nav][aria-current=page]')?.getAttribute('data-workspace-nav'),visibleViewCount:[...document.querySelectorAll('[data-workspace-view]')].filter(visible).length,overflowing,overviewLifetime:document.querySelector('[data-overview-lifetime]')?.textContent,overviewProfile:document.querySelector('[data-overview]')?.getAttribute('data-overview-profile')};
  `);
  await driver.screenshot(join(output, name)); evidence.screenshots.push(name); evidence.views.push({ screenshot: name, view, theme, textScale: text, ...data });
  if (data.pageWidth > width + 1 || data.overflowing.length) evidence.violations.push({ screenshot: name, pageWidth: data.pageWidth, viewportWidth: width, overflowing: data.overflowing });
  assert.equal(data.currentView, view); assert.equal(data.visibleViewCount, 1); assert.equal(data.fontStatus, 'loaded'); assert.match(data.bodyFont, /DM Sans/); assert.equal(data.uiFontLoaded, true); if (data.numberFont) { assert.match(data.numberFont, /DM Sans/); assert.match(data.numberVariant, /tabular-nums/); assert.equal(data.numberFontLoaded, true); assert.ok(data.numberDigitWidths[0] > 0); assert.ok(Math.max(...data.numberDigitWidths) - Math.min(...data.numberDigitWidths) < 0.1, 'Actual native DM Sans digits must have equal width.'); }
  if (data.lifetimeExact !== null) assert.equal(data.lifetimeTitle, data.lifetimeExact, 'Compact lifetime retains its exact integer title.');
  await scale(1);
}
async function overviewSet(profile) {
  await driver.navigate('overview');
  await driver.selectVisibleOption('#overview-profile', profile.label);
  await waitFor(async () => driver.execute('const node=document.querySelector("[data-overview]");return node?.getAttribute("data-overview-profile")===arguments[0]&&node.getAttribute("aria-busy")==="false";', [profile.id]), 'selected profile overview loaded');
}
async function overviewCaptures(profile, kind) {
  await overviewSet(profile);
  if (kind === 'activity') {
    assert.equal(await driver.execute('return document.querySelector("[data-overview-lifetime]").getAttribute("data-exact-value");'), '9007199254740993');
    await driver.execute('document.querySelector(".overview-account-footer button").focus();return true;');
    for (const value of ['\uE004', '\uE007']) {
      await driver.request('POST', `/session/${driver.session}/actions`, { actions: [{ type: 'key', id: 'reading-details', actions: [{ type: 'keyDown', value }, { type: 'keyUp', value }] }] });
      if (value === '\uE004') assert.equal(await driver.execute('return document.activeElement===document.querySelector(".overview-source-details > button");'), true, 'Tab reaches Reading details from View usage.');
    }
    await waitFor(() => driver.execute('return document.querySelector(".overview-source-details > button").getAttribute("aria-expanded")==="true";'), 'keyboard expands exact reading details');
    assert.equal(await driver.execute('return [...document.querySelectorAll(".overview-source-details bdi")].some(node=>node.textContent==="9007199254740993"&&node.getClientRects().length>0);'), true, 'Exact lifetime remains visible through the keyboard disclosure.');
    assert.match(await driver.execute('return document.querySelector(".overview-source-details").textContent;'), /Unverified identity/);
    await driver.request('POST', `/session/${driver.session}/actions`, { actions: [{ type: 'key', id: 'reading-details', actions: [{ type: 'keyDown', value: '\uE007' }, { type: 'keyUp', value: '\uE007' }] }] });
    await waitFor(() => driver.execute('return document.querySelector(".overview-source-details > button").getAttribute("aria-expanded")==="false";'), 'keyboard closes reading details');
  }
  else assert.deepEqual(await driver.execute('return [...document.querySelectorAll("[data-overview-quota] [role=meter]")].map(node=>node.getAttribute("aria-valuenow")).slice(0,2);'), ['75', '0']);
  for (const theme of ['light', 'dark']) {
    await capture(`overview-${kind}-${theme}-desktop.png`, 'overview', theme);
    await capture(`overview-${kind}-${theme}-360.png`, 'overview', theme, 360);
    await capture(`overview-${kind}-${theme}-text200-360.png`, 'overview', theme, 360, 2, kind === 'activity' ? '[data-overview-activity]' : '[data-overview-quota]');
  }
}

await mkdir(output, { recursive: true, mode: 0o700 });
process.once('exit', () => { driver?.kill('SIGKILL'); stopGroup(fixture?.bus, 'SIGKILL'); });
try {
  assert.equal(platform(), 'linux');
  evidence.application = { path: application, sha256: await hash(application), commit: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: repository, encoding: 'utf8' }).trim() };
  evidence.script = { path: fileURLToPath(import.meta.url), sha256: await hash(fileURLToPath(import.meta.url)) };
  evidence.harnessHashes = {}; for (const name of ['webdriver.mjs', 'private-fixture.mjs', ...Object.values(fixtures).map(value => `fixtures/${value.file}`)]) evidence.harnessHashes[name] = await hash(join(harness, name));
  fixture = await createFixture('reference-ui'); fixture.environment.HOME = join(fixture.root, 'synthetic-home'); await mkdir(fixture.environment.HOME, { mode: 0o700 }); workspace = dirname(fixture.database);
  for (const value of Object.values(fixtures)) { value.path = join(fixture.root, value.file); await copyFile(join(harness, 'fixtures', value.file), value.path); await chmod(value.path, 0o700); assert.equal(await hash(value.path), value.digest); }
  driver = new NativeDriver(application, fixture.environment, join(output, 'driver.log')); await driver.start();
  await waitFor(async () => driver.execute('return ["#workspace-passphrase","#workspace-confirmation"].every(selector=>{const node=document.querySelector(selector);return Boolean(node&&!node.disabled&&node.getClientRects().length);});'), 'fresh synthetic fallback setup');
  await driver.type('#workspace-passphrase', passphrase); await driver.type('#workspace-confirmation', passphrase); await driver.click('.unlock-form button[type=submit]');
  await waitFor(async () => (await driver.invoke('bootstrap')).value?.storage === 'ready', 'native encrypted workspace ready');
  const quota = await create('Synthetic quota account'), activity = await create('Synthetic activity account'), background = await create('Synthetic background account');
  evidence.profiles = [quota, activity, background]; await closeCreation();
  await capture('accounts-empty-readings-light-desktop.png', 'accounts', 'light');
  await approve('quota'); const savedQuota = await read(quota, 'quota');
  await backgroundReading(background);
  await approve('activity'); const savedActivity = await read(activity, 'activity');
  assert.equal(savedActivity.summary.lifetimeTokens, '9007199254740993');
  assert.equal(savedQuota.limits[0].primary.derivedRemainingPercent, 75);
  await driver.navigate('accounts');
  for (const profile of [quota, activity, background]) if (await driver.execute('return document.querySelector(arguments[0]).getAttribute("aria-expanded")==="true";', [disclosureSelector(profile)])) await driver.click(disclosureSelector(profile));
  const beforeNavigation = await liveHashes();
  await driver.navigate('usage'); await driver.navigate('overview'); await driver.navigate('accounts');
  assert.deepEqual(await liveHashes(), beforeNavigation);
  evidence.steps.push({ navigationUsesRealControls: true, navigationDoesNotChangeEncryptedStorage: true, threeCompactAccountRows: true, separateSyntheticQuotaAndActivityIdentityScopes: true });
  await overviewCaptures(quota, 'quota'); await overviewCaptures(activity, 'activity');
  for (const theme of ['light', 'dark']) {
    await capture(`accounts-compact-${theme}-desktop.png`, 'accounts', theme);
    await capture(`accounts-compact-${theme}-text200-360.png`, 'accounts', theme, 360, 2);
  }
  await capture('accounts-rows-dark-text200-360.png', 'accounts', 'dark', 360, 2, '.profile-row-summary');
  await capture('settings-light-desktop.png', 'settings', 'light');
  await capture('settings-dark-text200-360.png', 'settings', 'dark', 360, 2);
  await driver.navigate('settings');
  if (await driver.execute('return !document.querySelector("[data-provider-settings]").closest("details").open;')) await driver.click('[data-provider-settings]');
  if (await driver.execute('const node=document.querySelector("[data-provider-manual]");return Boolean(node&&!node.closest("details").open);')) await driver.click('[data-provider-manual]');
  await driver.type('#provider-executable', fixtures.activity.path); await driver.click('.provider-inspection-form button[type=submit]');
  await waitFor(async () => driver.execute('const node=document.querySelector("#provider-executable-approved");return Boolean(node&&!node.disabled&&node.getAttribute("aria-checked")==="false");'), 'visible unchecked executable consent after real source review');
  assert.equal(await driver.execute('return document.querySelector(".provider-approval-form button[type=submit]").disabled;'), true);
  await capture('settings-consent-dark-text200-360.png', 'settings', 'dark', 360, 2, '.provider-approval-form');
  await driver.navigate('usage');
  await driver.selectVisibleOption('[aria-label="Usage profile"]', activity.label);
  await waitFor(async () => driver.execute('const node=document.querySelector(".usage-content [data-activity-saved]");return Boolean(node&&node.getAttribute("data-activity-saved")===arguments[0]);', [activity.id]), 'selected saved usage profile');
  await capture('usage-light-desktop.png', 'usage', 'light');
  await capture('usage-dark-text200-360.png', 'usage', 'dark', 360, 2);
  await capture('usage-values-dark-text200-360.png', 'usage', 'dark', 360, 2, '.usage-content .activity-summary');
  assert.equal(await readFile(join(fixture.dirs.codex, 'synthetic-fixture.txt'), 'utf8'), CODEX_SENTINEL); assert.deepEqual(await readdir(fixture.dirs.codex), ['synthetic-fixture.txt']);
  for (const value of Object.values(fixtures)) assert.equal(await hash(value.path), value.digest);
  assert.equal(await hash(application), evidence.application.sha256);
  for (const [name, digest] of Object.entries(evidence.harnessHashes)) assert.equal(await hash(join(harness, name)), digest);
  assert.equal(await hash(evidence.script.path), evidence.script.sha256);
  evidence.result = evidence.violations.length ? 'failed' : 'passed'; if (evidence.result !== 'passed') process.exitCode = 1;
} catch (error) {
  evidence.error = error.stack ?? String(error); process.exitCode = 1;
  if (driver?.session) try { await driver.screenshot(join(output, 'failure.png')); evidence.screenshots.push('failure.png'); } catch {}
} finally {
  try { await driver?.stop(); } catch (error) { evidence.cleanupError = error.message; evidence.result = 'failed'; process.exitCode = 1; }
  if (fixture) {
    for (const name of ['synthetic-quota-observations.jsonl', 'synthetic-activity-observations.jsonl']) try {
      const rows = (await readFile(join(fixture.root, name), 'utf8')).trim().split('\n').filter(Boolean).map(JSON.parse);
      (evidence.providerObservations ??= {})[name] = rows;
      for (const row of rows.filter(value => value.kind === 'start')) try { const cmd = (await readFile(`/proc/${row.pid}/cmdline`)).toString().split('\0'); if (row.pid === row.processGroup && Object.values(fixtures).some(value => cmd.includes(value.path))) process.kill(-row.processGroup, 'SIGKILL'); } catch (error) { if (!['ENOENT', 'ESRCH'].includes(error.code)) throw error; }
    } catch (error) { if (error.code !== 'ENOENT') throw error; }
    stopGroup(fixture.bus, 'SIGKILL'); await rm(fixture.root, { recursive: true, force: true });
  }
  evidence.finishedAt = new Date().toISOString(); await writeFile(join(output, 'result.json'), `${JSON.stringify(evidence, null, 2)}\n`, { mode: 0o600 });
  console.log(`${evidence.requirement}: ${evidence.result}. Evidence: ${output}`); if (evidence.error) console.error(evidence.error);
}
