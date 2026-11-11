# Codebuff scanner qualification

L-1f11y, November 11, 2026. The production scanner matches eight of thirty
synthetic Codebuff cases on Linux. Codebuff stays disabled. Its token reports
need both reliable measurements and stable usage identity.

## Executed reports

The [fixture](../../tests/tools/test_ccusage_codebuff.py) runs the pinned
`ccusage` 20.0.48 binary. It checks the executable digest before each suite.
Bubblewrap supplies private homes and network and process namespaces.
Only selected synthetic roots enter the scanner as read-only mounts.
The suite compares two runs of each daily and session report, for 120 runs.
All repeat comparisons and before-and-after file hashes match.

The [qualification record](../../tests/tools/fixtures/codebuff/qualification.json)
keeps the required observations and actual reports separate.
Eighteen of sixty report outcomes match. Eight cases match in both views.

| Scenario | Required token observation | Pinned result |
| --- | ---: | ---: |
| Single chat, same-channel copy, or replay | 162 | 162 |
| Copy under a renamed channel or chat | 162 | 324 |
| Branch with 17 new tokens | 179 | 341 |
| Two distinct calls with equal counts | 324 | 324 |
| Conflicting copies | Unavailable | 179 or 162 by path order |
| Malformed, invalid UTF-8, or missing peer | Unavailable | 162 |
| Measured zero | One zero row | No rows |
| Explicit zero with conflicting fallback | 42 with zero input | 162 with 120 input |
| Independent total without categories | 162 with unavailable output | 162 labeled as output |
| Credits without token measurements | Token total unavailable | 0 tokens with credits |

Synthetic branch cases preserve the inherited message ID on purpose.
They test the scanner against known lineage in the fixture.
They do not establish how the producer creates branches.
Negative and fractional buckets and a contradictory total also return reports.
Without a message date, the scanner uses the chat creation time or file
modification time. Neither establishes the day of a later call.

## Producer source

The source review pins CodebuffAI/freebuff at
[`64998d85`](https://github.com/CodebuffAI/freebuff/tree/64998d85ade9672bc6c740a5bf10158324b2d079).
All fifteen inspected files match their Git blobs. The producer did not execute.

The [CLI message schema](https://github.com/CodebuffAI/freebuff/blob/64998d85ade9672bc6c740a5bf10158324b2d079/cli/src/types/chat.ts#L236)
stores a response ID, display time, optional credits, and other UI fields.
Its metadata has no token usage field. It identifies embedded run state as
an older format. The [writer](https://github.com/CodebuffAI/freebuff/blob/64998d85ade9672bc6c740a5bf10158324b2d079/cli/src/utils/run-state-storage.ts#L329)
serializes chat messages and run state separately.
The [provider metadata](https://github.com/CodebuffAI/freebuff/blob/64998d85ade9672bc6c740a5bf10158324b2d079/sdk/src/impl/model-provider.ts#L638)
for the Codebuff backend carries cost fields, without token buckets.
The [SDK](https://github.com/CodebuffAI/freebuff/blob/64998d85ade9672bc6c740a5bf10158324b2d079/sdk/src/run.ts#L323)
exposes per-request token usage through callbacks. The reviewed CLI persistence
path does not write those callbacks as per-request token records.

The [response ID](https://github.com/CodebuffAI/freebuff/blob/64998d85ade9672bc6c740a5bf10158324b2d079/cli/src/utils/ai-message-id.ts#L8)
identifies a UI response shell. It does not identify each model call in a turn.
The transcript schema does not identify the product that wrote it.
The source review covers this CLI format and Codebuff backend path.
It does not rule out different historical, desktop, or provider formats.

Two fixtures use the current CLI response shape. Neither includes token
measurements. The pinned scanner reports a zero-token row when credits exist.
It returns no rows when credits are absent. Neither outcome establishes
measured zero. The older synthetic usage fields remain consumer tests.

## Required boundary

To qualify current CLI token accounting, the producer must persist measured
counts with stable per-call origin IDs or an equivalent explicit coverage contract.
Copies, replay, and branches must preserve inherited origin. A new call needs
a distinct origin. If reports separate Codebuff from Freebuff, the source must
also establish product identity. Credits cannot supply token counts.

A candidate must reject incomplete captures and conflicts across selected roots.
It must preserve independent totals and unavailable fields under
[SC-022](../srs-clarifications.md#sc-022-incomplete-metrics-from-the-other-local-tools).
Historical formats can qualify separately if their producer contract supports it.
No scanner, application, catalog, or production pin changes in this slice.
