# Droid snapshot qualification

L-1f11z, November 11, 2026. The pinned scanner cannot qualify Droid snapshot
identity, revision order, or usage dates. Droid stays disabled.

## Executed reports

The [fixture](../../tests/tools/test_ccusage_droid.py) runs the pinned
`ccusage` 20.0.48 binary on 28 synthetic cases in Linux.
Both report views run twice, for 112 scanner runs. Bubblewrap gives each
run a private home and read-only mounts of the selected roots.
Repeat reports and source hashes match. No real Droid data enters the fixture.

The [qualification record](../../tests/tools/fixtures/droid/qualification.json)
separates required observations from actual reports.
Eight session outcomes match. Only the empty case matches both views.
The daily cases expect unavailable when the settings counter has no proven
day allocation. This diagnostic does not change the app's unplaced policy.

| Scenario | Required session tokens | Pinned result |
| --- | ---: | ---: |
| Single snapshot, same-name copy, or exact replay | 173 | 173 |
| Copy under a new filename | 173 | 346 |
| Synthetic inherited snapshot with 17 new tokens | 190 | 363 |
| Distinct sessions with the same filename | 346 | 173 |
| Conflicting equal-time snapshots | Unavailable | 190 or 173 by path order |
| Changed lock time without a revision | Unavailable | 190 |
| Lower counter without reset evidence | Unavailable | 70 |
| Malformed, invalid UTF-8, or missing peer | Unavailable | 173 |
| Measured zero | One zero row | No rows |
| Independent total without categories | 173 with unavailable output | 173 labeled as output |

The same-name copy and distinct-session cases present identical source bytes
at identical mounted paths. They have different known origins in the fixture.
The scanner returns identical reports. A filename and equal counts cannot
resolve this ambiguity. The synthetic fork case models inherited counters.
It does not establish that the actual Droid writer copies those counters.

Negative and fractional buckets and a contradictory total also return reports.
Missing and unknown models keep token totals, but the report still returns a
numeric estimate. Unstructured sibling text can select a model.
The existing Codebuff results also repeat with the shared test helper.

## Source boundary

The pinned [parser](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/droid/src/parser.rs)
uses the settings filename as the session ID. It uses provider lock time or
file modification time as the usage date. The
[loader](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/droid/src/loader.rs)
sorts by those times and uses path order to break ties.
Neither field proves the order of usage revisions.

Factory's [Sessions API](https://docs.factory.ai/api-reference/sessions)
documents a session ID, a separate last-update time, and an optional provider
lock timestamp in settings. It does not describe the lock time as a counter revision.
The [SDK usage contract](https://docs.factory.ai/sdk/typescript#token-and-context-usage)
distinguishes current-turn results from cumulative session updates.
It does not establish that the scanner's saved settings counter follows either contract.
The [SDK fork operation](https://docs.factory.ai/sdk/typescript#fork) creates a
successor from existing conversation history. The
[CLI reference](https://docs.factory.ai/reference/cli-reference#session-tree)
also describes shared fork history. These sources do not establish how saved
counters carry inherited usage or record a reset.

A candidate needs a documented source identity and coverage for each observation.
It needs a revision or equivalent ordering rule to choose among changed snapshots.
Forks need preserved usage origins or explicit coverage of inherited counts.
Dated totals need a real usage date. Until those fields are proven, ambiguous
snapshots must remain unavailable, and measured values must stay separate from
missing values under [SC-022](../srs-clarifications.md#sc-022-incomplete-metrics-from-the-other-local-tools).

No Droid writer, SDK, provider, or installed CLI executed.
The app, production scanner pin, and catalog stay unchanged.
