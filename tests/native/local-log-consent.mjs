import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { chmod, copyFile, cp, mkdir, readFile, readdir, readlink, rm, writeFile } from 'node:fs/promises';
import { readdirSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createFixture, stopGroup } from './private-fixture.mjs';
import { NativeDriver, waitFor } from './webdriver.mjs';

// First launch with synthetic Claude Code and Codex logs in a private home: one consent list on the
// welcome screen, then a real scan by the pinned ccusage binary of the chosen tool only (OU-LOG-001).
for (const name of ['OPENUSAGE_NATIVE_BINARY', 'OPENUSAGE_NATIVE_EVIDENCE', 'OPENUSAGE_LOGIN_VERIFIER']) assert.ok(process.env[name], name);
const repository = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const application = resolve(process.env.OPENUSAGE_NATIVE_BINARY), output = resolve(process.env.OPENUSAGE_NATIVE_EVIDENCE), verifier = resolve(process.env.OPENUSAGE_LOGIN_VERIFIER);
const ccusage = join(repository, 'apps/desktop/src-tauri/binaries/openusage-ccusage-x86_64-unknown-linux-gnu');
const ccusageDigest = '55a3fd41b29727672bef1242311758d1605a07765781d7a80a41d1b485d29ed1';
const verifierDigest = 'a2e4a64707081a9943f3c04f8237c2b16bea673e974e96393d6a8a7d9357f975';
const password = 'OpenUsage synthetic local log workspace 2026';
const hash = async path => createHash('sha256').update(await readFile(path)).digest('hex');
const evidence = {
  requirement: 'NATIVE-LOCAL-LOG-CONSENT-001', result: 'failed', startedAt: new Date().toISOString(),
  scope: 'Actual Linux Tauri native-test build, a private SQLCipher 4 password workspace, synthetic Claude Code and Codex logs in a private home, and the real pinned ccusage 20.0.48. No real logs and no provider account.',
  expectedResult: ['The welcome screen lists both detected tools, selected.', 'Only the chosen tool is granted and scanned.', 'With no account, Overview shows the saved Claude Code total (AT-91).', 'The scan saves each Claude Code session with its project and times (SC-021).', 'A restart asks no second consent and a second scan gives the same totals.', 'The synthetic logs stay unchanged.'],
  steps: [], oracles: [],
};
let fixture, driver, sequence = 0;
const call = async (command, args) => { const response = await driver.invoke(command, args); assert.equal(response.ok, true, `${command}: ${JSON.stringify(response)}`); return response.value; };
async function key(value) {
  await driver.request('POST', `/session/${driver.session}/actions`, { actions: [{ type: 'key', id: 'local-log-keyboard', actions: [{ type: 'keyDown', value }, { type: 'keyUp', value }] }] });
  await driver.request('DELETE', `/session/${driver.session}/actions`);
}
async function activate(selector, value = '') {
  await waitFor(() => driver.execute('const n=document.querySelector(arguments[0]);return Boolean(n&&!n.disabled&&n.getClientRects().length);', [selector]), `available ${selector}`);
  assert.equal(await driver.execute('const n=document.querySelector(arguments[0]);n.scrollIntoView({block:"nearest",inline:"nearest"});n.focus({preventScroll:true});return document.activeElement===n;', [selector]), true);
  await key(value);
}
const choices = () => driver.execute('return [...document.querySelectorAll(".local-log-choice .local-log-tool")].map(l=>({name:l.textContent.trim(),checked:l.querySelector("[role=checkbox]")?.getAttribute("aria-checked")}));');
async function open(setup) {
  driver = new NativeDriver(application, fixture.environment, join(output, `driver-${++sequence}.log`)); await driver.start();
  await waitFor(() => driver.execute('const a=document.querySelector("#workspace-passphrase"),b=document.querySelector("#workspace-confirmation");return Boolean(a&&!a.disabled&&(!arguments[0]||b&&!b.disabled));', [setup]), 'native password fields');
  if (setup) {
    await waitFor(async () => (await choices()).length === 2, 'the consent list');
    assert.deepEqual(await choices(), [{ name: 'Claude Code', checked: 'true' }, { name: 'Codex', checked: 'true' }]);
    evidence.steps.push('The welcome screen listed Claude Code and Codex, both selected.');
    await activate('#local-log-codex', '');
    await waitFor(async () => (await choices())[1]?.checked === 'false', 'Codex unselected');
    evidence.steps.push('The keyboard unselected Codex.');
  } else {
    assert.equal(await driver.execute('return document.querySelectorAll(".local-log-choice").length;'), 0);
  }
  await driver.type('#workspace-passphrase', password); if (setup) await driver.type('#workspace-confirmation', password);
  await key('');
  await waitFor(async () => (await call('bootstrap')).storage === 'ready', 'ready local log workspace');
  assert.equal((await call('bootstrap')).schemaVersion, 12);
}
async function scanned(after) {
  let view;
  await waitFor(async () => { view = await call('load_local_logs'); return view.scan.state === 'idle' && view.scan.lastScanAtUtc !== null && view.scan.lastScanAtUtc > after; }, 'a finished local log scan', 90000);
  assert.equal(view.consentRequired, false);
  assert.deepEqual(view.tools, [{ tool: 'claude', detected: true, granted: true }, { tool: 'codex', detected: true, granted: false }]);
  assert.equal(await driver.execute('return document.querySelector(".local-log-dialog")===null;'), true, 'A saved choice asks no second consent.');
  return view;
}
async function close() {
  const queue = [driver.child.pid]; let owned;
  for (let count = 0; queue.length && count < 64; count++) {
    const pid = queue.shift();
    try { if (await readlink(`/proc/${pid}/exe`) === application) { owned = pid; break; } queue.push(...(await readFile(`/proc/${pid}/task/${pid}/children`, 'utf8')).trim().split(/\s+/).filter(Boolean).map(Number)); }
    catch (error) { if (error.code !== 'ENOENT') throw error; }
  }
  assert.ok(owned, 'The native app must be an owned process.');
  execFileSync('/usr/bin/python3', [join(repository, 'tests/native/x11-close.py'), String(owned)], { env: fixture.environment, encoding: 'utf8', timeout: 10000 });
  await waitFor(async () => { try { await readlink(`/proc/${owned}/exe`); return false; } catch (error) { if (error.code === 'ENOENT') return true; throw error; } }, 'genuine native close');
  driver.session = undefined; await driver.stop(); driver = undefined;
}
async function oracle(label) {
  assert.equal(await hash(verifier), verifierDigest);
  const temporary = join(fixture.root, `local-log-oracle-${randomUUID()}`); await mkdir(temporary, { mode: 0o700 });
  try {
    const database = join(temporary, 'database.db'); await copyFile(fixture.database, database); await chmod(database, 0o600);
    try { await copyFile(`${fixture.database}-wal`, `${database}-wal`); await chmod(`${database}-wal`, 0o600); } catch (error) { if (error.code !== 'ENOENT') throw error; }
    const report = JSON.parse(execFileSync(verifier, ['database', database, join(dirname(fixture.database), 'database-key.v1'), '--profile-state'], { input: JSON.stringify({ passphrase: password }), encoding: 'utf8', env: fixture.environment, timeout: 30000, maxBuffer: 8 * 1024 * 1024 }));
    assert.equal(report.verified, true); assert.equal(Number(report.schemaVersion), 12);
    const tables = report.state.tables;
    const sources = tables.log_sources.map(s => ({ tool: s.tool, consent: s.consent, zone: s.time_zone }));
    const days = tables.local_usage_days.map(d => ({ date: d.local_date, tool: d.tool, model: d.model, total: d.total_tokens, value: d.estimated_value_usd_micros, coverage: d.coverage, scanner: d.scanner_version }))
      .sort((a, b) => `${a.date}${a.model}`.localeCompare(`${b.date}${b.model}`));
    evidence.oracles.push({ label, sources, days, differences: tables.local_usage_differences.length });
    return { sources, days, differences: tables.local_usage_differences.length };
  } finally { await rm(temporary, { recursive: true, force: true }); }
}
async function treeHashes(root) {
  const result = {};
  async function walk(dir) { for (const entry of (await readdir(dir, { withFileTypes: true })).sort((a, b) => a.name.localeCompare(b.name))) { const path = join(dir, entry.name); if (entry.isDirectory()) await walk(path); else result[path.slice(root.length)] = await hash(path); } }
  await walk(root); return result;
}

try {
  await mkdir(output, { recursive: true, mode: 0o700 });
  assert.equal(await hash(ccusage), ccusageDigest);
  evidence.applicationSha256 = await hash(application);
  fixture = await createFixture('local-log-consent');
  const logs = join(fixture.root, 'logs-home');
  await cp(join(repository, 'crates/openusage-locallogs/tests/fixtures/home'), logs, { recursive: true });
  // The Overview shows the last 30 days, so the synthetic Claude Code days move to the two days before today (UTC).
  const shifted = offset => { const d = new Date(); d.setUTCDate(d.getUTCDate() - offset); return d.toISOString().slice(0, 10); };
  const [older, newer] = [shifted(2), shifted(1)];
  const claudeLogs = join(logs, '.claude/projects/-home-dev-alpha');
  for (const name of readdirSync(claudeLogs)) {
    const path = join(claudeLogs, name);
    const shiftedLogs = (await readFile(path, 'utf8')).replace(/2026-11-0([45])T/g, (_, day) => `${day === '4' ? older : newer}T`);
    const rows = shiftedLogs.trim().split('\n').map((line, index) => {
      const row = JSON.parse(line);
      row.costUSD = name.startsWith('1') && index === 0 ? 0 : 999;
      return JSON.stringify(row);
    });
    await writeFile(path, `${rows.join('\n')}\n`, { mode: 0o600 });
  }
  evidence.steps.push(`The synthetic Claude Code days moved to ${older} and ${newer}, with stored charges of 0 and 999 dollars.`);
  const logsBefore = await treeHashes(logs);
  Object.assign(fixture.environment, { OPENUSAGE_NATIVE_LOG_HOME: logs, OPENUSAGE_NATIVE_CCUSAGE: ccusage });

  await open(true);
  const first = await scanned('');
  evidence.steps.push(`The first scan finished at ${first.scan.lastScanAtUtc}.`);
  // First launch reaches local history with no sign-in (OU-UI-025, AT-91).
  await waitFor(() => driver.execute('return Boolean(document.querySelector("[data-local-overview] [data-local-tool=claude]"));'), 'the This device section on Overview', 30000);
  assert.deepEqual(await driver.execute('return [...document.querySelectorAll("[data-local-overview] [data-local-tool-total]")].map(n=>n.getAttribute("aria-label"));'), ['Claude Code: 14,950 tokens']);
  assert.equal(await driver.execute('return Boolean(document.querySelector("[data-overview-add-account]"));'), true);
  evidence.steps.push('With no account, Overview showed Claude Code: 14,950 tokens and no Codex card.');
  // The same scan saved the Claude Code sessions with their project, and no reasoning count (SC-021).
  const usage = await call('load_local_usage', { days: 30, period: 'day' });
  assert.deepEqual(usage.sessions.map(s => [s.tool, s.key, s.project, s.startUtc, s.endUtc, s.totalTokens, s.reasoningTokens]), [
    ['claude', '33333333-cccc-4ccc-8ccc-333333333333', '-home-dev-alpha', `${newer}T13:00:00.000Z`, `${newer}T13:00:00.000Z`, '150', null],
    ['claude', '11111111-aaaa-4aaa-8aaa-111111111111', '-home-dev-alpha', `${older}T09:00:00.000Z`, `${newer}T11:00:00.000Z`, '14800', null],
  ]);
  assert.deepEqual(usage.projects, [{ tool: 'claude', project: '-home-dev-alpha', sessions: 2, totalTokens: '14950' }]);
  evidence.steps.push('The scan saved two Claude Code sessions in the project -home-dev-alpha, 14,950 tokens in all.');
  // The same scan saved the Claude Code hours, and Codex reports none (SC-021).
  const hourly = await call('load_local_usage', { days: 7, period: 'hour' });
  assert.deepEqual(hourly.hours.map(h => [h.startUtc, h.tool, h.totalTokens]), [
    [`${older}T09:00:00.000Z`, 'claude', '12500'], [`${newer}T11:00:00.000Z`, 'claude', '2300'], [`${newer}T13:00:00.000Z`, 'claude', '150'],
  ]);
  assert.deepEqual(hourly.hourTools, ['claude']);
  evidence.steps.push('The hourly view held three Claude Code hours with 12,500, 2,300 and 150 tokens.');
  await close();
  const saved = await oracle('after first launch');
  assert.deepEqual(saved.sources.map(s => [s.tool, s.consent]).sort(), [['claude', 'granted'], ['codex', 'withdrawn']]);
  assert.ok(saved.sources.find(s => s.tool === 'claude').zone, 'The first scan pins the zone.');
  assert.deepEqual(saved.days.map(d => [d.date, d.tool, d.model, d.total]), [
    [older, 'claude', 'claude-sonnet-4-5-20250929', '12500'],
    [newer, 'claude', 'claude-opus-4-1-20250805', '2300'],
    [newer, 'claude', 'totally-unknown-model-x', '150'],
  ]);
  assert.deepEqual(saved.days.map(d => d.value), ['31650', '76875', null], 'API-price estimates ignore stored charges, and an unknown model stays unavailable (OU-LOG-013, OU-LOG-014).');
  evidence.steps.push('Saved API-price estimates stayed 31,650 and 76,875 USD millionths. The unknown model kept no estimate.');
  assert.ok(saved.days.every(d => d.coverage === 'complete' && d.scanner === '20.0.48'));
  assert.equal(saved.differences, 0);

  await open(false);
  const second = await scanned(first.scan.lastScanAtUtc);
  evidence.steps.push(`A restart asked no consent, and the second scan finished at ${second.scan.lastScanAtUtc}.`);
  await close();
  const again = await oracle('after restart and second scan');
  assert.deepEqual(again, saved, 'The same logs give the same totals (OU-LOG-012).');
  assert.deepEqual(await treeHashes(logs), logsBefore, 'The scan leaves the synthetic logs unchanged.');
  assert.equal(await hash(application), evidence.applicationSha256);
  evidence.result = 'passed';
} catch (error) { evidence.error = error.stack ?? String(error); process.exitCode = 1; }
finally {
  try { await driver?.stop(); } finally {
    stopGroup(fixture?.bus, 'SIGKILL'); if (fixture) await rm(fixture.root, { recursive: true, force: true });
    evidence.finishedAt = new Date().toISOString();
    await writeFile(join(output, 'result.json'), JSON.stringify(evidence, null, 2) + '\n', { mode: 0o600 });
    console.log(`${evidence.requirement}: ${evidence.result}. Evidence: ${output}`);
  }
}
