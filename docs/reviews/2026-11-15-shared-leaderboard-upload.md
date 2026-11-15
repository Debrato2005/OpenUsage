# Shared leaderboard upload review

Recorded November 15, 2026. Scope: L-5b0, OU-BOARD-005/006, and the affected CLI and storage boundaries.
The normative SRS remains unchanged.

The upload builder now belongs to Rust storage instead of the CLI.
`Storage::leaderboard_upload` owns one read transaction on read-write and query-only connections.
It returns an owned string after the transaction ends.
Callers must release the storage lock before consent or transport.
The CLI keeps its existing error codes, messages, upload fields, and exact byte order.
No new database schema, provider access, scanner, credential operation, or network path is added.
Settings integration remains unbuilt.

Three storage tests check the new public boundary with private synthetic SQLCipher databases.
They compare both connection modes and reject writes.
A concurrent writer commits between scope and row reads.
The first upload must retain the old snapshot, and the next upload must contain the new counts.
A denied read must refuse the upload and release its transaction before a later write.
The existing CLI acceptance checks retain their independent expected fields and limits.

The first compile found a stale self-crate path after extraction.
The requested Astra agent corrected that reference and checked the module for other stale caller paths.
The agent ran no build and changed only the selected module.

The first focused test run stopped in fixture setup.
Its timestamp omitted the milliseconds required by the existing storage validator.
A second Astra agent corrected only that synthetic constant.
Production validation and all test assertions remain unchanged.

The private Linux CLI journey passes on the extracted builder.
Its real sync sends the exact preview bytes after credential-daemon restart and repeated uploads.
All 25 CLI unit and process tests pass.
All three new storage tests pass with canonical synthetic timestamps.
A controlled regression removes the transaction and returns the concurrent writer's new count instead of the old snapshot.
The test fails as expected. The original source is restored byte for byte before the final checks.

Both independent source reviews close after the crate-path correction and the architecture update.
They ran no builds or tests.
The shared-builder documentation draft scores 0.43 findings per 100 words.
Settings, hosted acceptance, and other platforms remain separate gates.

The final milestone passes full workspace tests, all-target Clippy, formatting, dependency policy, and staged whitespace checks.
The ordinary unconfigured build ignores native endpoint settings and makes no request or saved-data change.
The AST graph contains 15,585 nodes and 49,046 edges after this source change.
The older wiki remains navigation only.
Renderer source does not change in this slice.
