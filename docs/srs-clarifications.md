> Public source copy: raw acceptance evidence remains private. Historical test statements keep their original scope and do not certify this source snapshot.

# SRS clarification record

This is the record of how requirements were read. Entries keep their original date and wording, so "the normative SRS" in an entry means the baseline at that date. [Baseline 3.0](SRS.md) is current.

| ID | Topic | Status under baseline 3.0 |
|---|---|---|
| SC-001 | Reset confirmation with rapid new-window use | Proposed, not approved |
| SC-002 | Restore interruption | In force; acceptance open |
| SC-003 | Device-code challenge display | In force |
| SC-004 | Companion preparation through Add | In force |
| SC-005 | Browser callback isolation | Unresolved |
| SC-006 | Initial account snapshot | In force |
| SC-007 | Three primary pages | Folded into OU-UI-001 |
| SC-008 | Consumed lower thresholds | In force |
| SC-009 | Authentication-required evidence | In force; OU-ALERT-007 open |
| SC-010 | Selected-profile activity | In force |
| SC-011 | Baseline 3.0 replaces baseline 2.0 | Adopted |
| SC-012 | Unknown freshness in summary marks | Unresolved; existing state preserved |
| SC-013 | Rich dashboard under Section 2.3 | Resolved; owner amended OU-DES-003, OU-DES-005 and OU-DES-008 |
| SC-014 | OAuth in place of the app password | Proposed; owner decision open |
| SC-015 | All accounts as Overview's opening view | In force |
| SC-016 | Leaderboard and platforms in the first release | Adopted in SRS Section 0.6 |
| SC-017 | Local usage store | In force; two SRS gaps wait for the owner |
| SC-018 | One consent screen for local logs | In force |
| SC-019 | Local log totals in CLI JSON | In force |
| SC-020 | A read-only CLI and the WAL index | In force |
| SC-021 | Sessions, projects, hours and reasoning from the pinned `ccusage` | In force, amended at L-1g2; quota movement waits for OQ-1 |
| SC-022 | Incomplete metrics from the other local tools | Owner approved; implementation pending |
| SC-023 | Copied Amp usage in the pinned scanner | Confirmed AT-65 gap |
| SC-024 | Legacy local usage during grouped migration | Resolved under owner execution direction |

## SC-024: legacy local usage during grouped migration

Status: resolved, November 9, 2026, under the owner's instruction to compare the evidence, resolve the issues, and continue. The SRS stays unchanged.

OU-LOG-011 preserves accepted totals after log deletion. AT-65 requires copied usage to count once. Schema 10 stores per-folder, per-model summaries without event identities. The writer retains each model maximum independently.

A later grouped report cannot establish how much deleted usage overlaps those summaries. For example, two saved folders with 100 tokens each can represent 100 copied tokens or 200 distinct tokens. Both histories fit the saved summaries.

Each source also pins its own zone. The scanner keeps that source zone. Sources added after a display-zone change can therefore hold different day boundaries. Moving their old dates into one zone would violate OU-LOG-010.

Resolution:

- Preserve all legacy records, differences, source membership, and original zones in encrypted history, export, and backup.
- Start grouped accounting separately. Never add legacy totals to grouped totals or claim that a maximum reconstructs deleted unique usage.
- Show Not available for a combined historical total when overlap or incompatible day boundaries prevent proof. Keep the original source values accessible in the existing detail panel.
- Pin the new accounting scope at its first grouped scan. Keep legacy dates and zones unchanged. A rescan cannot silently replace legacy history.

The main view cannot claim a combined historical total without proof of overlap and common day boundaries. SC-022 governs missing measurements. This reading extends the same rule to legacy history. No real database or logs were inspected.

Source basis: [schema 10](../crates/openusage-storage/src/schema_v10.sql), [saved maxima and zones](../crates/openusage-storage/src/local_logs.rs), and the [per-source scanner](../apps/desktop/src-tauri/src/local_logs.rs). Source review establishes this rationale. [Acceptance traceability](acceptance-traceability.md) records the synthetic migration checks.

Comparison evidence: [ccusage Codex guidance](https://ccusage.com/guide/codex/) explains gaps after local log removal. [CodexBar request identities](https://github.com/steipete/CodexBar/blob/7447bc77c2497323d9c41e40164ce705533493c8/Sources/CodexBarCore/Vendored/CostUsage/CostUsageScanner%2BRequestIdentity.swift) suppress duplicate requests from event evidence. Its [release notes](https://github.com/steipete/CodexBar/releases) preserve a valid cache when roots cannot be read. These examples support keeping history and stating gaps. They give no proof that a merge of legacy sums is correct.

## SC-023: copied Amp usage in the pinned scanner

Status: confirmed acceptance gap, November 8, 2026. The SRS stays unchanged.

OU-DUP-004 places within-tool replay and fork handling in ccusage. AT-65 requires copied logs to count once. A synthetic Amp thread with 162 tokens counted 324 after an identical file copy. The copy kept the same thread and event IDs. The pinned scanner does not suppress this copy. The current upstream release and main source have no fix.

Amp remains disabled. An upgrade alone does not resolve this gap. A fix needs review and new fixture qualification before it can enter the release pin. On November 8, 2026, the owner approved downloads of pinned source and missing locked build dependencies to prepare and test the fix. The owner also replaced the blanket download restriction in the slice rules. This approval does not authorize an upstream message, pull request, or publication.

A local candidate now passes the stable-ID copy and failure cases. The [candidate record](reviews/2026-11-08-amp-dedup-design.md#local-candidate-after-review) holds its evidence and remaining gates. This does not qualify Amp or replace the release pin.

See the [executed fixture](../tests/tools/test_ccusage_formats.py) and the [upstream review](reviews/2026-11-08-ccusage-duplicate-handling.md).

## SC-022: incomplete metrics from the other local tools

Status: owner approved, November 8, 2026. Implementation is pending. The SRS stays unchanged.

The pinned scanner reports totals that its visible token buckets cannot always explain. Some session reports omit times and projects. OpenCode and Hermes can return stored charges even with `--mode calculate`. Pi and OpenClaw can omit hidden extra tokens from calculated cost. The [first](reviews/2026-11-08-l1f-formats-a.md) and [second](reviews/2026-11-08-l1f-formats-b.md) source notes record the exact contracts and synthetic fixture recipes.

The owner chose to show reliable totals and keep unavailable fields unavailable. Each unreliable breakdown, time, project, or estimate must show Not available. The app must not label a remainder as reasoning or invent a session time. Any schema change must preserve exact totals and carry absent values through storage and IPC. Unknown shapes still reject the scan under OU-LOG-007.

This decision authorizes the parser, storage, and interface changes needed for these incomplete reports. Each tool still needs its fixtures and acceptance checks before it is enabled.

## SC-021: sessions, projects, hours and reasoning from the pinned `ccusage`

Recorded November 8, 2026 against baseline 3.0. Status: in force; quota movement waits for the owner's OQ-1 decision.

OU-LOG-017 asks for hourly ranges and for breakdowns by reasoning, session and project. OU-LOG-018 asks for the quota movement of a session. Runs of the pinned `ccusage` 20.0.48 on the synthetic fixtures show what each tool reports:

| Data | Claude Code | Codex |
|---|---|---|
| Sessions (`<tool> session`) | Session ID, first and last activity, project folder, counts | Session ID, last activity, counts with reasoning |
| Project | The folder name that Claude Code writes, which encodes the path | None |
| Reasoning count | None | Yes |
| Hours (`blocks -n 1`) | One-hour blocks that start on whole UTC hours | No blocks |

The app reads these rules as follows:

- **Sessions:** the store saves each session with its key, project and times as `ccusage` reports them. A Codex session has a null start, because `ccusage` gives none. The app does not read a time from a file name.
- **Lower results:** a scan that reads less for a saved session keeps the saved counts, as for days (OU-LOG-011).
- **Reasoning and projects:** a tool that reports no reasoning count or project shows it as missing, not as zero (Section 0.2).
- **Hours:** hourly ranges cover Claude Code only. Where the pinned zone is not a whole number of hours from UTC, a UTC hour covers parts of two local hours, so the app shows no hourly range for that zone.
- **Quota movement:** a local session does not name a provider account. Until the owner settles OQ-1 and links log accounts to profiles, the app has no saved readings for a session and shows no quota movement. It never converts a percentage to tokens.

Amended November 8, 2026, at L-1g2: the hours rule above would hide hourly ranges in zones such as Asia/Kolkata, which is UTC+5:30. The app instead keeps each UTC hour and shows it at its local start time, so in such a zone each hour starts at :30. Schema 12 saves the hours in `local_usage_hours`, and a lower scan keeps the saved counts. The hourly view covers at most 7 days, because 168 bars is the most that one chart shows clearly. A tool without hours shows Not available in the chart key, never zero.

## SC-020: a read-only CLI and the WAL index

Recorded November 8, 2026 against baseline 3.0. Status: in force.

OU-CLI-002 says that the CLI shall not write the database. The CLI opens it read-only and refuses every write. When no WAL file exists, it opens the database as immutable and creates no file. When a WAL file exists, such as while the app runs, SQLite readers can update the shared index in the `-shm` file. That file holds lock and index state, not records. The database file and its WAL stay byte for byte unchanged, and no file is added. The `cli-status` native journey checks both cases.

## SC-019: local log totals in CLI JSON

Recorded November 8, 2026 against baseline 3.0. Status: in force.

OU-CLI-004 asks JSON output to give each value with its state, status, source and observation time. The reading statuses of Section 3.2 judge an adapter reading against other sources. A local log total is the authority only for what the log recorded on this device (Section 0.2, rule 1), so no status in Section 3.2 fits it. The CLI therefore gives local log totals `sourceLevel` `local_log`, the source as the `ccusage` version, the state `current`, the observation time, and a `status` of `null`.

## SC-018: one consent screen for local logs

Recorded November 8, 2026 against baseline 3.0. Status: in force.

Owner decision 9 and OU-LOG-001 ask for one consent screen with one action. With the system key store, the welcome screen already has one action, Create private workspace. The detected tools therefore appear on that screen, all selected, and the one action creates the workspace and saves the choice. With the password fallback, the same list sits above the password form. A workspace from before L-1d has no saved choice, so it shows one Local logs dialog with one action, Continue, after unlock. Escape does not skip it. Settings changes the choice later (OU-LOG-002). An unselected tool is saved as withdrawn, so no screen asks again.

## SC-017: local usage store

Recorded November 8, 2026 against baseline 3.0. Status: in force; the two gaps at the end wait for the owner.

L-1b stores local log totals in the `log_sources`, `local_usage_days` and `local_usage_differences` tables of SRS Section 13. The store reads the rules as follows:

- **Counts:** the pinned `ccusage` reports input, output, cache write, cache read and total counts. The store keeps these five counts as checked decimal text. It keeps no reasoning count, because `ccusage` reports none.
- **Estimated value:** the store keeps the whole number of millionths of a US dollar as digit text. The column name `estimated_value_usd_micros` is the label that Section 13 asks for.
- **Zone:** each source pins its zone at its first saved scan (OU-LOG-010). The store refuses a later scan in another zone for that source.
- **Lower results:** when a scan reads less for a day, the saved counts stay, the day gets `logs_gone` coverage, and the store saves the difference once for each saved and new pair (OU-LOG-011). A scan that reads all days and has no row for a saved day counts that day as zero. A scan that starts at a date does not judge earlier days.
- **Coverage:** the store writes `complete` and `logs_gone`. It does not write `partial`, because no requirement defines it.
- **Sessions and accounts:** `local_usage_sessions` and `log_accounts` are not built, because the daily report has no session or account data. All local totals stay under Unknown account until the owner settles OQ-1 (OU-LOG-015).
- **Consent:** the store refuses rows for a tool that has no granted source in the scan. Since L-1c, each run gives `ccusage` an empty private home and the folder variable of one granted tool only, so `ccusage` cannot see the folders of other tools.

Two gaps wait for the owner:

1. The `LocalUsageDay` type in Section 12 has no cache write field, so its parts do not add up to its total for Claude Code. A `cacheWriteTokens` field would close the gap.
2. The type has a `reasoningTokens` field that no current source fills. It stays null, because a missing count is not zero.

## SC-013: rich dashboard under Section 2.3

Recorded November 3, 2026 against baseline 3.0. Status: resolved November 6, 2026.

The owner tested the S-01 build and said its UI was much worse than the approved reference dashboard. That reference shows more numbers in place, a selected-account card, a full accounts table, page subtitles and second lines under some numbers, such as an exact count under a compact one.

OU-DES-005 allows a number with its name, and OU-DES-007 asks each number to open the detail panel. So the reference's density, cards, tiles and table columns meet Section 2.3. Page subtitles and second lines under numbers do not, because OU-DES-003 forbids taglines and secondary labels. S-03b leaves those two out and keeps exact values in the detail panel. If the owner wants them back, OU-DES-003 needs an owner change first.

On November 6, 2026 the owner rejected the S-03b build as well, asked for usability, interpretability and a modern look, and approved context such as "resets in" beside numbers. OU-DES-003 and OU-DES-005 now allow one short context line per number, and OU-DES-008 says helper text instead of secondary labels. Source, time, scope, coverage and derivation stay in the detail panel under OU-DES-007.

## SC-016: leaderboard and platforms in the first release

Recorded November 7, 2026 against baseline 3.0. Status: adopted in SRS Section 0.6.

The owner asked for a working product to launch for feedback, with the leaderboard in the first release for traction. The owner chose Linux first with Windows and macOS previews, and local logs, sharing, the CLI and npm installation as first-release features. For hosting the owner said to choose the best option that keeps an open-source service safe from malicious use; the [architecture decision](decisions/2026-11-07-leaderboard-service.md) records the choice. The Stage table, owner decisions 7 and 10, and the Section 16 platform rule were amended to match.

## SC-015: all accounts as Overview's opening view

Recorded November 6, 2026 against baseline 3.0. Status: in force.

The owner approved a modern Overview mockup that opens on all accounts, an unverified sum, and asked to build it. With two or more connected accounts, Overview therefore opens on All accounts, the Profile sum view of SC-010, until the person picks an account. The hero, the Share heading and Lifetime tokens carry the Unverified mark, the activity section keeps the SC-010 notice and per-account checkboxes, and Verified only keeps every verified-total gate. With one connected account, Overview opens on that account. This narrows SC-010's "explicit Profile sum view" for the opening state only.

## SC-014: OAuth in place of the app password

Recorded November 3, 2026 against baseline 3.0. Status: proposed; the owner decides.

The owner asked to replace the local app password with OAuth. The app password does not sign anyone in. It protects the database key when no system keychain is available. OAuth proves an identity to a server, and OpenUsage has none, so it cannot protect a local key. A key derived from expiring OAuth tokens would also lock the owner out. Account sign-in already uses OAuth through Codex.

S-03c removes the password instead: the system keychain holds the key by default, a password-protected workspace can move to the keychain, and the password stays only where no keychain exists.

## SC-012: unknown freshness in summary marks

Recorded November 3, 2026 against baseline 3.0. Status: unresolved; no normative or Rust freshness rule changed.

OU-DES-006 asks for one mark using the SRS name for the state. Saved quota IPC also has `freshness: unknown`, including after restart and a failed cache reload. Section 3.1 does not name that freshness state. Calling it Current, Stale, or Unverified would assert evidence the state does not supply; Unverified describes source matching under Section 3.2.

The Overview summary keeps the existing wording Freshness unknown as its single mark. Details retain the observation time and each connected profile's freshness. Incomplete takes precedence when a connected profile has no saved quota; Stale takes precedence over unknown freshness when all profiles have quota. The panel keeps every underlying state. Choosing a normative name for unknown freshness remains an owner decision. This does not approve an identity or source-audit contract.

## SC-001: reset confirmation with rapid new-window use

Status: proposed, not approved. No normative text or runtime reset rule changed.

SRS section 6.2 condition 4 requires a lower usage percentage before reset confirmation. Master Build Prompt D1 identifies a missed-reset case: new-window usage can already equal or exceed the prior reading. A missing percentage drop therefore cannot prove that no reset occurred.

The reviewed [Codex window schema](https://github.com/openai/codex/blob/rust-v0.163.0/codex-rs/app-server-protocol/schema/typescript/v2/RateLimitWindow.ts) contains no explicit generation identifier. The [quota response](https://github.com/openai/codex/blob/rust-v0.163.0/codex-rs/app-server-protocol/schema/typescript/v2/GetAccountRateLimitsResponse.ts) does not establish a replacement reset-proof contract.

Proposed narrow addition: distinguish an overdue expected reset from a reset whose completion evidence is unavailable. Continue bounded full-read reconciliation and preserve the latest measurement. Neither state permits a reset-completed alert or a new durable alert generation. Keep the five existing confirmation conditions until an approved provider-backed rule replaces them.

Implementation and migration remain not started. The future Rust state machine must preserve identity and bucket compatibility, stale-reading rejection, and alert deduplication across restarts. Acceptance must cover rapid new-window use, later confirmation, unknown reset times, sparse updates, clock changes, and repeated reads.

This proposal does not authorize treating elapsed time, a moved timestamp, or changed percentage alone as confirmation.


## SC-002: interruption after restore COMMIT and before acknowledgment

Status: implementation interpretation recorded for acceptance review. No normative text changed. Power-loss and native interruption acceptance remain open.

OU-BACKUP-010 requires the previous valid database to survive a failed or interrupted restore. OU-NFR-007 requires transactional writes. Before COMMIT, the previous live records survive. After COMMIT but before acknowledgment, complete restored records survive, while the encrypted pre-restore checkpoint preserves the previous records. A lost reply does not prove rollback.

The owned-process tests execute the production transaction and terminate their child before COMMIT and after COMMIT without acknowledgment. Both cases retain the original encrypted checkpoint. The user can prepare a new preview after restart rather than retry a consumed token. This interpretation does not permit partial records, deletion of the checkpoint, or automatic confirmation. See [the review](reviews/2026-10-18-restore-process-crash.md).


## SC-003: transient device-code challenge display

Status: the user approved the narrow interpretation on October 18, 2026. No normative SRS text changed.

OU-ACCT-005 requires device-code login, while section 19.1(1) broadly prohibits secret frontend payloads. The approved scope permits only the selected profile's validated verification URL and human user code in transient typed IPC after explicit consent. The code remains a sensitive challenge. OAuth tokens, provider login identifiers, device authorization secrets, credentials, and database keys stay in Rust or the provider.

Codes must stay out of logs, notifications, toasts, exports, backups, application persistence, and retained real test evidence. Terminal states and approval invalidation clear the challenge. Clipboard retention remains under operating-system control. See the [exact approved interpretation](proposals/device-code-display-interpretation.md). Synthetic native checks passed. Real OAuth remains unexecuted.

October 21 implementation update: the user's explicit per-account Sign in action supplies selected-profile consent. A separate Get sign-in code confirmation is no longer required. The named pending dialog retains Cancel; the fixed official verification page opens automatically. Production anonymous challenge/cancellation passed without retaining a code. Successful real OAuth remains unverified. See the evidence (private evidence omitted).

## SC-004: trusted companion preparation through Add

Status: user-directed implementation interpretation, October 21, 2026. The normative SRS stays unchanged.

The user requires normal onboarding without manual executable paths or technical approval panels. Add or Connect now authorizes preparation of the exact reviewed official companion. Rust retains the source pin, held-file checks, isolated homes, and process controls required by OU-PROTO-002/004/005. Sign-in retains explicit consent for the selected account. A present invalid companion fails closed. No credential import or runtime download is permitted.

Technical review remains available in Settings for troubleshooting. Test fixtures still require the explicit test-build approval path and cannot pass normal production preparation. See the [source review and decision](reviews/2026-10-21-onboarding-sources.md). This change does not grant another profile access to an active sign-in or saved reading. Scoped native Linux qualification passed. Real OAuth and release-platform acceptance remain open.

## SC-005: browser callback isolation remains unresolved

Status: confirmed implementation restriction, October 21, 2026. No normative requirement changed or waived.

OU-ACCT-005 requires browser login. OU-AUTH-007 restricts authentication operations to the selected profile. The pinned provider can cancel another listener on its fixed callback port. A probe reproduced that behavior against owned listeners. No supported app-server port override was found. A precheck leaves a race and does not establish isolation. The browser draft stays outside the working build while this conflict remains unresolved. Official device-code login remains available. See [the source review and executed result](reviews/2026-10-21-official-authentication.md).

## SC-006: initial account snapshot before measurements

Status: source-backed implementation correction, October 21, 2026. No normative requirement changed.

The pinned official provider sends an initial routing snapshot through `account/updated`. It is not by itself evidence of an account switch. Rust accepts one validated matching snapshot before the measurement baseline, supported by a full account response and routing context. A deferred snapshot requires another full account check before measurement requests. The snapshot never establishes identity, freshness, or reset evidence. Mismatches, duplicate snapshots, malformed envelopes, and all later account-update notifications still invalidate the collection. Message and read deadlines remain bounded. See the source and executed evidence (private evidence omitted).

## SC-007: three primary pages with contextual account management

Status: explicit user-directed navigation correction, October 21, 2026. The normative SRS stays unchanged.

OU-UI-001 lists Accounts and other functional views. The user requires only Overview, Usage and Settings in primary navigation. Account selection, sign-in, management and details remain accessible through Overview, Usage and contextual dialogs. This changes navigation placement, not the required capabilities. Unimplemented Compare, Subscriptions, Calendar and Notifications functions remain acceptance gaps; removing their navigation entries does not waive them.

Show each returned quota window. Do not infer a five-hour or weekly limit from the plan name. Refresh must preserve the current Overview route. See native evidence (private evidence omitted).


## SC-008: consumed lower thresholds and desktop delivery

Status: implementation interpretation, October 22, 2026. No normative text changed.

OU-ALERT-018 consumes lower satisfied thresholds after the first reading selects the highest threshold. The saved Covered state means that no separate OS notification was submitted for that lower level. It does not claim OS delivery. The consumed key survives restore and Undo even if the higher notification fails. Later crossings remain separate under OU-ALERT-002. The normative SRS is unchanged.

## SC-009: authentication-required evidence

The approved app-server interface can report `AuthenticationRequired`. For a profile with an accepted reading from the same home and association, OpenUsage can report that sign-in is required. This response does not establish `invalid_grant` or confirmed invalid credentials. Missing identity, unsupported methods, malformed responses, cancellation and stale operations do not qualify as authentication proof.

OU-ALERT-007 remains open for confirmed invalid-credential evidence through a supported interface. The added sign-in alert is a narrower capability, not a change to that requirement. The normative SRS is unchanged.

## SC-010: approved unverified selected-profile activity

The owner approved [the narrow amendment](proposals/user-curated-profile-activity.md) on October 22, 2026. An explicit Profile sum view may calculate observed subtotals and shares from the same pinned provider contract and matching provider dates. Its visible Unverified label and details retain unknown identity, application scope and day-definition limits. This does not certify compatibility or distinct accounts. Known duplicate identities and known contract differences are excluded. Verified combined totals retain every existing identity, scope and live-contract gate. The normative SRS file remains unchanged.

## SC-011: baseline 3.0 replaces baseline 2.0

Status: adopted November 2, 2026.

The owner's November 2 decisions conflicted with baseline 2.0. They widen the product to many tools, permit local logs and auditable Unofficial sources, add an opt-in hosted leaderboard, remove promotions, payments and receipt import, make paid code signing optional and cut secondary labels. [Baseline 3.0](SRS.md) records them, and its Section 22.1 lists every change. Baseline 2.0 remains in Git history at commit `5f1c2c7`.
