# SC-022: incomplete local metrics

Date: November 8, 2026. Status: implementation proposal under [approved SC-022](../srs-clarifications.md#sc-022-incomplete-metrics-from-the-other-local-tools).

## Recommendation

Store one authoritative total for each tool accounting scope and period. Keep consented folders as separate source records. Store model details as children, with nullable measurements. A period is either a reliable local day or unplaced usage. Give sessions nullable activity times and keep them separate from the period total. Never add session totals to daily totals.

Use a conservative per-tool policy for the pinned scanner. A valid date string alone cannot establish when usage occurred. For the first implementation, date only adapters that require an event timestamp on every accepted usage record. Show the other tools in an explicit unplaced scope, with Date shown as Not available.

This review inspected the current working tree and the two pinned-format notes. Graphify ran first. It changed no implementation and executed no fixture, migration, or native test. The source rules below need pinned-binary qualification before a tool becomes enabled.

## Current constraints

The [daily parser](../../crates/openusage-locallogs/src/report.rs) returns only model rows. It derives each total from four buckets and requires those sums to equal the tool, day, and report totals. The [storage validator](../../crates/openusage-storage/src/local_logs.rs) repeats that equality. The pinned scanner can retain extra tokens only at the tool/day level. Its model JSON omits that allocation. See [format A](2026-11-08-l1f-formats-a.md#shared-output-contract) and [format B](2026-11-08-l1f-formats-b.md#common-focused-json).

[Schema 10](../../crates/openusage-storage/src/schema_v10.sql) keys saved days and differences by source, date, and model. The writer keeps each model's higher total independently. [Schema 11](../../crates/openusage-storage/src/schema_v11.sql) requires session end time and all four buckets. Its reader filters sessions by that end time. Nullable fields alone will not solve these aggregation and range problems.

The [desktop reducer](../../apps/desktop/src-tauri/src/local_usage.rs), [share reducer](../../apps/desktop/src-tauri/src/share.rs), and [CLI status](../../crates/openusage-cli/src/status.rs) all sum model rows. They must switch to the authoritative period records. Otherwise a retained model row can inflate a new tool total.

## Date policy for the sixteen tools

All source links use commit `8995f3b7fd7110b8f37564f7d0468acc7a7d6b20`. “Dated candidate” means source inspection supports qualification. It is not an executed acceptance result. “Unplaced” is the conservative proposal when output cannot prove event-date attribution for every accepted record.

| Tool | Pinned source behavior | Proposed first policy |
|---|---|---|
| [Amp](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/amp/src/parser.rs) | Ledger events require a parseable timestamp. Message fallback requires usage or message time. Invalid or missing times skip the entry. | Dated candidate |
| [Pi](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/pi/src/parser.rs) | Each accepted assistant usage needs a parseable record timestamp. | Dated candidate |
| [Kilo](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/kilo/src/parser.rs) | Each accepted message needs positive numeric `time.created`. Seconds and milliseconds are accepted. | Dated candidate |
| [ZCode](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/zcode/src/parser.rs) | Each `model_usage` row needs positive millisecond `started_at`. This is the usage row's time, not a session-wide counter. | Dated candidate |
| [OpenCode](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/opencode/src/parser.rs) | Message time can fall back to epoch. Unbounded session reports can also contain cumulative session aggregates. | Unplaced |
| [Droid](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/droid/src/parser.rs) | `providerLockTimestamp`, then file modification time. The counters describe the session snapshot. | Unplaced |
| [Codebuff](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/codebuff/src/parser.rs) | Message time, chat-folder time, then file modification time. | Unplaced |
| [Hermes](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/hermes/src/parser.rs) | Positive numeric session `started_at` is required, but it dates cumulative session counters. | Unplaced |
| [Goose](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/goose/src/parser.rs) | Session `created_at` is required. It can be date-only. Accumulated counters receive that time. | Unplaced |
| [Copilot](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/copilot/src/parser.rs) | Session-state needs a timestamp. OpenTelemetry falls back to file modification time, then current time if metadata fails. | Unplaced |
| [Gemini](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/gemini/src/parser.rs) | Message time, session time, or file modification time. | Unplaced |
| [Antigravity](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/antigravity/src/parser.rs) | Source time, matched identity time, trajectory time, then file time. | Unplaced |
| [Kimi](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/kimi/src/parser.rs) | New records use milliseconds. Older records use seconds. Missing times can use file modification time. | Unplaced |
| [Qwen](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/qwen/src/parser.rs) | Missing timestamps use file modification time. | Unplaced |
| [OpenClaw](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/openclaw/src/parser.rs) | JSONL can use file time. Its additional SQLite input does not make the mixed tool output prove event dates. | Unplaced |
| [Grok](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/grok/src/parser.rs) | Positive agent time precedes envelope time. Missing time can become epoch. | Unplaced |

The Copilot fallback is an additional source finding. `parse_otel_file` supplies file time to `to_candidate`, which uses it when record time is absent. The helper can return current time. These branches are not proof from a binary run. [Exact fallback](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/copilot/src/parser.rs#L447), [file-time helper](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/copilot/src/parser.rs#L787-L794)

Epoch-only fallbacks may permit a later, narrower policy that separates the affected day. This proposal does not claim every row from an unplaced tool lacks a valid source time. The problem is that the pinned output omits enough provenance to apply one blanket reliable-date rule.

Add a typed catalog policy such as `periodPolicy: dated_daily | unplaced_session`. Keep session-time and project policies separate from it. The first selects a daily report. The second selects one full, unbounded session report as the source total. Do not pass a date filter for unplaced usage. The [current catalog](../../crates/openusage-locallogs/src/tools.rs) can carry these pinned policies without runtime configuration.

OpenCode needs this authority choice: its unbounded session report can include aggregate-only sessions that daily deliberately omits. Never require equality between those two reports or add their totals. [OpenCode loader](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/opencode/src/loader.rs)

## Consent roots and copied usage

The [scan worker](../../apps/desktop/src-tauri/src/local_logs.rs) runs each folder separately. The [workspace mapper](../../apps/desktop/src-tauri/src/workspace/local_logs.rs) saves each result under that folder's source ID. The reducer then sums sources. This loses any duplicate suppression that needs all roots in one scanner call. The storage API accepts several IDs, but rejects two sources for the same tool. Its plural argument does not support a same-tool batch.

Use one stable accounting scope per tool, with a separate membership record for its consented roots. Pass all available, granted roots together when that adapter supports it. Save the result once against the scope. Never save one combined result against every member. Root IDs still own consent, resolved paths, scan evidence, and withdrawal history. Do not infer accounts from that grouping. [SRS duplicate rules](../SRS.md#5d-duplicates)

The [Pi loader](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/pi/src/loader.rs#L82-L159) builds its replay plan and seen-entry set across loaded roots. The [Kilo loader](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/kilo/src/loader.rs#L17-L39) suppresses matching message IDs across databases. These are source findings, not executed AT-65 evidence. Each adapter still needs copied, forked, replayed, and changed-copy fixtures.

Batching is necessary for those cases, but it is not a universal fix. The pinned [Amp loader](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/amp/src/loader.rs#L12-L35) appends entries without duplicate suppression. The [Goose override](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/goose/src/paths.rs#L6-L20) accepts one root. Kilo's fallback identity includes a database path, so a copied record without an embedded ID needs a separate test. A dated candidate is not necessarily an AT-65 candidate. Do not enable an unqualified adapter by substituting equal-total or session-total heuristics.

Add a typed `rootMode: comma_list | single` capability. A bounded Rust list of individually validated consent roots can safely form the comma-list environment value. Keep rejecting literal commas and trimmed paths in each root. Preserve canonical-path checks and remove repeated roots. Use a stable root order because first-wins loaders can select different data from conflicting copies. Keep the private empty home, fixed arguments, and cleared environment. Recheck every member's consent before saving. Do not fall back to default homes for an empty list. [Current boundary](../../crates/openusage-locallogs/src/runner.rs)

Keep the accounting ID stable when a root is added, removed, or withdrawn. Record the roots used by each accepted scan. A missing or withdrawn root reduces coverage. Preserve the accepted snapshot and record lower results, without adding old and new membership snapshots. A maximum observed snapshot cannot recover unique usage missing from every current root. Show that limitation through coverage. Do not add saved missing-root totals to a grouped result, because their events can overlap.

Legacy per-folder summaries lack the event identities needed to remove overlap. Preserve those records as legacy history. Never sum that history with a new grouped result. For an affected period, a reliable combined historical total can remain unavailable. Mark that migration case for review against OU-LOG-011 and AT-65. Do not claim that summing, taking a maximum, or rescanning remaining logs repairs all deleted history. Conflicting pinned source zones also need explicit migration handling. Do not reinterpret old dates in a new zone.

## Minimal data contract

Use these logical records. Names are proposed, not implemented:

```text
LocalPeriod
  accounting_scope_id, tool, period_kind: day | unplaced, local_date: Option<Date>
  total_tokens: exact integer
  input, output, cache_write, cache_read, estimate: Option<exact integer>
  model_details_complete: bool
  scanner_version, normalization_version, total_basis, coverage, observed_at
  models: LocalModelDetail[]

LocalModelDetail
  model: bounded source label
  input, output, cache_write, cache_read, total, estimate: Option<exact integer>

LocalSession
  accounting_scope_id, tool, session_key, total_tokens
  project, first_activity_utc, last_activity_utc: Option<value>
  token categories, reasoning, estimate: Option<exact integer>
```

Null means Not available. A trusted zero stays zero. A full aggregate category or estimate is null if any contributing record lacks that metric. Known model estimates can remain visible individually. Do not expose a partial sum under the field for a complete tool estimate.

Keep total independent of the buckets. Validate known disjoint buckets as a bounded subtotal. Equality applies only when the breakdown is complete. Preserve reasoning's declared relationship to output. Do not add Codex reasoning twice or label an unexplained remainder as reasoning. A model subtotal becomes a model total only when the adapter proves complete attribution.

The raw parser still rejects unknown fields, wrong types, duplicate identities, invalid dates, overflow, and inconsistent report totals. Recognize only pinned optional fields such as `storedCost`, `credits`, and `messageCount`. Validate them before discarding data outside the storage allowlist. Normalize known lossy fields to null through the tool policy. Do not broaden parsing with arbitrary JSON maps. [Parser](../../crates/openusage-locallogs/src/report.rs), [SRS 5C](../SRS.md#5c-local-logs)

Use a parent `local_usage_periods` table and a child `local_usage_models` table. Keep decimal text and checked Rust arithmetic. Require a date for `day` and SQL null for `unplaced`. Separate partial unique indexes enforce one scope/day and one unplaced row per scope. Validate that a scope uses only its declared period policy. Rebuild sessions with nullable times and categories. Rebuild differences to reference the period and allow an optional model for retained legacy differences. A small membership table connects each tool scope to its existing consent sources.

Do not invent an `unknown-model` row to balance totals. Do not store a fake date, epoch, or scan time as usage time. Keep an optional project only when its policy proves an actual project label. Fixed tool labels and Pi's fallback `unknown` are unavailable projects.

## Writes, migration, and range behavior

Compare new and saved totals at the parent period. A lower result keeps the entire accepted parent and child snapshot and records one difference. An equal or higher result replaces the active detail set atomically. A lower model value in that replacement is a new attribution within the accepted total. Do not preserve independent model maxima as the current breakdown. Retain existing difference history. This policy applies OU-LOG-011 to the authoritative period total.

Parse all required outputs before a database transaction. Commit a period and its model details together. A failed optional session view must not make an accepted daily report look complete for sessions. Retain separate capability or outcome state for each view. The [current scan service](../../apps/desktop/src-tauri/src/local_logs.rs) saves daily data before parsing sessions, so its outcome model needs review.

The next migration is schema 13 in the inspected tree. Backfill each old source/day total with checked Rust sums of its model records. Mark its basis `legacy_model_sum`, because the old schema retained no independent upstream total. Preserve saved values, pinned zones, scanner evidence, differences, session IDs, and project privacy. Apply the legacy-overlap limit above before merging source scopes. Do not claim that a migration reconstructs an upstream observation. The [migration runner](../../crates/openusage-storage/src/lib.rs) already supports a Rust data hook inside its transaction.

Use checked `u128` for backfill and cross-row sums. Old individually valid model rows can sum above one row's bound. Do not use SQLite `SUM` over decimal text or convert totals through floating point. Keep rollback and foreign-key checks. Test migrated large values and failed backfill. No rescan or credential access is necessary to migrate saved data.

Keep version-specific readers and copy lists for schemas 10–12. Changing shared column constants would otherwise break old backups. Update [backup validation](../../crates/openusage-storage/src/backup_validation.rs), including its explicit `1..=12` allowlist, and [restore deletion/copy order](../../crates/openusage-storage/src/restore_apply.rs). The [application exporter](../../crates/openusage-storage/src/application_export.rs) can already emit SQL null and decimal text, but it first requires exact schema and content validation.

For presentation, return `dated` results and `unplaced` results as separate typed collections. The range control applies only to dated data. An Unplaced table can show tool, total, and Date: Not available. Untimed sessions need a separate bounded collection with null activity times. Exclude them from selected-range session/project sums, charts, streaks, hourly views, and quota matching. Do not substitute `updated_at` for activity time.

Session activity times are first and last retained usage, not proof that a session ended. Keep that meaning in the API and UI. A source total and its session detail can overlap fully. They remain separate views of usage.

## Affected surfaces and gates

| Surface | Required work |
|---|---|
| [Local-log parser](../../crates/openusage-locallogs/src/report.rs), [sessions](../../crates/openusage-locallogs/src/session.rs), [runner](../../crates/openusage-locallogs/src/runner.rs), [catalog](../../crates/openusage-locallogs/sources/tools.json) | Return parent totals, nullable details, and declared scope. Select one qualified report authority and root mode. Keep process, path, hash, and output limits. |
| [Storage](../../crates/openusage-storage/src/local_logs.rs), [migrations](../../crates/openusage-storage/src/lib.rs) | Add parent totals and model children. Preserve coherent snapshots, legacy differences, exact counts, consent, and pinned zones. |
| [Workspace mapping](../../apps/desktop/src-tauri/src/workspace/local_logs.rs), [desktop totals](../../apps/desktop/src-tauri/src/local_usage.rs) | Sum parents. Propagate nulls. Query dated and unplaced sessions separately. Bound both result sets. |
| [IPC contracts](../../apps/desktop/src/contracts.ts), [usage UI](../../apps/desktop/src/local-usage.tsx) | Update exact-key validators and null handling. Keep decimal strings. Do not call `BigInt` or date formatters on null. |
| [CLI report](../../crates/openusage-cli/src/report.rs), [status](../../crates/openusage-cli/src/status.rs), [schema version](../../crates/openusage-cli/src/main.rs) | Use authoritative totals and explicit unplaced scope. Version changed JSON contracts. A date-filtered report must not relabel unplaced totals as that range. |
| [Share](../../apps/desktop/src-tauri/src/share.rs) | Use dated parent totals for dated images. Preserve the existing rule that share reads no sessions or projects. |
| [Backup validation](../../crates/openusage-storage/src/backup_validation.rs), [restore](../../crates/openusage-storage/src/restore_apply.rs), [exports](../../crates/openusage-storage/src/application_export.rs) | Cover new records, old-schema normalization, nulls, reference checks, and hostile backup inputs. |

Before acceptance, execute pinned fixtures for all sixteen tools. Include missing times, fallback times, cumulative sessions crossing midnight, extra totals across two models, missing prices, unknown shapes, and repeat scans. Test lower totals and changed model attribution. Verify that unplaced totals never enter a dated sum. Add copied roots, conflicting copies, root withdrawal, and deleted-root history under AT-65. A successful single-root fixture does not qualify grouped accounting.

Run the affected security checks and the full renderer/workspace milestone through [the engineering workflow](../engineering-workflow.md). Native gates include local consent, SQLCipher persistence after restart, CLI read-only behavior, backup inspection, restore/checkpoint, application export, and share privacy. Extend the [local-log journey](../../tests/native/local-log-consent.mjs) with one dated and one unplaced tool. Extend the [native verifier](../../tests/native/backup-verifier/tests/current_schema_verifiers.rs) for nullable and independent totals.

Keep all data synthetic in private homes. A passing source review or parser test does not establish native acceptance. The implementation must update architecture, status, slices, and traceability in their [owning documents](../README.md).
