import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { chmod, copyFile, mkdir, readFile, readdir, readlink, rename, rm, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { fileURLToPath } from 'node:url';
import { CODEX_SENTINEL, createFixture, stopGroup } from './private-fixture.mjs';
import { NativeDriver, waitFor } from './webdriver.mjs';

const harness = dirname(fileURLToPath(import.meta.url));
const repository = resolve(process.env.OPENUSAGE_NATIVE_SOURCE_ROOT ?? join(harness, '../..'));
assert.ok(process.env.OPENUSAGE_NATIVE_BINARY && process.env.OPENUSAGE_LOGIN_VERIFIER, 'Select frozen application and oracle artifacts.');
const application = resolve(process.env.OPENUSAGE_NATIVE_BINARY), verifier = resolve(process.env.OPENUSAGE_LOGIN_VERIFIER);
const output = resolve(process.env.OPENUSAGE_NATIVE_EVIDENCE ?? join('/tmp', `openusage-monitoring-${randomUUID()}`));
const providerPath = 'crates/openusage-codex/tests/fixtures/synthetic-collection-provider.py';
const providerSha = 'afaeb19874b3b08b6439a9d5d82a6d2f873517697598bf695a2f92254a7f6bba';
const oracleSha = 'd4dcc3c1bce797976546b110a340f7a46aa922fded0fd2ba14459d1f54b3a1fb';
const passphrase = 'OpenUsage synthetic native monitoring workspace 2026';
const hash = async path => createHash('sha256').update(await readFile(path)).digest('hex');
const diagnostic = process.env.OPENUSAGE_MONITORING_STATUS_DIAGNOSTIC === '1';
const evidence = {
  requirement: 'NATIVE-RUNTIME-MONITORING-001', result: 'failed', startedAt: new Date().toISOString(),
  scope: 'Actual native synthetic login and explicit Start/Pause; a real 300-second scheduled interval, one accepted scheduled pair, an independent manual pair, and one pending scheduled pair cancelled before COMMIT. Independent encrypted-copy checks cover every column in sixteen schema-4 tables. Runtime eligibility is not inferred from saved connection status, a provider-owned synthetic marker, or metric bindings. No live authentication, real credentials, verified identity, populated daily or credit history, sleep injection, crash durability, OS scaling, or release-platform qualification.',
  steps: [], screenshots: [], oracles: [], closes: [],
};
let fixture, driver, workspace, executable, approvalId, sequence = 0;
const home = profile => join(workspace, 'profiles', profile.id);
const startButton = profile => `[data-monitoring-start="${profile.id}"]`;
const pauseButton = profile => `[data-monitoring-pause="${profile.id}"]`;
const collectionResult = profile => `[data-collection-profile="${profile.id}"]`;
async function call(command, args = {}) { const reply = await driver.invoke(command, args); assert.equal(reply.ok, true, `${command}: ${JSON.stringify(reply)}`); return reply.value; }
async function status(profile) { const value = await call('load_monitoring'); const selected = value.profiles.find(item => item.profileId === profile.id); assert.ok(selected); return { ...selected, globalState: value.state, readingRevision: value.readingRevision }; }
async function configure(profile, mode, requireLogin = true) { const path = join(home(profile), 'synthetic-collection-config.json'); await writeFile(`${path}.next`, JSON.stringify({ mode, requireLogin }) + '\n', { mode: 0o600 }); await rename(`${path}.next`, path); }
async function observations(profile) { try { return (await readFile(join(home(profile), 'synthetic-collection-observations.jsonl'), 'utf8')).trim().split('\n').filter(Boolean).map(JSON.parse); } catch (error) { if (error.code === 'ENOENT') return []; throw error; } }
async function appStarts(profile) { return (await observations(profile)).filter(row => row.kind === 'start' && row.arguments[0] === 'app-server'); }
async function reaped(child) { await waitFor(() => { try { process.kill(child.pid, 0); return false; } catch (error) { if (error.code === 'ESRCH') return true; throw error; } }, 'owned scheduled process reaped'); assert.throws(() => process.kill(-child.processGroup, 0), error => error.code === 'ESRCH'); }
async function saved(profile) { return { quota: await call('load_profile_quota', { profileId: profile.id }), activity: await call('load_profile_activity', { profileId: profile.id }) }; }
function pair(value, profile) { assert.equal(value.quota.profileId, profile.id); assert.equal(value.activity.profileId, profile.id); assert.equal(value.quota.bindingId, value.activity.bindingId); assert.notEqual(value.quota.collectionRunId, value.activity.collectionRunId); assert.equal(value.activity.summary.lifetimeTokens, '9007199254740993'); assert.equal(value.activity.summary.peakDailyTokens, '9223372036854775807'); assert.equal(value.quota.limits[0].primary.usedPercent, 25); for (const reading of Object.values(value)) { assert.equal(reading.sourceKind, 'test_fixture'); assert.equal(reading.identityConfidence, 'provisional'); assert.equal(reading.verifiedIdentityKey, null); } }
function corroboratePair(before, after, profile, reading) {
  const added = after.tables.collection_runs.filter(row => row.profile_id === profile.id && !before.tables.collection_runs.some(previous => previous.id === row.id));
  assert.equal(added.length, 2, 'Exactly two accepted method-specific runs must reach encrypted storage.');
  for (const [kind, method, table] of [['quota', 'account/rateLimits/read', 'current_quota_readings'], ['activity', 'account/usage/read', 'current_activity']]) {
    const receipt = reading[kind], rows = after.tables[table].filter(row => row.profile_id === profile.id); assert.equal(rows.length, 1);
    assert.equal(rows[0].binding_id, receipt.bindingId); assert.equal(rows[0].collection_run_id, receipt.collectionRunId);
    const run = added.find(row => row.id === receipt.collectionRunId); assert.ok(run); assert.equal(run.source_method, method); assert.equal(run.outcome, 'accepted');
  }
  const summary = after.tables.activity_summaries.find(row => row.id === reading.activity.collectionRunId); assert.ok(summary);
  assert.equal(summary.lifetime_tokens, reading.activity.summary.lifetimeTokens); assert.equal(summary.peak_daily_tokens, reading.activity.summary.peakDailyTokens);
  const quota = after.tables.quota_limits.find(row => row.collection_run_id === reading.quota.collectionRunId && row.limit_id === 'codex'); assert.ok(quota); assert.equal(String(quota.primary_used_percent), '25');
  for (const name of ['collection_runs', 'activity_summaries', 'activity_day_history', 'quota_snapshots', 'quota_limits', 'credit_snapshots', 'quota_reset_assessments']) for (const row of before.tables[name]) assert.ok(after.tables[name].some(current => JSON.stringify(current) === JSON.stringify(row)), `Immutable ${name} history must remain unchanged.`);
}
async function liveHashes() { const result = {}; for (const path of [fixture.database, `${fixture.database}-wal`, join(workspace, 'database-key.v1')]) try { result[path] = await hash(path); } catch (error) { if (error.code !== 'ENOENT') throw error; } return result; }
async function oracle(label) {
  const original = await liveHashes(), copy = join(fixture.root, `encrypted-monitoring-oracle-${randomUUID()}`);
  await mkdir(copy, { mode: 0o700 });
  try {
    const database = join(copy, 'monitoring.db'); await copyFile(fixture.database, database); await chmod(database, 0o600);
    try { await copyFile(`${fixture.database}-wal`, `${database}-wal`); await chmod(`${database}-wal`, 0o600); } catch (error) { if (error.code !== 'ENOENT') throw error; }
    const report = JSON.parse(execFileSync(verifier, ['database', database, join(workspace, 'database-key.v1'), '--profile-state'], { input: JSON.stringify({ passphrase }), env: fixture.environment, encoding: 'utf8', timeout: 60000, maxBuffer: 16 * 1024 * 1024 }));
    for (const key of ['verified', 'readOnly', 'encryptedPagesAuthenticated', 'originalProtectedKeyUsed', 'inputUnchanged', 'keyEnvelopeUnchanged', 'walRecordsUnchanged']) assert.equal(report[key], true, key);
    assert.equal(report.schemaVersion, 4); assert.equal(report.plaintextDiskStaging, false); assert.equal(Object.keys(report.state.tables).length, 16);
    assert.deepEqual(await liveHashes(), original);
    evidence.oracles.push({ label, actualSchemaVersion: 4, allSixteenTables: Object.fromEntries(Object.entries(report.state.tables).map(([name, rows]) => [name, { rows: rows.length, sha256: createHash('sha256').update(JSON.stringify(rows)).digest('hex') }])), derivedEncryptedCopy: true, persistentStateIsNotRuntimeMonitoringStatus: true, verification: { ...report, state: undefined } });
    return report.state;
  } finally { await rm(copy, { recursive: true, force: true }); }
}
async function open(setup = false) {
  driver = new NativeDriver(application, fixture.environment, join(output, `driver-${++sequence}.log`)); await driver.start();
  await waitFor(async () => driver.execute('const input=document.querySelector("#workspace-passphrase");return Boolean(input&&!input.disabled&&(!arguments[0]||document.querySelector("#workspace-confirmation")));', [setup]), 'native unlock fields');
  await driver.type('#workspace-passphrase', passphrase); if (setup) await driver.type('#workspace-confirmation', passphrase); await driver.click('.unlock-form button[type=submit]');
  await waitFor(async () => (await driver.invoke('bootstrap')).value?.storage === 'ready', 'unlocked monitoring workspace'); assert.equal((await call('bootstrap')).schemaVersion, 4);
}
async function ownedPid() { const queue = [driver.child.pid]; for (let count = 0; queue.length && count < 64; count++) { const pid = queue.shift(); try { if (await readlink(`/proc/${pid}/exe`) === application) return pid; queue.push(...(await readFile(`/proc/${pid}/task/${pid}/children`, 'utf8')).trim().split(/\s+/).filter(Boolean).map(Number)); } catch (error) { if (error.code !== 'ENOENT') throw error; } } assert.fail('No owned frozen desktop process.'); }
async function closeNormally() { const pid = await ownedPid(); evidence.closes.push(JSON.parse(execFileSync('/usr/bin/python3', [join(harness, 'x11-close.py'), String(pid)], { env: fixture.environment, encoding: 'utf8', timeout: 10000 }))); await waitFor(async () => { try { await readlink(`/proc/${pid}/exe`); return false; } catch (error) { if (error.code === 'ENOENT') return true; throw error; } }, 'normal native exit joins scheduler'); driver.session = undefined; await driver.stop(); driver = undefined; }
async function create(label) { await driver.openProfileCreate(); await driver.type('#profile-label', label); await driver.click('.profile-create-form button[type=submit]'); await waitFor(async () => driver.execute('const input=document.querySelector("#profile-label");return Boolean(input&&!input.disabled&&input.value==="");'), 'profile saved'); const value = (await call('list_profiles')).profiles.find(profile => profile.label === label); assert.ok(value); return value; }
async function approve(profile) {
  await driver.navigate('settings'); if (await driver.execute('return !document.querySelector("[data-provider-settings]").closest("details").open;')) await driver.click('[data-provider-settings]');
  if (await driver.execute('return !document.querySelector("[data-provider-manual]").closest("details").open;')) await driver.click('[data-provider-manual]');
  await driver.type('#provider-executable', executable); await driver.click('.provider-inspection-form button[type=submit]');
  await waitFor(async () => driver.execute('const checkbox=document.querySelector("#provider-executable-approved");return Boolean(checkbox&&!checkbox.disabled&&checkbox.getAttribute("aria-checked")==="false");'), 'unchecked source review');
  await driver.click('#provider-executable-approved'); await waitFor(async () => driver.execute('return document.querySelector("#provider-executable-approved")?.getAttribute("aria-checked")==="true"&&!document.querySelector(".provider-approval-form button[type=submit]").disabled;'), 'source consent');
  await driver.click('.provider-approval-form button[type=submit]'); await waitFor(async () => driver.execute('return document.activeElement?.matches(".provider-approval-result")&&document.querySelector("#provider-executable")?.value===""&&!document.querySelector("#provider-executable-approved");'), 'new approval committed');
  approvalId = (await call('check_profile_connection', { profileId: profile.id })).approvalId;
}
async function login(profile) {
  await configure(profile, 'success'); await driver.openProfileOptions(profile.id);
  await waitFor(async () => driver.execute('const button=document.querySelector(arguments[0]);return Boolean(button&&!button.disabled&&button.getClientRects().length);', [`[data-login-start="${profile.id}"]`]), 'available selected sign-in');
  await driver.click(`[data-login-start="${profile.id}"]`); await driver.click(`[data-login-consent="${profile.id}"] button[type=submit]`);
  await waitFor(async () => (await status(profile)).eligible, 'matched completed login grants runtime eligibility');
  await waitFor(async () => driver.execute('const node=document.querySelector(arguments[0]);return Boolean(node&&node.getClientRects().length&&!document.querySelector(arguments[1]));', [`.login-result[data-login-profile="${profile.id}"]`, `[data-login-cancel="${profile.id}"]`]), 'login terminal result');
  assert.equal((await call('list_profiles')).profiles.find(item => item.id === profile.id).connectionState, 'connected');
}
async function manual(profile) { const before = await saved(profile); await driver.openProfileReadings(profile.id); await waitFor(async () => driver.execute('const button=document.querySelector(arguments[0]);return Boolean(button&&!button.disabled);', [`[data-collection-read="${profile.id}"]`]), 'available manual refresh'); await driver.click(`[data-collection-read="${profile.id}"]`); await waitFor(async () => { const value = await saved(profile); return value.quota && value.activity && value.quota.collectionRunId !== before.quota?.collectionRunId && value.activity.collectionRunId !== before.activity?.collectionRunId; }, 'manual paired COMMIT'); await waitFor(async () => driver.execute('return document.activeElement===document.querySelector(arguments[0])&&!document.querySelector(arguments[1]);', [collectionResult(profile), `[data-collection-cancel="${profile.id}"]`]), 'manual result focus'); const value = await saved(profile); pair(value, profile); return value; }
async function start(profile) { await driver.openProfileOptions(profile.id); await waitFor(async () => driver.execute('const button=document.querySelector(arguments[0]);return Boolean(button&&!button.disabled&&button.getClientRects().length);', [startButton(profile)]), 'eligible Start monitoring'); await driver.click(startButton(profile)); await waitFor(async () => (await status(profile)).state === 'running', 'explicit monitoring start'); }
async function capture(name, selector, width = 1280, scale = 1) { for (let count = 0; count < 3; count++) { const viewport = await driver.execute('return {width:innerWidth,height:innerHeight};'); if (viewport.width === width && viewport.height === 850) break; const rect = await driver.request('GET', `/session/${driver.session}/window/rect`); await driver.request('POST', `/session/${driver.session}/window/rect`, { width: rect.width + width - viewport.width, height: rect.height + 850 - viewport.height }); } await driver.execute('document.documentElement.style.fontSize=arguments[0]===1?"":`${16*arguments[0]}px`;document.querySelector(arguments[1])?.scrollIntoView({block:"center"});return true;', [scale, selector]); await driver.screenshot(join(output, name)); evidence.screenshots.push(name); }
async function keyboardPause(profile) {
  await driver.openProfileOptions(profile.id); const manage = `[data-profile-id="${profile.id}"] [data-profile-action="options"]`; await driver.click(manage); await driver.click(manage);
  const focus = JSON.parse(execFileSync('/usr/bin/python3', [join(harness, 'x11-close.py'), String(await ownedPid()), 'focus'], { env: fixture.environment, encoding: 'utf8', timeout: 10000 }));
  await waitFor(async () => driver.execute('return document.hasFocus()&&document.activeElement===document.querySelector(arguments[0]);', [manage]), 'owned native window focused on Manage');
  let tabs = 0;
  for (; tabs < 16; tabs++) {
    if (await driver.execute('return document.activeElement===document.querySelector(arguments[0]);', [pauseButton(profile)])) break;
    await driver.execute('window.__monitorTabFrom=document.activeElement;return true;');
    await driver.request('POST', `/session/${driver.session}/actions`, { actions: [{ type: 'key', id: 'monitoring-tab', actions: [{ type: 'keyDown', value: '\uE004' }, { type: 'keyUp', value: '\uE004' }] }] }); await driver.request('DELETE', `/session/${driver.session}/actions`);
    await waitFor(async () => driver.execute('return document.activeElement!==window.__monitorTabFrom;'), 'native Tab focus transition', 1500);
  }
  assert.equal(await driver.execute('return document.activeElement===document.querySelector(arguments[0]);', [pauseButton(profile)]), true, 'Real Tab must reach the selected Pause control.');
  await capture('monitoring-pending-keyboard-pause.png', pauseButton(profile));
  assert.equal(await driver.execute('return document.activeElement===document.querySelector(arguments[0]);', [pauseButton(profile)]), true);
  await driver.request('POST', `/session/${driver.session}/actions`, { actions: [{ type: 'key', id: 'monitoring-pause-enter', actions: [{ type: 'keyDown', value: '\uE007' }, { type: 'keyUp', value: '\uE007' }] }] }); await driver.request('DELETE', `/session/${driver.session}/actions`);
  await waitFor(async () => { const value = await status(profile); return value.state === 'paused' && !value.readingActive; }, 'keyboard pause reaps exact scheduled request', 45000);
  evidence.keyboardPause = { method: 'Native WebDriver Tab and Enter', onlyOwnedWindow: focus, tabs, exactPauseDestination: true, noSyntheticDomKeyDispatch: true };
}

await mkdir(output, { recursive: true, mode: 0o700 });
process.once('exit', () => { driver?.kill('SIGKILL'); stopGroup(fixture?.bus, 'SIGKILL'); });
try {
  assert.equal(await hash(verifier), oracleSha); evidence.application = { path: application, sha256: await hash(application) }; evidence.verifier = { path: verifier, sha256: oracleSha, reusedReadOnlyQueryLogic: true };
  evidence.sourceCommit = execFileSync('git', ['rev-parse', 'HEAD'], { cwd: repository, encoding: 'utf8' }).trim();
  fixture = await createFixture('runtime-monitoring'); workspace = dirname(fixture.database); executable = join(fixture.root, 'synthetic-monitored-provider');
  await copyFile(join(repository, providerPath), executable); await chmod(executable, 0o700); assert.equal(await hash(executable), providerSha); evidence.syntheticProviderSha256 = providerSha;
  const sentinel = await hash(join(fixture.dirs.codex, 'synthetic-fixture.txt')); assert.equal(await readFile(join(fixture.dirs.codex, 'synthetic-fixture.txt'), 'utf8'), CODEX_SENTINEL);
  await open(true); const protectedKeyHash = await hash(join(workspace, 'database-key.v1'));
  for (const [command, args] of [['load_monitoring', { unexpected: true }], ['start_profile_monitoring', { profileId: randomUUID(), approvalId: randomUUID(), confirmed: true }], ['pause_profile_monitoring', { profileId: randomUUID(), unexpected: true }]]) assert.deepEqual(await driver.invoke(command, args), { ok: false, error: 'invalid_arguments' });
  const first = await create('Synthetic monitored profile'), other = await create('Synthetic expired sign-in');
  await configure(first, 'success', false); await approve(first); await manual(first);
  assert.equal((await status(first)).eligible, false); const seedStarts = (await appStarts(first)).length;
  assert.deepEqual(await driver.invoke('start_profile_monitoring', { profileId: first.id, approvalId }), { ok: false, error: 'monitoring_not_eligible' }); assert.equal((await appStarts(first)).length, seedStarts);
  await login(first);
  if (diagnostic) {
    evidence.requirement = 'NATIVE-MONITORING-STATUS-DIAGNOSTIC-001';
    evidence.scope = 'Bounded investigation of one post-login status command and the WebDriver async callback boundary. Not scheduled-monitoring acceptance.';
    const started = performance.now();
    console.log('Diagnostic: direct post-login refresh-status invocation begins.');
    try {
      const reply = await driver.invoke('load_profile_refresh_status', { profileId: first.id });
      evidence.steps.push({ directStatus: reply, elapsedMilliseconds: Math.round(performance.now() - started) });
      const idleSeconds = reply.ok ? reply.value.retryAfterSeconds : 0;
      console.log(`Diagnostic: preserving actual ${idleSeconds}s cooldown idle before the next status request.`);
      await delay(idleSeconds * 1000);
      const afterIdle = performance.now();
      try {
        const second = await driver.invoke('load_profile_refresh_status', { profileId: first.id });
        evidence.steps.push({ afterCooldownIdleStatus: second, idleSeconds, elapsedMilliseconds: Math.round(performance.now() - afterIdle) });
      } catch (error) {
        evidence.steps.push({ afterCooldownIdleStatusError: String(error), idleSeconds, elapsedMilliseconds: Math.round(performance.now() - afterIdle) });
      }
    } catch (error) {
      evidence.steps.push({ directStatusError: String(error), elapsedMilliseconds: Math.round(performance.now() - started) });
    }
    console.log('Diagnostic: independent page-side IPC probes begin.');
    await driver.execute('window.__monitorStatusProbe=[];for(const [command,args] of [["load_profile_refresh_status",{profileId:arguments[0]}],["bootstrap",{}],["load_monitoring",{}],["list_profiles",{}]]){window.__TAURI__.core.invoke(command,args).then(value=>window.__monitorStatusProbe.push({command,ok:true,retryAfterSeconds:value.retryAfterSeconds}),error=>window.__monitorStatusProbe.push({command,ok:false,error:String(error)}));}return true;', [first.id]);
    await waitFor(async () => (await driver.execute('return window.__monitorStatusProbe;')).length === 4, 'independent post-login IPC probes', 15000);
    const replies = await driver.execute('return window.__monitorStatusProbe;');
    assert.ok(replies.every(reply => reply.ok)); evidence.steps.push({ independentPageSideReplies: replies });
    await closeNormally(); evidence.result = 'passed';
  } else {
  await driver.waitForProfileRefresh(first.id); const beforeScheduled = await saved(first), beforeScheduledOracle = await oracle('seed before explicit monitoring start'), initialRevision = (await status(first)).readingRevision, offset = (await appStarts(first)).length;
  await start(first);
  await waitFor(async () => { const value = await status(first); return value.readingRevision !== initialRevision && !value.readingActive; }, 'first scheduled atomic COMMIT', 45000);
  const firstScheduled = await saved(first); pair(firstScheduled, first); assert.notEqual(firstScheduled.quota.collectionRunId, beforeScheduled.quota.collectionRunId);
  assert.equal((await call('bootstrap')).monitoring, 'running'); const firstStart = (await appStarts(first))[offset]; assert.ok(firstStart); await reaped(firstStart);
  await waitFor(async () => driver.execute('const quota=document.querySelector(arguments[0]),activity=document.querySelector(arguments[1]);return Boolean(quota&&activity&&quota.textContent.includes("75%")&&activity.textContent.includes("9007199254740993"));', [`[data-quota-saved="${first.id}"]`, `[data-activity-saved="${first.id}"]`]), 'background revision quietly reloads both real caches');
  assert.equal(await driver.execute('return document.activeElement===document.querySelector(arguments[0]);', [pauseButton(first)]), true, 'Background publication must not steal monitoring-control focus.');
  await driver.navigate('overview'); await capture('monitoring-running-overview-light.png', '.page-heading');
  const afterFirst = await oracle('first scheduled pair and existing history'); corroboratePair(beforeScheduledOracle, afterFirst, first, firstScheduled);
  await manual(first); const afterManual = await saved(first); assert.notEqual(afterManual.quota.collectionRunId, firstScheduled.quota.collectionRunId);
  corroboratePair(afterFirst, await oracle('manual pair after scheduled collection'), first, afterManual);
  const manualStart = (await appStarts(first))[offset + 1]; assert.ok(manualStart);
  const manualDelay = BigInt(manualStart.monotonicNs) - BigInt(firstStart.monotonicNs);
  assert.ok(manualDelay >= 0n && manualDelay < 30_000_000_000n, 'A scheduled read must not consume the manual cooldown.');
  evidence.steps.push({ actualManualStartAfterScheduledNanoseconds: manualDelay.toString(), scheduledDidNotConsumeManualCooldown: true });
  const beforeProfileEdits = await status(first), startsBeforeProfileEdits = (await appStarts(first)).length;
  const unrelatedView = await call('create_profile', { label: 'Synthetic unrelated profile' });
  const unrelated = unrelatedView.profiles.find(profile => profile.label === 'Synthetic unrelated profile'); assert.ok(unrelated);
  await call('rename_profile', { id: unrelated.id, label: 'Synthetic renamed profile' });
  await call('set_profile_archived', { id: unrelated.id, archived: true });
  const invalidRename = await driver.invoke('rename_profile', { id: first.id, label: '' });
  assert.equal(invalidRename.ok, false);
  const afterProfileEdits = await status(first);
  assert.equal(afterProfileEdits.state, 'running'); assert.equal(afterProfileEdits.eligible, true);
  assert.equal(afterProfileEdits.readingRevision, beforeProfileEdits.readingRevision);
  assert.equal((await appStarts(first)).length, startsBeforeProfileEdits);
  evidence.steps.push({ unrelatedCreateRenameArchiveAndFailedRenamePreserveRunningGrant: true, noProviderExecutionForProfileEdits: true });
  await configure(first, 'activity_hang');
  await login(other); await configure(other, 'signed_out'); await start(other);
  await waitFor(async () => { const value = await status(other); return value.state === 'paused' && !value.eligible && !value.readingActive; }, 'invalid authentication revokes only selected eligibility', 45000);
  assert.equal((await status(first)).state, 'running'); assert.equal((await status(other)).issue, 'sign_in_required');
  await waitFor(async () => driver.execute('const panel=document.querySelector(arguments[0]);return panel?.dataset.monitoringState==="paused"&&panel.textContent.includes("Sign in again");', [`[data-monitoring-profile="${other.id}"]`]), 'renderer displays selected-profile sign-in expiry');
  await capture('monitoring-expired-sign-in-light-360.png', `[data-profile-id="${other.id}"]`, 360);
  await driver.openProfileOptions(first.id); await driver.setAppearance('dark');
  await capture('monitoring-controls-dark-360.png', pauseButton(first), 360);
  await capture('monitoring-controls-dark-text200-360.png', pauseButton(first), 360, 2);
  await driver.execute('document.documentElement.style.fontSize="";return true;'); await driver.setAppearance('light');
  const beforePending = await oracle('before second scheduled start after independent auth pause');
  let pending; const deadline = process.hrtime.bigint() + 330_000_000_000n; let previousMinute = -1;
  while (process.hrtime.bigint() < deadline) {
    const starts = await appStarts(first);
    assert.ok(starts.length <= offset + 3, 'No replay burst or extra scheduled start.');
    const later = starts.slice(offset + 2);
    if (later.length) { const candidate = later[0]; if ((await observations(first)).some(row => row.pid === candidate.pid && row.kind === 'activity-pending')) { pending = candidate; break; } }
    const minute = Number((process.hrtime.bigint() - BigInt(firstStart.monotonicNs)) / 60_000_000_000n);
    if (minute !== previousMinute) { previousMinute = minute; console.log(`Native monitoring interval: minute ${minute}; next ${String((await status(first)).nextReadInSeconds)} seconds`); }
    await delay(1000);
  }
  assert.ok(pending, 'The second real scheduled start must reach pending activity.');
  const delta = BigInt(pending.monotonicNs) - BigInt(firstStart.monotonicNs); assert.ok(delta >= 300_000_000_000n, `Actual app-server interval below 300 seconds: ${delta}`);
  evidence.steps.push({ actualScheduledAppServerIntervalNanoseconds: delta.toString(), realWaitNoClockInjection: true, firstScheduledCommitted: true, secondScheduledCancelledBeforeCommit: true });
  assert.deepEqual(await oracle('pending second scheduled quota cannot publish half a pair'), beforePending); assert.equal((await status(first)).readingActive, true);
  await keyboardPause(first); await reaped(pending); assert.deepEqual(await oracle('explicit pause preserves every accepted table column'), beforePending);
  await capture('monitoring-paused-retained-light-360.png', `[data-profile-id="${first.id}"]`, 360); await capture('monitoring-paused-retained-text200-360.png', startButton(first), 360, 2); await driver.execute('document.documentElement.style.fontSize="";return true;');
  await closeNormally(); await open(); const restarted = await status(first); assert.equal(restarted.eligible, false); assert.equal(restarted.state, 'stopped'); assert.equal(restarted.globalState, 'stopped');
  assert.equal((await call('list_profiles')).profiles.find(profile => profile.id === first.id).connectionState, 'connected');
  assert.equal(await readFile(join(home(first), 'synthetic-monitored-login-completed'), 'utf8'), 'Synthetic matching login completion. No credentials.\n');
  await approve(first); assert.equal((await status(first)).eligible, false); const noStart = (await appStarts(first)).length;
  assert.deepEqual(await driver.invoke('start_profile_monitoring', { profileId: first.id, approvalId }), { ok: false, error: 'monitoring_not_eligible' }); assert.equal((await appStarts(first)).length, noStart);
  await driver.openProfileOptions(first.id); await capture('monitoring-restart-needs-sign-in-light-360.png', `[data-profile-id="${first.id}"]`, 360);
  await login(first); await configure(first, 'activity_hang'); const beforeExit = await oracle('before exit cancels pending fresh-runtime scheduled read'), exitOffset = (await appStarts(first)).length; await start(first);
  let exitChild; await waitFor(async () => { exitChild = (await appStarts(first))[exitOffset]; return Boolean(exitChild && (await observations(first)).some(row => row.pid === exitChild.pid && row.kind === 'activity-pending')); }, 'owned pending scheduled child before normal exit');
  await closeNormally(); await reaped(exitChild); assert.deepEqual(await oracle('normal exit joins scheduler without accepting pending half'), beforeExit);
  assert.deepEqual(await readdir(fixture.dirs.codex), ['synthetic-fixture.txt']); assert.equal(await hash(join(fixture.dirs.codex, 'synthetic-fixture.txt')), sentinel);
  assert.equal(await hash(join(workspace, 'database-key.v1')), protectedKeyHash);
  assert.equal(await hash(application), evidence.application.sha256); assert.equal(await hash(executable), providerSha);
  evidence.steps.push({ completedLoginRequiredDespiteMetricBindingAndPersistedConnected: true, providerOwnedSyntheticMarkerDoesNotGrantEligibilityAfterRestart: true, invalidAuthenticationPausesOnlySelectedProfile: true, exactScheduledPauseReaped: true, normalExitReapedWorkerChild: true, encryptedDatabaseKeyUnchangedByMonitoring: true });
  evidence.result = 'passed';
  }
} catch (error) {
  evidence.error = error.stack ?? String(error); process.exitCode = 1;
  if (driver?.session) try {
    await driver.execute('window.__monitorFailureIpc={};const profileId=document.querySelector("[data-profile-id]")?.getAttribute("data-profile-id");for(const [command,args] of [["bootstrap",{}],["load_monitoring",{}],["load_profile_refresh_status",{profileId}]]){window.__monitorFailureIpc[command]={pending:true};window.__TAURI__.core.invoke(command,args).then(()=>window.__monitorFailureIpc[command]={ok:true},error=>window.__monitorFailureIpc[command]={ok:false,error:String(error)});}return true;');
    await delay(1000); evidence.failureIpc = await driver.execute('return window.__monitorFailureIpc;');
  } catch (problem) { evidence.failureIpcProbeError = String(problem); }
  if (driver?.session) try { evidence.failureView = await driver.execute('return {activeTag:document.activeElement?.tagName,activeId:document.activeElement?.id,view:document.querySelector("[data-workspace-nav][aria-current=page]")?.getAttribute("data-workspace-nav"),profileCount:document.querySelectorAll("[data-profile-id]").length};'); await driver.screenshot(join(output, 'failure.png')); evidence.screenshots.push('failure.png'); } catch {}
} finally {
  try { await driver?.stop(); } catch (error) { evidence.cleanupError = error.message; evidence.result = 'failed'; process.exitCode = 1; }
  if (fixture) { stopGroup(fixture.bus, 'SIGKILL'); await rm(fixture.root, { recursive: true, force: true }); }
  evidence.finishedAt = new Date().toISOString(); await writeFile(join(output, 'result.json'), JSON.stringify(evidence, null, 2) + '\n', { mode: 0o600 }); console.log(`${evidence.requirement}: ${evidence.result}. Evidence: ${output}`); if (evidence.error) console.error(evidence.error);
}
