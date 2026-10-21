import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { chmod, copyFile, mkdir, readFile, readlink, rm, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createFixture, stopGroup } from './private-fixture.mjs';
import { NativeDriver, waitFor } from './webdriver.mjs';

// Presentation and real native zoom acceptance with explicitly approved synthetic provider data.
// The separate production receipt establishes real authorized account collection and recovery.
const repository = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const application = resolve(process.env.OPENUSAGE_NATIVE_BINARY);
const output = resolve(process.env.OPENUSAGE_NATIVE_EVIDENCE);
const passphrase = 'OpenUsage synthetic overview workspace 2026';
const evidence = { requirement: 'NATIVE-TOKEN-HEATMAP-001', result: 'failed', scope: 'Native WebKitGTK with synthetic saved readings: selected-account daily/weekly/cumulative calendar heatmaps, exact zero versus missing, partial coverage, individual selection and refused unverified combined activity, read-only bytes and no provider requests from history controls; three primary views, contextual account actions, both reported quota windows, selected background refresh, true-black colors, themes, responsive layout, actual WebKit zoom and genuine restart. Not real OAuth, verified combined-account activity, Windows/macOS or release qualification.', views: [], screenshots: [] };
let driver, fixture, sequence = 0;
const hash = async path => createHash('sha256').update(await readFile(path)).digest('hex');
async function call(command, args) { const reply = await driver.invoke(command, args); assert.equal(reply.ok, true, `${command}: ${JSON.stringify(reply)}`); return reply.value; }
async function liveHashes() {
  const result = {};
  for (const path of [fixture.database, `${fixture.database}-wal`, join(dirname(fixture.database), 'database-key.v1')]) {
    try { result[path] = await hash(path); } catch (error) { if (error.code !== 'ENOENT') throw error; result[path] = null; }
  }
  return result;
}
async function providerHashes(profiles) {
  return Promise.all(profiles.map(async profile => {
    try { return await hash(join(dirname(fixture.database), 'profiles', profile.id, 'synthetic-collection-observations.jsonl')); }
    catch (error) { if (error.code !== 'ENOENT') throw error; return null; }
  }));
}
async function heatmap(profileId, mode = 'daily', periodDays = 365) {
  return call('load_profile_heatmap', { profileId, mode, periodDays });
}
async function visibleHeatmap(profileId, mode = 'daily', periodDays = 365) {
  await waitFor(() => driver.execute('const node=document.querySelector("[data-overview] [data-token-heatmap]");return node?.getAttribute("data-profile-id")===arguments[0]&&node.getAttribute("data-mode")===arguments[1]&&node.getAttribute("data-period-days")===String(arguments[2])&&Boolean(node.querySelector("[data-heatmap-grid]"));', [profileId, mode, periodDays]), `loaded ${mode} ${periodDays}-day heatmap for selected account`);
}
async function key(value) {
  await driver.request('POST', `/session/${driver.session}/actions`, { actions: [{ type: 'key', id: 'native-keyboard', actions: [{ type: 'keyDown', value }, { type: 'keyUp', value }] }] });
  await driver.request('DELETE', `/session/${driver.session}/actions`);
}
async function open(setup = false) {
  driver = new NativeDriver(application, fixture.environment, join(output, `driver-${++sequence}.log`));
  await driver.start();
  await waitFor(() => driver.execute('return Boolean(document.querySelector("#workspace-passphrase")&&!document.querySelector("#workspace-passphrase").disabled);'), 'unlock fields');
  await driver.type('#workspace-passphrase', passphrase);
  if (setup) await driver.type('#workspace-confirmation', passphrase);
  await key('\uE007');
  await waitFor(async () => (await call('bootstrap')).storage === 'ready', 'ready workspace');
}
async function close() {
  const queue = [driver.child.pid]; let pid;
  while (queue.length) {
    const current = queue.shift();
    try { if (await readlink(`/proc/${current}/exe`) === application) { pid = current; break; } queue.push(...(await readFile(`/proc/${current}/task/${current}/children`, 'utf8')).trim().split(/\s+/).filter(Boolean).map(Number)); } catch (error) { if (error.code !== 'ENOENT') throw error; }
  }
  assert.ok(pid);
  execFileSync('/usr/bin/python3', [join(repository, 'tests/native/x11-close.py'), String(pid)], { env: fixture.environment, timeout: 10000 });
  await waitFor(async () => { try { await readlink(`/proc/${pid}/exe`); return false; } catch (error) { if (error.code === 'ENOENT') return true; throw error; } }, 'genuine app exit');
  driver.session = undefined; await driver.stop(); driver = undefined;
}
async function resize(width, height = 920) {
  for (let attempt = 0; attempt < 3; attempt++) {
    const view = await driver.execute('return {width:innerWidth,height:innerHeight};');
    if (view.width === width && view.height === height) return;
    const rect = await driver.request('GET', `/session/${driver.session}/window/rect`);
    await driver.request('POST', `/session/${driver.session}/window/rect`, { width: rect.width + width - view.width, height: rect.height + height - view.height });
  }
  assert.equal(await driver.execute('return innerWidth;'), width);
}
async function capture(name) {
  await driver.execute('scrollTo(0,0);return true;'); await driver.screenshot(join(output, name)); evidence.screenshots.push(name);
  await waitFor(() => driver.execute('return document.fonts.status === "loaded";'), 'loaded UI font');
  const geometry = await driver.execute('const panels=[...document.querySelectorAll(".overview-panels>.overview-panel")].map(node=>{const r=node.getBoundingClientRect();return {left:r.left,top:r.top,width:r.width,height:r.height,scrollHeight:node.scrollHeight,layout:getComputedStyle(node.parentNode).display,children:[...node.children].map(child=>{const q=child.getBoundingClientRect(),s=getComputedStyle(child);return {class:child.className,height:q.height,top:q.top,width:q.width,marginTop:s.marginTop,marginBottom:s.marginBottom,position:s.position};})};});return {width:innerWidth,scrollWidth:document.documentElement.scrollWidth,height:innerHeight,bodyFont:getComputedStyle(document.body).fontSize,titleFont:getComputedStyle(document.querySelector("h1")).fontSize,panels};');
  assert.ok(geometry.scrollWidth <= geometry.width + 1); evidence.views.push({ name, ...geometry });
}
await mkdir(output, { recursive: true, mode: 0o700 });
process.once('exit', () => { driver?.kill('SIGKILL'); stopGroup(fixture?.bus, 'SIGKILL'); });
try {
  evidence.applicationSha256 = await hash(application);
  fixture = await createFixture('overview-workspace');
  fixture.environment.HOME = join(fixture.root, 'synthetic-home'); await mkdir(fixture.environment.HOME, { mode: 0o700 });
  await open(true);
  assert.deepEqual(await driver.execute('return [...document.querySelectorAll("nav[aria-label=Main] [data-workspace-nav]")].map(node=>node.getAttribute("data-workspace-nav"));'), ['overview', 'usage', 'settings']);
  const executable = join(fixture.root, 'synthetic-collection-provider.py');
  await copyFile(join(repository, 'crates/openusage-codex/tests/fixtures/synthetic-collection-provider.py'), executable); await chmod(executable, 0o700);
  await driver.navigate('settings'); await driver.click('[data-provider-settings]'); await driver.click('[data-provider-manual]');
  await driver.type('#provider-executable', executable); await driver.click('.provider-inspection-form button[type=submit]');
  await waitFor(() => driver.execute('return Boolean(document.querySelector("#provider-executable-approved")&&!document.querySelector("#provider-executable-approved").disabled);'), 'fixture review');
  await driver.click('#provider-executable-approved'); await driver.click('.provider-approval-form button[type=submit]');
  await waitFor(() => driver.execute('return Boolean(document.querySelector(".provider-approval-result"));'), 'fixture approved');
  const profiles = [];
  for (let index = 0; index < 5; index++) {
    await driver.navigate('overview'); await driver.click('[data-overview-add-account]');
    await waitFor(async () => (await call('list_profiles')).profiles.length === index + 1, 'overview Add creates one profile');
    const profile = (await call('list_profiles')).profiles.find(item => !profiles.some(old => old.id === item.id)); profiles.push(profile);
    await waitFor(() => driver.execute('const panel=document.querySelector("[data-account-panel]");return Boolean(panel?.open&&panel.querySelector(arguments[0])?.getClientRects().length);', [`[data-profile-id="${profile.id}"]`]), 'Overview Add opens its isolated profile dialog');
    assert.equal(await driver.execute('return document.querySelector("nav[aria-label=Main] [data-workspace-nav=overview]")?.getAttribute("aria-current");'), 'page');
  }
  for (const profile of profiles.slice(0, 2)) {
    const home = join(dirname(fixture.database), 'profiles', profile.id);
    await writeFile(join(home, 'synthetic-collection-config.json'), JSON.stringify({ mode: 'dashboard_success', requireLogin: true }) + '\n', { mode: 0o600 });
    await driver.navigate('overview');
    await driver.click(`[data-overview-account="${profile.id}"] td:last-child button:first-child`);
    await waitFor(() => driver.execute('const panel=document.querySelector("[data-account-panel]");return Boolean(panel?.open&&panel.querySelector(arguments[0])?.getClientRects().length);', [`[data-profile-id="${profile.id}"]`]), 'selected Sign in opens account dialog');
    assert.equal(await driver.execute('return [...document.querySelectorAll("[data-account-panel] [data-profile-id]")].filter(node=>node.getClientRects().length).length;'), 1);
    await waitFor(async () => Boolean(await call('load_profile_quota', { profileId: profile.id })) && Boolean(await call('load_profile_activity', { profileId: profile.id })), 'selected GUI sign-in automatically saves paired fixture readings', 45000);
  }
  await driver.navigate('overview');
  await waitFor(() => driver.execute('return document.querySelector("[data-overview-lifetime]")?.getAttribute("data-exact-value")==="9007199254740993";'), 'selected saved count');
  await waitFor(() => driver.execute('return document.querySelector(".overview-account-identity")?.textContent.includes("ChatGPT Plus");'), 'saved provider plan');
  assert.equal(await driver.execute('return document.querySelector("[data-account-panel]").open;'), false);
  assert.equal(await driver.execute('return document.querySelector("[data-account-refresh-message]");'), null, 'Automatic login collection does not add an Overview banner.');
  assert.deepEqual(await driver.execute('return [...document.querySelectorAll("[data-overview-quota] [role=meter]")].map(node=>Number(node.getAttribute("aria-valuenow")));'), [75, 60]);
  assert.deepEqual(await driver.execute('return [...document.querySelectorAll("[data-overview-window]")].map(node=>node.getAttribute("data-overview-window"));'), ['Primary window', 'Secondary window']);
  await visibleHeatmap(profiles[0].id, 'daily', 90);
  assert.equal(await driver.execute('return document.querySelector("[data-overview-quota-donut]")?.getAttribute("data-remaining");'), '75');
  await resize(1586, 992); await capture('overview-reference-light.png');
  assert.equal(await driver.execute('return document.querySelectorAll("[data-overview-combined-usage],[data-overview-day]").length;'), 0, 'The bar chart is replaced without inventing combined totals.');
  assert.equal(evidence.views.at(-1).panels.length, 3);
  assert.ok(evidence.views.at(-1).panels.every(panel => Math.abs(panel.top - evidence.views.at(-1).panels[0].top) < 1));
  await visibleHeatmap(profiles[0].id, 'daily', 90);
  const beforeHeatmap = await liveHashes(), providersBeforeHeatmap = await providerHashes(profiles);
  const daily = await heatmap(profiles[0].id);
  assert.equal(daily.profileId, profiles[0].id);
  assert.equal(daily.coverage, 'unknown');
  assert.equal(daily.cells.length, 365);
  const expectedDays = ['1200000', '2450000', '0', '1780000', '3100000', '920000', '2220000'];
  assert.deepEqual(daily.cells.filter(cell => cell.tokens !== null).map(cell => cell.tokens), expectedDays);
  assert.equal(daily.scaleMaximum, '3100000');
  assert.ok(daily.cells.some(cell => cell.tokens === null && cell.knownDays === 0 && cell.observedAtUtc === null));
  assert.equal(await driver.execute('return document.querySelectorAll("[data-overview] [data-heatmap-cell][data-value=missing]").length;'), 83);
  assert.equal(await driver.execute('return document.querySelectorAll("[data-overview] [data-heatmap-cell][data-value=zero]").length;'), 1);
  assert.equal(await driver.execute('return document.querySelector(arguments[0])?.getAttribute("data-exact-value");', ['[data-overview] [data-heatmap-cell="2026-10-17"]']), '0');
  assert.equal(await driver.execute('return document.querySelector(arguments[0])?.getAttribute("data-exact-value");', ['[data-overview] [data-heatmap-cell="2026-10-19"]']), '3100000');
  await driver.execute('document.querySelector(arguments[0]).focus();return true;', ['[data-overview] [data-heatmap-cell="2026-10-15"]']);
  await key('\uE015');
  await waitFor(() => driver.execute('return document.activeElement?.getAttribute("data-heatmap-cell")==="2026-10-16"&&document.querySelector("[data-overview] [data-heatmap-selected]")?.textContent.includes("2450000");'), 'native arrow key explores the next exact daily cell');
  await driver.execute('document.querySelector("[data-overview] [data-heatmap-values-toggle]").focus();return true;');
  await key('\uE007');
  await waitFor(() => driver.execute('return Boolean(document.querySelector("[data-overview] [data-heatmap-values]")?.getClientRects().length);'), 'actual keyboard opens exact heatmap values');
  const exactValues = await driver.execute('return document.querySelector("[data-overview] [data-heatmap-values]").textContent;');
  assert.match(exactValues, /3100000/);
  assert.match(exactValues, /2026-10-17/);
  await key('\uE007');
  await waitFor(() => driver.execute('return !document.querySelector("[data-overview] [data-heatmap-values]")?.getClientRects().length;'), 'keyboard closes exact values');
  for (const days of [30, 90, 180, 365]) {
    await driver.selectVisibleOption('[data-overview] [aria-label="Activity history range"]', `${days} days`);
    await visibleHeatmap(profiles[0].id, 'daily', days);
    assert.equal(await driver.execute('return document.querySelectorAll("[data-overview] [data-heatmap-cell]").length;'), days);
  }
  await driver.click('[data-overview] [data-heatmap-mode=weekly]');
  await visibleHeatmap(profiles[0].id, 'weekly');
  const weekly = await heatmap(profiles[0].id, 'weekly');
  assert.deepEqual(weekly.cells.filter(cell => cell.tokens !== null).map(cell => [cell.startDate, cell.endDate, cell.tokens, cell.knownDays, cell.expectedDays]), [
    ['2026-10-12', '2026-10-18', '5430000', 4, 7],
    ['2026-10-19', '2026-10-25', '6240000', 3, 7],
  ]);
  assert.equal(await driver.execute('return document.querySelector(arguments[0])?.getAttribute("data-exact-value");', ['[data-overview] [data-heatmap-cell="2026-10-12"]']), '5430000');
  await driver.click('[data-overview] [data-heatmap-mode=cumulative]');
  await visibleHeatmap(profiles[0].id, 'cumulative');
  const cumulative = await heatmap(profiles[0].id, 'cumulative');
  assert.equal(cumulative.cells.at(-1).tokens, '11670000');
  assert.equal(cumulative.cells.at(-1).knownDays, 7);
  assert.equal(cumulative.cells.at(-1).expectedDays, 365);
  assert.equal(await driver.execute('return [...document.querySelectorAll("[data-overview] [data-heatmap-cell]")].at(-1)?.getAttribute("data-exact-value");'), '11670000');
  await driver.click('[data-overview] [data-heatmap-mode=daily]');
  await visibleHeatmap(profiles[0].id);
  await driver.selectVisibleOption('[data-overview] [aria-label="Activity account"]', profiles[1].label);
  await visibleHeatmap(profiles[1].id);
  assert.equal(await driver.execute('return document.querySelector("[data-overview]").getAttribute("data-overview-profile");'), profiles[1].id, 'Activity account selects the shared Overview account.');
  const otherHeatmap = await heatmap(profiles[1].id);
  assert.equal(otherHeatmap.profileId, profiles[1].id);
  assert.notEqual(otherHeatmap.bindingId, daily.bindingId);
  assert.notEqual(otherHeatmap.collectionRunId, daily.collectionRunId);
  await driver.selectVisibleOption('#overview-profile', profiles[4].label);
  assert.equal(await heatmap(profiles[4].id), null);
  await waitFor(() => driver.execute('return document.querySelector("[data-overview] [data-token-heatmap]")?.getAttribute("data-profile-id")===arguments[0]&&document.querySelectorAll("[data-overview] [data-heatmap-cell]").length===0;', [profiles[4].id]), 'unconnected account does not inherit another account heatmap');
  await driver.selectVisibleOption('#overview-profile', profiles[0].label);
  await visibleHeatmap(profiles[0].id);
  await driver.click('[data-overview] [aria-label="Activity accounts"] button:nth-child(2)');
  await waitFor(() => driver.execute('return document.querySelector("[data-overview] [data-activity-scope]")?.getAttribute("data-activity-scope")==="combined"&&Boolean(document.querySelector("[data-overview] [data-combined-availability]"));'), 'Combined checks saved eligibility');
  assert.equal(await driver.execute('return document.querySelectorAll("[data-overview] [data-combined-heatmap] [role=checkbox]").length;'), 5);
  assert.equal(await driver.execute('return document.querySelectorAll("[data-overview] [data-combined-heatmap] [role=checkbox][aria-checked=true]").length;'), 2, 'Only connected accounts are selected by default.');
  const combinedIds = profiles.slice(0, 2).map(profile => profile.id);
  const combined = await call('load_combined_heatmap', { profileIds: combinedIds });
  assert.deepEqual(combined, { status: 'unavailable', includedProfileIds: [], exclusions: combinedIds.map(profileId => ({ profileId, reason: 'identity_unverified' })) });
  assert.equal(await driver.execute('return document.querySelectorAll("[data-overview] [data-heatmap-grid],[data-overview] [data-heatmap-cell]").length;'), 0, 'Ineligible accounts never produce a combined calendar or invented totals.');
  assert.equal(await driver.execute('return document.querySelectorAll("[data-overview] .combined-exclusions li").length;'), 2);
  await driver.click('[data-overview] .combined-account-selection label:nth-child(2) [role=checkbox]');
  await waitFor(() => driver.execute('return document.querySelectorAll("[data-overview] .combined-exclusions li").length===1&&document.querySelectorAll("[data-overview] [data-combined-heatmap] [role=checkbox][aria-checked=true]").length===1;'), 'unchecking an account updates Combined eligibility');
  assert.deepEqual(await call('load_combined_heatmap', { profileIds: [profiles[0].id] }), { status: 'unavailable', includedProfileIds: [], exclusions: [{ profileId: profiles[0].id, reason: 'identity_unverified' }] });
  await driver.click('[data-overview] .combined-account-selection label:nth-child(2) [role=checkbox]');
  await waitFor(() => driver.execute('return document.querySelectorAll("[data-overview] .combined-exclusions li").length===2&&document.querySelectorAll("[data-overview] [data-combined-heatmap] [role=checkbox][aria-checked=true]").length===2;'), 'checking the second account restores its exclusion');
  await driver.click('[data-overview] [aria-label="Activity accounts"] button:first-child');
  await visibleHeatmap(profiles[0].id, 'daily', 90);
  assert.equal(await driver.execute('return document.querySelector("[data-overview]").getAttribute("data-overview-profile");'), profiles[0].id, 'Returning to Individual preserves its selected account.');
  assert.deepEqual(await liveHashes(), beforeHeatmap, 'Calendar and combined-eligibility reads and controls must preserve encrypted database, WAL and key bytes.');
  assert.deepEqual(await providerHashes(profiles), providersBeforeHeatmap, 'History browsing must not make provider requests.');
  evidence.heatmap = { defaultDays: 90, knownDays: 7, missingDays: 83, explicitZeroDays: 1, dailyExactValues: expectedDays, weeklyExactTotals: ['5430000', '6240000'], cumulativeAvailableTotal: '11670000', coverageUnknown: true, keyboardExactValues: true, ranges: [30, 90, 180, 365], accountIsolation: true, individualAccountSelector: true, combinedSelectionControls: true, unverifiedCombinedRefused: true, databaseWalKeyUnchanged: true, noProviderRequests: true };
  await driver.click('.overview-source-details > button');
  assert.ok((await driver.execute('return document.querySelector(".overview-source-details").textContent;')).includes('9007199254740993'));
  await driver.click('.overview-source-details > button');
  await driver.click('.overview-account-footer > button');
  await waitFor(() => driver.execute('return document.querySelector("[data-workspace-view=usage] [role=combobox]")?.textContent.includes(arguments[0]);', [profiles[0].label]), 'View usage selects the requested account');
  await driver.selectVisibleOption('[aria-label="Usage profile"]', profiles[1].label);
  await driver.navigate('overview');
  await driver.click('.overview-account-footer > button');
  await waitFor(() => driver.execute('return document.querySelector("[data-workspace-view=usage] [role=combobox]")?.textContent.includes(arguments[0]);', [profiles[0].label]), 'Repeated View usage restores its account');
  evidence.usageSelection = { repeatedAccountActionWinsOverUsageSelection: true };
  await driver.navigate('overview');
  // The prominent row action starts the existing sign-in controller for only this profile.
  await writeFile(join(dirname(fixture.database), 'profiles', profiles[2].id, 'synthetic-collection-config.json'), JSON.stringify({ mode: 'login_pending', requireLogin: true }) + '\n', { mode: 0o600 });
  await driver.click(`[data-overview-account="${profiles[2].id}"] td:last-child button:first-child`);
  await waitFor(() => driver.execute('return Boolean(document.querySelector(arguments[0])?.getClientRects().length);', [`[data-login-cancel="${profiles[2].id}"]`]), 'selected account synthetic sign-in started');
  assert.equal(await driver.execute('return [...document.querySelectorAll("[data-login-cancel]")].filter(node=>node.getClientRects().length).length;'), 1);
  await driver.click(`[data-login-cancel="${profiles[2].id}"]`);
  await waitFor(() => driver.execute('return !document.querySelector(arguments[0]);', [`[data-login-cancel="${profiles[2].id}"]`]), 'selected sign-in cancelled');
  evidence.accountActions = { overviewAddCreatedFiveSeparateProfiles: true, selectedSignInStartedOnlyThirdAccount: true, cancellationCompleted: true, onlyThreePrimaryViews: true, addAndSignInUseSelectedDialog: true };
  await driver.navigate('overview');
  await driver.selectVisibleOption('#overview-profile', profiles[0].label);
  const selectedId = await driver.execute('return document.querySelector("[data-overview]").getAttribute("data-overview-profile");');
  assert.ok(profiles.slice(0, 2).some(profile=>profile.id===selectedId));
  await driver.waitForProfileRefresh(selectedId);
  const beforeRefresh = await call('load_profile_quota', { profileId: selectedId });
  const otherId = profiles.slice(0, 2).find(profile=>profile.id!==selectedId).id;
  await driver.waitForProfileRefresh(otherId);
  const otherBeforeRefresh = await call('load_profile_quota', { profileId: otherId });
  // The visible countdown updates separately from backend cooldown admission.
  await waitFor(() => driver.execute('return document.querySelector(arguments[0])?.disabled===false;', [`[data-overview-row-refresh="${otherId}"]`]), 'other account visible cooldown finishes before independent refresh');
  const held = join(dirname(fixture.database), 'profiles', selectedId, 'synthetic-dashboard-hold');
  await writeFile(held, 'Owned synthetic refresh hold. No credentials.\n', { mode: 0o600 });
  try {
    await driver.click(`[data-overview-refresh="${selectedId}"]`);
    await waitFor(() => driver.execute('const button=document.querySelector(arguments[0]);return button?.disabled&&button.textContent==="Refreshing…"&&button.getAttribute("aria-busy")==="true";', [`[data-overview-refresh="${selectedId}"]`]), 'selected Overview refresh shows progress');
    assert.equal(await driver.execute('return document.querySelector("[data-account-panel]").open;'), false);
    assert.equal(await driver.execute('return document.querySelector("nav[aria-label=Main] [data-workspace-nav=overview]")?.getAttribute("aria-current");'), 'page');
    assert.equal(await driver.execute('return document.querySelector(arguments[0])?.textContent;', [`[data-overview-row-refresh="${selectedId}"]`]), 'Refreshing…');
    assert.equal(await driver.execute('return document.querySelector(arguments[0])?.disabled;', [`[data-overview-row-refresh="${otherId}"]`]), false);
  } finally { await rm(held, { force: true }); }
  await waitFor(async () => (await call('load_profile_quota', { profileId: selectedId })).collectionRunId !== beforeRefresh.collectionRunId, 'selected Refresh commits new saved reading', 45000);
  await waitFor(() => driver.execute('return document.querySelector("[data-account-refresh-message]")?.textContent.includes("Readings updated.");'), 'Overview reports successful refresh');
  assert.equal(await driver.execute('return document.querySelector("[data-account-panel]").open;'), false);
  assert.equal((await call('load_profile_quota', { profileId: otherId })).collectionRunId, otherBeforeRefresh.collectionRunId);
  evidence.backgroundRefresh = { realCooldownRespected: true, selectedAccountProgress: true, otherAccountUnchanged: true, overviewStayedVisible: true, noDialogOpened: true, savedNewReading: true };
  await waitFor(() => driver.execute('const button=document.querySelector(arguments[0]);return button?.disabled&&/^Refresh in \\d+s$/.test(button.textContent);', [`[data-overview-refresh="${selectedId}"]`]), 'Overview exposes selected cooldown');
  evidence.backgroundRefresh.cooldownVisible = true;
  await driver.click('[aria-label="Dismiss account update"]');

  await driver.navigate('overview');
  await driver.setAppearance('dark'); await resize(1280); await capture('overview-laptop-dark.png');
  evidence.darkTheme = await driver.execute('return { background:getComputedStyle(document.documentElement).backgroundColor,header:getComputedStyle(document.querySelector(".workspace-header")).backgroundColor,foreground:getComputedStyle(document.body).color };');
  assert.deepEqual(evidence.darkTheme, { background: 'rgb(0, 0, 0)', header: 'rgb(0, 0, 0)', foreground: 'rgb(204, 204, 204)' });
  const beforeZoom = await driver.execute('return {width:innerWidth,dpr:devicePixelRatio};');
  await driver.selectVisibleOption('#display-scale', '125%');
  await waitFor(async () => (await driver.execute('return innerWidth;')) < beforeZoom.width, 'native zoom changes CSS viewport');
  evidence.nativeZoom = { before: beforeZoom, after: await driver.execute('return {width:innerWidth,dpr:devicePixelRatio};') };
  assert.equal((await call('get_display_scale')).percent, 125);
  assert.equal((await driver.invoke('set_display_scale', { percent: 999 })).ok, false);
  assert.equal((await call('get_display_scale')).percent, 125);
  const savedBeforeRestart = await call('load_profile_quota', { profileId: profiles[0].id });
  const heatmapBeforeRestart = await heatmap(profiles[0].id), providersBeforeRestart = await providerHashes(profiles);
  await close(); await open();
  assert.equal((await call('get_display_scale')).percent, 125);
  await waitFor(() => driver.execute('return document.querySelector("#display-scale")?.textContent.trim()==="125%";'), 'restored native scale shown');
  const savedAfterRestart = await call('load_profile_quota', { profileId: profiles[0].id });
  assert.equal(savedAfterRestart.collectionRunId, savedBeforeRestart.collectionRunId);
  assert.equal(savedAfterRestart.observedAtUtc, savedBeforeRestart.observedAtUtc);
  await visibleHeatmap(profiles[0].id, 'daily', 90);
  // Freshness uses a process-local monotonic observation and becomes unknown after restart.
  // Saved dates, exact counts, levels, coverage and source references must remain identical.
  assert.deepEqual(await heatmap(profiles[0].id), {
    ...heatmapBeforeRestart,
    cells: heatmapBeforeRestart.cells.map(cell => ({ ...cell, freshness: 'unknown' })),
  });
  assert.deepEqual(await providerHashes(profiles), providersBeforeRestart, 'Restart restores saved calendar activity without provider requests.');
  evidence.genuineRestart = { scaleRestored: true, savedReadingUnchanged: true, heatmapSavedValuesRestoredExactly: true, sessionFreshnessAfterRestart: 'unknown', noProviderRequests: true };
  await driver.execute('document.querySelector("#display-scale").focus();return true;');
  await key('\uE007');
  await waitFor(() => driver.execute('return [...document.querySelectorAll("[role=option]")].some(node=>node.getClientRects().length);'), 'scale keyboard popup');
  for (const value of ['\uE011', '\uE015', '\uE007']) await key(value);
  await waitFor(async () => (await call('get_display_scale')).percent === 100, 'keyboard scale selection');
  await resize(360, 800); await capture('overview-narrow-dark.png');
  assert.equal(await hash(application), evidence.applicationSha256);
  await close(); evidence.result = 'passed';
} catch (error) { evidence.error = error.stack ?? String(error); process.exitCode = 1; if (driver?.session) try { await driver.screenshot(join(output, 'failure.png')); } catch {} }
finally {
  try { await driver?.stop(); } catch {} stopGroup(fixture?.bus, 'SIGKILL');
  if (fixture) await rm(fixture.root, { recursive: true, force: true });
  await writeFile(join(output, 'result.json'), JSON.stringify(evidence, null, 2) + '\n', { mode: 0o600 });
  console.log(`${evidence.requirement}: ${evidence.result}`); if (evidence.error) console.error(evidence.error);
}
