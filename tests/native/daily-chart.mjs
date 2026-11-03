import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { chmod, copyFile, mkdir, readFile, readdir, readlink, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { CODEX_SENTINEL, createFixture, stopGroup } from './private-fixture.mjs';
import { NativeDriver, waitFor } from './webdriver.mjs';

const harness = dirname(fileURLToPath(import.meta.url));
const repository = resolve(process.env.OPENUSAGE_NATIVE_SOURCE_ROOT ?? join(harness, '../..'));
assert.ok(process.env.OPENUSAGE_NATIVE_BINARY, 'An explicit frozen native-test application is required.');
const application = resolve(process.env.OPENUSAGE_NATIVE_BINARY);
const output = resolve(process.env.OPENUSAGE_NATIVE_EVIDENCE ?? join(tmpdir(), `openusage-daily-chart-${randomUUID()}`));
const fixtureDigest = '91e780f49518a25ffa75963b2856f5951c5fe6f4d1e1c9040b41a4231d0750ae';
const password = 'OpenUsage synthetic daily chart workspace 2026';
// The Overview daily bar chart became the token heatmap on October 21, 2026. Saved daily totals now open from Usage in the detail panel.
const detailsSelector = '[data-workspace-view=usage] [data-activity-details-open]';
const chartSelector = detailsSelector;
const panelSelector = '[data-detail-panel][open]';
const hash = async path => createHash('sha256').update(await readFile(path)).digest('hex');
const expectedRows = [{ date: '2024-02-29', tokens: '9007199254740993' }, { date: '2024-03-01', tokens: '0' }];
const expectedChart = { maxTokens: '9007199254740993', bars: [{ date: '2024-02-29', heightBasisPoints: 10000 }, { date: '2024-03-01', heightBasisPoints: 0 }] };
const evidence = {
  requirement: 'NATIVE-SAVED-DAILY-CHART-001', result: 'failed', startedAt: new Date().toISOString(),
  scope: 'Actual native GUI activity collection and selected-profile saved daily chart from a pinned synthetic provider. Exact counts, zero geometry, keyboard details, themes and restart cache are covered. No live provider usage, verified identity or aggregates, complete date coverage, or other-platform claim.',
  screenshots: [], views: [], readOnlyChecks: [], nativeCloses: [],
};
let fixture, driver, workspace, executable, executionLog, profile, sequence = 0;

async function observations() {
  try { return (await readFile(executionLog, 'utf8')).trim().split('\n').filter(Boolean).map(JSON.parse); }
  catch (error) { if (error.code === 'ENOENT') return []; throw error; }
}
async function liveHashes() {
  const result = {};
  for (const path of [fixture.database, `${fixture.database}-wal`, join(workspace, 'database-key.v1')]) {
    try { result[path] = await hash(path); }
    catch (error) { if (error.code !== 'ENOENT') throw error; result[path] = null; }
  }
  assert.ok(result[fixture.database]);
  assert.ok(result[join(workspace, 'database-key.v1')]);
  return result;
}
async function load() {
  const response = await driver.invoke('load_profile_activity', { profileId: profile.id });
  assert.equal(response.ok, true);
  return response.value;
}
async function open(setup = false) {
  driver = new NativeDriver(application, fixture.environment, join(output, `driver-${++sequence}.log`));
  await driver.start();
  await waitFor(() => driver.execute('const field=document.querySelector("#workspace-passphrase"),confirmation=document.querySelector("#workspace-confirmation");return Boolean(field&&!field.disabled&&(!arguments[0]||confirmation&&!confirmation.disabled));', [setup]), 'enabled workspace fields');
  await driver.type('#workspace-passphrase', password);
  if (setup) await driver.type('#workspace-confirmation', password);
  await driver.click('.unlock-form button[type=submit]');
  await waitFor(async () => (await driver.invoke('bootstrap')).value?.storage === 'ready', 'unlocked synthetic workspace');
}
async function desktopPid() {
  const queue = [driver.child.pid];
  for (let count = 0; queue.length && count < 64; count++) {
    const pid = queue.shift();
    try {
      if (await readlink(`/proc/${pid}/exe`) === application) return pid;
      queue.push(...(await readFile(`/proc/${pid}/task/${pid}/children`, 'utf8')).trim().split(/\s+/).filter(Boolean).map(Number));
    } catch (error) { if (error.code !== 'ENOENT') throw error; }
  }
  assert.fail('The owned native application was not found.');
}
async function key(value) {
  if (!await driver.execute('return document.hasFocus();')) {
    const pid = await desktopPid();
    evidence.nativeFocus = JSON.parse(execFileSync('/usr/bin/python3', [join(harness, 'x11-close.py'), String(pid), 'focus'], { env: fixture.environment, encoding: 'utf8', timeout: 10000 }));
    await waitFor(() => driver.execute('return document.hasFocus();'), 'owned native window focus');
  }
  await driver.request('POST', `/session/${driver.session}/actions`, { actions: [{ type: 'key', id: 'daily-chart-keyboard', actions: [{ type: 'keyDown', value }, { type: 'keyUp', value }] }] });
}
async function closeNormally() {
  const pid = await desktopPid();
  evidence.nativeCloses.push(JSON.parse(execFileSync('/usr/bin/python3', [join(harness, 'x11-close.py'), String(pid)], { env: fixture.environment, encoding: 'utf8', timeout: 10000 })));
  await waitFor(async () => {
    try { await readlink(`/proc/${pid}/exe`); return false; }
    catch (error) { if (error.code === 'ENOENT') return true; throw error; }
  }, 'genuine window manager close');
  driver.session = undefined;
  await driver.stop();
  driver = undefined;
}
async function overview() {
  await driver.navigate('usage');
  await waitFor(() => driver.execute('const n=document.querySelector(arguments[0]);return Boolean(n&&n.getClientRects().length);', [detailsSelector]), 'saved daily totals on Usage');
}
async function keyboardDetails() {
  for (let presses = 0; presses < 80 && !await driver.execute('return document.activeElement===document.querySelector(arguments[0]);', [detailsSelector]); presses++) await key('\uE004');
  assert.equal(await driver.execute('return document.activeElement===document.querySelector(arguments[0]);', [detailsSelector]), true, 'Real Tab must reach saved activity Details.');
  await key('\uE007');
  await waitFor(() => driver.execute('return Boolean(document.querySelector(arguments[0])?.querySelector(".activity-days")?.getClientRects().length);', [panelSelector]), 'Enter opens daily totals');
  await key('\uE00C');
  await waitFor(() => driver.execute('return !document.querySelector(arguments[0])&&document.activeElement===document.querySelector(arguments[1]);', [panelSelector, detailsSelector]), 'Escape closes daily totals and returns focus');
  await key('\uE007');
  await waitFor(() => driver.execute('return Boolean(document.querySelector(arguments[0])?.querySelector(".activity-days"));', [panelSelector]), 'Enter reopens daily totals');
  evidence.keyboardDetails = { actualTab: true, enterOpen: true, escapeClose: true, focusReturned: true };
}
async function chartView() {
  const saved = await load();
  if (!await driver.execute('return Boolean(document.querySelector(arguments[0])?.querySelector(".activity-days"));', [panelSelector])) {
    await driver.click(detailsSelector);
    await waitFor(() => driver.execute('return Boolean(document.querySelector(arguments[0])?.querySelector(".activity-days"));', [panelSelector]), 'daily totals panel');
  }
  const view = await driver.execute(`
    const panel=document.querySelector(arguments[0]);
    const rows=[...panel.querySelectorAll('[data-activity-day]')].map(row=>({date:row.querySelector('time').dateTime,tokens:row.querySelector('.activity-count bdi').textContent,observedAt:row.querySelector('[data-activity-observed]')?.dateTime,freshness:row.querySelector('.activity-day-observed').textContent.split(' · ')[0]}));
    const rows2=[...panel.querySelectorAll('.detail-rows dt')].map(dt=>[dt.textContent,dt.nextElementSibling?.textContent]);
    return {rows,details:Object.fromEntries(rows2),notAvailable:panel.textContent.includes('Not available'),pageWidth:document.scrollingElement.scrollWidth,width:innerWidth,panelWidth:panel.getBoundingClientRect().width,panelScroll:panel.scrollWidth};
  `, [panelSelector]);
  assert.deepEqual(view.rows.map(({ date, tokens }) => ({ date, tokens })), expectedRows);
  assert.deepEqual(view.rows.map(row => row.observedAt), saved.dailyUsageBuckets.map(row => row.observedAtUtc));
  const freshness = { fresh: 'Current', stale: 'Stale', unknown: 'Freshness unknown' };
  assert.deepEqual(view.rows.map(row => row.freshness), saved.dailyUsageBuckets.map(row => freshness[row.freshness]));
  assert.equal(view.rows[1].tokens, '0', 'An observed zero day stays a zero, not missing.');
  assert.equal(view.details.Source, 'Synthetic test data');
  assert.ok(view.pageWidth <= view.width + 1, 'The page must not overflow.');
  assert.ok(view.panelScroll <= view.panelWidth + 1, 'The daily totals panel must not overflow.');
  return view;
}
async function resize(width, height = 900) {
  for (let attempt = 0; attempt < 3; attempt++) {
    const current = await driver.execute('return {width:innerWidth,height:innerHeight};');
    if (current.width === width && current.height === height) return;
    const outer = await driver.request('GET', `/session/${driver.session}/window/rect`);
    await driver.request('POST', `/session/${driver.session}/window/rect`, { width: outer.width + width - current.width, height: outer.height + height - current.height });
  }
  assert.deepEqual(await driver.execute('return {width:innerWidth,height:innerHeight};'), { width, height });
}
async function capture(name, theme, width) {
  await driver.setAppearance(theme);
  await resize(width);
  const before = await liveHashes(), starts = (await observations()).length;
  await overview();
  await driver.execute('document.querySelector(arguments[0]).scrollIntoView({block:"center"});return true;', [chartSelector]);
  const view = await chartView();
  await driver.screenshot(join(output, name));
  evidence.screenshots.push(name); evidence.views.push({ name, ...view });
  assert.deepEqual(await liveHashes(), before, 'Chart navigation must not write database, WAL or key bytes.');
  assert.equal((await observations()).length, starts, 'Chart navigation must not execute a provider.');
  evidence.readOnlyChecks.push({ name, hashes: before, noProviderExecution: true, scope: 'After intentional theme commit; cache/chart rendering only.' });
}

await mkdir(output, { recursive: true, mode: 0o700 });
process.once('exit', () => { driver?.kill('SIGKILL'); stopGroup(fixture?.bus, 'SIGKILL'); });
try {
  evidence.application = { path: application, sha256: await hash(application) };
  evidence.script = { path: fileURLToPath(import.meta.url), sha256: await hash(fileURLToPath(import.meta.url)) };
  fixture = await createFixture('daily-chart');
  fixture.environment.HOME = join(fixture.root, 'synthetic-user-home');
  await mkdir(fixture.environment.HOME, { mode: 0o700 });
  workspace = dirname(fixture.database);
  executable = join(fixture.root, 'synthetic-activity-provider.py');
  executionLog = join(fixture.root, 'synthetic-activity-observations.jsonl');
  await copyFile(join(repository, 'tests/native/fixtures/synthetic-activity-provider.py'), executable);
  await chmod(executable, 0o700);
  assert.equal(await hash(executable), fixtureDigest);
  evidence.fixture = { sha256: fixtureDigest, synthetic: true };
  await open(true);
  await driver.openProfileCreate();
  await driver.type('#profile-label', 'Synthetic daily chart');
  await driver.click('.profile-create-form button[type=submit]');
  await waitFor(async () => {
    const result = await driver.invoke('list_profiles');
    profile = result.value?.profiles.find(row => row.label === 'Synthetic daily chart');
    return Boolean(result.ok && profile);
  }, 'GUI-created synthetic profile');
  await driver.navigate('settings');
  if (await driver.execute('return !document.querySelector("[data-provider-settings]").closest("details").open;')) await driver.click('[data-provider-settings]');
  if (await driver.execute('const button=document.querySelector("[data-provider-manual]");return Boolean(button&&!button.closest("details").open);')) await driver.click('[data-provider-manual]');
  await driver.type('#provider-executable', executable);
  await driver.click('.provider-inspection-form button[type=submit]');
  await waitFor(() => driver.execute('const box=document.querySelector("#provider-executable-approved");return Boolean(box&&!box.disabled);'), 'reviewed pinned fixture source');
  await driver.click('#provider-executable-approved');
  await waitFor(() => driver.execute('return document.querySelector("#provider-executable-approved").getAttribute("aria-checked")==="true"&&!document.querySelector(".provider-approval-form button[type=submit]").disabled;'), 'explicit source consent');
  await driver.click('.provider-approval-form button[type=submit]');
  await waitFor(() => driver.execute('return document.activeElement?.matches(".provider-approval-result");'), 'GUI-approved fixture');
  await driver.openProfileOptions(profile.id);
  const read = `[data-activity-read="${profile.id}"]`;
  await waitFor(() => driver.execute('const button=document.querySelector(arguments[0]);return Boolean(button&&!button.disabled&&button.getClientRects().length);', [read]), 'visible enabled Read activity');
  await driver.click(read);
  let saved;
  await waitFor(async () => { saved = await load(); return Boolean(saved?.accepted); }, 'GUI-collected durable activity');
  assert.equal(saved.identityConfidence, 'provisional');
  assert.equal(saved.verifiedIdentityKey, null);
  assert.equal(saved.sourceKind, 'test_fixture');
  assert.deepEqual(saved.dailyUsageBuckets.map(({ date, tokens }) => ({ date, tokens })), expectedRows);
  assert.deepEqual(saved.dailyChart, expectedChart);
  const records = await observations();
  const reads = records.filter(row => row.kind === 'request' && row.request.method === 'account/usage/read');
  assert.equal(reads.length, 1);
  const child = records.find(row => row.kind === 'start' && row.pid === reads[0].pid);
  assert.ok(child);
  assert.equal(child.environment.CODEX_HOME, join(workspace, 'profiles', profile.id));
  await waitFor(() => {
    for (const target of [child.pid, -child.processGroup]) {
      try { process.kill(target, 0); return false; }
      catch (error) { if (error.code !== 'ESRCH') throw error; }
    }
    return true;
  }, 'owned activity child and process group reaped');
  evidence.guiCollection = { profileId: profile.id, collectionRunId: saved.collectionRunId, exactDailyRows: expectedRows, dailyChart: saved.dailyChart, oneActivityRead: true, reaped: true };
  await driver.setAppearance('light');
  await overview();
  const beforeKeyboard = await liveHashes();
  await keyboardDetails();
  await chartView();
  assert.deepEqual(await liveHashes(), beforeKeyboard);
  await capture('daily-chart-light-1200.png', 'light', 1200);
  await capture('daily-chart-dark-1200.png', 'dark', 1200);
  await capture('daily-chart-dark-360.png', 'dark', 360);
  await closeNormally();
  const executions = (await observations()).length;
  await open();
  const restartedHashes = await liveHashes();
  const restarted = await load();
  assert.equal(restarted.collectionRunId, saved.collectionRunId);
  assert.equal(restarted.freshness, 'unknown');
  assert.ok(restarted.dailyUsageBuckets.every(row => row.freshness === 'unknown'));
  assert.deepEqual(restarted.dailyChart, expectedChart);
  assert.deepEqual(await driver.invoke('check_profile_connection', { profileId: profile.id }), { ok: false, error: 'provider_executable_not_approved' });
  await overview();
  await keyboardDetails();
  await chartView();
  assert.deepEqual(await liveHashes(), restartedHashes);
  assert.equal((await observations()).length, executions);
  evidence.restart = { genuineWindowClose: true, sameCollectionRun: true, noExecutableApproval: true, noProviderExecution: true, freshness: 'unknown', readOnlyHashes: restartedHashes };
  await capture('daily-chart-restart-light-1200.png', 'light', 1200);
  assert.equal(await readFile(join(fixture.dirs.codex, 'synthetic-fixture.txt'), 'utf8'), CODEX_SENTINEL);
  assert.deepEqual(await readdir(fixture.dirs.codex), ['synthetic-fixture.txt']);
  evidence.observations = await observations();
  evidence.syntheticCodexSentinelPreserved = true;
  await closeNormally();
  evidence.result = 'passed';
} catch (error) {
  evidence.error = error.stack ?? String(error); process.exitCode = 1;
  if (driver?.session) try { await driver.screenshot(join(output, 'failure.png')); evidence.screenshots.push('failure.png'); } catch {}
} finally {
  try { await driver?.stop(); } catch (error) { evidence.cleanupError = error.message; evidence.result = 'failed'; process.exitCode = 1; }
  if (fixture) for (const child of (await observations().catch(() => [])).filter(row => row.kind === 'start')) {
    try {
      const command = await readFile(`/proc/${child.pid}/cmdline`, 'utf8');
      if (command.split('\0').includes(executable)) process.kill(-child.processGroup, 'SIGKILL');
    } catch (error) { if (!['ENOENT', 'ESRCH'].includes(error.code)) { evidence.childCleanupError = error.message; evidence.result = 'failed'; process.exitCode = 1; } }
  }
  stopGroup(fixture?.bus);
  if (fixture) try { await rm(fixture.root, { recursive: true, force: true }); evidence.privateFixtureRemoved = true; }
  catch (error) { evidence.cleanupError = error.message; evidence.result = 'failed'; process.exitCode = 1; }
  evidence.finishedAt = new Date().toISOString();
  await writeFile(join(output, 'result.json'), `${JSON.stringify(evidence, null, 2)}\n`, { mode: 0o600 });
}
