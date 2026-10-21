import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { chmod, copyFile, mkdir, readFile, readlink, rm, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createFixture, stopGroup } from './private-fixture.mjs';
import { NativeDriver, waitFor } from './webdriver.mjs';

const repository = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const application = resolve(process.env.OPENUSAGE_NATIVE_BINARY);
const seeder = resolve(process.env.OPENUSAGE_NATIVE_HISTORY_FIXTURE);
const output = resolve(process.env.OPENUSAGE_NATIVE_EVIDENCE);
const password = 'OpenUsage synthetic quota history workspace 2026';
const hash = async path => createHash('sha256').update(await readFile(path)).digest('hex');
const evidence = { requirement: 'NATIVE-QUOTA-HISTORY-001', result: 'failed', scope: 'Actual native WebKitGTK history browsing in a temporary SQLCipher/WAL workspace. Two GUI synthetic sign-ins each automatically save one reading. A test-only Rust helper adds21valid snapshots through production Storage::accept_quota while the application is closed, enabling real20/2paging without bypassing provider cooldowns. Native profile isolation, cursor refusal, exact zero/missing values, read-only bytes, archived history and genuine restart are covered. No production-provider, whole-history export, or other-platform claim.', screenshots: [], readOnly: [], closes: 0 };
let fixture, driver, sequence = 0;
const profiles = [];
async function call(command, args) { const response = await driver.invoke(command, args); assert.equal(response.ok, true, `${command}: ${JSON.stringify(response)}`); return response.value; }
async function open(setup = false) {
  driver = new NativeDriver(application, fixture.environment, join(output, `driver-${++sequence}.log`)); await driver.start();
  await waitFor(() => driver.execute('return Boolean(document.querySelector("#workspace-passphrase")&&!document.querySelector("#workspace-passphrase").disabled);'), 'enabled workspace password field');
  await driver.type('#workspace-passphrase', password);
  if (setup) await driver.type('#workspace-confirmation', password);
  await driver.click('.unlock-form button[type=submit]');
  await waitFor(async () => (await call('bootstrap')).storage === 'ready', 'ready encrypted workspace');
}
async function close() {
  const pending = [driver.child.pid]; let appPid;
  while (pending.length) {
    const pid = pending.shift();
    try { if (await readlink(`/proc/${pid}/exe`) === application) { appPid = pid; break; } pending.push(...(await readFile(`/proc/${pid}/task/${pid}/children`, 'utf8')).trim().split(/\s+/).filter(Boolean).map(Number)); } catch (error) { if (error.code !== 'ENOENT') throw error; }
  }
  assert.ok(appPid); execFileSync('/usr/bin/python3', [join(repository, 'tests/native/x11-close.py'), String(appPid)], { env: fixture.environment, timeout: 10000 });
  await waitFor(async () => { try { await readlink(`/proc/${appPid}/exe`); return false; } catch (error) { if (error.code === 'ENOENT') return true; throw error; } }, 'genuine native exit');
  evidence.closes++; driver.session = undefined; await driver.stop(); driver = undefined;
}
async function liveHashes() {
  const result = {};
  for (const path of [fixture.database, `${fixture.database}-wal`, join(dirname(fixture.database), 'database-key.v1')]) {
    try { result[path] = await hash(path); } catch (error) { if (error.code !== 'ENOENT') throw error; result[path] = null; }
  }
  return result;
}
async function showHistory(profile) {
  await driver.navigate('usage');
  const current = (await call('list_profiles')).profiles.find(item => item.id === profile.id);
  await driver.selectVisibleOption('[aria-label="Usage profile"]', `${profile.label}${current.archived ? ' (archived)' : ''}`);
  const selector = `[data-quota-history="${profile.id}"] [data-quota-history-open]`;
  if (!await driver.execute('return document.querySelector(arguments[0])?.getAttribute("aria-expanded")==="true";', [selector])) await driver.click(selector);
  await waitFor(() => driver.execute('return document.querySelectorAll(arguments[0]+" [data-quota-history-run]").length>0;', [`[data-quota-history="${profile.id}"]`]), 'visible saved quota history');
}
async function rows() { return driver.execute('return [...document.querySelectorAll("[data-quota-history-run]")].map(node=>node.getAttribute("data-quota-history-run"));'); }
async function clickButton(text) {
  const element = await driver.execute('return [...document.querySelectorAll("[data-quota-history] button")].find(node=>node.textContent.trim()===arguments[0]);', [text]);
  assert.ok(element); await driver.request('POST', `/session/${driver.session}/element/${element['element-6066-11e4-a52e-4f735466cecf']}/click`, {});
}
await mkdir(output, { recursive: true, mode: 0o700 });
process.once('exit', () => { driver?.kill('SIGKILL'); stopGroup(fixture?.bus, 'SIGKILL'); });
try {
  evidence.applicationSha256 = await hash(application); evidence.seederSha256 = await hash(seeder);
  fixture = await createFixture('quota-history'); fixture.environment.HOME = join(fixture.root, 'synthetic-home'); await mkdir(fixture.environment.HOME, { mode: 0o700 });
  await open(true);
  const executable = join(fixture.root, 'synthetic-collection-provider.py'); await copyFile(join(repository, 'crates/openusage-codex/tests/fixtures/synthetic-collection-provider.py'), executable); await chmod(executable, 0o700);
  await driver.navigate('settings'); await driver.click('[data-provider-settings]'); await driver.click('[data-provider-manual]'); await driver.type('#provider-executable', executable); await driver.click('.provider-inspection-form button[type=submit]');
  await waitFor(() => driver.execute('return Boolean(document.querySelector("#provider-executable-approved")&&!document.querySelector("#provider-executable-approved").disabled);'), 'explicit fixture source review');
  await driver.click('#provider-executable-approved'); await driver.click('.provider-approval-form button[type=submit]'); await waitFor(() => driver.execute('return Boolean(document.querySelector(".provider-approval-result"));'), 'fixture approval');
  for (let index = 0; index < 2; index++) {
    await driver.navigate('overview'); await driver.click('[data-overview-add-account]');
    let profile; await waitFor(async () => { profile = (await call('list_profiles')).profiles.find(item => !profiles.some(previous => previous.id === item.id)); return Boolean(profile); }, 'new isolated profile'); profiles.push(profile);
    await writeFile(join(dirname(fixture.database), 'profiles', profile.id, 'synthetic-collection-config.json'), JSON.stringify({ mode: 'startup_snapshot', requireLogin: true }) + '\n', { mode: 0o600 });
    await driver.navigate('overview'); await driver.click(`[data-overview-account="${profile.id}"] td:last-child button:first-child`);
    await waitFor(async () => Boolean(await call('load_profile_quota', { profileId: profile.id })), 'automatic first quota COMMIT', 45000);
  }
  const otherBefore = await call('load_profile_quota', { profileId: profiles[1].id });
  await close();
  evidence.seed = JSON.parse(execFileSync(seeder, [fixture.root, profiles[0].id], { input: JSON.stringify({ passphrase: password }), encoding: 'utf8', timeout: 30000, env: { PATH: '/usr/bin:/bin' } }));
  await open(); const before = await liveHashes();
  const firstPage = await call('load_profile_quota_history', { profileId: profiles[0].id, beforeRunId: null });
  assert.equal(firstPage.entries.length, 20); assert.ok(firstPage.nextBeforeRunId);
  const nextPage = await call('load_profile_quota_history', { profileId: profiles[0].id, beforeRunId: firstPage.nextBeforeRunId });
  assert.equal(nextPage.entries.length, 2); assert.equal(nextPage.nextBeforeRunId, null);
  assert.equal(new Set([...firstPage.entries, ...nextPage.entries].map(row => row.collectionRunId)).size, 22);
  assert.equal((await driver.invoke('load_profile_quota_history', { profileId: profiles[1].id, beforeRunId: firstPage.nextBeforeRunId })).ok, false);
  await showHistory(profiles[0]); assert.deepEqual(await rows(), firstPage.entries.map(row => row.collectionRunId));
  await clickButton('Older readings'); await waitFor(async () => (await rows()).length === 2, 'native older-page rendering'); assert.deepEqual(await rows(), nextPage.entries.map(row => row.collectionRunId));
  const zero = nextPage.entries.find(row => row.limits[0]?.primary?.usedPercent === 0); assert.ok(zero);
  await driver.click(`[data-quota-history-run="${zero.collectionRunId}"] > button`);
  assert.match(await driver.execute('return document.querySelector(arguments[0]).textContent;', [`[data-quota-history-run="${zero.collectionRunId}"]`]), /0%.*100%/s);
  assert.equal(await driver.execute('return document.querySelector(arguments[0]).textContent.includes("Fresh");', [`[data-quota-history="${profiles[0].id}"]`]), false);
  await driver.screenshot(join(output, 'quota-history-native.png')); evidence.screenshots.push('quota-history-native.png');
  await clickButton('Newer readings'); await waitFor(async () => (await rows()).length === 20, 'native newer-page rendering');
  await showHistory(profiles[1]); const otherHistory = await call('load_profile_quota_history', { profileId: profiles[1].id, beforeRunId: null }); assert.equal(otherHistory.entries.length, 1); assert.deepEqual(await rows(), [otherBefore.collectionRunId]);
  assert.deepEqual(await liveHashes(), before); evidence.readOnly.push('Native paging, exact details, wrong-profile cursor refusal, profile switching preserve database/WAL/key bytes.');
  await driver.openProfileOptions(profiles[0].id); await driver.click(`[data-profile-id="${profiles[0].id}"] [data-profile-action="archive"]`);
  await waitFor(async () => (await call('list_profiles')).profiles.find(profile => profile.id === profiles[0].id)?.archived, 'archived profile retains history');
  await close(); await open();
  await showHistory(profiles[0]); assert.deepEqual(await rows(), firstPage.entries.map(row => row.collectionRunId));
  const afterRestart = await call('load_profile_quota_history', { profileId: profiles[0].id, beforeRunId: null }); assert.deepEqual(afterRestart, firstPage);
  const otherAfter = await call('load_profile_quota', { profileId: profiles[1].id });
  assert.equal(otherAfter.collectionRunId, otherBefore.collectionRunId); assert.equal(otherAfter.observedAtUtc, otherBefore.observedAtUtc);
  evidence.paging = { first: 20, older: 2, unique: 22, actualGuiNewerAndOlder: true, profileCursorIsolation: true, archivedHistoryRetainedAfterGenuineRestart: true, otherAccountUnchanged: true };
  const logs = await Promise.all(profiles.map(profile => readFile(join(dirname(fixture.database), 'profiles', profile.id, 'synthetic-collection-observations.jsonl'), 'utf8')));
  evidence.providerSessions = logs.map(log => log.trim().split('\n').map(JSON.parse).filter(row => row.kind === 'start' && row.arguments[0] === 'app-server').length);
  assert.deepEqual(evidence.providerSessions, [2, 2], 'Only initial login and automatic collection; history must not launch a provider.');
  assert.equal(await hash(application), evidence.applicationSha256); assert.equal(await hash(seeder), evidence.seederSha256);
  await close(); evidence.result = 'passed';
} catch (error) { evidence.error = error.stack ?? String(error); process.exitCode = 1; if (driver?.session) try { await driver.screenshot(join(output, 'failure.png')); } catch {} }
finally {
  try { await driver?.stop(); } catch {} stopGroup(fixture?.bus, 'SIGKILL'); if (fixture) await rm(fixture.root, { recursive: true, force: true });
  await writeFile(join(output, 'result.json'), JSON.stringify(evidence, null, 2) + '\n', { mode: 0o600 });
  console.log(`${evidence.requirement}: ${evidence.result}`); if (evidence.error) console.error(evidence.error);
}
