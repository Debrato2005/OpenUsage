# Qwen usage record qualification

L-1f11ad, November 11, 2026. The pinned scanner merges distinct Qwen calls
when their session, time, model, and counters match. Qwen stays disabled.

## Executed reports

The [fixture](../../tests/tools/test_ccusage_qwen.py) runs the production
`ccusage` 20.0.48 binary on private synthetic Linux input.
The [qualification record](../../tests/tools/fixtures/qwen/qualification.json)
covers 34 cases and 136 repeated scanner runs.
Ten cases match both views. Eleven session outcomes match.
The fixture supplies private homes and read-only selected roots through Bubblewrap.
Repeated reports and source hashes match.

| Scenario | Required tokens | Pinned result |
| --- | ---: | ---: |
| Single call, exact copy, or replay | 120 | 120 |
| Two distinct UUIDs with equal times and counters | 240 | 120 |
| Two distinct calls at different times | 240 | 240 |
| Copied UUID in a new session | 120 | 240 |
| Inherited fork and 17 new tokens | 137 | 257 |
| Cross-file conflicting counters for one UUID | Unavailable | 257 |
| Ordered usage replacement within one file | 137 | 257 |
| Inclusive cache count | 120 | 130 |
| Thoughts already included in output | 120 | 125 |
| Missing provider total defaulted to zero | Unavailable | 120 |
| Synthetic measured zero | One zero row | No rows, null totals |
| Malformed, invalid UTF-8, or missing peer | Unavailable | 120 from valid input |

The inherited-fork fixture represents the pinned source's copy behavior.
It does not execute the fork writer.
The synthetic compaction notice preserves the prior 120 tokens.
The null-to-usage enrichment also keeps 120 tokens.
These checks cover complete captured records under
[SC-025](../srs-clarifications.md#sc-025-captured-local-records-and-producer-completion).

A total-only record puts 120 tokens in output.
Missing time receives the file modification time as usage time.
Missing models produce numeric zero cost and an unpriced-model marker.
Negative, fractional, text, and contradictory counts produce successful reports.
Missing measurements must remain unavailable under
[SC-022](../srs-clarifications.md#sc-022-incomplete-metrics-from-the-other-local-tools).
Empty input returns null totals. The harness preserves that null value.
It distinguishes an empty capture from a measured-zero row.

## Source contract and next gate

The source review pins Qwen Code v0.27.2 at
[`f42fe0f3`](https://github.com/QwenLM/qwen-code/tree/f42fe0f3251172c1174d00382ece522eafb3c6c2).
All thirteen reviewed files match their digests and official Git blobs.
The producer did not execute.

The [record writer](https://github.com/QwenLM/qwen-code/blob/f42fe0f3251172c1174d00382ece522eafb3c6c2/packages/core/src/services/chatRecordingService.ts#L1779)
assigns UUIDs. The [fork operation](https://github.com/QwenLM/qwen-code/blob/f42fe0f3251172c1174d00382ece522eafb3c6c2/packages/core/src/services/sessionService.ts#L4666)
preserves UUIDs, timestamps, models, and usage while changing the session ID.
It records the original session and message under `forkedFrom`.
Session identity alone therefore cannot suppress inherited usage.

The [transcript reader](https://github.com/QwenLM/qwen-code/blob/f42fe0f3251172c1174d00382ece522eafb3c6c2/packages/core/src/utils/transcript-records.ts#L513)
merges ordered fragments by UUID. Later usage replaces earlier usage.
This supports ordered enrichment inside one transcript.
It does not establish authority between conflicting copies in separate roots.

The [Anthropic adapter](https://github.com/QwenLM/qwen-code/blob/f42fe0f3251172c1174d00382ece522eafb3c6c2/packages/core/src/core/anthropicContentGenerator/usage.ts#L45)
includes cache counts in prompt tokens. The
[output convention](https://github.com/QwenLM/qwen-code/blob/f42fe0f3251172c1174d00382ece522eafb3c6c2/packages/core/src/telemetry/gen-ai-usage.ts#L33)
depends on provider authentication. OpenAI, Responses, and Qwen OAuth include thoughts in output.
Gemini reports thoughts separately.
The reviewed assistant record does not prove that provider convention for each usage entry.

The [recording path](https://github.com/QwenLM/qwen-code/blob/f42fe0f3251172c1174d00382ece522eafb3c6c2/packages/core/src/core/llm-chat.ts#L7198)
coerces missing, negative, or nonfinite fields to zero before it saves usage.
The saved summary alone cannot distinguish unknown counters from measured zero.
Cache-presence details stay in a
[WeakMap](https://github.com/QwenLM/qwen-code/blob/f42fe0f3251172c1174d00382ece522eafb3c6c2/packages/core/src/telemetry/gen-ai-usage.ts#L10),
which the transcript does not serialize.

The stream recorder retains accepted assistant turns and can discard failed attempts.
These [recording rules](https://github.com/QwenLM/qwen-code/blob/f42fe0f3251172c1174d00382ece522eafb3c6c2/packages/core/src/core/llm-chat.ts#L7731)
do not establish a complete ledger of billed provider calls.
Managed transcripts and arbitrary imports remain outside this source contract.

The pinned [scanner parser](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/qwen/src/parser.rs)
uses session, timestamp, model, and counters for identity. It ignores the UUID.
It substitutes missing fields and discards read or record failures.
The [path resolver](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/qwen/src/paths.rs)
also omits missing roots.

A candidate needs bounded capture of every selected root and a proven usage identity.
It must reconcile supported record enrichment without summing snapshots.
It must preserve unavailable counters and refuse unresolved cross-file conflicts.
Counter semantics need their own source proof before costs or breakdowns qualify.
