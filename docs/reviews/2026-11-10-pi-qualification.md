# Pi qualification

Date: November 10, 2026. L-1f11c compares the pinned Linux scanner with Pi's primary source and 29 synthetic cases. Pi fails duplicate and completeness checks and stays disabled. No production code, tool selection, scanner pin, or Amp candidate changes.

## Source comparison

The scanner builds a duplicate key from the project, filename-derived session, timestamp, model, counts, and cost. It reads entry IDs for branch traversal but omits them from this key. Fork suppression compares usage signatures along the parent's current branch. File and JSON errors can leave a successful partial report. [Pinned loader](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/pi/src/loader.rs), [pinned parser](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/pi/src/parser.rs)

Pi's producer source is pinned to `0d1793c2bc7139d1df4547fa3c0aeacb233e0534`. Entry IDs are usually eight hex characters, with collisions checked within the session. They are not a global identity contract. Branch extraction preserves message IDs and creates a new session header with `parentSession`. Cross-project forks copy all non-header entries. A scanner must check lineage and conflicting content before joining these records. [Session manager](https://github.com/earendil-works/pi/blob/0d1793c2bc7139d1df4547fa3c0aeacb233e0534/packages/coding-agent/src/core/session-manager.ts)

The current format stores usage on compaction and branch-summary entries. It also has standalone usage entries, including cache warming. Those counts belong in token totals. Context size such as `tokensBefore` does not measure a new call. The pinned scanner reads only assistant-message usage, so it misses these other records. [Official format](https://pi.dev/docs/latest/session-format), [pinned format source](https://github.com/earendil-works/pi/blob/0d1793c2bc7139d1df4547fa3c0aeacb233e0534/packages/coding-agent/docs/session-format.md)

## Recorded results

Each comparison runs 58 daily/session reports. Fixtures use private homes, explicit roots, fixed offline arguments, and an independent token oracle. A populated ZCode root stays outside the selection. Source hashes remain unchanged. These tests model stored records; they do not run Pi or a model.

| Synthetic case | Expected tokens | Scanner tokens | Result |
|---|---:|---:|---|
| Baseline, same-project copy, repeated entry | 162 | 162 | Matched |
| Copy with a changed project or filename | 162 | 324 | Failed, including copies with entry IDs |
| Legacy copy without entry IDs | 162 | 324 | Failed |
| Two distinct IDs with equal usage and time | 324 | 162 | Failed. One call disappears |
| Same ID with conflicting content | Unavailable | 354 | Failed. Conflict is added |
| Linear fork prefix | 162 | 162 | Matched |
| Fork with 17 new tokens | 179 | 179 | Matched |
| Fork from an inactive branch | 202 | 219 | Failed |
| Clone of a branched history | 202 | 242 | Failed |
| Malformed lineage | Unavailable | 324 | Failed. Scan appears complete |
| Child whose parent is absent from the selected roots | 162 | 162 | Count matched for this isolated child |
| Compaction or branch summary with 49 tokens | 211 | 162 | Failed. Summary usage omitted |
| Compaction with context size but no usage | 162 | 162 | Matched. Context size adds no tokens |
| Standalone usage with known or unknown kind | 179 | 162 | Failed. Usage omitted |
| Unknown model, missing model, or missing token details | 162 | 162 | Total matched; unavailable fields remain unavailable |
| Missing timestamp | 162 with no known date | 0, no rows | Failed. Record omitted |
| Malformed JSON or invalid UTF-8 peer | Unavailable | 162 | Failed. Incomplete read appears successful |
| Derived transcript under `subagent-artifacts` | 162 | 162 | Excluded by the pinned scanner's path rule |
| Fresh child under `run-*` | 179 | 179 | Matched |
| Missing path-derived project | 162 | 162 | Project stays unavailable |
| Empty session | 0 | 0, no rows | No measurement in OpenUsage |

## Application boundary and next correction

The Rust regression preserves independent totals, known activity times, unavailable projects, missing models, and unavailable token details and prices. It does not repair the scanner's count errors. Equal times, paths, and token counts cannot prove event identity. No reasoning count or account identity is inferred.

A separate scanner candidate must use scoped session/entry identities, validate lineage across all branches, reject conflicting or incomplete reads, and include non-message usage. It must preserve missing fields and avoid treating context size as consumption. Legacy records without IDs and absent parent files need explicit capability rules. Global deduplication by the short entry ID alone would also be wrong. The candidate must remain local, with the production pin unchanged.

## Verification

Doctor, ten focused Rust period tests, and the pinned scanner matrix passed. Full Python/Rust verification, formatting, Clippy, and rebuilt native consent also passed. No automatic retry, timeout, or interruption occurred. During fixture review, the new fork call's timestamp was moved after its fork header. Both captures are retained privately; the first capture was not a failed check. An ad hoc catalog inspection assumed an array and failed. The corrected object lookup passed and confirmed the unchanged enabled tools and scanner pin.

Renderer checks, other native journeys, hosted CI, real logs, Windows, macOS, concurrent producer writes, and producer runtime execution have not run for this slice. AT-65 remains failed for Pi.

Fixtures: [generator and comparison](../../tests/tools/test_ccusage_pi.py), [recorded reports](../../crates/openusage-locallogs/tests/fixtures/remaining-tools/pi-qualification.json), and [Rust regression](../../crates/openusage-locallogs/tests/agent_periods.rs). Private evidence uses `.recovery/audit/l1f11c-*`.
