# Codex record identity candidate

November 12, 2026 · L-1f11an · Synthetic Linux x86_64 input.

The [separate candidate](../../patches/ccusage/20.0.48-codex-record-identity.patch) corrects the tested Codex 0.163.0 paginated format.
The [producer contract](2026-11-12-codex-producer-contract.md) owns its captured source evidence.
It preserves all 12,650 tokens in the full captured fork. Copies count once,
and distinct responses keep their usage. The production scanner and Pi oracle
retain their original pins. The application does not use this candidate.

## Accounting boundary

The loader selects the paginated parser before legacy replay. It requires the
tested producer version, ordinals, explicit usage identity, and complete numeric
categories. Once a marker selects this parser, an invalid capture cannot return
to the legacy parser. Mixed paginated and legacy selections refuse.

Accounting uses the emitted thread and response IDs. Output sessions use the
emitted session ID. The loader checks turn and root-turn IDs, record copies,
cumulative totals, and session ownership. It never treats time, paths, or equal
counts as proof of identity. The usage record contributes tokens once. Its
counter checks consistency and contributes no second usage value.

A fork requires its selected parent and the exact declared ordinal and byte
prefix. The loader never opens the declared parent path. A missing prefix
returns unavailable because the loader cannot check lineage and cumulative
usage. This is a conservative boundary under SC-025. It adds no idle-time or
producer-completion requirement. A complete usage prefix is an observation.

Measured zero creates a record. A header without usage creates no observation.
Absent model metadata preserves independent token totals without a model row or
estimate. An unknown model retains the missing-price marker. Missing, null,
fractional, negative, inconsistent, or excessive numeric categories refuse.

## Capture boundary

Linux capture uses directory descriptors and no-follow opens. It checks selected
roots, entries, source metadata, and bytes before it returns events. Changed or
failed reads return unavailable. The bounds cover roots, directory entries,
files, depth, bytes, lines, physical records, and exact counts. The [fixture pin](../../tests/tools/fixtures/codex-records-candidate/pin.json)
owns their exact values and build inputs.

Each invocation validates one capture. Daily and session invocations do not yet
share a capture. A combined report command remains an application integration
gate. Compaction, revert, subagents, compressed paginated input, other producer
versions, and other platforms remain outside this qualified boundary.

## Executed checks

The old Pi oracle fails eight independent captured report expectations.
The corrected candidate passes all eighteen report pairs across thirty-six
commands. The full fork remains 12,650 tokens after the actual missing-usage
and zero turns. Forty-seven legacy comparisons match the unchanged Pi oracle.
They preserve inherited failures and do not establish legacy correctness.

All 163 Codex tests, 73 Pi tests, ten Pi observation regressions, and twenty
focused candidate checks pass.
Adapter Clippy passes with dependencies excluded. Dependency Clippy fails on
an unchanged pricing source warning. The wider unified suite has the same
sixty-one passes and sixteen Pi fixture failures in both source trees.
The record does not claim that suite passes.

Review found an additional orphan-counter error. A nonzero recent count with
a zero cumulative count returned empty success. Its regression fails against the original oracle and passes after correction.
A zero-only counter still means no observation. Private diagnostics preserve
the original result.

The patch recipe reconstructs all pinned source files from the upstream commit
and ordered local patches. No new locked package enters the dependency graph.
The adapter adds direct uses of already locked ring and rustix packages.
Source, prices, compiler, executable, and old pins retain checked fingerprints.

A source mutation reverses the joint byte-limit guard. Its named Rust test
fails. The exact original source returns, and that test passes. All 536 source
fingerprints match again. The read-only release oracle stays unchanged.
Doctor and the selected staged checks pass.

## Build and remaining gates

Cargo runs from the candidate source root with its forty-seven SQLite trim
settings, one shared target, private homes, locked dependencies, and offline
mode. The assignment starts with an absent target. The final release reuses the
shared target after source corrections. Its elapsed time is not a clean
build measurement. The pin records exact commands and fingerprints.

The read-only candidate oracle has a separate physical copy from the build
output. Production packaging, application parsers, legacy history, session-key
migration, a combined capture command, native acceptance, reference-scale
performance, table output, real provider data, and other platforms remain open.
The production pin and enabled tool catalog stay unchanged.
