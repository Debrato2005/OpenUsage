import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { chmod, copyFile, lstat, mkdir, readFile, readdir, rm, writeFile } from 'node:fs/promises';
import { arch, platform, release, tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { CODEX_SENTINEL, createFixture, stopGroup } from './private-fixture.mjs';
import { NativeDriver, waitFor } from './webdriver.mjs';

const harness = dirname(fileURLToPath(import.meta.url));
const repository = resolve(process.env.OPENUSAGE_NATIVE_SOURCE_ROOT ?? join(harness, '../..'));
const application = resolve(process.env.OPENUSAGE_NATIVE_BINARY ?? join(repository, 'target/debug/openusage-desktop'));
const output = resolve(process.env.OPENUSAGE_NATIVE_EVIDENCE ?? join(tmpdir(), `openusage-refresh-cooldown-${randomUUID()}`));
const fixtureDigest = 'ffceb4436e3ddbaf93548e36aa057232fc62bbda7138e00f90f250480d17e25c';
const password = 'OpenUsage synthetic refresh workspace 2026';
const hash = async path => createHash('sha256').update(await readFile(path)).digest('hex');
const elapsed = start => Number(process.hrtime.bigint() - start) / 1e6;
const readSelector = id => `[data-quota-read="${id}"]`;
const activitySelector = id => `[data-activity-read="${id}"]`;
const savedSelector = id => `[data-quota-saved="${id}"]`;
const statusSelector = id => `[data-profile-refresh-status="${id}"]`;
const evidence = {
  requirement: 'NATIVE-MANUAL-REFRESH-COOLDOWN-001', result: 'failed', startedAt: new Date().toISOString(),
  scope: 'Actual native synthetic quota reads with a shared per-profile quota/activity manual-refresh cooldown. Direct IPC refusals, renderer reload, explicit executable reapproval, another profile, real elapsed time, retained saved values, and native screenshots. No product clock changes, cooldown bypass, real provider login, existing credential homes, scheduled monitoring, failure/cancellation/restore qualification, or other-platform acceptance.',
  platform: { os: platform(), release: release(), architecture: arch() }, steps: [], statuses: [], screenshots: [], views: [],
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
async function profiles() {
  const result = await driver.invoke('list_profiles'); assert.equal(result.ok, true); return result.value.profiles;
}
async function create(label) { await driver.openProfileCreate();
  await waitFor(async () => driver.execute('return Boolean(document.querySelector("#profile-label")&&!document.querySelector("#profile-label").disabled);'), 'enabled profile form');
  await driver.type('#profile-label', label); await driver.click('.profile-create-form button[type=submit]');
  await waitFor(async () => driver.execute('const node=document.querySelector("#profile-label");return !node.disabled&&node.value==="";'), 'created synthetic profile');
  const profile = (await profiles()).find(value => value.label === label); assert.ok(profile); return profile;
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
  const checked = await driver.invoke('check_profile_connection', { profileId: profile.id });
  assert.equal(checked.ok, true); assert.equal(checked.value.sourceKind, 'test_fixture'); approvalId = checked.value.approvalId; await driver.openProfileReadings(profile.id);
}
async function status(profile, label) {
  const result = await driver.invoke('load_profile_refresh_status', { profileId: profile.id }); assert.equal(result.ok, true);
  assert.deepEqual(Object.keys(result.value).sort(), ['profileId', 'retryAfterSeconds']);
  assert.equal(result.value.profileId, profile.id);
  assert.ok(Number.isInteger(result.value.retryAfterSeconds) && result.value.retryAfterSeconds >= 0 && result.value.retryAfterSeconds <= 4320);
  evidence.statuses.push({ label, ...result.value }); return result.value.retryAfterSeconds;
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
async function completedRead(profile, before, offset) {
  await waitFor(async () => { const value = await saved(profile); return value && value.collectionRunId !== before?.collectionRunId; }, 'new saved quota reading');
  const rows = await observations(), starts = new Map(rows.filter(row => row.kind === 'start').map(row => [row.pid, row]));
  const final = rows.slice(offset).find(row => row.kind === 'request' && row.request.method === 'account/read' && row.request.id === 4 && starts.get(row.pid)?.environment.CODEX_HOME === join(workspace, 'profiles', profile.id));
  assert.ok(final, 'The selected-home read must reach its final account request.'); await reaped(final);
  const value = await saved(profile); assert.equal(value.freshness, 'fresh');
  evidence.steps.push({ savedQuotaRead: profile.id, collectionRunId: value.collectionRunId, finalAccountRequest: true, ownedProcessReaped: true }); return value;
}
async function guiRead(profile) {
  await driver.openProfileReadings(profile.id);
  const before = await saved(profile), offset = (await observations()).length;
  await waitFor(async () => driver.execute('const node=document.querySelector(arguments[0]);return Boolean(node&&!node.disabled);', [readSelector(profile.id)]), 'enabled Read quotas action');
  await driver.readProfileQuota(profile.id);
  const value = await completedRead(profile, before, offset);
  await waitFor(async () => driver.execute('return document.activeElement?.matches(arguments[0]);', [savedSelector(profile.id)]), 'focused saved quota reading');
  return value;
}
async function blocked(profile, label) {
  const remaining = await status(profile, `${label}: before`);
  assert.ok(remaining > 0 && remaining <= 30, 'A successful refresh must keep only the thirty-second manual cooldown.');
  const before = await liveHashes(), rows = await observations(), value = await saved(profile);
  for (const command of ['begin_profile_quota_read', 'begin_profile_activity_read']) {
    assert.deepEqual(await driver.invoke(command, { profileId: profile.id, approvalId }), { ok: false, error: 'provider_refresh_cooldown' });
  }
  assert.deepEqual(await observations(), rows, 'A blocked refresh must execute no provider.');
  assert.deepEqual(await liveHashes(), before, 'A blocked refresh must preserve encrypted workspace bytes.');
  assert.deepEqual(retained(await saved(profile)), retained(value), 'A blocked refresh must preserve the saved reading.');
  evidence.steps.push({ label, quotaAndActivityBlocked: true, noProviderExecution: true, encryptedStorageUnchanged: true, savedValuesUnchanged: true });
}
async function capture(name, profile, waiting) {
  await waitFor(async () => driver.execute('const node=document.querySelector(arguments[0]);return Boolean(node)&&node.disabled===arguments[1];', [readSelector(profile.id), waiting]), 'quota control matches Rust cooldown');
  if (waiting) await waitFor(async () => driver.execute('const node=document.querySelector(arguments[0]);return Boolean(node&&node.getBoundingClientRect().width&&node.getBoundingClientRect().height&&node.textContent.trim());', [statusSelector(profile.id)]), 'visible refresh wait notice');
  const view = await driver.execute('const button=document.querySelector(arguments[0]),notice=document.querySelector(arguments[1]),activity=document.querySelector(arguments[2]);(notice??button).scrollIntoView({block:"center"});return {readDisabled:button.disabled,readLabel:button.textContent.trim(),activityDisabled:activity?.disabled??null,notice:notice?.textContent.trim()??null,width:innerWidth,pageWidth:document.scrollingElement.scrollWidth};', [readSelector(profile.id), statusSelector(profile.id), activitySelector(profile.id)]);
  assert.equal(view.readDisabled, waiting); assert.equal(view.activityDisabled, waiting);
  assert.ok(view.pageWidth <= view.width + 1, 'The refresh controls must fit the native viewport.');
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
  for (const path of ['refresh-cooldown.mjs', 'private-fixture.mjs', 'webdriver.mjs', 'fixtures/synthetic-quota-provider.py']) evidence.harnessHashes[path] = await hash(join(harness, path));
  fixture = await createFixture('refresh');
  fixture.environment.HOME = join(fixture.root, 'synthetic-home'); await mkdir(fixture.environment.HOME, { mode: 0o700 });
  workspace = dirname(fixture.database); executable = join(fixture.root, 'synthetic-quota-provider.py'); executionLog = join(fixture.root, 'synthetic-quota-observations.jsonl');
  await copyFile(join(harness, 'fixtures/synthetic-quota-provider.py'), executable); await chmod(executable, 0o700); assert.equal(await hash(executable), fixtureDigest);
  driver = new NativeDriver(application, fixture.environment, join(output, 'driver.log')); await driver.start();
  await waitFor(async () => driver.execute('return ["#workspace-passphrase","#workspace-confirmation"].every(selector=>{const node=document.querySelector(selector);return Boolean(node&&!node.disabled);});'), 'fresh fallback setup');
  await driver.type('#workspace-passphrase', password); await driver.type('#workspace-confirmation', password); await driver.click('.unlock-form button[type=submit]');
  await waitFor(async () => (await driver.invoke('bootstrap')).value?.storage === 'ready', 'private encrypted workspace');
  const first = await create('Refresh cooldown fixture'), other = await create('Independent refresh fixture');
  await approve(first);
  await driver.setAppearance('light');
  await waitFor(async () => driver.execute('return document.documentElement.dataset.theme==="light"&&!document.querySelector("#theme").disabled;'), 'light appearance');
  assert.equal(await status(first, 'before first refresh'), 0); assert.equal(await status(other, 'unused profile'), 0);
  const protectedKey = await hash(join(workspace, 'database-key.v1'));
  const firstStart = process.hrtime.bigint(), initial = await guiRead(first), initialView = await quotaView(first);
  assert.ok(initialView); assert.equal(initialView.time, initial.observedAtUtc);
  assert.deepEqual(initialView.meters.slice(0, 2), [{ value: '75', label: 'Five-hour: 75% remaining (derived)' }, { value: '0', label: 'Seven-day: 0% remaining (derived)' }]);
  await blocked(first, 'immediate retry');
  await capture('refresh-active-wait-light-desktop.png', first, true);

  const beforeReload = await liveHashes();
  await driver.request('POST', `/session/${driver.session}/refresh`, {});
  await waitFor(async () => driver.execute('return Boolean(window.__TAURI__?.core?.invoke&&document.querySelector("#profile-label"));'), 'renderer reload in the same native session');
  assert.deepEqual(await liveHashes(), beforeReload);
  assert.ok(await status(first, 'after renderer reload') > 0);
  await approve(first);
  await blocked(first, 'after renderer reload and explicit reapproval');
  await waitFor(async () => driver.execute('return Boolean(document.querySelector(arguments[0]));', [savedSelector(first.id)]), 'saved quota after renderer reload');
  assert.deepEqual(await quotaView(first), initialView);

  assert.equal(await status(other, 'other profile remains ready'), 0);
  const beforeOther = await saved(other), offset = (await observations()).length;
  const begun = await driver.invoke('begin_profile_quota_read', { profileId: other.id, approvalId }); assert.equal(begun.ok, true);
  assert.deepEqual(Object.keys(begun.value).sort(), ['accepted', 'approvalId', 'monitoring', 'profileId', 'requestId', 'sourceKind']);
  assert.equal(begun.value.profileId, other.id); assert.equal(begun.value.approvalId, approvalId); assert.equal(begun.value.sourceKind, 'test_fixture'); assert.equal(begun.value.accepted, false); assert.equal(begun.value.monitoring, 'stopped');
  const completion = await driver.invoke('complete_profile_quota_read', { profileId: other.id, requestId: begun.value.requestId }); assert.equal(completion.ok, true);
  await completedRead(other, beforeOther, offset);
  assert.deepEqual(retained(await saved(first)), retained(initial));
  assert.ok(await status(first, 'first profile still waiting after another profile read') > 0);
  evidence.steps.push({ separateProfileAllowedDuringFirstCooldown: true, firstProfileSavedValuesUnchanged: true, beginReceiptUnchanged: true });

  const waitingStorage = await liveHashes(), waitingExecutions = await observations();
  let latestStatus;
  await waitFor(async () => { latestStatus = await status(first, 'real-time wait'); return latestStatus === 0; }, 'real thirty-second refresh cooldown elapsed', 45000);
  const elapsedMilliseconds = elapsed(firstStart); assert.ok(elapsedMilliseconds >= 30000, 'The native refresh must wait thirty real seconds.');
  assert.deepEqual(await liveHashes(), waitingStorage); assert.deepEqual(await observations(), waitingExecutions);
  assert.deepEqual(retained(await saved(first)), retained(initial)); assert.deepEqual(await quotaView(first), initialView);
  await capture('refresh-ready-light-desktop.png', first, false);
  const replacement = await guiRead(first); assert.notEqual(replacement.collectionRunId, initial.collectionRunId);
  await blocked(first, 'new cooldown after the permitted GUI refresh');
  evidence.steps.push({ realElapsedMilliseconds: elapsedMilliseconds, statusReachedZero: latestStatus === 0, noExecutionOrStorageWriteWhileWaiting: true, permittedGuiRefreshSavedNewRun: true, nextImmediateQuotaAndActivityRefused: true });
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
  evidence.finishedAt = new Date().toISOString();
  await writeFile(join(output, 'result.json'), `${JSON.stringify(evidence, null, 2)}\n`, { mode: 0o600 });
  console.log(`${evidence.requirement}: ${evidence.result}. Evidence: ${output}`); if (evidence.error) console.error(evidence.error);
}
