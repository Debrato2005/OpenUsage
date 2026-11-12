# Data contracts

Reviewed: November 12, 2026, against source `9d9b113`. This document describes
implemented records and interfaces. [SRS Sections 12 and 17](SRS.md#17-data-contracts-and-numeric-rules)
remain normative. Planned fields stay explicit below. This document adds no API,
migration, adapter support, or acceptance result.

## Contract owners and versions

| Boundary | Canonical definition | Current version or result |
|---|---|---|
| Encrypted application records | [Storage migrations](../crates/openusage-storage/src/lib.rs) and `schema.sql` through `schema_v13.sql` beside it | Database schema 13, SQLCipher with WAL |
| Rust to renderer | [Commands, result types, and validators](../apps/desktop/src/contracts.ts) | Typed commands and results. No single version covers every command |
| CLI JSON | [CLI output contract](../crates/openusage-cli/README.md#output) | `schemaVersion: 2` |
| Complete application export | [Writer](../crates/openusage-storage/src/application_export.rs) | `format: openusage.application-records`, `formatVersion: 1`, `schemaVersion: 13` |
| Selected reading exports | [Record columns](../crates/openusage-storage/src/readings_export.rs) and [file writer](../apps/desktop/src-tauri/src/readings_export.rs) | JSON uses `schemaVersion: 1`. Latest, quota-history, and activity-history selections stay distinct |
| Encrypted backup | [Archive codec](../crates/openusage-import/src/lib.rs) and [backup validation](../crates/openusage-storage/src/backup_validation.rs) | Archive format 1, with a separate database schema version |

A database schema number does not version CLI JSON or an export format.
Field declarations and limits stay in their linked source. Consumers must use
those definitions rather than infer a field from a label or another route.
The [architecture](architecture.md) owns process, storage, and IPC boundaries.

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

Subscriptions are profile-owned, user-confirmed local terms. `price: null` is
unknown. A supplied zero price is free. `amountMinor`, `currencyScale`, dates, renewal
choice, and an opaque revision preserve those terms. They are not provider bills,
payments, exchange-rate conversions, or proof of a subscription cancellation.

## Record catalog

The following groups cover all 42 tables in schema 13. The migrations own columns,
constraints, indexes, and foreign keys. Table membership does not imply shared
identity or permission to combine metrics.

| Family | Tables |
|---|---|
| Workspace and association | `settings`, `profiles`, `accounts`, `profile_identity_events`, `profile_bindings` |
| Provider records | `collection_runs`, `activity_summaries`, `current_activity`, `current_activity_days`, `activity_day_history`, `quota_snapshots`, `quota_limits`, `credit_snapshots`, `current_quota_readings`, `current_quota`, `quota_reset_assessments` |
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
or `status: unavailable` for a metric value. These are not the planned SRS
reading statuses. Unsafe formula-like CSV text is refused.

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

## Planned contract fields and acceptance

SRS Section 17 defines `MetricState`, `SourceLevel`, `ReadingStatus`, and
`evidenceId`. Current provider DTOs instead expose their explicit freshness,
source-kind, identity, scope, and coverage fields. They do not yet expose the
complete uniform evidence/status contract. `official_artifact` is not a substitute
for every `ReadingStatus` or identity condition. S-04 and S-05 remain open.
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
