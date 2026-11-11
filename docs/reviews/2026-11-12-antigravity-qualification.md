# Antigravity usage qualification

L-1f11ai, November 12, 2026. The pinned scanner merges conflicting Antigravity
measurements into values that no record reported. Antigravity stays disabled.

## Executed reports

The [fixture](../../tests/tools/test_ccusage_antigravity.py) runs the production
`ccusage` 20.0.48 binary on private synthetic Linux databases.
The [record](../../tests/tools/fixtures/antigravity/qualification.json) covers
37 cases and 148 repeated scanner runs. Seventeen session outcomes match.
Three empty-input cases match both views. The catalog keeps usage unplaced.
Bubblewrap supplies private homes and read-only selected roots.
Repeated output and source hashes match.

| Scenario | Required tokens | Pinned session result |
| --- | ---: | ---: |
| One generation or step | 140 | 140 |
| Stable-ID copy, replay, or cross-table copy | 140 | 140 |
| Distinct response IDs with equal counts | 280 | 280 |
| Copied origin with 17 new tokens | 157 | 157 |
| Conflicting input for one ID | Unavailable | 157 |
| Two conflicting records of 157 and 160 tokens | Unavailable | 177 |
| Replay or copy without identity | 140 | 280 |
| One retry with the same or a distinct ID | 140 or 280 | 140 or 280 |
| Missing input or cache-read field | Unavailable | 40 or 130 |
| Measured zero | One zero row | No rows |
| Corrupt selected database or protobuf peer | Unavailable | 140 from valid input |

The required totals use supplied synthetic origins and measurements.
The SQLite tables and protobuf fields follow the consumer. They do not establish
a producer format. No Antigravity writer, fork, import, or retry executed.

The base compatibility input supplies 100 uncached input tokens, 10 cache reads,
5 cache writes, and 25 total output tokens. Output includes 20 visible tokens
and 5 reasoning tokens. These supplied values give 140 tokens. The scanner
merges shared identity aliases by taking category maxima. It can combine
incompatible observations into 177 tokens without refusing the conflict.

Missing time receives trajectory time or the fixed file date. Unknown models
receive numeric zero cost. Missing metrics become zero. An oversized count
saturates the total at the unsigned integer limit. These are consumer outcomes,
not qualified producer rules. Missing measurements stay unavailable under
[SC-022](../srs-clarifications.md#sc-022-incomplete-metrics-from-the-other-local-tools).

A separate WAL test runs both reports during an uncommitted update.
Each reads the committed 140 tokens. Source hashes stay unchanged.
This does not qualify arbitrary concurrent writers, checkpoints, or a shared
capture across both report commands. The checks cover captured records under
[SC-025](../srs-clarifications.md#sc-025-captured-local-records-and-producer-completion).

## Source contract and next gate

The official SDK source pin is
[`75ba22bd`](https://github.com/google-antigravity/antigravity-sdk-python/tree/75ba22bd00afae2f46393ff31f30c3f0df52982f),
version 0.1.23. Eight selected SDK files and three pinned consumer files match
their digests and Git blobs. Four extracted documentation snapshots also match
their recorded digests. The public SDK requires a separate compiled runtime.
Its checked tree does not include the local database writer or storage schema.

The public [IPC protobuf](https://github.com/google-antigravity/antigravity-sdk-python/blob/75ba22bd00afae2f46393ff31f30c3f0df52982f/google/antigravity/proto/localharness.proto#L650-L665)
uses different field numbers from the scanner's storage decoder.
The [SDK types](https://github.com/google-antigravity/antigravity-sdk-python/blob/75ba22bd00afae2f46393ff31f30c3f0df52982f/google/antigravity/types.py#L1113-L1140)
distinguish missing values from measured zero. Their cache and thinking formula
cannot qualify the separate local storage formula.
SDK usage also does not map one-to-one to execution steps. The public identity
uses trajectory and step fields. It does not establish the scanner's response,
message, or provider-message namespace.

Official [release notes](https://antigravity.google/docs/changelog/) mention
SQLite imports and WAL fixes. They do not define complete storage coverage,
identity preservation, encryption, compression, or rotation.

A candidate needs a versioned local persistence contract or an isolated pinned
producer oracle. It must establish counter and identity scope, table overlap,
retry accounting, missing metrics, and time meaning. Bounded capture must cover
all selected databases and required WAL state. Runtime, scale, grouped storage,
and application gates remain open.
