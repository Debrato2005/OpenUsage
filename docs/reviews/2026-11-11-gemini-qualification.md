# Gemini usage record qualification

L-1f11ac, November 11, 2026. The pinned scanner counts a copied Gemini chat
twice. Gemini stays disabled.

## Executed reports

The [fixture](../../tests/tools/test_ccusage_gemini.py) runs the pinned
`ccusage` 20.0.48 executable on private synthetic Linux input.
The [qualification record](../../tests/tools/fixtures/gemini/qualification.json)
covers 36 cases and 144 repeated scanner runs.
Eleven cases match both report views. Twelve session outcomes match.
Bubblewrap supplies private homes and mounts only selected synthetic roots.
Repeated reports and source hashes match.

| Scenario | Required tokens | Pinned result |
| --- | ---: | ---: |
| Single JSON or JSONL record | 125 | 125 |
| Same chat in two roots | 125 | 250 |
| JSON replay or mixed JSON and JSONL copy | 125 | 250 |
| JSONL replay within one file | 125 | 125 |
| Two distinct calls with equal counts | 250 | 250 |
| Synthetic inherited message and 17 new tokens | 142 | 267 |
| Conflicting JSONL records with the same ID | Unavailable | 142 or 125 by record order |
| Deprecated checkpoint with a usage record | 125 | 0 |
| Overlapping chat and stats files | Unavailable | 250 |
| Synthetic measured zero | One zero row | No rows |
| Malformed, invalid UTF-8, or missing peer | Unavailable | 125 from valid input |

The two valid rewind and removal fixtures preserve 142 recorded tokens.
A transcript edit cannot refund an earlier model call.
The tests cover complete captured records under
[SC-025](../srs-clarifications.md#sc-025-captured-local-records-and-producer-completion).
They do not claim that future file rewrites retain every prior record.
A malformed patch returns 125 instead of refusing the capture.

Missing models drop the usage record. Missing usage time receives a
session-based day in the daily report. A total-only record puts the total
in output. Negative and fractional counts and an inconsistent total
produce successful reports. Unavailable fields must remain unavailable
under [SC-022](../srs-clarifications.md#sc-022-incomplete-metrics-from-the-other-local-tools).

## Source contract

The producer review pins Gemini CLI v0.68.0 at
[`5a5c7180`](https://github.com/google-gemini/gemini-cli/tree/5a5c7180ff3f0a02511f54b54db7f228d4982a30).
All 32 downloaded files match their recorded digests and official Git blobs.
The producer did not execute. The scanner fixtures remain synthetic.

The [usage writer](https://github.com/google-gemini/gemini-cli/blob/5a5c7180ff3f0a02511f54b54db7f228d4982a30/packages/core/src/services/chatRecordingService.ts#L1137)
maps API usage into six token fields. It defaults absent fields to zero.
It can append the same message ID again when tokens become available.
Thus an all-zero summary alone cannot prove a measured zero.
The defaulted-total fixture returns 130 inferred tokens from a zero total.
The required observation stays unavailable.

The [API contract](https://aistudio.google.com/api-reference/generate-content)
includes cached tokens in prompt tokens. Its total also includes tool prompts,
thoughts, and output. The inclusive-cache fixture returns 132 tokens correctly.
Its input bucket contains 97 non-cache and tool-prompt tokens.
This check does not establish cost coverage for every model or token category.

The [JSONL reader](https://github.com/google-gemini/gemini-cli/blob/5a5c7180ff3f0a02511f54b54db7f228d4982a30/packages/core/src/services/chatRecordingService.ts#L304)
reconstructs a live conversation. It applies rewinds, removals, patches,
same-ID replacements, and deprecated message checkpoints.
Historical usage needs a separate rule for the complete captured records.
The [legacy migration](https://github.com/google-gemini/gemini-cli/blob/5a5c7180ff3f0a02511f54b54db7f228d4982a30/packages/core/src/services/chatRecordingService.ts#L706)
copies messages and their IDs from JSON into JSONL.
The mixed-format fixture therefore tests a producer-supported overlap.
The inherited-message fixture tests synthetic lineage, not an executed fork.

The pinned [scanner loader](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/gemini/src/loader.rs)
combines the events it reads from all files. Its
[JSONL parser](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/gemini/src/parser.rs#L165)
replaces earlier same-ID observations within one file.
It has no group-wide identity check or full capture refusal.

A separate candidate needs a bounded capture of all selected roots.
It must validate record envelopes and reconcile copies by proven IDs.
Conflicting usage and overlapping stats need a defined coverage rule or refusal.
