import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { chmod, copyFile, mkdir, readFile, readdir, readlink, rename, rm, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { CODEX_SENTINEL, createFixture, stopGroup } from './private-fixture.mjs';
import { NativeDriver, waitFor } from './webdriver.mjs';

const harness = dirname(fileURLToPath(import.meta.url));
const repository = resolve(process.env.OPENUSAGE_NATIVE_SOURCE_ROOT ?? join(harness, '../..'));
assert.ok(process.env.OPENUSAGE_NATIVE_BINARY, 'Select the frozen native-test application explicitly.');
assert.ok(process.env.OPENUSAGE_LOGIN_VERIFIER, 'Select the frozen read-only profile-state verifier explicitly.');
const application = resolve(process.env.OPENUSAGE_NATIVE_BINARY);
const verifier = resolve(process.env.OPENUSAGE_LOGIN_VERIFIER);
const output = resolve(process.env.OPENUSAGE_NATIVE_EVIDENCE ?? join('/tmp', `openusage-collection-${randomUUID()}`));
const coreFixture = 'crates/openusage-codex/tests/fixtures/synthetic-collection-provider.py';
const fixtureDigest = 'e88d9644b66d5cd0e46d51d365c32d8575176909531a92ecb580d3acf4b02aa8';
const verifierDigest = 'd4dcc3c1bce797976546b110a340f7a46aa922fded0fd2ba14459d1f54b3a1fb';
const passphrase = 'OpenUsage synthetic combined collection workspace 2026';
const hash = async path => createHash('sha256').update(await readFile(path)).digest('hex');
const evidence = { requirement: 'NATIVE-COMBINED-COLLECTION-001', result: 'failed', startedAt: new Date().toISOString(), scope: 'Synthetic native combined collection: primary GUI success, cancellation, account reassociation and restart; typed IPC negative boundaries and capability isolation; independent comparisons of all columns in sixteen SQLCipher tables using encrypted derived copies. No real provider account, scheduler eligibility, verified identity, live OAuth, or platform qualification.', steps: [], cases: [], screenshots: [], oracles: [], closes: [] };
let fixture, driver, workspace, executable, approvalId, sequence = 0;
const profiles = [];
const active = new Map();
const home = profile => join(workspace, 'profiles', profile.id);
const readSelector = profile => `[data-collection-read="${profile.id}"]`;
const cancelSelector = profile => `[data-collection-cancel="${profile.id}"]`;
const resultSelector = profile => `[data-collection-profile="${profile.id}"]`;
const args = receipt => ({ profileId: receipt.profileId, requestId: receipt.requestId });
async function call(command, parameters = {}) { const response = await driver.invoke(command, parameters); assert.equal(response.ok, true, `${command}: ${JSON.stringify(response)}`); return response.value; }
async function refused(command, parameters, error) { assert.deepEqual(await driver.invoke(command, parameters), { ok: false, error }); }
async function mode(profile, value) { const path = join(home(profile), 'synthetic-collection-config.json'); await writeFile(`${path}.next`, JSON.stringify({ mode: value }) + '\n', { mode: 0o600 }); await rename(`${path}.next`, path); }
async function rows(profile) { try { return (await readFile(join(home(profile), 'synthetic-collection-observations.jsonl'), 'utf8')).trim().split('\n').filter(Boolean).map(JSON.parse); } catch (error) { if (error.code === 'ENOENT') return []; throw error; } }
async function stage(profile, kind, offset = 0) { let found; await waitFor(async () => { found = (await rows(profile)).slice(offset).find(row => row.kind === kind); return Boolean(found); }, `owned collection ${kind}`); return found; }
async function reaped(row) { await waitFor(() => { try { process.kill(row.pid, 0); return false; } catch (error) { if (error.code === 'ESRCH') return true; throw error; } }, 'owned collection process reaped'); assert.throws(() => process.kill(-row.processGroup, 0), error => error.code === 'ESRCH'); }
async function liveHashes() { const result = {}; for (const path of [fixture.database, `${fixture.database}-wal`, join(workspace, 'database-key.v1')]) try { result[path] = await hash(path); } catch (error) { if (error.code !== 'ENOENT') throw error; } return result; }
async function oracle(label) {
  const before = await liveHashes(), copy = join(fixture.root, `encrypted-collection-oracle-${randomUUID()}`);
  await mkdir(copy, { mode: 0o700 });
  try {
    const database = join(copy, 'collection.db');
    await copyFile(fixture.database, database); await chmod(database, 0o600);
    try { await copyFile(`${fixture.database}-wal`, `${database}-wal`); await chmod(`${database}-wal`, 0o600); } catch (error) { if (error.code !== 'ENOENT') throw error; }
    const report = JSON.parse(execFileSync(verifier, ['database', database, join(workspace, 'database-key.v1'), '--profile-state'], { input: JSON.stringify({ passphrase }), env: fixture.environment, encoding: 'utf8', timeout: 60000, maxBuffer: 16 * 1024 * 1024 }));
    for (const key of ['verified', 'readOnly', 'encryptedPagesAuthenticated', 'originalProtectedKeyUsed', 'inputUnchanged', 'keyEnvelopeUnchanged', 'walRecordsUnchanged']) assert.equal(report[key], true, key);
    // This frozen oracle reports the actual schema; this caller requires exactly schema 4.
    assert.equal(report.schemaVersion, 4); assert.equal(report.plaintextDiskStaging, false); assert.equal(report.state.monitoring, 'stopped'); assert.equal(Object.keys(report.state.tables).length, 16);
    assert.deepEqual(await liveHashes(), before);
    evidence.oracles.push({ label, expectedSchemaVersion: 4, schemaExpectationEnforcedByCaller: true, actualSchemaVersion: report.schemaVersion, derivedEncryptedCopy: true, plaintextDiskStaging: false, tableHashes: Object.fromEntries(Object.entries(report.state.tables).map(([name, values]) => [name, { rows: values.length, sha256: createHash('sha256').update(JSON.stringify(values)).digest('hex') }])), verification: { ...report, state: undefined } });
    return report.state;
  } finally { await rm(copy, { recursive: true, force: true }); }
}
async function cached(profile) { return { quota: await call('load_profile_quota', { profileId: profile.id }), activity: await call('load_profile_activity', { profileId: profile.id }) }; }
function metrics(value) { const copy = structuredClone(value); for (const item of Object.values(copy)) if (item) delete item.freshness; return copy; }
function validatePair(value, profile) { assert.equal(value.quota.profileId, profile.id); assert.equal(value.activity.profileId, profile.id); assert.equal(value.quota.bindingId, value.activity.bindingId); assert.notEqual(value.quota.collectionRunId, value.activity.collectionRunId); assert.equal(value.activity.summary.lifetimeTokens, '9007199254740993'); assert.equal(value.activity.summary.peakDailyTokens, '9223372036854775807'); assert.equal(value.quota.limits[0].primary.usedPercent, 25); for (const item of [value.quota, value.activity]) { assert.equal(item.identityConfidence, 'provisional'); assert.equal(item.verifiedIdentityKey, null); assert.equal(item.sourceKind, 'test_fixture'); assert.equal(item.accepted, true); assert.equal(item.monitoring, 'stopped'); } }
function onlyCapabilityChanged(before, after, profile, capability) {
  const allowed = capability === 'activity' ? ['collection_runs', 'activity_summaries', 'current_activity', 'current_activity_days', 'activity_day_history'] : ['collection_runs', 'quota_snapshots', 'quota_limits', 'credit_snapshots', 'current_quota_readings', 'current_quota', 'quota_reset_assessments'];
  const left = structuredClone(before), right = structuredClone(after);
  const method = capability === 'activity' ? 'account/usage/read' : 'account/rateLimits/read';
  const added = after.tables.collection_runs.filter(row => row.profile_id === profile.id && !before.tables.collection_runs.some(previous => previous.id === row.id));
  assert.equal(added.length, 1, 'Exactly one supported method may add an accepted run.');
  assert.equal(added[0].source_method, method); assert.equal(added[0].outcome, 'accepted');
  for (const name of allowed) {
    const retained = row => row.profile_id !== profile.id || (name === 'collection_runs' && row.source_method !== method);
    left.tables[name] = left.tables[name].filter(retained);
    right.tables[name] = right.tables[name].filter(retained);
  }
  assert.deepEqual(right, left, 'Only supported capability rows for the selected profile may change.');
  for (const name of ['collection_runs', 'activity_summaries', 'activity_day_history', 'quota_snapshots', 'quota_limits', 'credit_snapshots', 'quota_reset_assessments']) for (const row of before.tables[name]) assert.ok(after.tables[name].some(current => JSON.stringify(current) === JSON.stringify(row)), `Immutable history retained: ${name}`);
}
async function open(setup = false) {
  driver = new NativeDriver(application, fixture.environment, join(output, `driver-${++sequence}.log`)); await driver.start();
  await waitFor(async () => driver.execute('const a=document.querySelector("#workspace-passphrase"),b=document.querySelector("#workspace-confirmation");return Boolean(a&&!a.disabled&&(!arguments[0]||b&&!b.disabled));', [setup]), 'native unlock fields');
  await driver.type('#workspace-passphrase', passphrase); if (setup) await driver.type('#workspace-confirmation', passphrase); await driver.click('.unlock-form button[type=submit]');
  await waitFor(async () => (await driver.invoke('bootstrap')).value?.storage === 'ready', 'unlocked schema-4 collection workspace');
  assert.equal((await call('bootstrap')).schemaVersion, 4);
}
async function ownedDesktopPid() {
  const queue = [driver.child.pid]; let pid;
  for (let count = 0; queue.length && count < 64; count++) { const candidate = queue.shift(); try { if (await readlink(`/proc/${candidate}/exe`) === application) { pid = candidate; break; } queue.push(...(await readFile(`/proc/${candidate}/task/${candidate}/children`, 'utf8')).trim().split(/\s+/).filter(Boolean).map(Number)); } catch (error) { if (error.code !== 'ENOENT') throw error; } }
  assert.ok(pid, 'Owned frozen desktop process required for genuine WM close.');
  return pid;
}
async function closeNormally() {
  const pid = await ownedDesktopPid();
  evidence.closes.push(JSON.parse(execFileSync('/usr/bin/python3', [join(harness, 'x11-close.py'), String(pid)], { env: fixture.environment, encoding: 'utf8', timeout: 10000 })));
  await waitFor(async () => { try { await readlink(`/proc/${pid}/exe`); return false; } catch (error) { if (error.code === 'ENOENT') return true; throw error; } }, 'genuine native process exit');
  driver.session = undefined; await driver.stop(); driver = undefined;
}
async function keyboardCancel(profile, child, offset) {
  const manage = `[data-profile-id="${profile.id}"] [data-profile-action="options"]`, cancel = cancelSelector(profile);
  await driver.openProfileOptions(profile.id);
  await driver.click(manage);
  await waitFor(async () => driver.execute('const node=document.querySelector(arguments[0]),cancel=document.querySelector(arguments[1]);return node?.getAttribute("aria-expanded")==="false"&&document.activeElement===node&&Boolean(cancel&&!cancel.disabled&&cancel.getClientRects().length);', [manage, cancel]), 'closed Manage retains trigger focus and visible active Cancel');
  process.kill(child.pid, 0);
  assert.equal((await rows(profile)).slice(offset).filter(row => row.kind === 'start' && row.arguments[0] === 'app-server').length, 1, 'Manage must retain the one active collection child.');
  const pid = await ownedDesktopPid();
  const focus = JSON.parse(execFileSync('/usr/bin/python3', [join(harness, 'x11-close.py'), String(pid), 'focus'], { env: fixture.environment, encoding: 'utf8', timeout: 10000 }));
  await waitFor(async () => driver.execute('return document.hasFocus()&&document.activeElement===document.querySelector(arguments[0]);', [manage]), 'only-owned native window focused on Manage');
  await driver.execute('window.__collectionKeyTrace=[];window.__collectionKeyListener=event=>window.__collectionKeyTrace.push({type:event.type,key:event.key,code:event.code,shiftKey:event.shiftKey});window.addEventListener("keydown",window.__collectionKeyListener,true);return true;');
  const reverseTab = JSON.parse(execFileSync('/usr/bin/python3', [join(harness, 'x11-reverse-tab.py'), String(focus.windowId)], { env: fixture.environment, encoding: 'utf8', timeout: 10000 }));
  await waitFor(async () => driver.execute('return document.activeElement===document.querySelector(arguments[0]);', [cancel]), 'physical reverse Tab reaches exact selected Cancel refresh', 1500);
  const trace = await driver.execute('window.removeEventListener("keydown",window.__collectionKeyListener,true);return window.__collectionKeyTrace;');
  assert.ok(trace.some(event => event.type === 'keydown' && event.code === 'Tab' && event.shiftKey), 'Observe the physical Shift+Tab key event.');
  const screenshot = 'collection-reverse-tab-cancel.png'; await driver.screenshot(join(output, screenshot)); evidence.screenshots.push(screenshot);
  assert.equal(await driver.execute('return document.activeElement===document.querySelector(arguments[0]);', [cancel]), true);
  await driver.request('POST', `/session/${driver.session}/actions`, { actions: [{ type: 'key', id: 'collection-cancel-enter', actions: [{ type: 'keyDown', value: '\uE007' }, { type: 'keyUp', value: '\uE007' }] }] });
  await driver.request('DELETE', `/session/${driver.session}/actions`);
  evidence.keyboardCancel = { onlyOwnedDesktopPid: pid, focus, reverseTab, trace, manageOpenedAndClosed: true, exactReverseTabDestination: 'selected Cancel refresh', enterCancellation: true, noSyntheticDomKeyDispatch: true };
}
async function create(label) { await driver.openProfileCreate(); await driver.type('#profile-label', label); await driver.click('.profile-create-form button[type=submit]'); await waitFor(async () => driver.execute('const input=document.querySelector("#profile-label");return !input.disabled&&input.value==="";'), 'profile creation finished'); const profile = (await call('list_profiles')).profiles.find(value => value.label === label); assert.ok(profile); profiles.push(profile); return profile; }
async function approveGui(profile) {
  await driver.navigate('settings'); if (await driver.execute('return !document.querySelector("[data-provider-settings]").closest("details").open;')) await driver.click('[data-provider-settings]');
  if (await driver.execute('const node=document.querySelector("[data-provider-manual]");return Boolean(node&&!node.closest("details").open);')) await driver.click('[data-provider-manual]');
  await driver.type('#provider-executable', executable); await driver.click('.provider-inspection-form button[type=submit]');
  await waitFor(async () => driver.execute('const node=document.querySelector("#provider-executable-approved");return Boolean(node&&!node.disabled&&node.getAttribute("aria-checked")==="false");'), 'unapproved reviewed collection fixture');
  assert.equal(await driver.execute('return document.querySelector(".provider-approval-form button[type=submit]").disabled;'), true);
  await driver.click('#provider-executable-approved');
  await waitFor(async () => driver.execute('return document.querySelector("#provider-executable-approved")?.getAttribute("aria-checked")==="true"&&document.querySelector(".provider-approval-form button[type=submit]")?.disabled===false;'), 'checked executable consent enables approval');
  await driver.click('.provider-approval-form button[type=submit]');
  await waitFor(async () => driver.execute('return Boolean(document.activeElement?.matches(".provider-approval-result"))&&document.querySelector("#provider-executable")?.value===""&&!document.querySelector("#provider-executable-approved");'), 'latest executable approval finishes and focuses its result');
  const probe = await call('check_profile_connection', { profileId: profile.id }); assert.equal(probe.persisted, false); assert.equal(probe.sourceKind, 'test_fixture'); approvalId = probe.approvalId;
}
async function begin(profile) { const receipt = await call('begin_profile_collection_read', { profileId: profile.id, approvalId }); assert.equal(receipt.accepted, false); assert.equal(receipt.monitoring, 'stopped'); assert.equal(receipt.sourceKind, 'test_fixture'); active.set(receipt.requestId, profile.id); return receipt; }
async function readIpc(profile, selectedMode, error) { await mode(profile, selectedMode); const receipt = await begin(profile); const response = await driver.invoke('complete_profile_collection_read', args(receipt)); active.delete(receipt.requestId); if (error) { assert.deepEqual(response, { ok: false, error }); return; } assert.equal(response.ok, true, JSON.stringify(response)); const value = response.value; assert.equal(value.profileId, profile.id); assert.equal(value.requestId, receipt.requestId); assert.equal(value.approvalId, approvalId); assert.equal(value.monitoring, 'stopped'); assert.equal(value.identityConfidence, 'provisional'); assert.equal(value.verifiedIdentityKey, null); assert.equal(value.accepted, Boolean(value.quota || value.activity)); if (value.quota && value.activity) { assert.equal(value.quota.bindingId, value.activity.bindingId); assert.notEqual(value.quota.collectionRunId, value.activity.collectionRunId); } return value; }
async function guiStart(profile, selectedMode) { await mode(profile, selectedMode); await driver.openProfileReadings(profile.id); await waitFor(async () => driver.execute('const node=document.querySelector(arguments[0]);return Boolean(node&&!node.disabled);', [readSelector(profile)]), 'enabled real Refresh readings button'); const offset = (await rows(profile)).length; await driver.click(readSelector(profile)); return offset; }
async function guiResult(profile, pattern) { await waitFor(async () => driver.execute('const node=document.querySelector(arguments[0]);return Boolean(node&&node.getClientRects().length&&document.activeElement===node&&!document.querySelector(arguments[1]));', [resultSelector(profile), cancelSelector(profile)]), 'focused combined result'); const text = await driver.execute('return document.querySelector(arguments[0]).textContent;', [resultSelector(profile)]); assert.match(text, pattern); return text; }
async function capture(name, selector, width = 1280) { for (let attempt = 0; attempt < 3; attempt++) { const size = await driver.execute('return {width:innerWidth,height:innerHeight};'); if (size.width === width && size.height === 850) break; const rect = await driver.request('GET', `/session/${driver.session}/window/rect`); await driver.request('POST', `/session/${driver.session}/window/rect`, { width: rect.width + width - size.width, height: rect.height + 850 - size.height }); } if (selector) await driver.execute('document.querySelector(arguments[0])?.scrollIntoView({block:"center"});return true;', [selector]); await driver.screenshot(join(output, name)); evidence.screenshots.push(name); }

await mkdir(output, { recursive: true, mode: 0o700 });
process.once('exit', () => { driver?.kill('SIGKILL'); stopGroup(fixture?.bus, 'SIGKILL'); });
try {
  evidence.application = { path: application, sha256: await hash(application) }; assert.equal(await hash(verifier), verifierDigest); evidence.verifier = { path: verifier, sha256: verifierDigest, reusedReadOnlyQueryLogic: true, expectedSchemaVersion: 4, schemaExpectationEnforcedByCaller: true };
  evidence.sourceCommit = execFileSync('git', ['rev-parse', 'HEAD'], { cwd: repository, encoding: 'utf8' }).trim();
  evidence.sourceHashes = {}; const sources = execFileSync('rg', ['--files', 'apps/desktop/src', 'apps/desktop/src-tauri/src', 'crates/openusage-codex/src', 'crates/openusage-storage/src'], { cwd: repository, encoding: 'utf8' }).trim().split('\n'); for (const path of [...sources, coreFixture, 'Cargo.lock']) evidence.sourceHashes[path] = await hash(join(repository, path));
  evidence.harnessHashes = {}; for (const path of ['collection.mjs', 'webdriver.mjs', 'private-fixture.mjs', 'x11-close.py', 'x11-reverse-tab.py']) evidence.harnessHashes[path] = await hash(join(harness, path));
  fixture = await createFixture('collection'); fixture.environment.HOME = join(fixture.root, 'synthetic-home'); await mkdir(fixture.environment.HOME, { mode: 0o700 }); workspace = dirname(fixture.database);
  executable = join(fixture.root, 'synthetic-collection-provider.py'); await copyFile(join(repository, coreFixture), executable); await chmod(executable, 0o700); assert.equal(await hash(executable), fixtureDigest);
  await open(true);
  const first = await create('Synthetic combined primary'), malformedQuota = await create('Synthetic malformed quota'), malformedActivity = await create('Synthetic malformed activity'), quotaUnsupported = await create('Synthetic unsupported quota'), activityUnsupported = await create('Synthetic unsupported activity'), bothUnsupported = await create('Synthetic unsupported both'), freshUnsupported = await create('Synthetic unavailable fresh'), reassociation = await create('Synthetic account review');
  evidence.profiles = profiles; await approveGui(first);
  const offset = await guiStart(first, 'success'); await guiResult(first, /Quota and activity readings saved/);
  let initial = await cached(first); validatePair(initial, first);
  const firstRows = (await rows(first)).slice(offset), methods = firstRows.filter(row => row.kind === 'request').map(row => row.request.method);
  assert.deepEqual(methods, ['initialize', 'initialized', 'account/read', 'account/rateLimits/read', 'account/usage/read', 'account/read']); assert.equal(firstRows.filter(row => row.kind === 'start' && row.arguments[0] === 'app-server').length, 1);
  await driver.navigate('overview'); await driver.selectVisibleOption('#overview-profile', first.label);
  await waitFor(async () => driver.execute('return document.querySelector("[data-overview-lifetime]")?.getAttribute("data-exact-value")==="9007199254740993";'), 'GUI Overview loads exact accepted activity');
  await capture('collection-both-saved-light.png', '[data-overview]');
  await driver.setAppearance('dark'); await capture('collection-both-saved-dark-360.png', '[data-overview]', 360);
  const beforeCooldown = await oracle('both capabilities committed by primary GUI action');
  for (const command of ['begin_profile_quota_read', 'begin_profile_activity_read', 'begin_profile_collection_read']) await refused(command, { profileId: first.id, approvalId }, 'provider_refresh_cooldown');
  assert.deepEqual(await oracle('cross-kind cooldown refuses without writes'), beforeCooldown);
  const waitStarted = performance.now(); await driver.waitForProfileRefresh(first.id); const actualWait = Math.round(performance.now() - waitStarted);
  assert.ok((await readIpc(first, 'success')).accepted); initial = await cached(first); validatePair(initial, first);
  evidence.steps.push({ acceptedAfterActualCooldown: true, actualCrossKindCooldownWaitMilliseconds: actualWait, primaryBothSavedViaGui: true, oneAppServerForBothCapabilities: true });
  // Seed independent profiles through typed IPC; subsequent fault cases cannot consume one another's cooldown.
  for (const profile of [malformedQuota, malformedActivity, quotaUnsupported, activityUnsupported, bothUnsupported, reassociation]) { const value = await readIpc(profile, 'success'); assert.ok(value.quota && value.activity); }
  const beforeRestart = await oracle('seeded selected-profile data before genuine restart');
  await closeNormally(); await open();
  await refused('begin_profile_collection_read', { profileId: first.id, approvalId }, 'provider_executable_not_approved');
  validatePair(await cached(first), first); assert.deepEqual(metrics(await cached(first)), metrics(initial)); assert.deepEqual(await oracle('genuine restart preserves all sixteen tables without executable approval'), beforeRestart);
  await driver.openProfileReadings(first.id); await capture('collection-saved-after-restart-no-approval.png', `[data-profile-id="${first.id}"]`);
  await approveGui(first);
  // Establish fresh anchors in this process; later unsupported reads must retain them.
  for (const profile of [quotaUnsupported, activityUnsupported]) assert.ok((await readIpc(profile, 'success')).accepted);
  const beforeCancel = await oracle('before native combined cancellation'); const cancelOffset = await guiStart(first, 'activity_hang'); const child = await stage(first, 'activity-pending', cancelOffset);
  for (const command of ['begin_profile_quota_read', 'begin_profile_activity_read', 'begin_profile_collection_read']) await refused(command, { profileId: first.id, approvalId }, 'provider_collection_in_progress');
  assert.deepEqual(await oracle('quota response held before activity cannot half-commit'), beforeCancel);
  await driver.navigate('overview'); await capture('collection-pending-keeps-saved-overview.png', '[data-overview]'); await driver.openProfileReadings(first.id); await keyboardCancel(first, child, cancelOffset); await guiResult(first, /Reading refresh stopped/); await reaped(child);
  assert.deepEqual(await oracle('cancel preserves both current values and complete history'), beforeCancel);
  for (const [profile, selectedMode] of [[malformedQuota, 'quota_invalid'], [malformedActivity, 'activity_invalid']]) { const before = await oracle(`before ${selectedMode}`); await readIpc(profile, selectedMode, 'provider_protocol_failed'); assert.deepEqual(await oracle(`${selectedMode} rejects the entire collection`), before); }
  for (const profile of [bothUnsupported, freshUnsupported]) { const before = await oracle(`before both unsupported ${profile.label}`), previousView = await cached(profile); const value = await readIpc(profile, 'both_unsupported'); assert.equal(value.accepted, false); assert.equal(value.quota, null); assert.equal(value.activity, null); assert.deepEqual(await oracle('both unsupported means no transaction or new binding'), before); assert.deepEqual(await cached(profile), previousView, 'Both unsupported capabilities preserve the complete cached view, including restart-unknown freshness.'); }
  const beforeReview = await oracle('before changed account review'); const previousBinding = (await cached(reassociation)).quota.bindingId;
  await guiStart(reassociation, 'stable_changed_identity'); await guiResult(reassociation, /no longer matches/); assert.deepEqual(await oracle('new account cannot replace either capability without consent'), beforeReview);
  await driver.waitForProfileRefresh(reassociation.id);
  await waitFor(async () => driver.execute('return !document.querySelector(arguments[0])?.disabled;', [`[data-collection-reassociate="${reassociation.id}"]`]), 'enabled current-sign-in review');
  await driver.click(`[data-collection-reassociate="${reassociation.id}"]`);
  const consent = `#collection-reassociation-consent-${reassociation.id}`, form = `${resultSelector(reassociation)} form`;
  await waitFor(async () => driver.execute('return Boolean(document.querySelector(arguments[0])&&!document.querySelector(arguments[0]).disabled);', [consent]), 'explicit current-sign-in collection consent');
  assert.equal(await driver.execute('return document.querySelector(arguments[0]).querySelector("button[type=submit]").disabled;', [form]), true);
  assert.match(await driver.execute('return document.querySelector(arguments[0]).textContent;', [form]), /synthetic-collection@example.invalid/); assert.match(await driver.execute('return document.querySelector(arguments[0]).textContent;', [form]), /changed@example.invalid/);
  await capture('collection-current-sign-in-explicit-review.png', form, 360); await driver.click(consent); await driver.click(`${form} button[type=submit]`); await guiResult(reassociation, /Quota and activity readings saved/);
  const reassociated = await cached(reassociation); validatePair(reassociated, reassociation); assert.notEqual(reassociated.quota.bindingId, previousBinding);
  const afterReview = await oracle('explicit reassociation commits both under one new binding');
  for (const name of ['profile_identity_events', 'collection_runs', 'activity_summaries', 'activity_day_history', 'quota_snapshots', 'quota_limits', 'credit_snapshots', 'quota_reset_assessments']) for (const row of beforeReview.tables[name]) assert.ok(afterReview.tables[name].some(current => JSON.stringify(current) === JSON.stringify(row)), `Reassociation retains complete ${name} history.`);
  const beforeOtherProfiles = structuredClone(beforeReview), afterOtherProfiles = structuredClone(afterReview);
  for (const name of Object.keys(beforeOtherProfiles.tables)) {
    beforeOtherProfiles.tables[name] = beforeOtherProfiles.tables[name].filter(row => row.profile_id !== reassociation.id);
    afterOtherProfiles.tables[name] = afterOtherProfiles.tables[name].filter(row => row.profile_id !== reassociation.id);
  }
  assert.deepEqual(afterOtherProfiles, beforeOtherProfiles, 'Reassociation must preserve other profiles and application settings.');
  await capture('collection-reassociated-both-saved.png', resultSelector(reassociation));
  for (const [profile, selectedMode, supported] of [[quotaUnsupported, 'quota_unsupported', 'activity'], [activityUnsupported, 'activity_unsupported', 'quota']]) { const before = await oracle(`before ${selectedMode}`), previous = await cached(profile), value = await readIpc(profile, selectedMode); assert.equal(value[supported] !== null, true); assert.equal(value[supported === 'quota' ? 'activity' : 'quota'], null); onlyCapabilityChanged(before, await oracle(`${selectedMode} accepts only the supported capability`), profile, supported); const after = await cached(profile); const retained = supported === 'quota' ? 'activity' : 'quota'; assert.equal(previous[retained].freshness, 'fresh', 'The retained side must have a live same-session observation anchor.'); assert.equal(after[retained].observedAtUtc, previous[retained].observedAtUtc); assert.equal(after[retained].freshness, previous[retained].freshness, 'Unsupported capability must preserve its prior freshness anchor.'); assert.deepEqual(metrics({ retained: after[retained] }), metrics({ retained: previous[retained] })); assert.notEqual(after[supported].collectionRunId, previous[supported].collectionRunId); evidence.cases.push({ mode: selectedMode, supportedOnlyCommitted: true, retainedSideValuesObservedAtAndFreshnessUnchanged: true, retainedFreshness: after[retained].freshness }); }
  // Approval replacement must quiesce a pending collection and prevent late acceptance.
  await driver.waitForProfileRefresh(first.id); const beforeReapproval = await oracle('before source replacement'); const pendingOffset = await guiStart(first, 'activity_hang'); const pending = await stage(first, 'activity-pending', pendingOffset); await approveGui(reassociation); await reaped(pending); assert.deepEqual(await oracle('reapproval cancels pending collection without writes'), beforeReapproval);
  // Archival is a typed trust-boundary check, separate from the primary GUI journey.
  await driver.waitForProfileRefresh(malformedQuota.id); await mode(malformedQuota, 'activity_hang'); const archiveOffset = (await rows(malformedQuota)).length; const archivedReceipt = await begin(malformedQuota);
  await driver.execute('window.__collectionArchiveCompletion=null;window.__TAURI__.core.invoke("complete_profile_collection_read",arguments[0]).then(value=>window.__collectionArchiveCompletion={ok:true,value},error=>window.__collectionArchiveCompletion={ok:false,error});return true;', [args(archivedReceipt)]);
  const archivedChild = await stage(malformedQuota, 'activity-pending', archiveOffset); const beforeArchive = await oracle('before archive quiescence'); await call('set_profile_archived', { id: malformedQuota.id, archived: true }); await reaped(archivedChild); active.delete(archivedReceipt.requestId);
  await waitFor(async () => driver.execute('return Boolean(window.__collectionArchiveCompletion);'), 'archived pending collection refused'); assert.deepEqual(await driver.execute('return window.__collectionArchiveCompletion;'), { ok: false, error: 'provider_collection_cancelled' });
  const afterArchive = await oracle('archive prevents late combined publication'); const normalized = structuredClone(afterArchive); const oldProfile = beforeArchive.tables.profiles.find(row => row.id === malformedQuota.id), archivedProfile = normalized.tables.profiles.find(row => row.id === malformedQuota.id); assert.equal(archivedProfile.archived, 1); archivedProfile.archived = oldProfile.archived; archivedProfile.updated_at = oldProfile.updated_at; normalized.profiles.find(row => row.id === malformedQuota.id).archived = beforeArchive.profiles.find(row => row.id === malformedQuota.id).archived; assert.deepEqual(normalized, beforeArchive);
  assert.equal(await readFile(join(fixture.dirs.codex, 'synthetic-fixture.txt'), 'utf8'), CODEX_SENTINEL); assert.deepEqual(await readdir(fixture.dirs.codex), ['synthetic-fixture.txt']);
  evidence.observations = Object.fromEntries(await Promise.all(profiles.map(async profile => [profile.id, await rows(profile)])));
  for (const observed of Object.values(evidence.observations).flat().filter(row => row.kind === 'start')) await reaped(observed);
  evidence.steps.push({ unsupportedMethodsIsolated: true, malformedCapabilityPreservesAllTables: true, cancellationPreservesBothValuesAndHistory: true, genuineRestartAndNoApprovalCache: true, explicitReassociationOnly: true, reapprovalAndArchiveInvalidatePendingCollection: true, schedulingNotImplemented: true, restoreRaceExecuted: false });
  assert.equal(await hash(application), evidence.application.sha256); assert.equal(await hash(verifier), verifierDigest); assert.equal(await hash(executable), fixtureDigest); for (const [path, digest] of Object.entries(evidence.sourceHashes)) assert.equal(await hash(join(repository, path)), digest, `Source drift: ${path}`); for (const [path, digest] of Object.entries(evidence.harnessHashes)) assert.equal(await hash(join(harness, path)), digest, `Harness drift: ${path}`);
  evidence.result = 'passed';
} catch (error) {
  evidence.error = error.stack ?? String(error); process.exitCode = 1;
  if (driver?.session) {
    try {
      evidence.failureView = await driver.execute('const active=document.activeElement,visible=node=>Boolean(node.getClientRects().length&&!node.closest("[hidden],[inert]"));return {currentView:document.querySelector("[data-workspace-nav][aria-current=page]")?.getAttribute("data-workspace-nav"),activeElement:active?{tag:active.tagName,id:active.id,role:active.getAttribute("role"),buttonLabel:active.tagName==="BUTTON"?active.textContent.trim().slice(0,80):null,collectionProfile:active.closest("[data-collection-profile]")?.getAttribute("data-collection-profile"),cancelProfile:active.getAttribute("data-collection-cancel"),visible:visible(active)}:null,results:[...document.querySelectorAll("[data-collection-profile]")].slice(0,10).map(node=>({profileId:node.getAttribute("data-collection-profile"),role:node.getAttribute("role"),visible:visible(node),focused:node===active,messages:[...node.children].filter(child=>child.tagName==="P"&&!child.hasAttribute("data-provider-fixture")).slice(0,2).map(child=>child.textContent.trim().slice(0,300))})),pending:[...document.querySelectorAll("[data-collection-cancel]")].slice(0,10).map(node=>({profileId:node.getAttribute("data-collection-cancel"),disabled:node.disabled,visible:visible(node),focused:node===active})),approval:{resultFocused:Boolean(active?.matches(".provider-approval-result")),inputCleared:document.querySelector("#provider-executable")?.value==="",consentState:document.querySelector("#provider-executable-approved")?.getAttribute("aria-checked")}};');
    } catch (diagnosticError) { evidence.failureDiagnosticsError = diagnosticError.message; }
    try { await driver.screenshot(join(output, 'failure.png')); evidence.screenshots.push('failure.png'); } catch {}
  }
}
finally {
  if (driver?.session) for (const [requestId, profileId] of active) try { await driver.invoke('cancel_profile_collection_read', { profileId, requestId }); } catch {}
  try { await driver?.stop(); } catch (error) { evidence.cleanupError = error.message; evidence.result = 'failed'; process.exitCode = 1; }
  if (fixture) { stopGroup(fixture.bus, 'SIGKILL'); await rm(fixture.root, { recursive: true, force: true }); }
  evidence.finishedAt = new Date().toISOString(); await writeFile(join(output, 'result.json'), JSON.stringify(evidence, null, 2) + '\n', { mode: 0o600 }); console.log(`${evidence.requirement}: ${evidence.result}. Evidence: ${output}`); if (evidence.error) console.error(evidence.error);
}
