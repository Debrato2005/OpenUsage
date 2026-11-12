# Leaderboard HTTP client

Recorded November 13, 2026. The primary agent selected this design under the owner's explicit authorization to research blockers and continue implementation. This decision covers the public leaderboard client only. It does not approve private-sync hosting or deployment.

## Decision

Use the already locked `reqwest` 0.13.5 in a reusable Rust `openusage-board` crate. Disable default features. Enable `blocking` and `native-tls-vendored`. Select the native TLS backend explicitly. GitHub forms use a bounded encoder whose owned buffers are zeroized. The unused reqwest `form` feature stays disabled. CLI calls can block their command thread. Desktop calls must use an owned blocking task outside the UI thread and storage lock.

The native TLS choice reuses the workspace's OpenSSL build on Linux. Other platforms still need their own build and acceptance checks. A new transport does not qualify those platforms.

The implementation shall:

- Require an explicitly configured HTTPS service origin and public GitHub OAuth client ID. An unconfigured build makes no request.
- Keep GitHub device endpoints fixed. Reject credentials, query strings, and fragments in the service origin. Never accept a response-selected service URL.
- Disable redirects, system and environment proxies, transparent decompression, and automatic HTTP retries.
- Set a five-second connect deadline and fifteen-second total request deadline. Read at most 64 KiB plus one refusal byte. Reject oversized request and response bodies.
- Validate the exact upload whitelist before transmission and preserve accepted bytes unchanged. The saved-data builder remains the only payload producer.
- Keep tokens in opaque zeroized Rust values. Errors and debug output contain no tokens, response bodies, provider data, or credential-bearing URLs.
- Require TLS 1.2 or later, certificate and hostname validation, and disabled connection byte logging. Loopback endpoints and synthetic trust roots remain test-only.

GitHub device authorization requests no repository or email scope. Polling obeys the returned interval, slowdown, and expiry. A terminal or expired flow does not restart itself. This protocol polling is distinct from transport retries.

The module does not own credential persistence, local database reads, publication, or provider sign-in. Those callers need separate integration acceptance. The existing service and GitHub application still need owner setup before a live join can work.

## Evidence and acceptance

The [versioned reqwest feature documentation](https://docs.rs/reqwest/0.13.5/reqwest/#optional-features) and [client builder](https://docs.rs/reqwest/0.13.5/reqwest/blocking/struct.ClientBuilder.html) define the selected controls. The [GitHub device-flow documentation](https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/authorizing-oauth-apps#device-flow) defines authentication timing. These sources were read on November 13, 2026.

Acceptance must exercise real local HTTP/TLS traffic with synthetic credentials. It covers request paths, exact upload bytes, certificate rejection, redirects, size and time limits, authentication states, and safe errors. Hosted GitHub and Cloudflare behavior remains separate from local acceptance.
