# OpenCode observation candidate

L-1f11q, November 11, 2026. This scanner candidate addresses three
lost observations from the [qualification record](2026-11-11-opencode-qualification.md).
OpenCode stays disabled while the remaining capture checks are open.

## Observation rules

A missing model or provider label can keep a valid explicit unsigned total.
The new path requires that total to cover the known token categories. Checked
subtraction keeps its remainder separate from output. A reasoning value cannot
increase the independent total. An absent model stays absent.

An explicit unsigned zero keeps a zero observation. An absent or invalid total
does not create a new zero row. Empty aggregate placeholders retain their
previous behavior. The existing fallback for fully attributed records remains
part of the raw scanner contract. OpenUsage discards these source token buckets
and estimates for OpenCode under SC-022.

OpenCode remains in the unplaced scope. The scanner's Unix epoch fallback for
an absent timestamp supplies no activity metadata to OpenUsage. Conflicting
IDs, incomplete reads, copied records without IDs, partial totals, and unresolved
fork boundaries still need a strict capture correction.

## Build and checks

The patch applies to ccusage `8995f3b7fd7110b8f37564f7d0468acc7a7d6b20`.
The build uses the existing locked dependencies and verified offline pricing.
Set `CCUSAGE_VERSION=20.0.48-openusage.opencode-observations-candidate.1`
for the release CLI build in the shared Cargo target.

The [comparison](../../tests/tools/test_ccusage_opencode_candidate.py) uses an
explicit candidate path and digest. It compares every prior qualification
case, the sixteen format baselines, and the enabled Claude Code and Codex
reports. The three changed cases have independent total and row expectations.
The remaining source failures stay explicit in the recorded matrix.

All 83 adapter tests, selected adapter Clippy, and the five Python candidate
tests passed. The latter compare the complete prior reports outside the three
corrected cases. The corrected matrix has fifteen matching observations and
eleven remaining qualification failures. No source capture defect is waived.

Patch [20.0.48-opencode-observations.patch](../../patches/ccusage/20.0.48-opencode-observations.patch)
has SHA-256 `83d30c081c57ff270a1ee108296e1071fb49c9671d2070f570d20bc09234f61c`.
The release binary has SHA-256
`9dba22b237879c847b01a7ed242d3f1113fce955be1c8591c85cf2bc99553d95`.
The lockfile is unchanged from the pinned source. The candidate remains
unqualified for incomplete or conflicting source inputs.
