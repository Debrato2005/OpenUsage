# OpenClaw transcript qualification

L-1f11af, November 12, 2026. The pinned scanner loses distinct OpenClaw events
when their session, time, model, and counts match. OpenClaw stays disabled.

## Executed reports

The [fixture](../../tests/tools/test_ccusage_openclaw.py) runs the production
`ccusage` 20.0.48 binary on private synthetic Linux input.
The [qualification record](../../tests/tools/fixtures/openclaw/qualification.json)
covers 52 cases and 208 repeated scanner runs.
Fourteen cases match both views. Sixteen session outcomes match.
Bubblewrap supplies private homes and read-only selected roots.
Repeated reports and source hashes match.

| Scenario | Required tokens | Pinned result |
| --- | ---: | ---: |
| Single JSONL or SQLite event | 135 | 135 |
| Exact copy or replay | 135 | 135 |
| Distinct event IDs with equal times and counts | 270 | 135 |
| Distinct calls at different times | 270 | 270 |
| Renamed JSONL session copy | 135 | 270 |
| Inherited fork and 17 new tokens | 152 | 287 |
| Conflicting SQLite rows with one event ID | Unavailable | 135 or 152 by row order |
| Distinct agents with equal SQLite values | 270 | 135 |
| JSONL and its migrated SQLite copy | 135 | 135 |
| Distinct JSONL and SQLite IDs with equal values | 270 | 135 |
| Same-ID conflict between JSONL and SQLite | Unavailable | 287 |
| Explicit unavailable usage in either format | Unavailable | 135 |
| Reliable total with unavailable cache split | 135 with unavailable categories | 135 with numeric categories |
| Synthetic measured zero | One zero row | No rows |
| Invalid JSONL, SQLite, or selected peer | Unavailable | 135 from valid input |

The synthetic lineage and call origins supply the required totals.
No OpenClaw writer, fork, or migration executed.
A reset archive and a compaction notice preserve the captured 135 tokens.
The tests do not establish complete compaction accounting.

Missing time receives file time or the SQLite row's creation time.
Missing models yield numeric zero cost and an unpriced-model marker.
A total-only record puts 135 tokens in output.
A 145-token independent total with 135 visible tokens still receives a numeric estimate.
Negative, fractional, text, and contradictory counts produce successful reports.
Missing measurements must stay unavailable under
[SC-022](../srs-clarifications.md#sc-022-incomplete-metrics-from-the-other-local-tools).
The scanner also ignores explicit usage and cache availability flags.

A separate WAL test runs both report views while an update transaction stays open.
Both views read the committed 135 tokens and omit the uncommitted replacement.
Source hashes stay unchanged. This test does not qualify arbitrary concurrent writers.
The SQLite fixtures use a minimal compatibility table, not the complete producer schema.
These checks cover captured records under
[SC-025](../srs-clarifications.md#sc-025-captured-local-records-and-producer-completion).

## Source contract and next gate

The review pins OpenClaw v2026.11.3 at
[`847611dc`](https://github.com/openclaw/openclaw/tree/847611dcaa3bf0e2783335f961dd1fd0580e4d9f).
All forty-six selected files match their digests and official Git blobs.
The review uses source only. Current native persistence uses per-agent SQLite.
The JSONL fixtures describe compatibility inputs, not an executed historical writer.

Fresh [message appends](https://github.com/openclaw/openclaw/blob/847611dcaa3bf0e2783335f961dd1fd0580e4d9f/src/config/sessions/session-accessor.sqlite-transcript-message-append.ts#L209)
use supplied IDs or generate random UUIDs.
The [identity table](https://github.com/openclaw/openclaw/blob/847611dcaa3bf0e2783335f961dd1fd0580e4d9f/src/state/openclaw-agent-schema.sql#L567)
scopes IDs to a session within an agent database.
Optional provider response and runtime turn IDs do not establish a universal usage identity.

The [parent fork](https://github.com/openclaw/openclaw/blob/847611dcaa3bf0e2783335f961dd1fd0580e4d9f/src/config/sessions/session-accessor.sqlite-parent-fork.ts#L328)
copies branch entries into a new session and retains usage and event IDs.
Its header carries `parentSession` ancestry.
Session-plus-event identity therefore counts inherited usage again.
An event ID alone also cannot reconcile unrelated stores safely.

The [replay writer](https://github.com/openclaw/openclaw/blob/847611dcaa3bf0e2783335f961dd1fd0580e4d9f/src/config/sessions/session-accessor.sqlite-transcript-message-append.ts#L158)
can relocate idempotency ownership to a replayed entry.
The [importer](https://github.com/openclaw/openclaw/blob/847611dcaa3bf0e2783335f961dd1fd0580e4d9f/src/config/sessions/session-accessor.sqlite-import.ts#L213)
compares serialized source events. Equal counts and times do not prove a migration copy.
The store also supports payload rewrites under retained sequence and creation time.

The [usage contract](https://github.com/openclaw/openclaw/blob/847611dcaa3bf0e2783335f961dd1fd0580e4d9f/packages/llm-core/src/types.ts#L309)
keeps completeness and cache telemetry separate from numeric fields.
Unavailable or provisional usage must not become a complete total.
The [Responses mapper](https://github.com/openclaw/openclaw/blob/847611dcaa3bf0e2783335f961dd1fd0580e4d9f/packages/ai/src/providers/openai-responses-terminal-usage.ts#L35)
normalizes cache overlap and defaults missing counters to zero.
Absence of an availability flag does not prove that every field was measured.

The current [payload writer](https://github.com/openclaw/openclaw/blob/847611dcaa3bf0e2783335f961dd1fd0580e4d9f/src/config/sessions/transcript-payload.ts#L238)
can store Zstandard bytes in `event_zstd` with `event_json` set to null.
It records exact expanded length and navigation metadata.
The pinned reader selects only text `event_json` and skips null values.
Source inspection therefore exposes a compressed-row coverage gap.
No compressed producer record executed in this slice.
Canonical cold archives also need their own coverage contract.

The pinned [JSONL parser](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/openclaw/src/parser.rs)
discards the event ID from loaded JSONL usage.
The [SQLite reader](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/openclaw/src/sqlite.rs)
keeps event IDs, but the
[loader](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/openclaw/src/loader.rs)
also replaces SQLite entries by a migration fingerprint.
Distinct IDs can therefore collapse without any JSONL input.
It suppresses later same-ID rows without checking conflicting usage.
Read, schema, and record failures can leave a partial successful report.

A candidate needs bounded capture of every selected source and its journal files.
It must preserve distinct events and reconcile proven copied ancestry.
Unknown formats, unresolved identities, and partial captures must remain unavailable.
Producer runtime, compression, migration, scale, and application gates remain open.
