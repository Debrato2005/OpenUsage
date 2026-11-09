# ZCode qualification

Date: November 10, 2026. L-1f11b uses private synthetic SQLite files and the pinned Linux scanner. ZCode stays disabled because conflict and incomplete-read checks fail. The production scanner pin and the unpublished Amp candidate are unchanged.

## Source comparison

The pinned scanner reads completed `model_usage` rows and deduplicates stored IDs across roots. Conflicting IDs keep the first record. Read errors and unsupported schemas can return partial results with a successful exit. Missing optional columns use zeros or a computed fallback. [Pinned loader](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/zcode/src/loader.rs)

ZCode's own source at `a9fe401eede6e6e5e01be9048679c206fe6f02d9` updates usage by primary ID. Its usage repository also prunes old rows after 30 days. This supports OpenUsage's separate rule to retain accepted history after source deletion. [Usage repository](https://github.com/zai-org/ZCode/blob/a9fe401eede6e6e5e01be9048679c206fe6f02d9/apps/zcode-cli/packages/adapters/src/storage/session-store/repositories/usage.ts)

The fork transaction copies conversation records, parts, entries, goals, and command state. It does not copy usage-ledger rows. Compaction records its model call as a separate usage fact. The fixtures model those storage effects, without executing the producer or a model. They do not claim that a full ZCode fork ran. [Fork transaction](https://github.com/zai-org/ZCode/blob/a9fe401eede6e6e5e01be9048679c206fe6f02d9/apps/zcode-cli/packages/adapters/src/storage/session-store/sqlite-session-store.ts), [compaction call](https://github.com/zai-org/ZCode/blob/a9fe401eede6e6e5e01be9048679c206fe6f02d9/apps/zcode-cli/packages/core/src/runtime/methods/compact-active.ts)

## Recorded results

Nineteen cases produce 38 daily/session reports per comparison. Each uses an empty private home, fixed offline arguments, and explicit roots. A populated Pi root stays outside the selection. Source hashes remain unchanged.

| Synthetic case | Expected tokens | Scanner tokens | Result |
|---|---:|---:|---|
| Baseline, exact copy, or repeated row update | 130 | 130 | Matched |
| Copy with 17 new tokens | 147 | 147 | Matched |
| Forked transcript without a new ledger fact | 130 | 130 | Matched |
| Fork with 17 new tokens | 147 | 147 | Matched |
| Compaction with a 49-token model call | 179 | 179 | Matched |
| Conflicting ID, both root orders | Unavailable | 130 or 160 | Failed. Order changes the result |
| Unknown model or custom provider | 130 | 130 | Count matched. Estimate stays unavailable |
| Legacy schema without optional columns | Full total unproven | 120 | Documented fallback only |
| Missing timestamp | 130 with no known date | 0, no rows | Failed. Record omitted |
| Missing or empty project | 130 | 130 | Count matched. Project needs the fix below |
| Invalid or unsupported peer database | Unavailable | 130 | Failed. Incomplete read appears successful |
| Noncompleted row | Excluded by scanner contract | 0, no rows | Scope limitation |
| Empty database | 0 | 0, no rows | Matched. No measurement in OpenUsage |

## OpenUsage correction

The scanner substitutes `ZCode` when the joined project directory is missing or empty. OpenUsage previously accepted that label as a project. The new regression first failed with `Some("ZCode")` where no project was available. The catalog now selects a ZCode project policy, and Rust removes that exact fallback. Known paths and reported activity times stay available. A path such as `/synthetic/ZCode` remains a project. [Pinned parser](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/zcode/src/parser.rs)

Token details and estimates remain unavailable under SC-022. A reported total remainder never becomes reasoning, even though the source database can have a reasoning column. No account identity is inferred.

## Verification and remaining gates

After the correction, 28 crate tests, nine period tests, the enabled-tool pinned-binary regression, and the ZCode comparison passed. The full Python/Rust plan, renderer milestone, formatting, Clippy, and rebuilt native consent and CLI-status journeys also passed. The renderer check covers both engines and the shared catalog change. No automatic retry, timeout, or interruption occurred.

Conflicting identities, incomplete reads, and untimed records need a scanner candidate before enablement. The current aggregate report cannot recover that missing evidence. Legacy fallback totals also need a capability decision based on source evidence. Hosted CI, real logs, Windows, macOS, concurrent WAL reads, and producer execution have not run for this slice.

Fixtures: [generator and comparison](../../tests/tools/test_ccusage_zcode.py), [recorded reports](../../crates/openusage-locallogs/tests/fixtures/remaining-tools/zcode-qualification.json), and [Rust regression](../../crates/openusage-locallogs/tests/agent_periods.rs). Primary-source receipts and test results remain private under `.recovery/audit/l1f11b-*`.
