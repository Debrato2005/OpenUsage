# Codex snapshot report consumer

November 12, 2026 · L-1f11ap · Synthetic Linux x86_64 evidence.

The [Rust parser](../../crates/openusage-locallogs/src/codex_snapshot.rs) now reads the separate candidate envelope into existing grouped
period and session records. It preserves independent exact totals, reported
categories, nullable estimates, model attribution, session identity, and last
activity. This change adds no scanner activation or application dispatch.

## Report boundary

Eleven recorded envelopes come from the pinned candidate with synthetic inputs
and the earlier synthetic producer capture. The generator checks the executable
pin, independent totals, and unchanged source fingerprints. The fixture contains
reports and fingerprints, with no prompt or message body. Two Python checks bind
the fixture to the scanner, producer capture, and generator. A synthetic preview
field makes the privacy guard fail. Exact restoration passes.

The parser returns existing grouped types. It shares the strict envelope,
required-field helper, and Codex model type through visibility-only changes.
A bounded visitor rejects duplicate model keys. The [crate contract](../../crates/openusage-locallogs/README.md#codex-candidate-snapshot) owns field
rules, numeric bounds, unavailable values, ordering, and estimate reconciliation.

A missing model keeps the total and known categories, with no invented model
or estimate. A partial model map keeps its reported subsets, with unavailable
model totals. The shared saved-data reader already hides incomplete model
aggregates. A reported zero has records. Empty input has none. Sessions keep
their emitted keys and reported last activity, with no invented start or project.
Session totals never enter daily totals.

## Estimate precision

A controlled source partition exposed a real scanner boundary. Five distinct
threads report 182, 265, 46, 205, and 104 input tokens. Both views total 802.
Their grand cost literals differ at the final floating digit. Standard Rust
parsing and the existing conversion give 1,003 and 1,002 microdollars.
An exact equality check rejects this valid report.

The first fixture helper also parsed and serialized JSON values before the
parser received them. That could erase the final decimal. Tests now retain the
original section bytes. The consumer retains each cost token until standard
Rust float parsing, immediately before the unchanged microdollar conversion.
Legacy JSON features and parsers stay unchanged.

The observed partition fails before correction and passes afterward. Each row
keeps its own estimate. The consumer returns no combined estimate across both
reports. Larger price conflicts and null-versus-present disagreement still fail.
The crate contract states the exact integer bounds after conversion.

## Executed checks

The first 17-token unattributed regression failed before implementation. The
known-model and duplicate-key regressions also failed before their corrections.
Fifteen public parser tests cover all eleven recorded cases and malformed
variants. The full fork keeps 12,650 tokens across three sessions.

The default crate passes 65 tests. The final Pi qualification feature run passes
66, including the actual pinned production scanner with private synthetic logs.
Clippy and workspace formatting pass. Five earlier guard mutations fail on the pre-precision source
and restore exactly. Their receipts retain that source hash. A parent mutation widens the grand-estimate guard. Its
named regression fails, then passes after exact source restoration. Doctor and
the selected staged project checks pass.

Application scanning, persistence integration, native acceptance, reference-scale
performance, other platforms, and production pin replacement remain open.
The next slice exercises this parser through a native qualification feature.
No real provider account, credential home, or history was inspected.
