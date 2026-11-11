# Grok usage qualification

L-1f11ah, November 12, 2026. The pinned scanner accepts conflicting Grok IDs,
incomplete usage, and malformed peers. Grok stays disabled.

## Executed reports

The [fixture](../../tests/tools/test_ccusage_grok.py) runs the production
`ccusage` 20.0.48 binary on private synthetic Linux input.
The [record](../../tests/tools/fixtures/grok/qualification.json) covers 46 cases
and 184 repeated scanner runs. Twenty session outcomes match.
Only empty and metadata-only cases match both views. Grok's catalog policy
keeps usage unplaced: turn completion does not date each contributing call.
Bubblewrap supplies one read-only selected home and an empty private home.
Repeated output and source hashes match.

| Scenario | Required tokens | Pinned session result |
| --- | ---: | ---: |
| Single turn or replay | 120 | 120 |
| Copy with preserved event ID | 120 | 120 |
| Copied origin and 17 new tokens | 137 | 137 |
| Distinct IDs with equal counts | 240 | 240 |
| Conflicting event ID | Unavailable | 120 or 137 by record order |
| Renamed copy without an event ID | 120 | 240 |
| Parent includes a separately saved 17-token child | 120 | 137 |
| Explicit incomplete usage | Unavailable | 120 |
| Independent total without categories | 120 | No rows |
| Missing input with an independent total | 120 | 20 |
| Measured zero | One zero row | No rows |
| Malformed selected peer | Unavailable | 120 from valid input |

Required totals use supplied synthetic origins. The parent/child case supplies
known overlap. No producer fork, child run, or writer executed.
Most cases use minimal consumer-compatible records. Four added cases exercise
the current envelope, a rewritten fork, an incomplete flag, and an unsupported
method. They remain handwritten fixtures.

The base 100 input tokens include 40 cache reads and 5 cache writes.
Output is 20, including 10 reasoning tokens. The total is therefore 120.
The scanner reports 55 uncached input tokens. Missing details become zero,
cache counts clamp to input, and contradictory totals are ignored.
Negative, fractional, and text counts yield successful reports.
Missing time receives the Unix epoch. Unknown models get numeric zero cost.
Missing or unmeasured details must remain unavailable under
[SC-022](../srs-clarifications.md#sc-022-incomplete-metrics-from-the-other-local-tools).

Zero, large, and partial reported charges leave the API-price estimate unchanged
in calculate mode. This does not prove that any reported charge is complete.
The tests cover captured records under
[SC-025](../srs-clarifications.md#sc-025-captured-local-records-and-producer-completion).

## Source contract and next gate

The official source pin is
[`xai-org/grok-build` at `2bdd1d6a`](https://github.com/xai-org/grok-build/tree/2bdd1d6a6369de0e8c68132ea4539e9abd9e14a8),
with `SOURCE_REV` `559751fdcec02d413e4c57c8832ab275e4f44980`.
All twenty-three selected files match their digests and official Git blobs.
The source audit does not map this commit to CLI 1.0.0 or the observed 1.0.77
release. No runtime executed.

The [typed envelope](https://github.com/xai-org/grok-build/blob/2bdd1d6a6369de0e8c68132ea4539e9abd9e14a8/crates/codegen/xai-grok-shell/src/extensions/notification.rs#L1125-L1144)
requires `prompt_id` and `stop_reason`. Its usage is optional.
The actor resets the prompt ledger for each prompt. These turn totals differ
from the separate cumulative session-usage endpoint.
The [ledger](https://github.com/xai-org/grok-build/blob/2bdd1d6a6369de0e8c68132ea4539e9abd9e14a8/crates/codegen/xai-chat-state/src/actor/mutations.rs#L473-L492)
can fold child tokens into parent prompt totals. Distinct event IDs therefore
do not prove independent usage.

The [fork copier](https://github.com/xai-org/grok-build/blob/2bdd1d6a6369de0e8c68132ea4539e9abd9e14a8/crates/codegen/xai-grok-shell/src/session/storage/jsonl/copy.rs#L169-L197)
changes session attribution and outer write time while retaining event metadata
and usage. Event IDs use a session ID and process counter. Copied-home resumes
and reseeding still need qualification.

The [usage contract](https://github.com/xai-org/grok-build/blob/2bdd1d6a6369de0e8c68132ea4539e9abd9e14a8/crates/codegen/xai-grok-shell/src/extensions/notification.rs#L104-L178)
keeps incomplete usage and partial costs explicit. Some backends have no
cache-write signal and serialize zero. Zero cost ticks can mean unreported cost.
These values cannot establish measured zero. API-price estimates remain separate
from producer charges.

A candidate needs one bounded capture, strict envelopes, conflict refusal,
qualified origin and parent/child attribution, and checked counts. A lagging
summary alone does not invalidate a complete captured update. Failed or partial
reads remain unavailable. Release mapping, runtime, compaction, rotation,
scale, grouped storage, and application gates remain open.
