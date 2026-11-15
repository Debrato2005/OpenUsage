# Online leaderboard CLI review

Recorded November 15, 2026. Scope: L-5a, OU-BOARD-001 through OU-BOARD-006 and OU-BOARD-010, plus the affected CLI requirements.
The owner authorized autonomous implementation, bounded blocker agents, and reviewed branch pushes.
No deployment, release, or pull request is authorized.

## Implemented boundary

The Rust CLI joins through GitHub device authorization, previews exact saved rows, syncs, reconnects, and leaves.
The shared session owns origin-bound OS persistence and a stable pending device ID.
The transport keeps its existing HTTPS policy.
The [CLI contract](../../crates/openusage-cli/README.md#online-leaderboard-commands) owns commands, output fields, and exit codes.
The [enrollment contract](../../crates/openusage-board/README.md#serialized-enrollment) owns recovery behavior.
The [keys contract](../../crates/openusage-keys/README.md#credential-coordination) owns coordination and expected-value replacement.

## Independent review

The specification reviewer found two concrete gaps.
Application-data paths could select separate locks for one credential slot.
Production sessions now use one Linux UID-based guard with no environment-selected fallback.
Native bus validation also accepted a path prefix that could escape the private fixture.
All touched native callers now use the shared validator before any keyring call.
The validator binds canonical owned private directories and the exact owned socket address.
The reviewer cleared both corrections through static source review.

The standards reviewer required current contract owners and suggested typed command dispatch.
The changed contract owners now describe the implemented boundary.
The online caller uses a typed command enum with explicit output cases.
The reviewer found no remaining concrete violation in the corrected paths.
Both reviews were static only. Neither reviewer ran tests or builds.
An Astra agent at extra-high effort implemented the guard and resolved its namespace blocker.
The primary agent owns integration, documentation, verification, and Git.

## Local evidence

The private Linux CLI journey uses compiled native-test binaries, SQLCipher, GNOME Secret Service, and a local HTTPS stub.
It checks consent, no request before enrollment, exact dry-run bytes, repeated sync, and saved-file preservation.
A failed OS save leaves one pending device ID. Explicit resume reuses it.
A concurrent process with another XDG data path refuses.
The typed active credential survives a private daemon restart.
Rejected leave keeps enrollment. Reconnect keeps the same device ID.
Confirmed leave clears the selected enrollment.
Token strings never reach command output.
Native traversal and fallback bus addresses refuse before credential or network access.

The ordinary unconfigured build also ignores valid native endpoint settings and makes no request.

The separate vault journey checks same-item replacement, restart, isolation, malformed and duplicate records, locks, and unavailable service.
Guard tests use private synthetic files and actual competing processes.
Production runtime policy tests use private synthetic runtime directories.
The end-to-end CLI journey uses an explicitly private fixture lock path.
It does not test a configured production command against the host runtime or a live account.

GitHub's current device-flow documentation permits an optional expiry and refresh-token triplet.
The parser accepts a complete valid triplet, refuses malformed combinations, and discards the refresh credential.
The access token is consumed once by join.
The source is [GitHub's authorization documentation](https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/authorizing-oauth-apps), read on November 15, 2026.

## Open gates

Settings integration, hosted GitHub and Cloudflare acceptance, and other platforms remain open.
[SC-029](../srs-clarifications.md#sc-029-local-deletion-and-public-enrollment) records the separate local-erasure gap after enrollment.
The service is not deployed. Missing build configuration keeps ordinary commands inert.
Renderer behavior did not change in this slice.
The HTTP and TLS libraries can retain internal secret copies.
The guard coordinates OpenUsage processes, not other programs under the same OS user.

## Authorization encoder correction

The final source audit found allocation growth after secret writes in the authorization encoders.
A supported maximum GitHub token makes a 323-byte join body.
The serializer's old initial capacity was 128 bytes.
The old form encoder could also grow after it wrote the device code.
An Astra blocker agent replaced join encoding with a fixed zeroizing writer.
The form encoder now computes its exact capacity before it writes a secret.
Four regressions cover wire bytes, supported boundaries, escaping, and independent overflow refusal.
This bound covers the application's encoders, not internal library copies.
