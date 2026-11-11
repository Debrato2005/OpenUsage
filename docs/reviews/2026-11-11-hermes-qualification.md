# Hermes saved-session qualification

L-1f11aa, November 11, 2026. The pinned scanner selects conflicting Hermes
session totals by root order. Hermes stays disabled.

## Executed reports

The [fixture](../../tests/tools/test_ccusage_hermes.py) uses the pinned
`ccusage` 20.0.48 executable under Bubblewrap on Linux.
The [qualification record](../../tests/tools/fixtures/hermes/qualification.json)
covers 28 synthetic SQLite cases and 112 repeated scanner runs.
Nine session outcomes match. Only the empty database matches both views.
The daily cases expect unavailable because a session start time does not
allocate cumulative usage to its actual days.

| Scenario | Required session tokens | Pinned result |
| --- | ---: | ---: |
| Single session, exact copy, or replay under the original fixture contract | 173 | 173 |
| Current producer total with reasoning inside output | 162 | 173 |
| Conflicting copies | Unavailable | 173 or 190 by root order |
| Synthetic inherited counter with 17 new tokens | 190 | 363 |
| Child with 17 tokens of local usage | 190 | 190 |
| Distinct calls with equal counts | 346 | 346 |
| Copied usage with a rewritten ID | 173 | 346 |
| Invalid database, missing column, or missing root | Unavailable | 173 from the valid peer |
| Missing model or time | 173 with unavailable estimate | No rows |
| Missing reasoning measurement | Total unavailable | 162 |
| Unknown model with recorded charge | 173 with unavailable estimate | 173 with cost 9 |

The rewritten-ID and distinct-call cases present identical source bytes.
Their known fixture origins differ. The reports match exactly.
The two child cases compare different counter coverage. A parent ID alone
cannot choose between those meanings.

A fractional token field becomes an integer. A negative or text input field
becomes zero. A zero-valued session row yields no usage row.
The scanner reports a positive stored actual cost in Calculate mode.
With null or zero actual cost, the tested priced model uses the calculated
estimate despite a positive stored estimate. The null and zero cases
remain separate in the fixture.

One separate test holds an SQLite write transaction open.
The reader reports 190 committed tokens in both views and excludes the
uncommitted update. Source hashes and private homes stay unchanged.
This test does not cover a spilled transaction or a concurrent checkpoint.

## Producer boundary

The source review pins NousResearch/hermes-agent at
[`78abaafc`](https://github.com/NousResearch/hermes-agent/tree/78abaafcccec20a546c94e8734a70e38c08a6a69).
The [usage writer](https://github.com/NousResearch/hermes-agent/blob/78abaafcccec20a546c94e8734a70e38c08a6a69/hermes_state_usage.py#L285)
can add a delta or replace the session counters. These writes do not advance
the runtime or transcript revision. Neither a counter maximum nor those
revisions can resolve the tested conflicts.
The writer also stores some auxiliary calls in per-model aggregates without
adding them to the session row. A session row therefore does not establish
complete usage coverage.

The [branch writer](https://github.com/NousResearch/hermes-agent/blob/78abaafcccec20a546c94e8734a70e38c08a6a69/hermes_state_mutation_branch.py#L75)
copies transcript rows and records the parent. The shared
[child insert](https://github.com/NousResearch/hermes-agent/blob/78abaafcccec20a546c94e8734a70e38c08a6a69/hermes_state_compression.py#L326)
omits the parent's usage counters. This reviewed branch path starts with
zero counter defaults. The inherited-counter fixture therefore tests an
ambiguity, not the behavior of that branch writer.
The [import path](https://github.com/NousResearch/hermes-agent/blob/78abaafcccec20a546c94e8734a70e38c08a6a69/hermes_state_portability.py#L789)
keeps session IDs and skips existing IDs. It can detach an imported child
when the parent is absent. An absent parent cannot prove independent usage.

The [current usage contract](https://github.com/NousResearch/hermes-agent/blob/78abaafcccec20a546c94e8734a70e38c08a6a69/agent/usage_pricing.py#L75)
sums input, output, and both cache buckets. The
[turn writer](https://github.com/NousResearch/hermes-agent/blob/78abaafcccec20a546c94e8734a70e38c08a6a69/agent/turn_usage.py#L275)
stores the corresponding buckets and the separate reasoning detail.
The pinned scanner adds reasoning to that total. A synthetic case with the
current interpretation therefore reports 173 where the source contract gives 162.

A single isolated import probe stopped at a missing YAML dependency.
It did not construct `SessionDB` or write producer data.
No installed CLI, provider, credentials, or real Hermes logs entered the test.

A candidate needs explicit usage coverage and identity for the qualified
format. It must reject conflicting copies and incomplete captures across
all selected roots. Unknown counts, dates, and estimates must remain
unavailable under [SC-022](../srs-clarifications.md#sc-022-incomplete-metrics-from-the-other-local-tools).
No tool catalog, production scanner pin, or application code changes.
