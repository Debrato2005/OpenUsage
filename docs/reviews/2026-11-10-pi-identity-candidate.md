# Local Pi identity candidate

Date: November 10, 2026. L-1f11e adds a separate identity patch after the [usage patch](2026-11-10-pi-usage-candidate.md). Both target ccusage commit `8995f3b7fd7110b8f37564f7d0468acc7a7d6b20`. Pi stays disabled. The production scanner and unpublished Amp patch are unchanged.

## Source and correction

Three sources guide the change: Pi's producer contract, the pinned scanner, and synthetic cases with independent expected counts. Pi assigns short entry IDs within a session. Branch creation preserves those IDs, can remove label records, and changes parent links. A full clone preserves all branches. The old scanner compares a prefix of usage values on the last branch. This loses separate equal-sized calls and counts some copied branches twice. [Producer source](https://github.com/earendil-works/pi/blob/0d1793c2bc7139d1df4547fa3c0aeacb233e0534/packages/coding-agent/src/core/session-manager.ts), [format contract](https://github.com/earendil-works/pi/blob/0d1793c2bc7139d1df4547fa3c0aeacb233e0534/packages/coding-agent/docs/session-format.md).

The candidate uses source session and entry IDs. A copied session keeps its ID after a file or folder rename. Each new entry retains its count, even when all usage values equal another call. The reported working directory supplies the project. An absent directory stays unknown. These IDs describe source events, not accounts.

A parent link must resolve among files selected for the same store. It never permits a read elsewhere. Every branch is checked. Copied records remain in their source session; new child calls stay in the child. Comparison between parent and child excludes `parentId`, because Pi can change that field when removing labels. Other source fields must agree. Within one session, parent links must also agree. Independent sessions and sibling calls keep separate short-ID scopes. No time, path, or equal-count heuristic establishes identity.

Version 2 and 3 trees require valid headers, IDs, and entry links. Legacy version 1, headerless input, missing IDs, absent parents, cycles, and conflicts return an error without a partial report. Raw source comparison preserves the difference between absent metrics and reported zeros. It also checks zero-usage and non-usage records. Malformed records and read failures now propagate an error. Static errors contain no source paths or content. Validation runs before the report mapper.

## Evidence

The test-first checks reproduced renamed-copy duplication, lost distinct calls, ignored conflicts, inactive-branch duplication, and full-clone duplication. The first full adapter run also found an old test that expected file-derived session names. The new source-ID policy replaces that assumption. The old loader tests for signature-prefix matching and accepted legacy data are replaced with source-ID and explicit unsupported-input tests. Existing mapper, price, path, and report tests remain.

All 42 final adapter tests passed. Coverage includes growing copies, reversed root order, nested forks, removed labels, sibling short-ID collisions, named stores, threading modes, invalid lineage, conflicting headers, malformed records, and derived transcript exclusion. Clippy passed after replacing one unnecessary test clone. The first compile attempt needed an explicit error type. Formatting passed. Guessed core-source and closure-helper paths failed; corrected file discovery found them. The doc helper assumed an individual review entry in the document map. The map uses a directory entry; the corrected helper keeps that structure. There was no automatic retry, timeout, or interruption.

Five candidate Python tests passed. Forty Pi cases produce 80 recorded daily/session outcomes: 25 accepted cases, including an empty tree, and 15 explicit unavailable cases. Multi-root cases run in reverse order too. The other fifteen formats and existing Claude/Codex reports match the reference. Empty roots and five JSON/table views pass. Every report uses a private empty home. Source file hashes stay unchanged, and an unselected ZCode source contributes nothing.

The fresh-child fixture now has a null parent entry ID, as an independent source session requires. Its pinned output is unchanged. The baseline fixture remains the production scanner's recorded evidence. Twelve OpenUsage period tests passed. The new test reads actual candidate reports and preserves independent totals, missing details and estimates, incomplete model attribution, and source activity times.

| Case | Production pin | Identity candidate | Expected |
|---|---:|---:|---:|
| Copy with renamed file or project folder | 324 | 162 | 162 |
| Distinct IDs with equal usage | 162 | 324 | 324 |
| Fork of an inactive branch | 219 | 202 | 202 |
| Full clone of branched history | 242 | 202 | 202 |
| Same ID with changed usage | 354 | Unavailable | Unavailable |
| Summary usage added to baseline | 162 | 211 | 211 |
| Standalone total without categories | No row | 17 | 17 |

Full OpenUsage Python/Rust verification and rebuilt native consent passed. Native consent uses the production scanner and does not prove candidate readiness. Saved-history preservation after scan failure remains covered by the grouped runner and storage tests in the full plan.

## Build and boundary

The build uses Rust 1.99.1, locked offline dependencies, and the existing verified pricing input. Its version is `20.0.48-openusage.pi-identity-candidate.1`. It changes no dependency or wire field. The Cargo, pnpm, and flake locks remain unchanged. The identity patch passes an apply check on the separate usage-only worktree and a reverse check on its own worktree.

Patch: [20.0.48-pi-identity.patch](../../patches/ccusage/20.0.48-pi-identity.patch), SHA-256 `04868d7358aa572a1e6782ea94d35639bffca359f199cafd9a7359f21fd85177`. Candidate binary SHA-256: `d29a3b913bf30ac86598465331fd2ab5fb4098155c12441da259ffb255702902`. Private metadata records the source, both patch hashes, real upstream lockfiles, pricing input, build command, and checks. No extra binary copy is retained.

Renderer checks, other native journeys, pinned upstream Nix/Rust 1.97.1, the full upstream workspace, hosted CI, real accounts/logs, Windows, macOS, producer execution, concurrent writes, and performance limits did not run in this slice.

## Remaining gates

Directory discovery can still omit failures, and concurrent producer writes need a coherent-read policy and tests. Missing source times now reject the scan; they do not yet become a supported unplaced observation. Unknown formats and large inputs need bounded tests. These gates remain open before Pi enablement or a production pin change. All sixteen candidate tools remain disabled. Only Claude Code and Codex are enabled.
