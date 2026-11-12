# Codex combined capture candidate

November 12, 2026 · L-1f11ao · Synthetic Linux x86_64 input.

The [separate candidate](../../patches/ccusage/20.0.48-codex-snapshot.patch) adds one command for complete Codex daily and session
reports. Both sections derive from one validated event vector. The command
reuses the strict paginated parser from the [record candidate](2026-11-12-codex-record-identity-candidate.md). Production
scanning and the enabled catalog stay unchanged.

## Command and capture

The command requires JSON and offline mode. It refuses date filters, partial
sections, a session filter, JSON queries, and hidden cost fields. The envelope
contains schemaVersion, tool, daily, and session. Each report keeps the existing
Codex-specific JSON shape.

The snapshot path captures selected input directly. It never invokes legacy
replay or runs two report scans. The ordinary daily and session commands keep
their earlier parser selection. The snapshot refuses legacy and mixed formats.
Its numeric, identity, lineage, and capture limits remain those of the strict
record candidate.

An existing empty selected directory produces two empty reports. A missing,
unreadable, linked, or non-directory selected root refuses. An invalid sessions
or archive peer cannot disappear from an otherwise successful report. An absent
default Codex home also refuses. Selection runs again after capture.

This command observes one bounded capture. It does not establish a filesystem
transaction, producer completion, or consistency across separate invocations.
Complete usage prefixes remain observations under SC-025. Measured zero remains
distinct from no usage. Unavailable model attribution and estimates stay unavailable.

## Executed boundary

Nine cases from the producer capture run twice.
Copy, time, and conflict mutations stay separate from unaltered producer output. Both sections match independent totals and
their separate command controls. The full fork keeps 12,650 tokens. Conflicts
and missing selected parents refuse without JSON output. Copied and distinct
responses retain the earlier identity rules.

Eight Linux x86_64 controls change a private source through append, rewrite,
replacement, or deletion. During capture, each change causes refusal with empty
stdout. After validation, both report totals remain 17. A later invocation
reads 29 tokens or no observations after deletion.

The test pauses the private child at system-call boundaries with ptrace. It
changes no production scanner code. Each successful capture opens the source
twice: once to read and once to check the captured bytes. Both reports then
use the same event vector. These controls establish the tested command boundary.
They do not establish another platform or a real provider history.

Thirteen focused snapshot checks pass. Twenty prior Codex checks, nine Pi
snapshot checks, ten Pi observation checks, and forty-seven legacy comparisons
also pass. The source suites pass 163 Codex, 73 Pi, and 99 parser tests.
Scoped Clippy, formatting, the release build, and doctor pass. The wider unified
suite was not rerun. Its earlier sixteen Pi fixture failures remain open.

The first reconstruction comparison used different sort orders. A diagnostic
comparison found identical paths and source hashes. The corrected comparison
matches all 536 source records after nine ordered patches. No dependency package
or production pin changes.

A source mutation reverses the JSON-query guard. The named parser test fails.
After exact restoration, it passes. All source fingerprints match the pin.
Doctor and the selected staged checks pass.

## Build and remaining gates

The source-root build uses pinned Rust, offline prices, locked dependencies,
private homes, forty-seven SQLite trim settings, and one shared target.
The assignment starts with an absent target. The final build reuses compiled
outputs. Its elapsed time is not a clean build measurement. The [fixture pin](../../tests/tools/fixtures/codex-snapshot-candidate/pin.json)
owns exact commands, source hashes, build inputs, and the read-only oracle.

Application envelope parsing, grouped scan selection, consent integration,
history preservation, native acceptance, packaging, reference-scale performance,
and other platforms remain open. The next slice implements only the Rust report
consumer. The current application continues to use its production scanner.
