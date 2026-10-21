> Public source copy: raw acceptance evidence remains private. Historical test statements keep their original scope and do not certify this source snapshot.

# Implementation status

Updated: October 21, 2026. OpenUsage remains in development. The normative [SRS](SRS.md) is unchanged. Only local commits are permitted; never push, publish, create pull requests or modify a remote.

## Current product

- Two authorized OpenAI profiles have completed official sign-in and saved quota/activity collection. The first profile's readings and connection stayed unchanged when the second connected. Genuine cache restart passed. The user also confirmed both saved accounts and readings in the redesigned production overview. See real-account evidence (private evidence omitted) and production overview review (private evidence omitted).
- The overview follows the supplied reference: selected account with quota/statistic tiles, account-share availability, combined-usage availability with explicitly selected-account activity, and a compact account list. DM Sans, accessible coss controls, both themes and persistent native display scaling are implemented. Unsupported data stays unavailable; no account-share totals or token quotas are invented.
- Add creates isolated default-named profiles, up to 10 active accounts. Prominent per-account Sign in, Cancel, Retry/Reconnect and automatic initial collection use the selected profile. Routine onboarding uses a bundled pinned official companion without a manual path. Credentials remain provider-managed in separate homes.
- Rust owns persistence, provider execution, authentication, scheduling, exact counts and IPC validation. SQLCipher 4/WAL protects application records. OS-backed database keys are used where available; the preserved WSL workspace uses its required fallback password because its session key store is unavailable. Provider keyring support remains incomplete.
- Runtime monitoring, manual refresh cooldown, provider backoff, saved daily activity, current quota/reset assessments, current-reading CSV/JSON export, encrypted authenticated backups, confirmed restore and Undo are implemented within their recorded Linux scopes.

## Development storage and publication checks

The P0 cleanup reduced the measured workspace from 78.578 to 8.112 GiB while preserving Git history, dirty source, protected data and essential evidence. Obsolete executable copies, 94 clean worktrees, redundant bundles and verified staging/cache copies were retired. Their historical receipts do not qualify a newly rebuilt application. Source preservation refs retain uncommitted work without changing its real indexes.

Normal freezing now records provenance only; retained binaries are explicit and capped. Repository-private native fixtures clean setup failures and normal teardown. The storage command checks allocation and safely previews/applies bounded, marker-owned diagnostic cleanup. Seventeen storage tests passed on Linux, including measurement of existing runtime sockets without opening them. Cleanup still refuses special files. Two consecutive renderer builds retained identical 2,457,600-byte outputs; three synthetic private D-Bus fixture cycles left no additional workspace. Cross-platform CI is defined but has not run. No Rust rebuild was performed to recreate deleted caches. See [contributor instructions](../CONTRIBUTING.md) and [retention policy](artifact-retention.md).

The project root is now the normal reviewed working checkout on `main`. Independent checks matched all 338 reviewed files, their modes and the Git index. The original Git metadata, exact staged index and 47 replaced source files remain in private local backups. All 604 reviewed commits and 220 local branch tips survived the switch. No remote operation occurred. These counts describe the consolidation baseline, before later commits. Remote branch cleanup remains a separate task that requires permission to change the remote.

## Publication security follow-up

A security policy records reporting boundaries, the glib advisory and release gaps. Exact SQLCipher and OpenSSL license texts accompany the source assets. Two native harnesses now use portable repository paths and an explicitly selected, pinned official executable. Discovery uses a private fixture home without reading an existing credential home. Syntax and six setup checks passed; these checks do not establish native sign-in acceptance. Review of the filtered source history completed. Later storage and ignore-policy commits passed targeted secret scans with credential verification disabled. Original private history and evidence remain local. Source review does not qualify a native release.

The native notice inventory now maps all 486 locked registry packages to 288 exact deduplicated texts. Original notices and explicitly paired standard bodies retain their source grants. Archive checksums, license expressions, text hashes and both source mirrors pass verification. This preparation does not qualify a linked binary or replace its platform/system-library and MPL compliance checks.

## Latest verified slice

Overview, Usage and Settings are the only primary pages. Account actions use a contextual dialog. Overview shows both reported quota windows together and keeps Refresh on the same page, with selected-account progress, visible cooldown and actionable errors. Settings retains account renaming, archiving and recovery. Dark appearance uses VS Code-inspired true-black colors. No plan-derived windows or aggregate values are invented.

The native three-view journey (private evidence omitted) passed selected synthetic sign-in/cancellation, automatic saved readings, both quota bars, same-page Refresh with another account unchanged, exact details, both themes, narrow reflow, genuine native zoom and saved readings/scale after restart. Both native configurations compiled. All 254 renderer cases passed across the recorded runs. Six final color/accessibility cases, build/type checks and 43 contracts passed. The prior 272 Rust tests and strict workspace Clippy passed; final UI refinements leave Rust unchanged. One Rust test remains ignored. Production refused all five synthetic sources before execution. The encrypted real-account workspace was preserved. Its prior process was absent after the executor restart.

The user requested this revision after rejecting the previous visual result. Owner acceptance of this revision remains open. Earlier reference captures retain their original artifact scope.

## Previous functional slice

Usage now exports every saved quota snapshot for its selected profile as CSV or JSON, including older pages, previous associations and archived profiles. One Rust-owned SQLCipher snapshot supplies all rows. Default exports use aliases; names and exact local references need explicit consent. Email and credentials are always omitted. Oversized selections fail without truncation. Existing current-reading exports remain available separately.

The native export journey (private evidence omitted) passed all-22-snapshot CSV/JSON parity, per-profile isolation, private files, archived restart, refused requests and unchanged database/WAL/key bytes. The earlier history paging journey (private evidence omitted) retains its scope. Both native configurations compiled. The existing user window was preserved.

Verification: build/type checks and 43 IPC contracts passed. The renderer milestone passed 237 cases initially; all five failed cases passed their focused rerun after fixing export wrapping and the saving-button test selector. The unrelated Overview navigation case also passed its rerun. 272 Rust tests passed, one ignored; strict workspace Clippy passed. A redundant cooldown assertion in a slow encrypted-backup test was removed; dedicated admission and exact-time boundary tests retain cooldown coverage. Production timing is unchanged.

## Restore and recovery checkpoint

Confirmed archive restore and checkpoint recovery are implemented. Earlier native journeys verified authenticated previews, explicit consent, reversible encrypted checkpoints, original protected keys/home mappings, process quiescence, stale-token refusal, restore/Undo/restart and current-schema backup content. Focused checks compare all columns in 16 tables. Rust tests cover rollback and owned-process interruption. These results do not establish power-loss durability or unexecuted platform acceptance. See [acceptance traceability](acceptance-traceability.md) for exact artifacts and limits.

## Still incomplete

- Durable alerts, notification delivery, per-profile preferences and quiet hours.
- Full activity-summary/history browsing, full activity-history/application export, retention and complete local deletion.
- Subscription, promotion, billing, deadline and verified account-comparison requirements.
- Real credential renewal/revocation acceptance, provider OS credential storage, safe browser callback isolation, and SIWC compatibility with both official usage operations.
- Sleep/wake acceptance, release-scale performance, packaging/signing, Windows/macOS execution and the complete SRS acceptance matrix.

The three-view Overview revision is locally verified within the recorded synthetic Linux scope. Durable alerts and remaining activity-history/export journeys remain open. Real authorized-account verification remains separate from synthetic-provider evidence. Current file-level status and the exact working checkpoint are retained in the local `.recovery/active-checkpoint.json`; historical execution details remain in their linked evidence and local commits.
