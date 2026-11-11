# Copilot usage qualification

L-1f11ag, November 12, 2026. The pinned scanner counts renamed Copilot copies
twice and accepts incomplete input. Copilot stays disabled.

## Executed reports

The [fixture](../../tests/tools/test_ccusage_copilot.py) runs the production
`ccusage` 20.0.48 binary on private synthetic Linux input.
The [record](../../tests/tools/fixtures/copilot/qualification.json) covers
48 cases and 192 repeated scanner runs. Five cases match both reports.
Nineteen session outcomes match. Bubblewrap supplies read-only selected input
and an empty private home. Repeated output and source hashes match.

Each case selects one `COPILOT_HOME`. Copies use separate paths inside that home.
The required totals use supplied synthetic call origins. No Copilot writer,
fork, import, or exporter executed.

| Scenario | Required tokens | Pinned session result |
| --- | ---: | ---: |
| One shutdown, replay, or one chat span | 120 | 120 |
| Renamed session copy | 120 | 240 |
| Inherited copy with 17 new tokens | 137 | 257 |
| Two independent sessions | 240 | 240 |
| Cumulative growth, reversed order, or replay | 137 | 137 |
| Conflicting shutdown ID | Unavailable | 137 or 120 by record order |
| New checkpoint with accounting totals | 120 | No rows |
| Linked checkpoint and archive | 120 | No rows |
| Missing or malformed required archive | Unavailable | 120 from the head |
| Copied telemetry file | 120 | 240 |
| Distinct spans with equal counts | 240 | 240 |
| Conflicting telemetry identity | Unavailable | 257 |
| Malformed selected peer | Unavailable | 120 from valid input |

The shutdown total includes cache tokens in input. Reasoning is a subset of
output. The base input of 100 and output of 20 therefore give 120 tokens.
The scanner reports 85 uncached input, 10 cache reads, and 5 cache writes.
Missing cache detail becomes numeric zero. Negative, fractional, and text
counts produce successful reports. Missing time discards shutdown usage;
telemetry without time receives the fixed file date. An unpriced model gets
numeric zero cost. These results fail the relevant unavailable-state checks.

A synthetic zero shutdown keeps one zero row. A zero chat span disappears.
Empty files and header-only input contain no observations. A cumulative
shutdown cannot place its contributing calls on individual days, so its dated
distribution must remain unavailable. The fixture keeps this separate from
its session total under [SC-022](../srs-clarifications.md#sc-022-incomplete-metrics-from-the-other-local-tools).

The overlap controls distinguish a span covered by a shutdown from a later new
call. They return 120 and 240 tokens. A partial telemetry sample with a
137-token shutdown returns 137. These controls do not prove complete coverage
for arbitrary traces, sub-agents, or metered tools.

## Source contract and next gate

The audit checks forty-five downloaded files against their digests and official
Git blobs. The released SDK
[v1.0.31](https://github.com/github/copilot-sdk/tree/15de061c7c871e34698a74e353e9afbf0250a1a3)
pins CLI 1.0.109. Its event schema matches the inspected development schema.
The development source pins CLI 1.0.110-1. These are distinct version scopes.

The [released schema](https://github.com/github/copilot-sdk/blob/15de061c7c871e34698a74e353e9afbf0250a1a3/nodejs/src/generated/session-events.ts#L4454-L4537)
allows accounting snapshots, provider/model totals, and source-session receipt
watermarks in usage checkpoints. The scanner reads checkpoint credit totals
but ignores these token snapshots. Ephemeral `assistant.usage` events do not
establish complete persisted call history.

The [journal contract](https://github.com/github/copilot-sdk/blob/15de061c7c871e34698a74e353e9afbf0250a1a3/docs/features/session-persistence.md#L186-L221)
allows numbered archives and a replacement head with checkpoint prefixes.
History reads require predecessor archives. The pinned
[discovery code](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/copilot/src/paths.rs)
reads only each immediate session directory's `events.jsonl`.
The fixture exercises compatible record shapes; it does not reproduce runtime
rotation or establish every raw telemetry envelope.

The public CLI repository does not include the inspected writer implementation.
SDK types and documentation therefore do not prove fork identity, receipt
replay, crash recovery, or cumulative transitions. Equal counts and timestamps
cannot establish copied origin. Reported credits also remain separate from
OpenUsage estimates at API prices.

A candidate needs bounded capture of every required source, strict records,
qualified origin and overlap rules, and checked counts. It must preserve absent
metrics and refuse unsupported journals or accounting states. Writer runtime,
concurrent capture, scale, grouped storage, and application gates remain open.
