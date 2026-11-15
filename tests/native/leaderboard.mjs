import assert from 'node:assert/strict';
import { execFile, execFileSync, spawn } from 'node:child_process';
import { createHash } from 'node:crypto';
import { createServer } from 'node:https';
import { mkdir, readFile, readlink, rm, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { promisify } from 'node:util';
import { createFixture, stopGroup } from './private-fixture.mjs';
import { startSecretService, stopSecretService } from './secret-service.mjs';
import { NativeDriver, waitFor } from './webdriver.mjs';

assert.equal(process.env.OPENUSAGE_NATIVE_PRIVATE_XVFB, '1', 'Use a private display.');
for (const name of ['OPENUSAGE_NATIVE_BINARY', 'OPENUSAGE_BOARD_SEED_BINARY', 'OPENUSAGE_NATIVE_EVIDENCE']) assert.ok(process.env[name], name);
const run = promisify(execFile);
const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const application = resolve(process.env.OPENUSAGE_NATIVE_BINARY);
const seed = resolve(process.env.OPENUSAGE_BOARD_SEED_BINARY);
const output = resolve(process.env.OPENUSAGE_NATIVE_EVIDENCE);
const digest = bytes => createHash('sha256').update(bytes).digest('hex');
const evidence = { result: 'failed', requirement: 'L-5b, OU-BOARD-001/002/003/004/005/006/010, OU-SEC-003, OU-DATA-016/017', scope: 'Compiled Linux native-test app, private display, GNOME Secret Service, SQLCipher seeded through a typed Rust fixture, and synthetic HTTPS GitHub/service endpoints. No live GitHub, hosted service, user keyring, or scanner qualification.', startedAt: new Date().toISOString(), checks: [], screenshots: [] };
let fixture, driver, server, guardHolder, appPid, sequence = 0;
const requests = [], joins = [], uploads = [], publicViews = [], issuedTokens = [];
let token = '', tokenSequence = 0, revoked = false, selection = 'alpha';
let hold = null, held = null, saveFailure = false, cleanupFailure = false, lostAck = false, pendingOAuth = false;
const alpha = 'gho_synthetic_access_token_1234567890';
const beta = 'gho_synthetic_beta_access_token_1234567890';
const deviceCode = 'a'.repeat(40);
const view = async (refresh = false) => {
  const result = await driver.invoke(refresh ? 'refresh_leaderboard' : 'load_leaderboard');
  assert.equal(result.ok, true, JSON.stringify(result));
  publicViews.push(result.value); return result.value;
};
async function terminal(id) {
  let result;
  await waitFor(async () => {
    result = await view();
    return result.operation?.id === id && result.operation.phase === 'finished' && !result.busy;
  }, 'terminal board operation', 25000);
  return result;
}
async function begin(kind, extra = {}) {
  const result = await driver.invoke('begin_leaderboard', { kind, ...extra, confirmed: true });
  assert.equal(result.ok, true, JSON.stringify(result));
  assert.equal(result.value.operation.kind, kind); return result.value.operation.id;
}
async function rejected(command, args, error) {
  assert.deepEqual(await driver.invoke(command, args), { ok: false, error });
}
async function button(text, scope = '[data-leaderboard]') {
  let node;
  await waitFor(async () => {
    node = await driver.execute('const scope=document.querySelector(arguments[1]);return scope?[...scope.querySelectorAll("button")].find(node=>node.getClientRects().length&&getComputedStyle(node).visibility!=="hidden"&&!node.disabled&&node.getAttribute("aria-disabled")!=="true"&&node.textContent.trim()===arguments[0])??null:null;', [text, scope]);
    return Boolean(node);
  }, `Available board button: ${text}`);
  await driver.execute('arguments[0].scrollIntoView({block:"center"});return true;', [node]);
  await driver.request('POST', `/session/${driver.session}/element/${node['element-6066-11e4-a52e-4f735466cecf']}/click`, {});
}
async function visibleText(text) {
  await waitFor(() => driver.execute('return document.querySelector("[data-leaderboard]")?.textContent.includes(arguments[0]);', [text]), text);
}
async function settings() {
  await driver.navigate('settings');
  if (!await driver.execute('return document.querySelector("[data-leaderboard-toggle]").parentElement.open;')) await driver.click('[data-leaderboard-toggle]');
  await waitFor(() => driver.execute('return !document.querySelector("[data-leaderboard]").textContent.includes("Please wait");'), 'board status loaded');
}
async function capture(name) {
  await driver.execute('document.querySelector("[data-leaderboard]").scrollIntoView({block:"center"});return true;');
  await driver.screenshot(join(output, name)); evidence.screenshots.push(name);
}
async function start() {
  driver = new NativeDriver(application, fixture.environment, join(output, `driver-${++sequence}.log`));
  await driver.start();
  await waitFor(async () => (await driver.invoke('bootstrap')).value?.storage === 'ready', 'ready synthetic workspace');
  const queue = [driver.child.pid]; appPid = null;
  for (let count = 0; queue.length && count < 64; count++) {
    const pid = queue.shift();
    try {
      if (await readlink(`/proc/${pid}/exe`) === application) { appPid = pid; break; }
      queue.push(...(await readFile(`/proc/${pid}/task/${pid}/children`, 'utf8')).trim().split(/\s+/).filter(Boolean).map(Number));
    } catch (error) { if (error.code !== 'ENOENT') throw error; }
  }
  assert.ok(appPid);
}
async function closeNormally(release) {
  const pid = appPid;
  const result = JSON.parse(execFileSync('/usr/bin/python3', [join(root, 'tests/native/x11-close.py'), String(pid)], { env: fixture.environment, encoding: 'utf8', timeout: 10000 }));
  assert.ok(result);
  if (release) await release();
  await waitFor(async () => { try { await readlink(`/proc/${pid}/exe`); return false; } catch (error) { if (error.code === 'ENOENT') return true; throw error; } }, 'owned native exit', 25000);
  driver.session = undefined; await driver.stop(); driver = undefined;
}
async function seeded(mode) {
  await run(seed, ['--ignored', '--exact', 'seed_private_cli_workspace'], { env: { ...fixture.environment, OPENUSAGE_BOARD_FIXTURE_MODE: mode }, timeout: 20000, maxBuffer: 65536 });
}
function releaseHeld() { const callback = held; held = null; hold = null; assert.ok(callback); callback(); }
async function confirmDestructive(kind) {
  const previous = (await view()).operation?.id;
  await settings();
  if (kind === 'leave') await button('Leave the leaderboard');
  else {
    if (!await driver.execute('return document.querySelector("[data-leaderboard] .provider-advanced").open;')) await driver.click('[data-leaderboard] .provider-advanced > summary');
    await button(kind === 'forget' ? 'Clear local enrollment' : 'Leave with GitHub');
  }
  await waitFor(() => driver.execute('return Boolean(document.querySelector("[data-leaderboard] dialog[open][role=alertdialog]"));'), 'explicit departure confirmation');
  const dialog = '[data-leaderboard] dialog[open][role=alertdialog]';
  assert.equal(await driver.execute('return document.querySelector(arguments[0]+" button[type=submit]").disabled;', [dialog]), true);
  await driver.click(`${dialog} [role=checkbox]`);
  await driver.click(`${dialog} button[type=submit]`);
  let current;
  await waitFor(async () => { current = await view(); return Boolean(current.operation && current.operation.id !== previous); }, 'board operation admitted');
  return terminal(current.operation.id);
}
try {
  await mkdir(output, { recursive: true, mode: 0o700 });
  evidence.sourceCommit = (await run('git', ['rev-parse', 'HEAD'], { cwd: root, timeout: 10000 })).stdout.trim();
  evidence.dirtySource = (await run('git', ['status', '--porcelain'], { cwd: root, timeout: 10000 })).stdout.length > 0;
  evidence.applicationSha256 = digest(await readFile(application));
  evidence.seedSha256 = digest(await readFile(seed));
  fixture = await createFixture('settings-leaderboard');
  const home = join(fixture.root, 'home'); await mkdir(home, { mode: 0o700 });
  Object.assign(fixture.environment, { HOME: home, OPENUSAGE_PRIVATE_KEYRING_FIXTURE: fixture.root });
  await startSecretService(fixture);
  await seeded('gui');
  const certificate = join(fixture.root, 'board-ca.pem'), key = join(fixture.root, 'board-key.pem');
  await run('openssl', ['req', '-x509', '-newkey', 'rsa:2048', '-nodes', '-sha256', '-days', '1', '-subj', '/CN=127.0.0.1', '-addext', 'subjectAltName=IP:127.0.0.1', '-addext', 'basicConstraints=critical,CA:TRUE', '-keyout', key, '-out', certificate], { timeout: 15000 });
  server = createServer({ cert: await readFile(certificate), key: await readFile(key) }, async (request, response) => {
    try {
      const chunks = []; for await (const chunk of request) chunks.push(chunk);
      const body = Buffer.concat(chunks).toString(); requests.push(request.url);
      const reply = (status, value) => { response.writeHead(status, { 'content-type': 'application/json' }); response.end(JSON.stringify(value)); };
      const complete = async () => {
        assert.equal(request.method, 'POST');
        if (request.url === '/login/device/code') {
          assert.equal(body, 'client_id=synthetic-native-client');
          return reply(200, { device_code: deviceCode, user_code: 'ABCD-EFGH', verification_uri: 'https://github.com/login/device', expires_in: pendingOAuth ? 3 : 600, interval: 1 });
        }
        if (request.url === '/login/oauth/access_token') {
          assert.equal(new URLSearchParams(body).get('device_code'), deviceCode);
          return pendingOAuth ? reply(200, { error: 'authorization_pending' }) : reply(200, { access_token: selection === 'alpha' ? alpha : beta, token_type: 'bearer', scope: '' });
        }
        if (request.url === '/v1/join') {
          const input = JSON.parse(body); assert.deepEqual(Object.keys(input).sort(), ['deviceId', 'githubToken']); assert.equal(input.githubToken, alpha);
          joins.push(input.deviceId); token = (++tokenSequence).toString(16).padStart(64, '0'); issuedTokens.push(token); revoked = false;
          if (saveFailure) { saveFailure = false; await stopSecretService(fixture); }
          if (lostAck) { lostAck = false; response.destroy(); return; }
          return reply(200, { token, login: 'synthetic-user' });
        }
        if (request.url === '/v1/leave/github') {
          const input = JSON.parse(body); assert.deepEqual(Object.keys(input), ['githubToken']);
          assert.equal(input.githubToken, selection === 'alpha' ? alpha : beta);
          const proof = request.headers.authorization === undefined ? null : selection === 'alpha' && !revoked && request.headers.authorization === `Bearer ${token}`;
          if (selection === 'alpha') revoked = true;
          if (cleanupFailure) { cleanupFailure = false; await stopSecretService(fixture); }
          return reply(200, { left: true, login: 'synthetic-user', localTokenRevoked: proof });
        }
        assert.equal(request.headers.authorization, `Bearer ${token}`);
        if (revoked) return reply(401, { error: 'not_joined' });
        if (request.url === '/v1/sync') { uploads.push(body); return reply(200, { accepted: JSON.parse(body).rows.length }); }
        assert.equal(request.url, '/v1/leave');
        if (cleanupFailure) { cleanupFailure = false; await stopSecretService(fixture); }
        revoked = true; return reply(200, { left: true });
      };
      if (request.url === hold) { assert.equal(held, null); held = () => { void complete().catch(error => { evidence.stubError = error.message; response.destroy(); }); }; }
      else await complete();
    } catch (error) { evidence.stubError = error.message; response.destroy(); }
  });
  await new Promise(done => server.listen(0, '127.0.0.1', done));
  const origin = `https://127.0.0.1:${server.address().port}`;
  fixture.environment.OPENUSAGE_NATIVE_BOARD_ORIGIN = origin;
  await start();
  await driver.navigate('settings'); assert.equal(requests.length, 0);
  await settings(); assert.equal((await view()).enrollment, 'not_joined'); assert.equal(requests.length, 0);
  await rejected('load_leaderboard', { token: 'synthetic-only' }, 'invalid_arguments');
  await rejected('begin_leaderboard', { kind: 'join', confirmed: false }, 'consent_required');
  await rejected('begin_leaderboard', { kind: 'sync', days: 0, confirmed: true }, 'invalid_arguments');
  await rejected('begin_leaderboard', { kind: 'join', days: 1, confirmed: true }, 'invalid_arguments');
  await rejected('open_leaderboard_login_page', { operationId: '../other', url: 'https://outside.invalid' }, 'invalid_arguments');
  assert.equal(requests.length, 0);
  evidence.checks.push('Launch, local status, invalid input, and refused consent made no service request.');
  hold = '/login/oauth/access_token';
  await button('Join with GitHub');
  let current; await waitFor(async () => { current = await view(); return current.operation?.phase === 'authorizing'; }, 'public GitHub challenge');
  assert.equal(current.operation.userCode, 'ABCD-EFGH');
  await waitFor(() => driver.execute('const code=document.querySelector("[data-leaderboard] input[id^=github-code-]");return Boolean(code&&code.getClientRects().length&&code.value===arguments[0]);', ['ABCD-EFGH']), 'visible GitHub challenge');
  await capture('github-code.png');
  await waitFor(() => Boolean(held), 'initial OAuth reply held for challenge capture'); releaseHeld();
  current = await terminal(current.operation.id); assert.equal(current.operation.outcome.state, 'joined'); assert.equal(current.enrollment, 'joined'); assert.equal(joins.length, 1);
  await visibleText('Joined as @synthetic-user.'); await capture('joined.png');
  const firstDevice = joins[0];
  const joinReceipt = current.operation;
  guardHolder = spawn(seed, ['--ignored', '--exact', 'seed_private_cli_workspace'], { env: { ...fixture.environment, OPENUSAGE_BOARD_FIXTURE_MODE: 'hold_guard' }, stdio: 'ignore', detached: true });
  const guardExit = new Promise(resolve => guardHolder.once('exit', resolve));
  await waitFor(async () => { try { await readFile(join(fixture.root, 'board-guard-ready')); return true; } catch (error) { if (error.code === 'ENOENT') return false; throw error; } }, 'competing process guard');
  const beforeGuard = requests.length;
  current = await view(true); assert.equal(current.statusIssue, 'busy'); assert.equal(current.enrollment, 'unavailable'); assert.deepEqual(current.operation, joinReceipt);
  await rejected('delete_all_local_data', { confirmed: true }, 'local_deletion_busy');
  assert.equal(requests.length, beforeGuard);
  await writeFile(join(fixture.root, 'board-guard-release'), 'release'); await guardExit; guardHolder = null;
  current = await view(true); assert.equal(current.enrollment, 'joined'); assert.equal(current.statusIssue, undefined); assert.deepEqual(current.operation, joinReceipt);
  evidence.checks.push('A competing Rust process blocks status inspection and local deletion. The confirmed join receipt survives failed inspection and the next successful refresh.');
  await button('Preview upload');
  await waitFor(async () => { current = await view(); return current.operation?.phase === 'preview'; }, 'retained exact preview');
  const preview = current.operation.body; assert.equal(JSON.parse(preview).rows[0].totalTokens, 46); assert.equal(uploads.length, 0);
  await waitFor(() => driver.execute('const preview=document.querySelector("[data-leaderboard] dialog[open] [data-leaderboard-preview]");return Boolean(preview&&preview.getClientRects().length&&preview.textContent===arguments[0]);', [preview]), 'visible exact upload preview');
  await capture('upload-preview.png');
  await seeded('gui_update');
  await button('Upload', '[data-leaderboard] dialog[open]');
  current = await terminal(current.operation.id); assert.equal(current.operation.outcome.state, 'synced'); assert.deepEqual(uploads, [preview]);
  const syncId = await begin('sync', { days: 30 });
  await waitFor(async () => { current = await view(); return current.operation?.phase === 'preview'; }, 'next saved snapshot');
  assert.equal(JSON.parse(current.operation.body).rows[0].totalTokens, 92);
  await rejected('delete_all_local_data', { confirmed: true }, 'local_deletion_busy');
  await rejected('begin_leaderboard', { kind: 'forget', confirmed: true }, 'busy');
  await driver.invoke('cancel_leaderboard', { operationId: syncId }); await terminal(syncId);
  evidence.checks.push('Actual GUI join persisted a credential. Sync sent exactly the reviewed 46-token body after a concurrent saved-data change to 92. Cancelled preview made no upload.');
  hold = '/login/oauth/access_token'; const before = joins.length; const cancelId = await begin('reconnect');
  await waitFor(() => Boolean(held), 'delayed OAuth reply');
  await driver.invoke('cancel_leaderboard', { operationId: cancelId }); releaseHeld();
  current = await terminal(cancelId); assert.equal(current.operation.outcome.state, 'cancelled'); assert.equal(joins.length, before);
  hold = '/v1/join'; const settling = await begin('reconnect'); await waitFor(() => Boolean(held), 'reserved join request');
  const cancelled = await driver.invoke('cancel_leaderboard', { operationId: settling }); assert.equal(cancelled.value.operation.phase, 'settling'); assert.equal(cancelled.value.busy, true);
  releaseHeld(); current = await terminal(settling); assert.equal(current.operation.outcome.state, 'joined'); assert.equal(joins.at(-1), firstDevice);
  evidence.checks.push('Cancellation before dispatch discarded late OAuth authorization. Dispatch first settled and saved the real join result with the stable device ID.');
  selection = 'beta'; const recoveryJoins = joins.length; const foreign = await begin('recover_leave'); current = await terminal(foreign);
  assert.equal(current.operation.outcome.state, 'left'); assert.equal(current.operation.outcome.credentials, 'retained'); assert.equal(joins.length, recoveryJoins);
  selection = 'alpha'; assert.equal((await view(true)).enrollment, 'joined');
  saveFailure = true; const partial = await begin('reconnect'); current = await terminal(partial);
  assert.deepEqual(current.operation.outcome, { state: 'failed', issue: 'save_failed', remoteEffect: 'confirmed' });
  const retainedReceipt = await view(); assert.deepEqual(retainedReceipt.operation, current.operation);
  await startSecretService(fixture); const resumed = await begin('reconnect'); current = await terminal(resumed); assert.equal(current.operation.outcome.state, 'joined'); assert.equal(joins.at(-1), firstDevice);
  evidence.checks.push('Same-login foreign recovery retained the local account without joining. Confirmed remote join plus failed local saving kept its receipt and recovered the same device.');
  lostAck = true; const unknown = await begin('reconnect'); current = await terminal(unknown);
  assert.deepEqual(current.operation.outcome, { state: 'failed', issue: 'remote_unavailable', remoteEffect: 'unknown' });
  assert.equal(current.enrollment, 'joined');
  const reconcile = await begin('reconnect'); current = await terminal(reconcile); assert.equal(current.operation.outcome.state, 'joined'); assert.equal(joins.at(-1), firstDevice);
  pendingOAuth = true; const expiredJoins = joins.length; const expired = await begin('reconnect'); current = await terminal(expired); pendingOAuth = false;
  assert.deepEqual(current.operation.outcome, { state: 'failed', issue: 'authorization_expired', remoteEffect: 'none' }); assert.equal(joins.length, expiredJoins);
  evidence.checks.push('A lost join acknowledgment reports an unknown public result and retains enrollment. Explicit reconnect repairs the stable device. Pending OAuth expires without a join.');
  cleanupFailure = true; const cleanup = await begin('leave'); current = await terminal(cleanup);
  assert.deepEqual(current.operation.outcome, { state: 'failed', issue: 'local_cleanup_failed', remoteEffect: 'confirmed' }); assert.equal(current.enrollment, 'unavailable');
  const beforeUnavailableRecovery = joins.length;
  const withoutStore = await begin('recover_leave'); current = await terminal(withoutStore);
  assert.equal(current.operation.outcome.state, 'left'); assert.equal(current.operation.outcome.credentials, 'unavailable'); assert.equal(joins.length, beforeUnavailableRecovery);
  await startSecretService(fixture);
  const repaired = await begin('reconnect'); current = await terminal(repaired); assert.equal(current.operation.outcome.state, 'joined'); assert.equal(joins.at(-1), firstDevice);
  evidence.checks.push('Confirmed public departure plus failed cleanup reports a partial result. Fresh GitHub departure still works with no credential service, adds no enrollment slot, and reports local status unavailable.');
  current = await confirmDestructive('leave'); assert.equal(current.operation.outcome.state, 'left'); assert.equal(current.enrollment, 'not_joined');
  const joinAgain = await begin('join'); current = await terminal(joinAgain); assert.equal(current.operation.outcome.state, 'joined');
  await closeNormally(); delete fixture.environment.OPENUSAGE_NATIVE_BOARD_ORIGIN;
  const beforeForget = requests.length; await start(); await settings(); assert.equal((await view()).configured, false);
  current = await confirmDestructive('forget'); assert.equal(current.operation.outcome.state, 'forgotten'); assert.equal(requests.length, beforeForget);
  evidence.checks.push('The actual confirmations distinguish global public departure from local forgetting. An unconfigured restarted build clears the local slot without service requests.');
  await closeNormally(); fixture.environment.OPENUSAGE_NATIVE_BOARD_ORIGIN = origin; await start(); await settings();
  const localJoin = await begin('join'); current = await terminal(localJoin); assert.equal(current.operation.outcome.state, 'joined');
  const beforeDelete = requests.length;
  const deletion = await driver.invoke('delete_all_local_data', { confirmed: true }); assert.equal(deletion.ok, true, JSON.stringify(deletion)); assert.equal(deletion.value.storage, 'setup_required');
  assert.equal(requests.length, beforeDelete);
  const setup = await driver.invoke('setup_workspace', { passphrase: null }); assert.equal(setup.ok, true, JSON.stringify(setup));
  assert.equal((await driver.invoke('save_local_log_consent', { tools: [] })).ok, true);
  const fresh = await view(); assert.equal(fresh.enrollment, 'not_joined'); assert.equal(fresh.login, null); assert.equal(fresh.operation, null); assert.equal(await readlink(`/proc/${appPid}/exe`), application);
  evidence.checks.push('Local deletion followed by fresh setup in the same Rust process clears the cached login, receipt, and credential without public departure.');
  hold = '/login/oauth/access_token'; const preQuitJoins = joins.length; await begin('join'); await waitFor(() => Boolean(held), 'OAuth held at Quit');
  await closeNormally(async () => { releaseHeld(); }); assert.equal(joins.length, preQuitJoins);
  await start(); await settings(); assert.equal((await view()).enrollment, 'pending');
  evidence.checks.push('A genuine native close before public dispatch discards the late OAuth reply and preserves the pending device for explicit recovery.');
  const quitBefore = joins.length; hold = '/v1/join'; const quitting = await begin('join'); await waitFor(() => Boolean(held), 'join held at Quit');
  await closeNormally(async () => { releaseHeld(); }); assert.equal(joins.length, quitBefore + 1);
  await start(); await settings(); current = await view(); assert.equal(current.enrollment, 'joined');
  evidence.checks.push('A genuine native close during a reserved join settled HTTP and saved the credential before exit. Restart reads the completed enrollment.');
  const text = JSON.stringify(publicViews) + await driver.execute('return document.documentElement.outerHTML;');
  assert.equal(new Set(issuedTokens).size, issuedTokens.length);
  for (const secret of [alpha, beta, deviceCode, ...issuedTokens]) assert.equal(text.includes(secret), false);
  assert.equal(evidence.stubError, undefined);
  evidence.previewSha256 = digest(Buffer.from(preview)); evidence.uploadSha256 = digest(Buffer.from(uploads[0]));
  await capture('final.png'); await closeNormally();
  assert.equal(digest(await readFile(application)), evidence.applicationSha256);
  evidence.result = 'passed';
} catch (error) {
  evidence.error = error.stack ?? String(error); process.exitCode = 1;
  const last = publicViews.at(-1);
  if (last) evidence.lastPublicState = {
    configured: last.configured, enrollment: last.enrollment, busy: last.busy, statusIssue: last.statusIssue,
    operation: last.operation && { kind: last.operation.kind, phase: last.operation.phase, outcome: last.operation.outcome },
  };
  if (driver?.session) try { await driver.screenshot(join(output, 'failure.png')); evidence.screenshots.push('failure.png'); } catch {}
} finally {
  if (held) releaseHeld();
  try { await driver?.stop(); } catch (error) { evidence.cleanupError = error.message; evidence.result = 'failed'; process.exitCode = 1; }
  if (guardHolder) stopGroup(guardHolder, 'SIGKILL');
  if (server) { server.closeAllConnections(); await new Promise(done => server.close(done)); }
  if (fixture) { await stopSecretService(fixture); stopGroup(fixture.bus, 'SIGKILL'); await rm(fixture.root, { recursive: true, force: true }); }
  evidence.requestCounts = Object.fromEntries([...new Set(requests)].map(path => [path, requests.filter(value => value === path).length]));
  evidence.finishedAt = new Date().toISOString();
  await writeFile(join(output, 'result.json'), JSON.stringify(evidence, null, 2) + '\n', { mode: 0o600 });
  console.log(`Native Settings leaderboard: ${evidence.result}. Evidence: ${output}`);
  if (evidence.error) console.error(evidence.error);
}
