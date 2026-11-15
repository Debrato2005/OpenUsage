// Runs the Worker against SQLite through a small D1 stand-in. No network: GitHub is a stub.
// Run with: node --test services/leaderboard/test/
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import { DatabaseSync } from 'node:sqlite';
import worker, { DAY_CEILING, TITLE, handle, validRows } from '../src/worker.mjs';

/** The part of the D1 binding that the Worker uses: prepare, bind, first, all, run and batch. */
function d1({ beforeRun, afterRun, beforeRead, beforeBatch, afterBatch } = {}) {
  const db = new DatabaseSync(':memory:');
  db.exec('PRAGMA foreign_keys = ON');
  db.exec(readFileSync(new URL('../migrations/0001_init.sql', import.meta.url), 'utf8'));
  const execute = (sql, params) => {
    const before = db.prepare('SELECT total_changes() AS n').get().n;
    const results = db.prepare(sql).all(...params);
    return { success: true, results, meta: { changes: db.prepare('SELECT total_changes() AS n').get().n - before } };
  };
  const statement = (sql, params = []) => ({
    bind: (...values) => statement(sql, values),
    first: async () => { await beforeRead?.(sql, params); return db.prepare(sql).get(...params) ?? null; },
    all: async () => { await beforeRead?.(sql, params); return execute(sql, params); },
    run: async () => {
      await beforeRun?.(sql, params);
      const result = execute(sql, params);
      return afterRun ? afterRun(sql, result) : result;
    },
    beforeRun: () => beforeRun?.(sql, params),
    runSync: () => {
      const result = execute(sql, params);
      return afterRun ? afterRun(sql, result) : result;
    },
  });
  return {
    raw: db,
    prepare: sql => statement(sql),
    async batch(statements) {
      await beforeBatch?.();
      // Async hooks run before the transaction. Its statements cannot interleave.
      for (const statement of statements) await statement.beforeRun();
      db.exec('BEGIN');
      let results;
      try { results = statements.map(s => s.runSync()); db.exec('COMMIT'); } catch (error) { db.exec('ROLLBACK'); throw error; }
      return afterBatch ? afterBatch(results) : results;
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

const get = (path, token) => new Request(`https://board.example${path}`, { headers: token ? { authorization: `Bearer ${token}` } : {} });
const row = (date, total, extra = {}) => ({ date, tool: 'claude', model: 'claude-sonnet-4-5', inputTokens: total - 3, outputTokens: 1, cacheReadTokens: 1, cacheWriteTokens: 1, totalTokens: total, estimatedValueMicros: 900, ...extra });

async function join(e, githubToken, deviceId = DEVICE, now = NOW) {
  const response = await handle(post('/v1/join', { githubToken, deviceId }), e, deps, now);
  return { status: response.status, body: await response.json() };
}
async function sync(e, token, rows, deviceId = DEVICE, now = NOW) {
  const response = await handle(post('/v1/sync', { deviceId, rows }, token), e, deps, now);
  return { status: response.status, body: await response.json() };
}
async function board(e, query = '', now = NOW) {
  const response = await handle(get(`/v1/leaderboard${query}`), e, deps, now);
  return { status: response.status, body: await response.json() };
}

async function ownRank(e, token, query = '', now = NOW) {
  const response = await handle(get(`/v1/me/rank${query}`, token), e, deps, now);
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
  if (path === '/v1/leave/github') return { e, input: { githubToken: 'gho_synthetic_token_alpha_0001' } };
  if (path === '/v1/join') return { e, input: { githubToken: 'gho_synthetic_token_alpha_0001', deviceId: DEVICE } };
  const { token } = (await join(e, 'gho_synthetic_token_alpha_0001')).body;
  return { e, token, input: { deviceId: DEVICE, rows: [row('2026-11-08', 10)] } };
}

test('a revoked leave cannot delete a different account after user ID reuse', async () => {
  let armed = false, replacement;
  const e = env({ beforeBatch: async () => {
    if (!armed) return;
    armed = false;
    assert.equal((await handle(post('/v1/leave', {}, token), e, deps, NOW)).status, 200);
    replacement = (await join(e, 'gho_synthetic_token_beta_00001')).body.token;
    assert.equal(e.DB.raw.prepare('SELECT id FROM users').get().id, 1);
    assert.equal((await sync(e, replacement, [row('2026-11-08', 24)])).status, 200);
  } });
  const token = (await join(e, 'gho_synthetic_token_alpha_0001')).body.token;
  armed = true;
  const response = await handle(post('/v1/leave', {}, token), e, deps, NOW);
  assert.equal(response.status, 401);
  assert.deepEqual(await response.json(), { error: 'not_joined' });
  assert.equal((await ownRank(e, replacement)).body.entry.totalTokens, 24);
});

test('a stalled sync cannot write into a different account after user ID and device UUID reuse', async () => {
  const e = env();
  const token = (await join(e, 'gho_synthetic_token_alpha_0001')).body.token;
  let replacement, pulls = 0;
  const request = new Request('https://board.example/v1/sync', {
    method: 'POST', duplex: 'half', headers: { authorization: `Bearer ${token}` },
    body: new ReadableStream({ async pull(controller) {
      if (pulls++) return controller.close();
      assert.equal((await handle(post('/v1/leave', {}, token), e, deps, NOW)).status, 200);
      replacement = (await join(e, 'gho_synthetic_token_beta_00001')).body.token;
      assert.equal(e.DB.raw.prepare('SELECT id FROM users').get().id, 1);
      assert.equal((await sync(e, replacement, [row('2026-11-08', 24)])).status, 200);
      controller.enqueue(encode(JSON.stringify({ deviceId: DEVICE, rows: [row('2026-11-08', DAY_CEILING + 1)] })));
    } }, { highWaterMark: 0 }),
  });
  const response = await handle(request, e, deps, NOW);
  assert.equal(response.status, 401);
  assert.deepEqual(await response.json(), { error: 'not_joined' });
  assert.equal((await ownRank(e, replacement)).body.entry.totalTokens, 24);
});
test('joining cannot attach a fresh token to another account after user ID reuse', async () => {
  let armed = false, replacement;
  const e = env({ beforeRun: async sql => {
    if (!armed || !/^INSERT INTO devices/.test(sql)) return;
    armed = false;
    assert.equal((await handle(post('/v1/leave', {}, token), e, deps, NOW)).status, 200);
    replacement = (await join(e, 'gho_synthetic_token_beta_00001')).body.token;
    assert.equal(e.DB.raw.prepare('SELECT id FROM users WHERE github_id = 202').get().id, 1);
    assert.equal((await sync(e, replacement, [row('2026-11-08', 24)])).status, 200);
  } });
  const token = (await join(e, 'gho_synthetic_token_alpha_0001')).body.token;
  armed = true;
  const joined = await join(e, 'gho_synthetic_token_alpha_0001');
  assert.equal(joined.status, 200);
  const betaRank = await ownRank(e, replacement);
  assert.equal(betaRank.status, 200);
  assert.equal(betaRank.body.entry.login, 'beta');
  assert.equal((await ownRank(e, joined.body.token)).body.entry, null);
  assert.equal((await sync(e, joined.body.token, [row('2026-11-08', 10)])).status, 200);
  assert.equal((await ownRank(e, joined.body.token)).body.entry.login, 'alpha');
});
test('fresh GitHub departure works at the device cap without another enrollment', async () => {
  const e = env(), tokens = [];
  const earlier = new Date(NOW.getTime() - 3600_000);
  for (let n = 0; n < 5; n++) {
    const joined = await join(e, 'gho_synthetic_token_alpha_0001', `3333333${n}-3333-4333-8333-333333333333`, n < 3 ? earlier : NOW);
    assert.equal(joined.status, 200);
    tokens.push(joined.body.token);
  }
  assert.equal((await join(e, 'gho_synthetic_token_alpha_0001')).body.error, 'too_many_devices');
  assert.equal((await sync(e, tokens[0], [row('2026-11-08', DAY_CEILING + 1)], '33333330-3333-4333-8333-333333333333')).status, 200);
  const beta = (await join(e, 'gho_synthetic_token_beta_00001')).body.token;
  await sync(e, beta, [row('2026-11-08', 24)]);
  const response = await handle(post('/v1/leave/github', { githubToken: 'gho_synthetic_token_alpha_0001' }), e,
    { fetchGitHubUser: async () => ({ id: 101, login: 'renamed-alpha' }) }, NOW);
  assert.equal(response.status, 200);
  assert.deepEqual(await response.json(), { left: true, login: 'renamed-alpha', localTokenRevoked: null });
  for (const token of tokens) assert.equal((await ownRank(e, token)).status, 401);
  for (const table of ['users', 'devices', 'days', 'flags']) {
    const column = table === 'users' ? 'github_id' : 'user_id';
    assert.equal(e.DB.raw.prepare(`SELECT count(*) AS n FROM ${table} WHERE ${column} = ?`).get(table === 'users' ? 101 : 1).n, 0);
  }
  assert.equal((await ownRank(e, beta)).body.entry.totalTokens, 24);
  assert.equal((await join(e, 'gho_synthetic_token_alpha_0001')).status, 429, 'Departure preserves the join allowance.');
});

test('GitHub departure reports only the exact token witness and preserves accounts with the same device UUID', async () => {
  for (const mode of ['none', 'same', 'foreign', 'rotated', 'absent']) {
    const e = env();
    const alpha = (await join(e, 'gho_synthetic_token_alpha_0001')).body.token;
    const beta = (await join(e, 'gho_synthetic_token_beta_00001')).body.token;
    await sync(e, alpha, [row('2026-11-08', 12)]);
    await sync(e, beta, [row('2026-11-08', 24)]);
    if (mode === 'rotated') assert.equal((await join(e, 'gho_synthetic_token_alpha_0001')).status, 200);
    const witness = mode === 'none' ? undefined : mode === 'foreign' ? beta : mode === 'absent' ? 'f'.repeat(64) : alpha;
    const response = await handle(post('/v1/leave/github', { githubToken: 'gho_synthetic_token_alpha_0001' }, witness), e, deps, NOW);
    assert.equal(response.status, 200, mode);
    assert.deepEqual(await response.json(), { left: true, login: 'alpha', localTokenRevoked: mode === 'none' ? null : mode === 'same' });
    assert.equal((await ownRank(e, alpha)).status, 401);
    assert.equal((await ownRank(e, beta)).body.entry.totalTokens, 24);
    const dump = JSON.stringify(['users', 'devices', 'days', 'flags', 'limits'].map(table => e.DB.raw.prepare(`SELECT * FROM ${table}`).all()));
    for (const secret of [alpha, beta, 'gho_synthetic_token_alpha_0001', '203.0.113.9']) assert.ok(!dump.includes(secret));
  }
});

test('GitHub departure uses numeric identity and succeeds when the account is already absent', async () => {
  const e = env();
  const alpha = (await join(e, 'gho_synthetic_token_alpha_0001')).body.token;
  const beta = (await join(e, 'gho_synthetic_token_beta_00001')).body.token;
  await sync(e, beta, [row('2026-11-08', 24)]);
  const renamed = { fetchGitHubUser: async () => ({ id: 101, login: 'beta' }) };
  for (const [witness, expected] of [[alpha, true], [alpha, false], [undefined, null]]) {
    const response = await handle(post('/v1/leave/github', { githubToken: 'gho_synthetic_token_alpha_0001' }, witness), e, renamed, NOW);
    assert.equal(response.status, 200);
    assert.deepEqual(await response.json(), { left: true, login: 'beta', localTokenRevoked: expected });
    assert.equal((await ownRank(e, beta)).body.entry.totalTokens, 24);
  }
  assert.equal(e.DB.raw.prepare('SELECT count(*) AS n FROM devices').get().n, 1);
});

test('GitHub departure checks the witness in the deletion snapshot after token rotation', async () => {
  let armed = false, current;
  const e = env({ beforeBatch: async () => {
    if (!armed) return;
    armed = false;
    current = (await join(e, 'gho_synthetic_token_alpha_0001')).body.token;
  } });
  const alpha = (await join(e, 'gho_synthetic_token_alpha_0001')).body.token;
  armed = true;
  const response = await handle(post('/v1/leave/github', { githubToken: 'gho_synthetic_token_alpha_0001' }, alpha), e, deps, NOW);
  assert.equal(response.status, 200);
  assert.deepEqual(await response.json(), { left: true, login: 'alpha', localTokenRevoked: false });
  assert.equal((await ownRank(e, current)).status, 401);
});

test('GitHub departure requires fresh valid GitHub authorization despite a current service token', async () => {
  const e = env();
  const alpha = (await join(e, 'gho_synthetic_token_alpha_0001')).body.token;
  for (const input of [{}, { githubToken: 'short' }, { githubToken: 1 }, { githubToken: 'x'.repeat(256) },
    { githubToken: 'gho_synthetic_token_alpha_0001', deviceId: DEVICE }, { githubToken: 'gho_synthetic_token_alpha_0001', login: 'alpha' }]) {
    const response = await handle(post('/v1/leave/github', input, alpha), e, deps, NOW);
    assert.equal(response.status, 400);
    assert.deepEqual(await response.json(), { error: 'invalid_request' });
  }
  const invalid = await handle(post('/v1/leave/github', { githubToken: 'gho_unknown_token_for_nobody' }, alpha), e, deps, NOW);
  assert.equal(invalid.status, 401);
  assert.deepEqual(await invalid.json(), { error: 'github_refused' });
  for (const account of [null, { id: 0, login: 'alpha' }, { id: '101', login: 'alpha' }, { id: Number.MAX_SAFE_INTEGER + 1, login: 'alpha' }, { id: 101, login: '<script>' }]) {
    const response = await handle(post('/v1/leave/github', { githubToken: 'gho_synthetic_token_alpha_0001' }, alpha), e,
      { fetchGitHubUser: async () => account }, NOW);
    assert.equal(response.status, 401);
    assert.deepEqual(await response.json(), { error: 'github_refused' });
  }
  const failed = await handle(post('/v1/leave/github', { githubToken: 'gho_synthetic_token_alpha_0001' }, alpha), e,
    { fetchGitHubUser: async () => { throw new Error('synthetic secret'); } }, NOW);
  assert.equal(failed.status, 500);
  assert.deepEqual(await failed.json(), { error: 'unavailable' });
  for (const authorization of ['', 'Basic abc', 'Bearer short']) {
    const response = await handle(post('/v1/leave/github', { githubToken: 'gho_synthetic_token_alpha_0001' }, undefined, { authorization }), e, deps, NOW);
    assert.equal(response.status, 401);
  }
  assert.equal((await ownRank(e, alpha)).status, 200);
});

test('GitHub departure has separate account and edge limits and does not spend a join allowance', async () => {
  const e = env(), counts = new Map();
  e.JOIN_LIMITER.limit = async ({ key }) => {
    const count = (counts.get(key) ?? 0) + 1;
    counts.set(key, count);
    return { success: count <= 3 };
  };
  for (let n = 0; n < 3; n++) assert.equal((await join(e, 'gho_synthetic_token_alpha_0001')).status, 200);
  for (let n = 0; n < 3; n++) assert.equal((await handle(post('/v1/leave/github', { githubToken: 'gho_synthetic_token_alpha_0001' }), e, deps, NOW)).status, 200);
  assert.equal((await handle(post('/v1/leave/github', { githubToken: 'gho_synthetic_token_alpha_0001' }), e, deps, NOW)).status, 429);
  assert.equal(e.DB.raw.prepare("SELECT count FROM limits WHERE key = 'join:101'").get().count, 3);
  assert.equal(e.DB.raw.prepare("SELECT count FROM limits WHERE key = 'leave-github:101'").get().count, 3);
  const fresh = env();
  assert.equal((await handle(post('/v1/leave/github', { githubToken: 'gho_synthetic_token_alpha_0001' }), fresh, deps, NOW)).status, 200);
  for (let n = 0; n < 3; n++) assert.equal((await join(fresh, 'gho_synthetic_token_alpha_0001')).status, 200);
});

test('concurrent GitHub departures share the account allowance and cross an hour independently of joins', { timeout: 5000 }, async () => {
  const e = env({ beforeRun: simultaneousWrites(/^INSERT INTO limits/, 8) });
  const responses = await Promise.all(Array.from({ length: 8 }, () => handle(post('/v1/leave/github', { githubToken: 'gho_synthetic_token_alpha_0001' }), e, deps, NOW)));
  assert.equal(responses.filter(response => response.status === 200).length, 3);
  assert.equal(responses.filter(response => response.status === 429).length, 5);
  assert.equal((await handle(post('/v1/leave/github', { githubToken: 'gho_synthetic_token_alpha_0001' }), e, deps, new Date('2026-11-08T13:00:00Z'))).status, 200);
  assert.equal((await join(e, 'gho_synthetic_token_alpha_0001')).status, 200);
});

test('departure batches roll back deletion failures and refuse malformed result arrays', async () => {
  for (const route of ['/v1/leave', '/v1/leave/github']) {
    let armed = false;
    const e = env({ afterRun: (sql, result) => {
      if (armed && /^DELETE FROM users/.test(sql)) throw new Error('synthetic failure after deletion');
      return result;
    } });
    const alpha = (await join(e, 'gho_synthetic_token_alpha_0001')).body.token;
    await sync(e, alpha, [row('2026-11-08', 24)]);
    armed = true;
    const response = await handle(post(route, { githubToken: 'gho_synthetic_token_alpha_0001' }, alpha), e, deps, NOW);
    assert.equal(response.status, 500);
    assert.deepEqual(await response.json(), { error: 'unavailable' });
    assert.equal((await ownRank(e, alpha)).body.entry.totalTokens, 24);
  }
  for (const result of [undefined, [], [{ success: true, results: [] }], [{ success: true, results: [] }, { success: false, results: [] }],
    [{ success: true, results: [{ local_token_revoked: 2 }] }, { success: true, results: [] }]]) {
    let armed = false;
    const e = env({ afterBatch: actual => armed ? result : actual });
    const alpha = (await join(e, 'gho_synthetic_token_alpha_0001')).body.token;
    armed = true;
    const response = await handle(post('/v1/leave/github', { githubToken: 'gho_synthetic_token_alpha_0001' }, alpha), e, deps, NOW);
    assert.equal(response.status, 500);
    assert.deepEqual(await response.json(), { error: 'unavailable' });
  }
});

test('sync rejects token rotation before its snapshot even for empty rows', async () => {
  for (const rows of [[], [row('2026-11-08', 12)]]) {
    let armed = false, current;
    const e = env({ beforeBatch: async () => {
      if (!armed) return;
      armed = false;
      current = (await join(e, 'gho_synthetic_token_alpha_0001')).body.token;
    } });
    const alpha = (await join(e, 'gho_synthetic_token_alpha_0001')).body.token;
    armed = true;
    assert.deepEqual(await sync(e, alpha, rows), { status: 401, body: { error: 'not_joined' } });
    assert.equal((await ownRank(e, current)).body.entry, null);
  }
});

test('sync limits follow GitHub identity through departure and reused user IDs', async () => {
  const e = env();
  const alpha = (await join(e, 'gho_synthetic_token_alpha_0001')).body.token;
  for (let n = 0; n < 30; n++) assert.equal((await sync(e, alpha, [])).status, 200);
  assert.equal((await handle(post('/v1/leave', {}, alpha), e, deps, NOW)).status, 200);
  const beta = (await join(e, 'gho_synthetic_token_beta_00001')).body.token;
  assert.equal(e.DB.raw.prepare('SELECT id FROM users').get().id, 1);
  assert.equal((await sync(e, beta, [])).status, 200);
  const rejoined = (await join(e, 'gho_synthetic_token_alpha_0001')).body.token;
  assert.equal((await sync(e, rejoined, [])).status, 429);
});

test('join, sync, and GitHub departure accept valid JSON at the exact byte cap', async () => {
  for (const path of ['/v1/join', '/v1/sync', '/v1/leave/github']) {
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
      else if (path === '/v1/leave/github') assert.deepEqual(result, { left: true, login: 'alpha', localTokenRevoked: null });
      else assert.deepEqual(result, { accepted: 1 });
      assert.deepEqual(stats, { pulls: 3, bytes: BODY_CAP, cancels: 0 });
    }
  }
});

test('join, sync, and GitHub departure cancel at one byte over the cap despite missing or smaller headers', async () => {
  for (const path of ['/v1/join', '/v1/sync', '/v1/leave/github']) {
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
  for (const path of ['/v1/join', '/v1/sync', '/v1/leave/github']) {
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
  for (const path of ['/v1/join', '/v1/sync', '/v1/leave/github']) {
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
  for (const path of ['/v1/join', '/v1/sync', '/v1/leave/github']) {
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
  for (const path of ['/v1/join', '/v1/sync', '/v1/leave/github']) {
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
  for (const path of ['/v1/join', '/v1/sync', '/v1/leave/github']) {
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
  for (const path of ['/v1/join', '/v1/sync', '/v1/leave/github']) {
    const { e, token } = await bodyTarget(path);
    const { request, stats } = streamedPost(path, [new Uint16Array([0x7b, 0x7d]), encode(' ')], { token });
    const response = await handle(request, e, deps, NOW);
    assert.equal(response.status, 500);
    assert.deepEqual(await response.json(), { error: 'unavailable' });
    assert.deepEqual(stats, { pulls: 1, bytes: 4, cancels: 1 });
  }
});

test('failed or stalled cancellation cannot delay the overflow response', { timeout: 5000 }, async () => {
  for (const path of ['/v1/join', '/v1/sync', '/v1/leave/github']) {
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
    ['/v1/leave/github', 'missing-limiter', 503, 'limiter_unavailable'],
    ['/v1/leave/github', 'edge-limit', 429, 'rate_limited'],
    ['/v1/sync', 'missing-token', 401, 'not_joined'],
    ['/v1/sync', 'hour-limit', 429, 'rate_limited'],
  ]) {
    const { e, token, input } = await bodyTarget(path);
    if (mode === 'missing-limiter') delete e.JOIN_LIMITER;
    if (mode === 'edge-limit') e.JOIN_LIMITER.limit = async () => ({ success: false });
    if (mode === 'hour-limit') e.DB.raw.prepare('INSERT INTO limits VALUES (?1, ?2, ?3)').run('sync-github:101', '2026-11-08T12', 30);
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
    if (initial) e.DB.raw.prepare('INSERT INTO limits VALUES (?1, ?2, ?3)').run('sync-github:101', '2026-11-08T12', initial);
    armed = true;
    const responses = await Promise.all(Array.from({ length: 35 }, (_, n) => sync(e, n % 2 ? first : second, [], n % 2 ? DEVICE : SECOND)));
    assert.equal(responses.filter(x => x.status === 200).length, 30 - initial);
    for (const refused of responses.filter(x => x.status !== 200)) {
      assert.deepEqual(refused, { status: 429, body: { error: 'rate_limited' } });
    }
    assert.equal(e.DB.raw.prepare('SELECT count FROM limits WHERE key = ?1 AND hour = ?2').get('sync-github:101', '2026-11-08T12').count, 30);
    assert.equal((await sync(e, first, [])).status, 429);
    assert.equal((await sync(e, beta, [])).status, 200);
    assert.equal((await sync(e, first, [], DEVICE, new Date('2026-11-08T13:00:00Z'))).status, 200);
    assert.equal(e.DB.raw.prepare('SELECT count FROM limits WHERE key = ?1 AND hour = ?2').get('sync-github:101', '2026-11-08T13').count, 1);
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

test('own rank uses the public order and returns a snapshot for a joined user with no data', async () => {
  const e = env();
  const alpha = (await join(e, 'gho_synthetic_token_alpha_0001')).body.token;
  const beta = (await join(e, 'gho_synthetic_token_beta_00001')).body.token;
  assert.deepEqual(await ownRank(e, alpha), { status: 200, body: {
    title: TITLE, period: '30d', metric: 'tokens', snapshotAt: NOW.toISOString(), totalParticipants: 0, entry: null,
  } });
  assert.equal((await sync(e, beta, [row('2026-11-08', 20)])).status, 200);
  assert.equal((await sync(e, alpha, [row('2026-11-08', 10)])).status, 200);
  const result = await ownRank(e, alpha);
  assert.equal(result.status, 200);
  assert.equal(result.body.totalParticipants, 2);
  assert.deepEqual(result.body.entry, (await board(e)).body.entries[1]);
});

test('own rank includes participants beyond the public top 100 and gives ties distinct stable positions', async () => {
  const e = env();
  e.JOIN_LIMITER.limit = async () => ({ success: true });
  const tokens = [];
  for (let index = 0; index < 105; index++) {
    const login = `participant-${String(index).padStart(3, '0')}`;
    const response = await handle(post('/v1/join', { githubToken: 'gho_synthetic_many_participants', deviceId: DEVICE }), e,
      { fetchGitHubUser: async () => ({ id: 1000 + index, login }) }, NOW);
    assert.equal(response.status, 200);
    const { token } = await response.json();
    tokens.push(token);
    assert.equal((await sync(e, token, [row('2026-11-08', 10)])).status, 200);
  }
  for (const metric of ['tokens', 'value']) {
    const query = `?period=all&metric=${metric}`;
    const entries = (await board(e, query)).body.entries;
    assert.equal(entries.length, 100);
    assert.deepEqual(entries.map(entry => entry.rank), Array.from({ length: 100 }, (_, index) => index + 1));
    assert.equal(entries.at(-1).login, 'participant-099');
    for (const index of [0, 99, 100, 104]) {
      const own = await ownRank(e, tokens[index], query);
      assert.equal(own.status, 200);
      assert.equal(own.body.totalParticipants, 105);
      assert.deepEqual(own.body.entry, { rank: index + 1, login: `participant-${String(index).padStart(3, '0')}`, totalTokens: 10, estimatedValueMicros: 900 });
      if (index < 100) assert.deepEqual(own.body.entry, entries[index]);
    }
  }
});

test('rank reads reject unknown, duplicate, and invalid selectors on both routes', async () => {
  const e = env();
  const { token } = (await join(e, 'gho_synthetic_token_alpha_0001')).body;
  for (const query of ['?period=1y', '?metric=money', '?period=', '?metric=', '?period=7d&user=alpha', '?period=7d&period=30d', '?metric=tokens&metric=tokens']) {
    for (const route of ['/v1/leaderboard', '/v1/me/rank']) {
      const response = await handle(get(`${route}${query}`, token), e, deps, NOW);
      assert.equal(response.status, 400, `${route}${query}`);
      assert.deepEqual(await response.json(), { error: 'invalid_request' });
    }
  }
});

test('own rank requires a current device token and leaves all device tokens revoked at once', async () => {
  const e = env();
  const first = (await join(e, 'gho_synthetic_token_alpha_0001')).body.token;
  const second = (await join(e, 'gho_synthetic_token_alpha_0001', SECOND)).body.token;
  assert.equal((await sync(e, first, [row('2026-11-08', 10)])).status, 200);
  for (const authorization of [undefined, 'Basic abc', `bearer ${first}`, `Bearer ${'F'.repeat(64)}`, `Bearer ${'f'.repeat(63)}`, `Bearer ${'f'.repeat(64)}`]) {
    const request = new Request('https://board.example/v1/me/rank', { headers: authorization ? { authorization } : {} });
    const response = await handle(request, e, deps, NOW);
    assert.equal(response.status, 401);
    assert.deepEqual(await response.json(), { error: 'not_joined' });
  }
  const limits = e.DB.raw.prepare('SELECT * FROM limits ORDER BY key, hour').all();
  for (let index = 0; index < 35; index++) assert.equal((await ownRank(e, first)).status, 200);
  assert.deepEqual(e.DB.raw.prepare('SELECT * FROM limits ORDER BY key, hour').all(), limits);
  const rotated = (await join(e, 'gho_synthetic_token_alpha_0001')).body.token;
  assert.deepEqual(await ownRank(e, first), { status: 401, body: { error: 'not_joined' } });
  assert.equal((await ownRank(e, rotated)).status, 200);
  assert.equal((await ownRank(e, second)).status, 200);
  assert.equal((await handle(post('/v1/leave', {}, second), e, deps, NOW)).status, 200);
  for (const token of [first, second, rotated]) assert.deepEqual(await ownRank(e, token), { status: 401, body: { error: 'not_joined' } });
  assert.deepEqual((await board(e)).body.entries, []);
});

test('own rank rechecks token revocation in the snapshot query', async () => {
  let revoke;
  const e = env({ beforeRead: async sql => {
    if (!sql.startsWith('WITH aggregates') || !revoke) return;
    const token = revoke;
    revoke = undefined;
    assert.equal((await handle(post('/v1/leave', {}, token), e, deps, NOW)).status, 200);
  } });
  const token = (await join(e, 'gho_synthetic_token_alpha_0001')).body.token;
  assert.equal((await sync(e, token, [row('2026-11-08', 10)])).status, 200);
  revoke = token;
  assert.deepEqual(await ownRank(e, token), { status: 401, body: { error: 'not_joined' } });
});

test('rank database failures return only unavailable and unauthenticated reads stop before aggregates', async () => {
  for (const target of ['authentication', 'ranking']) {
    let armed = false;
    const e = env({ beforeRead: sql => {
      if (armed && (target === 'authentication' ? sql.startsWith('SELECT user_id FROM devices') : sql.startsWith('WITH aggregates'))) {
        throw new Error('synthetic private database detail');
      }
    } });
    const token = (await join(e, 'gho_synthetic_token_alpha_0001')).body.token;
    armed = true;
    assert.deepEqual(await ownRank(e, token), { status: 500, body: { error: 'unavailable' } });
    if (target === 'ranking') {
      assert.deepEqual(await board(e), { status: 500, body: { error: 'unavailable' } });
      assert.deepEqual(await ownRank(e, 'f'.repeat(64)), { status: 401, body: { error: 'not_joined' } });
    }
  }
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
        const query = `?period=${period}&metric=${metric}`;
        const result = await board(e, query);
        assert.equal(result.status, 200);
        assert.deepEqual(result.body.entries, [{ rank: 1, login: 'alpha', totalTokens: 20, estimatedValueMicros: expected }]);
        const own = await ownRank(e, token, query);
        assert.equal(own.status, 200);
        assert.equal(own.body.totalParticipants, 1);
        assert.deepEqual(own.body.entry, result.body.entries[0]);
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
    const query = `?period=all&metric=${metric}`;
    const result = await board(e, query);
    assert.equal(result.status, 200);
    for (const [token, login] of [[alpha, 'alpha'], [alphaPhone, 'alpha'], [beta, 'beta']]) {
      const own = await ownRank(e, token, query);
      assert.equal(own.status, 200);
      assert.equal(own.body.totalParticipants, 2);
      assert.deepEqual(own.body.entry, result.body.entries.find(entry => entry.login === login));
    }
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
      const query = `?period=${period}&metric=${metric}`;
      const result = await board(e, query);
      assert.equal(result.status, 200);
      assert.deepEqual(result.body.entries, [{ rank: 1, login: 'alpha', totalTokens, estimatedValueMicros }]);
      const own = await ownRank(e, token, query);
      assert.equal(own.status, 200);
      assert.equal(own.body.totalParticipants, 1);
      assert.deepEqual(own.body.entry, result.body.entries[0]);
    }
  }
  e.DB.raw.exec('UPDATE flags SET cleared = 1');
  const cleared = await board(e, '?period=7d&metric=value');
  assert.equal(cleared.status, 200);
  assert.deepEqual(cleared.body.entries, [{ rank: 1, login: 'alpha', totalTokens: DAY_CEILING + 11, estimatedValueMicros: null }]);
  assert.deepEqual((await ownRank(e, token, '?period=7d&metric=value')).body.entry, cleared.body.entries[0]);
  e.DB.raw.exec("UPDATE users SET hidden = 1 WHERE login = 'alpha'");
  for (const metric of ['tokens', 'value']) {
    const query = `?period=all&metric=${metric}`;
    assert.deepEqual((await board(e, query)).body.entries, []);
    const own = await ownRank(e, token, query);
    assert.equal(own.status, 200);
    assert.equal(own.body.totalParticipants, 0);
    assert.equal(own.body.entry, null);
  }
});

test('own rank keeps measured zero separate from no data and reveals no moderation reason', async () => {
  const e = env();
  const alpha = (await join(e, 'gho_synthetic_token_alpha_0001')).body.token;
  const beta = (await join(e, 'gho_synthetic_token_beta_00001')).body.token;
  assert.equal((await sync(e, beta, [row('2026-11-08', 10)])).status, 200);
  const noData = await ownRank(e, alpha);
  assert.equal(noData.body.entry, null);
  assert.equal(noData.body.totalParticipants, 1);
  assert.equal((await sync(e, alpha, [row('2026-11-08', 0, {
    inputTokens: 0, outputTokens: 0, cacheReadTokens: 0, cacheWriteTokens: 0, estimatedValueMicros: 0,
  })])).status, 200);
  assert.deepEqual((await ownRank(e, alpha)).body.entry, { rank: 2, login: 'alpha', totalTokens: 0, estimatedValueMicros: 0 });
  e.DB.raw.exec("UPDATE users SET hidden = 1 WHERE login = 'alpha'");
  assert.deepEqual(await ownRank(e, alpha), noData);
  e.DB.raw.exec("UPDATE users SET hidden = 0 WHERE login = 'alpha'");
  assert.equal((await sync(e, alpha, [row('2026-11-08', DAY_CEILING + 1)])).status, 200);
  assert.deepEqual(await ownRank(e, alpha), noData);
});

test('historical rank snapshots exclude later dates after a clock change', async () => {
  const e = env();
  const { token } = (await join(e, 'gho_synthetic_token_alpha_0001')).body;
  assert.equal((await sync(e, token, [row('2026-11-08', 20), row('2026-11-07', 10)])).status, 200);
  const earlier = new Date('2026-11-07T12:00:00Z');
  for (const period of ['7d', '30d', 'all']) {
    for (const metric of ['tokens', 'value']) {
      const query = `?period=${period}&metric=${metric}`;
      const publicResult = await board(e, query, earlier);
      assert.equal(publicResult.status, 200);
      assert.deepEqual(publicResult.body.entries, [{ rank: 1, login: 'alpha', totalTokens: 10, estimatedValueMicros: 900 }]);
      const own = await ownRank(e, token, query, earlier);
      assert.equal(own.status, 200);
      assert.equal(own.body.snapshotAt, earlier.toISOString());
      assert.equal(own.body.totalParticipants, 1);
      assert.deepEqual(own.body.entry, publicResult.body.entries[0]);
    }
  }
});

test('rank APIs preserve safe integer limits and refuse unsafe token or value aggregates', { timeout: 10000 }, async () => {
  for (const field of ['tokens', 'value']) {
    const e = env();
    const { token } = (await join(e, 'gho_synthetic_token_alpha_0001')).body;
    const fullRows = Math.floor(Number.MAX_SAFE_INTEGER / 1_000_000_000_000);
    const remainder = Number.MAX_SAFE_INTEGER - fullRows * 1_000_000_000_000;
    const makeRow = (index, amount) => row('2026-11-08', field === 'tokens' ? amount : 3, {
      model: `m-${index}`, estimatedValueMicros: field === 'value' ? amount : 0,
    });
    const rows = Array.from({ length: fullRows + 1 }, (_, index) => makeRow(index, index === fullRows ? remainder : 1_000_000_000_000));
    for (let start = 0; start < rows.length; start += 100) {
      const hourOffset = Math.floor(start / 3000);
      const now = new Date(NOW.getTime() + hourOffset * 3600_000);
      assert.equal((await sync(e, token, rows.slice(start, start + 100), DEVICE, now)).status, 200);
    }
    // A maintainer permits the synthetic large token day before the read.
    e.DB.raw.exec('UPDATE flags SET cleared = 1');
    for (const period of ['7d', '30d', 'all']) {
      for (const metric of ['tokens', 'value']) {
        const query = `?period=${period}&metric=${metric}`;
        const publicResult = await board(e, query);
        assert.equal(publicResult.status, 200);
        assert.equal(publicResult.body.entries[0][field === 'tokens' ? 'totalTokens' : 'estimatedValueMicros'], Number.MAX_SAFE_INTEGER);
        const own = await ownRank(e, token, query);
        assert.equal(own.status, 200);
        assert.deepEqual(own.body.entry, publicResult.body.entries[0]);
      }
    }
    assert.equal((await sync(e, token, [makeRow(fullRows, remainder + 2)])).status, 200);
    for (const metric of ['tokens', 'value']) {
      assert.deepEqual(await board(e, `?metric=${metric}`), { status: 500, body: { error: 'unavailable' } });
      assert.deepEqual(await ownRank(e, token, `?metric=${metric}`), { status: 500, body: { error: 'unavailable' } });
    }
    if (field === 'value') {
      // Safe top-100 entries cannot conceal an unsafe own aggregate at rank 101.
      for (let index = 0; index < 100; index++) {
        e.DB.raw.prepare('INSERT INTO users (id, github_id, login, created_at) VALUES (?1, ?2, ?3, ?4)').run(1000 + index, 2000 + index, `safe-${index}`, NOW.toISOString());
        e.DB.raw.prepare('INSERT INTO devices VALUES (?1, ?2, ?3, ?4)').run(1000 + index, DEVICE, String(index).padStart(64, '0'), NOW.toISOString());
        e.DB.raw.prepare("INSERT INTO days VALUES (?1, ?2, '2026-11-08', 'claude', 'm', 50000, 0, 0, 0, 50000, 0)").run(1000 + index, DEVICE);
      }
      const publicResult = await board(e);
      assert.equal(publicResult.status, 200);
      assert.equal(publicResult.body.entries.length, 100);
      assert.deepEqual(await ownRank(e, token), { status: 500, body: { error: 'unavailable' } });
    }
    e.DB.raw.exec("UPDATE users SET hidden = 1 WHERE login = 'alpha'");
    const hidden = await ownRank(e, token);
    assert.equal(hidden.status, 200);
    assert.equal(hidden.body.entry, null);
    assert.equal(hidden.body.totalParticipants, field === 'value' ? 100 : 0);
  }
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

test('scheduled cleanup removes expired counters without traffic and preserves active allowances', async () => {
  const e = env();
  const token = (await join(e, 'gho_synthetic_token_alpha_0001')).body.token;
  await sync(e, token, [row('2026-11-08', 24)]);
  await handle(post('/v1/leave', {}, token), e, deps, NOW);
  e.DB.raw.prepare('INSERT INTO limits VALUES (?1, ?2, ?3)').run('legacy:1', '2026-10-01T00', 1);
  await worker.scheduled({ scheduledTime: new Date('2026-11-08T14:00:00Z').getTime() }, e);
  assert.deepEqual(e.DB.raw.prepare('SELECT key, count FROM limits ORDER BY key').all().map(x => ({ ...x })), [
    { key: 'join:101', count: 1 }, { key: 'sync-github:101', count: 1 },
  ]);
  await worker.scheduled({ scheduledTime: new Date('2026-11-08T11:00:00Z').getTime() }, e);
  assert.equal(e.DB.raw.prepare('SELECT count(*) AS n FROM limits').get().n, 2);
  await worker.scheduled({ scheduledTime: new Date('2026-11-08T15:00:00Z').getTime() }, e);
  assert.equal(e.DB.raw.prepare('SELECT count(*) AS n FROM limits').get().n, 0);
  assert.match(readFileSync(new URL('../wrangler.toml', import.meta.url), 'utf8'), /\[triggers\]\s*crons = \["0 \* \* \* \*"\]/);
});

test('scheduled cleanup reports database failures and rejects an invalid clock before a write', async () => {
  for (const result of [null, { success: false }, { success: true }, { success: true, meta: { changes: -1 } }]) {
    let writes = 0;
    const e = env({ beforeRun: () => { writes++; if (result === null) throw new Error('synthetic failure'); }, afterRun: () => result });
    await assert.rejects(() => worker.scheduled({ scheduledTime: NaN }, e));
    assert.equal(writes, 0);
    await assert.rejects(() => worker.scheduled({ scheduledTime: NOW.getTime() }, e));
    assert.equal(writes, 1);
  }
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
