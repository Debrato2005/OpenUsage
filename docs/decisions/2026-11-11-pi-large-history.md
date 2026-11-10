# Large Pi histories need one capture

November 11, 2026 · L-1f11m · Design selected for a prototype.

Keep Pi disabled. Test one bounded scan that checks copies and forks across
all selected roots before it builds either report. Leave the production pin in
place until qualification passes.

## Limits

OU-NFR-020 proposes 60 seconds from consent to first history on a reference
machine with 1 GiB of synthetic logs. The pinned Pi candidate refuses more
than 128 MiB of source bytes during discovery. This cap prevents a complete
Pi observation even when every other input fits. The 32 MiB file limit and
100,000-record limit can also block histories with a different distribution.

Raising only the byte cap would let the scanner retain more raw input at once.
The [scale test](../performance-report.md#pi-candidate-on-linux) measured
672,820 KiB peak child memory on a 93,528,000-byte, 100,000-record workload.
That result does not predict release performance at 1 GiB.

## Batch experiment

The [experiment](../engineering-workflow.md#pi-large-history-experiment) ran
on Linux x86_64 with the pinned debug scanner and pinned Pi writer. The writer
created a private session with 120 tokens and a fork with 31 new tokens. Each
successful check compared exact daily and session totals from one report pair.

| Selected roots | Result |
|---|---|
| Original and renamed copy, in either order | 120 tokens in the original session |
| Original and a copy with different assistant text | Joint scan refused. Each root alone gave the same complete report. |
| Original and independent session with the same entry IDs and counts | 240 tokens across two sessions |
| Fork alone, with the parent file outside the selection | Refused |
| Original, fork, and renamed copy | 151 tokens. The original kept 120 and the fork kept 31. |

A second fixture contained 32 copies of the original file. Blank space padded
each file to 32 MiB. Each of eight roots fit the 128 MiB cap. The joint 1 GiB
selection produced no report. Each isolated batch reported 120 tokens, so the
sum would be 960. The unpadded copy check establishes why the sum is wrong.
The harness checked source hashes before and after each set of scans.

Separate reports lose the evidence needed for reconciliation. For example, a
copy and a conflict can yield identical pairs of daily and session reports.
Even a merge that uses session IDs cannot tell those cases apart. An isolated
fork can also fail before it has a report to merge.

## Prototype boundary

1. Open only the approved roots. Keep the candidate's checks for missing or
   changed files, link traversal, and directory changes. Resolve parent paths
   only against the selected manifest. A parent path never grants read access.
2. Parse within a bounded file or record buffer. Keep the minimum state needed
   to check headers, record trees, replays, and parent links across the whole
   capture. Test canonical record fingerprints for conflict checks, including
   context records and absent fields. Keep record parent links separately.
   Qualify fingerprint equality against the current strict comparison rules.
   Keep raw text in process memory only for the active input buffer.
3. Set limits on bytes, files, entries, nesting, records, index memory, output,
   and time. Recheck source bytes and the manifest across the capture before
   publication. Refuse a failed or over-budget capture in full. A disk index
   adds private state and cleanup, so defer it until a measured need exists.
4. Check identity before aggregation. Keep independent calls separate, suppress
   proven copies, and reject conflicts. Build both reports from the same
   accepted entries with checked integer sums and the pinned day zone. Preserve
   missing measurements and reported zeros. Keep the grouped runner and its
   consent recheck and one SQLCipher transaction. A refusal preserves saved
   history under [SC-025](../srs-clarifications.md#sc-025-captured-local-records-and-producer-completion).

## Remaining evidence

This experiment tests report composition and the input cap. Padding isolates
bytes from record density. The experiment did not measure first history, test
streaming, or set a new memory or time limit. The 1 GiB release test still needs
representative and dense fixtures, the reference machine, and timing from
consent through visible saved history. Measure scanner memory, storage time,
output size, and interaction latency. Repeat identity, privacy, and interrupted
capture checks before considering enablement. Real logs, Windows, and macOS
remain outside this result.
