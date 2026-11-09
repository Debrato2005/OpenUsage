# Pi zero usage and absent times

L-1f11h, November 10, 2026. The local Pi candidate preserves reported zero
observations and rejects usage without a reliable independent total. Missing
measurements remain unavailable. Pi stays disabled. The production scanner pin
and unpublished Amp patch are unchanged.

## Producer contract

The pinned producer is Pi `0d1793c2bc7139d1df4547fa3c0aeacb233e0534`.
Its [Usage type](https://github.com/earendil-works/pi/blob/0d1793c2bc7139d1df4547fa3c0aeacb233e0534/packages/ai/src/types.ts#L467)
requires `totalTokens`. It defines reasoning as an optional subset of output.
A category subtotal cannot establish an absent independent total.
The [session contract](https://github.com/earendil-works/pi/blob/0d1793c2bc7139d1df4547fa3c0aeacb233e0534/packages/coding-agent/src/core/session-manager.ts#L43)
requires timestamps on the header and every entry. Assistant and standalone
usage records require usage. Compaction and branch summaries allow absent usage.

The producer's `appendMessage` assigns an entry timestamp when it appends the
record. The nested assistant timestamp describes request start. These values
have different meanings. The candidate uses the reported entry timestamp.
It does not substitute a header, nested request time, file name, or modification
time. Missing or invalid required timestamps reject the scan. This follows
Pi's dated report policy and needs no SRS change. Other twelve tools stay unplaced.

A reported zero is source evidence, not proof of billing or successful execution.
The tests include zero records with `stop`, `error`, and `aborted` reasons.
They make no provider-runtime or real-account claim. Reasoning, category values,
and estimates remain unavailable in OpenUsage under SC-022.

## Comparison and correction

Seventy private synthetic cases compare the pinned scanner, previous snapshot
candidate, and independent expectations. The first comparison ran 210 commands.
The pinned scanner omits zero records and missing-time observations. It reports
155 when categories sum to 155 but the independent total is absent.
The previous snapshot candidate disagrees with 27 expectations. It drops zero
rows, accepts absent standalone usage, and derives totals from partial categories.

The sixth local Pi patch removes the zero-record filter. It requires an explicit
unsigned independent total in each present usage object. That total must cover
the known categories. An absent standalone usage object rejects the scan.
Optional summary usage can stay absent. Such context records add no measurement
and cannot create a zero observation.

The same captured entries still produce both reports. Copies and forks retain
one zero observation, while distinct source calls remain separate. Zero activity
on an earlier day survives beside later positive usage. Partial categories with
a valid total remain supported. The wire format has numeric placeholders, but
OpenUsage keeps these categories and estimates unavailable.

The strict OpenUsage parser already accepts zero rows and nullable report times.
New tests cover both. Empty reports have no rows. A zero report paired with an
empty report fails even though their numeric totals match. The storage regression
saves zero and absent times, reopens SQLCipher, accepts a higher reading, then
preserves it after zero, empty, and rejected scans. Legacy history remains separate
under SC-024. No storage schema, production Rust behavior, or UI changed.

## Executed evidence

All 70 Pi adapter tests and ten candidate Python tests passed. The new matrix
records 30 supported and 40 unavailable cases. The inherited tests cover the
prior identity, format, filesystem, and snapshot cases, other fifteen tools,
enabled Claude/Codex, private homes, and unchanged source hashes.
Five snapshot parser tests and the focused storage regression passed.
Layered patch checks, selected adapter Clippy, and formatting passed.

Full Python/Rust verification passed, including workspace tests and Clippy.
The rebuilt desktop passed native local-log consent with the production scanner
in a private synthetic Linux home. The test preserved consent isolation and
saved usage after rejected scans. Renderer checks were not applicable to this
change. The native check does not enable or qualify Pi.

Two intended regressions failed before the correction and passed afterward.
A broad upstream Clippy command stopped at an unchanged core pricing warning
under Rust 1.99.1. The selected adapter check with `--no-deps` passed.
Local research search worked, but its reader rejected raw TypeScript content.
Direct pinned-source downloads supplied the files. Initial navigation used
missing paths, and a private helper failed while inserting verification settings.
Those setup errors were corrected before the affected checks.
No automatic retry, timeout, or interruption occurred.

## Build and remaining gates

Source: ccusage `8995f3b7fd7110b8f37564f7d0468acc7a7d6b20`, followed by the usage,
identity, format, filesystem, snapshot, and observation patches in that order.
The build uses Rust 1.99.1, the shared target, and verified offline pricing.
Lockfiles and package versions match the preceding candidate.

Patch: [20.0.48-pi-observations.patch](../../patches/ccusage/20.0.48-pi-observations.patch),
SHA-256 `3d461fefc8bcdeaeabcd45ebbf9dd22a758276ac7cd0f2888ca2e3da8d3be199`.
Version: `20.0.48-openusage.pi-observations-candidate.1`.
Binary SHA-256: `3b95f48181b0bca981924c4cdea6fe6a98d27b90b785e964899bf9962251f844`.
Private metadata records source hashes, commands, failures, and build provenance.
No extra executable copy is retained.

Producer execution, concurrent producer writes, release-scale performance, and
other platforms remain unqualified. Native consent uses the production scanner
and cannot qualify this candidate. Renderer checks, other native journeys, the
full upstream workspace, pinned upstream Nix/Rust 1.97.1, hosted CI, and real logs
did not run. L-1f11i covers bounded producer execution next. L-1f11 remains open.
