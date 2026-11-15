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
settings exist and pass validation. The ordinary-build API has no URL setter. GitHub
device endpoints remain fixed. Callers do not supply credentials in URLs.

The client requires TLS 1.2 or later and keeps connection byte logging disabled.
Certificate and hostname validation remain enabled.

The client blocks its calling thread. Desktop callers must use an owned blocking
task outside the UI thread and storage lock. The transport has no database, browser launcher, or scheduler.
The separate session coordinator below owns the OS credential boundary.

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
Join encoding uses a fixed zeroizing writer. OAuth forms reserve exact capacity before secret writes.
HTTP, TLS, and parser libraries can retain internal secret copies.
GitHub form requests use the crate's small encoder into a zeroized buffer.
The reqwest `form` feature stays disabled because this path does not use it.

## Exact uploads and rank values

The [shared storage builder](../../docs/data-contracts.md#shared-leaderboard-upload) remains the sole payload builder.
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
overrides exist only under `cfg(test)` or `native-test`.
Native CLI acceptance also checks the private fixture before credential access.

The TLS test requires Python 3 with `ssl` and the `openssl` executable. It reports
missing prerequisites as failures. It uses fixed argument lists, private temporary
files, stdout readiness, and bounded child cleanup. No test uses a real GitHub
login, provider home, OS credential store, or cloud service.

The request-deadline case shortens the timeout only in the test configuration.
Production requests keep the five-second connect and fifteen-second total
budgets. Hosted GitHub, Cloudflare, other platforms, and Settings integration need
separate acceptance.

## Enrollment credential format

`Enrollment` is a pure Rust codec for the leaderboard credential slot.
It performs no network, file, database, or OS-store operation. The caller supplies
the configured service origin when it restores a credential. A stored URL never
selects the destination.

Version 1 contains only `schemaVersion`, `serviceOrigin`, `deviceId`, `login`,
and `serviceToken`. The codec reuses the transport's canonical HTTPS origin
rules and permits at most 2,048 origin bytes. The stored origin includes its
final slash. The ID passes lowercase UUIDv4 validation. The enrollment caller
must generate it randomly. The login is descriptive metadata, not provider
identity or proof of current authorization. The token keeps the existing
service-token type and its validation.

The envelope is an ASCII JSON object of at most 4,096 bytes. Missing, unknown,
duplicate, null, invalid, and wrongly typed fields refuse. Escapes and positional
arrays refuse before deserialization. The encoder writes through a fixed
zeroizing buffer. Input bytes, output bytes, and owned service tokens are
zeroized on drop. Debug output is redacted, and errors never contain the input.
Deliberate Rust access supplies the service token to the existing transport.
The envelope has no general serializer or display implementation.

Eight public API tests cover round trips, origin binding, byte limits, field
refusal, secret handling, and a 257-byte record. These tests use synthetic
credentials without network or OS-store access. The CLI journey below
checks the typed OS-store boundary with separate synthetic credentials. No GitHub access token, refresh token, provider credential,
or source history belongs in this envelope.

## Serialized enrollment

EnrollmentSession coordinates the OS credential record and the lifetime guard.
The transport keeps its HTTP policy and owns no OS-store operation.
The [keys contract](../openusage-keys/README.md#credential-coordination) owns the shared lock and replacement rules.

Join saves a random UUIDv4 pending record before the first remote authorization.
An explicit resumed join reuses the same ID.
The pending record contains schemaVersion, phase, serviceOrigin, and deviceId.
Its phase is pending. It contains no service token or GitHub credential.
The active record keeps the existing version 1 enrollment format.
Both states bind the configured origin before any network request.
A stored origin never chooses a destination.

A successful remote join replaces the exact expected OS item without a delete gap.
If saving fails, the saved pending or previous record retains the device ID.
The CLI reports a partial result and requires explicit recovery.
It never calls leave as a rollback.
Reconnect reuses the saved device ID and requires explicit consent.
An account change does not delete the previous account's public data.
Login remains descriptive metadata, not verified identity.

GitHub can return an optional expiry and refresh-token triplet.
The client accepts a complete valid triplet and refuses partial or malformed fields.
It consumes the access token once and discards the refresh credential.
No GitHub credential enters the enrollment record.


See the [native CLI journey](../../tests/native/README.md#native-leaderboard-cli) for its executed boundary.

## Departure recovery

`leave_with_github(token, local_token)` consumes fresh GitHub authorization without joining.
It creates no enrollment or device slot.
An optional service token supplies a cleanup witness, not departure authority.
The configured service origin remains the only destination.

The response requires an object with exactly `left`, `login`, and `localTokenRevoked`.
The witness must be present, including explicit `null` when no service token was supplied.
A supplied token requires a boolean witness.
Only `true` permits cleanup of that exact local credential.
A login or device UUID never proves ownership.
Missing, duplicate, unknown, wrongly typed, and positional-array replies refuse.
All transport replies require JSON objects.

The recovery encoder uses a fixed zeroizing writer of 273 bytes.
Join keeps its separate 323-byte bound.
Neither writer grows after secret writes.
The [CLI contract](../openusage-cli/README.md#departure-recovery-and-local-forgetting) owns consent and local outcomes.
