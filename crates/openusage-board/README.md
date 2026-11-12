# Leaderboard client

`openusage-board` is the shared Rust transport for leaderboard authorization,
join, sync, leave, and the user's own rank. The
[HTTP client decision](../../docs/decisions/2026-11-13-leaderboard-http-client.md)
owns the transport policy. The
[service contract](../../services/leaderboard/README.md) owns the v1 API.

## Configuration

`BoardClient::from_build_config()` reads two compile-time settings:

- `OPENUSAGE_BOARD_ORIGIN`: the canonical HTTPS service origin, with an optional trailing slash.
- `OPENUSAGE_GITHUB_OAUTH_CLIENT_ID`: the public GitHub OAuth application ID.

Missing settings produce an inert client. No method sends a request until both
settings exist and pass validation. The public API has no URL setter. GitHub
device endpoints remain fixed. Callers do not supply credentials in URLs.

The client requires TLS 1.2 or later and keeps connection byte logging disabled.
Certificate and hostname validation remain enabled.

The client blocks its calling thread. Desktop callers must use an owned blocking
task outside the UI thread and storage lock. The crate has no database, key
store, browser launcher, or scheduler.

## Authorization

`device_start(now)` starts one explicit flow. `now` is a monotonic elapsed
`Duration`. Every later `device_poll(&mut flow, now)` uses the same clock.
The caller presents `flow.user_code()` and the fixed verification URI.
The private device code never leaves the transport API.

The flow exposes its expiry and next permitted poll time. Early or backward
clock values make no request. Pending replies keep the interval. A slowdown
adds at least five seconds and honors a larger returned interval. Terminal and
expired flows never restart themselves. A failed request also reserves the next
poll interval.

The request asks for no OAuth scope. Only an empty returned scope can produce a
`GithubToken`. Tokens with old repository, email, or administrative grants refuse
authorization. The token has no public constructor, serializer, display method,
or raw-value accessor. `join(token, &device_id)` consumes it once.

`ServiceToken::from_secret` restores a token from the caller's OS credential
store. `expose()` gives deliberate access for credential storage. Its caller
owns persistence and revocation. Neither secret type implements `Display` or
`Serialize`. Debug output redacts secrets, flows, joined identity, and rank data.
The crate zeroizes its owned secret strings and transient HTTP buffers.
The HTTP and TLS libraries can retain their own internal copies.
GitHub form requests use the crate's small encoder into a zeroized buffer.
The reqwest `form` feature stays disabled because this path does not use it.

## Exact uploads and rank values

The saved-data serializer remains the sole payload builder.
`ApprovedSyncBody::from_json(bytes, now)` checks the existing strict v1 whitelist,
UUID, calendar dates, counts, sums, unique row keys, and service limits. It keeps
the accepted bytes unchanged. `sync(&token, &body)` sends those exact bytes and
requires the accepted row count to match. It does not fill missing values or
build another payload.

`own_rank(&token, period, metric)` checks the returned title, selected period and
metric, timestamp, login, rank, and participant count. V1 numeric values must be
safe integers no larger than 9,007,199,254,740,991. The Rust API retains exact
`u64` values and nullable estimates. Callers can send decimal strings across IPC.
A null entry stays unavailable.

## Acceptance

Tests call the public methods against real loopback HTTP servers. A private
synthetic certificate and Python TLS server test normal certificate rejection
and an explicitly trusted test certificate. TLS trust changes and HTTP endpoint
overrides exist only under `cfg(test)`.

The TLS test requires Python 3 with `ssl` and the `openssl` executable. It reports
missing prerequisites as failures. It uses fixed argument lists, private temporary
files, stdout readiness, and bounded child cleanup. No test uses a real GitHub
login, provider home, OS credential store, or cloud service.

The request-deadline case shortens the timeout only in the test configuration.
Production requests keep the five-second connect and fifteen-second total
budgets. Hosted GitHub, Cloudflare, other platforms, and caller integration need
separate acceptance.
