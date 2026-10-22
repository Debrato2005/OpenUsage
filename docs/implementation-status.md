> Public source copy: raw acceptance evidence remains private. Historical test statements keep their original scope and do not certify this source snapshot.

# Implementation status

Updated: October 22, 2026. OpenUsage remains in development. The normative [SRS](SRS.md) is unchanged. Only local commits are permitted. Remote branch deletion requires separate owner approval. Agents must not push or publish.

## Completed assignment: synchronized profile activity

The owner approved the narrow [selected-profile aggregation amendment](proposals/user-curated-profile-activity.md). Overview now has Individual, Profile sum and Verified combined views. One Rust response supplies the heatmap, observed total, contribution shares, exclusions and coverage for one selection and date range. Daily, ISO-week and cumulative projections reuse the existing calendar calculation.

Profile sums carry a visible Unverified label. Details explain overlapping accounts, unknown application scope and unknown day definitions. The calculation requires the same pinned provider contract and matching provider dates. Known scope differences and known duplicate identities are excluded. Verified totals still require distinct approved identity keys and verified compatible semantics. The current managed adapter supplies neither proof, so verified totals remain unavailable.

Rust uses checked exact integer arithmetic. Shares use the same observed denominator and deterministic basis-point rounding. Zero totals have no percentage shares. Missing contributions remain unknown. Each cell and the equivalent text table retain coverage. Quota percentages and provider lifetime summaries never enter these sums. Large values now show readable abbreviations, with grouped exact integers in tooltips and Values.

The actual native-test journey passed two isolated synthetic sign-ins, automatic saved readings, synchronized charts, custom date input events, all three modes, strict IPC refusal, the verified gate and genuine restart. Database, WAL and protected-key bytes stayed unchanged during read-only aggregation. This does not qualify real-provider identity or scope contracts, OAuth renewal or other platforms.

Verification: 164 desktop Rust tests, strict Clippy and 55 IPC checks passed. All 370 renderer cases passed across the milestone, remaining-engine and focused correction runs. The initial milestone hit its deadline. The clock assertion now checks an actual tick without assuming a zero-second startup. The final native harness waits for the enabled unlock form after restart.

Private evidence: activity-dashboard-native-final-ready. The normative SRS file, SQLCipher 4/WAL and schema 9 remain unchanged. No new dependency, provider endpoint or executable copy was added.

## Current recovery regression: schema 9

The backup-content and checkpoint-recovery native harnesses now expect schema 9. Both passed against production executable 6c6672bbcfe27df9e54aa828fc8b65b80c797f4056aaa8b79b28053f0d1bf708. The journeys used private synthetic workspaces on an isolated X11 display. The active desktop first failed its exact-window focus gate. No security or focus assertion was removed.

Confirmed archive restore, explicit checkpoint recovery, two genuine window closes, stale-preview refusal, four checkpoint checks and preserved original home mappings passed. Independent read-only SQLCipher checks verified encryption, WAL records, the protected key and selected semantic fields. They do not compare every column or prove power-loss durability. Both current-schema oracle tests and the owned-process interruption test passed. No real provider session or other-platform acceptance ran.

The artifact audit found 17 provenance receipts, no retained executables associated with them and no malformed receipts. It followed no links and deleted nothing. Source, history and recovery evidence remain preserved. Private evidence: recovery-goal-current-backup-isolated and recovery-goal-current-checkpoint. The normative SRS and application storage behavior are unchanged.

## Completed assignment: heatmap layout and Linux window controls

Usage opens a full-year calendar. Overview keeps its compact 90-day view. All seven weekday labels are visible. Neutral padding completes the first and last weeks without creating saved readings, zero counts or interactive dates. Missing days use solid cells and remain distinct from confirmed zero. The selected range and reported-day count stay visible.

Linux uses native GTK window controls. GTK retains dragging, resizing and accessible buttons. Windows and macOS keep their existing decorations. Test windows carry an explicit Test workspace title.

Build/type checks, 54 contracts, strict Clippy and the external-navigation rejection test passed. All 44 affected renderer cases passed across the recorded runs. Both browser engines then passed the new corner-padding regression. The actual Linux native-test window passed full-year layout, complete week columns, seven labels, exact saved dates, compact Overview and maximize/restore/close actions. Its rendered interface was inspected. Production compiled separately.

WSLg/Weston kept WM_STATE Normal after both the minimize button and a direct GDK iconify request. Host taskbar minimization remains unqualified. This is a recorded platform limitation, not a passing minimize check.

Combined totals remain unavailable. The current managed account interface does not establish distinct verified identities and compatible activity coverage. Presentation now explains this restriction compactly and links to individual activity. Provider operations, protected storage and aggregation safeguards are unchanged.

Private evidence: heatmap-window-native. No executable copies were retained. This bounded assignment stops after its local commit.

## Completed assignment: clock, time zones and heatmap

The header clock updates each second. Settings → Date and time saves a recognized IANA time zone, including India, UTC and system time. Overview, quota readings, history and alert timestamps use that choice with zone labels. Exact source timestamps remain UTC. Notification quiet hours keep their separate time-zone setting.

Overview and account details show live expected-reset countdowns. Elapsed time never restores quota or confirms a reset. The blue calendar heatmap now fills its available width with larger cells and smaller gaps. Native tooltips and keyboard details retain exact counts. Missing days stay distinct from confirmed zero. Provider-reported dates never shift when the display time zone changes.

SQLCipher 4/WAL and schema 9 remain unchanged. The existing encrypted settings field stores the choice. Authenticated backups accept recognized zones and preserve them through restore. Unknown zones and extra IPC fields are refused. No dependency or provider endpoint was added.

The Linux native-test journey passed clock ticks, both quota windows, selected-account isolation, India time, invalid-input refusal, backup/restore and genuine restart. It used approved synthetic-provider readings and a controlled renderer clock, not real-provider reset or OAuth acceptance. The actual rendered window was inspected. The production configuration compiled. All 153 storage tests and 11 workspace tests passed; strict Clippy and 54 contract checks passed. The renderer milestone passed 360 cases initially. Six failures exposed missing fixture support and a lost timestamp attribute. After those corrections, all 12 selected cases passed in Chromium and WebKit, covering all 366 cases across the recorded runs.

Private evidence: `time-zone-heatmap-native`. No executable copies were retained. This bounded assignment stops after its local commit.

## Completed assignment: manual subscriptions

Settings → Subscriptions now supports create, edit, archive, unarchive and confirmed delete for each account. User-confirmed terms stay separate from provider plans and payments. Rust stores exact nonnegative i64 minor units with ISO currency scales. Unknown prices remain null. Date-only terms remain date-only. Revision checks refuse stale and cross-account edits.

Schema 9 migrates transactionally and preserves subscription records through encrypted backups, restore, Undo and restart. The SRS, SQLCipher 4/WAL and authentication boundaries remain unchanged. Earlier saved drafts are now integrated.

The full Rust workspace passed 384 tests, with one intentional child test ignored. Strict Clippy, 53 contracts, 360 renderer cases and the current-schema oracle test passed. The final production Linux native journey passed GUI CRUD/archive, cancellation, invalid-input refusal, account isolation and genuine restart. Seven independent checks covered SQLCipher snapshots, exact large amounts and authenticated backup contents across 31 tables. Restore and Undo preserved subscription fields. Native recovery used typed IPC consent with synthetic profiles, not real-provider sessions. The subscription panel was inspected in the actual window.

Private evidence: `subscriptions-native`. Production SHA-256: `3a7eca31e890b5d0999462f9f43daf5b8e7f1df3f9e7e7c95bbebc5b155d4d95`. No executable copies were retained. Windows/macOS and broader billing features remain outside this assignment. Execution stops after the local commit.

## Engineering workflow optimization

The bounded tooling task adds offline environment diagnostics, changed-file verification plans and content-free trace analysis. Existing cleanup and Git secret checks remain. All 26 local Python tool tests passed. Documentation-only cycles stabilized at three logs totaling 2,112 bytes. No application build, provider session or native acceptance ran. Windows/macOS CI is configured but unexecuted. See the [commands and measured results](engineering-workflow.md).

At the tooling checkpoint, subscription drafts remained preserved and unintegrated in `6e11336`. The completed subscription assignment above supersedes that pending state.

## Current product

- Two authorized OpenAI profiles have completed official sign-in and saved quota/activity collection. The first profile's readings and connection stayed unchanged when the second connected. Genuine cache restart passed. The user also confirmed both saved accounts and readings in the redesigned production overview. See real-account evidence (private evidence omitted) and production overview review (private evidence omitted).
- The overview follows the supplied reference: selected account with quota/statistic tiles, account-share availability, combined-usage availability with explicitly selected-account activity, and a compact account list. DM Sans, accessible coss controls, both themes and persistent native display scaling are implemented. Unsupported data stays unavailable; no account-share totals or token quotas are invented.
- Add creates isolated default-named profiles, up to 10 active accounts. Prominent per-account Sign in, Cancel, Retry/Reconnect and automatic initial collection use the selected profile. Routine onboarding uses a bundled pinned official companion without a manual path. Credentials remain provider-managed in separate homes.
- Rust owns persistence, provider execution, authentication, scheduling, exact counts and IPC validation. SQLCipher 4/WAL protects application records. OS-backed database keys are used where available; the preserved WSL workspace uses its required fallback password because its session key store is unavailable. New Linux profiles can use protected provider storage. Real ChatGPT and other-platform qualification remain incomplete.
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

New empty Linux profiles use strict OS credential storage when the validated desktop store is available. A private marker preserves that choice. Existing file-backed sessions remain intact. Protected profiles refuse an unavailable store instead of falling back to files. Confirmed deletion removes only exact matching provider entries, including archived and retained homes, without reading secrets.

The production native journey passed protected credential retrieval, restart, selected official logout, unavailable and duplicate-entry refusal, confirmed cleanup retry and unrelated-entry preservation. It used nonfunctional synthetic API-key credentials and the pinned official companion. It does not prove ChatGPT OAuth, token renewal, real revocation or provider usage through this backend.

The full workspace passed 374 Rust tests before the shutdown correction, with one intentional child test ignored. The final provider library passed 67 tests and the application provider module passed 32. Six focused credential cases, strict Clippy, 52 contracts, 342 renderer cases and the production build passed. Native verification exposed immediate termination leaving official helper symlinks. Bounded graceful EOF now permits cleanup before forced termination. Arbitrary links remain refused. Older remnants still require a successful authorized provider operation before deletion.

Private evidence: `provider-keyring-production-eof-native`, `provider-keyring-account-native-regression` and `provider-keyring-production-refusal`. The restored production executable refused all six synthetic providers before execution and returned with the same hash. The latter passed two synthetic ChatGPT-shaped sign-ins, automatic collection, sign-out, exact saved-record preservation and genuine restart through the file-backed path. Schema 8, SQLCipher 4/WAL, the normative SRS and dependency locks remain unchanged. Other platforms and real ChatGPT sessions through the protected backend remain unqualified.

Per-account Sign out uses the official account/logout method after explicit consent. Rust reserves only the selected profile, checks the approved source and held home, and waits outside the workspace lock. A strict empty acknowledgement and a signed-out account read precede the saved status change. Cached readings and history remain. Other profiles retain their authorization and records. OpenAI can attempt session revocation. OpenUsage does not promise that revocation succeeded.

The Linux native journey passed two synthetic account sign-ins, automatic collection, confirmed sign-out, failure refusal, restart, second-account refresh and selected-account reconnect. Six independent checks compared all 30 stored tables. Only the selected connection status and update time changed on sign-out. The confirmation dialog was inspected. Production refused all six synthetic providers before execution. Private evidence: account-signout-native-current and account-signout-production-refusal. Real OAuth revocation and other platforms remain unqualified.

Strict Clippy, 52 contracts and both final builds passed. All 342 renderer cases passed across the bounded broad run and final focused runs. The first run caught a dialog event bubbling defect. Cancel and Escape now leave the parent account panel open. The broad command reached its deadline. The remaining cases passed separately. All 368 Rust cases passed across the workspace and focused runs, with one intentional child test ignored. Two activity tests failed initially and passed in the focused run. Their intermittent cause remains unresolved.

The pinned Linux companion passed a separate local keyring probe with internet sockets blocked. Two fresh synthetic credentials stayed isolated across child restarts. Exact deletion preserved the second and unrelated entries. This proves local keyring feasibility only. Product provider storage still uses isolated provider-managed files. Protected provider storage, real token renewal and revocation remain open.

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
- Complete application export and selected quota/activity history have scoped Linux acceptance evidence. Local deletion still requires platform acceptance. History remains unlimited by default, without silent pruning.
- Subscription, promotion, billing, deadline and verified account-comparison requirements.
- Real credential renewal/revocation acceptance, protected provider storage qualification across supported platforms, safe browser callback isolation, and SIWC compatibility with both official usage operations.
- Sleep/wake acceptance, release-scale performance, packaging/signing, Windows/macOS execution and the complete SRS acceptance matrix.

The three-view Overview revision is locally verified within the recorded synthetic Linux scope. Deadline alerts and the remaining platform and live-provider journeys remain open. Real authorized-account verification remains separate from synthetic-provider evidence. Current file-level status and the exact working checkpoint are retained in the local `.recovery/active-checkpoint.json`; historical execution details remain in their linked evidence and local commits.
