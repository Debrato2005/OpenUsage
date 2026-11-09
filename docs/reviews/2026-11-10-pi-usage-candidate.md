# Local Pi usage candidate

Date: November 10, 2026. L-1f11d prepares a separate patch against ccusage commit `8995f3b7fd7110b8f37564f7d0468acc7a7d6b20`. The patch includes usage that the pinned scanner omits. Pi stays disabled. The production pin and the unpublished Amp patch are unchanged.

## Correction

The [Pi qualification](2026-11-10-pi-qualification.md) compares the pinned scanner with Pi's producer source. The producer records model calls on `compaction`, `branch_summary`, and standalone `usage` entries. The scanner previously required usage nested inside an assistant message. [Producer contract](https://github.com/earendil-works/pi/blob/0d1793c2bc7139d1df4547fa3c0aeacb233e0534/packages/coding-agent/docs/session-format.md)

The candidate reads those three record types. It accepts an arbitrary standalone usage kind, including cache warming. It never adds `tokensBefore` or a summary's text as usage. It does not infer a missing model or timestamp. A total without token details stays separate from the token categories. It never becomes an output or reasoning count. The existing assistant-message mapping remains unchanged.

The shared ccusage wire format still has numeric category fields. Those fields do not prove a measured zero when the source omitted them. OpenUsage's candidate regression preserves nullable details and prices under SC-022. Missing model attribution also makes model totals unavailable. The candidate is not a new price or identity contract.

## Evidence

Three test-first regressions captured the missing standalone record, missing summary records, and an inferred output count from a total-only record. Each failed for its expected reason before its correction. All 38 Pi adapter tests then passed. The changed adapter passed Clippy with `--no-deps`. The formatter first reported a tuple-layout difference; formatting corrected it, and the final check passed.

The candidate build uses Rust 1.99.1, locked offline dependencies, the existing verified pricing input, and version `20.0.48-openusage.pi-candidate.1`. It adds no dependency or report field. The Cargo, pnpm, and flake lockfiles are unchanged. The [Amp build note](2026-11-08-amp-dedup-design.md#build-preparation) owns the source, toolchain, and pricing preparation.

Five Python tests passed. They compare all sixteen baseline formats and empty roots, existing Claude/Codex reports, Pi's 29-case matrix, and five JSON/table views. Source hashes remain unchanged, and each report uses an empty private home. The table views cover unified daily and weekly reports plus Pi daily, monthly, and session reports.

| Fixture | Production pin | Candidate | Independent expectation |
|---|---:|---:|---:|
| Baseline plus a compaction call | 162 | 211 | 211 |
| Baseline plus a branch-summary call | 162 | 211 | 211 |
| Baseline plus cache-warming usage | 162 | 179 | 179 |
| Baseline plus an unknown usage kind | 162 | 179 | 179 |
| Standalone total without categories or model | No row | 17 | 17, with unavailable details |

The other 25 cases in the Pi matrix match the production baseline, including its known failures. Eleven OpenUsage period tests passed. They read the candidate's actual reports and preserve independent totals, unavailable details and estimates, incomplete model attribution, and known activity times. Full OpenUsage Python/Rust verification, formatting, Clippy, and rebuilt native consent also passed. The final Python plan includes the added reference check for the total-only record.

The patch passes an apply check against pristine Pi files from the exact source commit and a reverse check against its separate worktree. The original Amp worktree remains untouched. A guessed documentation path and lockfile lookup failed during inspection. The actual paths were then found and checked. The native-artifact helper also rejected the upstream lockfile layout. A separate metadata record preserves its actual source, lockfiles, pricing, binary hash, and executed checks. No automatic retry or interruption occurred.

## Remaining gates

Pi still fails copied-file identity, distinct calls with equal counts, inactive-branch forks, conflicting IDs, malformed lineage, untimed records, and incomplete reads. This patch does not resolve those issues. Malformed or absent usage fields and concurrent writes also need stricter completeness rules. An absent source value must not become a successful zero result.

The pinned upstream Nix/Rust 1.97.1 environment, full upstream workspace, hosted CI, real logs, Windows, macOS, producer execution, and performance limits have not run. Native consent remains a production-pin regression, not native acceptance of this candidate. No tool is enabled and no release pin is promoted.

Patch: [20.0.48-pi-usage.patch](../../patches/ccusage/20.0.48-pi-usage.patch). Comparison: [test_ccusage_pi_candidate.py](../../tests/tools/test_ccusage_pi_candidate.py). Recorded output: [candidate reports](../../crates/openusage-locallogs/tests/fixtures/remaining-tools/pi-usage-candidate.json) and [CLI views](../../crates/openusage-locallogs/tests/fixtures/remaining-tools/pi-usage-candidate-cli.json). Private receipts use `.recovery/audit/l1f11d-*`.
