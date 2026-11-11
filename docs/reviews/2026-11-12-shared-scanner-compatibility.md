# Shared scanner compatibility

November 12, 2026 · L-1f11al · Linux x86_64, private synthetic input.

The bounded Pi release matches production in all 47 enabled-tool report
comparisons. The matrix runs 188 commands. Ten independent expectations fail
in both binaries. This comparison does not qualify a production pin change.
The enabled catalog and production pin stay unchanged.

## Artifacts and scope

The production scanner is `20.0.48`, with the digest in the [Linux source pin](../../crates/openusage-locallogs/sources/ccusage-20.0.48-linux-x86_64.json).
The candidate is `20.0.48-openusage.pi-bounded-release.1`, with the bounded
release digest in the [Pi producer pin](../../tests/tools/fixtures/pi-producer/pin.json). The test checks both hashes before
execution and after the matrix. It also checks its pin and fixture inputs.

The comparison covers Claude Code daily, session, and block reports, plus
Codex daily and session reports. Each command runs twice per binary. An earlier
baseline test also passed five report pairs. The native application result
remains in the separate [Pi qualification record](2026-11-12-pi-native-qualification.md).

Cases cover baseline logs, copied roots, repeated roots, renamed copies,
stored charges, unknown models, empty input, spaced JSON, malformed peers,
and distinct equal-usage Codex sessions. Every run uses an empty private home
and one explicit tool variable. All nineteen private source sets retain their
hashes. No real history, account, credential, or provider session enters the test.

## Independent observations

| Case | Expected observation | Both scanners |
| --- | --- | --- |
| Renamed Codex copy, session report | 4,600 tokens once | 9,200 tokens |
| Two distinct equal-usage Codex sessions, daily report | 9,200 tokens | 4,600 tokens |
| Spaced Claude JSON, three reports | 14,950 tokens | No observations |
| Malformed peer, five reports | Refusal | Success with the valid peer's partial report |

The other 37 comparisons match their independent expectations. Baseline
Claude Code totals are 14,950 and Codex totals are 4,600. Empty reports remain
unavailable observations. A wire zero with no rows is not a measured zero.
The harness distinguishes missing, invalid, zero, and positive measurements.

The [Codex source](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/codex/src/aggregate.rs) scopes its fallback deduplication key by session only
for session reports. Other report kinds omit that scope. Thus, equal timestamps,
models, and counts can merge distinct sessions. The session report instead
uses a path-derived identity, so a renamed copy can count twice. The pinned
producer contract still needs qualification before a correction.

The [Claude loader](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/claude/src/lib.rs) checks the exact bytes `"usage":{`. Spaced JSON lacks that
marker. Compact JSON keeps model and charge cases separate from this format
limit. A malformed peer is a stable invalid input, not a valid captured prefix.
No idle-time or producer-completion rule enters this comparison.

The source review checked three consumer files against their Git blobs at
ccusage commit `8995f3b7fd7110b8f37564f7d0468acc7a7d6b20`. The build does not
change those files. Their digests remain with the private audit receipt.

## Executed checks

The corrected live matrix completed once. All 47 comparisons repeat and match
the reference. Both executables, fixture inputs, and pin inputs stay unchanged.
The matrix records completion separately from source preservation. A partial
run cannot claim that every source check completed.

Seven focused harness tests passed. The full Python suite passed with 69 tests
and 39 skips. A candidate-only extra field makes the comparison fail while
token counts still match. The diagnostic restores its in-memory wrapper and
checks unchanged source hashes. The original partial probe remains separate.
It stopped in the malformed-peer builder before the correction.

Doctor and the selected verification plan passed. New application, Rust,
renderer, and native code did not change in this slice. The earlier native
results retain their original artifact scope. Hosted CI and other platforms
remain separate gates.

## Next gate

Qualify captured Codex event identity against the pinned producer. Check copies,
distinct sessions with equal usage, and declared lineage without using time or
equal counts as identity. Build a separate correction only after that contract
is clear. Keep the production pin and enabled catalog unchanged.
