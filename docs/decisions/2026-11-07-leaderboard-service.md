# Leaderboard service architecture

Recorded November 7, 2026. Status: approved. The owner asked for the leaderboard in the first release and delegated the hosting choice ("do whatever is the best"), on the condition that the open-source service must not invite malicious activity. This record is the architecture decision that OU-BOARD-011 and the SRS decision boundary in Section 2 require.

## Decision

The Leaderboard Service is a Cloudflare Worker with one Cloudflare D1 database. Its source lives in `services/leaderboard/` in this repository. The owner deploys it to the owner's Cloudflare account. The desktop app and CLI work fully without it (OU-BOARD-011).

Cloudflare was chosen over the alternatives because it has no server to patch, a free tier that covers launch traffic, built-in rate limiting at the edge, and a small attack surface. Supabase would add PostgreSQL, which the SRS decision boundary bars. A GitHub-only design, with pull requests or gists as submissions, cannot enforce replacement, limits or removal on leaving. A self-hosted server adds patching, TLS and backups to the owner's work.

## Identity

- Joining uses the GitHub OAuth device flow (OU-BOARD-002) with the public client ID of an OAuth App that the owner registers. No client secret exists anywhere, so none can leak from the open-source client.
- The client sends the GitHub token once, to `POST /v1/join`. The service reads the numeric GitHub user ID and login from `api.github.com/user`, then discards the GitHub token. It never stores it.
- The service returns a random 256-bit service token. It stores only the token's SHA-256 hash. The client keeps the token in the OS credential store.
- Leaving (`POST /v1/leave`) deletes the user's rows and token at once, which is sooner than OU-BOARD-010 requires. The user can also revoke the OAuth App in GitHub.

## What an upload may contain

An upload holds only the fields that OU-BOARD-003 permits, from local log totals only (OU-BOARD-006):

| Field | Rule |
|---|---|
| `deviceId` | A random UUID created on joining; not derived from hardware |
| `date` | A provider date within the last 400 days, and not in the future |
| `tool` | One value from a fixed list, such as `claude-code` or `codex` |
| `model` | Lowercase letters, digits, `.`, `-` and `:`, at most 64 characters |
| token counts | Input, output, cache read, cache write and total, as integers from 0 to 10<sup>12</sup> |
| `estimatedValueMicros` | An integer from 0 to 10<sup>12</sup>, or null for an unknown model |

Requests are limited to 64 KiB and 400 rows. Unknown fields, wrong types, totals that do not add up, and duplicate keys reject the whole request. Nothing else is accepted, so no prompt, code, path, project, email, account ID or device name can arrive (OU-BOARD-004).

## Abuse resistance

- **No free text.** Public pages show the GitHub login, ranks, token totals and estimated value. They never show model names or other uploaded strings, so the service cannot be used to publish messages.
- **Replacement, not accumulation.** Rows are keyed by user, device, date, tool and model, and a repeated sync replaces them (OU-BOARD-009). Replaying a request cannot inflate a total.
- **Rate limits.** At most 3 joins per IP address per hour, and 30 syncs per user per hour, enforced at the edge. More than 5 devices per user are refused.
- **Plausibility flags.** A day above a fixed ceiling, set initially at 2 billion tokens for one user, is flagged and left out of rankings until a maintainer reviews it. The title says Self-reported (OU-BOARD-007), because no total can be proven.
- **No admin endpoint.** Maintainers hide or remove entries with the D1 command line under their Cloudflare login. No web endpoint can be attacked to gain moderation powers.
- **Output safety.** The API returns JSON only. The public page escapes every value and sends a strict Content Security Policy.
- **No telemetry.** The service keeps no request logs of its own. Cloudflare's rate limiting sees IP addresses transiently; the database never stores them.

## What the owner must do before launch

1. Create a Cloudflare account and a D1 database.
2. Register a GitHub OAuth App with device flow enabled, and put its public client ID in the client build configuration.
3. Deploy with `wrangler deploy` from `services/leaderboard/`.
4. Publish the moderation policy beside the service source.

Until then, the client's Join control stays unavailable, and `openusage board sync --dry-run` still prints the exact upload (OU-BOARD-005).
