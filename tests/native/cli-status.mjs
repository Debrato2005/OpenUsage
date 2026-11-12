import assert from 'node:assert/strict';
import { execFile, execFileSync } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { cp, mkdir, readFile, readdir, readlink, rm, writeFile } from 'node:fs/promises';
import { readdirSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { promisify } from 'node:util';
import { fileURLToPath } from 'node:url';
import { createFixture, stopGroup } from './private-fixture.mjs';
import { startSecretService, stopSecretService } from './secret-service.mjs';
import { NativeDriver, waitFor } from './webdriver.mjs';

// The CLI reads saved data from a real OS-key-store workspace, read-only and with no prompt,
// while the app runs and after it closes (OU-CLI-002, OU-CLI-006, AT-78, AT-79).
for (const name of ['OPENUSAGE_NATIVE_BINARY', 'OPENUSAGE_NATIVE_EVIDENCE', 'OPENUSAGE_NATIVE_CLI']) assert.ok(process.env[name], name);
const repository = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const application = resolve(process.env.OPENUSAGE_NATIVE_BINARY), output = resolve(process.env.OPENUSAGE_NATIVE_EVIDENCE), cli = resolve(process.env.OPENUSAGE_NATIVE_CLI);
const ccusage = join(repository, 'apps/desktop/src-tauri/binaries/openusage-ccusage-x86_64-unknown-linux-gnu');
const run = promisify(execFile);
const hash = async path => createHash('sha256').update(await readFile(path)).digest('hex');
const evidence = {
  requirement: 'NATIVE-CLI-STATUS-001', result: 'failed', startedAt: new Date().toISOString(),
  scope: 'Actual Linux Tauri native-test build and native-test openusage CLI, a private GNOME Secret Service, a private SQLCipher 4 key-store workspace, synthetic Claude Code logs and the real pinned ccusage 20.0.48. No real logs, accounts or credential stores.',
  expectedResult: ['status --json and statusline give the exact saved total for today while the app runs and after it closes.', 'The CLI leaves the database, its WAL and the file set unchanged when the app is closed.', 'With the key store gone, statusline prints OpenUsage locked with exit code 7 and no prompt.'],
  steps: [],
};
let fixture, driver, sequence = 0;
const call = async (command, args) => { const response = await driver.invoke(command, args); assert.equal(response.ok, true, `${command}: ${JSON.stringify(response)}`); return response.value; };
async function key(value) {
  await driver.request('POST', `/session/${driver.session}/actions`, { actions: [{ type: 'key', id: 'cli-keyboard', actions: [{ type: 'keyDown', value }, { type: 'keyUp', value }] }] });
  await driver.request('DELETE', `/session/${driver.session}/actions`);
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
async function openusage(args, environment = fixture.environment) {
  try {
    const { stdout, stderr } = await run(cli, args, { env: environment, timeout: 15000, maxBuffer: 1024 * 1024 });
    return { code: 0, stdout, stderr };
  } catch (error) {
    if (typeof error.code !== 'number') throw error;
    return { code: error.code, stdout: error.stdout, stderr: error.stderr };
  }
}
async function workspaceFiles() {
  const directory = dirname(fixture.database), result = {};
  for (const name of (await readdir(directory)).sort()) {
    try { result[name] = await hash(join(directory, name)); } catch (error) { if (error.code !== 'EISDIR') throw error; result[name] = 'directory'; }
  }
  return result;
}
// A SQLite reader can update the shared WAL index in -shm. The database, its WAL and the file set stay unchanged.
const stable = files => Object.fromEntries(Object.entries(files).map(([name, digest]) => [name, name.endsWith('-shm') ? 'wal index' : digest]));
async function expectToday(label, today) {
  const status = await openusage(['status', '--json']);
  assert.equal(status.code, 0, `${label}: ${status.stderr}`);
  const report = JSON.parse(status.stdout);
  assert.equal(report.schemaVersion, 2);
  assert.deepEqual(report.profiles, []);
  assert.equal(report.local.scopes[0].date, today);
  assert.deepEqual(report.local.scopes.map(t => [t.tool, t.kind, t.timeZone, t.metrics.totalTokens.value, t.metrics.estimatedValueUsdMicros.value]), [['claude', 'legacy', 'UTC', '2450', null]]);
  assert.equal(report.local.scopes[0].metrics.totalTokens.observedAtUtc, null);
  assert.equal(report.local.scopes[0].metrics.reasoningTokens.state, 'not_available');
  const line = await openusage(['statusline']);
  assert.deepEqual([line.code, line.stdout], [0, 'Claude Code legacy 1 2.4K today (UTC)\n']);
  evidence.steps.push(`${label}: status --json gave Claude Code 2450 tokens for ${today}, and statusline gave one line.`);
}

try {
  await mkdir(output, { recursive: true, mode: 0o700 });
  evidence.applicationSha256 = await hash(application); evidence.cliSha256 = await hash(cli);
  fixture = await createFixture('cli-status');
  await startSecretService(fixture);
  const logs = join(fixture.root, 'logs-home');
  await cp(join(repository, 'crates/openusage-locallogs/tests/fixtures/home/.claude'), join(logs, '.claude'), { recursive: true });
  await cp(join(repository, 'crates/openusage-locallogs/tests/fixtures/codex'), join(logs, '.codex'), { recursive: true });
  const shifted = offset => { const d = new Date(); d.setUTCDate(d.getUTCDate() - offset); return d.toISOString().slice(0, 10); };
  const [yesterday, today] = [shifted(1), shifted(0)];
  const claudeLogs = join(logs, '.claude/projects/-home-dev-alpha');
  for (const name of readdirSync(claudeLogs)) {
    const path = join(claudeLogs, name);
    await writeFile(path, (await readFile(path, 'utf8')).replace(/2026-11-0([45])T/g, (_, day) => `${day === '4' ? yesterday : today}T`), { mode: 0o600 });
  }
  Object.assign(fixture.environment, { OPENUSAGE_NATIVE_LOG_HOME: logs, OPENUSAGE_NATIVE_CCUSAGE: ccusage });

  driver = new NativeDriver(application, fixture.environment, join(output, `driver-${++sequence}.log`)); await driver.start();
  await waitFor(() => driver.execute('const b=[...document.querySelectorAll("button")].find(n=>n.textContent.trim()==="Create private workspace");return Boolean(b&&!b.disabled&&document.querySelectorAll(".local-log-choice .local-log-tool").length===2);'), 'key-store setup with the consent list');
  assert.equal(await driver.execute('const b=[...document.querySelectorAll("button")].find(n=>n.textContent.trim()==="Create private workspace");b.focus();return document.activeElement===b;'), true);
  await key('');
  await waitFor(async () => (await call('bootstrap')).storage === 'ready', 'ready key-store workspace');
  assert.equal((await call('bootstrap')).keyProtection, 'os_store');
  await waitFor(async () => (await call('load_local_logs')).scan.lastScanAtUtc !== null, 'the first local log scan', 90000);
  evidence.steps.push('The app created a key-store workspace with both tools selected and scanned the synthetic logs.');

  await expectToday('With the app open', today);
  await close();
  const before = await workspaceFiles();
  await expectToday('With the app closed', today);
  assert.deepEqual(stable(await workspaceFiles()), stable(before), 'The CLI leaves the database, its WAL and the file set unchanged.');
  evidence.steps.push(`With the app closed, the CLI left the database, its WAL and the file set unchanged: ${Object.keys(before).join(', ')}.`);

  const reportEnvironment = { ...fixture.environment, HOME: logs, TZ: 'UTC', OPENUSAGE_CCUSAGE: ccusage };
  const noConsent = await openusage(['report', '--json'], reportEnvironment);
  assert.equal(noConsent.code, 3);
  assert.equal(JSON.parse(noConsent.stdout).error.code, 'consent_required');
  const scan = async () => {
    const result = await openusage(['report', '--yes', '--json'], reportEnvironment);
    assert.equal(result.code, 0, result.stdout + result.stderr);
    const document = JSON.parse(result.stdout);
    assert.equal(document.schemaVersion, 2);
    return document.tools.map(t => [t.tool, t.metrics.totalTokens.value]);
  };
  const original = await scan();
  assert.deepEqual(original.map(([tool]) => tool), ['claude', 'codex']);
  await cp(join(logs, '.claude'), join(logs, '.config/claude'), { recursive: true });
  assert.deepEqual(await scan(), original, 'Copied Claude roots count once in the complete daily report.');
  assert.deepEqual(stable(await workspaceFiles()), stable(before), 'Report saves no workspace records.');
  evidence.steps.push('Report refused absent consent, accepted --yes, kept copied Claude roots at the original totals, and left saved workspace files unchanged.');

  const shiftedFiles = [];
  async function moveSourceDays(directory) {
    for (const entry of await readdir(directory, { withFileTypes: true })) {
      const path = join(directory, entry.name);
      if (entry.isDirectory()) await moveSourceDays(path);
      else {
        assert.ok(entry.isFile() && entry.name.endsWith('.jsonl'));
        await writeFile(path, (await readFile(path, 'utf8')).replace(/\d{4}-\d{2}-\d{2}T/g, `${yesterday}T`), { mode: 0o600 });
        shiftedFiles.push(path);
      }
    }
  }
  for (const folder of ['.claude', '.config/claude', '.codex']) await moveSourceDays(join(logs, folder));
  const sourceHashes = () => Promise.all(shiftedFiles.map(async path => [path, await hash(path)]));
  const shiftedBefore = await sourceHashes();
  const emptyRange = await openusage(['report', '--yes', '--days', '1'], reportEnvironment);
  assert.deepEqual([emptyRange.code, emptyRange.stdout, emptyRange.stderr], [0, `Local logs from ${today} to ${today}, days in UTC\nNo local usage in this date range\n`, '']);
  const emptyJson = await openusage(['report', '--yes', '--days', '1', '--json'], reportEnvironment);
  assert.equal(emptyJson.code, 0);
  assert.deepEqual(JSON.parse(emptyJson.stdout).tools, []);
  assert.deepEqual(await scan(), original);
  assert.deepEqual(await sourceHashes(), shiftedBefore);
  assert.deepEqual(stable(await workspaceFiles()), stable(before));
  evidence.steps.push('An empty selected day reports no usage. Older source logs keep their totals, and report leaves source and workspace files unchanged.');

  driver = new NativeDriver(application, fixture.environment, join(output, `driver-${++sequence}.log`));
  await driver.start();
  await waitFor(async () => (await call('bootstrap')).storage === 'ready', 'reopened key-store workspace');
  const created = await call('create_profile', { label: 'CLI connection control' });
  assert.equal(created.profiles.length, 1);
  assert.equal(created.profiles[0].connectionState, 'authentication_required');
  await close();
  const connectionBefore = await workspaceFiles();
  const connectionJson = await openusage(['status', '--json']);
  assert.equal(connectionJson.code, 0);
  const profile = JSON.parse(connectionJson.stdout).profiles[0];
  assert.equal(profile.connectionState, 'authentication_required');
  assert.deepEqual(profile.quota, { state: 'not_available', status: null, source: null, observedAtUtc: null, windows: [] });
  const connectionText = await openusage(['status']);
  assert.equal(connectionText.code, 0);
  assert.ok(connectionText.stdout.startsWith('CLI connection control Authentication required\n  Quota Not available\n'));
  const connectionLine = await openusage(['statusline']);
  assert.deepEqual([connectionLine.code, connectionLine.stdout, connectionLine.stderr], [0, 'CLI connection control Not available Authentication required | Claude Code legacy 1 2.4K today (UTC)\n', '']);
  assert.deepEqual(stable(await workspaceFiles()), stable(connectionBefore));
  assert.deepEqual(await sourceHashes(), shiftedBefore);
  evidence.steps.push('A native profile without a sign-in shows Authentication required and unavailable quota. CLI reads preserve saved files and source hashes.');

  await stopSecretService(fixture);
  const locked = await openusage(['statusline']);
  assert.deepEqual([locked.code, locked.stdout, locked.stderr], [7, 'OpenUsage locked\n', '']);
  const json = await openusage(['status', '--json']);
  assert.equal(json.code, 7);
  assert.equal(JSON.parse(json.stdout).error.code, 'locked');
  evidence.steps.push('With the Secret Service gone, statusline and status --json gave locked with exit code 7 and no prompt.');
  assert.deepEqual(stable(await workspaceFiles()), stable(connectionBefore));
  assert.equal(await hash(application), evidence.applicationSha256);
  evidence.result = 'passed';
} catch (error) { evidence.error = error.stack ?? String(error); process.exitCode = 1; }
finally {
  try { await driver?.stop(); } finally {
    try { await stopSecretService(fixture); } catch { /* already stopped */ }
    stopGroup(fixture?.bus, 'SIGKILL'); if (fixture) await rm(fixture.root, { recursive: true, force: true });
    evidence.finishedAt = new Date().toISOString();
    await writeFile(join(output, 'result.json'), JSON.stringify(evidence, null, 2) + '\n', { mode: 0o600 });
    console.log(`${evidence.requirement}: ${evidence.result}. Evidence: ${output}`);
  }
}
