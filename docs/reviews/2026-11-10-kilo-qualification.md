# Kilo qualification

Date: November 10, 2026. L-1f11a uses the pinned Linux scanner and private synthetic SQLite files. Kilo fails AT-65 and stays disabled. No production pin changed.

## Source comparison

The scanner pin is `ccusage` 20.0.48, source commit `8995f3b7fd7110b8f37564f7d0468acc7a7d6b20`. Its Kilo loader reads the legacy `message` table. A payload ID permits deduplication across roots. Without that ID, the key contains the database path and row ID. Duplicate IDs keep the first record without checking for conflicts. Database and row errors can return partial data without a failing exit. [Pinned loader](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/kilo/src/loader.rs)

Kilo's own source at `b6a49d01c10d7e26c42a8a14b3441b7a1856c04c` stores the ID in a column and removes it from the JSON payload. It updates repeated messages by primary ID. The earlier minimal fixture included a payload ID, so its passing copy check did not cover this layout. [Kilo projector](https://github.com/Kilo-Org/kilocode/blob/b6a49d01c10d7e26c42a8a14b3441b7a1856c04c/packages/core/src/session/projector.ts)

Kilo's legacy fork gives copied messages new IDs and keeps their token counts. It clears their recorded cost. Its compaction creates a separate assistant summary and can replay a user message. The fixtures reproduce these storage effects with invented test records, without executing Kilo or calling a model. [Fork source](https://github.com/Kilo-Org/kilocode/blob/b6a49d01c10d7e26c42a8a14b3441b7a1856c04c/packages/opencode/src/session/session.ts), [compaction source](https://github.com/Kilo-Org/kilocode/blob/b6a49d01c10d7e26c42a8a14b3441b7a1856c04c/packages/opencode/src/session/compaction.ts)

This source snapshot is not an installed or tested Kilo release. It also defines a newer `session_message` table. That format remains unqualified; these tests cover the legacy table only. [Kilo schema](https://github.com/Kilo-Org/kilocode/blob/b6a49d01c10d7e26c42a8a14b3441b7a1856c04c/packages/core/src/session/sql.ts)

## Recorded results

All 16 cases record both daily and session JSON. Each report uses an empty private home, fixed offline arguments, and selected Kilo roots. A populated Pi root remains outside the selection. Source hashes stay unchanged.

| Synthetic case | Independent expected tokens | Scanner tokens | Result |
|---|---:|---:|---|
| One legacy message | 173 | 173 | Matched |
| Copied legacy database | 173 | 346 | Failed |
| Copy with an embedded payload ID | 173 | 173 | Matched, narrower layout |
| Forked legacy history | 173 | 346 | Failed |
| Fork plus 17 new tokens | 190 | 363 | Failed |
| Replayed update of the same row | 173 | 173 | Matched |
| Original plus a 49-token summary and user replay | 222 | 222 | Matched |
| Conflicting payloads with one ID, both root orders | Unavailable | 173 or 190 | Failed; order changes the result |
| Unknown model, with and without a stored charge | 173 | 173 | Counts match; see price gap below |
| Missing timestamp | 173, with no known date | 0, no rows | Failed; record omitted |
| Total present, token details absent | 173 | 173 | Count matches; output detail is invented upstream |
| Malformed row beside a valid row | Unavailable | 173 | Failed; incomplete scan appears successful |
| Invalid database beside a valid database | Unavailable | 173 | Failed; incomplete scan appears successful |
| Empty database | 0 | 0, no rows | Matched; OpenUsage has no measurement |

An unknown model with a positive stored charge produces zero calculated cost without a missing-price flag. Without the charge, the scanner flags missing pricing. A total-only record becomes output tokens upstream. OpenUsage already leaves both estimates and token details unavailable under SC-022. The new Rust regression checks that boundary against the captured reports. Missing session times and projects also remain unavailable. [Pinned parser](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/kilo/src/parser.rs)

## Resolution and remaining gates

The enabled-tool gate contains these count defects. Report parsing cannot recover event identities, fork ancestry, or omitted rows from aggregate JSON. No total, time, price, or account identity is inferred to hide them. The tests preserve the bad scanner outputs as regression evidence; passing comparisons do not mean AT-65 passed.

A future scanner candidate must use stable stored IDs, reject conflicting or incomplete reads, and prove fork handling from source metadata. It must also cover untimed records and the newer table format. Equal counts, zero cost, matching times, and session titles cannot establish duplicate identity. The production pin must stay unchanged until the required comparison and approval gates pass.

The focused scanner comparison and eight Rust period tests passed. The selected full Python and Rust plan, formatting, Clippy, and rebuilt native consent journey passed. No automatic retry, timeout, or interruption occurred. Renderer tests, other native journeys, hosted CI, real logs, Windows, macOS, and Kilo enablement did not run.

Fixtures: [generator and comparison](../../tests/tools/test_ccusage_kilo.py), [recorded reports](../../crates/openusage-locallogs/tests/fixtures/remaining-tools/kilo-qualification.json), and [Rust regression](../../crates/openusage-locallogs/tests/agent_periods.rs). Private source receipts and command results are in `.recovery/audit/l1f11a-*`.
