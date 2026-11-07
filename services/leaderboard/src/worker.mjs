// The opt-in OpenUsage leaderboard (SRS Section 14A). It accepts only the fields that
// OU-BOARD-003 permits, from local log totals, and it ranks values that it cannot prove.
// See docs/decisions/2026-11-07-leaderboard-service.md for every rule in this file.

export const TITLE = 'Self-reported';
export const TOOLS = ['claude', 'codex', 'opencode', 'amp', 'droid', 'codebuff', 'hermes', 'pi', 'goose', 'kilo', 'copilot', 'gemini', 'antigravity', 'kimi', 'qwen', 'openclaw', 'grok', 'zcode'];
const MAX_BODY = 64 * 1024;
const MAX_ROWS = 400;
const MAX_COUNT = 1_000_000_000_000;
const MAX_DEVICES = 5;
const JOINS_PER_HOUR = 3;
const SYNCS_PER_HOUR = 30;
/** A user day above this total is flagged and left out of the ranks until a maintainer reviews it. */
export const DAY_CEILING = 2_000_000_000;
const RANK_LIMIT = 100;
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
const MODEL = /^[a-z0-9.:-]{1,64}$/;
const LOGIN = /^[A-Za-z0-9](?:[A-Za-z0-9-]{0,38})$/;
const ROW_KEYS = ['date', 'tool', 'model', 'inputTokens', 'outputTokens', 'cacheReadTokens', 'cacheWriteTokens', 'totalTokens', 'estimatedValueMicros'];

class Refused extends Error {
  constructor(status, code) { super(code); this.status = status; this.code = code; }
}

const json = (status, body) => new Response(JSON.stringify(body), {
  status,
  headers: { 'content-type': 'application/json; charset=utf-8', 'x-content-type-options': 'nosniff', 'cache-control': 'no-store' },
});
const refuse = (status, code) => { throw new Refused(status, code); };

function exactKeys(value, keys) {
  return value !== null && typeof value === 'object' && !Array.isArray(value)
    && Object.keys(value).length === keys.length && keys.every(key => Object.hasOwn(value, key));
}
const count = value => Number.isSafeInteger(value) && value >= 0 && value <= MAX_COUNT;

async function body(request) {
  const declared = Number(request.headers.get('content-length') ?? '0');
  if (declared > MAX_BODY) refuse(413, 'too_large');
  const bytes = await request.arrayBuffer();
  if (bytes.byteLength > MAX_BODY) refuse(413, 'too_large');
  try { return JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(bytes)); } catch { return refuse(400, 'invalid_json'); }
}

async function sha256(text) {
  const digest = await crypto.subtle.digest('SHA-256', new TextEncoder().encode(text));
  return [...new Uint8Array(digest)].map(b => b.toString(16).padStart(2, '0')).join('');
}

function newToken() {
  const bytes = crypto.getRandomValues(new Uint8Array(32));
  return [...bytes].map(b => b.toString(16).padStart(2, '0')).join('');
}

const day = date => date.toISOString().slice(0, 10);
const hour = date => date.toISOString().slice(0, 13);

/** A date within the last 400 days and not in the future, as a real calendar date. */
function validDate(value, now) {
  if (typeof value !== 'string' || !/^\d{4}-\d{2}-\d{2}$/.test(value)) return false;
  const parsed = new Date(`${value}T00:00:00Z`);
  if (Number.isNaN(parsed.getTime()) || day(parsed) !== value) return false;
  const earliest = new Date(now); earliest.setUTCDate(earliest.getUTCDate() - 400);
  return value <= day(now) && value >= day(earliest);
}

/** Every row must hold exactly the permitted fields, add up, and be unique (OU-BOARD-003, OU-BOARD-004). */
export function validRows(rows, now) {
  if (!Array.isArray(rows) || rows.length > MAX_ROWS) return false;
  const keys = new Set();
  for (const row of rows) {
    if (!exactKeys(row, ROW_KEYS) || !validDate(row.date, now) || !TOOLS.includes(row.tool) || typeof row.model !== 'string' || !MODEL.test(row.model)) return false;
    const parts = [row.inputTokens, row.outputTokens, row.cacheReadTokens, row.cacheWriteTokens];
    if (!parts.every(count) || !count(row.totalTokens) || parts.reduce((a, b) => a + b, 0) !== row.totalTokens) return false;
    if (!(row.estimatedValueMicros === null || count(row.estimatedValueMicros))) return false;
    const key = `${row.date}|${row.tool}|${row.model}`;
    if (keys.has(key)) return false;
    keys.add(key);
  }
  return true;
}

async function limited(db, key, limit, now) {
  const window = hour(now);
  const row = await db.prepare('SELECT count FROM limits WHERE key = ?1 AND hour = ?2').bind(key, window).first();
  if (row && row.count >= limit) return true;
  await db.prepare('INSERT INTO limits (key, hour, count) VALUES (?1, ?2, 1) ON CONFLICT (key, hour) DO UPDATE SET count = count + 1').bind(key, window).run();
  await db.prepare('DELETE FROM limits WHERE hour < ?1').bind(hour(new Date(now.getTime() - 2 * 3600_000))).run();
  return false;
}

async function device(request, db) {
  const header = request.headers.get('authorization') ?? '';
  const token = /^Bearer ([0-9a-f]{64})$/.exec(header)?.[1];
  if (!token) refuse(401, 'not_joined');
  const found = await db.prepare('SELECT user_id, device_id FROM devices WHERE token_hash = ?1').bind(await sha256(token)).first();
  return found ?? refuse(401, 'not_joined');
}

/** Joining reads the GitHub account once and discards the GitHub token (OU-BOARD-002). */
async function join(request, env, deps, now) {
  const ip = request.headers.get('cf-connecting-ip') ?? '';
  // The edge limiter sees the IP address for a moment. The database never stores it.
  if (!env.JOIN_LIMITER) refuse(503, 'limiter_unavailable');
  if (!(await env.JOIN_LIMITER.limit({ key: ip })).success) refuse(429, 'rate_limited');
  const input = await body(request);
  if (!exactKeys(input, ['githubToken', 'deviceId']) || typeof input.githubToken !== 'string' || input.githubToken.length < 20 || input.githubToken.length > 255
    || typeof input.deviceId !== 'string' || !UUID.test(input.deviceId)) refuse(400, 'invalid_request');
  const account = await deps.fetchGitHubUser(input.githubToken);
  if (!account || !Number.isSafeInteger(account.id) || account.id <= 0 || typeof account.login !== 'string' || !LOGIN.test(account.login)) refuse(401, 'github_refused');
  if (await limited(env.DB, `join:${account.id}`, JOINS_PER_HOUR, now)) refuse(429, 'rate_limited');
  await env.DB.prepare('INSERT INTO users (github_id, login, created_at) VALUES (?1, ?2, ?3) ON CONFLICT (github_id) DO UPDATE SET login = excluded.login')
    .bind(account.id, account.login, now.toISOString()).run();
  const user = await env.DB.prepare('SELECT id FROM users WHERE github_id = ?1').bind(account.id).first();
  const known = await env.DB.prepare('SELECT 1 AS known FROM devices WHERE user_id = ?1 AND device_id = ?2').bind(user.id, input.deviceId).first();
  const devices = await env.DB.prepare('SELECT count(*) AS n FROM devices WHERE user_id = ?1').bind(user.id).first();
  if (!known && devices.n >= MAX_DEVICES) refuse(409, 'too_many_devices');
  const token = newToken();
  await env.DB.prepare('INSERT INTO devices (user_id, device_id, token_hash, created_at) VALUES (?1, ?2, ?3, ?4) ON CONFLICT (user_id, device_id) DO UPDATE SET token_hash = excluded.token_hash')
    .bind(user.id, input.deviceId, await sha256(token), now.toISOString()).run();
  return json(200, { token, login: account.login });
}

/** A repeated sync replaces the same device, date, tool and model (OU-BOARD-009). */
async function sync(request, env, now) {
  const { user_id: user, device_id: deviceId } = await device(request, env.DB);
  if (await limited(env.DB, `sync:${user}`, SYNCS_PER_HOUR, now)) refuse(429, 'rate_limited');
  const input = await body(request);
  if (!exactKeys(input, ['deviceId', 'rows']) || input.deviceId !== deviceId || !validRows(input.rows, now)) refuse(400, 'invalid_request');
  const statements = input.rows.map(row => env.DB.prepare(
    `INSERT INTO days (user_id, device_id, date, tool, model, input_tokens, output_tokens, cache_read_tokens, cache_write_tokens, total_tokens, estimated_value_micros)
     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
     ON CONFLICT (user_id, device_id, date, tool, model) DO UPDATE SET input_tokens = excluded.input_tokens, output_tokens = excluded.output_tokens,
       cache_read_tokens = excluded.cache_read_tokens, cache_write_tokens = excluded.cache_write_tokens, total_tokens = excluded.total_tokens,
       estimated_value_micros = excluded.estimated_value_micros`,
  ).bind(user, deviceId, row.date, row.tool, row.model, row.inputTokens, row.outputTokens, row.cacheReadTokens, row.cacheWriteTokens, row.totalTokens, row.estimatedValueMicros));
  const dates = [...new Set(input.rows.map(row => row.date))];
  for (const date of dates) {
    statements.push(env.DB.prepare(
      `INSERT INTO flags (user_id, date) SELECT ?1, ?2 WHERE (SELECT coalesce(sum(total_tokens), 0) FROM days WHERE user_id = ?1 AND date = ?2) > ?3
       ON CONFLICT (user_id, date) DO NOTHING`,
    ).bind(user, date, DAY_CEILING));
  }
  if (statements.length) await env.DB.batch(statements);
  return json(200, { accepted: input.rows.length });
}

/** Leaving removes the data from public view and from storage at once (OU-BOARD-010). */
async function leave(request, env) {
  const { user_id: user } = await device(request, env.DB);
  await env.DB.batch([
    env.DB.prepare('DELETE FROM days WHERE user_id = ?1').bind(user),
    env.DB.prepare('DELETE FROM flags WHERE user_id = ?1').bind(user),
    env.DB.prepare('DELETE FROM devices WHERE user_id = ?1').bind(user),
    env.DB.prepare('DELETE FROM users WHERE id = ?1').bind(user),
  ]);
  return json(200, { left: true });
}

/** Ranks over 7 days, 30 days or all time, by tokens or estimated value (OU-BOARD-008). */
export async function ranks(db, period, metric, now) {
  const since = period === 'all' ? '0000-00-00' : day(new Date(now.getTime() - (period === '7d' ? 6 : 29) * 86_400_000));
  const order = metric === 'value' ? 'value DESC, tokens DESC' : 'tokens DESC, value DESC';
  const { results } = await db.prepare(
    `SELECT u.login AS login, sum(d.total_tokens) AS tokens, sum(coalesce(d.estimated_value_micros, 0)) AS value
     FROM days d JOIN users u ON u.id = d.user_id
     WHERE u.hidden = 0 AND d.date >= ?1
       AND NOT EXISTS (SELECT 1 FROM flags f WHERE f.user_id = d.user_id AND f.date = d.date AND f.cleared = 0)
     GROUP BY u.id ORDER BY ${order}, u.login LIMIT ?2`,
  ).bind(since, RANK_LIMIT).all();
  return results.map((row, index) => ({ rank: index + 1, login: row.login, totalTokens: row.tokens, estimatedValueMicros: row.value }));
}

const escape = text => String(text).replace(/[&<>"']/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c]);

/** The public page: plain HTML, every value escaped, and a policy that allows no script. */
async function page(env, now) {
  const entries = await ranks(env.DB, '30d', 'tokens', now);
  const rows = entries.map(e => `<tr><td>${e.rank}</td><td>${escape(e.login)}</td><td>${escape(e.totalTokens.toLocaleString('en-US'))}</td></tr>`).join('');
  const html = `<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><title>OpenUsage leaderboard, ${TITLE}</title></head>
<body><h1>OpenUsage leaderboard, ${TITLE}</h1><p>Tokens from local logs in the last 30 days. Values are self-reported and cannot be proven.</p>
<table><thead><tr><th>Rank</th><th>GitHub login</th><th>Tokens</th></tr></thead><tbody>${rows}</tbody></table></body></html>`;
  return new Response(html, {
    headers: {
      'content-type': 'text/html; charset=utf-8',
      'content-security-policy': "default-src 'none'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'",
      'x-content-type-options': 'nosniff',
      'referrer-policy': 'no-referrer',
    },
  });
}

export async function fetchGitHubUser(token) {
  const response = await fetch('https://api.github.com/user', {
    headers: { authorization: `Bearer ${token}`, accept: 'application/vnd.github+json', 'user-agent': 'OpenUsage-Leaderboard' },
  });
  if (!response.ok) return null;
  const user = await response.json();
  return { id: user.id, login: user.login };
}

export async function handle(request, env, deps = { fetchGitHubUser }, now = new Date()) {
  try {
    const { pathname, searchParams } = new URL(request.url);
    const route = `${request.method} ${pathname}`;
    if (route === 'POST /v1/join') return await join(request, env, deps, now);
    if (route === 'POST /v1/sync') return await sync(request, env, now);
    if (route === 'POST /v1/leave') return await leave(request, env);
    if (route === 'GET /v1/leaderboard') {
      const period = searchParams.get('period') ?? '30d', metric = searchParams.get('metric') ?? 'tokens';
      if (!['7d', '30d', 'all'].includes(period) || !['tokens', 'value'].includes(metric) || [...searchParams.keys()].some(k => !['period', 'metric'].includes(k))) refuse(400, 'invalid_request');
      return json(200, { title: TITLE, period, metric, entries: await ranks(env.DB, period, metric, now) });
    }
    if (route === 'GET /') return await page(env, now);
    return json(404, { error: 'not_found' });
  } catch (error) {
    if (error instanceof Refused) return json(error.status, { error: error.code });
    return json(500, { error: 'unavailable' });
  }
}

export default { fetch: (request, env) => handle(request, env) };
