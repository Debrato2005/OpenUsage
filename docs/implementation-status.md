# Implementation status

Updated: October 21, 2026. OpenUsage remains in development. The normative [SRS](SRS.md) is unchanged. Only local commits are permitted; never push, publish, create pull requests or modify a remote.

## Current product

- Two authorized OpenAI profiles have completed official sign-in and saved quota/activity collection. The first profile's readings and connection stayed unchanged when the second connected. Genuine cache restart passed. The user also confirmed both saved accounts and readings in the redesigned production overview. See [real-account evidence](evidence/native-startup-snapshot/) and [production overview review](evidence/native-reference-overview/user-review.json).
- The overview follows the supplied reference: selected account with quota/statistic tiles, account-share availability, combined-usage availability with explicitly selected-account activity, and a compact account list. DM Sans, accessible coss controls, both themes and persistent native display scaling are implemented. Unsupported data stays unavailable; no account-share totals or token quotas are invented.
- Add creates isolated default-named profiles, up to 10 active accounts. Prominent per-account Sign in, Cancel, Retry/Reconnect and automatic initial collection use the selected profile. Routine onboarding uses a bundled pinned official companion without a manual path. Credentials remain provider-managed in separate homes.
- Rust owns persistence, provider execution, authentication, scheduling, exact counts and IPC validation. SQLCipher 4/WAL protects application records. OS-backed database keys are used where available; the preserved WSL workspace uses its required fallback password because its session key store is unavailable. Provider keyring support remains incomplete.
- Runtime monitoring, manual refresh cooldown, provider backoff, saved daily activity, current quota/reset assessments, current-reading CSV/JSON export, encrypted authenticated backups, confirmed restore and Undo are implemented within their recorded Linux scopes.

## Latest verified slice

The reference Overview is rebuilt as one coherent desktop layout. At the supplied 1586 × 992 viewport, native panels measure about 591 / 307 / 363 pixels wide and 410 pixels tall. Five account rows fit below them. DM Sans, one selected quota window with a switcher, statistic tiles, restrained surfaces, light/dark themes and direct account actions preserve the requested hierarchy. Aggregate values remain explicitly unavailable because current provider identity, scope and coverage cannot establish compatible totals. The bar chart is labeled Selected account only, and its range counts saved dates rather than implying complete calendar coverage.

[Native reference evidence](evidence/native-reference-dashboard/) passed actual Add/sign-in/cancellation, automatic saved readings, exact details, seven synthetic daily bars, unavailable aggregation, range selection, both themes, 360-pixel reflow, actual native zoom and saved readings/scale after genuine restart. [Design QA](../design-qa.md) records direct full-view and focused comparisons and the corrected native layout faults. The final production binary refused all five synthetic providers before execution.

Both native configurations compiled. 272 Rust tests passed, one ignored; strict workspace Clippy and 43 contracts passed. All 48 affected renderer cases passed across recorded runs, with four final targeted density/accessibility checks. Unchanged broad native backup/restore suites were not repeated. The existing production process was no longer running when the final build was prepared; its data was preserved.

## Previous functional slice

Usage now exports every saved quota snapshot for its selected profile as CSV or JSON, including older pages, previous associations and archived profiles. One Rust-owned SQLCipher snapshot supplies all rows. Default exports use aliases; names and exact local references need explicit consent. Email and credentials are always omitted. Oversized selections fail without truncation. Existing current-reading exports remain available separately.

The [native export journey](evidence/native-quota-history-export/) passed all-22-snapshot CSV/JSON parity, per-profile isolation, private files, archived restart, refused requests and unchanged database/WAL/key bytes. The earlier [history paging journey](evidence/native-quota-history/) retains its scope. Both native configurations compiled. The existing user window was preserved.

Verification: build/type checks and 43 IPC contracts passed. The renderer milestone passed 237 cases initially; all five failed cases passed their focused rerun after fixing export wrapping and the saving-button test selector. The unrelated Overview navigation case also passed its rerun. 272 Rust tests passed, one ignored; strict workspace Clippy passed. A redundant cooldown assertion in a slow encrypted-backup test was removed; dedicated admission and exact-time boundary tests retain cooldown coverage. Production timing is unchanged.

## Restore and recovery checkpoint

Confirmed archive restore and checkpoint recovery are implemented. Earlier native journeys verified authenticated previews, explicit consent, reversible encrypted checkpoints, original protected keys/home mappings, process quiescence, stale-token refusal, restore/Undo/restart and current-schema backup content. Focused checks compare all columns in 16 tables. Rust tests cover rollback and owned-process interruption. These results do not establish power-loss durability or unexecuted platform acceptance. See [acceptance traceability](acceptance-traceability.md) for exact artifacts and limits.

## Still incomplete

- Durable alerts, notification delivery, per-profile preferences and quiet hours.
- Full activity-summary/history browsing, full activity-history/application export, retention and complete local deletion.
- Subscription, promotion, billing, deadline and verified account-comparison requirements.
- Real credential renewal/revocation acceptance, provider OS credential storage, safe browser callback isolation, and SIWC compatibility with both official usage operations.
- Sleep/wake acceptance, release-scale performance, packaging/signing, Windows/macOS execution and the complete SRS acceptance matrix.

The reference Overview redesign is locally verified. Durable alerts and remaining activity-history/export journeys remain open. Real authorized-account verification remains separate from synthetic-provider evidence. Current file-level status and the exact working checkpoint are retained in the local `.recovery/active-checkpoint.json`; historical execution details remain in their linked evidence and local commits.
