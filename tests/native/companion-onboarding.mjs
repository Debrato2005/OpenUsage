import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { lstat, mkdir, readFile, readdir, readlink, rm, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { CODEX_SENTINEL, createFixture, stopGroup } from './private-fixture.mjs';
import { NativeDriver, waitFor } from './webdriver.mjs';

assert.ok(process.env.OPENUSAGE_NATIVE_BINARY, 'A frozen production application is required.');
const application = resolve(process.env.OPENUSAGE_NATIVE_BINARY);
const companion = join(dirname(application), 'openusage-codex');
const expectedCompanion = '399ff69c431c066e40de318fba45c2b07f45a76b19939d247ee4828fc70c4215';
const corrupt = process.env.OPENUSAGE_NATIVE_COMPANION_CORRUPT === '1';
const output = resolve(process.env.OPENUSAGE_NATIVE_EVIDENCE ?? join('/tmp', `openusage-companion-onboarding-${randomUUID()}`));
const harness = dirname(fileURLToPath(import.meta.url));
const password = 'OpenUsage synthetic bundled companion workspace 2026';
const hash = async path => createHash('sha256').update(await readFile(path)).digest('hex');
const evidence = {
  requirement: 'NATIVE-COMPANION-ONBOARDING-001', result: 'failed', startedAt: new Date().toISOString(),
  scope: 'Production native GUI with a fixed official bundled Codex companion, fresh synthetic HOME/workspace and PATH without installed Codex. Visible per-account sign-in, an official device challenge cancelled without retaining its code, actual signed-out check, second-profile isolation and genuine restart are covered. No real authenticated session, usage, existing credential inspection, verified identity or release-platform qualification.',
  variant: corrupt ? 'present-corrupt-companion-refusal' : 'official-companion', screenshots: [], steps: [], closes: [],
};
let fixture, driver, workspace, sequence = 0;

async function call(command, args = {}) {
  const response = await driver.invoke(command, args);
  assert.equal(response.ok, true, `${command}: ${JSON.stringify(response)}`);
  return response.value;
}
async function profiles() { return (await call('list_profiles')).profiles; }
async function homeFiles(path) {
  const entries = {};
  async function visit(directory, relative) {
    for (const entry of await readdir(directory, { withFileTypes: true })) {
      const full = join(directory, entry.name), name = join(relative, entry.name), metadata = await lstat(full);
      if (entry.isSymbolicLink()) {
        // The pinned official CLI creates these aliases. Record targets without following them.
        assert.match(name, /^tmp\/arg0-linux\/codex-arg0[^/]+\/(?:apply_patch|applypatch|codex-linux-sandbox)$/);
        const target = await readlink(full);
        assert.equal(target, companion);
        entries[name] = { link: target };
      }
      else if (entry.isDirectory()) { entries[name] = { directory: true, mode: metadata.mode & 0o777 }; await visit(full, name); }
      else { assert.equal(entry.isFile(), true); entries[name] = { sha256: await hash(full), mode: metadata.mode & 0o777 }; }
    }
  }
  await visit(path, '');
  return entries;
}
async function open(setup = false) {
  driver = new NativeDriver(application, fixture.environment, join(output, `driver-${++sequence}.log`));
  driver.logs = []; driver.logs.push = () => 0;
  await driver.start();
  await waitFor(() => driver.execute('const input=document.querySelector("#workspace-passphrase"),confirmation=document.querySelector("#workspace-confirmation");return Boolean(input&&!input.disabled&&(!arguments[0]||confirmation&&!confirmation.disabled));', [setup]), 'private workspace setup/unlock');
  await driver.type('#workspace-passphrase', password);
  if (setup) await driver.type('#workspace-confirmation', password);
  await driver.click('.unlock-form button[type=submit]');
  await waitFor(async () => (await driver.invoke('bootstrap')).value?.storage === 'ready', 'unlocked private workspace');
  assert.equal((await call('bootstrap')).schemaVersion, 5);
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
  assert.fail('The owned application process was not found.');
}
async function closeNormally() {
  const pid = await desktopPid();
  evidence.closes.push(JSON.parse(execFileSync('/usr/bin/python3', [join(harness, 'x11-close.py'), String(pid)], { env: fixture.environment, encoding: 'utf8', timeout: 10000 })));
  await waitFor(async () => {
    try { await readlink(`/proc/${pid}/exe`); return false; }
    catch (error) { if (error.code === 'ENOENT') return true; throw error; }
  }, 'genuine native window exit');
  driver.session = undefined; await driver.stop(); driver = undefined;
}
async function capture(name) {
  assert.equal(await driver.execute('return Boolean(document.querySelector(".login-prompt input"));'), false, 'Never capture a live authentication challenge.');
  await driver.screenshot(join(output, name));
  evidence.screenshots.push(name);
}
async function add(label, success = true) {
  await driver.navigate('accounts');
  await driver.click('[data-account-connect]');
  // Preparation verifies the large companion while holding the workspace lock.
  // Observe its visible completion before issuing another workspace command.
  await waitFor(() => driver.execute('const row=[...document.querySelectorAll(".profile-list > li")].find(node=>node.querySelector(".profile-label")?.textContent===arguments[0]);return Boolean(row&&(!arguments[1]||[...row.querySelectorAll("[data-login-start]")].some(button=>!button.disabled&&button.getClientRects().length)));', [label, success]), `${label} row prepared`, 60000);
  let profile;
  await waitFor(async () => { profile = (await profiles()).find(row => row.label === label); return Boolean(profile); }, `GUI-created ${label}`);
  if (success) await waitFor(() => driver.execute('const button=document.querySelector(arguments[0]);return Boolean(button&&!button.disabled&&button.getClientRects().length&&document.activeElement===button);', [`[data-login-start="${profile.id}"]`]), `${label} prepared and Sign in focused`, 60000);
  return profile;
}
async function assertNormalFlow() {
  const view = await driver.execute(`
    const visible=node=>Boolean(node&&node.getClientRects().length&&getComputedStyle(node).visibility!=='hidden');
    return {view:document.querySelector('[data-workspace-nav][aria-current=page]')?.getAttribute('data-workspace-nav'),approvalCheckbox:visible(document.querySelector('#provider-executable-approved')),pathField:visible(document.querySelector('#provider-executable')),technicalPanel:visible(document.querySelector('.provider-panel')),devicePrompts:document.querySelectorAll('.login-prompt,[data-login-cancel]').length};
  `);
  assert.deepEqual(view, { view: 'accounts', approvalCheckbox: false, pathField: false, technicalPanel: false, devicePrompts: 0 });
  return view;
}
function approval(value) {
  assert.equal(value.path, companion);
  assert.equal(value.sha256, expectedCompanion);
  assert.equal(value.sourceKind, 'official_artifact');
  assert.equal(value.sourceVerified, true);
  assert.equal(value.testedVersion, '0.163.0');
  assert.equal(value.versionChecked, false);
  assert.match(value.approvalId, /^[0-9a-f-]{36}$/);
}
async function noMetrics(profile) {
  assert.equal(await call('load_profile_activity', { profileId: profile.id }), null);
  assert.equal(await call('load_profile_quota', { profileId: profile.id }), null);
  assert.equal((await profiles()).find(row => row.id === profile.id).connectionState, 'authentication_required');
}

await mkdir(output, { recursive: true, mode: 0o700 });
process.once('exit', () => { driver?.kill('SIGKILL'); stopGroup(fixture?.bus, 'SIGKILL'); });
try {
  const metadata = await lstat(companion);
  assert.equal(metadata.isFile(), true); assert.equal(metadata.isSymbolicLink(), false);
  const digest = await hash(companion);
  if (corrupt) assert.notEqual(digest, expectedCompanion); else assert.equal(digest, expectedCompanion);
  evidence.application = { path: application, sha256: await hash(application) };
  evidence.companion = { path: companion, sha256: digest, expectedSha256: expectedCompanion };
  evidence.script = { path: fileURLToPath(import.meta.url), sha256: await hash(fileURLToPath(import.meta.url)) };
  fixture = await createFixture('companion-onboarding');
  fixture.environment.HOME = join(fixture.root, 'synthetic-user-home');
  fixture.environment.PATH = '/usr/bin:/bin';
  for (const name of ['OPENAI_API_KEY', 'OPENAI_ACCESS_TOKEN', 'CODEX_API_KEY', 'CHATGPT_ACCESS_TOKEN']) delete fixture.environment[name];
  await mkdir(fixture.environment.HOME, { mode: 0o700 });
  for (const path of ['/usr/bin/codex', '/bin/codex']) await assert.rejects(lstat(path), { code: 'ENOENT' });
  workspace = dirname(fixture.database);
  await open(true);
  assert.deepEqual(await profiles(), []);
  assert.deepEqual(await driver.invoke('prepare_provider_connection', { extra: true }), { ok: false, error: 'invalid_arguments' });
  const first = await add('Account A', !corrupt);
  if (corrupt) {
    await waitFor(() => driver.execute('return [...document.querySelectorAll("[role=alert]")].some(node=>node.getClientRects().length&&/could not verify|could not prepare|not available/i.test(node.textContent));'), 'visible corrupt companion failure');
    const refused = await driver.invoke('prepare_provider_connection');
    assert.equal(refused.ok, false);
    assert.ok(['provider_setup_unavailable', 'provider_source_unverified', 'invalid_provider_executable'].includes(refused.error));
    assert.equal(await driver.execute('return [...document.querySelectorAll("[data-login-start]")].some(node=>node.getClientRects().length);'), false);
    await noMetrics(first);
    await assert.rejects(lstat(`${companion}.executed`), { code: 'ENOENT' });
    await capture('companion-corrupt-refused.png');
    evidence.steps.push({ corruptCompanionRefused: true, error: refused.error, noSignInAction: true });
  } else {
    const applications = join(fixture.dirs.data, 'applications'); await mkdir(applications, { mode: 0o700 });
    const handler = join(fixture.root, 'test-browser.py');
    await writeFile(handler, '#!/usr/bin/python3\nimport sys\nassert sys.argv[1:] == ["https://auth.openai.com/codex/device"]\n', { mode: 0o700 });
    await writeFile(join(applications, 'openusage-test-browser.desktop'), `[Desktop Entry]\nType=Application\nName=OpenUsage test browser\nExec=/usr/bin/python3 ${handler} %u\nNoDisplay=true\nTerminal=false\nMimeType=x-scheme-handler/https;\n`, { mode: 0o600 });
    await writeFile(join(fixture.dirs.config, 'mimeapps.list'), '[Default Applications]\nx-scheme-handler/https=openusage-test-browser.desktop;\n', { mode: 0o600 });
    const prepared = await call('prepare_provider_connection'); approval(prepared);
    evidence.steps.push({ addAccountPreparedOfficialCompanion: true, normalFlow: await assertNormalFlow(), approval: prepared });
    await capture('companion-account-a-sign-in.png');
    // The test-owned browser handler prevents desktop browser launch and retains no code.
    await driver.click(`[data-login-start="${first.id}"]`);
    await waitFor(() => driver.execute('const code=document.querySelector(arguments[0]);return Boolean(code&&code.getClientRects().length&&code.value.length>0);', [`#login-code-${first.id}`]), 'official device prompt without retaining its value', 60000);
    await driver.click(`[data-login-cancel="${first.id}"]`);
    await waitFor(() => driver.execute('return !document.querySelector(arguments[0])&&Boolean(document.querySelector(arguments[1]));', [`#login-code-${first.id}`, `[data-login-start="${first.id}"]`]), 'cancel clears the real challenge');
    await driver.openProfileOptions(first.id);
    await driver.click(`[data-provider-check="${first.id}"]`);
    await waitFor(() => driver.execute('const result=document.querySelector(arguments[0]);return document.activeElement===result&&Boolean(result.querySelector("strong"))&&/no authorized account/.test(result.textContent);', [`[data-provider-profile="${first.id}"].provider-diagnostic`]), 'actual official signed-out account response', 60000);
    assert.match(await driver.execute('return document.querySelector(arguments[0]).textContent;', [`[data-provider-profile="${first.id}"].provider-diagnostic`]), /Sign-in required.*no authorized account.*Codex 0\.163\.0/s);
    await noMetrics(first);
    const firstHome = join(workspace, 'profiles', first.id), before = await homeFiles(firstHome);
    const keyHash = await hash(join(workspace, 'database-key.v1'));
    await capture('companion-official-signed-out.png');
    const second = await add('Account B');
    assert.notEqual(second.id, first.id);
    const secondHome = join(workspace, 'profiles', second.id);
    const firstMetadata = await lstat(firstHome), secondMetadata = await lstat(secondHome);
    assert.equal(firstMetadata.isDirectory(), true); assert.equal(secondMetadata.isDirectory(), true);
    assert.notEqual(`${firstMetadata.dev}:${firstMetadata.ino}`, `${secondMetadata.dev}:${secondMetadata.ino}`);
    assert.equal(firstMetadata.mode & 0o777, 0o700); assert.equal(secondMetadata.mode & 0o777, 0o700);
    assert.deepEqual(await homeFiles(firstHome), before);
    assert.equal(await hash(join(workspace, 'database-key.v1')), keyHash);
    const reused = await call('prepare_provider_connection'); approval(reused);
    assert.equal(reused.approvalId, prepared.approvalId, 'Adding another account must preserve the existing provider context.');
    await assertNormalFlow(); await noMetrics(first); await noMetrics(second);
    const originals = await profiles();
    const homes = await Promise.all(originals.map(row => homeFiles(join(workspace, 'profiles', row.id))));
    evidence.steps.push({ actualOfficialSignedOutCheck: true, officialDeviceChallengeCancelledWithoutCapture: true, secondAddPreservesFirstHomeAndApproval: true, distinctManagedHomes: true });
    await closeNormally();
    await open();
    assert.deepEqual(await profiles(), originals);
    assert.deepEqual(await Promise.all(originals.map(row => homeFiles(join(workspace, 'profiles', row.id)))), homes);
    assert.equal(await hash(join(workspace, 'database-key.v1')), keyHash);
    assert.deepEqual(await driver.invoke('check_profile_connection', { profileId: first.id }), { ok: false, error: 'provider_executable_not_approved' });
    await noMetrics(first); await noMetrics(second);
    const monitoring = await call('load_monitoring'); assert.equal(monitoring.state, 'stopped');
    await driver.navigate('accounts');
    for (const profile of [first, second]) {
      await waitFor(() => driver.execute('const button=document.querySelector(arguments[0]);return Boolean(button&&button.getClientRects().length&&!button.disabled);', [`[data-profile-connect="${profile.id}"]`]), 'each signed-out restored row has visible Sign in');
    }
    await noMetrics(first); await noMetrics(second);
    assert.deepEqual(await homeFiles(firstHome), homes[originals.findIndex(row => row.id === first.id)]);
    evidence.steps.push({ visiblePerAccountSignInAfterRestart: true, firstAccountHomePreserved: true });
    await capture('companion-restart-two-signed-out-accounts.png');
    evidence.steps.push({ genuineRestart: true, retainedProfiles: originals, keyAndHomesPreserved: true, noRuntimeApprovalOrAuthenticationCarriedAcrossRestart: true, monitoring: monitoring.state });
  }
  assert.equal(await readFile(join(fixture.dirs.codex, 'synthetic-fixture.txt'), 'utf8'), CODEX_SENTINEL);
  assert.deepEqual(await readdir(fixture.dirs.codex), ['synthetic-fixture.txt']);
  assert.equal(await hash(companion), digest);
  evidence.syntheticCodexSentinelPreserved = true;
  await closeNormally(); evidence.result = 'passed';
} catch (error) {
  evidence.error = error.stack ?? String(error); process.exitCode = 1;
  if (driver?.session) try { await capture('failure.png'); } catch {}
} finally {
  try { await driver?.stop(); } catch (error) { evidence.cleanupError = error.message; evidence.result = 'failed'; process.exitCode = 1; }
  stopGroup(fixture?.bus);
  if (fixture) try { await rm(fixture.root, { recursive: true, force: true }); evidence.privateFixtureRemoved = true; }
  catch (error) { evidence.cleanupError = error.message; evidence.result = 'failed'; process.exitCode = 1; }
  evidence.finishedAt = new Date().toISOString();
  await writeFile(join(output, 'result.json'), `${JSON.stringify(evidence, null, 2)}\n`, { mode: 0o600 });
  console.log(`${evidence.requirement}: ${evidence.result}`);
  if (evidence.error) console.error(evidence.error);
}
