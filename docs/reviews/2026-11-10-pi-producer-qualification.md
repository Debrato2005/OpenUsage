# Pi producer execution and write qualification

L-1f11i, November 10, 2026. The bounded investigation is complete. Pi still fails
the completeness gate. The candidate can accept a valid prefix while the
producer has not finished its write. Pi stays disabled. The production pin,
Pi candidate binary, and unpublished Amp patch are unchanged.

## Pinned execution

The fixture runs Pi's unchanged `SessionManager` at
`0d1793c2bc7139d1df4547fa3c0aeacb233e0534` with Node 22.23.2.
The [writer](https://github.com/earendil-works/pi/blob/0d1793c2bc7139d1df4547fa3c0aeacb233e0534/packages/coding-agent/src/core/session-manager.ts)
creates a file at the first conversation message. It writes buffered records
through one descriptor, then appends later records. Branch creation rewrites a
new file, and `forkFrom` copies records through another descriptor.
The pinned [upstream tests](https://github.com/earendil-works/pi/blob/0d1793c2bc7139d1df4547fa3c0aeacb233e0534/packages/coding-agent/test/session-manager/file-operations.test.ts)
cover delayed file creation, appends, and write failures.

A test loader resolves two value imports to their original pinned modules:
`getCurrentSystemMessage` and `uuidv7`. It replaces no writer, identity, or
filesystem implementation. Six dependencies come from the producer's lockfile,
with their integrity checked and no lifecycle scripts. The fixture pin checks
48 source and dependency files. This is session-writer execution, not a full
Pi CLI, installed SDK, provider, or account test.

Each writer uses an isolated mount namespace, private home, fixed synthetic
clock, and no network. The scanner receives only selected roots and an empty
private home. Generated IDs stay unchanged in source files. Recorded reports
normalize temporary paths and the generated branch ID. Price fields round to
12 decimal places to remove float addition-order differences. Counts and all
other fields stay exact. OpenUsage already keeps these estimates unavailable.

## Completed producer operations

Thirteen cases match independent expectations. They cover setup-only and
user-only sessions, assistant usage, reopening and appending, compaction,
branch summaries, standalone usage, optional summary usage, inactive branches,
branched files, forks, copies, zero observations, and absent categories.

| Case | Independent total |
|---|---:|
| Assistant, copied file, or optional summary usage | 120 |
| Reopened append or buffered flush | 137 |
| Compaction, branch summary, and standalone usage | 203 |
| Inactive branch | 160 |
| Branched session with new usage | 191 |
| Fork with new usage | 151 |
| Absent categories with a reported total | 162 |

Setup-only and user-only sessions return no usage rows. An explicit zero retains
one session and one day. Copied and inherited usage counts once. The scanner
leaves source hashes unchanged. Reports retain absent categories and estimates
under SC-022.

## Controlled schedules and qualification failure

Twelve schedules pause an owned Linux x86_64 scanner immediately before a source
open or after a source read returns. The real producer then completes an append,
compaction, branch, branched-file creation, fork, or optional-summary write.
Every overlapping scan returns unavailable with no report. Each subsequent
stable scan matches its independent expected total. The schedule changes no
scanner bytes, source contents, or syscall results.

Two other schedules pause the producer after a completed filesystem write.
The operation still has buffered records to write. The scanner runs to completion
while that producer remains paused. This exposes the remaining gate:

| Producer pause | Scanner result | Result after the producer finishes |
|---|---|---|
| After the session header | Empty report | 137 tokens |
| After a standalone usage record | 17 tokens | 137 tokens |

Both prefixes are valid JSONL. Daily and session reports agree because they use
one captured input. Neither the file syntax nor equal report totals proves that
the producer finished its operation. The checks record both cases with
`qualified: false`. Passing regression tests preserve this failure evidence.
They do not turn it into completed support.

The first pass checks metadata and bytes during collection. It cannot identify
a producer paused with a valid prefix. A delay or repeated equal read cannot
prove completion either. This is a publication-contract gap, not a report-parser
shape error. No timing heuristic, invented measurement, or broader process
inspection was added. L-1f11j must determine a supported completion boundary
before Pi enablement.

The storage regression starts with 120 saved tokens. Empty and 17-token prefixes
retain that value and record lower differences. The completed 137-token report
raises it. This protects accepted history under OU-LOG-011 and SC-024, but it
cannot recover new usage that the source has not yet published.

## Executed checks and limits

Twelve candidate Python tests passed. They include the thirteen producer cases,
fourteen schedules, and the inherited observation, identity, format, filesystem,
other-tool, and source-isolation checks. Seven snapshot parser tests and the
focused saved-history regression passed. JavaScript syntax checks passed.
Full Python/Rust verification passed, including workspace tests and Clippy.
The rebuilt desktop passed native local-log consent with the production scanner
in a private synthetic Linux home. This check preserves consent isolation and
saved data after rejected scans. Other native journeys did not run.

The first sandbox probe lacked library links. The next used an absent system
Node path. Correct library links and a read-only mount of the actual Node binary
resolved those setup errors. A trace helper initially passed a bytearray to
`os.fsdecode`; converting it to bytes fixed the failure. Two report comparisons
then failed on tiny price differences. A focused diagnostic found only cost-field
differences. The fixture normalizes their precision without changing counts.
No automatic retry, timeout, or interruption occurred during this slice.

Local search returned primary sources despite two search-engine errors. The
pinned GitHub page and source archive were available. Private metadata records
archive and dependency integrity, runtime scope, commands, and failures.
The existing [observation candidate](2026-11-10-pi-observations-candidate.md)
owns the unchanged scanner build and patch chain. No extra executable copy is
retained. No producer or scanner source correction was made.

The full producer test suite, installed Pi CLI, real accounts and logs, arbitrary
write interleavings, crash durability, performance, Windows, macOS, hosted CI,
and the full upstream Rust workspace did not run. Renderer checks were not
applicable. Native consent uses the production scanner and cannot qualify Pi.
