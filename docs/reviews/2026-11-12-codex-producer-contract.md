# Codex paginated producer contract

November 12, 2026 · L-1f11am · Linux x86_64, synthetic app-server output.

The [pinned Codex 0.163.0 producer](../../crates/openusage-codex/provider-sources/codex-0.163.0-linux-x86_64.json) emitted records with explicit identity and
lineage. Both scanners still lose 1,150 own fork tokens in the full capture.
This result establishes the tested producer boundary before a separate correction.
The production scanner and Pi candidate pins stay unchanged.

## Executed producer

The experiment uses the reviewed 0.163.0 executable and source commit
`83086188ae0614bd882818ff1aa23f3194962012`. Fifteen selected source files match
their pinned Git blobs and SHA-256 digests. The recorder now lives in the
separate rollout crate. The old recorder URL returned 404. Local research could
read the official documentation but rejected raw Rust text. Selected official
source bytes were then retrieved and checked against the cached source tree.

Bubblewrap mounts the executable and harness read-only. A private writable
workspace supplies HOME, CODEX_HOME, and XDG paths. The network namespace differs
from the host and has only loopback, with no outbound route. A local Responses
fixture supplies six controlled replies. No authorization header reaches it.
The producer and source pin retain their hashes.

The app-server uses the experimental API with its tested default paginated
history. The first process closes before a second process resumes the thread.
These results do not establish every CLI entry point or history mode.

## Captured contract

| Observation | Executed result |
| --- | --- |
| One response | Adjacent usage record and token counter describe the same 4,600 tokens |
| Resume after restart | Existing bytes and thread/session identity survive. The new response adds 2,300 |
| Separate equal-usage thread | Distinct emitted identities and another 4,600 tokens |
| Fork | Its own usage is 1,150. Its cumulative total is 8,050, including 6,900 inherited tokens |
| Missing usage | No usage record for that response |
| Reported zero | An explicit zero usage record |

A [`token_usage_record`](https://github.com/openai/codex/blob/83086188ae0614bd882818ff1aa23f3194962012/codex-rs/protocol/src/protocol.rs) carries `thread_id`, `session_id`, `turn_id`,
`root_turn_id`, and `response_id`.
The producer also records ordinals. The fork declares its parent with
`history_base` and an exclusive ordinal and byte boundary. In the final capture,
that boundary is ordinal 24 and byte 19,100. The child begins at that ordinal
and holds its own response records. It does not copy the parent's usage rows.
Those exact bytes belong to this capture, not every future run.

The usage record and counter are two representations of one response. They
must not enter one sum twice. A cumulative thread value is not the child's
own usage. Missing usage is not zero. These statements concern captured source
records, not provider billing or verified account identity.

## Consumer comparison

Nine cases produce 18 report pairs and 72 repeated commands. Both binaries
repeat and match in every pair. Ten pairs meet independent expectations.
Eight do not:

| Case | Both scanners |
| --- | --- |
| Renamed copy, session report | 9,200 instead of 4,600 |
| Equal-time mutation, daily report | 4,600 instead of 9,200, despite distinct emitted IDs |
| Conflicting copy, both reports | Success instead of refusal |
| Full fork capture, both reports | 11,500 instead of 12,650 |
| Missing selected parent, both reports | Empty success instead of the conservative refusal expectation |

Copy, replay, conflict, and equal-time mutations remain separate from unaltered
producer output. The missing-parent refusal is a conservative test expectation.
The next candidate must decide which explicit own records remain valid under
[SC-025](../srs-clarifications.md#sc-025-captured-local-records-and-producer-completion) when a referenced prefix is unavailable. It must not open unselected paths.

The prefix control isolates the fork loss. The parent, second thread, and
child's valid prefix report 12,650. Extending the child with actual missing-usage
and zero turns lowers the total to 11,500. The parser's
[`detect_rewritten_burst`](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/codex/src/parser.rs#L137) function classifies close token counters as replay.
It cannot establish identity from that timing pattern.

## Checks and limits

Eleven focused tests passed with the live comparison. Doctor and the selected
engineering checks passed. All selected source sets, executable pins, fixture
inputs, and the private receipt retain their hashes.

The [tracked fixture](../../tests/tools/fixtures/codex-producer/capture.json) contains identity/usage projections, original byte counts,
record hashes, and source fingerprints. The unaltered capture stays private.
Live comparison requires the exact receipt whose hash the fixture records.
The producer harness can also make a new private capture. Updating a capture
projection remains a separate reviewed action.

The first sandbox attempt failed before producer execution. A corrected mount
path handling preceded the next trial. Failed receipts remain separate from
the final capture. No command received an automatic retry.

Legacy history, compaction, subagents, revert, real providers, native acceptance,
and other platforms remain untested in this producer experiment. Application
code did not change. The earlier native results keep their original scope.
