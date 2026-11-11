// Runs the Worker against SQLite through a small D1 stand-in. No network: GitHub is a stub.
// Run with: node --test services/leaderboard/test/
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import { DatabaseSync } from 'node:sqlite';
import { DAY_CEILING, TITLE, handle, validRows } from '../src/worker.mjs';

/** The part of the D1 binding that the Worker uses: prepare, bind, first, all, run and batch. */
function d1({ beforeRun, afterRun } = {}) {
  const db = new DatabaseSync(':memory:');
  db.exec('PRAGMA foreign_keys = ON');
  db.exec(readFileSync(new URL('../migrations/0001_init.sql', import.meta.url), 'utf8'));
  const statement = (sql, params = []) => ({
    bind: (...values) => statement(sql, values),
    first: async () => db.prepare(sql).get(...params) ?? null,
    all: async () => ({ results: db.prepare(sql).all(...params) }),
    run: async () => {
      await beforeRun?.(sql, params);
      const result = { success: true, meta: { changes: Number(db.prepare(sql).run(...params).changes) } };
      return afterRun ? afterRun(sql, result) : result;
    },
    runSync: () => db.prepare(sql).run(...params),
  });
  return {
    raw: db,
    prepare: sql => statement(sql),
    async batch(statements) {
      db.exec('BEGIN');
      try { for (const s of statements) s.runSync(); db.exec('COMMIT'); } catch (error) { db.exec('ROLLBACK'); throw error; }
    },
  };
}

const NOW = new Date('2026-11-08T12:00:00Z');
const DEVICE = '11111111-1111-4111-8111-111111111111';
const SECOND = '22222222-2222-4222-8222-222222222222';
const accounts = { 'gho_synthetic_token_alpha_0001': { id: 101, login: 'alpha' }, 'gho_synthetic_token_beta_00001': { id: 202, login: 'beta' } };
const deps = { fetchGitHubUser: async token => accounts[token] ?? null };
function env(hooks) {
  const allowed = { count: 0 };
  return { DB: d1(hooks), JOIN_LIMITER: { limit: async () => ({ success: ++allowed.count <= 50 }) }, allowed };
}
const post = (path, body, token, extra = {}) => new Request(`https://board.example${path}`, {
  method: 'POST', body: typeof body === 'string' ? body : JSON.stringify(body),
  headers: { 'content-type': 'application/json', 'cf-connecting-ip': '203.0.113.9', ...(token ? { authorization: `Bearer ${token}` } : {}), ...extra },
});
const get = path => new Request(`https://board.example${path}`);
const row = (date, total, extra = {}) => ({ date, tool: 'claude', model: 'claude-sonnet-4-5', inputTokens: total - 3, outputTokens: 1, cacheReadTokens: 1, cacheWriteTokens: 1, totalTokens: total, estimatedValueMicros: 900, ...extra });
async function join(e, githubToken, deviceId = DEVICE, now = NOW) {
  const response = await handle(post('/v1/join', { githubToken, deviceId }), e, deps, now);
  return { status: response.status, body: await response.json() };
}
async function sync(e, token, rows, deviceId = DEVICE, now = NOW) {
  const response = await handle(post('/v1/sync', { deviceId, rows }, token), e, deps, now);
  return { status: response.status, body: await response.json() };
}
async function board(e, query = '') {
  const response = await handle(get(`/v1/leaderboard${query}`), e, deps, NOW);
  return { status: response.status, body: await response.json() };
}

const BODY_CAP = 64 * 1024;
const encode = text => new TextEncoder().encode(text);
function paddedJson(input, size = BODY_CAP) {
  const text = JSON.stringify(input);
  return encode(text + ' '.repeat(size - encode(text).length));
}

function streamedPost(path, chunks, { token, headers = {}, cancel, failAt } = {}) {
  const stats = { pulls: 0, bytes: 0, cancels: 0 };
  const stream = new ReadableStream({
    pull(controller) {
      const index = stats.pulls++;
      if (index === failAt) return controller.error(new Error('synthetic stream failure'));
      if (index === chunks.length) return controller.close();
      stats.bytes += chunks[index].byteLength;
      controller.enqueue(chunks[index]);
    },
    cancel() { stats.cancels++; return cancel?.(); },
  }, { highWaterMark: 0 });
  return {
    stats,
    request: new Request(`https://board.example${path}`, {
      method: 'POST', body: stream, duplex: 'half',
      headers: { 'content-type': 'application/json', ...(token ? { authorization: `Bearer ${token}` } : {}), ...headers },
    }),
  };
}

async function bodyTarget(path) {
  const e = env();
  if (path === '/v1/join') return { e, input: { githubToken: 'gho_synthetic_token_alpha_0001', deviceId: DEVICE } };
  const { token } = (await join(e, 'gho_synthetic_token_alpha_0001')).body;
  return { e, token, input: { deviceId: DEVICE, rows: [row('2026-11-08', 10)] } };
}

test('join and sync accept valid JSON at the exact byte cap', async () => {
  for (const path of ['/v1/join', '/v1/sync']) {
    for (const declared of [undefined, '1', String(BODY_CAP)]) {
      const { e, token, input } = await bodyTarget(path);
      const bytes = paddedJson(input);
      const { request, stats } = streamedPost(path, [bytes.subarray(0, 123), bytes.subarray(123)], {
        token, headers: declared === undefined ? {} : { 'content-length': declared },
      });
      const response = await handle(request, e, deps, NOW);
      assert.equal(response.status, 200, `${path}, declared ${declared}`);
      const result = await response.json();
      if (path === '/v1/join') assert.match(result.token, /^[0-9a-f]{64}$/);
      else assert.deepEqual(result, { accepted: 1 });
      assert.deepEqual(stats, { pulls: 3, bytes: BODY_CAP, cancels: 0 });
    }
  }
});

test('join and sync cancel at one byte over the cap despite missing or smaller headers', async () => {
  for (const path of ['/v1/join', '/v1/sync']) {
    for (const declared of [undefined, '1', String(BODY_CAP)]) {
      const { e, token, input } = await bodyTarget(path);
      const { request, stats } = streamedPost(path, [paddedJson(input), encode(' '), new Uint8Array(BODY_CAP)], {
        token, headers: declared === undefined ? {} : { 'content-length': declared },
      });
      const response = await handle(request, e, deps, NOW);
      assert.equal(response.status, 413);
      assert.deepEqual(await response.json(), { error: 'too_large' });
      assert.deepEqual(stats, { pulls: 2, bytes: BODY_CAP + 1, cancels: 1 });
      assert.equal(e.DB.raw.prepare('SELECT count(*) AS n FROM days').get().n, 0);
      assert.equal(e.DB.raw.prepare('SELECT count(*) AS n FROM devices').get().n, token ? 1 : 0);
    }
  }
});

test('overflow cancels an oversized first chunk and stops an ongoing stream', async () => {
  for (const path of ['/v1/join', '/v1/sync']) {
    for (const chunkSize of [1024 * 1024, 16 * 1024]) {
      const { e, token } = await bodyTarget(path);
      const chunk = new Uint8Array(chunkSize);
      const { request, stats } = streamedPost(path, Array(chunkSize > BODY_CAP ? 2 : 64).fill(chunk), { token });
      const response = await handle(request, e, deps, NOW);
      assert.equal(response.status, 413);
      assert.deepEqual(await response.json(), { error: 'too_large' });
      const pulls = Math.floor(BODY_CAP / chunkSize) + 1;
      assert.deepEqual(stats, { pulls, bytes: pulls * chunkSize, cancels: 1 });
    }
  }
});

test('a declared length above the cap rejects before the first body read', async () => {
  for (const path of ['/v1/join', '/v1/sync']) {
    const { e, token, input } = await bodyTarget(path);
    const { request, stats } = streamedPost(path, [encode(JSON.stringify(input))], {
      token, headers: { 'content-length': String(BODY_CAP + 1) },
    });
    const response = await handle(request, e, deps, NOW);
    assert.equal(response.status, 413);
    assert.deepEqual(await response.json(), { error: 'too_large' });
    assert.deepEqual(stats, { pulls: 0, bytes: 0, cancels: 0 });
  }
});

test('the body cap counts UTF-8 bytes and permits a split character', async () => {
  const bytes = paddedJson({ unexpected: 'é'.repeat(30_000) });
  const split = bytes.indexOf(0xc3) + 1;
  for (const path of ['/v1/join', '/v1/sync']) {
    for (const excess of [0, 1]) {
      const { e, token } = await bodyTarget(path);
      const chunks = [bytes.subarray(0, split), bytes.subarray(split)];
      if (excess) chunks.push(encode(' '));
      const { request, stats } = streamedPost(path, chunks, { token });
      const response = await handle(request, e, deps, NOW);
      assert.equal(response.status, excess ? 413 : 400);
      assert.deepEqual(await response.json(), { error: excess ? 'too_large' : 'invalid_request' });
      assert.equal(stats.bytes, BODY_CAP + excess);
      assert.equal(stats.cancels, excess);
    }
  }
});

test('invalid JSON and invalid UTF-8 remain invalid_json errors', async () => {
  const invalid = [null, [], [encode('{')], [new Uint8Array([0xc3])], [encode('{"unexpected":"'), new Uint8Array([0xc3, 0x28]), encode('"}')]];
  for (const path of ['/v1/join', '/v1/sync']) {
    for (const chunks of invalid) {
      const { e, token } = await bodyTarget(path);
      const request = chunks === null ? post(path, undefined, token) : streamedPost(path, chunks, { token }).request;
      const response = await handle(request, e, deps, NOW);
      assert.equal(response.status, 400);
      assert.deepEqual(await response.json(), { error: 'invalid_json' });
    }
  }
});

test('stream errors never accept a partial or complete JSON body', async () => {
  for (const path of ['/v1/join', '/v1/sync']) {
    for (const failAt of [0, 1, 2]) {
      const { e, token, input } = await bodyTarget(path);
      const bytes = encode(JSON.stringify(input));
      const { request } = streamedPost(path, [bytes.subarray(0, 5), bytes.subarray(5)], { token, failAt });
      const response = await handle(request, e, deps, NOW);
      assert.equal(response.status, 500);
      assert.deepEqual(await response.json(), { error: 'unavailable' });
      assert.equal(e.DB.raw.prepare('SELECT count(*) AS n FROM days').get().n, 0);
      assert.equal(e.DB.raw.prepare('SELECT count(*) AS n FROM devices').get().n, token ? 1 : 0);
    }
  }
});

test('non-byte chunks fail closed and cancel the stream', async () => {
  for (const path of ['/v1/join', '/v1/sync']) {
    const { e, token } = await bodyTarget(path);
    const { request, stats } = streamedPost(path, [new Uint16Array([0x7b, 0x7d]), encode(' ')], { token });
    const response = await handle(request, e, deps, NOW);
    assert.equal(response.status, 500);
    assert.deepEqual(await response.json(), { error: 'unavailable' });
    assert.deepEqual(stats, { pulls: 1, bytes: 4, cancels: 1 });
  }
});

test('failed or stalled cancellation cannot delay the overflow response', { timeout: 5000 }, async () => {
  for (const path of ['/v1/join', '/v1/sync']) {
    for (const cancel of [
      () => { throw new Error('synthetic cancellation failure'); },
      () => Promise.reject(new Error('synthetic cancellation rejection')),
      () => new Promise(() => {}),
    ]) {
      const { e, token } = await bodyTarget(path);
      const { request, stats } = streamedPost(path, [new Uint8Array(BODY_CAP + 1)], { token, cancel });
      const response = await handle(request, e, deps, NOW);
      assert.equal(response.status, 413);
      assert.deepEqual(await response.json(), { error: 'too_large' });
      assert.deepEqual(stats, { pulls: 1, bytes: BODY_CAP + 1, cancels: 1 });
      await new Promise(resolve => setImmediate(resolve));
    }
  }
});

test('authentication and rate limits still reject before body reads', async () => {
  for (const [path, mode, status, error] of [
    ['/v1/join', 'missing-limiter', 503, 'limiter_unavailable'],
    ['/v1/join', 'edge-limit', 429, 'rate_limited'],
    ['/v1/sync', 'missing-token', 401, 'not_joined'],
    ['/v1/sync', 'hour-limit', 429, 'rate_limited'],
  ]) {
    const { e, token, input } = await bodyTarget(path);
    if (mode === 'missing-limiter') delete e.JOIN_LIMITER;
    if (mode === 'edge-limit') e.JOIN_LIMITER.limit = async () => ({ success: false });
    if (mode === 'hour-limit') e.DB.raw.prepare('INSERT INTO limits VALUES (?1, ?2, ?3)').run('sync:1', '2026-11-08T12', 30);
    const { request, stats } = streamedPost(path, [encode(JSON.stringify(input))], { token: mode === 'missing-token' ? undefined : token });
    const response = await handle(request, e, deps, NOW);
    assert.equal(response.status, status);
    assert.deepEqual(await response.json(), { error });
    assert.deepEqual(stats, { pulls: 0, bytes: 0, cancels: 0 });
  }
});

// Hold all writes until every concurrent request reaches the mutation.
// This also forces separate reads in the old code to see the same count.
function simultaneousWrites(pattern, participants) {
  const ready = Promise.withResolvers();
  let arrivals = 0;
  return async sql => {
    if (!pattern.test(sql) || arrivals >= participants) return;
    if (++arrivals === participants) ready.resolve();
    await ready.promise;
  };
}

test('concurrent joins obey the account limit for new and existing counters', { timeout: 5000 }, async () => {
  for (const initial of [0, 2]) {
    const e = env({ beforeRun: simultaneousWrites(/^INSERT INTO limits/, 8) });
    if (initial) e.DB.raw.prepare('INSERT INTO limits VALUES (?1, ?2, ?3)').run('join:101', '2026-11-08T12', initial);
    const responses = await Promise.all(Array.from({ length: 8 }, () => join(e, 'gho_synthetic_token_alpha_0001')));
    assert.equal(responses.filter(x => x.status === 200).length, 3 - initial);
    for (const refused of responses.filter(x => x.status !== 200)) {
      assert.deepEqual(refused, { status: 429, body: { error: 'rate_limited' } });
    }
    assert.equal(e.DB.raw.prepare('SELECT count FROM limits WHERE key = ?1 AND hour = ?2').get('join:101', '2026-11-08T12').count, 3);
    assert.equal((await join(e, 'gho_synthetic_token_alpha_0001')).status, 429);
    assert.equal((await join(e, 'gho_synthetic_token_beta_00001')).status, 200);
    assert.equal((await join(e, 'gho_synthetic_token_alpha_0001', DEVICE, new Date('2026-11-08T13:00:00Z'))).status, 200);
    assert.equal(e.DB.raw.prepare('SELECT count FROM limits WHERE key = ?1 AND hour = ?2').get('join:101', '2026-11-08T13').count, 1);
  }
});

test('concurrent syncs share the user limit across devices', { timeout: 5000 }, async () => {
  for (const initial of [0, 29]) {
    let armed = false;
    const barrier = simultaneousWrites(/^INSERT INTO limits/, 35);
    const e = env({ beforeRun: sql => armed ? barrier(sql) : undefined });
    const first = (await join(e, 'gho_synthetic_token_alpha_0001')).body.token;
    const second = (await join(e, 'gho_synthetic_token_alpha_0001', SECOND)).body.token;
    const beta = (await join(e, 'gho_synthetic_token_beta_00001')).body.token;
    if (initial) e.DB.raw.prepare('INSERT INTO limits VALUES (?1, ?2, ?3)').run('sync:1', '2026-11-08T12', initial);
    armed = true;
    const responses = await Promise.all(Array.from({ length: 35 }, (_, n) => sync(e, n % 2 ? first : second, [], n % 2 ? DEVICE : SECOND)));
    assert.equal(responses.filter(x => x.status === 200).length, 30 - initial);
    for (const refused of responses.filter(x => x.status !== 200)) {
      assert.deepEqual(refused, { status: 429, body: { error: 'rate_limited' } });
    }
    assert.equal(e.DB.raw.prepare('SELECT count FROM limits WHERE key = ?1 AND hour = ?2').get('sync:1', '2026-11-08T12').count, 30);
    assert.equal((await sync(e, first, [])).status, 429);
    assert.equal((await sync(e, beta, [])).status, 200);
    assert.equal((await sync(e, first, [], DEVICE, new Date('2026-11-08T13:00:00Z'))).status, 200);
    assert.equal(e.DB.raw.prepare('SELECT count FROM limits WHERE key = ?1 AND hour = ?2').get('sync:1', '2026-11-08T13').count, 1);
  }
});

test('concurrent device joins cannot exceed the cap and known devices can rejoin', { timeout: 5000 }, async () => {
  let armed = false;
  const barrier = simultaneousWrites(/^INSERT INTO devices/, 2);
  const e = env({ beforeRun: sql => armed ? barrier(sql) : undefined });
  const original = (await join(e, 'gho_synthetic_token_alpha_0001')).body.token;
  for (let n = 0; n < 3; n++) {
    e.DB.raw.prepare('INSERT INTO devices VALUES (1, ?1, ?2, ?3)').run(`4444444${n}-4444-4444-8444-444444444444`, String(n).repeat(64), NOW.toISOString());
  }
  armed = true;
  const candidates = [SECOND, '55555555-5555-4555-8555-555555555555'];
  const responses = await Promise.all(candidates.map(id => join(e, 'gho_synthetic_token_alpha_0001', id)));
  assert.deepEqual(responses.map(x => x.status).sort(), [200, 409]);
  assert.deepEqual(responses.find(x => x.status === 409).body, { error: 'too_many_devices' });
  assert.equal(e.DB.raw.prepare('SELECT count(*) AS n FROM devices WHERE user_id = 1').get().n, 5);
  const winner = responses.findIndex(x => x.status === 200);
  assert.equal((await sync(e, responses[winner].body.token, [], candidates[winner])).status, 200);
  assert.equal((await sync(e, original, [])).status, 200);
  const nextHour = new Date('2026-11-08T13:00:00Z');
  const rejoined = await join(e, 'gho_synthetic_token_alpha_0001', DEVICE, nextHour);
  assert.equal(rejoined.status, 200);
  assert.notEqual(rejoined.body.token, original);
  assert.equal((await sync(e, original, [])).status, 401);
  assert.equal((await sync(e, rejoined.body.token, [])).status, 200);
  assert.equal(e.DB.raw.prepare('SELECT created_at FROM devices WHERE device_id = ?1').get(DEVICE).created_at, NOW.toISOString());
  assert.equal((await join(e, 'gho_synthetic_token_alpha_0001', candidates[1 - winner], nextHour)).status, 409);
  assert.equal(e.DB.raw.prepare('SELECT count(*) AS n FROM devices WHERE user_id = 1').get().n, 5);
});

test('concurrent joins for one new device preserve one row and rotate the token', { timeout: 5000 }, async () => {
  const e = env({ beforeRun: simultaneousWrites(/^INSERT INTO devices/, 2) });
  const responses = await Promise.all(Array.from({ length: 2 }, () => join(e, 'gho_synthetic_token_alpha_0001')));
  assert.deepEqual(responses.map(x => x.status), [200, 200]);
  assert.equal(e.DB.raw.prepare('SELECT count(*) AS n FROM devices').get().n, 1);
  const tokens = responses.map(x => x.body.token);
  assert.notEqual(tokens[0], tokens[1]);
  const statuses = await Promise.all(tokens.map(async token => (await sync(e, token, [])).status));
  assert.deepEqual(statuses.sort(), [200, 401]);
});

test('database failures do not issue tokens or accept syncs', async () => {
  for (const target of ['join-limit', 'sync-limit', 'device']) {
    let armed = false;
    const pattern = target === 'device' ? /^INSERT INTO devices/ : /^INSERT INTO limits/;
    const e = env({ beforeRun: sql => { if (armed && pattern.test(sql)) throw new Error('synthetic database failure'); } });
    const original = (await join(e, 'gho_synthetic_token_alpha_0001')).body.token;
    const before = e.DB.raw.prepare('SELECT token_hash FROM devices').get().token_hash;
    armed = true;
    const response = target === 'sync-limit'
      ? await sync(e, original, [row('2026-11-08', 10)])
      : await join(e, 'gho_synthetic_token_alpha_0001');
    assert.deepEqual(response, { status: 500, body: { error: 'unavailable' } });
    assert.equal(e.DB.raw.prepare('SELECT token_hash FROM devices').get().token_hash, before);
    assert.equal(e.DB.raw.prepare('SELECT count(*) AS n FROM days').get().n, 0);
  }
});

test('invalid write results do not issue tokens or accept syncs', async () => {
  for (const result of [{ success: false, meta: { changes: 1 } }, { success: true }, { success: true, meta: { changes: 2 } }]) {
    for (const target of ['join-limit', 'sync-limit', 'device']) {
      let armed = false;
      const pattern = target === 'device' ? /^INSERT INTO devices/ : /^INSERT INTO limits/;
      const e = env({ afterRun: (sql, actual) => armed && pattern.test(sql) ? result : actual });
      const original = (await join(e, 'gho_synthetic_token_alpha_0001')).body.token;
      armed = true;
      const response = target === 'sync-limit'
        ? await sync(e, original, [row('2026-11-08', 10)])
        : await join(e, 'gho_synthetic_token_alpha_0001');
      assert.deepEqual(response, { status: 500, body: { error: 'unavailable' } });
      assert.equal(e.DB.raw.prepare('SELECT count(*) AS n FROM days').get().n, 0);
    }
  }
});

test('joining stores only a token hash and the GitHub login, never the GitHub token or an IP', async () => {
  const e = env();
  const joined = await join(e, 'gho_synthetic_token_alpha_0001');
  assert.equal(joined.status, 200);
  assert.match(joined.body.token, /^[0-9a-f]{64}$/);
  assert.equal(joined.body.login, 'alpha');
  const dump = JSON.stringify(e.DB.raw.prepare("SELECT * FROM users, devices").all());
  for (const secret of ['gho_synthetic_token_alpha_0001', joined.body.token, '203.0.113.9']) assert.ok(!dump.includes(secret), secret);
  assert.equal((await join(e, 'gho_unknown_token_for_nobody')).body.error, 'github_refused');
  for (const bad of [{ githubToken: 'short', deviceId: DEVICE }, { githubToken: 'gho_synthetic_token_alpha_0001', deviceId: 'not-a-uuid' }, { githubToken: 'gho_synthetic_token_alpha_0001', deviceId: DEVICE, extra: 1 }]) {
    assert.equal((await handle(post('/v1/join', bad), e, deps, NOW)).status, 400);
  }
  // The edge limiter and the hourly limit per GitHub account both refuse.
  const blocked = { DB: e.DB, JOIN_LIMITER: { limit: async () => ({ success: false }) } };
  assert.equal((await handle(post('/v1/join', { githubToken: 'gho_synthetic_token_beta_00001', deviceId: DEVICE }), blocked, deps, NOW)).status, 429);
  assert.equal((await handle(post('/v1/join', { githubToken: 'gho_synthetic_token_beta_00001', deviceId: DEVICE }), { DB: e.DB }, deps, NOW)).status, 503);
  for (let n = 0; n < 2; n++) assert.equal((await join(e, 'gho_synthetic_token_alpha_0001')).status, 200);
  assert.equal((await join(e, 'gho_synthetic_token_alpha_0001')).status, 429);
});

test('a sync accepts only permitted fields and replaces the same device, date, tool and model', async () => {
  const e = env();
  const { token } = (await join(e, 'gho_synthetic_token_alpha_0001')).body;
  const rows = [row('2026-11-07', 1000), row('2026-11-08', 2000, { tool: 'codex', model: 'gpt-5-codex', estimatedValueMicros: null })];
  assert.deepEqual(await sync(e, token, rows), { status: 200, body: { accepted: 2 } });
  // A repeated sync replaces the same device, date, tool and model (OU-BOARD-009).
  assert.equal((await sync(e, token, [row('2026-11-07', 1500)])).status, 200);
  assert.equal(e.DB.raw.prepare('SELECT sum(total_tokens) AS t FROM days').get().t, 3500);
  const refused = [
    [row('2026-11-07', 10, { prompt: 'text' })],
    [row('2026-11-07', 10, { totalTokens: 11 })],
    [row('2026-11-07', 10), row('2026-11-07', 20)],
    [row('2026-11-09', 10)],
    [row('2025-10-01', 10)],
    [row('2026-02-30', 10)],
    [row('2026-11-07', 10, { tool: 'cursor' })],
    [row('2026-11-07', 10, { model: '/home/dev/project' })],
    [row('2026-11-07', 10, { model: 'Claude <b>' })],
    [row('2026-11-07', 10, { inputTokens: -1, totalTokens: 9 + 3 - 3 })],
  ];
  for (const bad of refused) assert.equal((await sync(e, token, bad)).status, 400, JSON.stringify(bad[0]));
  assert.equal((await sync(e, token, rows, SECOND)).status, 400, 'The device must match the token.');
  assert.equal((await sync(e, 'f'.repeat(64), rows)).status, 401);
  assert.equal((await handle(post('/v1/sync', 'x'.repeat(70_000), token), e, deps, NOW)).status, 413);
  assert.equal(validRows(rows, NOW), true);
  // 401 rows do not fit in 64 KiB, so the row limit is checked on its own.
  assert.equal(validRows(Array.from({ length: 401 }, (_, n) => row('2026-11-07', 10, { model: `m${n}` })), NOW), false);
  assert.equal(e.DB.raw.prepare('SELECT sum(total_tokens) AS t FROM days').get().t, 3500);
});

test('ranks sum devices, leave hidden users and flagged days out, and say Self-reported', async () => {
  const e = env();
  const alpha = (await join(e, 'gho_synthetic_token_alpha_0001')).body.token;
  const alphaPhone = (await join(e, 'gho_synthetic_token_alpha_0001', SECOND)).body.token;
  const beta = (await join(e, 'gho_synthetic_token_beta_00001')).body.token;
  await sync(e, alpha, [row('2026-11-08', 3000)]);
  await sync(e, alphaPhone, [row('2026-11-08', 2000)], SECOND);
  await sync(e, beta, [row('2026-11-01', 4000, { estimatedValueMicros: 99_000 })]);
  let result = await board(e, '?period=30d&metric=tokens');
  assert.equal(result.body.title, TITLE);
  assert.deepEqual(result.body.entries.map(x => [x.rank, x.login, x.totalTokens]), [[1, 'alpha', 5000], [2, 'beta', 4000]]);
  assert.deepEqual((await board(e, '?period=7d')).body.entries.map(x => x.login), ['alpha']);
  assert.deepEqual((await board(e, '?period=all&metric=value')).body.entries.map(x => x.login), ['beta', 'alpha']);
  // An implausible day is flagged and leaves the ranks until a maintainer clears it.
  const huge = DAY_CEILING + 1;
  await sync(e, beta, [row('2026-11-02', huge)]);
  assert.deepEqual((await board(e, '?period=all')).body.entries.map(x => [x.login, x.totalTokens]), [['alpha', 5000], ['beta', 4000]]);
  e.DB.raw.exec('UPDATE flags SET cleared = 1');
  assert.equal((await board(e, '?period=all')).body.entries[0].login, 'beta');
  e.DB.raw.exec("UPDATE users SET hidden = 1 WHERE login = 'beta'");
  assert.deepEqual((await board(e, '?period=all')).body.entries.map(x => x.login), ['alpha']);
  for (const query of ['?period=1y', '?metric=money', '?period=7d&user=alpha']) assert.equal((await board(e, query)).status, 400);
});

test('ranks keep an estimate unavailable if any included row lacks it', async () => {
  for (const [estimates, expected] of [
    [[900, null], null],
    [[null, null], null],
    [[0, 0], 0],
    [[900, 300], 1200],
  ]) {
    const e = env();
    const { token } = (await join(e, 'gho_synthetic_token_alpha_0001')).body;
    assert.deepEqual(await sync(e, token, [
      row('2026-11-07', 10, { estimatedValueMicros: estimates[0] }),
      row('2026-11-08', 10, { estimatedValueMicros: estimates[1] }),
    ]), { status: 200, body: { accepted: 2 } });
    for (const period of ['7d', '30d', 'all']) {
      for (const metric of ['tokens', 'value']) {
        const result = await board(e, `?period=${period}&metric=${metric}`);
        assert.equal(result.status, 200);
        assert.deepEqual(result.body.entries, [{ rank: 1, login: 'alpha', totalTokens: 20, estimatedValueMicros: expected }]);
      }
    }
  }
});

test('ranks preserve device estimates, measured zero, and stable ties after replacement', async () => {
  const e = env();
  const alpha = (await join(e, 'gho_synthetic_token_alpha_0001')).body.token;
  const alphaPhone = (await join(e, 'gho_synthetic_token_alpha_0001', SECOND)).body.token;
  const beta = (await join(e, 'gho_synthetic_token_beta_00001')).body.token;
  assert.equal((await sync(e, alpha, [row('2026-11-08', 10)])).status, 200);
  assert.equal((await sync(e, alphaPhone, [row('2026-11-08', 10, { estimatedValueMicros: null })], SECOND)).status, 200);
  assert.equal((await sync(e, beta, [row('2026-11-08', 20, { estimatedValueMicros: 0 })])).status, 200);
  const entries = async metric => {
    const result = await board(e, `?period=all&metric=${metric}`);
    assert.equal(result.status, 200);
    return result.body.entries.map(x => [x.rank, x.login, x.totalTokens, x.estimatedValueMicros]);
  };
  for (const metric of ['tokens', 'value']) {
    assert.deepEqual(await entries(metric), [[1, 'beta', 20, 0], [2, 'alpha', 20, null]]);
  }
  assert.equal((await sync(e, beta, [row('2026-11-08', 20, { estimatedValueMicros: null })])).status, 200);
  for (const metric of ['tokens', 'value']) {
    assert.deepEqual(await entries(metric), [[1, 'alpha', 20, null], [2, 'beta', 20, null]]);
  }
  assert.equal((await sync(e, beta, [row('2026-11-08', 30, { estimatedValueMicros: null })])).status, 200);
  for (const metric of ['tokens', 'value']) {
    assert.deepEqual(await entries(metric), [[1, 'beta', 30, null], [2, 'alpha', 20, null]]);
  }
  assert.equal((await sync(e, alphaPhone, [row('2026-11-08', 10, { estimatedValueMicros: 0 })], SECOND)).status, 200);
  assert.deepEqual(await entries('value'), [[1, 'alpha', 20, 900], [2, 'beta', 30, null]]);
  assert.deepEqual(await entries('tokens'), [[1, 'beta', 30, null], [2, 'alpha', 20, 900]]);
  assert.equal((await sync(e, beta, [row('2026-11-08', 20)])).status, 200);
  for (const metric of ['tokens', 'value']) {
    assert.deepEqual(await entries(metric), [[1, 'alpha', 20, 900], [2, 'beta', 20, 900]]);
  }
});

test('only included dates and visible unflagged rows affect estimate availability', async () => {
  const e = env();
  const { token } = (await join(e, 'gho_synthetic_token_alpha_0001')).body;
  assert.equal((await sync(e, token, [
    row('2026-11-08', 10),
    row('2026-11-01', 10, { estimatedValueMicros: null }),
    row('2026-10-10', 10, { estimatedValueMicros: 0 }),
    row('2026-10-09', 10, { estimatedValueMicros: null }),
    row('2026-11-02', DAY_CEILING + 1, { estimatedValueMicros: null }),
  ])).status, 200);
  for (const [period, totalTokens, estimatedValueMicros] of [['7d', 10, 900], ['30d', 30, null], ['all', 40, null]]) {
    for (const metric of ['tokens', 'value']) {
      const result = await board(e, `?period=${period}&metric=${metric}`);
      assert.equal(result.status, 200);
      assert.deepEqual(result.body.entries, [{ rank: 1, login: 'alpha', totalTokens, estimatedValueMicros }]);
    }
  }
  e.DB.raw.exec('UPDATE flags SET cleared = 1');
  const cleared = await board(e, '?period=7d&metric=value');
  assert.equal(cleared.status, 200);
  assert.deepEqual(cleared.body.entries, [{ rank: 1, login: 'alpha', totalTokens: DAY_CEILING + 11, estimatedValueMicros: null }]);
  e.DB.raw.exec("UPDATE users SET hidden = 1 WHERE login = 'alpha'");
  for (const metric of ['tokens', 'value']) assert.deepEqual((await board(e, `?period=all&metric=${metric}`)).body.entries, []);
});

test('leaving removes every row at once, and more than five devices are refused', async () => {
  const e = env();
  const tokens = [];
  for (let n = 0; n < 5; n++) tokens.push((await join(e, n < 3 ? 'gho_synthetic_token_alpha_0001' : 'gho_synthetic_token_beta_00001', `3333333${n}-3333-4333-8333-333333333333`)).body.token);
  // The join limit is per GitHub account, so alpha's three joins are used. Beta joins two more devices.
  for (let n = 5; n < 8; n++) await join(e, 'gho_synthetic_token_beta_00001', `3333333${n}-3333-4333-8333-333333333333`);
  const beta = e.DB.raw.prepare("SELECT count(*) AS n FROM devices d JOIN users u ON u.id = d.user_id WHERE u.login = 'beta'").get().n;
  assert.equal(beta, 3, 'Beta stays within the hourly join limit.');
  await sync(e, tokens[0], [row('2026-11-08', 100)], '33333330-3333-4333-8333-333333333333');
  const left = await handle(post('/v1/leave', {}, tokens[0]), e, deps, NOW);
  assert.deepEqual(await left.json(), { left: true });
  for (const table of ['days', 'flags']) assert.equal(e.DB.raw.prepare(`SELECT count(*) AS n FROM ${table}`).get().n, 0);
  assert.equal(e.DB.raw.prepare("SELECT count(*) AS n FROM users WHERE login = 'alpha'").get().n, 0);
  assert.equal((await handle(post('/v1/leave', {}, tokens[1]), e, deps, NOW)).status, 401, 'Every device token of the user is gone.');
  // A sixth device is refused for one account.
  const crowded = env();
  for (let n = 0; n < 5; n++) crowded.DB.raw.prepare('INSERT INTO users (id, github_id, login, created_at) VALUES (1, 101, ?1, ?2) ON CONFLICT DO NOTHING').run('alpha', NOW.toISOString());
  for (let n = 0; n < 5; n++) crowded.DB.raw.prepare('INSERT INTO devices VALUES (1, ?1, ?2, ?3)').run(`4444444${n}-4444-4444-8444-444444444444`, String(n).repeat(64), NOW.toISOString());
  assert.equal((await join(crowded, 'gho_synthetic_token_alpha_0001', '55555555-5555-4555-8555-555555555555')).body.error, 'too_many_devices');
});

test('the public page escapes every value and allows no script', async () => {
  const e = env();
  e.DB.raw.prepare('INSERT INTO users (id, github_id, login, created_at) VALUES (1, 9, ?1, ?2)').run('<script>x</script>', NOW.toISOString());
  e.DB.raw.prepare('INSERT INTO devices VALUES (1, ?1, ?2, ?3)').run(DEVICE, 'a'.repeat(64), NOW.toISOString());
  e.DB.raw.prepare("INSERT INTO days VALUES (1, ?1, '2026-11-08', 'claude', 'm', 7, 0, 0, 0, 7, NULL)").run(DEVICE);
  const response = await handle(get('/'), e, deps, NOW);
  const html = await response.text();
  assert.ok(html.includes('&lt;script&gt;x&lt;/script&gt;') && !html.includes('<script>'));
  assert.ok(html.includes(TITLE));
  assert.match(response.headers.get('content-security-policy'), /default-src 'none'/);
  assert.equal((await handle(get('/admin'), e, deps, NOW)).status, 404);
});
