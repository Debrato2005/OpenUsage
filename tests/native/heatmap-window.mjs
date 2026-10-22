import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { chmod, copyFile, mkdir, readFile, readlink, rm, writeFile } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { createFixture, stopGroup } from './private-fixture.mjs';
import { NativeDriver, waitFor } from './webdriver.mjs';

// Native-test WebKitGTK with the approved synthetic provider in a private workspace.
const application = resolve(process.env.OPENUSAGE_NATIVE_BINARY);
const output = resolve(process.env.OPENUSAGE_NATIVE_EVIDENCE);
const evidence = { result: 'failed', scope: 'Linux native-test Tauri window with approved synthetic-provider readings. Full-year Usage, compact Overview, and real GTK title-bar controls under WSLg/Weston. No real provider, combined totals or other-platform qualification.' };
let fixture, driver;
let sequence = 0;
try {
  await mkdir(output, { recursive: true, mode: 0o700 });
  evidence.applicationSha256 = createHash('sha256').update(await readFile(application)).digest('hex');
  fixture = await createFixture('heatmap-window');
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
  await waitFor(async () => Boolean((await driver.invoke('load_profile_activity', { profileId: a.id })).value), 'paired saved activity');
  await driver.request('POST', `/session/${driver.session}/actions`, { actions: [{ type: 'key', id: 'close-account-dialog', actions: [{ type: 'keyDown', value: '\uE00C' }, { type: 'keyUp', value: '\uE00C' }] }] });
  await waitFor(() => driver.execute('return !document.querySelector("[data-account-panel]")?.open;'), 'Escape closes account details');
  await driver.navigate('overview');
  await waitFor(() => driver.execute('return Boolean(document.querySelector("[data-overview] [data-heatmap-grid]"));'), 'compact Overview calendar');
  assert.equal(await driver.execute('return document.querySelector("[data-overview] [data-token-heatmap]").dataset.periodDays;'), '90');
  await driver.navigate('usage');
  await waitFor(() => driver.execute('return document.querySelector("[data-workspace-view=usage] [data-token-heatmap]")?.dataset.periodDays==="365"&&document.querySelectorAll("[data-workspace-view=usage] [data-heatmap-cell]").length===365;'), 'full-year native Usage heatmap');
  const size = await driver.execute('const n=document.querySelector("[data-workspace-view=usage] [data-token-heatmap]");return {panel:n.clientWidth,plot:n.querySelector(".react-activity-calendar svg").getBoundingClientRect().width};');
  assert.ok(size.plot > size.panel * .75, JSON.stringify(size));
  const days = await driver.execute('return [...document.querySelectorAll("[data-workspace-view=usage] .react-activity-calendar svg text")].map(n=>n.textContent);');
  for (const day of ['Mon','Tue','Wed','Thu','Fri','Sat','Sun']) assert.ok(days.includes(day));
  const grid = await driver.execute('const n=document.querySelector("[data-workspace-view=usage] [data-token-heatmap]");return {readings:n.querySelectorAll("[data-heatmap-cell]").length,padding:n.querySelectorAll("[data-heatmap-padding]").length,interactivePadding:n.querySelectorAll("[data-heatmap-padding][role=button]").length};');
  assert.equal((grid.readings + grid.padding) % 7, 0);
  assert.equal(grid.interactivePadding, 0);
  evidence.calendar = grid;
  assert.match(await driver.execute('return document.querySelector("[data-workspace-view=usage] [data-heatmap-coverage]").textContent;'), /7 of 365 days reported/);
  const queue = [driver.child.pid]; let owned;
  for (let n = 0; queue.length && n < 64; n++) {
    const pid = queue.shift();
    try { if (await readlink(`/proc/${pid}/exe`) === application) { owned = pid; break; } queue.push(...(await readFile(`/proc/${pid}/task/${pid}/children`, 'utf8')).trim().split(/\s+/).filter(Boolean).map(Number)); }
    catch (error) { if (error.code !== 'ENOENT') throw error; }
  }
  assert.ok(owned);
  const focus = JSON.parse(execFileSync('/usr/bin/python3', [resolve('tests/native/x11-close.py'), String(owned), 'focus'], { env: fixture.environment, encoding: 'utf8', timeout: 10000 }));
  execFileSync('/usr/bin/import', ['-silent', '-window', String(focus.windowId), join(output, 'native-year-heatmap.png')], { env: fixture.environment, timeout: 10000 });
  evidence.windowManager = execFileSync('/usr/bin/xprop', ['-root', '_NET_SUPPORTING_WM_CHECK'], { env: fixture.environment, encoding: 'utf8', timeout: 5000 }).trim();
  const click = action => JSON.parse(execFileSync('/usr/bin/python3', [resolve('tests/native/x11-titlebar.py'), String(owned), String(focus.windowId), action], { env: fixture.environment, encoding: 'utf8', timeout: 10000 }));
  const state = () => execFileSync('/usr/bin/xprop', ['-id', String(focus.windowId), '_NET_WM_STATE', 'WM_STATE'], { env: fixture.environment, encoding: 'utf8', timeout: 5000 });
  evidence.controls = [click('maximize')];
  await waitFor(() => state().includes('_NET_WM_STATE_MAXIMIZED_VERT'), 'native maximize button');
  evidence.controls.push(click('maximize'));
  await waitFor(() => !state().includes('_NET_WM_STATE_MAXIMIZED_VERT'), 'native restore button');
  evidence.controls.push(click('minimize'));
  let minimized = /Iconic/.test(state());
  if (!minimized) {
    // Compare the button with a direct native request before reporting a WM limitation.
    evidence.controls.push(click('iconify'));
    minimized = /Iconic/.test(state());
    if (!minimized) evidence.limitation = 'WSLg/Weston keeps WM_STATE Normal after both the native minimize button and a direct GDK iconify request. Host taskbar minimization is unqualified.';
  }
  evidence.controls.push(click('restore'));
  await waitFor(() => /Normal/.test(state()), 'native window restored');
  evidence.controls.push(click('close'));
  await waitFor(async () => { try { await readlink(`/proc/${owned}/exe`); return false; } catch (error) { if (error.code==='ENOENT') return true; throw error; } }, 'native close button exits');
  driver.session = undefined;
  evidence.result = minimized ? 'passed' : 'passed_with_platform_limitation'; evidence.acceptance = { fullYear: true, completeWeekColumns: true, sevenWeekdayLabels: true, fillsWidth: true, compactOverview: true, missingNotZero: true, nativeMaximizeRestoreClose: true, nativeMinimize: minimized };
} catch (error) { evidence.error = error.stack ?? String(error); process.exitCode = 1; }
finally { try { await driver?.stop(); } finally { stopGroup(fixture?.bus, 'SIGKILL'); if (fixture) await rm(fixture.root, { recursive: true, force: true }); await writeFile(join(output, 'result.json'), JSON.stringify(evidence, null, 2)+'\n', { mode: 0o600 }); } }
