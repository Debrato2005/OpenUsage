# Data contracts

Updated: November 19, 2026. The [evidence record](reviews/2026-11-16-durable-reading-evidence.md) covers the schema-14 change. This document describes
implemented records and interfaces. [SRS Sections 12 and 17](SRS.md#17-data-contracts-and-numeric-rules)
remain normative. Planned fields stay explicit below. This document adds no API,
migration, adapter support, or acceptance result.

## Contract owners and versions

| Boundary | Canonical definition | Current version or result |
|---|---|---|
| Encrypted application records | [Storage migrations](../crates/openusage-storage/src/lib.rs) and `schema.sql` through `schema_v14.sql` beside it | Database schema 14, SQLCipher with WAL |
| Rust to renderer | [Commands, result types, and validators](../apps/desktop/src/contracts.ts) | Typed commands and results. No single version covers every command |
| CLI JSON | [CLI output contract](../crates/openusage-cli/README.md#output) | `schemaVersion: 2` |
| Shared leaderboard upload | [Shared projection](#shared-leaderboard-upload) and [CLI preview contract](../crates/openusage-cli/README.md#offline-leaderboard-preview) | Exact prospective `/v1/sync` body. It is not wrapped in CLI schema 2 |
| Leaderboard HTTP transport | [Client types](../crates/openusage-board/src/protocol.rs) and [service API](../services/leaderboard/README.md#api) | V1 numeric wire values. Approved uploads retain their exact bytes. Rank counts reject values above the safe integer limit. Missing estimates remain null |
| Leaderboard credential vault, added November 15 | [Vault contract](../crates/openusage-keys/README.md#leaderboard-credential-storage) and [enrollment format](../crates/openusage-board/README.md#enrollment-credential-format) | Pending and active version 1 records inside the opaque OS slot. The [session contract](../crates/openusage-board/README.md#serialized-enrollment) owns recovery and origin binding |
| Fresh departure and local forgetting | [Recovery contract](../crates/openusage-cli/README.md#departure-recovery-and-local-forgetting) and [service API](../services/leaderboard/README.md#api) | Fresh departure requires an explicit nullable token witness. CLI schema 2 reports confirmed remote state separately from local credential state. Login and device ID are not ownership proof |
| Complete local enrollment erasure | [Vault erasure contract](../crates/openusage-keys/README.md#complete-local-credential-erasure) | The existing local-deletion intent stays version 1. It contains no leaderboard token or payload. A failed erasure retains the intent for explicit resume |
| Complete application export | [Writer](../crates/openusage-storage/src/application_export.rs) | `format: openusage.application-records`, `formatVersion: 1`, `schemaVersion: 14` |
| Selected reading exports | [Record columns](../crates/openusage-storage/src/readings_export.rs) and [file writer](../apps/desktop/src-tauri/src/readings_export.rs) | JSON uses `schemaVersion: 2`. Latest, quota-history, and activity-history selections stay distinct |
| Encrypted backup | [Archive codec](../crates/openusage-import/src/lib.rs) and [backup validation](../crates/openusage-storage/src/backup_validation.rs) | Archive format 1, with a separate database schema version |

A database schema number does not version CLI JSON or an export format.
Field declarations and limits stay in their linked source. Consumers must use
those definitions rather than infer a field from a label or another route.
The [architecture](architecture.md) owns process, storage, and IPC boundaries.

## Settings exchange contract

The commands are load_leaderboard, refresh_leaderboard, begin_leaderboard,
cancel_leaderboard, confirm_leaderboard_sync, and open_leaderboard_login_page.
All reject unknown arguments. Actions require explicit confirmation.
Sync accepts 1 through 366 days and defaults to 30 days.
Other actions reject day selection.
Operation IDs are random UUIDs. The browser and confirmation commands require the current operation ID.

The view separates current enrollment metadata from the last operation receipt.
A failed inspection keeps a confirmed receipt and adds a separate status issue.
Enrollment can be unchecked, absent, pending, joined, invalid, or unavailable.
An unavailable credential store never means no enrollment.
Only joined metadata supplies a current login.
Receipts distinguish confirmed success, cancellation, failure before public dispatch, confirmed partial results, and unknown public effects.
Accepted row counts cross IPC as decimal strings.

The renderer checks the exact upload whitelist, canonical JSON, row keys, dates, known tools, bounds, and null estimates.
Rust owns arithmetic and service validation.
The preview is the sole exception to ordinary decimal-string count fields because it displays the exact v1 numeric wire body.


## Own-rank exchange

The command load_leaderboard_rank accepts only period and metric.
Periods are 7d, 30d, and all. Metrics are tokens and value.
The response includes Self-reported, the selected period and metric, the snapshot time, eligible participant count, and an optional entry.
The entry contains rank, public login, total tokens, and nullable estimated value in USD millionths.
Counts cross IPC as canonical decimal strings within the existing v1 safe-integer limit.
The snapshot time uses canonical UTC milliseconds.
A missing entry is Not available. A missing estimate stays null, and a measured zero remains zero.
The renderer rejects private fields, invalid counts, and mismatched selections.
The service owns eligibility and ordering. The desktop adds no filter or rank calculation.


## Meaning of identifiers and provenance

The [draft identity contract](identity-contract.md) owns identity policy,
reassociation, duplicate limits, and the pending approval and live-test gates.
The record fields below remain the current implemented contract.

A profile UUID identifies one local profile. Its managed home reference identifies
the approved environment. A binding UUID identifies one accepted association with
that environment and descriptive account evidence. A collection-run UUID identifies
one accepted method result. These identifiers are not verified provider identity.

[ProfileBinding and reading inputs](../crates/openusage-storage/src/activity.rs)
carry the profile, home, binding, source kind, provider version, and times.
The coordinator retains executable approval, request, generation, and physical-home
checks through acceptance. Renderer approval IDs and request IDs authorize a
specific operation. They do not replace the stored binding or collection-run IDs.
Home references and storage revisions for profile removal stay in Rust.
The [profile-removal boundary](architecture.md#profile-removal) owns their lifetime.

Accepted provider quota and activity readings have provisional identity and
`verifiedIdentityKey: null`.
Email and plan describe the association. Neither establishes a stable identity.
`official_artifact` and `test_fixture` classify the executable source. They do not
prove compatible scope, complete coverage, distinct accounts, or verified totals.
Current quota and activity scopes explicitly retain unknown application scope.

The [collection writer](../crates/openusage-storage/src/collection.rs) can accept
quota, activity, or both supported methods in one transaction. Two accepted methods
keep distinct run IDs and matching association and source metadata. A missing
capability preserves its previous accepted state unless approved reassociation
clears it. A failed or cancelled collection cannot publish half of a supported pair.

## Reading evidence

[Evidence inputs and lookup](../crates/openusage-storage/src/reading_evidence.rs) belong to Rust.
Every new activity or quota method acceptance requires evidence with matching run, method, profile, binding, provider version, origin, and observation time.
This includes the delayed, non-live acceptance routes.
A combined collection commits one record per accepted method in the same transaction.
Replayed run IDs and malformed or mismatched evidence fail without retained writes.

Three versions have different meanings: the Codex provider version, the adapter revision, and the response projection version.
The current adapter ID is `codex_app_server`, adapter revision is 1, and response projection version is 1.
The adapter revision changes when its parser or checks change.
Evidence records the method observation time from its parent collection run.
The fixed parser checks are `rpc_envelope`, `request_id`, and `response_fields`, each `passed`.
The comparison result is `not_performed`. No independent comparison is implied.

Source level and reading status are `official` for the existing official Codex interface.
The separate executable origin remains `official_artifact` or `test_fixture`.
A synthetic fixture does not establish real-provider qualification or verified identity.
The provisional account association remains separate from response evidence.

Evidence stores lowercase hexadecimal SHA-256 hashes for the redacted response and format fingerprint.
The [producer contract](../crates/openusage-codex/README.md#response-evidence-producer) owns projection and privacy rules.
Unused-field metadata contains positive counts at nine fixed schema locations, without names or values.
The record contains no email, home path, account description, prompt, code, token, or cookie.
SQL rejects updates after insertion. Explicit record deletion follows the existing collection-run cascade.

Trusted migration marks preexisting method runs in `reading_evidence_legacy`.
It invents no historical hashes, checks, adapter revisions, comparisons, or status records.
Lookup returns `LegacyGap` for a marker, `None` for an unknown or mismatched target, and an error for unexplained or conflicting absence.
Complete records and markers cannot coexist for one run.
Schema-14 snapshot checks require exactly one of them for every collection run.
The [SC-031 conflict](srs-clarifications.md#sc-031-evidence-for-historical-readings) remains open. It approves no exception to OU-AUD-001.

Encrypted backup validation supports schemas 1 through 14.
Trusted normalization preserves old values and adds only the migration markers.
Restore, checkpoints, and complete application export preserve the two evidence tables.
Selected reading exports and current desktop quota and activity reads carry saved audit metadata.

## Evidence access

The Rust lookup accepts one `ReadingEvidenceTarget`: profile UUID, method, and run UUID.
One SQL snapshot requires all three to match.
A wrong target returns no record. Malformed identifiers and corrupt evidence remain errors.
The read changes no stored record and requires no provider approval or request.

The command `load_profile_reading_evidence` accepts only `profileId`, `runId`, and `method`.
Methods are `activity` and `quota`.
The workspace must be unlocked and the profile must exist. Archived profiles remain readable.
A complete response contains `profileId`, `availability: "complete"`, and the validated record.
A historical response contains only `profileId`, `availability: "legacy_gap"`, `runId`, and `method`.
An unavailable target returns null. A failed read never becomes a historical gap.

The renderer rejects extra fields and binds each response to the requested profile, method, and run.
It checks hashes, canonical dates, numeric versions, fixed checks, known locations, and count bounds.
Evidence dates use positive Gregorian years, as Rust storage requires.
The Overview activity and quota panels load evidence only after Show.
Their cache also binds provider version, origin, and observation time.
A late or mismatched response cannot replace another selection. Evidence failure preserves the displayed metric.

Normal cards remain unchanged. The existing panel shows status, source level, adapter revision, projection version, hashes, checks, comparison, and unused-field counts.
A historical gap shows no invented status or hashes.
The current-reading contract below covers saved quota and activity.
Other evidence clients and all-status alerts remain open.

## Exact values, missing values, and units

| Value | Contract |
|---|---|
| Provider counts and summary durations | Canonical nonnegative decimal strings. [Storage validation](../crates/openusage-storage/src/activity.rs) accepts values through `i64::MAX` |
| Local record counts and estimates | Grouped records use bounded `u64` values. [Grouped validation](../crates/openusage-storage/src/local_periods.rs) currently caps each metric at `1_000_000_000_000_000` |
| Local totals across periods | [Shared projections](../crates/openusage-storage/src/local_usage_read.rs) use `u128` and cross IPC as decimal strings |
| Quota percentage | Current storage uses an integer from 0 through 100. Derived remaining percentage is `100 - usedPercent` |
| Subscription price | [Rust parses](../crates/openusage-storage/src/subscriptions.rs) decimal major units into exact minor units using the stored currency scale. No floating-point rounding |
| Local API-price estimate | Integer millionths of USD. It is an estimate, separate from subscription money and provider credits |
| Provider dates | Original valid Gregorian `YYYY-MM-DD`, without timezone conversion |
| Observation times | UTC timestamps identify when a result was observed. They do not replace missing activity times |
| Reset deadlines | Unix seconds in quota windows. Assessments use UTC timestamps. Elapsed time alone does not confirm reset |

For example, `"9007199254740993"` remains exact across the renderer boundary.
Do not convert an exact count into a JavaScript `number` before summing it.
For USD at scale 2, subscription price `"0.10"` has `amountMinor: "10"`.
A local estimate of `"100000"` microdollars also displays as USD 0.10, but has
a different meaning. Display rounding never changes the saved source value.

A reported zero is a measurement. An absent value stays `null` or the route's
explicit unavailable state. An empty selected range is not a zero observation.
An unavailable component makes its aggregate unavailable. Independent reported
totals do not become the sum of incomplete model or token details.
[SC-022](srs-clarifications.md#sc-022-incomplete-metrics-from-the-other-local-tools)
owns the missing-metric reading.

## Provider quota and activity

[Quota types](../crates/openusage-storage/src/quota.rs) separate limit identity,
optional primary and secondary windows, reset credits, and optional credit details.
A missing window is `null`. A missing deadline remains unknown. Saved quota adds
the local reset assessment: `expected`, `due`, `unconfirmed`, or `confirmed`.
[Reset assessment](../crates/openusage-storage/src/quota_resets.rs) requires its
recorded conditions. A timer reaching zero does not create a confirmed generation.

[Activity types](../crates/openusage-storage/src/activity.rs) separate the summary,
the latest daily-response state, current days, and accepted daily history.
`dailyState` is `unavailable`, `empty`, or `returned`. Saved current days can remain
when a newer response supplies no daily rows. Each retained day keeps its own
observation and provenance. Daily rows replace the accepted value for that date.
Repeated snapshots do not add the same day again.

Desktop freshness is `unknown`, `fresh`, or `stale`. Saved connection state and
clock freshness describe different facts. A current-looking number does not
prove the account remains connected. The [CLI contract](../crates/openusage-cli/README.md)
owns its distinct `current`, `stale`, and `unknown` clock rules.

Overview activity summaries use their saved activity source, version, observation, and freshness.
Quota and reset-credit details use saved quota metadata. Missing activity never borrows quota provenance.

`load_profile_quota` and `load_profile_activity` return null when the profile has no saved reading.
A saved reading includes an `audit` object. Complete evidence supplies
`availability: "complete"`, `readingStatus: "official"`, and `sourceLevel: "official"`.
A trusted historical marker supplies only `availability: "legacy_gap"`.
Corrupt evidence fails the read. The compact object carries no response hashes or check details.

Each saved activity day includes its own binding ID, collection run ID, provider version,
executable origin, observation time, and audit object. The current summary and daily rows
use one storage snapshot. The renderer checks that rows from the same run agree on their context.

Individual activity and the explicitly Unverified profile sum stay separate.
Verified combined totals remain unavailable without approved identity and scope.
[SC-010](srs-clarifications.md#sc-010-approved-unverified-selected-profile-activity)
owns the allowed profile-sum reading. A profile sum does not become account proof.

## Local usage and subscriptions

[LocalUsageRead](../crates/openusage-storage/src/local_usage_read.rs) keeps grouped
periods, unplaced records, and legacy folder history separate. Dated grouped
periods use their saved timezone. Unplaced periods and untimed sessions stay
outside date filters. A scan time never invents an activity date.

The independent period total remains separate from model subtotals and sessions.
Input, output, cache-write, cache-read, reasoning, project, activity times, and
estimates can be missing. Reasoning is part of output where the source defines
it that way. It must not be added again as another independent token class.
The [local logs contract](../crates/openusage-locallogs/README.md#grouped-scans)
owns source policies, report requirements, scanner pins, and qualification limits.

A lower scan records a difference and keeps accepted usage. Retained records keep
their original scan provenance. Latest scan metadata stays separate.
[SC-024](srs-clarifications.md#sc-024-legacy-local-usage-during-grouped-migration)
prevents silent replacement or combination of legacy folder totals.
Only Claude Code and Codex are enabled. Candidate contracts do not enable tools.

`LocalLogsView` optionally returns `connections`, with one tool and one declared connection in each entry.
Rust requires current detection and saved consent. Empty suggestions omit the field. Legacy responses without it remain valid.
The renderer rejects undeclared, duplicate, absent, or unconsented suggestions.

A connection suggestion describes a supported flow. It proves no account identity, profile association, or quota reading.
Overview reuses its existing account control to open the unselected account manager.
Profile creation and sign-in retain their existing explicit actions.
The tool detail panel shows unsupported quota connections as Unsupported.

Subscriptions are profile-owned, user-confirmed local terms. `price: null` is
unknown. A supplied zero price is free. `amountMinor`, `currencyScale`, dates, renewal
choice, and an opaque revision preserve those terms. They are not provider bills,
payments, exchange-rate conversions, or proof of a subscription cancellation.

## Record catalog

The following groups cover all 44 tables in schema 14. The migrations own columns,
constraints, indexes, and foreign keys. Table membership does not imply shared
identity or permission to combine metrics.

| Family | Tables |
|---|---|
| Workspace and association | `settings`, `profiles`, `accounts`, `profile_identity_events`, `profile_bindings` |
| Provider records | `collection_runs`, `activity_summaries`, `current_activity`, `current_activity_days`, `activity_day_history`, `quota_snapshots`, `quota_limits`, `credit_snapshots`, `current_quota_readings`, `current_quota`, `quota_reset_assessments` |
| Reading evidence | `reading_evidence`, `reading_evidence_legacy` |
| Local usage | `log_sources`, `local_usage_days`, `local_usage_differences`, `local_usage_sessions`, `local_usage_hours`, `local_usage_scopes`, `local_usage_scans`, `local_usage_scan_roots`, `local_usage_records`, `local_usage_record_models`, `local_usage_scan_differences` |
| Subscription terms | `subscriptions` |
| Alerts and delivery | `alert_events`, `notification_event_ids`, `reset_alert_preferences`, `reset_alert_overrides`, `reset_notification_outbox`, `reset_notification_receipts`, `threshold_alert_preferences`, `threshold_alert_overrides`, `threshold_alert_events`, `operational_alert_preferences`, `operational_alert_overrides`, `operational_alert_events`, `operational_alert_outcomes`, `operational_alert_state` |

Current projections and historical snapshots have different purposes. Alert
history, queued delivery, submission receipts, and operational incident state also
have different lifetimes. The [removal contract](architecture.md#profile-removal)
defines which profile-owned rows disappear and which delivery controls remain.
Logical deletion makes no physical-erasure promise. Earlier backups can restore
removed history through the separate restore flow.

## Exports, backups, and privacy

Selected reading exports have explicit scope and identifier consent. Latest
readings and history routes are distinct. History has binding and run ID columns.
Identifier consent controls their values.
The source column lists own exact field order. Exact values remain strings.
JSON uses `null` and CSV uses an empty cell for a missing value. The selected provider export uses `status: available`
or `status: unavailable` for a metric value. The separate `reading_status` field carries the saved SRS reading status when evidence is complete. Unsafe formula-like CSV text is refused.

## Selected reading audit fields

Selected reading JSON uses schema version 2.
Latest readings, quota history, and activity history keep their existing scope and identifier consent.
CSV appends the same eleven columns after each route's original columns.
Metric `status` remains `available` or `unavailable`.
It describes the metric value, not the reading status.

| Field | Meaning |
|---|---|
| `evidence_availability` | `complete`, trusted `legacy_gap`, or `not_available` for an absent reading |
| `reading_status`, `source_level` | Saved status and adapter source level when complete |
| `adapter_id`, `adapter_revision`, `projection_version` | Saved adapter and projection metadata |
| `redacted_response_hash`, `format_fingerprint` | Saved lowercase SHA-256 digests |
| `check_results`, `unused_fields` | Canonical JSON text within a scalar cell |
| `comparison_result` | Saved comparison result |

All scalar values remain strings. JSON uses null for an absent field, and CSV uses an empty cell.
Complete evidence can accompany an unavailable metric.
Historical gaps supply no invented status, source level, adapter, hash, checks, or comparison.
Absent readings supply no audit values beyond their availability.
An unexplained gap, wrong target, or corrupt evidence refuses the complete export.
The existing strict evidence reader runs inside the export's deferred transaction.
Profile metadata, values, and evidence share one WAL snapshot.
Retained daily values keep their own observation's evidence.

The audit cells add no raw response, private field name, credential, email, code, prompt, or path.
Existing source-kind fields keep synthetic fixture provenance explicit.
Run and binding references retain the route's existing alias and identifier-consent rules.
No new identifier field accompanies audit metadata.
SC-031 remains open. These fields do not establish historical compliance or real-provider qualification.


The complete application export streams one validated database snapshot. Every
SQLite integer, including a flag or scale, becomes a decimal string. TEXT remains
a string, and SQL NULL becomes JSON null. Table column metadata preserves the
declared storage type. Do not apply this encoding to typed IPC booleans or numbers.
The export includes saved descriptions, local provenance, subscription terms,
and retained journals. It is not an anonymized reading export or a restore command.
It contains no provider credential home or external log file.

The encrypted backup codec authenticates its archive header, manifest, and frames.
Its separate schema field identifies the database version. The app validates
schema, ownership, paths, and approved restore intent before replacing records.
[Architecture](architecture.md) and the [threat model](threat-model.md) own these
boundaries. Credentials, provider authentication sessions, browser cookies, and auth-file contents
never become application database records or renderer responses.

## Local share images

The [share commands](../apps/desktop/src-tauri/src/share.rs) accept an optional
`range`: `year`, `7d`, `30d`, or `month`. Omission preserves the 365-day view.
Only Wrapped accepts shorter ranges. Rust selects dates in the saved source
zone before it reads and aggregates history. A calendar month means its first
day through the current source day.

Wrapped returns exact decimal token and estimate values in `details`, with
dates, zone, saved/missing day counts, and estimate coverage. No-data returns
null values, not measured zero. The image retains the Estimated label and
an Incomplete mark when only part of saved usage has an estimate. Provider
identifiers, paths, project names, and account labels do not enter images.
The [typed renderer contract](../apps/desktop/src/contracts.ts) owns validation.

## Planned contract fields and acceptance

Baseline 3.1 adds private snapshot requirements in SRS Sections 14B and 14C.
Private wire format, service, and storage migration remain planned.
The future snapshot must preserve exact values, nulls, scope, source times,
and revision order. It must not reuse the public leaderboard schema for
provider readings or copy the local database to a cloud service.

The [response evidence producer](../crates/openusage-codex/README.md#response-evidence-producer) supplies metadata to Rust callers.
The desktop and storage retain that metadata under the contract above.
The [evidence access contract](#evidence-access) supplies Overview detail disclosure.

SRS Section 17 defines `MetricState`, `SourceLevel`, `ReadingStatus`, and
`evidenceId`. Current desktop quota and activity expose the compact audit fields above.
Other provider views still need the complete evidence and status contract. S-04 and S-05 remain open.
Summary durations and streak counts currently use strings, unlike the SRS numeric
examples. [SC-027](srs-clarifications.md#sc-027-section-17-and-current-dtos) records this
open representation gap. The [current status](implementation-status.md) owns other product gaps.

[Acceptance traceability](acceptance-traceability.md) owns executed evidence.
Contract and source references here describe the reviewed implementation.
The source at `9d9b113` passed hosted Linux checks and Renderer in
[run 43210371418](https://github.com/Debrato2005/OpenUsage/actions/runs/43210371418).
That run includes Rust tests, qualification builds, and native oracle tests.
Native GUI results remain separate private synthetic Linux evidence.
Writing this document does not qualify real providers, other platforms, or planned adapters.

## Shared leaderboard upload

`Storage::leaderboard_upload(device_id, days, now)` returns the exact v1 upload string.
It reads saved local aggregates through one transaction on read-write or query-only connections.
It must run outside another transaction.
A failed read or refusal drops the transaction and returns no upload.
The caller must release its storage lock before authorization, confirmation, or HTTP.

The [CLI preview contract](../crates/openusage-cli/README.md#offline-leaderboard-preview) owns selection and refusal rules.
The shared builder preserves field order, limits, pinned day zones, unknown estimates, and overlap refusal.
It reads no provider reading or credential.
It starts no scanner and makes no network request.
The CLI maps typed errors to its existing codes and messages.
Settings can use this storage boundary, but its caller remains unbuilt.
