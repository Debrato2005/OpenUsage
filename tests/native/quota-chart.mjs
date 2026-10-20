// Native GUI journey with an unchanged, pinned synthetic provider. No real account data.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { chmod, copyFile, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { createFixture, stopGroup } from './private-fixture.mjs';
import { NativeDriver, waitFor } from './webdriver.mjs';
const repository = resolve(process.env.OPENUSAGE_NATIVE_SOURCE_ROOT);
const output = resolve(process.env.OPENUSAGE_NATIVE_EVIDENCE);
const binary = resolve(process.env.OPENUSAGE_NATIVE_BINARY);
const password = 'OpenUsage synthetic quota chart check 2026';
const hash = async path => createHash('sha256').update(await readFile(path)).digest('hex');
const result = { requirement: 'NATIVE-QUOTA-DOUGHNUT-001', result: 'failed', scope: 'Synthetic native quota collection and exact selected-window chart; no real OAuth, account totals, or production-provider metrics.', screenshots: [] };
let fixture, driver;
await mkdir(output, { recursive: true, mode: 0o700 });
try {
  fixture = await createFixture('quota-chart');
  fixture.environment.HOME = join(fixture.root, 'synthetic-home');
  await mkdir(fixture.environment.HOME, { mode: 0o700 });
  const executable = join(fixture.root, 'synthetic-quota-provider.py');
  await copyFile(join(repository, 'tests/native/fixtures/synthetic-quota-provider.py'), executable); await chmod(executable, 0o700);
  result.fixtureSha256 = await hash(executable);
  driver = new NativeDriver(binary, fixture.environment, join(output, 'driver.log')); await driver.start();
  await waitFor(() => driver.execute('return Boolean(document.querySelector("#workspace-passphrase")&&!document.querySelector("#workspace-passphrase").disabled);'), 'fresh setup');
  await driver.type('#workspace-passphrase', password); await driver.type('#workspace-confirmation', password); await driver.click('.unlock-form button[type=submit]');
  await waitFor(async () => (await driver.invoke('bootstrap')).value?.storage === 'ready', 'SQLCipher workspace');
  await driver.openProfileCreate(); await driver.type('#profile-label', 'Synthetic quota chart'); await driver.click('.profile-create-form button[type=submit]');
  let profile; await waitFor(async () => { profile = (await driver.invoke('list_profiles')).value?.profiles[0]; return Boolean(profile); }, 'GUI-created account');
  await driver.navigate('settings'); await driver.click('[data-provider-settings]'); await driver.click('[data-provider-manual]');
  await driver.type('#provider-executable', executable); await driver.click('.provider-inspection-form button[type=submit]');
  await waitFor(() => driver.execute('return Boolean(document.querySelector("#provider-executable-approved")&&!document.querySelector("#provider-executable-approved").disabled);'), 'reviewed synthetic source');
  await driver.click('#provider-executable-approved'); await driver.click('.provider-approval-form button[type=submit]');
  await waitFor(() => driver.execute('return document.activeElement?.matches(".provider-approval-result");'), 'explicit approval');
  await driver.readProfileQuota(profile.id);
  let saved; await waitFor(async () => { saved = (await driver.invoke('load_profile_quota', { profileId: profile.id })).value; return Boolean(saved); }, 'saved exact quotas');
  const window = saved.limits[0].primary; assert.equal(window.derivedRemainingPercent, 75); assert.equal(window.usedPercent, 25);
  const workspace = dirname(fixture.database);
  const hashes = async () => Object.fromEntries(await Promise.all([fixture.database, `${fixture.database}-wal`, join(workspace, 'database-key.v1')].map(async path => [path, await hash(path)])));
  result.views = [];
  for (const theme of ['light', 'dark']) {
    await driver.setAppearance(theme); const before = await hashes();
    await driver.navigate('overview');
    await waitFor(() => driver.execute('return Boolean(document.querySelector("[data-overview-quota-chart]"));'), 'saved quota chart');
    const view = await driver.execute('const root=document.querySelector("[data-overview-quota-chart]"),r=root.getBoundingClientRect();return{label:root.getAttribute("aria-label"),dash:root.querySelector(".overview-quota-ring-value").getAttribute("stroke-dasharray"),center:root.querySelector(".overview-quota-ring-count").textContent,caption:root.querySelector("figcaption").textContent,width:r.width,pageWidth:document.scrollingElement.scrollWidth,viewport:innerWidth};');
    assert.equal(view.dash, '75 100'); assert.equal(view.center, '75%'); assert.match(view.label, /75% remaining.*25% used/); assert.match(view.caption, /25% used/); assert.ok(view.width > 0 && view.pageWidth <= view.viewport + 1);
    const name = `quota-doughnut-${theme}.png`; await driver.screenshot(join(output, name)); result.screenshots.push(name); result.views.push(view);
    assert.deepEqual(await hashes(), before, 'Rendering must preserve encrypted database, WAL and wrapped key bytes.');
  }
  result.result = 'passed';
} catch (error) { result.error = error.stack ?? String(error); process.exitCode = 1; }
finally {
  try { await driver?.stop(); } catch (error) { result.cleanupError = error.message; process.exitCode = 1; }
  stopGroup(fixture?.bus, 'SIGKILL'); if (fixture) await rm(fixture.root, { recursive: true, force: true });
  await writeFile(join(output, 'result.json'), JSON.stringify(result, null, 2) + '\n', { mode: 0o600 });
  console.log(`${result.requirement}: ${result.result}`); if (result.error) console.error(result.error);
}
