# Settings leaderboard review

Recorded November 15, 2026 (UTC). Scope: L-5b and the affected leaderboard, credential, CLI, IPC, deletion, and shutdown boundaries.
The normative SRS remains unchanged.

Settings now offers explicit GitHub join, reconnect, exact upload review, global departure, fresh GitHub departure, and local forgetting.
The collapsed section makes no status call. Status inspection makes no service request.
An unconfigured build keeps enrollment unchecked and still permits explicit local forgetting.
The [architecture](../architecture.md#settings-leaderboard-worker) owns worker ordering and shutdown.
The [data contract](../data-contracts.md#settings-exchange-contract) owns commands and public states.
The shared Rust departure session and client context preserve one implementation for CLI and desktop callers.

The worker owns credentials, authorization, and the approved upload.
The renderer can confirm only the retained operation ID.
Cancellation before dispatch prevents public mutation. A reserved request settles before local reconciliation and exit.
A confirmed remote result remains distinct from local saving or cleanup failure.
Unknown acknowledgments never become cancellation or confirmed departure.
Local forgetting and whole-workspace deletion do not issue public departure.

The first native run found missing IPC day checks.
Astra added validation before admission and three credential-free worker regressions.
The next run found a synthetic device code outside the transport contract.
The harness now uses a valid fixed code and retains safe failure diagnostics.
The compiled journey then found that a refused local deletion discarded a confirmed receipt.
The lease now preserves preflight results and arms cache clearing only after credential preparation succeeds.
Two additional worker regressions cover admission, receipt preservation, and panic cleanup.

The Standards review found stale join text after a fresh status read and lost keyboard focus after operations.
Both findings have renderer regressions that failed before the fixes.
Fourteen focused checks pass across Chromium and WebKit, including accessibility checks and collapse races.
The read-only reviewer closes both UI findings and the later deletion-lease correction.
The independent Spec review found no SRS defect.
Reviewers ran no tests.

The native harness required two further fixture corrections.
It now waits for visible GUI controls after Rust publishes progress.
Synthetic service credentials follow the transport's hexadecimal contract.
These corrections retain strict production checks and meaningful assertions.
The native journey uses private homes, SQLCipher, GNOME Secret Service, synthetic HTTPS, and an actual compiled Linux app.
It does not inspect a user credential home or contact live GitHub.

The final compiled CLI journey passes after the shared recovery extraction.
The standalone backup verifier also passes with its locked dependency set.
Hosted run 43732934551 failed because that standalone lock omitted storage's new serde dependency.
Astra added that single dependency edge without changing versions or checksums.
The replacement hosted result remains pending until the next push.
Hosted run 43730532947 passes for the earlier departure-recovery commit b29df65.

The first full renderer pass found two stale navigation expectations, one in each engine.
The exact section list now includes Leaderboard and keeps the existing focus checks.
An added Enter-key check confirms focus on the new section.
All fifty checks in the affected specification pass.
The corrected full renderer run passes all 508 cases.

## Acceptance

The combined native Settings journey passes all eleven recorded groups.
Its retained 46-token preview uploads unchanged after saved history changes to 92 tokens.
Both native close boundaries and restart checks pass.
The separate local-deletion regression journey passes.
The corrected full renderer run passes all 508 cases.
The full Rust workspace passes 591 tests, with two ignored tests.
Default all-target Clippy passes after one equivalent let-chain correction in cancellation.
The final compiled native journey passes all eleven groups after the cancellation style correction.
Native-feature Clippy also passes.
The ordinary unconfigured build ignores a valid private endpoint override and makes no request or saved-data change.
The initial combined runner stopped at the stale navigation tests and ran no Rust checks.
Separate corrected runs supply the completed renderer and Rust results.

Live GitHub, hosted D1, scheduled cleanup, deployment, real histories, and other platforms remain separate gates.
Own-rank client views remain under S-23.
No release, deployment, production configuration, scanner activation, or new identity claim is part of this change.

The closing documentation draft scores 0.52 findings per 100 words.
The AST graph contains 15,806 nodes and 49,574 edges.
