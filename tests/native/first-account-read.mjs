import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { chmod, copyFile, lstat, mkdir, readFile, readdir, readlink, rm, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { CODEX_SENTINEL, createFixture, stopGroup } from './private-fixture.mjs';
import { NativeDriver, waitFor } from './webdriver.mjs';

const harness = dirname(fileURLToPath(import.meta.url));
const repository = resolve(process.env.OPENUSAGE_NATIVE_SOURCE_ROOT ?? join(harness, '../..'));
assert.ok(process.env.OPENUSAGE_NATIVE_BINARY, 'Select an explicit frozen native-test application.');
const application = resolve(process.env.OPENUSAGE_NATIVE_BINARY);
const output = resolve(process.env.OPENUSAGE_NATIVE_EVIDENCE ?? join('/tmp', `openusage-first-account-read-${randomUUID()}`));
const providerPath = 'crates/openusage-codex/tests/fixtures/synthetic-collection-provider.py';
const providerSha = 'e88d9644b66d5cd0e46d51d365c32d8575176909531a92ecb580d3acf4b02aa8';
const passphrase = 'OpenUsage synthetic initial account reading 2026';
const hash = async path => createHash('sha256').update(await readFile(path)).digest('hex');
const evidence = {
  requirement: 'NATIVE-FIRST-ACCOUNT-READ-001', result: 'failed', startedAt: new Date().toISOString(),
  scope: 'Actual native-test GUI Add OpenAI account and synthetic device sign-in for two isolated profiles. Each successful matched login automatically collects one supported quota/activity pair without a Refresh click; selected saved views, exact values, per-profile process/protocol isolation and genuine close/restart cache are checked. Explicit Advanced approval is test-fixture provisioning only. No real OAuth, real credentials, existing credential-home inspection, production official-source approval, independent SQLCipher oracle, historical data, aggregation, or platform qualification.',
  steps: [], screenshots: [], closes: [], profiles: [],
};
let fixture, driver, workspace, executable, sequence = 0;
const profiles = [];
const home = profile => join(workspace, 'profiles', profile.id);
async function call(command, args = {}) {
  const reply = await driver.invoke(command, args);
  assert.equal(reply.ok, true, `${command}: ${JSON.stringify(reply)}`);
  return reply.value;
}
async function cached(profile) {
  return { quota: await call('load_profile_quota', { profileId: profile.id }), activity: await call('load_profile_activity', { profileId: profile.id }) };
}
async function observations(profile) {
  try { return (await readFile(join(home(profile), 'synthetic-collection-observations.jsonl'), 'utf8')).trim().split('\n').filter(Boolean).map(JSON.parse); }
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
function validatePair(value, profile) {
  assert.ok(value.quota && value.activity);
  assert.equal(value.quota.bindingId, value.activity.bindingId);
  assert.notEqual(value.quota.collectionRunId, value.activity.collectionRunId);
  assert.equal(value.quota.limits[0].primary.usedPercent, 25);
  assert.equal(value.quota.limits[0].primary.derivedRemainingPercent, 75);
  assert.equal(value.activity.summary.lifetimeTokens, '9007199254740993');
  assert.equal(value.activity.summary.peakDailyTokens, '9223372036854775807');
  assert.equal(value.activity.summary.currentStreakDays, '0');
  assert.equal(value.activity.summary.longestRunningTurnSec, null);
  for (const reading of Object.values(value)) {
    assert.equal(reading.profileId, profile.id);
    assert.equal(reading.providerVersion, '0.163.0');
    assert.equal(reading.sourceKind, 'test_fixture');
    assert.equal(reading.identityConfidence, 'provisional');
    assert.equal(reading.verifiedIdentityKey, null);
    assert.equal(reading.accepted, true);
    assert.equal(reading.coverage, 'unknown');
  }
}
function withoutFreshness(value) {
  const normalized = structuredClone(value);
  for (const reading of Object.values(normalized)) {
    delete reading.freshness;
    for (const bucket of reading.dailyUsageBuckets ?? []) delete bucket.freshness;
  }
  return normalized;
}
async function open(setup = false) {
  driver = new NativeDriver(application, fixture.environment, join(output, `driver-${++sequence}.log`));
  await driver.start();
  await waitFor(() => driver.execute('const input=document.querySelector("#workspace-passphrase"),confirmation=document.querySelector("#workspace-confirmation");return Boolean(input&&!input.disabled&&(!arguments[0]||confirmation&&!confirmation.disabled));', [setup]), 'enabled private workspace fields');
  await driver.type('#workspace-passphrase', passphrase);
  if (setup) await driver.type('#workspace-confirmation', passphrase);
  await driver.click('.unlock-form button[type=submit]');
  await waitFor(async () => (await driver.invoke('bootstrap')).value?.storage === 'ready', 'unlocked native workspace');
  assert.equal((await call('bootstrap')).schemaVersion, 4);
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
  assert.fail('The owned frozen desktop process was not found.');
}
async function closeNormally() {
  const pid = await desktopPid();
  evidence.closes.push(JSON.parse(execFileSync('/usr/bin/python3', [join(harness, 'x11-close.py'), String(pid)], { env: fixture.environment, encoding: 'utf8', timeout: 10000 })));
  await waitFor(async () => { try { await readlink(`/proc/${pid}/exe`); return false; } catch (error) { if (error.code === 'ENOENT') return true; throw error; } }, 'genuine native window close');
  driver.session = undefined;
  await driver.stop(); driver = undefined;
}
async function approveFixture(expectedProfiles = []) {
  await driver.navigate('settings');
  if (await driver.execute('return !document.querySelector("[data-provider-settings]").closest("details").open;')) await driver.click('[data-provider-settings]');
  if (await driver.execute('return !document.querySelector("[data-provider-manual]").closest("details").open;')) await driver.click('[data-provider-manual]');
  await driver.type('#provider-executable', executable);
  await driver.click('.provider-inspection-form button[type=submit]');
  await waitFor(() => driver.execute('const box=document.querySelector("#provider-executable-approved");return Boolean(box&&!box.disabled&&box.getAttribute("aria-checked")==="false");'), 'explicit unchecked synthetic source approval');
  assert.match(await driver.execute('return document.querySelector(".provider-executable-review").textContent;'), /Synthetic test data/);
  await driver.click('#provider-executable-approved');
  await waitFor(() => driver.execute('return !document.querySelector(".provider-approval-form button[type=submit]").disabled;'), 'fixture consent enables approval');
  await driver.click('.provider-approval-form button[type=submit]');
  await waitFor(() => driver.execute('return Boolean(document.querySelector(".provider-approval-result"))&&!document.querySelector("#provider-executable-approved");'), 'fixture approved before adding accounts');
  assert.deepEqual((await call('list_profiles')).profiles.map(profile => profile.id).sort(), expectedProfiles.map(profile => profile.id).sort(), 'Explicit test fixture approval must not create or replace accounts.');
}
async function addAccount(label) {
  await driver.navigate('accounts');
  await driver.click('[data-account-connect]');
  let profile;
  await waitFor(async () => { profile = (await call('list_profiles')).profiles.find(item => item.label === label); return Boolean(profile); }, `primary Add creates ${label}`);
  profiles.push(profile);
  assert.equal(profile.connectionState, 'authentication_required');
  const metadata = await lstat(home(profile));
  assert.equal(metadata.isDirectory(), true); assert.equal(metadata.isSymbolicLink(), false); assert.equal(metadata.mode & 0o777, 0o700);
  assert.deepEqual(await cached(profile), { quota: null, activity: null });
  assert.deepEqual(await observations(profile), []);
  await writeFile(join(home(profile), 'synthetic-collection-config.json'), JSON.stringify({ mode: 'startup_snapshot', requireLogin: true }) + '\n', { mode: 0o600 });
  await waitFor(() => driver.execute('const button=document.querySelector(arguments[0]);return Boolean(button&&!button.disabled&&button.getClientRects().length);', [`[data-login-start="${profile.id}"]`]), 'new account visible Sign in action');
  await driver.click(`[data-login-start="${profile.id}"]`);
  await waitFor(async () => { const value = await cached(profile); return Boolean(value.quota && value.activity); }, `automatic initial paired COMMIT for ${label}`, 45000);
  const saved = await cached(profile); validatePair(saved, profile);
  await driver.openProfileReadings(profile.id);
  await waitFor(() => driver.execute('const result=document.querySelector(arguments[0]);return Boolean(result&&result.getClientRects().length&&result.textContent.includes("Quota and activity readings saved.")&&!document.querySelector(arguments[1]));', [`[data-collection-profile="${profile.id}"]`, `[data-collection-cancel="${profile.id}"]`]), 'automatic initial read reaches existing saved result');
  await waitFor(() => driver.execute('return document.querySelector(arguments[0])?.textContent.trim()==="Connected";', [`[data-profile-id="${profile.id}"] .profile-connection-state`]), 'GUI connection row refreshed from storage');
  await savedGui(profile);
  const rows = await observations(profile), starts = rows.filter(row => row.kind === 'start' && row.arguments[0] === 'app-server');
  assert.equal(starts.length, 2, 'One login process and exactly one automatic combined collection process.');
  const methodSets = starts.map(child => rows.filter(row => row.pid === child.pid && row.kind === 'request').map(row => row.request.method));
  assert.deepEqual(methodSets, [
    ['initialize', 'initialized', 'account/login/start', 'account/read'],
    ['initialize', 'initialized', 'account/read', 'account/rateLimits/read', 'account/usage/read', 'account/read'],
  ]);
  assert.equal(rows.filter(row => row.kind === 'login-challenge-emitted').length, 1);
  assert.equal(rows.filter(row => row.kind === 'post-login-account-read').length, 1);
  assert.equal(await readFile(join(home(profile), 'synthetic-monitored-login-completed'), 'utf8'), 'Synthetic matching login completion. No credentials.\n');
  const runtimeHome = (await readFile(join(home(profile), 'synthetic-runtime-home'), 'utf8')).trim();
  assert.ok(runtimeHome.startsWith(`${home(profile)}/openusage-provider-`), 'Provider runtime HOME remains inside this exact fresh managed profile.');
  evidence.profiles.push({ id: profile.id, label, managedHome: home(profile), runtimeHome, starts, methods: methodSets, syntheticChallengeCount: 1, saved });
  evidence.steps.push({ profileId: profile.id, primaryAdd: true, explicitPerAccountSignIn: true, startupAccountSnapshot: true, automaticCombinedRead: true, refreshClicked: false, savedViewsVisible: true });
  return { profile, saved, observations: rows };
}
async function savedGui(profile) {
  await driver.openProfileReadings(profile.id);
  await waitFor(() => driver.execute('const quota=document.querySelector(arguments[0]),activity=document.querySelector(arguments[1]);return Boolean(quota?.getClientRects().length&&activity?.getClientRects().length&&quota.querySelector("[role=meter]")?.getAttribute("aria-valuenow")==="75"&&activity.querySelector("[data-activity-metric=lifetimeTokens]")?.textContent.trim()==="9007199254740993");', [`[data-quota-saved="${profile.id}"]`, `[data-activity-saved="${profile.id}"]`]), 'exact saved quota and activity visible');
}
async function overviewRows(expectedFreshness) {
  await driver.navigate('overview');
  for (const profile of profiles) {
    await waitFor(() => driver.execute('const row=document.querySelector(arguments[0]),quota=row?.querySelector("[data-overview-account-quota]"),observed=row?.querySelector("[data-overview-account-observed]"),meter=quota?.querySelector("[role=meter]");return Boolean(quota?.textContent.includes("75% left")&&meter?.getAttribute("aria-valuenow")==="75"&&meter.getAttribute("aria-label").includes("remaining (derived)")&&observed?.querySelector("time[datetime]")&&(arguments[1]==="Fresh"?!observed.textContent.includes("Freshness unknown"):observed.textContent.includes(arguments[1])));', [`[data-overview-account="${profile.id}"]`, expectedFreshness]), 'each saved account quota visible without selecting its profile');
  }
}
async function capture(name, selector) {
  await driver.execute('document.querySelector(arguments[0])?.scrollIntoView({block:"center"});return true;', [selector]);
  await driver.screenshot(join(output, name)); evidence.screenshots.push(name);
}
async function reaped(child) {
  await waitFor(() => { try { process.kill(child.pid, 0); return false; } catch (error) { if (error.code === 'ESRCH') return true; throw error; } }, 'owned synthetic child reaped');
  assert.throws(() => process.kill(-child.processGroup, 0), error => error.code === 'ESRCH');
}

await mkdir(output, { recursive: true, mode: 0o700 });
process.once('exit', () => { driver?.kill('SIGKILL'); stopGroup(fixture?.bus, 'SIGKILL'); });
try {
  evidence.application = { path: application, sha256: await hash(application) };
  assert.equal(await hash(join(repository, providerPath)), providerSha);
  evidence.sourceCommit = execFileSync('git', ['rev-parse', 'HEAD'], { cwd: repository, encoding: 'utf8' }).trim();
  evidence.harnessHashes = {};
  for (const path of ['first-account-read.mjs', 'webdriver.mjs', 'private-fixture.mjs', 'x11-close.py']) evidence.harnessHashes[path] = await hash(join(harness, path));
  fixture = await createFixture('first-account-read');
  fixture.environment.HOME = join(fixture.root, 'synthetic-user-home');
  await mkdir(fixture.environment.HOME, { mode: 0o700 });
  workspace = dirname(fixture.database);
  executable = join(fixture.root, 'synthetic-collection-provider.py');
  await copyFile(join(repository, providerPath), executable); await chmod(executable, 0o700);
  assert.equal(await hash(executable), providerSha);
  await open(true); await approveFixture();
  const first = await addAccount('Account A');
  await capture('first-account-auto-read-saved.png', `[data-collection-profile="${first.profile.id}"]`);
  const keySha = await hash(join(workspace, 'database-key.v1'));
  const second = await addAccount('Account B');
  assert.notEqual(first.profile.id, second.profile.id);
  assert.notEqual(first.saved.quota.bindingId, second.saved.quota.bindingId);
  assert.deepEqual(await cached(first.profile), first.saved, 'Adding/signing in/reading Account B preserves Account A full cached views.');
  assert.deepEqual(await observations(first.profile), first.observations, 'Account B must not execute in Account A managed home.');
  assert.notEqual(evidence.profiles[0].runtimeHome, evidence.profiles[1].runtimeHome);
  assert.deepEqual((await readdir(join(workspace, 'profiles'))).sort(), profiles.map(profile => profile.id).sort());
  assert.equal(await hash(join(workspace, 'database-key.v1')), keySha);
  await driver.navigate('overview'); await driver.selectVisibleOption('#overview-profile', 'Account B');
  await waitFor(() => driver.execute('return document.querySelector("[data-overview-lifetime]")?.getAttribute("data-exact-value")==="9007199254740993";'), 'Overview loads Account B automatic reading');
  await overviewRows('Fresh');
  await capture('second-account-auto-read-overview.png', '[data-overview-accounts-table]');
  await writeFile(join(home(second.profile), 'synthetic-collection-config.json'), JSON.stringify({ mode: 'activity_invalid', requireLogin: true }) + '\n', { mode: 0o600 });
  await driver.navigate('accounts'); await driver.openProfileReadings(second.profile.id);
  await driver.waitForProfileRefresh(second.profile.id);
  await waitFor(() => driver.execute('return !document.querySelector(arguments[0]).disabled;', [`[data-collection-read="${second.profile.id}"]`]), 'manual retry becomes available through the normal cooldown');
  await driver.click(`[data-collection-read="${second.profile.id}"]`);
  await waitFor(() => driver.execute('const result=document.querySelector(arguments[0]);return Boolean(result&&result.getAttribute("role")==="alert"&&!document.querySelector(arguments[1]));', [`[data-collection-profile="${second.profile.id}"]`, `[data-collection-cancel="${second.profile.id}"]`]), 'malformed provider response reaches an actionable failure');
  assert.deepEqual(withoutFreshness(await cached(second.profile)), withoutFreshness(second.saved), 'A failed pair must preserve both previous saved readings.');
  assert.deepEqual(await cached(first.profile), first.saved, 'A failure in Account B must preserve Account A.');
  assert.deepEqual(await observations(first.profile), first.observations);
  second.observations = await observations(second.profile);
  const failedStarts = second.observations.filter(row => row.kind === 'start' && row.arguments[0] === 'app-server');
  assert.equal(failedStarts.length, 3, 'Failure must not trigger an automatic retry loop.');
  await reaped(failedStarts.at(-1));
  await savedGui(second.profile);
  evidence.failureRecovery = { selectedProfile: second.profile.id, malformedActivityRefused: true, bothPreviousReadingsPreserved: true, otherProfileUnchanged: true, noAutomaticRetry: true };
  await closeNormally();
  for (const record of evidence.profiles) for (const child of record.starts) await reaped(child);
  await open();
  const restartedHashes = await liveHashes();
  assert.equal(await hash(join(workspace, 'database-key.v1')), keySha);
  for (const value of [first, second]) {
    const saved = await cached(value.profile); validatePair(saved, value.profile);
    assert.equal(saved.quota.freshness, 'unknown'); assert.equal(saved.activity.freshness, 'unknown');
    assert.deepEqual(withoutFreshness(saved), withoutFreshness(value.saved));
    await savedGui(value.profile);
    assert.deepEqual(await observations(value.profile), value.observations, 'Reopening saved views must not execute or automatically sign in/read.');
  }
  const monitoring = await call('load_monitoring');
  assert.equal(monitoring.state, 'stopped'); assert.ok(monitoring.profiles.every(profile => !profile.eligible && !profile.readingActive));
  assert.deepEqual(await liveHashes(), restartedHashes, 'Saved-cache navigation after restart preserves encrypted database/WAL/key bytes.');
  await driver.navigate('settings'); await driver.selectVisibleOption('#theme', 'Dark');
  const themedHashes = await liveHashes();
  await overviewRows('Freshness unknown');
  await capture('two-account-saved-after-restart.png', '[data-overview-accounts-table]');
  assert.deepEqual(await liveHashes(), themedHashes, 'Saved-cache navigation after the explicit theme change preserves encrypted database/WAL/key bytes.');
  assert.equal(await readFile(join(fixture.dirs.codex, 'synthetic-fixture.txt'), 'utf8'), CODEX_SENTINEL);
  assert.deepEqual(await readdir(fixture.dirs.codex), ['synthetic-fixture.txt']);
  const profilesBeforeReconnect = (await call('list_profiles')).profiles;
  const firstBeforeReconnect = await cached(first.profile);
  const firstLogBeforeReconnect = await observations(first.profile);
  const secondBeforeReconnect = await cached(second.profile);
  const secondLogBeforeReconnect = await observations(second.profile);
  await approveFixture(profilesBeforeReconnect); // Explicit synthetic test provisioning, not a production setup step.
  await writeFile(join(home(second.profile), 'synthetic-collection-config.json'), JSON.stringify({ mode: 'startup_deferred', requireLogin: true }) + '\n', { mode: 0o600 });
  await driver.navigate('accounts');
  await waitFor(() => driver.execute('const button=document.querySelector(arguments[0]);return Boolean(button&&!button.disabled&&button.getClientRects().length);', [`[data-profile-connect="${second.profile.id}"]`]), 'selected account row Refresh after test fixture reapproval');
  await driver.click(`[data-profile-connect="${second.profile.id}"]`);
  await waitFor(async () => { const value = await cached(second.profile); return Boolean(value.quota&&value.activity&&value.quota.collectionRunId!==secondBeforeReconnect.quota.collectionRunId&&value.activity.collectionRunId!==secondBeforeReconnect.activity.collectionRunId); }, 'explicit row Refresh saves a new pair using the existing synthetic login marker', 45000);
  await waitFor(() => driver.execute('const result=document.querySelector(arguments[0]);return Boolean(result&&result.getAttribute("role")==="status"&&result.textContent.includes("Quota and activity readings saved.")&&!document.querySelector(arguments[1]));', [`[data-collection-profile="${second.profile.id}"]`, `[data-collection-cancel="${second.profile.id}"]`]), 'reconnect reaches the saved result without a new login');
  const reconnected = await cached(second.profile); validatePair(reconnected, second.profile);
  assert.equal(reconnected.quota.bindingId, secondBeforeReconnect.quota.bindingId, 'Same managed home and account evidence retain the original binding.');
  assert.deepEqual(await cached(first.profile), firstBeforeReconnect);
  assert.deepEqual(await observations(first.profile), firstLogBeforeReconnect);
  assert.deepEqual((await call('list_profiles')).profiles, profilesBeforeReconnect, 'Refresh reuses the same profiles.');
  let additional = (await observations(second.profile)).slice(secondLogBeforeReconnect.length);
  const reconnectStarts = additional.filter(row => row.kind === 'start' && row.arguments[0] === 'app-server');
  assert.equal(reconnectStarts.length, 1);
  assert.deepEqual(additional.filter(row => row.kind === 'request').map(row => row.request.method), ['initialize', 'initialized', 'account/read', 'account/read', 'account/rateLimits/read', 'account/usage/read', 'account/read']);
  assert.equal(additional.some(row => row.kind.startsWith('login-')), false, 'Existing authorized managed home must not start a new login.');
  await reaped(reconnectStarts[0]);
  let afterReconnectMonitoring = await call('load_monitoring');
  assert.equal(afterReconnectMonitoring.state, 'stopped'); assert.ok(afterReconnectMonitoring.profiles.every(profile => !profile.eligible&&!profile.readingActive));
  await driver.openProfileOptions(second.profile.id);
  assert.equal(await driver.execute('return document.querySelector(arguments[0])?.disabled;', [`[data-monitoring-start="${second.profile.id}"]`]), true);
  await writeFile(join(home(second.profile), 'synthetic-collection-config.json'), JSON.stringify({ mode: 'signed_out', requireLogin: true }) + '\n', { mode: 0o600 });
  await driver.waitForProfileRefresh(second.profile.id);
  await waitFor(() => driver.execute('return !document.querySelector(arguments[0]).disabled;', [`[data-collection-read="${second.profile.id}"]`]), 'normal thirty-second manual cooldown after reconnect');
  await driver.click(`[data-profile-connect="${second.profile.id}"]`);
  await waitFor(() => driver.execute('const result=document.querySelector(arguments[0]);return Boolean(result&&result.getAttribute("role")==="alert"&&result.textContent.includes("signed out")&&!document.querySelector(arguments[1]));', [`[data-collection-profile="${second.profile.id}"]`, `[data-collection-cancel="${second.profile.id}"]`]), 'signed-out reconnect returns an actionable failure');
  await waitFor(() => driver.execute('const button=document.querySelector(arguments[0]);return Boolean(button&&!button.disabled&&button.getClientRects().length);', [`[data-login-start="${second.profile.id}"]`]), 'Sign in remains available for failed authorization');
  assert.deepEqual(withoutFreshness(await cached(second.profile)), withoutFreshness(reconnected), 'Signed-out reconnect retains both latest accepted readings.');
  assert.deepEqual(await cached(first.profile), firstBeforeReconnect);
  assert.deepEqual(await observations(first.profile), firstLogBeforeReconnect);
  additional = (await observations(second.profile)).slice(secondLogBeforeReconnect.length);
  const allReconnectStarts = additional.filter(row => row.kind === 'start' && row.arguments[0] === 'app-server');
  assert.equal(allReconnectStarts.length, 2, 'One explicit successful Refresh and one explicit signed-out Refresh, with no retry loop.');
  const failedStart = allReconnectStarts[1];
  assert.deepEqual(additional.filter(row => row.pid===failedStart.pid&&row.kind==='request').map(row => row.request.method), ['initialize', 'initialized', 'account/read']);
  assert.equal(additional.some(row => row.kind.startsWith('login-')), false);
  await reaped(failedStart);
  afterReconnectMonitoring = await call('load_monitoring');
  assert.equal(afterReconnectMonitoring.state, 'stopped'); assert.ok(afterReconnectMonitoring.profiles.every(profile => !profile.eligible&&!profile.readingActive));
  assert.equal(await hash(join(workspace, 'database-key.v1')), keySha);
  assert.equal((await call('list_profiles')).profiles.find(profile => profile.id === second.profile.id).connectionState, 'authentication_required');
  await waitFor(() => driver.execute('return document.querySelector(arguments[0])?.textContent.trim()==="Not connected";', [`[data-profile-id="${second.profile.id}"] .profile-connection-state`]), 'signed-out failure updates the durable and visible account status');
  for (const profile of profiles) {
    const selector = `[data-profile-id="${profile.id}"] [data-profile-action="readings"]`;
    if (await driver.execute('return document.querySelector(arguments[0]).getAttribute("aria-expanded")==="true";', [selector])) await driver.click(selector);
  }
  await capture('account-rows-dark-desktop.png', '.accounts-section');
  evidence.readability = await driver.execute('const button=document.querySelector("[data-login-start]"),style=getComputedStyle(button);return {bodyFont:getComputedStyle(document.body).fontSize,buttonFont:style.fontSize,buttonHeight:button.getBoundingClientRect().height,buttonRadius:style.borderRadius,panelRadius:getComputedStyle(document.querySelector(".profile-list")).borderRadius};');
  await driver.setAppearance('light');
  for (let attempt = 0; attempt < 3; attempt++) {
    const size = await driver.execute('return {width:innerWidth,height:innerHeight};');
    if (size.width === 360 && size.height === 800) break;
    const rect = await driver.request('GET', `/session/${driver.session}/window/rect`);
    await driver.request('POST', `/session/${driver.session}/window/rect`, { width: rect.width + 360 - size.width, height: rect.height + 800 - size.height });
  }
  evidence.readability.narrow = await driver.execute('return {width:innerWidth,scrollWidth:document.documentElement.scrollWidth};');
  assert.equal(evidence.readability.narrow.width, 360);
  assert.ok(evidence.readability.narrow.scrollWidth <= 361, 'Larger controls must reflow without page overflow.');
  await capture('account-rows-light-360.png', '.accounts-section');
  evidence.reconnect = { explicitTestFixtureReapproval: true, selectedProfile: second.profile.id, sameProfilesAndBinding: true, successfulCombinedSessions: 1, signedOutSessions: 1, newLoginSessions: 0, previousReadingsRetainedOnFailure: true, otherAccountUnchanged: true, noMonitoringGrant: true, reportedConnectionAfterAuthFailure: 'authentication_required' };
  evidence.observations = Object.fromEntries(await Promise.all(profiles.map(async profile => [profile.id, await observations(profile)])));
  evidence.restart = { genuineWindowClose: true, bothSavedPairsPreserved: true, savedNavigationWithoutProviderExecution: true, noAutomaticRead: true, bothOverviewRowsVisible: true, theme: 'dark', unchangedDatabaseWalKeyBeforeExplicitReconnect: themedHashes, runtimeMonitoringGrantsAbsent: true };
  assert.equal(await hash(application), evidence.application.sha256);
  assert.equal(await hash(executable), providerSha);
  for (const [path, digest] of Object.entries(evidence.harnessHashes)) assert.equal(await hash(join(harness, path)), digest, `Harness drift: ${path}`);
  await closeNormally(); evidence.result = 'passed';
} catch (error) {
  evidence.error = error.stack ?? String(error); process.exitCode = 1;
  if (driver?.session && evidence.screenshots.length < 3) try { await capture('failure.png', '.app-shell'); } catch {}
} finally {
  try { await driver?.stop(); } catch (error) { evidence.cleanupError = error.message; evidence.result = 'failed'; process.exitCode = 1; }
  if (fixture) for (const profile of profiles) for (const child of (await observations(profile).catch(() => [])).filter(row => row.kind === 'start')) {
    try { const command = await readFile(`/proc/${child.pid}/cmdline`, 'utf8'); if (command.split('\0').includes(executable)) process.kill(-child.processGroup, 'SIGKILL'); }
    catch (error) { if (!['ENOENT', 'ESRCH'].includes(error.code)) { evidence.childCleanupError = error.message; evidence.result = 'failed'; process.exitCode = 1; } }
  }
  stopGroup(fixture?.bus, 'SIGKILL');
  if (fixture) try { await rm(fixture.root, { recursive: true, force: true }); evidence.privateFixtureRemoved = true; }
  catch (error) { evidence.cleanupError = error.message; evidence.result = 'failed'; process.exitCode = 1; }
  evidence.finishedAt = new Date().toISOString();
  await writeFile(join(output, 'result.json'), JSON.stringify(evidence, null, 2) + '\n', { mode: 0o600, flag: 'wx' });
  console.log(`${evidence.requirement}: ${evidence.result}. Evidence: ${output}`);
  if (evidence.error) console.error(evidence.error);
}
