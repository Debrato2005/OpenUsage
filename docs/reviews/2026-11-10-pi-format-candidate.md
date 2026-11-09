# Local Pi format candidate

Date: November 10, 2026. L-1f11f0 adds a third local Pi patch after the [usage](2026-11-10-pi-usage-candidate.md) and [identity](2026-11-10-pi-identity-candidate.md) patches. Pi stays disabled. The production scanner and unpublished Amp patch are unchanged. Filesystem completeness follows in L-1f11f1.

## Source and correction

The review compares the pinned scanner, the producer contract, and synthetic results with independent expectations. The scanner's byte filter can skip an escaped JSON key. Its separate typed parse can discard a record after validation. Parsing a header as a message can also replace its source ID with a filename. These failures were reproduced before the correction.

Pi's format separates session metadata, assistant messages, usage records, and context records. A context edit changes future context without changing raw usage. [Producer format](https://github.com/earendil-works/pi/blob/0d1793c2bc7139d1df4547fa3c0aeacb233e0534/packages/coding-agent/docs/session-format.md), [producer implementation](https://github.com/earendil-works/pi/blob/0d1793c2bc7139d1df4547fa3c0aeacb233e0534/packages/coding-agent/src/core/session-manager.ts).

Reports now use the validated source records directly. Escaped keys retain their meaning. Header IDs and projects come from validated metadata. Invalid present fields, unknown record types, and duplicate object keys fail the scan. Duplicate checks apply at every depth and treat escaped key spellings as the same key. Errors contain no source content or path.

An independent assistant total stays separate from token categories. The previous fallback assigned the total to output when details were absent. SC-022 does not permit that inference. The shared wire format still uses numeric categories, but OpenUsage keeps missing details and estimates unavailable. This change does not infer reasoning counts, times, or account identity.

## Evidence

Five initial tests failed for the expected reasons: escaped usage disappeared, header metadata changed identity, invalid model fields were accepted, duplicate keys were accepted, and a total became output. After the correction, one existing mapper test still expected the old inferred output. Its expectation now follows SC-022. All 50 final adapter tests passed, with Clippy and formatting.

Six candidate Python tests passed. They compare the earlier 40-case identity matrix, nineteen new format cases, empty roots, other fifteen tools, enabled Claude/Codex, and five JSON/table views. The identity matrix keeps every outcome and total. Its missing-details case now has no inferred output category.

The nineteen format cases record 38 daily/session outcomes: five accepted cases and fourteen unavailable cases. They cover escaped keys, duplicate keys, unrelated header fields, invalid types and counts, context edits, and independent totals. Each report uses a private empty home. Source hashes stay unchanged, and an unselected ZCode source contributes nothing.

Thirteen OpenUsage period tests passed. The new test reads the actual format-candidate reports and preserves totals, missing details and estimates, reported projects, and known activity times. Full OpenUsage Python/Rust verification and rebuilt native consent also passed.

The initial patch-application commands had an incorrect relative path and changed no files. Correct absolute paths then applied both prerequisite patches. There was no automatic retry, timeout, or interruption.

## Build and remaining gates

The candidate uses ccusage source `8995f3b7fd7110b8f37564f7d0468acc7a7d6b20`, Rust 1.99.1, locked offline dependencies, and the existing verified pricing input. It adds no dependency or wire field. Cargo, pnpm, and flake locks are unchanged. Version: `20.0.48-openusage.pi-completeness-candidate.1`.

Patch: [20.0.48-pi-format.patch](../../patches/ccusage/20.0.48-pi-format.patch), SHA-256 `a4f347098e7377328aa718f75e2c3bfee78ceea772409805f32a6acb5a2027cb`. Binary SHA-256: `6113b25dd422d42b2de07575d4aae136040e4a5363d8595a681700079fe58b85`. Layered apply and reverse checks passed. Private metadata retains the source, patch layers, lockfiles, pricing input, and executed checks. No extra binary copy is retained.

Native consent uses the production scanner and does not qualify this candidate. Renderer checks, other native journeys, pinned upstream Nix/Rust 1.97.1, full upstream workspace, hosted CI, real logs, and other platforms did not run. Producer execution, concurrent writes, directory failures, and large-input limits also remain unverified.

L-1f11f remains open. Its next slice must reject omitted or changed selected inputs and bound source reads. Missing source times remain unavailable. Only Claude Code and Codex are enabled.
