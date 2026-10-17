import assert from 'node:assert/strict';
import { spawn, execFileSync } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { access, chmod, mkdir, mkdtemp, readFile, readdir, rm, writeFile } from 'node:fs/promises';
import { constants } from 'node:fs';
import { createServer } from 'node:net';
import { arch, platform, release, tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { setTimeout as delay } from 'node:timers/promises';

// Native Linux WebDriver test. No renderer mocks or provider account access.
const repository = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const sourceRepository = resolve(process.env.OPENUSAGE_NATIVE_SOURCE_ROOT ?? repository);
const application = resolve(process.env.OPENUSAGE_NATIVE_BINARY ?? join(repository, 'target/debug/openusage-desktop'));
const output = resolve(process.env.OPENUSAGE_NATIVE_EVIDENCE ?? join(tmpdir(), `openusage-native-evidence-${randomUUID()}`));
const expectedBootstrap = {
  storage: 'setup_required', theme: 'system', schemaVersion: null,
  activeProfiles: null, monitoring: 'stopped', provider: 'not_configured',
};
const evidence = {
  requirement: 'NATIVE-SMOKE-001',
  startedAt: new Date().toISOString(),
  platform: { os: platform(), release: release(), architecture: arch() },
  nodeVersion: process.versions.node,
  fixtureType: 'Native Tauri binary with empty application directories and synthetic CODEX_HOME',
  expectedResult: { bootstrap: expectedBootstrap, unknownCommandRejected: true, invalidArgumentsRejected: true, missingMetrics: 'Not available', keyboardPrivacy: true },
  result: 'failed',
  steps: [],
};
let fixture;
let driver;
let driverFailure;
let session;
let endpoint;

async function request(method, path, body) {
  const response = await fetch(`${endpoint}${path}`, {
    method,
    headers: { 'content-type': 'application/json' },
    ...(body === undefined ? {} : { body: JSON.stringify(body) }),
    signal: AbortSignal.timeout(30000),
  });
  const payload = await response.json();
  if (!response.ok) {
    throw new Error(`WebDriver ${method} ${path}: ${payload.value?.message ?? response.status}`);
  }
  return payload.value;
}

const execute = script => request('POST', `/session/${session}/execute/sync`, { script, args: [] });

async function waitUntil(check, label, timeout = 15000) {
  const deadline = Date.now() + timeout;
  do {
    if (driverFailure) throw driverFailure;
    if (await check()) return;
    await delay(100);
  } while (Date.now() < deadline);
  throw new Error(`Timed out: ${label}`);
}

async function unusedPort() {
  const server = createServer();
  await new Promise((resolvePort, reject) => {
    server.once('error', reject);
    server.listen(0, '127.0.0.1', resolvePort);
  });
  const port = server.address().port;
  await new Promise((resolveClose, reject) => server.close(error => error ? reject(error) : resolveClose()));
  return port;
}

function stopDriver(signal = 'SIGTERM') {
  if (!driver?.pid) return;
  try { process.kill(-driver.pid, signal); } catch (error) {
    if (error.code !== 'ESRCH') throw error;
  }
}

const onSignal = () => stopDriver();
process.once('SIGINT', onSignal);
process.once('SIGTERM', onSignal);
process.once('exit', () => stopDriver('SIGKILL'));

await mkdir(output, { recursive: true, mode: 0o700 });
try {
  assert.equal(platform(), 'linux', 'This external-driver smoke supports Linux only. Other platforms need separate native evidence.');
  assert.ok(Number(process.versions.node.split('.')[0]) >= 22, 'Use the pinned Node 22 toolchain.');
  await access(application, constants.X_OK);
  evidence.application = { path: application, sha256: createHash('sha256').update(await readFile(application)).digest('hex') };
  evidence.sourceVersion = JSON.parse(await readFile(join(sourceRepository, 'package.json'), 'utf8')).version;
  evidence.commit = execFileSync('git', ['rev-parse', 'HEAD'], { cwd: sourceRepository, encoding: 'utf8' }).trim();
  evidence.dirtyPaths = execFileSync('git', ['status', '--porcelain'], { cwd: sourceRepository, encoding: 'utf8' }).trim().split('\n').filter(Boolean);
  fixture = await mkdtemp(join(tmpdir(), 'openusage-native-fixture-'));
  await chmod(fixture, 0o700);
  const isolated = Object.fromEntries(['config', 'data', 'cache', 'state', 'runtime', 'codex'].map(name => [name, join(fixture, name)]));
  await Promise.all(Object.values(isolated).map(path => mkdir(path, { mode: 0o700 })));
  const sentinel = 'OpenUsage native smoke: synthetic home. No credentials.\n';
  await writeFile(join(isolated.codex, 'synthetic-fixture.txt'), sentinel, { mode: 0o600 });
  const [driverPort, nativePort] = await Promise.all([unusedPort(), unusedPort()]);
  endpoint = `http://127.0.0.1:${driverPort}`;
  const driverArgs = ['--port', String(driverPort), '--native-port', String(nativePort), '--native-host', '127.0.0.1'];
  if (process.env.WEBKIT_WEBDRIVER) driverArgs.push('--native-driver', resolve(process.env.WEBKIT_WEBDRIVER));
  const logs = [];
  driver = spawn(process.env.TAURI_DRIVER ?? 'tauri-driver', driverArgs, {
    cwd: fixture,
    detached: true,
    stdio: ['ignore', 'pipe', 'pipe'],
    env: {
      ...process.env,
      CODEX_HOME: isolated.codex,
      XDG_CONFIG_HOME: isolated.config,
      XDG_DATA_HOME: isolated.data,
      XDG_CACHE_HOME: isolated.cache,
      XDG_STATE_HOME: isolated.state,
      XDG_RUNTIME_DIR: isolated.runtime,
    },
  });
  driver.stdout.on('data', chunk => logs.push(chunk));
  driver.stderr.on('data', chunk => logs.push(chunk));
  driver.once('error', error => { driverFailure = error; });
  driver.once('exit', (code, signal) => { driverFailure = new Error(`tauri-driver exited: ${code ?? signal}`); });
  evidence.isolation = { driverEnvironment: 'Synthetic CODEX_HOME and fresh XDG directories. HOME stays unchanged.', osCredentialStore: 'Not accessed by startup. Storage tests require a separate credential namespace.' };
  try {
    await waitUntil(async () => {
      try { return (await request('GET', '/status')).ready === true; } catch { return false; }
    }, 'native WebDriver readiness');
    const created = await request('POST', '/session', {
      capabilities: { alwaysMatch: { 'tauri:options': { application } } },
    });
    session = created.sessionId;
    assert.equal(typeof session, 'string', 'The native driver must create a real session.');
    evidence.capabilities = created.capabilities;
    await request('POST', `/session/${session}/timeouts`, { script: 15000, pageLoad: 30000, implicit: 0 });
    await waitUntil(async () => Boolean(await execute('return window.__TAURI__?.core?.invoke && document.querySelector("#startup-heading");')), 'Tauri bridge and startup view');
    const bootstrap = await request('POST', `/session/${session}/execute/async`, {
      script: 'const done = arguments[arguments.length - 1]; window.__TAURI__.core.invoke("bootstrap").then(value => done({ok:true,value}), error => done({ok:false,error:String(error)}));',
      args: [],
    });
    assert.equal(bootstrap.ok, true, `Native bootstrap failed: ${bootstrap.error}`);
    assert.deepEqual(bootstrap.value, expectedBootstrap);
    evidence.actualBootstrap = bootstrap.value;
    evidence.steps.push('Real native bootstrap returned the expected setup state.');
    const denied = await request('POST', `/session/${session}/execute/async`, {
      script: 'const done = arguments[arguments.length - 1]; window.__TAURI__.core.invoke("openusage_unregistered_native_smoke").then(() => done(false), () => done(true));',
      args: [],
    });
    assert.equal(denied, true, 'An unregistered native command must reject.');
    evidence.steps.push('An unregistered native command rejected.');
    const invalidArguments = await request('POST', `/session/${session}/execute/async`, {
      script: 'const done = arguments[arguments.length - 1]; window.__TAURI__.core.invoke("bootstrap", {unexpected:true}).then(() => done({rejected:false}), error => done({rejected:true,error}));',
      args: [],
    });
    assert.deepEqual(invalidArguments, { rejected: true, error: 'invalid_arguments' });
    evidence.steps.push('Native bootstrap rejected an unexpected argument with invalid_arguments.');
    await waitUntil(async () => (await execute('return document.querySelector("#startup-heading")?.textContent;')) === 'Set up your private workspace', 'setup guidance after native bootstrap');
    const view = await execute('return {heading:document.querySelector("#startup-heading").textContent, monitoring:document.querySelector(".status-pill").textContent.trim(), metrics:Object.fromEntries([...document.querySelectorAll(".metric")].map(card=>[card.querySelector(".metric-label").firstChild.textContent.trim(),card.querySelector(".metric-value").textContent.trim()])), privacyExpanded:document.querySelector("[aria-controls=privacy-details]").getAttribute("aria-expanded")};');
    assert.match(view.monitoring, /Monitoring stopped/);
    assert.equal(view.metrics['Quota remaining'], 'Not available');
    assert.equal(view.metrics['Token activity'], 'Not available');
    assert.equal(view.metrics['Active profiles'], 'Locked');
    assert.equal(view.privacyExpanded, 'false');
    evidence.actualView = view;
    evidence.steps.push('Native startup showed unavailable provider metrics and stopped monitoring.');
    await execute('document.querySelector(".brand").focus(); return true;');
    await request('POST', `/session/${session}/actions`, { actions: [{ type: 'key', id: 'native-keyboard', actions: [
      { type: 'keyDown', value: '\uE004' }, { type: 'keyUp', value: '\uE004' },
      { type: 'keyDown', value: '\uE004' }, { type: 'keyUp', value: '\uE004' },
    ] }] });
    assert.equal(await execute('return document.activeElement?.getAttribute("aria-controls");'), 'privacy-details');
    await request('POST', `/session/${session}/actions`, { actions: [{ type: 'key', id: 'native-keyboard', actions: [
      { type: 'keyDown', value: '\uE007' }, { type: 'keyUp', value: '\uE007' },
    ] }] });
    await waitUntil(async () => (await execute('return document.querySelector("[aria-controls=privacy-details]")?.getAttribute("aria-expanded");')) === 'true', 'keyboard access to privacy details');
    assert.equal(await execute('return document.querySelector("#privacy-heading")?.textContent;'), 'Your data stays yours');
    evidence.steps.push('Tab navigation and Enter opened privacy details in the native webview.');
    await request('POST', `/session/${session}/execute/async`, {
      script: 'const done = arguments[arguments.length - 1]; document.fonts.ready.then(() => requestAnimationFrame(() => requestAnimationFrame(() => done(true))));',
      args: [],
    });
    assert.equal(await execute('return document.querySelector("#startup-heading")?.textContent;'), 'Set up your private workspace');
    const screenshot = await request('GET', `/session/${session}/screenshot`);
    await writeFile(join(output, 'startup.png'), Buffer.from(screenshot, 'base64'), { mode: 0o600 });
    evidence.screenshot = 'startup.png';
    assert.deepEqual(await readdir(isolated.codex), ['synthetic-fixture.txt']);
    assert.equal(await readFile(join(isolated.codex, 'synthetic-fixture.txt'), 'utf8'), sentinel);
    evidence.steps.push('The synthetic Codex home stayed unchanged.');
    evidence.result = 'passed';
  } finally {
    if (session) {
      try { await request('DELETE', `/session/${session}`); } catch (error) {
        evidence.cleanupError = error.message;
        evidence.result = 'failed';
        process.exitCode = 1;
      }
    }
    stopDriver();
    await delay(300);
    stopDriver('SIGKILL');
    await writeFile(join(output, 'driver.log'), Buffer.concat(logs), { mode: 0o600 });
  }
} catch (error) {
  evidence.error = error.message;
  process.exitCode = 1;
} finally {
  if (fixture) await rm(fixture, { recursive: true, force: true });
  evidence.finishedAt = new Date().toISOString();
  await writeFile(join(output, 'result.json'), `${JSON.stringify(evidence, null, 2)}\n`, { mode: 0o600 });
  console.log(`${evidence.requirement}: ${evidence.result}. Evidence: ${output}`);
  if (evidence.error) console.error(evidence.error);
}
