import { EXPECTED_SCHEMA_VERSION, EXPECTED_TABLE_COUNT } from './schema.mjs';
import assert from 'node:assert/strict';
import { execFileSync, spawn } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { chmod, copyFile, cp, mkdir, readFile, readlink, rename, rm, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createFixture, stopGroup } from './private-fixture.mjs';
import { NativeDriver, waitFor } from './webdriver.mjs';
import { loginVerifierDigest, runLoginVerifier } from './login-verifier.mjs';

// Linux acceptance uses private homes, a private bus, Xvfb, and exact reviewed fixtures.
for (const name of ['OPENUSAGE_NATIVE_BINARY', 'OPENUSAGE_NATIVE_EVIDENCE']) assert.ok(process.env[name], name);
assert.equal(process.platform, 'linux');
const harness = dirname(fileURLToPath(import.meta.url)), repository = resolve(harness, '../..');
const application = resolve(process.env.OPENUSAGE_NATIVE_BINARY), output = resolve(process.env.OPENUSAGE_NATIVE_EVIDENCE);
const production = process.env.OPENUSAGE_SHUTDOWN_PRODUCTION_GATE === '1';
const password = 'OpenUsage synthetic owned shutdown workspace 2026';
const hash = async path => createHash('sha256').update(await readFile(path)).digest('hex');
const evidence = { requirement: production ? 'NATIVE-SCANNER-PRODUCTION-GATE-001' : 'NATIVE-OWNED-SHUTDOWN-001',
  result: 'failed', startedAt: new Date().toISOString(),
  scope: production ? 'Normal Linux build ignores the exact native-test scanner override in a private password workspace. No real logs, provider, credential, or startup setting.' : 'Actual Linux native-test app Quit with a slow owned scanner, a test-owned inherited stdout keeper, an unrelated sentinel, and a pending scheduled synthetic provider collection. All encrypted schema-14 tables are compared through a read-only oracle. No live provider, tray, autostart, Windows, or macOS acceptance.', steps: [], oracles: [] };
let fixture, driver, keeper, sentinel, logs, scanner, workspace;
const call = async (command, args = {}) => { const reply = await driver.invoke(command, args); assert.equal(reply.ok, true, `${command}: ${JSON.stringify(reply)}`); return reply.value; };
async function rows(path) { try { return (await readFile(path, 'utf8')).trim().split('\n').filter(Boolean).map(JSON.parse); } catch (error) { if (error.code === 'ENOENT') return []; throw error; } }
const scannerEvents = () => rows(join(logs, '.claude/synthetic-scanner-events.jsonl'));
const alive = pid => { try { process.kill(pid, 0); return true; } catch (error) { if (error.code === 'ESRCH') return false; throw error; } };
async function appPid() {
  const queue = [driver.child.pid];
  for (let count = 0; queue.length && count < 64; count++) {
    const pid = queue.shift();
    try { if (await readlink(`/proc/${pid}/exe`) === application) return pid; queue.push(...(await readFile(`/proc/${pid}/task/${pid}/children`, 'utf8')).trim().split(/\s+/).filter(Boolean).map(Number)); }
    catch (error) { if (error.code !== 'ENOENT') throw error; }
  }
  assert.fail('No owned desktop process.');
}
async function oracle(label) {
  assert.ok(process.env.OPENUSAGE_LOGIN_VERIFIER);
  const directory = join(fixture.root, `oracle-${randomUUID()}`); await mkdir(directory, { mode: 0o700 });
  try {
    const database = join(directory, 'workspace.db'); await copyFile(fixture.database, database); await chmod(database, 0o600);
    try { await copyFile(`${fixture.database}-wal`, `${database}-wal`); await chmod(`${database}-wal`, 0o600); } catch (error) { if (error.code !== 'ENOENT') throw error; }
    const report = JSON.parse(await runLoginVerifier(process.env.OPENUSAGE_LOGIN_VERIFIER, loginVerifierDigest(),
      ['database', database, join(workspace, 'database-key.v1'), '--profile-state'], { input: JSON.stringify({ passphrase: password }), env: fixture.environment, encoding: 'utf8', timeout: 60000, maxBuffer: 16 * 1024 * 1024 }));
    for (const key of ['verified', 'readOnly', 'encryptedPagesAuthenticated', 'originalProtectedKeyUsed', 'inputUnchanged', 'keyEnvelopeUnchanged', 'walRecordsUnchanged']) assert.equal(report[key], true, key);
    assert.equal(report.schemaVersion, EXPECTED_SCHEMA_VERSION); assert.equal(Object.keys(report.state.tables).length, EXPECTED_TABLE_COUNT);
    evidence.oracles.push({ label, schemaVersion: report.schemaVersion, tables: Object.fromEntries(Object.entries(report.state.tables).map(([name, value]) => [name, { rows: value.length, sha256: createHash('sha256').update(JSON.stringify(value)).digest('hex') }])), readOnly: true, encryptedCopy: true });
    return report.state.tables;
  } finally { await rm(directory, { recursive: true, force: true }); }
}
async function configure(home, mode) { const path = join(home, 'synthetic-collection-config.json'); await writeFile(`${path}.next`, JSON.stringify({ mode, requireLogin: true }), { mode: 0o600 }); await rename(`${path}.next`, path); }
async function closeNormally() {
  const pid = await appPid(), started = performance.now();
  evidence.nativeClose = JSON.parse(execFileSync('/usr/bin/python3', [join(harness, 'x11-close.py'), String(pid)], { env: fixture.environment, encoding: 'utf8', timeout: 10000 }));
  await waitFor(() => !alive(pid), 'bounded normal native exit', 8000);
  evidence.quitMilliseconds = Math.round(performance.now() - started);
  assert.ok(evidence.quitMilliseconds < 8000);
  driver.session = undefined; await driver.stop(); driver = undefined;
}

await mkdir(output, { recursive: true, mode: 0o700 });
process.once('exit', () => { driver?.kill('SIGKILL'); stopGroup(keeper, 'SIGKILL'); stopGroup(sentinel, 'SIGKILL'); stopGroup(fixture?.bus, 'SIGKILL'); });
try {
  evidence.applicationSha256 = await hash(application);
  if (!production) {
    evidence.oracleSha256 = await hash(process.env.OPENUSAGE_LOGIN_VERIFIER);
    assert.equal(evidence.oracleSha256, loginVerifierDigest());
  }
  evidence.sourceCommit = execFileSync('git', ['rev-parse', 'HEAD'], { cwd: repository, encoding: 'utf8' }).trim();
  evidence.sourceHashes = {};
  for (const path of ['crates/openusage-locallogs/src/runner.rs', 'crates/openusage-locallogs/src/grouped.rs', 'crates/openusage-locallogs/src/lib.rs', 'apps/desktop/src-tauri/src/local_logs.rs', 'apps/desktop/src-tauri/src/monitoring.rs', 'apps/desktop/src-tauri/src/shutdown.rs', 'apps/desktop/src-tauri/src/main.rs', 'tests/native/owned-shutdown.mjs', 'tests/native/fixtures/synthetic-scanner.py', 'Cargo.lock']) evidence.sourceHashes[path] = await hash(join(repository, path));
  fixture = await createFixture('owned-shutdown'); workspace = dirname(fixture.database);
  logs = join(fixture.root, 'home'); await cp(join(repository, 'crates/openusage-locallogs/tests/fixtures/home'), logs, { recursive: true });
  scanner = join(fixture.root, 'synthetic-scanner'); await copyFile(join(harness, 'fixtures/synthetic-scanner.py'), scanner); await chmod(scanner, 0o700);
  Object.assign(fixture.environment, { HOME: logs, OPENUSAGE_NATIVE_LOG_HOME: logs, OPENUSAGE_NATIVE_SCANNER_FIXTURE: scanner, TZ: 'UTC' });
  await writeFile(join(logs, '.claude/synthetic-scanner-mode'), 'seed', { mode: 0o600 });
  for (const [kind, name] of [['daily', 'daily-by-agent'], ['session', 'session'], ['blocks', 'blocks']]) await copyFile(join(repository, `crates/openusage-locallogs/tests/fixtures/ccusage-20.0.48-${name}-claude.json`), join(logs, `.claude/synthetic-scanner-${kind}.json`));
  driver = new NativeDriver(application, fixture.environment, join(output, 'driver.log')); await driver.start();
  await waitFor(() => driver.execute('return ["#workspace-passphrase","#workspace-confirmation"].every(selector=>{const n=document.querySelector(selector);return Boolean(n&&!n.disabled);});'), 'private password setup');
  await driver.type('#workspace-passphrase', password); await driver.type('#workspace-confirmation', password); await driver.click('.unlock-form button[type=submit]');
  await waitFor(async () => (await call('bootstrap')).storage === 'ready', 'private workspace ready');
  await call('save_local_log_consent', { tools: ['claude'] });
  if (production) {
    await waitFor(async () => { const scan = (await call('load_local_logs')).scan; return ['failed', 'unavailable'].includes(scan.state) || scan.lastScanAtUtc !== null; }, 'normal scanner attempt completed', 30000);
    assert.deepEqual(await scannerEvents(), [], 'Normal build must not run the test fixture override.');
    const bytes = await readFile(application);
    assert.equal(bytes.includes(Buffer.from('OPENUSAGE_NATIVE_SCANNER_FIXTURE')), false, 'Normal binary must omit the fixture override.');
    assert.equal(bytes.includes(Buffer.from('20.0.48-openusage.shutdown-fixture.1')), false, 'Normal binary must omit the fixture version.');
    evidence.steps.push({ compiledFixtureOverrideAbsent: true, exactFixtureNotExecuted: true, privateHome: true });
    await closeNormally();
  } else {
    await waitFor(async () => (await call('load_local_logs')).scan.lastScanAtUtc !== null, 'seed scan accepted');
    const profile = (await call('create_profile', { label: 'Synthetic shutdown account' })).profiles[0]; assert.ok(profile);
    const home = join(workspace, 'profiles', profile.id); await configure(home, 'success');
    const executable = join(fixture.root, 'synthetic-provider'); await copyFile(join(repository, 'crates/openusage-codex/tests/fixtures/synthetic-collection-provider.py'), executable); await chmod(executable, 0o700);
    assert.equal(await hash(executable), '53a853b737ceca311649e9f1d9ebd1341154a5f782efd8e915f7f834c819a6dc');
    const candidate = await call('inspect_provider_executable', { path: executable }); assert.equal(candidate.sourceKind, 'test_fixture');
    const approval = await call('approve_provider_executable', { candidateId: candidate.candidateId, confirmed: true });
    const approvalId = approval.approvalId;
    const login = await call('begin_profile_login', { profileId: profile.id, approvalId, flow: 'device_code', confirmed: true });
    const loginArgs = { profileId: profile.id, attemptId: login.attemptId };
    await call('await_profile_login_prompt', loginArgs); await call('complete_profile_login', loginArgs);
    const seed = await call('begin_profile_collection_read', { profileId: profile.id, approvalId });
    assert.equal((await call('complete_profile_collection_read', { profileId: profile.id, requestId: seed.requestId })).accepted, true);
    await configure(home, 'activity_hang');
    await call('start_profile_monitoring', { profileId: profile.id, approvalId });
    let provider;
    await waitFor(async () => { provider = (await rows(join(home, 'synthetic-collection-observations.jsonl'))).findLast(row => row.kind === 'activity-pending'); return (await call('load_monitoring')).profiles.some(row => row.profileId === profile.id && row.readingActive) && provider && alive(provider.pid); }, 'pending scheduled provider worker');
    keeper = spawn(scanner, ['--pipe-keeper', join(logs, '.claude')], { detached: true, stdio: 'ignore', env: fixture.environment });
    await waitFor(async () => (await scannerEvents()).some(row => row.kind === 'keeper-ready'), 'external pipe keeper');
    sentinel = spawn('/usr/bin/python3', ['-c', 'import time; time.sleep(120)'], { detached: true, stdio: 'ignore', env: fixture.environment });
    await writeFile(join(logs, '.claude/synthetic-scanner-mode'), 'slow', { mode: 0o600 });
    await call('save_local_log_consent', { tools: ['claude'] });
    await waitFor(async () => (await scannerEvents()).some(row => row.kind === 'inherited-pipe-ready'), 'owned scanner waiting with inherited stdout');
    const events = await scannerEvents(), owned = events.findLast(row => row.kind === 'start' && row.mode === 'slow'), holder = events.find(row => row.kind === 'pipe-held');
    assert.ok(owned && holder && alive(owned.pid) && alive(holder.holderPid) && alive(sentinel.pid));
    const before = await oracle('pending scanner and scheduled provider before Quit');
    const keyHash = await hash(join(workspace, 'database-key.v1'));
    await driver.screenshot(join(output, 'before-quit.png'));
    await closeNormally();
    assert.equal(alive(owned.pid), false); assert.equal(alive(provider.pid), false);
    for (const [kind, pid] of [['external keeper', keeper.pid], ['inherited FD holder', holder.holderPid], ['unrelated sentinel', sentinel.pid]]) assert.equal(alive(pid), true, `${kind} must survive app Quit`);
    const after = await oracle('after Quit'); assert.deepEqual(after, before, 'Quit must preserve every accepted table and publish no pending report.');
    assert.equal(await hash(join(workspace, 'database-key.v1')), keyHash);
    evidence.steps.push({ ownedScannerReaped: true, ownedScheduledProviderReaped: true, externalKeeperSurvived: true, inheritedFdHolderSurvived: true, unrelatedSentinelSurvived: true, allFortyTwoTablesUnchanged: true, protectedKeyUnchanged: true, providerAndScannerWereConcurrent: true });
  }
  assert.equal(await hash(application), evidence.applicationSha256);
  evidence.result = 'passed';
} catch (error) {
  evidence.error = error.stack ?? String(error); process.exitCode = 1;
  if (driver?.session) try { await driver.screenshot(join(output, 'failure.png')); } catch {}
} finally {
  try { await driver?.stop(); } catch (error) { evidence.cleanupError = error.message; evidence.result = 'failed'; process.exitCode = 1; }
  stopGroup(keeper, 'SIGTERM'); stopGroup(sentinel, 'SIGTERM');
  if (fixture) { stopGroup(fixture.bus, 'SIGKILL'); await rm(fixture.root, { recursive: true, force: true }); }
  evidence.finishedAt = new Date().toISOString(); await writeFile(join(output, 'result.json'), JSON.stringify(evidence, null, 2) + '\n', { mode: 0o600 });
  console.log(`${evidence.requirement}: ${evidence.result}. Evidence: ${output}`); if (evidence.error) console.error(evidence.error);
}
