> Public source copy: raw acceptance evidence remains private. Historical test statements keep their original scope and do not certify this source snapshot.

# Implementation status

Updated: October 22, 2026. OpenUsage remains in development. The normative [SRS](SRS.md) is unchanged. Only local commits are permitted. Remote branch deletion requires separate owner approval. Agents must not push or publish.

## Current product

- Two authorized OpenAI profiles have completed official sign-in and saved quota/activity collection. The first profile's readings and connection stayed unchanged when the second connected. Genuine cache restart passed. The user also confirmed both saved accounts and readings in the redesigned production overview. See real-account evidence (private evidence omitted) and production overview review (private evidence omitted).
- The overview follows the supplied reference: selected account with quota/statistic tiles, account-share availability, combined-usage availability with explicitly selected-account activity, and a compact account list. DM Sans, accessible coss controls, both themes and persistent native display scaling are implemented. Unsupported data stays unavailable; no account-share totals or token quotas are invented.
- Add creates isolated default-named profiles, up to 10 active accounts. Prominent per-account Sign in, Cancel, Retry/Reconnect and automatic initial collection use the selected profile. Routine onboarding uses a bundled pinned official companion without a manual path. Credentials remain provider-managed in separate homes.
- Rust owns persistence, provider execution, authentication, scheduling, exact counts and IPC validation. SQLCipher 4/WAL protects application records. OS-backed database keys are used where available; the preserved WSL workspace uses its required fallback password because its session key store is unavailable. Provider keyring support remains incomplete.
- Runtime monitoring, manual refresh cooldown, provider backoff, saved daily activity, current quota/reset assessments, saved reset-event history, opt-in quota threshold/reset, sign-in-required and repeated-failure notifications with quiet hours, saved activity-summary history, selected quota/activity history and current-reading CSV/JSON export, encrypted authenticated backups, confirmed restore and Undo are implemented within their recorded Linux scopes.

## Development storage and publication checks

The P0 cleanup reduced the measured workspace from 78.578 to 8.112 GiB while preserving Git history, dirty source, protected data and essential evidence. Obsolete executable copies, 94 clean worktrees, redundant bundles and verified staging/cache copies were retired. Their historical receipts do not qualify a newly rebuilt application. Source preservation refs retain uncommitted work without changing its real indexes.

Normal freezing now records provenance only; retained binaries are explicit and capped. Repository-private native fixtures clean setup failures and normal teardown. The storage command checks allocation and safely previews/applies bounded, marker-owned diagnostic cleanup. Seventeen storage tests passed on Linux, including measurement of existing runtime sockets without opening them. Cleanup still refuses special files. Two consecutive renderer builds retained identical 2,457,600-byte outputs; three synthetic private D-Bus fixture cycles left no additional workspace. Cross-platform CI is defined but has not run. No Rust rebuild was performed to recreate deleted caches. See [contributor instructions](../CONTRIBUTING.md) and [retention policy](artifact-retention.md).

The project root is now the normal reviewed working checkout on `main`. Independent checks matched all 338 reviewed files, their modes and the Git index. The original Git metadata, exact staged index and 47 replaced source files remain in private local backups. All 604 reviewed commits and 220 local branch tips survived the switch. No remote operation occurred. These counts describe the consolidation baseline, before later commits. Remote branch cleanup remains a separate task that requires permission to change the remote.

The Git workflow repair leaves one local main branch and one public main branch. One ancestry merge preserves all 607 prior commits without changing the source tree or rewriting an existing commit. The 219 merged local branches were retired and the redundant publication mirror was removed. Default pushes target only origin/main. Four hook tests passed with real Gitleaks, including synthetic-secret redaction, refused private artifacts and preservation checks for branch deletion. After the owner published main and approved cleanup, one atomic operation deleted only the 219 unchanged reviewed remote branches. A fresh read confirmed main and its hash stayed unchanged. No source commits were pushed by the agent. See the [completed cleanup record](branch-cleanup-plan.md).

## Publication security follow-up

A security policy records reporting boundaries, the glib advisory and release gaps. Exact SQLCipher and OpenSSL license texts accompany the source assets. Two native harnesses now use portable repository paths and an explicitly selected, pinned official executable. Discovery uses a private fixture home without reading an existing credential home. Syntax and six setup checks passed; these checks do not establish native sign-in acceptance. Review of the filtered source history completed. Later storage and ignore-policy commits passed targeted secret scans with credential verification disabled. Original private history and evidence remain local. Source review does not qualify a native release.

The native notice inventory now maps all 486 locked registry packages to 288 exact deduplicated texts. Original notices and explicitly paired standard bodies retain their source grants. Archive checksums, license expressions, text hashes and both source mirrors pass verification. This preparation does not qualify a linked binary or replace its platform/system-library and MPL compliance checks.

## Latest verified slice

Settings exports all saved application records as JSON after explicit consent. One validated SQLCipher 4/WAL snapshot includes archived accounts, history, preferences, alerts and local references. Exact integers remain decimal strings. Null stays distinct from zero. Credentials, keys, provider-home files and device display calibration stay excluded. This plaintext export is separate from encrypted backup recovery.

All 362 Rust tests passed, with one intentional child test ignored. Strict Clippy, 52 contracts and the final build passed. All 330 renderer cases passed across the broad run and 36 final focused cases. The initial run caught narrow export wrapping and two activity-history failures. The final run used frozen source and passed.

The Linux native journey passed an empty workspace, two synthetic sign-ins, automatic collection, archived history and genuine restart. Five independent read-only checks compared all 30 table rowsets and every populated field. All 74 populated records matched. Database, WAL and wrapped-key bytes stayed unchanged. Export started no provider sessions. Private permissions, consent, exclusions and rejected arguments passed. The rendered receipt was inspected.

Private evidence: application-export-native and application-export-production-refusal. The production build refused all five synthetic providers. Schema 8 and the normative SRS stay unchanged. The storage crate now uses the existing locked JSON serializer. No dependency version changed. The 5 GiB and two-minute limits refuse incomplete output. Other platforms, live-account export and power-loss durability remain unqualified.

After confirmed deletion, OpenUsage checks the current credential store before presenting fresh setup. A store that became available during fallback use now permits setup without an app password. Nine focused deletion cases and strict Clippy passed after a regression first reproduced the old prompt. The default native Linux journey used a real private Secret Service. It confirmed deletion, setup without an app password, unrelated-key preservation and a genuine restart into the new OS-protected workspace. No provider session or existing credential home was used.

Private evidence: `deletion-fresh-key-store-production-native`. This follow-up changes fresh onboarding only. The prior complete deletion journey retains its original executable scope.

Settings offers confirmed deletion of managed OpenUsage account data. Rust stops owned provider work, clears grants, closes SQLCipher and removes managed homes and recovery files. It removes only the matching application key. A private marker binds each retry to the original database and wrapped-key file identities. Startup blocks new setup during incomplete deletion. Every retry requires confirmation. The instance lease, external exports, external backups, other Codex installations and device display scale remain.

All 354 workspace tests passed, with one intentional child test ignored. Strict Clippy, 51 contracts and 318 renderer cases passed. Twelve final renderer cases and the final build passed. The Linux native journey used fresh synthetic account homes and a real private Secret Service. It checked saved readings, unsafe-path refusal, confirmed deletion, pending restart, new setup, exact key removal and preservation of unrelated credentials and external files.

The test measured a WebDriver mouse coordinate error at 125% scale. Real keyboard actions completed the affected forms. No application authentication or security rule changed. Windows refuses deletion until equivalent ownership checks exist. macOS, live-account deletion, physical secure erasure and power-loss durability remain unqualified. Complete application export remains open.

The development profiles optimize SHA-2 in both workspaces. One measured hash of the pinned 296.9 MB companion decreased from 4,626 to 136 ms with an identical digest. All four approval checks remain. The default native build refused all five synthetic providers before execution.

Private evidence: `local-data-deletion-optimized-native` and `local-data-deletion-optimized-production-refusal`. The default executable returned byte-identically after the test. Schema 8, SQLCipher 4/WAL, the normative SRS and dependency locks remain unchanged.

## Earlier verified slices


Usage now exports saved activity history as CSV or JSON from the existing history panel. Each file keeps separate summary snapshots and daily accepted/prior values. It calculates no events, deltas or totals. One SQLCipher/WAL snapshot covers the selected profile, including archived profiles and earlier connections. Aliases are the default. Explicit consent permits local names and references, but never email or credentials.

All 341 Rust tests passed, with one intentional child test ignored. Four final storage cases, strict Clippy, build/type checks, 50 contracts and 306 renderer cases passed. A privacy test denies reads of email, plan and managed-home references. Closing an export panel now releases only its own busy state and ignores stale replies.

The explicit native-test build passed two synthetic sign-ins and automatic collection. CSV and JSON matched all 146 records from 22 summaries and seven retained daily observations. Four independent checks compared all 30 tables and unchanged database/WAL/key bytes. Consent, redaction, file permissions, invalid requests, account isolation, shared export routes and archived restart passed. The rendered receipt passed visual review. The first native run caught a receipt-text parsing error in the test; the corrected selector passed without application changes.

Private evidence: `activity-summary-history-export-native-receipt`. The production executable compiled and returned with the same hash after the test. Schema 8 and dependency locks remain unchanged. The existing 50,000-row/64 MiB limit refuses oversized exports without a partial file. Complete application export, local deletion, real-provider history and other platforms remain open.


Activity history opens on demand in Usage. Rust reads 20 saved summaries per page from one consistent SQLCipher/WAL snapshot. Exact counts, missing values, original connections and source details remain separate for each reading. The app infers no events or usage deltas. Archived accounts remain available after restart. Schema 8 remains unchanged.

All 337 Rust tests passed, with one intentional child test ignored. Strict Clippy, build/type checks, 49 contracts and 294 renderer cases passed. The explicit native-test build passed two synthetic sign-ins, automatic collection, 20/2 paging, exact values, cursor isolation and archived restart.

Four independent checks compared all 30 tables and unchanged database/WAL/key bytes. Browsing started no provider sessions. I inspected the rendered history. The first native run caught a missing command permission. The fix granted that command only to the local main window.

Private evidence: `activity-summary-history-native-acl`. Real-provider history, history export and other platforms remain separate.

Sign-in-required and repeated-collection-failure alerts now share the existing Notifications form, quiet hours and desktop delivery journal. All four alert types default to off and retain independent account overrides. Three accepted transient failures produce one alert. A fresh accepted live reading closes the incident in the same transaction.

Schema 8 keeps immutable failure evidence separate from quota proof. Current incident state and submission receipts survive archive restore, Undo and checkpoint recovery. Ordinary successful polls add no operational history. Cancellation, unsupported capabilities, malformed data and unavailable identity do not count as transient failures.

The workspace passed 330 Rust tests, with one intentional child-only test ignored. Strict Clippy, 48 IPC contracts, 282 renderer cases, both schema-oracle tests and 14 explicit test-feature storage cases passed. The final failure-proof representation also passed 21 focused provider tests and both authentication-race tests.

The default production build passed native authenticated backup-content, checkpoint recovery and fixture refusal. The explicit test build passed controlled supervised failures, actual retry delays, account isolation, quiet queues, two private desktop-service acknowledgements, genuine restart, archive restore and Undo. Five independent read-only checks covered all 30 tables without changing database, WAL or key bytes.

These results cover synthetic Linux journeys. They do not establish real-provider invalid-credential detection, desktop visibility, notification click routing or other platforms. A supported authentication-required response permits an actionable sign-in alert after a prior accepted reading. It does not prove invalid credentials. OU-ALERT-007 remains open under [SC-009](srs-clarifications.md#sc-009-authentication-required-evidence). Provider polling is unchanged.

Usage alerts at 80%, 90% and 100% now share one Notifications form. Reset alerts, quiet hours and independent account overrides use the same form. Rust accepts crossings only from fresh readings in a confirmed quota window. A first reading at 95% queues only 90% and consumes 80% as Covered. Later crossings remain separate.

Schema 7 stores crossing evidence separately from reset proof. Durable window anchors prevent replay after restore within an unchanged profile association. A new association does not inherit these records. Legacy orphan reset receipts keep an unknown anchor when their original proof is unavailable.

Verification passed: 309 Rust tests across the recorded workspace and two repaired legacy-fixture runs, with one intentional child-only test ignored. All 13 test-feature storage cases, strict workspace Clippy and 47 IPC contracts passed. The 280 renderer cases and two schema-oracle tests passed across the full and focused runs. The default Linux build refused synthetic notification queues and passed authenticated backup-content and native checkpoint recovery.

The explicit test build sent five acknowledged notifications to a private D-Bus service. It passed quiet hours, per-account crossings, genuine restart, confirmed archive restore and Undo. Six read-only encrypted-copy checks covered all 24 tables. They compared every field in the four alert-related tables. The replay checks retain the same controlled account association. They do not prove deduplication across a newly created association.

These checks do not qualify live-provider sessions, OS visibility, click routing, other platforms or power-loss durability. Provider polling is unchanged. The ten-second worker reads only the local queue.

Linux requires an actual service acknowledgement before recording submission. The [Freedesktop protocol](https://specifications.freedesktop.org/notification/latest/protocol.html) supplies that reply; [Tauri’s plugin implementation](https://raw.githubusercontent.com/tauri-apps/plugins-workspace/v2/plugins/notification/src/desktop.rs) returns before its OS send finishes. Windows/macOS use the same maintained native component as that plugin, with a bounded worker, and remain unqualified. The unchanged 486-package notice baseline plus the [24-package supplement](../apps/desktop/public/licenses/notification-dependencies.json) covers all 510 locked registry packages with exact original text hashes. This is notice preparation, not native release qualification.

Overview, Usage and Settings now use a compact top navigation. The dashboard uses the full window width, with appearance and display scale in the header and privacy details in Settings. Build/type checks, 45 IPC contracts and all 268 renderer cases passed, including keyboard navigation, both themes and enlarged text. The current production-configuration Linux app passed the focused native navigation journey: four window widths from 360 to 1280 pixels, real keyboard actions, both themes, unchanged synthetic profiles and saved 125% native scaling after a genuine close/reopen. The rendered light, dark and narrow views were inspected. No provider requests or existing credential-home reads were used. This changes presentation only; Rust and storage behavior are unchanged.

Confirmed resets now create durable local events in the same transaction as the accepted quota reading. Schema 5 records only new confirmed generations, without replaying existing history during migration. Settings provides a bounded, read-only Alert history with exact source details and navigation to the event's account. These records do not send desktop notifications. All 283 Rust tests passed across the recorded workspace/storage runs, with one intentional child-only test ignored. Strict workspace Clippy, build/type checks, 45 contracts, 266 renderer cases and two current-schema oracle tests passed. The current production-configuration Linux build passed native alert history, populated alert-ledger archive restore/Undo/restart, backup-content refusal and GUI checkpoint recovery. The populated-ledger facet used confirmed typed native IPC; the checkpoint journey exercised GUI consent. All twelve alert columns, event IDs, protected key and source archives/checkpoints were preserved. These were private synthetic journeys, not live-provider, desktop-delivery, crash-durability or cross-platform qualification. Standalone verifier profiles now match the root's reduced-debug profiles to avoid a duplicate full-debug cache.

Composed controls now retain the shared Button wrapping and sizing rules when a primitive replaces its `data-slot`. A regression reproduced the export-trigger overflow at 360px and 200% text in both renderer engines before the fix. Build/type checks, 44 contracts and all 256 renderer cases passed afterward. A fresh default production-configuration Linux build passed native authenticated backup export and checkpoint restore/recovery across genuine window closes, including stale-preview refusal, corrupt-copy refusal, preserved profile homes and enlarged-text reflow. Source and executable integrity checks passed. These journeys used isolated synthetic records, not real provider sessions, and do not establish crash durability or other-platform acceptance. Verification used the shared build output directly and retained only a provenance receipt, not another executable copy.

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

- Deadline notifications and confirmed invalid-credential detection. The four implemented alert types have opt-in, per-profile overrides and quiet hours. Real-provider-triggered and installed-platform delivery remain open.
- Complete application export, explicit retention rules and platform acceptance for local deletion. Selected quota/activity history browsing and exports now have scoped acceptance evidence.
- Subscription, promotion, billing, deadline and verified account-comparison requirements.
- Real credential renewal/revocation acceptance, provider OS credential storage, safe browser callback isolation, and SIWC compatibility with both official usage operations.
- Sleep/wake acceptance, release-scale performance, packaging/signing, Windows/macOS execution and the complete SRS acceptance matrix.

The three-view Overview revision is locally verified within the recorded synthetic Linux scope. Deadline alerts and remaining activity-history/export journeys remain open. Real authorized-account verification remains separate from synthetic-provider evidence. Current file-level status and the exact working checkpoint are retained in the local `.recovery/active-checkpoint.json`; historical execution details remain in their linked evidence and local commits.
