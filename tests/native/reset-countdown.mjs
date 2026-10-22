import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { chmod, copyFile, mkdir, readFile, readlink, rm, writeFile } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { createFixture, stopGroup } from './private-fixture.mjs';
import { NativeDriver, waitFor } from './webdriver.mjs';

// Native-test WebKitGTK with the approved synthetic provider and a controlled renderer clock.
const application = resolve(process.env.OPENUSAGE_NATIVE_BINARY);
const output = resolve(process.env.OPENUSAGE_NATIVE_EVIDENCE);
const evidence = { result: 'failed', scope: 'Linux native-test Tauri countdown display with approved synthetic-provider saved readings and a controlled renderer clock. No real account, production-provider or other-platform qualification.' };
let fixture, driver;
let sequence = 0;
async function select(label) { await driver.selectVisibleOption('#overview-profile', label); }
try {
  await mkdir(output, { recursive: true, mode: 0o700 });
  evidence.applicationSha256 = createHash('sha256').update(await readFile(application)).digest('hex');
  fixture = await createFixture('reset-countdown');
  fixture.environment.HOME = join(fixture.root, 'synthetic-home');
  await mkdir(fixture.environment.HOME, { mode: 0o700 });
  driver = new NativeDriver(application, fixture.environment, join(output, `driver-${++sequence}.log`));
  await driver.start();
  await waitFor(() => driver.execute('return Boolean(document.querySelector("#workspace-passphrase")&&!document.querySelector("#workspace-passphrase").disabled);'), 'private workspace setup');
  const password = 'OpenUsage synthetic countdown workspace 2026';
  assert.equal((await driver.invoke('load_time_zone')).error, 'workspace_locked');
  await driver.type('#workspace-passphrase', password);
  await driver.type('#workspace-confirmation', password);
  await driver.click('.unlock-form button[type=submit]');
  await waitFor(async () => (await driver.invoke('bootstrap')).value?.storage === 'ready', 'ready workspace');
  for (const label of ['Countdown account A', 'Countdown account B']) {
    await driver.openProfileCreate();
    await driver.type('#profile-label', label);
    await driver.click('.profile-create-form button[type=submit]');
    await waitFor(() => driver.execute('return document.querySelector("#profile-label")?.value==="";'), 'profile created');
  }
  const profiles = (await driver.invoke('list_profiles')).value.profiles;
  const a = profiles.find(p => p.label === 'Countdown account A'), b = profiles.find(p => p.label === 'Countdown account B');
  assert.ok(a && b);
  const executable = join(fixture.root, 'synthetic-collection-provider.py');
  await copyFile(resolve('crates/openusage-codex/tests/fixtures/synthetic-collection-provider.py'), executable);
  await chmod(executable, 0o700);
  await driver.navigate('settings');
  await driver.click('[data-provider-settings]'); await driver.click('[data-provider-manual]');
  await driver.type('#provider-executable', executable); await driver.click('.provider-inspection-form button[type=submit]');
  await waitFor(() => driver.execute('return Boolean(document.querySelector("#provider-executable-approved")&&!document.querySelector("#provider-executable-approved").disabled);'), 'reviewed synthetic source');
  await driver.click('#provider-executable-approved'); await driver.click('.provider-approval-form button[type=submit]');
  await waitFor(() => driver.execute('return Boolean(document.querySelector(".provider-approval-result"));'), 'fixture approved');
  const home = join(fixture.dirs.data, 'app.openusage.desktop/workspace/profiles', a.id);
  await writeFile(join(home, 'synthetic-collection-config.json'), JSON.stringify({ mode: 'dashboard_success', requireLogin: true }), { mode: 0o600 });
  await driver.navigate('overview');
  await driver.click(`[data-overview-account="${a.id}"] td:last-child button:first-child`);
  let quota;
  await waitFor(async () => { const reply = await driver.invoke('load_profile_quota', { profileId: a.id }); quota = reply.value; return Boolean(reply.ok && quota); }, 'synthetic sign-in saves initial quota', 45000);
  const observations = join(home, 'synthetic-collection-observations.jsonl');
  const providerBefore = await readFile(observations, 'utf8');
  const deadline = quota.limits[0].primary.resetsAtUnix * 1000;
  await driver.execute('const original=Date.now;const offset=arguments[0]-8000-original();Date.now=()=>original()+offset;window.dispatchEvent(new Event("focus"));return true;', [deadline]);
  await driver.navigate('overview');
  await waitFor(() => driver.execute('return document.querySelector("[data-overview] [data-reset-countdown]")?.textContent.trim()==="In 1m";'), 'live native countdown');
  assert.equal(await driver.execute('return document.querySelectorAll("[data-overview] [data-reset-countdown]").length;'), 2);
  await waitFor(() => driver.execute('return document.querySelector("[data-overview] [data-reset-countdown=due]")?.textContent.trim()==="Awaiting reset confirmation";'), 'expected deadline passes locally');
  assert.equal(await driver.execute('return document.querySelector("[data-overview] [role=meter]").getAttribute("aria-valuenow");'), '75');
  assert.match(await driver.execute('return document.querySelector(".overview-reset time").textContent;'), /UTC/);
  await driver.screenshot(join(output, 'reset-countdown.png'));
  await select(b.label);
  await waitFor(() => driver.execute('return document.querySelector("[data-overview]").dataset.overviewProfile===arguments[0]&&!document.querySelector("[data-overview] [data-reset-countdown]");', [b.id]), 'missing timestamp stays unavailable for second account');
  await driver.openProfileReadings(a.id);
  await waitFor(() => driver.execute('return document.querySelector("[data-quota-saved] [data-reset-countdown=due]")?.textContent.trim()==="Awaiting reset confirmation";'), 'native account details countdown');
  assert.equal(await readFile(observations, 'utf8'), providerBefore);
  assert.deepEqual((await driver.invoke('load_profile_quota', { profileId: a.id })).value.limits, quota.limits);
  await driver.navigate('settings');
  await driver.selectVisibleOption('#time-zone', 'India · Kolkata (UTC+05:30)');
  await waitFor(async () => (await driver.invoke('load_time_zone')).value?.timeZone === 'Asia/Kolkata', 'India preference saved by Rust');
  const clockBefore = await driver.execute('return document.querySelector("[data-live-clock]").dateTime;');
  await waitFor(() => driver.execute('return document.querySelector("[data-live-clock]").dateTime!==arguments[0];', [clockBefore]), 'native clock ticks');
  assert.match(await driver.execute('return document.querySelector("[data-live-clock]").textContent;'), /GMT\+5:30|GMT\+05:30|IST/);
  for (const args of [{ timeZone: 'Invalid/Zone' }, { timeZone: '../UTC' }, { timeZone: 'UTC', extra: true }]) {
    assert.equal((await driver.invoke('set_time_zone', args)).error, 'invalid_arguments');
    assert.equal((await driver.invoke('load_time_zone')).value.timeZone, 'Asia/Kolkata');
  }
  const archivePassword = 'OpenUsage synthetic timezone archive 2026';
  const archive = await driver.invoke('create_backup', { password: archivePassword }); assert.equal(archive.ok, true);
  const stale = await driver.invoke('prepare_restore_preview', { fileName: archive.value.fileName, password: archivePassword }); assert.equal(stale.ok, true);
  assert.equal((await driver.invoke('set_time_zone', { timeZone: 'UTC' })).ok, true);
  assert.equal((await driver.invoke('apply_restore', { previewId: stale.value.previewId, confirmed: true })).error, 'stale_restore_preview');
  const preview = await driver.invoke('prepare_restore_preview', { fileName: archive.value.fileName, password: archivePassword }); assert.equal(preview.ok, true);
  const applied = await driver.invoke('apply_restore', { previewId: preview.value.previewId, confirmed: true }); assert.equal(applied.ok, true);
  assert.equal((await driver.invoke('load_time_zone')).value.timeZone, 'Asia/Kolkata');
  // Close through X11 rather than treating a renderer reload as an application restart.
  const queue = [driver.child.pid]; let owned;
  for (let n = 0; queue.length && n < 64; n++) {
    const pid = queue.shift();
    try { if (await readlink(`/proc/${pid}/exe`) === application) { owned = pid; break; } queue.push(...(await readFile(`/proc/${pid}/task/${pid}/children`, 'utf8')).trim().split(/\s+/).filter(Boolean).map(Number)); }
    catch (error) { if (error.code !== 'ENOENT') throw error; }
  }
  assert.ok(owned);
  execFileSync('/usr/bin/python3', [resolve('tests/native/x11-close.py'), String(owned)], { env: fixture.environment, timeout: 10000 });
  await waitFor(async () => { try { await readlink(`/proc/${owned}/exe`); return false; } catch (error) { if (error.code === 'ENOENT') return true; throw error; } }, 'genuine native exit');
  driver.session = undefined; await driver.stop();
  driver = new NativeDriver(application, fixture.environment, join(output, `driver-${++sequence}.log`)); await driver.start();
  await waitFor(() => driver.execute('return Boolean(document.querySelector("#workspace-passphrase")&&!document.querySelector("#workspace-passphrase").disabled);'), 'restart unlock');
  await driver.type('#workspace-passphrase', password); await driver.click('.unlock-form button[type=submit]');
  await waitFor(async () => (await driver.invoke('bootstrap')).value?.storage === 'ready', 'restart ready');
  await waitFor(() => driver.execute('return /GMT\\+5:30|GMT\\+05:30|IST/.test(document.querySelector("[data-live-clock]")?.textContent);'), 'saved India clock after restart');
  assert.equal((await driver.invoke('load_time_zone')).value.timeZone, 'Asia/Kolkata');
  assert.deepEqual((await driver.invoke('load_profile_quota', { profileId: a.id })).value.limits, quota.limits);
  await driver.navigate('overview');
  await waitFor(() => driver.execute('return Boolean(document.querySelector("[data-heatmap-grid]"));'), 'native heatmap after restart');
  assert.match(await driver.execute('return document.querySelector(".overview-reset time").textContent;'), /05:30/);
  await driver.screenshot(join(output, 'india-clock-heatmap.png'));
  evidence.result = 'passed';
  evidence.acceptance = { liveCountdown: true, bothWindows: true, expectedTimeDoesNotResetQuota: true, utcTimestamp: true, accountIsolation: true, missingTimestamp: true, accountDetails: true, noProviderOperations: true, liveClock: true, indiaTimeZone: true, invalidZonesRefused: true, encryptedBackupRestore: true, genuineRestart: true };
} catch (error) {
  evidence.error = error.stack ?? String(error); process.exitCode = 1;
  try { evidence.display = await driver?.execute('return {selected:document.querySelector("[data-overview]")?.dataset.overviewProfile,quota:document.querySelector("[data-overview-quota]")?.textContent};'); } catch {}
}
finally {
  try { await driver?.stop(); }
  finally { stopGroup(fixture?.bus, 'SIGKILL'); if (fixture) await rm(fixture.root, { recursive: true, force: true }); await writeFile(join(output, 'result.json'), `${JSON.stringify(evidence, null, 2)}\n`, { mode: 0o600 }); }
}
