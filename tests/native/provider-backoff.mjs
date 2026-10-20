import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { chmod, copyFile, lstat, mkdir, readFile, readdir, rename, rm, writeFile } from 'node:fs/promises';
import { arch, platform, release, tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { fileURLToPath } from 'node:url';
import { CODEX_SENTINEL, createFixture, stopGroup } from './private-fixture.mjs';
import { NativeDriver, waitFor } from './webdriver.mjs';

const harness = dirname(fileURLToPath(import.meta.url));
const repository = resolve(process.env.OPENUSAGE_NATIVE_SOURCE_ROOT ?? join(harness, '../..'));
const application = resolve(process.env.OPENUSAGE_NATIVE_BINARY ?? join(repository, 'target/debug/openusage-desktop'));
const output = resolve(process.env.OPENUSAGE_NATIVE_EVIDENCE ?? join(tmpdir(), `openusage-provider-backoff-${randomUUID()}`));
const fixtureDigest = 'ffceb4436e3ddbaf93548e36aa057232fc62bbda7138e00f90f250480d17e25c';
const password = 'OpenUsage synthetic provider backoff workspace 2026';
const hash = async path => createHash('sha256').update(await readFile(path)).digest('hex');
const elapsed = start => Number(process.hrtime.bigint() - start) / 1e6;
const readSelector = id => `[data-quota-read="${id}"]`;
const savedSelector = id => `[data-quota-saved="${id}"]`;
const resultSelector = id => `.quota-result[data-quota-profile="${id}"]`;
const statusSelector = id => `[data-profile-refresh-status="${id}"]`;
const evidence = {
  requirement: 'NATIVE-PROVIDER-BACKOFF-001', result: 'failed', startedAt: new Date().toISOString(),
  scope: 'Actual native per-profile retry backoff after controlled owned-provider process exits. The unchanged reviewed synthetic quota fixture waits for a response, then the test kills only its verified owned process group. Real provider EOF/exit classification, real jittered waits, shared quota/activity refusal, saved readings, independent profiles, renderer reload, executable reapproval, success resetting the failure streak, and manual cooldown. No fake clock, product timer bypass, modified fixture bytes, real provider login, existing credential homes, scheduled monitoring, maximum backoff or other-platform acceptance.',
  platform: { os: platform(), release: release(), architecture: arch() }, steps: [], statuses: [], failures: [], waits: [], screenshots: [], views: [],
};
let fixture, driver, workspace, executable, executionLog, approvalId;

async function observations() {
  try { return (await readFile(executionLog, 'utf8')).trim().split('\n').filter(Boolean).map(JSON.parse); }
  catch (error) { if (error.code === 'ENOENT') return []; throw error; }
}
async function liveHashes() {
  const value = {};
  for (const path of [fixture.database, `${fixture.database}-wal`, join(workspace, 'database-key.v1')]) value[path] = await hash(path);
  return value;
}
async function config(profile, mode) {
  const path = join(fixture.root, 'synthetic-quota-provider-config.json');
  let value = { defaultMode: 'multi', modesByHome: {} };
  try { value = JSON.parse(await readFile(path, 'utf8')); } catch (error) { if (error.code !== 'ENOENT') throw error; }
  value.modesByHome[join(workspace, 'profiles', profile.id)] = mode;
  await writeFile(`${path}.next`, `${JSON.stringify(value)}\n`, { mode: 0o600 }); await rename(`${path}.next`, path);
}
async function create(label) { await driver.openProfileCreate();
  await waitFor(async () => driver.execute('return Boolean(document.querySelector("#profile-label")&&!document.querySelector("#profile-label").disabled);'), 'enabled profile form');
  await driver.type('#profile-label', label); await driver.click('.profile-create-form button[type=submit]');
  await waitFor(async () => driver.execute('const node=document.querySelector("#profile-label");return !node.disabled&&node.value==="";'), 'created synthetic profile');
  const result = await driver.invoke('list_profiles'); assert.equal(result.ok, true);
  const profile = result.value.profiles.find(value => value.label === label); assert.ok(profile); return profile;
}
async function approve(profile) { await driver.navigate('settings');
  if (await driver.execute('return !document.querySelector("[data-provider-settings]").closest("details").open;')) await driver.click('[data-provider-settings]');
  if (await driver.execute('const node=document.querySelector("[data-provider-manual]");return Boolean(node&&!node.closest("details").open);')) await driver.click('[data-provider-manual]');
  await driver.type('#provider-executable', executable); await driver.click('.provider-inspection-form button[type=submit]');
  await waitFor(async () => driver.execute('const node=document.querySelector("#provider-executable-approved");return Boolean(node&&!node.disabled);'), 'reviewed synthetic executable');
  await driver.click('#provider-executable-approved');
  await waitFor(async () => driver.execute('return document.querySelector("#provider-executable-approved").getAttribute("aria-checked")==="true"&&!document.querySelector(".provider-approval-form button[type=submit]").disabled;'), 'explicit source consent');
  await driver.click('.provider-approval-form button[type=submit]');
  await waitFor(async () => driver.execute('return document.activeElement?.matches(".provider-approval-result");'), 'source approval result');
  const result = await driver.invoke('check_profile_connection', { profileId: profile.id });
  assert.equal(result.ok, true); assert.equal(result.value.sourceKind, 'test_fixture'); approvalId = result.value.approvalId; await driver.openProfileReadings(profile.id);
}
async function status(profile, label) {
  const result = await driver.invoke('load_profile_refresh_status', { profileId: profile.id }); assert.equal(result.ok, true);
  assert.deepEqual(Object.keys(result.value).sort(), ['profileId', 'retryAfterSeconds']); assert.equal(result.value.profileId, profile.id);
  const seconds = result.value.retryAfterSeconds;
  assert.ok(Number.isInteger(seconds) && seconds >= 0 && seconds <= 4320);
  if (label) evidence.statuses.push({ label, ...result.value }); return seconds;
}
async function saved(profile) {
  const result = await driver.invoke('load_profile_quota', { profileId: profile.id }); assert.equal(result.ok, true);
  if (result.value !== null) { assert.equal(result.value.profileId, profile.id); assert.equal(result.value.accepted, true); assert.equal(result.value.sourceKind, 'test_fixture'); }
  return result.value;
}
function retained(value) { const { freshness, ...record } = value; return record; }
async function quotaView(profile) {
  return driver.execute('const root=document.querySelector(arguments[0]);return root&&{time:root.querySelector(".quota-observed time")?.dateTime,meters:[...root.querySelectorAll("[role=meter]")].map(node=>({value:node.getAttribute("aria-valuenow"),label:node.getAttribute("aria-label")}))};', [savedSelector(profile.id)]);
}
async function reaped(row) {
  await waitFor(() => {
    for (const target of [row.pid, -row.processGroup]) try { process.kill(target, 0); return false; } catch (error) { if (error.code !== 'ESRCH') throw error; }
    return true;
  }, 'owned synthetic process and group reaped');
}
async function stage(profile, kind, offset) {
  let found;
  await waitFor(async () => {
    const rows = await observations(), starts = new Map(rows.filter(row => row.kind === 'start').map(row => [row.pid, row]));
    found = rows.slice(offset).find(row => row.kind === kind && starts.get(row.pid)?.environment.CODEX_HOME === join(workspace, 'profiles', profile.id));
    return Boolean(found);
  }, `${kind} for the selected synthetic profile`);
  return found;
}
async function guiRead(profile) {
  await driver.openProfileReadings(profile.id);
  await config(profile, 'multi'); const before = await saved(profile), offset = (await observations()).length;
  await waitFor(async () => driver.execute('const node=document.querySelector(arguments[0]);return Boolean(node&&!node.disabled);', [readSelector(profile.id)]), 'enabled Read quotas action');
  await driver.click(readSelector(profile.id));
  await waitFor(async () => { const value = await saved(profile); return value && value.collectionRunId !== before?.collectionRunId; }, 'new saved quota reading');
  const child = await stage(profile, 'quota-read-pending', offset); await reaped(child);
  await waitFor(async () => driver.execute('return document.activeElement?.matches(arguments[0]);', [savedSelector(profile.id)]), 'focused saved quota reading');
  const value = await saved(profile); assert.equal(value.freshness, 'fresh');
  evidence.steps.push({ successfulGuiRead: profile.id, collectionRunId: value.collectionRunId, ownedProcessReaped: true }); return value;
}
async function fail(profile, label, minimum, maximum) {
  await driver.openProfileReadings(profile.id);
  await config(profile, 'pending'); const before = await liveHashes(), previous = await saved(profile), view = await quotaView(profile), offset = (await observations()).length;
  assert.equal(await status(profile, `${label}: eligible before failure`), 0);
  await waitFor(async () => driver.execute('const node=document.querySelector(arguments[0]);return Boolean(node&&!node.disabled);', [readSelector(profile.id)]), 'eligible GUI read before controlled provider exit');
  await driver.click(readSelector(profile.id));
  const child = await stage(profile, 'quota-read-pending', offset);
  assert.ok(Number.isSafeInteger(child.pid) && child.pid > 1 && child.pid !== process.pid);
  assert.equal(child.pid, child.processGroup, 'Only the owned fixture process-group leader can be killed.');
  const command = (await readFile(`/proc/${child.pid}/cmdline`)).toString().split('\0');
  assert.ok(command.includes(executable), 'The live child must execute the exact private synthetic fixture.');
  const failedAt = process.hrtime.bigint(); process.kill(-child.processGroup, 'SIGKILL');
  await waitFor(async () => driver.execute('const node=document.querySelector(arguments[0]);return Boolean(node&&node.getAttribute("role")==="alert"&&document.activeElement===node&&node.textContent.includes("Codex could not read quotas."));', [resultSelector(profile.id)]), 'focused native failure after real owned-provider EOF');
  await reaped(child);
  const seconds = await status(profile, `${label}: retry delay`);
  assert.ok(seconds >= minimum - 1 && seconds <= maximum, `Retry delay ${seconds}s must match jittered range ${minimum}..${maximum}s, allowing one elapsed second.`);
  assert.deepEqual(await liveHashes(), before); assert.deepEqual(retained(await saved(profile)), retained(previous)); assert.deepEqual(await quotaView(profile), view);
  evidence.failures.push({ label, profileId: profile.id, pid: child.pid, processGroup: child.processGroup, controlledOwnedGroupSignal: 'SIGKILL', selectedHomeQuotaRequestObserved: true, exactPrivateFixtureCommandChecked: true, actualProviderEof: true, observedRetryAfterSeconds: seconds, expectedJitterRangeSeconds: [minimum, maximum], manualCooldownCanRaiseBaseDelay: minimum === 24, encryptedStorageUnchanged: true, savedValuesAndTimeUnchanged: true });
  return { failedAt, minimum, maximum, label };
}
async function blocked(profile, label) {
  assert.ok(await status(profile, label) > 0);
  const before = await liveHashes(), rows = await observations(), value = await saved(profile);
  for (const command of ['begin_profile_quota_read', 'begin_profile_activity_read']) assert.deepEqual(await driver.invoke(command, { profileId: profile.id, approvalId }), { ok: false, error: 'provider_refresh_backoff' });
  assert.deepEqual(await observations(), rows); assert.deepEqual(await liveHashes(), before); assert.deepEqual(retained(await saved(profile)), retained(value));
  evidence.steps.push({ label, quotaAndActivityBackoffRefused: true, noProviderExecution: true, storageAndSavedValuesUnchanged: true });
}
async function waitReady(profile, label, failure) {
  const before = await liveHashes(), rows = await observations(), value = await saved(profile), start = process.hrtime.bigint();
  await waitFor(async () => { const seconds = await status(profile); if (seconds === 0) return true; await delay(Math.min(seconds, 1) * 1000); return false; }, label, 90000);
  const sinceFailure = failure ? elapsed(failure.failedAt) : null;
  if (failure) assert.ok(sinceFailure >= failure.minimum * 1000, 'The retry deadline must consume real elapsed time.');
  assert.deepEqual(await liveHashes(), before); assert.deepEqual(await observations(), rows);
  assert.deepEqual(retained(await saved(profile)), retained(value));
  evidence.waits.push({ label, waitedMilliseconds: elapsed(start), millisecondsSinceControlledExit: sinceFailure, realMonotonicTime: true, noExecutionOrStorageChange: true });
}
async function capture(name, profile) {
  await waitFor(async () => driver.execute('const button=document.querySelector(arguments[0]),notice=document.querySelector(arguments[1]);return Boolean(button?.disabled&&notice?.textContent.trim());', [readSelector(profile.id), statusSelector(profile.id)]), 'visible disabled retry controls');
  const view = await driver.execute('const root=document.querySelector(arguments[0]),button=document.querySelector(arguments[1]),notice=document.querySelector(arguments[2]);notice.scrollIntoView({block:"center"});return {notice:notice.textContent.trim(),readDisabled:button.disabled,savedTime:root.querySelector(".quota-observed time")?.dateTime,width:innerWidth,pageWidth:document.scrollingElement.scrollWidth};', [savedSelector(profile.id), readSelector(profile.id), statusSelector(profile.id)]);
  assert.ok(view.pageWidth <= view.width + 1);
  await driver.screenshot(join(output, name)); evidence.screenshots.push(name); evidence.views.push({ screenshot: name, ...view });
}

await mkdir(output, { recursive: true, mode: 0o700 });
process.once('exit', () => { driver?.kill('SIGKILL'); stopGroup(fixture?.bus, 'SIGKILL'); });
try {
  assert.equal(platform(), 'linux');
  evidence.application = { path: application, sha256: await hash(application), commit: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: repository, encoding: 'utf8' }).trim() };
  evidence.sourceDirtyPaths = execFileSync('git', ['status', '--porcelain'], { cwd: repository, encoding: 'utf8' }).trim().split('\n').filter(Boolean);
  evidence.sourceHashes = {};
  const sources = execFileSync('rg', ['--files', 'apps/desktop/src', 'apps/desktop/src-tauri/src', 'apps/desktop/src-tauri/permissions', 'crates/openusage-codex/src', 'crates/openusage-storage/src'], { cwd: repository, encoding: 'utf8' }).trim().split('\n');
  for (const path of [...sources, 'Cargo.toml', 'Cargo.lock', 'package.json', 'package-lock.json', 'apps/desktop/src-tauri/Cargo.toml', 'apps/desktop/src-tauri/tauri.conf.json', 'apps/desktop/src-tauri/capabilities/main-window.json']) evidence.sourceHashes[path] = await hash(join(repository, path));
  evidence.harnessHashes = {};
  for (const path of ['provider-backoff.mjs', 'private-fixture.mjs', 'webdriver.mjs', 'fixtures/synthetic-quota-provider.py']) evidence.harnessHashes[path] = await hash(join(harness, path));
  fixture = await createFixture('backoff'); fixture.environment.HOME = join(fixture.root, 'synthetic-home'); await mkdir(fixture.environment.HOME, { mode: 0o700 });
  workspace = dirname(fixture.database); executable = join(fixture.root, 'synthetic-quota-provider.py'); executionLog = join(fixture.root, 'synthetic-quota-observations.jsonl');
  await copyFile(join(harness, 'fixtures/synthetic-quota-provider.py'), executable); await chmod(executable, 0o700); assert.equal(await hash(executable), fixtureDigest);
  driver = new NativeDriver(application, fixture.environment, join(output, 'driver.log')); await driver.start();
  await waitFor(async () => driver.execute('return ["#workspace-passphrase","#workspace-confirmation"].every(selector=>{const node=document.querySelector(selector);return Boolean(node&&!node.disabled);});'), 'fresh fallback setup');
  await driver.type('#workspace-passphrase', password); await driver.type('#workspace-confirmation', password); await driver.click('.unlock-form button[type=submit]');
  await waitFor(async () => (await driver.invoke('bootstrap')).value?.storage === 'ready', 'private encrypted workspace');
  const first = await create('Provider backoff fixture'), other = await create('Independent backoff fixture');
  await approve(first); await driver.setAppearance('light');
  await waitFor(async () => driver.execute('return document.documentElement.dataset.theme==="light"&&!document.querySelector("#theme").disabled;'), 'light appearance');
  const protectedKey = await hash(join(workspace, 'database-key.v1')), initial = await guiRead(first), initialView = await quotaView(first);
  assert.deepEqual(initialView.meters.slice(0, 2), [{ value: '75', label: 'Five-hour: 75% remaining (derived)' }, { value: '0', label: 'Seven-day: 0% remaining (derived)' }]);
  assert.ok(await status(first, 'successful seed manual cooldown') <= 30);
  await waitReady(first, 'real manual cooldown after the saved seed');
  const firstFailure = await fail(first, 'first transient failure', 24, 36);
  await blocked(first, 'first failure blocks both read kinds'); await capture('backoff-first-failure-light-desktop.png', first);
  await waitReady(first, 'real first retry delay', firstFailure);
  const secondFailure = await fail(first, 'second transient failure', 48, 72);
  await blocked(first, 'second failure blocks both read kinds'); await capture('backoff-doubled-failure-light-desktop.png', first);

  const beforeReload = await liveHashes();
  await driver.request('POST', `/session/${driver.session}/refresh`, {});
  await waitFor(async () => driver.execute('return Boolean(window.__TAURI__?.core?.invoke&&document.querySelector("#profile-label"));'), 'renderer reload in the same native session');
  assert.deepEqual(await liveHashes(), beforeReload); assert.ok(await status(first, 'backoff survives renderer reload') > 0);
  await approve(first); await blocked(first, 'backoff survives executable reinspection and reapproval');
  assert.deepEqual(retained(await saved(first)), retained(initial));
  await guiRead(other); assert.deepEqual(retained(await saved(first)), retained(initial));
  assert.ok(await status(first, 'independent profile does not clear backoff') > 0);
  await waitReady(first, 'real doubled retry delay', secondFailure);
  assert.deepEqual(await quotaView(first), initialView);
  const recovered = await guiRead(first); assert.notEqual(recovered.collectionRunId, initial.collectionRunId);
  const postSuccess = await status(first, 'success leaves only manual cooldown'); assert.ok(postSuccess > 0 && postSuccess <= 30);
  await waitReady(first, 'real manual cooldown after successful recovery');
  await fail(first, 'first failure after successful recovery', 24, 36);
  await blocked(first, 'success reset the failure streak to base retry'); await capture('backoff-reset-base-light-desktop.png', first);
  assert.deepEqual(retained(await saved(first)), retained(recovered));
  assert.equal(await hash(join(workspace, 'database-key.v1')), protectedKey);
  assert.equal(await readFile(join(fixture.dirs.codex, 'synthetic-fixture.txt'), 'utf8'), CODEX_SENTINEL); assert.deepEqual(await readdir(fixture.dirs.codex), ['synthetic-fixture.txt']);
  evidence.observations = await observations();
  for (const row of evidence.observations.filter(row => row.kind === 'start')) { assert.ok([first.id, other.id].some(id => row.environment.CODEX_HOME === join(workspace, 'profiles', id))); assert.deepEqual(row.unexpectedEnvironmentKeys, []); await reaped(row); await assert.rejects(lstat(dirname(row.environment.HOME)), { code: 'ENOENT' }); }
  assert.equal(await hash(executable), fixtureDigest); assert.equal(await hash(application), evidence.application.sha256);
  for (const [path, digest] of Object.entries(evidence.sourceHashes)) assert.equal(await hash(join(repository, path)), digest);
  for (const [path, digest] of Object.entries(evidence.harnessHashes)) assert.equal(await hash(join(harness, path)), digest);
  evidence.result = 'passed';
} catch (error) {
  evidence.error = error.stack ?? String(error); process.exitCode = 1;
  if (executionLog) try { evidence.observations = await observations(); } catch {}
  if (driver?.session) try { await driver.screenshot(join(output, 'failure.png')); evidence.screenshots.push('failure.png'); } catch {}
} finally {
  try { await driver?.stop(); } catch (error) { evidence.cleanupError = error.message; evidence.result = 'failed'; process.exitCode = 1; }
  if (fixture) {
    if (evidence.result !== 'passed') for (const row of evidence.observations ?? []) if (row.kind === 'start') try {
      const command = await readFile(`/proc/${row.pid}/cmdline`);
      if (row.pid === row.processGroup && command.includes(Buffer.from(`${executable}\0`))) process.kill(-row.processGroup, 'SIGKILL');
    } catch (error) { if (!['ENOENT', 'ESRCH'].includes(error.code)) throw error; }
    stopGroup(fixture.bus, 'SIGKILL'); await rm(fixture.root, { recursive: true, force: true });
  }
  evidence.finishedAt = new Date().toISOString(); await writeFile(join(output, 'result.json'), `${JSON.stringify(evidence, null, 2)}\n`, { mode: 0o600 });
  console.log(`${evidence.requirement}: ${evidence.result}. Evidence: ${output}`); if (evidence.error) console.error(evidence.error);
}
