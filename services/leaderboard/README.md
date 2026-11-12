# Leaderboard service

The opt-in OpenUsage leaderboard (SRS Section 14A), as one Cloudflare Worker and one D1 database. The [architecture decision](../../docs/decisions/2026-11-07-leaderboard-service.md) sets every rule here. The desktop app and the CLI work fully without this service (OU-BOARD-011).

## API

| Request | Result |
|---|---|
| `POST /v1/join` with `{"githubToken", "deviceId"}` | Reads the GitHub user ID and login once, discards the GitHub token, and returns a 256-bit service token for this device. Only its SHA-256 hash is stored |
| `POST /v1/sync` with `Authorization: Bearer <token>` and `{"deviceId", "rows"}` | Replaces each row for the same device, date, tool and model (OU-BOARD-009) |
| `POST /v1/leave` with `Authorization: Bearer <token>` | Deletes the user, every device, and every row at once (OU-BOARD-010) |
| `GET /v1/leaderboard?period=7d\|30d\|all&metric=tokens\|value` | The top 100, titled Self-reported (OU-BOARD-007, OU-BOARD-008) |
| `GET /v1/me/rank?period=7d\|30d\|all&metric=tokens\|value` with `Authorization: Bearer <token>` | The joined user's rank across all eligible participants, including positions beyond 100 |
| `GET /` | The same ranks as plain HTML. Every value is escaped, and the content security policy allows no script |

A row holds only `date`, `tool`, `model`, `inputTokens`, `outputTokens`, `cacheReadTokens`, `cacheWriteTokens`, `totalTokens` and `estimatedValueMicros` (OU-BOARD-003). An unknown field, a wrong type, totals that do not add up, a duplicate key, a date outside the last 400 days, an unknown tool, or a model outside lowercase letters, digits, `.`, `-` and `:` rejects the whole request. A request is at most 64 KiB and 400 rows.

For each user and period, `estimatedValueMicros` is `null` if any included row has no estimate. Token totals remain available. A measured zero stays `0`. Value ranks put numeric estimates before unavailable estimates, then use token totals and login to break ties.

`GET /v1/me/rank` uses the same device token as sync (OU-BOARD-013, OU-BOARD-014). It returns `title`, `period`, `metric`, `snapshotAt`, `totalParticipants`, and `entry`. The UTC timestamp `snapshotAt` records the request time for one database snapshot. `totalParticipants` counts all users with eligible rows for that period. `entry` has the same fields as a public entry, with the ordinal rank across all eligible participants. Its rank can exceed 100.

A joined user without eligible rows receives `entry: null`. Hidden users and users with only flagged dates receive the same response shape, without moderation details.

Both rank routes default to `period=30d&metric=tokens`. Invalid or duplicate selectors and unknown query parameters return `400 invalid_request`. Each period includes the current UTC date and excludes later dates. The 7-day and 30-day periods include the previous 6 and 29 dates. All time includes every stored date through the current date.

Ranks use distinct ordinal positions. Token ranks sort by token total, estimated value, and login. Value ranks sort by estimated value, token total, and login. Totals sort from highest to lowest. Numeric estimates precede unavailable estimates.

Logins sort in SQLite's binary order. The internal user ID breaks any remaining tie. Public and authenticated reads share this order and the same row filters.

SQLite computes integer sums and returns their decimal text before the Worker converts them to JSON numbers. A returned aggregate above 9,007,199,254,740,991 or a SQLite integer overflow returns `500 unavailable`. This preserves exact values within the JSON number contract. The v1 service API keeps its existing numeric wire format. Accepted rows and retained history can exceed those aggregate bounds. An unsafe own entry returns the same error even when it falls below the public top 100.

Rust clients must convert counts and estimated values to decimal strings before the frontend boundary (OU-NUM-004). Website adapters must do the same before frontend calculation.

The service checks the device token before the aggregate query, then checks it again within the snapshot query. Leaving or token rotation before that snapshot returns `401 not_joined`. Rank reads preserve the existing hourly counters for joins and syncs.

## Limits

The service reads request bytes into a fixed 64 KiB buffer. It cancels the stream when the next chunk exceeds the remaining space. It rejects a declared length above the cap before it reads the body. Missing or smaller length headers do not bypass the cap. Invalid JSON or UTF-8 returns `400 invalid_json`. Stream read errors return `500 unavailable`.

A failed or stalled cancellation does not delay the overflow response.

- The edge limiter allows 3 joins a minute for each IP address. Cloudflare's rate limiting binding counts only 10-second or 60-second periods, and the database never stores an IP address.
- Each GitHub account can join 3 times an hour, and each user can sync 30 times an hour. These hourly counts are kept in D1 by account, not by IP address.
- A user can have at most 5 devices.
- A user day above 2 billion tokens is flagged and left out of the ranks until a maintainer clears it.

Each hourly counter uses one conditional SQL update. Concurrent requests cannot spend the same remaining allowance.
The device limit and token write use one conditional SQL statement. A known device can rejoin at the cap and rotate its token.
Database errors return `500 unavailable`. A rejected hourly request returns `429 rate_limited`. A new device at the cap returns `409 too_many_devices`.

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

The tests control concurrent writes to check hourly limits and the device cap. They also check token rotation, hour changes, and database errors.
The tests check exact byte limits, split UTF-8, early cancellation, stream errors, and cancellation errors for joins and authenticated syncs.
The tests compare public and authenticated ranks across 105 participants, ties, periods, device sums, unavailable estimates, measured zero, hidden users, and flagged dates. They also check query refusal, token rotation, leaving, revocation during a read, database errors, and clock changes. Synthetic syncs test exact token and value sums at the JSON integer limit and refusal above it.
These tests use synthetic streams and local SQLite on Linux. Hosted Workers, D1, and deployment remain unverified.
