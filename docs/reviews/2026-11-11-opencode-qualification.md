# OpenCode qualification

L-1f11p, November 11, 2026. The pinned ccusage 20.0.48 binary ran 26 private
synthetic cases on Linux. Twelve matched the required observations. Fourteen
failed qualification. OpenCode stays disabled.

## Source boundary

The fixture follows the pinned loader's legacy message JSON, `message` table,
and v2 `session_message` and `session_v2` tables. The v2 fork boundary names a
parent message. Its sequence number identifies the copied prefix. These are
consumer-contract fixtures. They do not establish producer or real-log coverage.
The source is ccusage `8995f3b7fd7110b8f37564f7d0468acc7a7d6b20`.
[Loader](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/opencode/src/loader.rs),
[parser](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/opencode/src/parser.rs)

## Results

Each comparison runs both reports with an empty private home, explicit source
roots, fixed offline arguments, and the checked production digest. A populated
Pi source stays outside the selection. Source hashes remain unchanged.

| Case | Required observation | Pinned report |
|---|---|---|
| JSON or database, copy, replay, or both formats | 173 tokens | 173 |
| Two distinct message IDs | 346 tokens | 346 |
| Summary with usage | 222 tokens | 222 |
| v2 fork through a resolved boundary, with a new call | 190 tokens | 190. Parent keeps 173 and fork keeps 17. |
| Conflicting JSON copies, both root orders | Unavailable | 173 or 190, depending on root order |
| Conflicting database and JSON copies | Unavailable | 173. The database wins silently. |
| Copied records without message IDs | Unavailable | 346 |
| Missing timestamp | 173 in the unplaced scope, with time unavailable | 173. The scanner's January 1, 1970 date stays outside OpenUsage metadata. |
| Missing model or provider | 173, with unavailable attribution | No day or session rows |
| Missing independent total and incomplete categories | Unavailable | 120 |
| Reported zero | One zero observation | No day or session rows |
| Malformed JSON, invalid UTF-8, or malformed database row | Unavailable | Successful partial report of 173 |
| Invalid database or missing explicit root beside valid input | Unavailable | Successful partial report of 173 |
| Fork with an unresolved boundary | Unavailable | 363, including its copied prefix |
| Empty source | No observation | No rows |

The [test](../../tests/tools/test_ccusage_opencode.py) keeps independent token
expectations separate from observed reports. A null expectation requires an
unavailable result. It never means zero. The [record](../../crates/openusage-locallogs/tests/fixtures/remaining-tools/opencode-qualification.json)
keeps day and session totals and a digest of each complete report.

## Required correction

The loader ignores some parse, directory, and database failures. It suppresses
duplicate IDs before it checks whether their source records agree. The parser
also drops measured zeros and usage without model or provider labels. These
rules need a strict source capture before enablement. Copies require source evidence.
Equal totals alone cannot establish identity. Any correction must preserve the
valid JSON, database, replay, summary, and v2 fork results above.

OpenUsage's catalog selects an unplaced period for OpenCode and discards the
scanner's activity metadata under SC-022. An absent time can keep its reliable
total in that scope. The app's grouped runner rejects OpenCode through the catalog gate.
Producer execution, concurrent database writes, scale, native OpenCode journeys,
real logs, Windows, and macOS remain unqualified.
