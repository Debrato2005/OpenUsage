# Leaderboard enrollment contract

November 15, 2026 · L-5a3 · Private synthetic Linux credentials.

The [client contract](../../crates/openusage-board/README.md#enrollment-credential-format)
owns the implemented format. This slice adds a pure codec, with no network,
OS-store, CLI, or desktop caller. Online enrollment remains open.

## Source research

GitHub documents device authorization without a client secret. Its minimum poll
interval, slowdown response, and expiry govern the existing flow. GitHub also
documents optional expiration and refresh fields in token responses. The
current strict transport refuses that additional shape. This remains a separate
compatibility gate before an expiring-token app configuration can work.
[GitHub authorization documentation](https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/authorizing-oauth-apps#device-flow).

Stored credentials do not prove that the service still accepts them. Revoked
GitHub tokens cannot regain validity. The service exchanges the GitHub token
for a device credential, so the saved envelope needs no GitHub refresh
credential. [GitHub expiration and revocation documentation](https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/token-expiration-and-revocation)
and the [service API](../../services/leaderboard/README.md#api).

Both official pages were retrieved through local-research on November 15.
The complete extracted text was read. The first page used two consecutive
windows, and the second used one. Original-page visuals were not inspected.
Search succeeded through DuckDuckGo. Brave returned a rate-limit failure.
No live sign-in, existing credentials, or real provider logs were used.

## Checks

Eight public API tests pass. They check origin binding, round trips, required
fields, duplicate and unknown fields, nulls, types, bounds, and secret redaction.
They also reject escaped keys and values, non-ASCII input, and complete
positional arrays. A 257-byte record checks encoding at the identified buffer
boundary. The fixed output buffer cannot grow during serialization.

Independent standards and SRS reviews closed their findings after the buffer
and parser corrections. These reviews inspected source only. Full workspace
and Clippy results remain in [traceability](../acceptance-traceability.md).

The actual typed record has not passed an OS-store round trip. Enrollment
coordination across processes, online callers, revocation handling, exact real
upload comparison, hosted access, and other platforms remain open.
