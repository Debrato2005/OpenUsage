# Implementation status

Updated: October 21, 2026. OpenUsage remains in development. The normative [SRS](SRS.md) is unchanged. Only local commits are permitted; never push, publish, create pull requests or modify a remote.

## Current product

- Two authorized OpenAI profiles have completed official sign-in and saved quota/activity collection. The first profile's readings and connection stayed unchanged when the second connected. Genuine cache restart passed. The user also confirmed both saved accounts and readings in the redesigned production overview. See [real-account evidence](evidence/native-startup-snapshot/) and [production overview review](evidence/native-reference-overview/user-review.json).
- The overview follows the supplied composition: selected account, quota doughnut, available daily activity, and compact account list. DM Sans, accessible coss controls, both themes and persistent native display scaling are implemented. Unsupported data stays unavailable; no account-share totals or token quotas are invented.
- Add creates isolated default-named profiles, up to 10 active accounts. Prominent per-account Sign in, Cancel, Retry/Reconnect and automatic initial collection use the selected profile. Routine onboarding uses a bundled pinned official companion without a manual path. Credentials remain provider-managed in separate homes.
- Rust owns persistence, provider execution, authentication, scheduling, exact counts and IPC validation. SQLCipher 4/WAL protects application records. OS-backed database keys are used where available; the preserved WSL workspace uses its required fallback password because its session key store is unavailable. Provider keyring support remains incomplete.
- Runtime monitoring, manual refresh cooldown, provider backoff, saved daily activity, current quota/reset assessments, current-reading CSV/JSON export, encrypted authenticated backups, confirmed restore and Undo are implemented within their recorded Linux scopes.

## Latest functional slice

Usage now opens quota history on request. Rust returns bounded 20-entry pages from a consistent SQLCipher snapshot. The UI retains exact percentages, expected resets and source/association details, keeps missing windows distinct from zero, and supports archived profiles. Browsing makes no provider requests and does not change storage.

The [native history journey](evidence/native-quota-history/) passed 20/2 paging over 22 synthetic snapshots, selected-profile cursor isolation, zero values, unchanged database/WAL/key bytes, archived history after genuine restart, and unchanged other-account readings. A test-only helper seeded 21 snapshots through production storage while the app was closed. Three genuine closes passed. Both native builds compiled. The existing user window was preserved.

Verification: 268 Rust tests passed, one ignored; strict workspace Clippy passed; build/type checks and 42 IPC contracts passed. The renderer milestone passed 239 cases, with one unchanged navigation-interrupted case passing its targeted rerun. All 240 cases passed across their recorded runs. Six new cases cover the history workflow and accessibility. Prior backup/restore receipts keep their original scope; unchanged broad native suites were not repeated.

## Restore and recovery checkpoint

Confirmed archive restore and checkpoint recovery are implemented. Earlier native journeys verified authenticated previews, explicit consent, reversible encrypted checkpoints, original protected keys/home mappings, process quiescence, stale-token refusal, restore/Undo/restart and current-schema backup content. Focused checks compare all columns in 16 tables. Rust tests cover rollback and owned-process interruption. These results do not establish power-loss durability or unexecuted platform acceptance. See [acceptance traceability](acceptance-traceability.md) for exact artifacts and limits.

## Still incomplete

- Durable alerts, notification delivery, per-profile preferences and quiet hours.
- Full activity-summary/history browsing, whole-history/application export, retention and complete local deletion.
- Subscription, promotion, billing, deadline and verified account-comparison requirements.
- Real credential renewal/revocation acceptance, provider OS credential storage, safe browser callback isolation, and SIWC compatibility with both official usage operations.
- Sleep/wake acceptance, release-scale performance, packaging/signing, Windows/macOS execution and the complete SRS acceptance matrix.

The next functional work is durable alerts and the remaining history/export journeys. Real authorized-account verification remains separate from synthetic-provider evidence. Current file-level status and the exact working checkpoint are retained in the local `.recovery/active-checkpoint.json`; historical execution details remain in their linked evidence and local commits.
