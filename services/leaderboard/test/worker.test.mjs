// Runs the Worker against SQLite through a small D1 stand-in. No network: GitHub is a stub.
// Run with: node --test services/leaderboard/test/
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import { DatabaseSync } from 'node:sqlite';
import { DAY_CEILING, TITLE, handle, validRows } from '../src/worker.mjs';

/** The part of the D1 binding that the Worker uses: prepare, bind, first, all, run and batch. */
function d1() {
  const db = new DatabaseSync(':memory:');
  db.exec('PRAGMA foreign_keys = ON');
  db.exec(readFileSync(new URL('../migrations/0001_init.sql', import.meta.url), 'utf8'));
  const statement = (sql, params = []) => ({
    bind: (...values) => statement(sql, values),
    first: async () => db.prepare(sql).get(...params) ?? null,
    all: async () => ({ results: db.prepare(sql).all(...params) }),
    run: async () => ({ meta: { changes: Number(db.prepare(sql).run(...params).changes) } }),
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
function env() {
  const allowed = { count: 0 };
  return { DB: d1(), JOIN_LIMITER: { limit: async () => ({ success: ++allowed.count <= 50 }) }, allowed };
}
const post = (path, body, token, extra = {}) => new Request(`https://board.example${path}`, {
  method: 'POST', body: typeof body === 'string' ? body : JSON.stringify(body),
  headers: { 'content-type': 'application/json', 'cf-connecting-ip': '203.0.113.9', ...(token ? { authorization: `Bearer ${token}` } : {}), ...extra },
});
const get = path => new Request(`https://board.example${path}`);
const row = (date, total, extra = {}) => ({ date, tool: 'claude', model: 'claude-sonnet-4-5', inputTokens: total - 3, outputTokens: 1, cacheReadTokens: 1, cacheWriteTokens: 1, totalTokens: total, estimatedValueMicros: 900, ...extra });
async function join(e, githubToken, deviceId = DEVICE) {
  const response = await handle(post('/v1/join', { githubToken, deviceId }), e, deps, NOW);
  return { status: response.status, body: await response.json() };
}
async function sync(e, token, rows, deviceId = DEVICE) {
  const response = await handle(post('/v1/sync', { deviceId, rows }, token), e, deps, NOW);
  return { status: response.status, body: await response.json() };
}
async function board(e, query = '') {
  const response = await handle(get(`/v1/leaderboard${query}`), e, deps, NOW);
  return { status: response.status, body: await response.json() };
}

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
