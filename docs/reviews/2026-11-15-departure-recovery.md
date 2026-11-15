# Departure recovery review

Recorded November 15, 2026. Scope: L-5a6, SC-030, OU-BOARD-001/002/010, and affected CLI and security boundaries.
The normative SRS remains unchanged.

## Recovery effects

Fresh GitHub authorization selects the account by numeric GitHub ID.
The new departure route creates no user, device, or token.
It works at the device cap and treats an already absent account as removed.
The optional service bearer is only a witness for local cleanup.
The response requires an explicit nullable witness, and the client refuses a missing field.
Only a matching token permits removal of the exact expected local record.
Matching display names and device UUIDs cannot prove account identity.

The CLI keeps its shared guard through authorization, departure, and local cleanup.
An unavailable store still permits fresh remote departure and reports unknown local state.
Pending and unrelated local records remain.
Cleanup failure after confirmed departure gives a partial result.
Separate explicit local forgetting clears the selected slot without service configuration or network access.
It requires usable storage and never treats an unavailable store as empty.

## Transaction and retention corrections

The review reproduced stale internal user ID reuse during join, sync, and leave.
Credential-qualified statements now resolve identity in the same transaction as mutations.
Revocation before sync refuses even when the upload has no rows.
Departure cascades account data in that transaction.
A forced failure after deletion proves rollback in the local SQLite stand-in.

Sync limits use stable GitHub IDs.
Fresh departure has a separate allowance and cannot restore the join allowance.
An hourly scheduled handler removes old counters without waiting for traffic.
Successful timely scheduling limits counter retention to less than three hours.
Hosted schedule behavior remains unverified.
The [service contract](../../services/leaderboard/README.md) links the official D1 and schedule sources read for these decisions.

## Executed evidence

Forty-eight service tests pass against synthetic SQLite and a GitHub stub.
Fifty-one focused Rust and CLI tests pass.
They include required-nullable witness refusal, exact request bytes, bounded encoding, consent, and command parsing.
The native Linux journey uses a compiled native-test CLI, private GNOME Secret Service, SQLCipher, and synthetic HTTPS endpoints.
It proves recovery without a store, same-login account isolation, failed cleanup, pending-record preservation, and explicit local forgetting.
The journey preserves saved database files and keeps secrets out of command output.

Independent reviewers found no concrete specification defect, documented-standard breach, or actionable structural smell.
Both reviews read source only and ran no tests.
Native-feature Clippy passes with warnings denied.

Documentation prose scores 0.71 and 0.63 findings per 100 words in the two draft groups.

Full workspace tests pass in 446.947 seconds.
The final milestone also passes all-target workspace Clippy, formatting, service tests, native syntax, and staged whitespace checks.
The ordinary unconfigured CLI ignores valid native endpoints and makes no request or saved-data change.
The AST graph is current. Its older wiki remains navigation only.
No renderer or desktop application source changes in this slice.

## Limits

No live GitHub account, hosted D1, Cron Trigger, production runtime, or other platform is qualified.
The renderer and desktop application source do not change in this slice.
Settings integration remains open.
Raw test output and private evidence remain outside Git.
