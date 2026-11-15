import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { createHash } from 'node:crypto';
import { createServer } from 'node:https';
import { mkdir, readFile, readdir, rm, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { promisify } from 'node:util';
import { createFixture, stopGroup } from './private-fixture.mjs';
import { startSecretService, stopSecretService } from './secret-service.mjs';
import { waitFor } from './webdriver.mjs';

const run = promisify(execFile);
const repository = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
for (const key of ['OPENUSAGE_NATIVE_CLI', 'OPENUSAGE_BOARD_SEED_BINARY', 'OPENUSAGE_NATIVE_EVIDENCE']) assert.ok(process.env[key], key);
const executable = resolve(process.env.OPENUSAGE_NATIVE_CLI);
const seed = resolve(process.env.OPENUSAGE_BOARD_SEED_BINARY);
const output = resolve(process.env.OPENUSAGE_NATIVE_EVIDENCE);
const evidence = {
  requirement: 'OU-BOARD-001/002/003/004/005/006/010, OU-CLI-001/002/005/007/008',
  scope: 'Compiled Linux native-test CLI, private GNOME Secret Service and SQLCipher, synthetic local HTTPS OAuth/service stub. No live GitHub or hosted service. No user keyring.',
  result: 'failed', startedAt: new Date().toISOString(), checks: [],
};
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
let fixture, server, port, mode = 'authorize', failSave = true, rejected = false, token = '', sequence = 0;
const requests = [], joins = [], uploads = [], responses = [];
process.once('exit', () => { fixture?.daemon?.kill('SIGKILL'); stopGroup(fixture?.bus, 'SIGKILL'); });
async function cli(args, environment = fixture.environment) {
  try {
    const result = await run(executable, ['board', ...args], { env: environment, timeout: 30000, maxBuffer: 131072 });
    const response = { code: 0, ...result }; responses.push(response); return response;
  } catch (error) {
    if (typeof error.code !== 'number') throw error;
    const response = { code: error.code, stdout: error.stdout, stderr: error.stderr }; responses.push(response); return response;
  }
}
const expected = async (args, code) => {
  const response = await cli([...args, '--json']);
  assert.equal(response.code, code, response.stdout + response.stderr);
  return JSON.parse(response.stdout);
};
async function stableFiles() {
  const directory = dirname(fixture.database), result = {};
  for (const name of (await readdir(directory)).sort()) {
    if (name.endsWith('-shm')) { result[name] = 'wal index'; continue; }
    result[name] = hash(await readFile(join(directory, name)));
  }
  return result;
}
try {
  await mkdir(output, { recursive: true, mode: 0o700 });
  evidence.sourceCommit = (await run('git', ['rev-parse', 'HEAD'], { cwd: repository, timeout: 10000 })).stdout.trim();
  evidence.dirtySource = (await run('git', ['status', '--porcelain'], { cwd: repository, timeout: 10000 })).stdout.length > 0;
  evidence.executableSha256 = hash(await readFile(executable));
  evidence.seedSha256 = hash(await readFile(seed));
  fixture = await createFixture('board-cli');
  const home = join(fixture.root, 'home'); await mkdir(home, { mode: 0o700 });
  Object.assign(fixture.environment, { HOME: home, OPENUSAGE_PRIVATE_KEYRING_FIXTURE: fixture.root });
  await startSecretService(fixture);
  await run(seed, ['--ignored', '--exact', 'seed_private_cli_workspace'], { env: fixture.environment, timeout: 15000 });
  const baseline = await stableFiles();
  const cert = join(fixture.root, 'board-ca.pem'), key = join(fixture.root, 'board-key.pem');
  await run('openssl', ['req', '-x509', '-newkey', 'rsa:2048', '-nodes', '-sha256', '-days', '1', '-subj', '/CN=127.0.0.1', '-addext', 'subjectAltName=IP:127.0.0.1', '-addext', 'basicConstraints=critical,CA:TRUE', '-keyout', key, '-out', cert], { timeout: 15000 });
  server = createServer({ cert: await readFile(cert), key: await readFile(key) }, async (request, response) => {
    try {
      const chunks = []; for await (const chunk of request) chunks.push(chunk);
      const body = Buffer.concat(chunks).toString();
      requests.push({ path: request.url, body, authorization: request.headers.authorization });
      const reply = (status, value) => { response.writeHead(status, { 'content-type': 'application/json' }); response.end(JSON.stringify(value)); };
      assert.equal(request.method, 'POST');
      if (request.url === '/login/device/code') {
        assert.equal(body, 'client_id=synthetic-native-client');
        return reply(200, { device_code: 'a'.repeat(40), user_code: 'WDJB-MJHT', verification_uri: 'https://github.com/login/device', expires_in: 30, interval: 1 });
      }
      if (request.url === '/login/oauth/access_token') {
        assert.ok(body.includes('device_code=' + 'a'.repeat(40)));
        if (mode === 'pending') return reply(200, { error: 'authorization_pending' });
        return reply(200, { access_token: 'gho_synthetic_access_token_1234567890', token_type: 'bearer', scope: '',
          refresh_token: 'ghr_synthetic_refresh_token_1234567890', expires_in: 28800, refresh_token_expires_in: 15897600 });
      }
      if (request.url === '/v1/join') {
        const parsed = JSON.parse(body);
        assert.deepEqual(Object.keys(parsed).sort(), ['deviceId', 'githubToken']);
        assert.equal(parsed.githubToken, 'gho_synthetic_access_token_1234567890');
        assert.match(parsed.deviceId, /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/);
        joins.push(parsed.deviceId); token = String.fromCharCode(98 + sequence++).repeat(64);
        if (failSave) { failSave = false; await stopSecretService(fixture); }
        return reply(200, { token, login: 'synthetic-user' });
      }
      assert.equal(request.headers.authorization, `Bearer ${token}`);
      if (rejected) return reply(401, { error: 'not_joined' });
      if (request.url === '/v1/sync') {
        uploads.push(body); return reply(200, { accepted: JSON.parse(body).rows.length });
      }
      assert.equal(request.url, '/v1/leave');
      return reply(200, { left: true });
    } catch (error) { response.destroy(error); evidence.stubError = error.message; }
  });
  await new Promise(done => server.listen(0, '127.0.0.1', done)); port = server.address().port;
  fixture.environment.OPENUSAGE_NATIVE_BOARD_ORIGIN = `https://127.0.0.1:${port}/`;
  if (process.env.OPENUSAGE_BOARD_EXPECT_NORMAL === '1') {
    for (const action of ['join', 'sync', 'leave']) {
      assert.equal((await expected([action, '--yes'], 14)).error.code, 'board_unconfigured');
    }
    assert.equal(requests.length, 0);
    assert.deepEqual(await stableFiles(), baseline);
    evidence.checks.push('The ordinary unconfigured build ignored a valid private endpoint override and made no requests or saved-data changes.');
    evidence.result = 'passed';
  } else {
  for (const bus of [
    `unix:path=${fixture.root}/runtime/../runtime/bus`,
    `unix:path=${fixture.root}/../../outside/bus`,
    `${fixture.environment.DBUS_SESSION_BUS_ADDRESS};unix:path=/outside/bus`,
  ]) {
    const invalid = await cli(['join', '--yes', '--json'], { ...fixture.environment, DBUS_SESSION_BUS_ADDRESS: bus });
    assert.equal(invalid.code, 14);
  }
  assert.equal(requests.length, 0);
  evidence.checks.push('Native traversal and fallback bus addresses refused before credential or network access.');
  assert.equal((await expected(['join'], 3)).error.code, 'consent_required');
  assert.equal((await expected(['sync', '--yes'], 15)).error.code, 'board_credential');
  assert.equal(requests.length, 0);
  const prospective = await cli(['sync', '--dry-run', '--device-id', 'f9168c5e-ceb2-4faa-b6bf-329bf39fa1e4']);
  assert.equal(prospective.code, 0);
  assert.equal(requests.length, 0);
  evidence.checks.push('No-consent join, unenrolled sync, and dry run made no network request.');
  const partial = await expected(['join', '--yes'], 18);
  assert.equal(partial.error.code, 'board_partial');
  assert.equal(joins.length, 1);
  await startSecretService(fixture);
  mode = 'pending';
  const before = requests.length;
  const joining = cli(['join', '--yes', '--json']);
  await waitFor(() => requests.slice(before).some(r => r.path === '/login/device/code'), 'pending join starts');
  const alternate = join(fixture.root, 'alternate-data'); await mkdir(alternate, { mode: 0o700 });
  const busy = await cli(['join', '--yes', '--json'], { ...fixture.environment, XDG_DATA_HOME: alternate });
  assert.equal(busy.code, 16); assert.equal(JSON.parse(busy.stdout).error.code, 'board_busy');
  mode = 'authorize';
  const resumed = await joining;
  assert.equal(resumed.code, 0, resumed.stdout + resumed.stderr);
  const device = JSON.parse(resumed.stdout).deviceId;
  assert.equal(joins[0], device); assert.equal(joins[1], device);
  assert.match(resumed.stderr, /https:\/\/github.com\/login\/device/);
  evidence.checks.push('Failed local save retained the pending device. Explicit resume reused it. A concurrent process refused.');
  const count = requests.length;
  assert.equal((await expected(['join', '--yes'], 15)).error.code, 'board_credential');
  assert.equal(requests.length, count);
  const other = { ...fixture.environment, OPENUSAGE_NATIVE_BOARD_ORIGIN: 'https://127.0.0.1:1/' };
  const mismatch = await cli(['sync', '--yes', '--json'], other);
  assert.equal(mismatch.code, 15); assert.equal(requests.length, count);
  await stopSecretService(fixture); await startSecretService(fixture);
  const preview = await cli(['sync', '--dry-run', '--device-id', device]);
  assert.equal(preview.code, 0);
  const bytes = preview.stdout;
  const parsed = JSON.parse(bytes);
  assert.deepEqual(parsed.rows.map(row => [row.tool, row.model, row.inputTokens, row.outputTokens, row.cacheReadTokens, row.cacheWriteTokens, row.totalTokens, row.estimatedValueMicros]),
    [['claude', 'synthetic-model', 31, 7, 5, 3, 46, 12345]]);
  assert.deepEqual(Object.keys(parsed).sort(), ['deviceId', 'rows']);
  const sync = await expected(['sync', '--yes'], 0);
  assert.equal(sync.accepted, 1); assert.equal(uploads[0], bytes);
  const repeated = await expected(['sync', '--yes'], 0);
  assert.equal(repeated.accepted, 1); assert.equal(uploads[1], bytes);
  evidence.uploadSha256 = hash(bytes);
  evidence.checks.push('After a daemon restart, real and repeated sync sent the exact dry-run bytes and independent local totals.');
  rejected = true;
  assert.equal((await expected(['leave', '--yes'], 17)).error.code, 'board_remote');
  rejected = false;
  assert.equal((await expected(['sync', '--yes'], 0)).accepted, 1);
  const reconnect = await expected(['join', '--reconnect', '--yes'], 0);
  assert.equal(reconnect.deviceId, device); assert.equal(joins.at(-1), device);
  assert.equal((await expected(['sync', '--yes'], 0)).accepted, 1);
  assert.equal((await expected(['leave', '--yes'], 0)).state, 'left');
  const endCount = requests.length;
  assert.equal((await expected(['sync', '--yes'], 15)).error.code, 'board_credential');
  assert.equal(requests.length, endCount);
  assert.deepEqual(await stableFiles(), baseline);
  for (const response of responses) {
    for (const secret of ['gho_synthetic', 'ghr_synthetic', 'b'.repeat(64), 'c'.repeat(64), 'd'.repeat(64), '/synthetic/project']) {
      assert.ok(!(response.stdout + response.stderr).includes(secret));
    }
  }
  evidence.checks.push('Rejected leave retained enrollment. Reconnect rotated one device. Confirmed leave removed enrollment. Saved SQLCipher files stayed unchanged. No token reached command output.');
  evidence.requestCounts = Object.fromEntries([...new Set(requests.map(r => r.path))].map(path => [path, requests.filter(r => r.path === path).length]));
  evidence.result = 'passed';
  }
} catch (error) { evidence.error = [error.stack ?? String(error), error.stdout ?? '', error.stderr ?? ''].join('\n'); process.exitCode = 1; }
finally {
  if (server) await new Promise(done => server.close(done));
  await stopSecretService(fixture); stopGroup(fixture?.bus, 'SIGKILL');
  if (fixture) await rm(fixture.root, { recursive: true, force: true });
  evidence.finishedAt = new Date().toISOString();
  await writeFile(join(output, 'result.json'), JSON.stringify(evidence, null, 2) + '\n', { mode: 0o600 });
  console.log(`Private board CLI: ${evidence.result}. Evidence: ${output}`);
  if (evidence.error) console.error(evidence.error);
}
