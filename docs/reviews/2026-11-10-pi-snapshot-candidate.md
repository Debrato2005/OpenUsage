# Pi report consistency candidate

L-1f11g, November 10, 2026. The local candidate now builds Pi daily and session
reports from one captured set of entries. OpenUsage requires that envelope for
its disabled Pi path. Only Claude Code and Codex are enabled. The production
scanner pin and unpublished Amp patch are unchanged.

## Source and controlled observations

The [pinned Pi producer](https://github.com/earendil-works/pi/blob/0d1793c2bc7139d1df4547fa3c0aeacb233e0534/packages/coding-agent/src/core/session-manager.ts)
appends, rewrites, and creates fork files. The preceding
[filesystem review](2026-11-10-pi-filesystem-candidate.md) records those source
operations and the bounded reader. OpenUsage previously launched daily and
session commands separately. The pinned ccusage unified sections path also
loads each view separately. One process alone would not resolve that gap.

Seven synthetic cases used the preceding filesystem candidate and controlled
writes between commands. Stable input returned 162 in both views. An append
returned 162 then 179; a rewrite returned 162 then 40; deletion returned 162
then no sessions. Malformed replacement failed the second report. Changed dates
and replacement session IDs each kept 162 in both views. Thus equal totals
cannot establish common input. Scanner calls left each controlled source
unchanged. The private observation record retains all fourteen report outcomes.

## Correction

`pi snapshot --json --offline` loads validated entries once. It builds both
views from that in-memory vector, with no second source read or raw-log copy.
The version 1 envelope contains `tool`, `daily`, and `session`. Date filters,
`--last`, output transformations, and online mode are rejected. Ordinary Pi
report commands retain their behavior.

OpenUsage checks the envelope and passes the original nested JSON to the strict
report parsers. Duplicate keys remain visible to validation. Both reports must
be empty or both must have equal independent totals. Missing fields, unknown
versions, inconsistent reports, and command failures reject the scan. There is
no fallback to two commands. The existing deadline, output bound, process lock,
binary digest, consent, and root checks still apply.

This rule covers a newly collected scan. Saved history can retain records from
different accepted scans under OU-LOG-011. Session totals cannot reconstruct
saved day totals. A failed scan returns no replacement data. Missing token
categories and estimates remain unavailable under SC-022.

## Executed checks

All 67 Pi adapter tests and 96 CLI parser tests passed. New tests change source
files after capture through append, rewrite, replacement, and deletion. Both
views retain the captured entries; the next load sees the change. Independent
totals and empty input have separate cases. Formatting and selected Clippy pass.

Nine candidate Python tests passed. Five recorded snapshot cases cover baseline,
total-only, unattributed, empty, and malformed inputs. The previous forty
identity, nineteen format, and sixteen filesystem cases also use the snapshot
command and match their expected reports or unavailable outcomes. Checks cover
source preservation, private homes, other fifteen formats, enabled Claude/Codex,
and existing JSON/table views. All 32 OpenUsage local-log unit tests passed,
including one-process collection, refusal after changed consent, invalid
envelopes without fallback, output bounds, and nullable metrics.

Full OpenUsage Python/Rust verification and rebuilt native consent passed. Existing grouped storage tests retain saved history after rejected or lower scans. The native check used the production scanner in a private synthetic Linux home.

The first offline parser test could not start because locked test dependencies
were absent. Approved downloads supplied console 0.16.6, similar 2.7.0, and
insta 1.49.0, with Cargo retries disabled. The intended regression then failed
on the unsupported snapshot command. The runner regression first observed two
processes. Both passed after the correction. Clippy rejected placement of the
new report function after a test module; moving it resolved the error. Read-only
navigation used missing upstream paths before corrected discovery. No automatic
retry, timeout, or interruption occurred.

Two OpenUsage plans stopped at their whitespace check. Patch context lines
contain a required space before upstream JSON tabs. A scoped Git attribute now
allows that sequence in ccusage patches without changing their source context.
The first correction was overridden by the existing nested attributes file.
The final rule is in that owning file. Other whitespace checks remain active.
Both failed plans ran no tests.

## Build and remaining gates

Source: ccusage `8995f3b7fd7110b8f37564f7d0468acc7a7d6b20`, with the usage,
identity, format, filesystem, and snapshot patches in that order. The build
uses Rust 1.99.1 and verified offline pricing. No lockfile changes in this
layer. OpenUsage enables serde_json's already locked `raw_value` feature.

Patch: [20.0.48-pi-snapshot.patch](../../patches/ccusage/20.0.48-pi-snapshot.patch),
SHA-256 `99f53b938ede8b7d41e597ef0d31014ce14a9841bba9521a04ab22f1f547efbd`.
Version: `20.0.48-openusage.pi-snapshot-candidate.1`. Binary SHA-256:
`353d81e9b9c7aa6dbb3986395e990f9357cf9732359f3487e530debfa141bfbb`.
Layered apply and reverse checks passed. Private metadata retains build inputs
and check references. No extra binary copy is retained.

The common entry vector does not make filesystem reads atomic. Continuous
producer execution, zero-usage observations, absent times, and release-scale
performance still need qualification. Linux/openat2 remains the candidate's
supported read boundary; other platforms remain unqualified. L-1f11 stays open.

Renderer checks, other native journeys, pinned upstream Nix/Rust 1.97.1, the full
upstream workspace, hosted CI, real logs, and producer execution did not run.
Native consent uses the production scanner and cannot qualify the candidate.
