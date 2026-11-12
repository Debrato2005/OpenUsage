import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { chmod, copyFile, cp, lstat, mkdir, readFile, readdir, readlink, rm, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createFixture, stopGroup } from './private-fixture.mjs';
import { NativeDriver, waitFor } from './webdriver.mjs';
import { loginVerifierDigest, runLoginVerifier } from './login-verifier.mjs';

for (const name of ['OPENUSAGE_NATIVE_BINARY', 'OPENUSAGE_NATIVE_EVIDENCE', 'OPENUSAGE_NATIVE_CODEX_SCANNER', 'OPENUSAGE_LOGIN_VERIFIER', 'OPENUSAGE_LOGIN_VERIFIER_SHA256']) assert.ok(process.env[name], name);
const ordinary = process.argv.includes('--ordinary');
assert.ok(process.argv.slice(2).every(arg => arg === '--ordinary'));
const repository = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const application = resolve(process.env.OPENUSAGE_NATIVE_BINARY), output = resolve(process.env.OPENUSAGE_NATIVE_EVIDENCE);
const candidate = resolve(process.env.OPENUSAGE_NATIVE_CODEX_SCANNER), verifier = resolve(process.env.OPENUSAGE_LOGIN_VERIFIER);
const verifierDigest = loginVerifierDigest();
const pin = JSON.parse(await readFile(join(repository, 'tests/tools/fixtures/codex-snapshot-candidate/pin.json'), 'utf8'));
const capture = JSON.parse(await readFile(join(repository, 'tests/tools/fixtures/codex-producer/capture.json'), 'utf8'));
const password = 'OpenUsage synthetic Codex workspace 2026', backupPassword = 'OpenUsage synthetic Codex backup 2026';
const hash = async path => createHash('sha256').update(await readFile(path)).digest('hex');
const exact = new Intl.NumberFormat('en-US');
const lines = rows => rows.map(row => JSON.stringify(row)).join('\n') + '\n';
const localTables = tables => Object.fromEntries(Object.entries(tables).filter(([name]) => name === 'log_sources' || name.startsWith('local_usage_')));
const signatures = rows => rows.map(row => JSON.stringify(Object.entries(row).sort())).sort();
function matches(actual, expected) {
  assert.deepEqual(Object.keys(actual).sort(), Object.keys(expected).sort());
  for (const name of Object.keys(expected)) assert.deepEqual(signatures(actual[name]), signatures(expected[name]), name);
}
function records(thread, total, model = 'gpt-5') {
  const usage = { input_tokens: total, cached_input_tokens: 0, cache_write_input_tokens: 0, output_tokens: 0, reasoning_output_tokens: 0, total_tokens: total };
  return [
    ['session_meta', { id: thread, session_id: thread, cli_version: '0.163.0', history_mode: 'paginated' }],
    ['event_msg', { type: 'task_started', turn_id: 'turn-a', root_turn_id: 'turn-a' }],
    ['turn_context', { turn_id: 'turn-a', root_turn_id: 'turn-a', model }],
    ['token_usage_record', { thread_id: thread, session_id: thread, turn_id: 'turn-a', root_turn_id: 'turn-a', response_id: `${thread}-response`, usage, turn_token_usage: usage, thread_token_usage: usage }],
    ['event_msg', { type: 'token_count', info: { last_token_usage: usage, total_token_usage: usage } }],
  ].map(([type, payload], ordinal) => ({ type, payload, ordinal, timestamp: '2026-11-11T12:00:00.000Z' }));
}
const evidence = {
  requirement: ordinary ? 'NATIVE-CODEX-PRODUCTION-GATE-001' : 'NATIVE-CODEX-QUALIFICATION-001', result: 'failed', startedAt: new Date().toISOString(),
  scope: 'Native Linux, private SQLCipher workspace, synthetic paginated records and a pinned synthetic producer capture. No real logs or provider account.',
  scannerVersion: ordinary ? '20.0.48' : pin.version, scannerSha256: ordinary ? pin.productionSha256 : pin.executableSha256,
  verifierSha256: verifierDigest, steps: [], oracles: [],
};
let fixture, driver, sequence = 0, selected, sessions, canary, lastUsageIpc;
const call = async (command, args) => {
  const response = await driver.invoke(command, args);
  assert.equal(response.ok, true, `${command}: ${JSON.stringify(response)}`);
  if (command === 'load_local_usage') {
    const view = response.value;
    const fields = ['tool', 'totalTokens', 'inputTokens', 'outputTokens', 'cacheWriteTokens', 'cacheReadTokens', 'reasoningTokens', 'estimatedValueUsdMicros', 'estimateComplete', 'logsGone'];
    lastUsageIpc = {
      timeZone: String(view.timeZone).slice(0, 80),
      scannerVersions: view.scannerVersions.slice(0, 4).map(value => String(value).slice(0, 128)),
      tools: view.tools.slice(0, 18).map(row => Object.fromEntries(fields.map(name => [name, typeof row[name] === 'string' ? row[name].slice(0, 128) : row[name] === null || typeof row[name] === 'boolean' ? row[name] : 'invalid']))),
      sessionCount: view.sessions.length, modelCount: view.models.length,
    };
  }
  return response.value;
};
async function key(value) {
  await driver.request('POST', `/session/${driver.session}/actions`, { actions: [{ type: 'key', id: 'codex-keyboard', actions: [{ type: 'keyDown', value }, { type: 'keyUp', value }] }] });
  await driver.request('DELETE', `/session/${driver.session}/actions`);
}
async function open(setup) {
  driver = new NativeDriver(application, fixture.environment, join(output, `driver-${++sequence}.log`));
  await driver.start();
  await waitFor(() => driver.execute('const a=document.querySelector("#workspace-passphrase"),b=document.querySelector("#workspace-confirmation");return Boolean(a&&!a.disabled&&(!arguments[0]||b&&!b.disabled));', [setup]), 'password fields');
  if (setup) {
    assert.deepEqual((await call('load_local_logs')).tools, [{ tool: 'codex', detected: true, granted: false }]);
    await waitFor(() => driver.execute('return document.querySelectorAll(".local-log-choice .local-log-tool").length===1;'), 'Codex consent choice');
    assert.deepEqual(await driver.execute('return [...document.querySelectorAll(".local-log-choice .local-log-tool")].map(n=>n.textContent.trim());'), ['Codex']);
  } else assert.equal(await driver.execute('return document.querySelectorAll(".local-log-choice").length;'), 0);
  await driver.type('#workspace-passphrase', password);
  if (setup) await driver.type('#workspace-confirmation', password);
  await key('\uE007');
  await waitFor(async () => (await call('bootstrap')).storage === 'ready', 'ready workspace');
  assert.equal((await call('bootstrap')).schemaVersion, 13);
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
    if (view.scan.state === 'failed') throw new Error(`Codex scan failed: ${JSON.stringify(view.scan)}`);
    return view.scan.state === 'idle' && view.scan.lastScanAtUtc && view.scan.lastScanAtUtc > after;
  }, 'finished Codex scan', 90000);
  assert.deepEqual(view.tools, [{ tool: 'codex', detected: true, granted: true }]);
  return view.scan.lastScanAtUtc;
}
async function displayed(total, input, estimate, sessionCount) {
  await driver.navigate('overview');
  const view = await call('load_local_usage', { days: 30, period: 'day' });
  const label = `Codex: ${exact.format(BigInt(total))} tokens`;
  await waitFor(() => driver.execute('return [...document.querySelectorAll("[data-local-overview] [data-local-tool-total]")].some(n=>n.getAttribute("aria-label")===arguments[0]);', [label]), label, 30000);
  const tool = view.tools.find(row => row.tool === 'codex');
  assert.equal(tool.totalTokens, String(total));
  assert.equal(tool.inputTokens, String(input));
  assert.equal(tool.estimatedValueUsdMicros, estimate === null ? null : String(estimate));
  assert.deepEqual(view.scannerVersions, [evidence.scannerVersion]);
  assert.equal(view.sessions.length, sessionCount);
  assert.equal(view.sessions.reduce((sum, row) => sum + BigInt(row.totalTokens), 0n), BigInt(total));
  assert.ok(view.sessions.every(row => row.project === null && row.startUtc === null && row.endUtc !== null));
  if (evidence.pinnedZone) assert.equal(view.timeZone, evidence.pinnedZone);
  const card = '[data-local-overview] [data-local-tool="codex"]';
  if (estimate === null) assert.equal(await driver.execute('return document.querySelector(arguments[0]+" .local-tool-context").textContent;', [card]), 'Estimated value Not available');
  await driver.click(`${card} [data-local-tool-total]`);
  await waitFor(() => driver.execute('return Boolean(document.querySelector("[data-detail-panel][open] .detail-rows"));'), 'Codex details');
  const details = await driver.execute('return Object.fromEntries([...document.querySelectorAll("[data-detail-panel][open] dt")].map(n=>[n.textContent,n.nextElementSibling.textContent]));');
  assert.equal(details.Input, exact.format(BigInt(input)));
  assert.equal(details.Total, exact.format(BigInt(total)));
  if (estimate === null) assert.equal(details['Estimated value'], 'Not available');
  else assert.ok(details['Estimated value'].startsWith(estimate === 0 ? '$0.00' : '$0.03'));
  assert.ok(details.Source.includes(pin.version));
  assert.ok(details.Dates.endsWith(view.timeZone));
  if (estimate === null && !evidence.unavailableScreenshot) {
    evidence.unavailableScreenshot = 'codex-unavailable-details.png';
    await driver.screenshot(join(output, evidence.unavailableScreenshot));
  }
  await driver.closeDetail();
  return view;
}
async function sourceHashes() {
  const result = [];
  async function visit(path) {
    for (const entry of await readdir(path, { withFileTypes: true })) {
      const file = join(path, entry.name);
      if (entry.isDirectory()) await visit(file);
      else { assert.ok(entry.isFile()); result.push([file, await hash(file)]); }
    }
  }
  await visit(selected);
  result.push([canary, await hash(canary)]);
  return result.sort(([a], [b]) => a.localeCompare(b));
}
async function oracle(label, mode = 'database', path = fixture.database) {
  const temporary = join(fixture.root, `oracle-${randomUUID()}`);
  await mkdir(temporary, { mode: 0o700 });
  try {
    let input = path;
    if (mode === 'database') {
      input = join(temporary, 'database.db');
      await copyFile(path, input); await chmod(input, 0o600);
      try { await copyFile(`${path}-wal`, `${input}-wal`); await chmod(`${input}-wal`, 0o600); }
      catch (error) { if (error.code !== 'ENOENT') throw error; }
    }
    const raw = await runLoginVerifier(verifier, verifierDigest, [mode, input, join(dirname(fixture.database), 'database-key.v1'), '--profile-state'], {
      input: JSON.stringify({ passphrase: password, backupPassword }), encoding: 'utf8', env: fixture.environment, timeout: 30000, maxBuffer: 8 * 1024 * 1024,
    });
    const report = JSON.parse(execFileSync('/usr/bin/python3', ['-c', 'import json,sys;print(json.dumps(json.load(sys.stdin,parse_int=str)))'], {
      input: raw, encoding: 'utf8', timeout: 10000, maxBuffer: 8 * 1024 * 1024, env: fixture.environment,
    }));
    for (const flag of ['verified', 'readOnly', 'inputUnchanged', 'keyEnvelopeUnchanged', 'walRecordsUnchanged']) assert.equal(report[flag], true);
    assert.equal(report.schemaVersion, '13');
    evidence.oracles.push({ label, mode, tables: Object.fromEntries(Object.entries(report.state.tables).map(([name, rows]) => [name, { count: rows.length, sha256: createHash('sha256').update(JSON.stringify(rows)).digest('hex') }])) });
    return report.state.tables;
  } finally { await rm(temporary, { recursive: true, force: true }); }
}
function checkGrouped(tables, total, input, estimate, count) {
  const saved = tables.local_usage_records.filter(row => row.tool === 'codex');
  const days = saved.filter(row => row.kind === 'day'), sessions = saved.filter(row => row.kind === 'session');
  assert.ok(days.length > 0 && days.every(row => row.input_tokens !== null));
  assert.equal(days.reduce((sum, row) => sum + BigInt(row.total_tokens), 0n), BigInt(total));
  assert.equal(sessions.reduce((sum, row) => sum + BigInt(row.total_tokens), 0n), BigInt(total));
  assert.equal(sessions.length, count);
  assert.equal(days.reduce((sum, row) => sum + BigInt(row.input_tokens), 0n), BigInt(input));
  if (estimate === null) assert.ok(days.some(row => row.estimated_value_micros === null));
  else {
    assert.ok(days.every(row => row.estimated_value_micros !== null));
    assert.equal(days.reduce((sum, row) => sum + BigInt(row.estimated_value_micros), 0n), BigInt(estimate));
  }
  assert.deepEqual(tables.local_usage_scopes.map(row => [row.tool, row.time_zone, row.period_kind]), [['codex', evidence.pinnedZone, 'day']]);
  const scans = new Map(tables.local_usage_scans.map(row => [row.id, row]));
  assert.ok(saved.length > 0);
  for (const row of saved) assert.equal(scans.get(row.scan_id)?.scanner_version, pin.version);
  const latest = tables.local_usage_scans.filter(row => row.tool === 'codex' && row.latest === '1');
  assert.equal(latest.length, 1);
  assert.ok(saved.every(row => row.scan_id === latest[0].id));
  assert.equal(tables.local_usage_days.length, 0);
  assert.equal(tables.local_usage_sessions.length, 0);
}
async function wrongScanner() {
  const marker = join(fixture.root, 'wrong-scanner-ran'), fake = join(fixture.root, 'wrong-scanner');
  const quoted = "'" + marker.replaceAll("'", "'\\''") + "'";
  await writeFile(fake, `#!/bin/sh\nprintf ran > ${quoted}\nexit 0\n`, { mode: 0o700 });
  return { marker, fake };
}
async function backupAndExport(expected) {
  const receipt = await call('create_backup', { password: backupPassword });
  const archive = join(fixture.dirs.downloads, 'OpenUsage', receipt.fileName);
  assert.equal((await lstat(archive)).mode & 0o777, 0o600);
  matches(localTables(await oracle('authenticated grouped backup', 'backup', archive)), localTables(expected));
  assert.deepEqual(await driver.invoke('export_application_data', { confirmed: false }), { ok: false, error: 'invalid_arguments' });
  const exported = await call('export_application_data', { confirmed: true });
  const payload = JSON.parse(await readFile(join(fixture.dirs.downloads, 'OpenUsage', exported.fileName), 'utf8'));
  assert.equal(payload.integerEncoding, 'decimal-string');
  const tables = Object.fromEntries(payload.tables.map(table => [table.name, table.rows.map(row => Object.fromEntries(table.columns.map((column, index) => [column.name, row[index]])))]));
  matches(tables, expected);
  await call('set_theme', { theme: 'dark' });
  const preview = await call('prepare_restore_preview', { fileName: receipt.fileName, password: backupPassword });
  assert.equal(preview.restoreAvailable, true);
  matches(localTables(await oracle('grouped restore checkpoint', 'checkpoint', join(dirname(fixture.database), 'restore-checkpoints', `${preview.checkpoint.id}.sqlite`))), localTables(expected));
  assert.deepEqual(await driver.invoke('apply_restore', { previewId: preview.previewId, confirmed: false }), { ok: false, error: 'invalid_arguments' });
  await call('apply_restore', { previewId: preview.previewId, confirmed: true });
  matches(localTables(await oracle('grouped restore applied')), localTables(expected));
  evidence.steps.push('Authenticated backup, full JSON export, restore checkpoint, and applied restore retain every local field.');
}

try {
  await mkdir(output, { recursive: true, mode: 0o700 });
  assert.equal(await hash(candidate), pin.executableSha256);
  evidence.applicationSha256 = await hash(application);
  fixture = await createFixture('codex-qualification');
  const home = join(fixture.root, 'home'); await mkdir(home, { mode: 0o700 });
  const logs = join(fixture.root, 'logs home café');
  selected = join(logs, '.codex'); sessions = join(selected, 'sessions');
  await mkdir(sessions, { recursive: true, mode: 0o700 });
  canary = join(home, 'not-selected.jsonl');
  await writeFile(canary, lines(records('canary', 999999)), { mode: 0o600 });
  Object.assign(fixture.environment, { HOME: home, OPENUSAGE_NATIVE_LOG_HOME: logs, OPENUSAGE_NATIVE_CODEX_SCANNER: candidate });
  if (ordinary) {
    const production = join(repository, 'apps/desktop/src-tauri/binaries/openusage-ccusage-x86_64-unknown-linux-gnu');
    assert.equal(await hash(production), pin.productionSha256);
    fixture.environment.OPENUSAGE_NATIVE_CCUSAGE = production;
    await cp(join(repository, 'crates/openusage-locallogs/tests/fixtures/codex'), selected, { recursive: true });
    const { marker, fake } = await wrongScanner(); fixture.environment.OPENUSAGE_NATIVE_CODEX_SCANNER = fake;
    const before = await sourceHashes();
    await open(true); await scan();
    const view = await call('load_local_usage', { days: 365, period: 'day' });
    assert.deepEqual(view.scannerVersions, ['20.0.48']);
    assert.equal(view.tools.find(row => row.tool === 'codex').totalTokens, '4600');
    await assert.rejects(readFile(marker), { code: 'ENOENT' });
    await close();
    const tables = await oracle('ordinary production build');
    assert.equal(tables.local_usage_records.length, 0);
    assert.ok(tables.local_usage_days.length > 0);
    assert.deepEqual(await sourceHashes(), before);
    evidence.steps.push('The ordinary build offers Codex and uses the production scanner. It ignores the candidate override.');
  } else {
    assert.ok(process.env.OPENUSAGE_CODEX_CAPTURE_RECEIPT, 'OPENUSAGE_CODEX_CAPTURE_RECEIPT');
    const receiptPath = resolve(process.env.OPENUSAGE_CODEX_CAPTURE_RECEIPT);
    assert.equal(await hash(receiptPath), capture.receiptSha256);
    const receipt = JSON.parse(await readFile(receiptPath, 'utf8'));
    const zero = join(sessions, 'zero.jsonl');
    await writeFile(zero, lines(records('native-zero', 0)), { mode: 0o600 });
    const initialSources = await sourceHashes();
    await open(true);
    let last = await scan();
    evidence.pinnedZone = (await displayed(0, 0, 0, 1)).timeZone;
    assert.deepEqual(await sourceHashes(), initialSources);
    checkGrouped(await oracle('measured zero'), 0, 0, 0, 1);
    for (const role of ['parent', 'second', 'fork']) {
      const meta = capture.files[role], data = Buffer.from(receipt.rollouts[meta.originalPath]);
      assert.equal(createHash('sha256').update(data).digest('hex'), meta.sha256);
      assert.equal(data.length, meta.bytes);
      const relative = meta.originalPath.replace(/^codex\//, '');
      assert.equal(relative.split('/').includes('..'), false);
      const path = join(selected, relative);
      await mkdir(dirname(path), { recursive: true, mode: 0o700 });
      await writeFile(path, data, { mode: 0o600 });
      if (role === 'parent') await copyFile(path, join(dirname(path), 'renamed-parent-copy.jsonl'));
    }
    const captureSources = await sourceHashes();
    await call('save_local_log_consent', { tools: ['codex'] }); last = await scan(last);
    const lineage = await displayed(12650, 8250, 27156, 4);
    assert.equal(lineage.tools[0].outputTokens, '1650'); assert.equal(lineage.tools[0].cacheReadTokens, '2750'); assert.equal(lineage.tools[0].reasoningTokens, '550');
    assert.deepEqual(await sourceHashes(), captureSources);
    checkGrouped(await oracle('full fork and copied parent'), 12650, 8250, 27156, 4);
    await driver.screenshot(join(output, 'codex-full-fork.png'));
    const zone = evidence.pinnedZone === 'America/New_York' ? 'Asia/Kolkata' : 'America/New_York';
    await call('set_time_zone', { timeZone: zone });
    await writeFile(join(sessions, 'unattributed.jsonl'), lines(records('native-unattributed', 17, null)), { mode: 0o600 });
    const grownSources = await sourceHashes();
    await call('save_local_log_consent', { tools: ['codex'] }); await scan(last);
    const partial = await displayed(12667, 8267, null, 5);
    assert.ok(partial.models.length > 0);
    assert.ok(partial.models.every(row => row.totalTokens === null && row.estimatedValueUsdMicros === null));
    assert.equal(partial.sessions.find(row => row.key === 'native-unattributed').estimatedValueUsdMicros, null);
    assert.equal(partial.sessions.find(row => row.key === 'native-zero').estimatedValueUsdMicros, '0');
    const bad = join(sessions, 'broken.jsonl'); await writeFile(bad, '{"type":', { mode: 0o600 });
    const badHash = await hash(bad);
    await call('save_local_log_consent', { tools: ['codex'] });
    await waitFor(async () => (await call('load_local_logs')).scan.state === 'failed', 'refused Codex input', 30000);
    await displayed(12667, 8267, null, 5);
    assert.equal(await hash(bad), badHash); await rm(bad);
    assert.deepEqual(await sourceHashes(), grownSources);
    await close();
    const saved = await oracle('after refused input'); checkGrouped(saved, 12667, 8267, null, 5);
    await open(false); await scan(); await displayed(12667, 8267, null, 5);
    await call('save_local_log_consent', { tools: [] });
    assert.equal((await call('load_local_logs')).tools[0].granted, false);
    const beforeWithdrawalReopen = await oracle('withdrawn before close');
    await close();
    await writeFile(join(sessions, 'after-withdrawal.jsonl'), lines(records('native-withdrawn', 23)), { mode: 0o600 });
    const withdrawnSources = await sourceHashes();
    await open(false); await displayed(12667, 8267, null, 5);
    assert.equal((await call('load_local_logs')).tools[0].granted, false);
    const withdrawn = await oracle('after withdrawal and reopen');
    matches(localTables(withdrawn), localTables(beforeWithdrawalReopen));
    assert.equal(withdrawn.log_sources[0].consent, 'withdrawn');
    checkGrouped(withdrawn, 12667, 8267, null, 5);
    await backupAndExport(withdrawn);
    await close(); await open(false); await displayed(12667, 8267, null, 5); await close();
    matches(localTables(await oracle('restored genuine reopen')), localTables(withdrawn));
    const { marker, fake } = await wrongScanner(); fixture.environment.OPENUSAGE_NATIVE_CODEX_SCANNER = fake;
    await open(false); await call('save_local_log_consent', { tools: ['codex'] });
    await waitFor(async () => (await call('load_local_logs')).scan.state === 'failed', 'wrong scanner digest refusal', 30000);
    await displayed(12667, 8267, null, 5); await assert.rejects(readFile(marker), { code: 'ENOENT' });
    await close();
    const refused = await oracle('wrong digest');
    for (const name of Object.keys(localTables(withdrawn)).filter(name => name !== 'log_sources')) matches({ [name]: refused[name] }, { [name]: withdrawn[name] });
    assert.deepEqual(await sourceHashes(), withdrawnSources);
    evidence.steps.push('Zero remains measured. Copies count once. The full fork saves 12650 tokens. Missing attribution keeps 12667 tokens without an estimate.');
    evidence.steps.push('Failed input, withdrawal, restore, genuine reopen, and a wrong scanner digest preserve saved records.');
  }
  assert.equal(await hash(application), evidence.applicationSha256);
  assert.equal(await hash(candidate), pin.executableSha256);
  evidence.result = 'passed';
} catch (error) {
  evidence.error = error.stack ?? String(error); process.exitCode = 1;
  evidence.failureDiagnostics = { lastUsageIpc: lastUsageIpc ?? null, localOverviewLabels: null };
  if (driver?.session) {
    try {
      const response = await fetch(`${driver.endpoint}/session/${driver.session}/execute/sync`, {
        method: 'POST', headers: { 'content-type': 'application/json' }, signal: AbortSignal.timeout(2000),
        body: JSON.stringify({ script: 'return [...document.querySelectorAll("[data-local-overview] [data-local-tool-total]")].slice(0,18).map(n=>(n.getAttribute("aria-label")??"").slice(0,160));', args: [] }),
      });
      const payload = await response.json();
      if (response.ok && Array.isArray(payload.value)) evidence.failureDiagnostics.localOverviewLabels = payload.value;
    } catch { evidence.failureDiagnostics.overviewUnavailable = true; }
  }
}
finally {
  try { await driver?.stop(); }
  finally {
    stopGroup(fixture?.bus, 'SIGKILL');
    if (fixture) await rm(fixture.root, { recursive: true, force: true });
    evidence.finishedAt = new Date().toISOString();
    await writeFile(join(output, 'result.json'), JSON.stringify(evidence, null, 2) + '\n', { mode: 0o600 });
    console.log(`${evidence.requirement}: ${evidence.result}. Evidence: ${output}`);
    if (evidence.error) console.error(evidence.error);
  }
}
