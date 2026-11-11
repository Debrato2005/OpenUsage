# Kimi wire qualification

L-1f11ae, November 11, 2026. The pinned scanner counts inherited Kimi usage
again after the session folder changes. Kimi stays disabled.

## Executed reports

The [fixture](../../tests/tools/test_ccusage_kimi.py) runs the production
`ccusage` 20.0.48 binary on private synthetic Linux input.
The [qualification record](../../tests/tools/fixtures/kimi/qualification.json)
covers 38 cases and 152 repeated scanner runs.
Nine cases match both views. Eleven session outcomes match.
Bubblewrap supplies private homes and mounts only selected read-only roots.
Repeated reports and source hashes match.

| Scenario | Required tokens | Pinned result |
| --- | ---: | ---: |
| Single old-format status update | 135 | 135 |
| Exact old-format copy or replay | 135 | 135 |
| Distinct old message IDs with equal values | 270 | 270 |
| Old-format copy under a new session folder | 135 | 270 |
| Synthetic inherited fork and 17 new tokens | 152 | 287 |
| Conflicting old usage for one message ID | Unavailable | 287 |
| New-format distinct calls with equal values | 270 | 135 |
| New-format usage from independent agent paths | 270 | 135 |
| New-format turn and session requests | 270 | 135 |
| New-format session or prediction request alone | 135 | No rows |
| New-format missing cache measurement | Unavailable | 125 |
| Synthetic measured zero in either format | One zero row | No rows |
| Malformed, invalid UTF-8, or missing peer | Unavailable | 135 from valid input |

The lineage and distinct-call cases use known synthetic origins.
They do not execute a producer fork or request.
The new replay and distinct-call fixtures contain identical saved records.
Their required totals differ because the fixture observer knows their origins.
This does not qualify an ordinal or ancestry-based identity rule.
A synthetic compaction notice preserves the old-format 135 tokens.
Missing usage time receives the file modification time.
Missing models receive fallback labels and numeric estimates.
A total-only old record puts 135 tokens in output.
Negative, fractional, text, and contradictory counts also produce reports.

These checks cover complete captured records under
[SC-025](../srs-clarifications.md#sc-025-captured-local-records-and-producer-completion).
Missing measurements must stay unavailable under
[SC-022](../srs-clarifications.md#sc-022-incomplete-metrics-from-the-other-local-tools).

## Source contract and next gate

The source audit separates two official producers. It pins legacy Kimi CLI at
[`9ab1286b`](https://github.com/MoonshotAI/kimi-cli/tree/9ab1286b8fe4e6bcd116949a27ce5e0ac3389c82)
and current Kimi Code at
[`482936af`](https://github.com/MoonshotAI/kimi-code/tree/482936afceaec4d513aa28f7b7e6b461e35e105f).
All sixty selected source files match their digests and official Git blobs.
Neither producer executed.

Legacy [status usage](https://github.com/MoonshotAI/kimi-cli/blob/9ab1286b8fe4e6bcd116949a27ce5e0ac3389c82/src/kimi_cli/wire/types.py#L176)
describes a completed step. Its message ID comes from the provider response and can be absent.
The four token categories are additive in the
[legacy contract](https://github.com/MoonshotAI/kimi-cli/blob/9ab1286b8fe4e6bcd116949a27ce5e0ac3389c82/packages/kosong/src/kosong/chat_provider/__init__.py#L99).
The [fork writer](https://github.com/MoonshotAI/kimi-cli/blob/9ab1286b8fe4e6bcd116949a27ce5e0ac3389c82/src/kimi_cli/session_fork.py#L239)
copies retained wire lines unchanged into a new session.
The old-format fork fixture therefore represents source-supported behavior.
The wire timestamp records local append time. The status update carries no model.
Current settings or fallback labels cannot prove the historical model.

Current [usage records](https://github.com/MoonshotAI/kimi-code/blob/482936afceaec4d513aa28f7b7e6b461e35e105f/packages/agent-core-v2/src/agent/usage/usageOps.ts#L7)
carry agent ID, model, counts, scope, and local time.
They omit a request, message, or turn ID. The
[base event](https://github.com/MoonshotAI/kimi-code/blob/482936afceaec4d513aa28f7b7e6b461e35e105f/packages/agent-core-v2/src/app/event/event2.ts#L22)
adds no unique ID.

The current [usage model](https://github.com/MoonshotAI/kimi-code/blob/482936afceaec4d513aa28f7b7e6b461e35e105f/packages/agent-core-v2/src/session/usage/usageAgentModel.ts#L25)
adds every request record. Scope describes the request source.
Turn, session, and prediction records are separate contributions at this source pin.
Full compaction uses session scope. The scanner's cumulative-summary assumption conflicts with this source.
This corrects the assumption in the earlier [format baseline](2026-11-08-l1f-formats-b.md#kimi)
for the current producer pin. The earlier recorded outputs remain evidence.

The current [token contract](https://github.com/MoonshotAI/kimi-code/blob/482936afceaec4d513aa28f7b7e6b461e35e105f/packages/agent-core-v2/src/human/llm-requester/usage.ts#L1)
keeps cache reads optional. A missing cache read leaves the total unavailable.
The [usage service](https://github.com/MoonshotAI/kimi-code/blob/482936afceaec4d513aa28f7b7e6b461e35e105f/packages/agent-core-v2/src/session/usage/sessionUsageService.ts#L19)
also saves an empty usage object when no measurement exists.
Its availability notification does not become part of that usage record.
Zero-looking records therefore do not prove measured zero.

Current [forks](https://github.com/MoonshotAI/kimi-code/blob/482936afceaec4d513aa28f7b7e6b461e35e105f/packages/agent-core-v2/src/workspace/sessionLifecycle/sessionLifecycleService.ts#L716)
copy retained wires and save ancestry in session state.
Any rule that combines copies needs qualified ancestry and prefix handling.
Compaction, auxiliary calls, migrations, and all historical formats remain outside the executed fixture scope.

The pinned [scanner parser](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/kimi/src/parser.rs)
uses folder session ID, message ID when present, timestamp, model, and counters
as its duplicate key. The new format has no message ID in that key.
The parser supplies missing counters and times, then drops zero usage.
The [loader](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/kimi/src/loader.rs)
and [path resolver](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/kimi/src/paths.rs)
discard read failures and missing roots.

A candidate needs a bounded capture and a separate contract for each producer format.
It must preserve distinct usage and suppress proven inherited copies.
Unresolved identities, incomplete counts, and conflicting copies must remain unavailable.
