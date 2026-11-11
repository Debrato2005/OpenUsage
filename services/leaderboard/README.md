# Leaderboard service

The opt-in OpenUsage leaderboard (SRS Section 14A), as one Cloudflare Worker and one D1 database. The [architecture decision](../../docs/decisions/2026-11-07-leaderboard-service.md) sets every rule here. The desktop app and the CLI work fully without this service (OU-BOARD-011).

## API

| Request | Result |
|---|---|
| `POST /v1/join` with `{"githubToken", "deviceId"}` | Reads the GitHub user ID and login once, discards the GitHub token, and returns a 256-bit service token for this device. Only its SHA-256 hash is stored |
| `POST /v1/sync` with `Authorization: Bearer <token>` and `{"deviceId", "rows"}` | Replaces each row for the same device, date, tool and model (OU-BOARD-009) |
| `POST /v1/leave` with `Authorization: Bearer <token>` | Deletes the user, every device, and every row at once (OU-BOARD-010) |
| `GET /v1/leaderboard?period=7d\|30d\|all&metric=tokens\|value` | The top 100, titled Self-reported (OU-BOARD-007, OU-BOARD-008) |
| `GET /` | The same ranks as plain HTML. Every value is escaped, and the content security policy allows no script |

A row holds only `date`, `tool`, `model`, `inputTokens`, `outputTokens`, `cacheReadTokens`, `cacheWriteTokens`, `totalTokens` and `estimatedValueMicros` (OU-BOARD-003). An unknown field, a wrong type, totals that do not add up, a duplicate key, a date outside the last 400 days, an unknown tool, or a model outside lowercase letters, digits, `.`, `-` and `:` rejects the whole request. A request is at most 64 KiB and 400 rows.

For each user and period, `estimatedValueMicros` is `null` if any included row has no estimate. Token totals remain available. A measured zero stays `0`. Value ranks put numeric estimates before unavailable estimates, then use token totals and login to break ties.

## Limits

- The edge limiter allows 3 joins a minute for each IP address. Cloudflare's rate limiting binding counts only 10-second or 60-second periods, and the database never stores an IP address.
- Each GitHub account can join 3 times an hour, and each user can sync 30 times an hour. These hourly counts are kept in D1 by account, not by IP address.
- A user can have at most 5 devices.
- A user day above 2 billion tokens is flagged and left out of the ranks until a maintainer clears it.

## Moderation

The [moderation policy](MODERATION.md) says how entries are reviewed. There is no admin endpoint. Maintainers hide an entry or clear a flag with the D1 command line under their Cloudflare login, for example:

```sh
wrangler d1 execute openusage-leaderboard --remote --command "UPDATE users SET hidden = 1 WHERE login = 'example'"
wrangler d1 execute openusage-leaderboard --remote --command "UPDATE flags SET cleared = 1 WHERE user_id = 1 AND date = '2026-11-08'"
```

## Deploy

The owner does these steps (checklist item G-L):

1. Create a Cloudflare account and a D1 database named `openusage-leaderboard`, then put its ID in `wrangler.toml`.
2. Apply the schema: `wrangler d1 migrations apply openusage-leaderboard --remote`.
3. Deploy: `wrangler deploy`.

## Tests

`node --test services/leaderboard/test/worker.test.mjs` runs the Worker against SQLite through a small D1 stand-in, with GitHub as a stub and no network.
