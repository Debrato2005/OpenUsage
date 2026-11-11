import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { chmod, copyFile, mkdir, readFile, readlink, rm, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createFixture, stopGroup } from './private-fixture.mjs';
import { NativeDriver, waitFor } from './webdriver.mjs';

for (const name of ['OPENUSAGE_NATIVE_BINARY', 'OPENUSAGE_NATIVE_EVIDENCE', 'OPENUSAGE_NATIVE_PI_SCANNER', 'OPENUSAGE_LOGIN_VERIFIER', 'OPENUSAGE_LOGIN_VERIFIER_SHA256']) assert.ok(process.env[name], name);
const disabled = process.argv.includes('--expect-disabled');
assert.ok(process.argv.slice(2).every(arg => arg === '--expect-disabled'));
const repository = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const application = resolve(process.env.OPENUSAGE_NATIVE_BINARY);
const output = resolve(process.env.OPENUSAGE_NATIVE_EVIDENCE);
const candidate = resolve(process.env.OPENUSAGE_NATIVE_PI_SCANNER);
const verifier = resolve(process.env.OPENUSAGE_LOGIN_VERIFIER);
const verifierDigest = process.env.OPENUSAGE_LOGIN_VERIFIER_SHA256;
assert.match(verifierDigest, /^[0-9a-f]{64}$/);
const pin = JSON.parse(await readFile(join(repository, 'tests/tools/fixtures/pi-producer/pin.json'), 'utf8'));
const password = 'OpenUsage synthetic Pi workspace 2026';
const hash = async path => createHash('sha256').update(await readFile(path)).digest('hex');
const time = new Date().toISOString();
const day = time.slice(0, 10);
const header = { type: 'session', version: 3, id: randomUUID(), timestamp: `${day}T00:00:00.000Z`, cwd: '/synthetic/native-pi' };
const entry = (id, parentId, total) => ({ type: 'usage', id, parentId, timestamp: time, kind: 'cache_warm', usage: { totalTokens: total } });
const first = entry('aaaaaaaa', null, 120);
const lines = rows => rows.map(row => JSON.stringify(row)).join('\n') + '\n';
const evidence = {
  requirement: disabled ? 'NATIVE-PI-GATE-001' : 'NATIVE-PI-QUALIFICATION-001',
  result: 'failed', startedAt: new Date().toISOString(),
  scope: 'Native Linux, private SQLCipher workspace and synthetic Pi records. No real logs or provider account.',
  candidateVersion: pin.boundedReleaseScannerVersion, candidateSha256: pin.boundedReleaseScannerSha256,
  verifierSha256: verifierDigest, steps: [], oracles: [],
};
let fixture, driver, sequence = 0, original, copied, canary;
const call = async (command, args) => {
  const response = await driver.invoke(command, args);
  assert.equal(response.ok, true, `${command}: ${JSON.stringify(response)}`);
  return response.value;
};
async function key(value) {
  await driver.request('POST', `/session/${driver.session}/actions`, { actions: [{ type: 'key', id: 'pi-keyboard', actions: [{ type: 'keyDown', value }, { type: 'keyUp', value }] }] });
  await driver.request('DELETE', `/session/${driver.session}/actions`);
}
const choices = () => driver.execute('return [...document.querySelectorAll(".local-log-choice .local-log-tool")].map(n=>n.textContent.trim());');
async function open(setup) {
  driver = new NativeDriver(application, fixture.environment, join(output, `driver-${++sequence}.log`));
  await driver.start();
  await waitFor(() => driver.execute('const a=document.querySelector("#workspace-passphrase"),b=document.querySelector("#workspace-confirmation");return Boolean(a&&!a.disabled&&(!arguments[0]||b&&!b.disabled));', [setup]), 'password fields');
  if (setup) {
    assert.deepEqual((await call('load_local_logs')).tools, disabled ? [] : [{ tool: 'pi', detected: true, granted: false }]);
    if (!disabled) await waitFor(async () => (await choices()).length === 1, 'Pi consent choice');
    assert.deepEqual(await choices(), disabled ? [] : ['Pi']);
  }
  else assert.equal(await driver.execute('return document.querySelectorAll(".local-log-choice").length;'), 0);
  await driver.type('#workspace-passphrase', password);
  if (setup) await driver.type('#workspace-confirmation', password);
  await key('');
  await waitFor(async () => (await call('bootstrap')).storage === 'ready', 'ready workspace');
}
async function close() {
  const pending = [driver.child.pid];
  let owned;
  for (let count = 0; pending.length && count < 64; count++) {
    const pid = pending.shift();
    try {
      if (await readlink(`/proc/${pid}/exe`) === application) { owned = pid; break; }
      pending.push(...(await readFile(`/proc/${pid}/task/${pid}/children`, 'utf8')).trim().split(/\s+/).filter(Boolean).map(Number));
    } catch (error) { if (error.code !== 'ENOENT') throw error; }
  }
  assert.ok(owned, 'The app must belong to this journey.');
  execFileSync('/usr/bin/python3', [join(repository, 'tests/native/x11-close.py'), String(owned)], { env: fixture.environment, timeout: 10000 });
  await waitFor(async () => {
    try { await readlink(`/proc/${owned}/exe`); return false; }
    catch (error) { if (error.code === 'ENOENT') return true; throw error; }
  }, 'native close');
  driver.session = undefined;
  await driver.stop();
  driver = undefined;
}
async function scan(after = '') {
  let view;
  await waitFor(async () => {
    view = await call('load_local_logs');
    return view.scan.state === 'idle' && view.scan.lastScanAtUtc && view.scan.lastScanAtUtc > after;
  }, 'finished Pi scan', 90000);
  assert.deepEqual(view.tools, [{ tool: 'pi', detected: true, granted: true }]);
  return view.scan.lastScanAtUtc;
}
async function displayed(total) {
  const label = `Pi: ${total} tokens`;
  await waitFor(() => driver.execute('return [...document.querySelectorAll("[data-local-overview] [data-local-tool-total]")].some(n=>n.getAttribute("aria-label")===arguments[0]);', [label]), label, 30000);
  const view = await call('load_local_usage', { days: 30, period: 'day' });
  assert.equal(view.tools.find(tool => tool.tool === 'pi').totalTokens, String(total));
  assert.deepEqual(view.scannerVersions, [pin.boundedReleaseScannerVersion]);
  assert.equal(view.tools.find(tool => tool.tool === 'pi').inputTokens, null);
  assert.equal(view.tools.find(tool => tool.tool === 'pi').estimatedValueUsdMicros, null);
  if (evidence.pinnedZone) assert.equal(view.timeZone, evidence.pinnedZone);
  return view;
}
async function sources() {
  return Promise.all([original, copied, canary].map(async path => [path, await hash(path)]));
}
async function oracle(label) {
  assert.equal(await hash(verifier), verifierDigest);
  const temporary = join(fixture.root, `oracle-${randomUUID()}`);
  await mkdir(temporary, { mode: 0o700 });
  try {
    const database = join(temporary, 'database.db');
    await copyFile(fixture.database, database);
    await chmod(database, 0o600);
    try { await copyFile(`${fixture.database}-wal`, `${database}-wal`); }
    catch (error) { if (error.code !== 'ENOENT') throw error; }
    const raw = execFileSync(verifier, ['database', database, join(dirname(fixture.database), 'database-key.v1'), '--profile-state'], {
      input: JSON.stringify({ passphrase: password }), encoding: 'utf8', env: fixture.environment, timeout: 30000, maxBuffer: 8 * 1024 * 1024,
    });
    const result = JSON.parse(raw);
    assert.equal(result.verified, true);
    assert.equal(Number(result.schemaVersion), 13);
    const tables = result.state.tables;
    const saved = {
      sources: tables.log_sources.map(row => [row.tool, row.consent, row.time_zone]),
      scopes: tables.local_usage_scopes.filter(row => row.tool === 'pi').map(row => [row.tool, row.time_zone, row.period_kind]),
      scans: tables.local_usage_scans.filter(row => row.tool === 'pi').map(row => [row.id, row.scanner_version, row.latest]),
      records: tables.local_usage_records.filter(row => row.tool === 'pi').map(row => [row.kind, row.period_key, row.total_tokens, row.input_tokens, row.estimated_value_micros]).sort(),
      recordScans: tables.local_usage_records.filter(row => row.tool === 'pi').map(row => [row.kind, row.period_key, row.scan_id]).sort(),
    };
    evidence.oracles.push({ label, ...saved });
    if (!disabled) {
      assert.equal(saved.scans.length, 1);
      const [[scanId, scannerVersion, latest]] = saved.scans;
      assert.equal(scannerVersion, pin.boundedReleaseScannerVersion);
      assert.equal(latest, 1);
      assert.deepEqual(saved.recordScans, saved.records.map(([kind, period]) => [kind, period, scanId]));
    }
    return saved;
  } finally { await rm(temporary, { recursive: true, force: true }); }
}

try {
  await mkdir(output, { recursive: true, mode: 0o700 });
  assert.equal(await hash(candidate), pin.boundedReleaseScannerSha256);
  evidence.applicationSha256 = await hash(application);
  fixture = await createFixture('pi-qualification');
  const logs = join(fixture.root, 'logs home café');
  const sessions = join(logs, '.pi/agent/sessions/--synthetic-native-pi--');
  await mkdir(sessions, { recursive: true, mode: 0o700 });
  original = join(sessions, 'original.jsonl'); copied = join(sessions, 'copied.jsonl');
  canary = join(fixture.root, 'not-selected.jsonl');
  await writeFile(original, lines([header, first]), { mode: 0o600 });
  await copyFile(original, copied);
  await writeFile(canary, lines([{ ...header, id: randomUUID() }, entry('cccccccc', null, 999999)]), { mode: 0o600 });
  Object.assign(fixture.environment, { OPENUSAGE_NATIVE_LOG_HOME: logs, OPENUSAGE_NATIVE_PI_SCANNER: candidate });
  const initialSources = await sources();
  await open(true);
  if (disabled) {
    assert.deepEqual((await call('load_local_logs')).tools, []);
    assert.equal((await driver.invoke('save_local_log_consent', { tools: ['pi'] })).ok, false);
    await close();
    assert.deepEqual((await oracle('ordinary build')).records, []);
    assert.deepEqual(await sources(), initialSources);
    evidence.steps.push('The ordinary build did not offer or grant Pi.');
  } else {
    const firstScan = await scan();
    evidence.pinnedZone = (await displayed(120)).timeZone;
    const changedZone = evidence.pinnedZone === 'America/New_York' ? 'Asia/Kolkata' : 'America/New_York';
    assert.deepEqual(await call('set_time_zone', { timeZone: changedZone }), { timeZone: changedZone });
    assert.deepEqual(await sources(), initialSources);
    await driver.screenshot(join(output, 'pi-first-history.png'));
    await writeFile(original, lines([header, first, entry('bbbbbbbb', 'aaaaaaaa', 17)]), { mode: 0o600 });
    const grownSources = await sources();
    await call('save_local_log_consent', { tools: ['pi'] });
    await scan(firstScan);
    await displayed(137);
    assert.deepEqual(await sources(), grownSources);
    const bad = join(sessions, 'broken.jsonl');
    await writeFile(bad, '{"type":', { mode: 0o600 });
    const badBefore = await hash(bad);
    await call('save_local_log_consent', { tools: ['pi'] });
    await waitFor(async () => (await call('load_local_logs')).scan.state === 'failed', 'failed Pi scan', 30000);
    await displayed(137);
    assert.equal(await hash(bad), badBefore);
    assert.deepEqual(await sources(), grownSources);
    await close();
    const saved = await oracle('after refused input');
    assert.deepEqual(saved.scopes, [['pi', evidence.pinnedZone, 'day']]);
    assert.deepEqual(saved.records.map(([kind, , ...metrics]) => [kind, ...metrics]), [
      ['day', '137', null, null],
      ['session', '137', null, null],
    ]);
    await rm(bad);
    await open(false);
    await scan();
    await displayed(137);
    await call('save_local_log_consent', { tools: [] });
    assert.equal((await call('load_local_logs')).tools[0].granted, false);
    await close();
    await writeFile(original, lines([header, first, entry('bbbbbbbb', 'aaaaaaaa', 17), entry('dddddddd', 'bbbbbbbb', 23)]), { mode: 0o600 });
    const withdrawnSources = await sources();
    await open(false);
    await displayed(137);
    assert.equal((await call('load_local_logs')).tools[0].granted, false);
    await close();
    const withdrawn = await oracle('after withdrawal and restart');
    assert.deepEqual(withdrawn.records, saved.records);
    assert.deepEqual(withdrawn.scopes, saved.scopes);
    assert.equal(withdrawn.sources[0][1], 'withdrawn');
    assert.deepEqual(await sources(), withdrawnSources);
    const marker = join(fixture.root, 'wrong-scanner-ran');
    const fake = join(fixture.root, 'wrong-scanner');
    const quotedMarker = "'" + marker.replaceAll("'", "'\\''") + "'";
    await writeFile(fake, `#!/bin/sh\nprintf ran > ${quotedMarker}\nexit 0\n`, { mode: 0o700 });
    fixture.environment.OPENUSAGE_NATIVE_PI_SCANNER = fake;
    await open(false);
    await call('save_local_log_consent', { tools: ['pi'] });
    await waitFor(async () => (await call('load_local_logs')).scan.state === 'failed', 'wrong scanner digest refusal', 30000);
    await displayed(137);
    await assert.rejects(readFile(marker), { code: 'ENOENT' });
    await close();
    assert.deepEqual((await oracle('after wrong scanner digest')).records, saved.records);
    assert.deepEqual(await sources(), withdrawnSources);
    evidence.steps.push('Copies counted once. Growth saved 137 tokens. Failed input and withdrawal retained saved usage.');
  }
  assert.equal(await hash(application), evidence.applicationSha256);
  assert.equal(await hash(candidate), pin.boundedReleaseScannerSha256);
  evidence.result = 'passed';
} catch (error) { evidence.error = error.stack ?? String(error); process.exitCode = 1; }
finally {
  try { await driver?.stop(); }
  finally {
    stopGroup(fixture?.bus, 'SIGKILL');
    if (fixture) await rm(fixture.root, { recursive: true, force: true });
    evidence.finishedAt = new Date().toISOString();
    await writeFile(join(output, 'result.json'), JSON.stringify(evidence, null, 2) + '\n', { mode: 0o600 });
    console.log(`${evidence.requirement}: ${evidence.result}. Evidence: ${output}`);
  }
}
