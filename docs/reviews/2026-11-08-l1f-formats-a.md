# L-1f pinned formats: OpenCode, Amp, Droid, Codebuff, Hermes, Pi, Goose, Kilo

Date: November 8, 2026. Source: `ccusage` 20.0.48, commit `8995f3b7fd7110b8f37564f7d0468acc7a7d6b20`.

## Decision support

Pi is the first candidate for the current session contract. It emits first and last usage times. The other seven focused reports omit those fields. Pi still needs tests for inferred token buckets, missing models, extra totals, and copied histories. No tool qualifies for release from this research alone.

OpenCode and Hermes ignore Calculate mode when a positive stored cost exists. Kilo can hide a missing price in Calculate mode. These facts block a blanket claim that their reported cost is a public API estimate. The sections below give the owning source.

All fixtures below are source-derived recipes. This research did not execute them or the pinned binary. It read public source, without packages, archives, real logs, or credentials. See [the provenance record](2026-11-08-l1f-source-research.md) for the pin and its attestation limits.

## Shared output contract

Each focused session command returns `{"sessions": [...], "totals": {...}}`. Each row has these fields:

```text
sessionId: string
inputTokens, outputTokens, cacheCreationTokens, cacheReadTokens, totalTokens: u64
totalCost: number
modelsUsed: string[]
modelBreakdowns: ModelBreakdown[]
credits?: number
messageCount?: u64
```

`credits` and `messageCount` appear when the summary contains them. Pi adds `firstActivity`, `lastActivity`, and `projectPath`, each a string or null. There is no serialized reasoning field. Daily rows use `date` in place of `sessionId`. [Shared serializer](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/crates/ccusage-core/src/agent_report.rs)

```text
ModelBreakdown:
  modelName: string
  inputTokens, outputTokens, cacheCreationTokens, cacheReadTokens: u64
  cost: number
  missingPricing?: true
  storedCost?: number
```

The core omits the internal extra count from each model. It emits neither per-model `totalTokens` nor per-model reasoning. Focused daily JSON therefore cannot recover the allocation that unified JSON lacks. A row's residual can preserve an unallocated total. It cannot prove the residual belongs to a specific model or represents reasoning. [Core types](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/crates/ccusage-core/src/types.rs)

`totals` contains the four buckets, `totalTokens`, and `totalCost`. It adds `credits` only when the sum is positive. It adds sorted, distinct `unpricedModels` only when model breakdowns contain missing prices. It does not include `messageCount`. Token sums saturate at `u64::MAX`. [Totals serializer](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/crates/ccusage-core/src/output.rs)

The seven reports that omit time fields are [OpenCode](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/opencode/src/report.rs), [Amp](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/amp/src/report.rs), [Droid](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/droid/src/report.rs), [Codebuff](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/codebuff/src/report.rs), [Hermes](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/hermes/src/report.rs), [Goose](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/goose/src/report.rs), and [Kilo](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/kilo/src/report.rs). This omission applies even where internal entries contain timestamps.

## Missing counts and total fallback

OpenCode, Amp, Droid, Codebuff, Pi, and Kilo call the same fallback. Let `B` be the four-bucket sum, `E` the explicit extra count, and `T` the supplied total.

```text
R = max(T - B - E, 0)
if output == 0: output = R
else: E = E + R
```

When `R` is zero, the buckets stay unchanged. A supplied total below the known sum does not reduce that sum. A supplied zero output can become inferred output. Output JSON does not mark that inference. Preserve this distinction in the OpenUsage contract or guard unsupported source cases. [Fallback source](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/crates/ccusage-core/src/utils.rs)

The shared JSON reader maps missing, null, negative, fractional, and string token values to zero. The report cannot distinguish them from an explicit integer zero. [Numeric reader](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/common/src/jsonl.rs)

`storedCost` appears only in Auto mode. Its absence in Calculate mode does not prove that an adapter ignored recorded cost. [Cost helper](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/crates/ccusage-core/src/cost.rs)

## Fixture recipes

Use a private synthetic home, UTC, offline pricing, and explicit source roots. Execute both focused daily and focused session commands. Use a known model and a separate unknown model. Compare Auto with Calculate before accepting cost behavior.

The recipes use `2026-11-08T03:04:05.006Z`, or `1794107045006` milliseconds. Their four buckets are input 120, output 30, cache creation 5, and cache read 7. Their sum is 162. Where explicit reasoning is 11, the expected row total is 173. These are expected token results, not executed evidence or fixed expected prices.

### Pi

Set `PI_AGENT_DIR` to `<root>/sessions`. Create `sessions/project-z/agent_session-z.jsonl` with this line:

```json
{"type":"message","timestamp":"2026-11-08T03:04:05.006Z","message":{"role":"assistant","model":"claude-sonnet-4-20250514","usage":{"input":120,"output":30,"cacheRead":7,"cacheWrite":5,"totalTokens":162,"cost":{"total":9}}}}
```

The parser derives `session-z` from the filename and `project-z` from the first component after `sessions`. It prefixes the model with `[pi] `. Calculate ignores the stored cost. However, its cost input excludes extra tokens. A line without a model still contributes tokens but has no model breakdown or missing-price marker. All-zero usage produces no entry. [Pi parser](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/pi/src/parser.rs)

The default Pi store searches the full path for a literal `sessions` component. It uses the next component as the project. Without that component, it emits `projectPath: "unknown"`. An override named `pi-sessions` therefore loses the project label despite a `project-z` subfolder. [Exact path derivation](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/pi/src/parser.rs#L561-L579)

The main implementation agent reported a pinned-binary run with this row in private `pi-sessions/project-z/agent_session-z.jsonl`. It reported daily total 162 and matching expected prices. Both activity timestamps matched the fixture, while `projectPath` was `unknown`. This research agent did not execute that run.

The [Pi report](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/pi/src/report.rs) includes session metadata. The [session accumulator](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/crates/ccusage-core/src/summary.rs) formats first and last retained usage timestamps as RFC3339 with milliseconds. For this recipe both equal the fixture timestamp. They describe usage activity, not session completion.

The loader skips derived `subagent-artifacts` transcripts. It suppresses a copied prefix only when parent lineage, fork time, and usage signatures match. It then deduplicates a composite of project, session, time, model, counts, and cost. Add valid and malformed lineage fixtures. Keep `run-*` child sessions. [Pi loader](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/pi/src/loader.rs)

### Amp

Set `AMP_DATA_DIR` to `<root>/amp`. Create `amp/threads/thread-z.json`:

```json
{"id":"thread-z","messages":[{"role":"assistant","messageId":4,"usage":{"cacheCreationInputTokens":5,"cacheReadInputTokens":7}}],"usageLedger":{"events":[{"id":"event-z","timestamp":"2026-11-08T03:04:05.006Z","model":"claude-sonnet-4-20250514","toMessageId":4,"tokens":{"input":120,"output":30,"total":162},"credits":2}]}}
```

Expected `sessionId` is `thread-z`, with `credits: 2`. Ledger events need a timestamp, model, and tokens. Cache joins require integer message IDs. A present ledger event array takes precedence even when empty. Without that array, assistant message usage supplies the same buckets through camelCase names. Its usage timestamp and model precede the message fields. Extra tokens enter output-side pricing. [Amp parser](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/amp/src/parser.rs)

The loader appends entries without deduplication. A copied thread in another configured root can count again. Cover ledger precedence, an unmatched cache join, fallback message usage, and duplicate roots. [Amp loader](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/amp/src/loader.rs)

### Droid

Set `DROID_SESSIONS_DIR` to `<root>/sessions`. Create `sessions/session-z.settings.json`:

```json
{"model":"claude-sonnet-4-20250514","providerLock":"anthropic","providerLockTimestamp":"2026-11-08T03:04:05.006Z","tokenUsage":{"inputTokens":120,"outputTokens":30,"cacheCreationTokens":5,"cacheReadTokens":7,"thinkingTokens":11,"totalTokens":173}}
```

Expected `sessionId` is `session-z`. Thinking contributes extra tokens and output-side price. The date uses `providerLockTimestamp`, or the file modification time. Missing models trigger a sibling `.jsonl` read, then a provider-based unknown model. The fallback timestamp is not proof of usage time. [Droid parser](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/droid/src/parser.rs)

The loader keeps the latest timestamp per filename-derived session ID. This represents a cumulative snapshot, not separate daily events. Test two snapshots, a missing timestamp, and missing model data. [Droid loader](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/droid/src/loader.rs)

### Codebuff

Set `CODEBUFF_DATA_DIR` to `<root>/manicode`. Create `manicode/projects/project-z/chats/chat-z/chat-messages.json`:

```json
[{"id":"message-z","role":"assistant","timestamp":"2026-11-08T03:04:05.006Z","metadata":{"model":"claude-sonnet-4-20250514","usage":{"inputTokens":120,"outputTokens":30,"cacheCreationInputTokens":5,"cacheReadInputTokens":7,"totalTokens":162,"credits":2}}}]
```

Expected `sessionId` is `manicode/project-z/chat-z`. Accepted roles include `ai`, `agent`, and `assistant`. Usage precedence is `metadata.usage`, `metadata.codebuff.usage`, then nested run-state provider usage. Fallback fills zero buckets. Dates fall back from message fields to the chat folder, then file modification time. Cost always uses calculated pricing, including extras. Credits alone can produce an entry. [Codebuff parser](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/codebuff/src/parser.rs)

The loader uses the last duplicate after sorting file paths. IDs include the derived session context. Without a message ID, the key includes the ordinal, timestamp, model, and token buckets. Cover copied files, missing dates, credits-only messages, and all three usage locations. [Codebuff loader](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/codebuff/src/loader.rs)

### Hermes

Set `HERMES_HOME` to `<root>/hermes`. Create `hermes/state.db` with this schema and row:

```sql
CREATE TABLE sessions (
  id TEXT, model TEXT, billing_provider TEXT, started_at REAL,
  message_count INTEGER, input_tokens INTEGER, output_tokens INTEGER,
  cache_read_tokens INTEGER, cache_write_tokens INTEGER, reasoning_tokens INTEGER,
  estimated_cost_usd REAL, actual_cost_usd REAL
);
INSERT INTO sessions VALUES (
  'session-z', 'claude-sonnet-4-20250514', 'anthropic', 1794107045.006,
  3, 120, 30, 7, 5, 11, 8, 9
);
```

All selected columns must exist. The loader opens the database read-only and keeps the first session ID across roots. A query failure produces no entries. [Hermes loader](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/hermes/src/loader.rs)

Expected `sessionId` is `session-z`, with `messageCount: 3` and total 173. Reasoning is additive. Dates use `started_at`, accepting seconds or milliseconds. Missing or negative tokens become zero. The parser selects actual cost before estimated cost. A positive selected cost wins in every mode. Zero selected cost triggers pricing, even if the other cost field is positive. Test null, zero, and positive costs separately. [Hermes parser](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/hermes/src/parser.rs)

### Goose

Set `GOOSE_PATH_ROOT` to `<root>/goose`. Create `goose/data/sessions/sessions.db`:

```sql
CREATE TABLE sessions (
  id TEXT, model_config_json TEXT, provider_name TEXT, created_at TEXT,
  total_tokens INTEGER, input_tokens INTEGER, output_tokens INTEGER,
  accumulated_total_tokens INTEGER, accumulated_input_tokens INTEGER,
  accumulated_output_tokens INTEGER
);
INSERT INTO sessions VALUES (
  'session-z', '{"model_name":"claude-sonnet-4-20250514"}', 'anthropic',
  '2026-11-08T03:04:05.006Z', 150, 120, 30, 161, 120, 30
);
```

All selected columns must exist. Deduplication includes the database path, so copied databases can count again. [Goose loader](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/goose/src/loader.rs)

Expected `sessionId` is `session-z`, with input 120, output 30, and total 161. Both cache fields are synthetic zeros. Positive accumulated counters precede positive base counters. Zero accumulated counters fall back to base values. A positive total remainder is extra usage and receives output pricing. A total below input plus output does not reduce those buckets. The parser accepts several date formats, including date-only input as midnight. [Goose parser](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/goose/src/parser.rs)

### Kilo

Set `KILO_DATA_DIR` to `<root>/kilo`. Create `kilo/kilo.db` with `message(id TEXT, session_id TEXT, data TEXT)`. Insert row IDs `message-z` and `session-z`. Use this JSON string as `data`:

```json
{"role":"assistant","id":"message-z","session_id":"session-z","modelID":"claude-sonnet-4-20250514","providerID":"anthropic","time":{"created":1794107045006},"tokens":{"input":120,"output":30,"cache":{"read":7,"write":5},"reasoning":11,"total":173},"cost":9}
```

The loader deduplicates message IDs across roots. If the payload has no ID, its fallback includes database path and row ID. [Kilo loader](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/kilo/src/loader.rs)

The role, model, token block, and positive numeric timestamp are required. Timestamps accept seconds or milliseconds. The payload uses `session_id`, unlike OpenCode's `sessionID`. Reasoning contributes extra tokens. Calculate ignores the stored cost and prices extras as output. However, a positive stored cost suppresses missing-price detection in Calculate mode. An unknown model can thus produce zero without `missingPricing`. This needs an explicit fixture and cost handling. [Kilo parser](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/kilo/src/parser.rs)

### OpenCode

Set `OPENCODE_DATA_DIR` to `<root>/opencode`. For legacy JSON, create `opencode/storage/message/session-z/message-z.json`:

```json
{"id":"message-z","sessionID":"session-z","modelID":"claude-sonnet-4-20250514","providerID":"anthropic","time":{"created":1794107045006},"tokens":{"input":120,"output":30,"cache":{"read":7,"write":5},"reasoning":11,"total":173},"cost":9}
```

Model, provider, and tokens are required. Missing timestamps become the Unix epoch. Legacy timestamps use milliseconds only. Positive recorded cost wins even in Calculate mode. Reasoning remains extra usage. [OpenCode parser](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/opencode/src/parser.rs)

SQLite fixtures need these separate cases:

| Case | Schema and payload |
|---|---|
| Legacy message | `message(id TEXT, session_id TEXT, data TEXT, time_created INTEGER)`. Use the legacy JSON in `data`. Also test the schema without `time_created`. |
| V2 message | `session_message(id TEXT, session_id TEXT, type TEXT, data TEXT, time_created INTEGER, seq INTEGER)`. Use `type='assistant'`. Replace model fields with `model:{"id":"claude-sonnet-4-20250514","providerID":"anthropic"}`. Also test absent time and sequence columns. |
| Session aggregate | `session_v2` or `session` with `id`, `time_created`, `cost`, `tokens_input`, `tokens_output`, `tokens_cache_read`, and `tokens_cache_write`. Add optional `tokens_reasoning` and JSON `model`. |

The loader prefers `opencode.db`, then the first sorted supported `opencode-*.db`. It reads one database per root. Database message IDs precede duplicate JSON IDs. Aggregate fallback applies only to unbounded session reports without message usage for that session. The `session` aggregate also requires a `session_message` table. Daily reports exclude those cumulative aggregates. Fork-copy suppression requires sequence and fork metadata. Add legacy/V2 overlap, fork chains, unresolved forks, and aggregate-only fixtures. [OpenCode loader](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/opencode/src/loader.rs)

Native Claude links occur under `content[].native.sessionID` for `claude-agent` V2 messages. Separate isolated tool runs cannot use another source to suppress those duplicates. This is an integration inference, not evidence that the pinned binary miscounts a combined run. [OpenCode source guide](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/docs/guide/opencode/index.md)

## Path and consent boundaries

| Tool | Override semantics that the runner must preserve |
|---|---|
| [Pi paths](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/pi/src/paths.rs) | Sessions folder. `--pi-path` precedes `PI_AGENT_DIR`. Both split commas, trim whitespace, and do not expand `~`. Empty values fall back to the private home. |
| [Amp paths](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/amp/src/paths.rs) | Root above `threads`. Comma-separated roots. |
| [Droid paths](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/droid/src/paths.rs) | Sessions folder. Comma-separated roots. Model fallback reads the settings file's sibling JSONL. |
| [Codebuff paths](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/codebuff/src/paths.rs) | Channel root or folder whose final name is exactly `projects`. A different folder name changes discovery and session identity. |
| [Hermes paths](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/hermes/src/paths.rs) | Root above `state.db`. Comma-separated roots. |
| [Goose paths](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/goose/src/paths.rs) | One root, without comma splitting. It appends `data/sessions/sessions.db`. Default homes use a different layout. Paths undergo canonicalization. |
| [Kilo paths](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/kilo/src/paths.rs) | Root above `kilo.db`. Comma-separated roots. |
| [OpenCode paths](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/opencode/src/paths.rs) | Comma-separated roots. Default honors absolute `XDG_DATA_HOME`. Non-UTF8 overrides use a separate single-path branch. |

Test commas, surrounding whitespace, links, aliases, and empty overrides against a populated sentinel source outside consent. A database symlink needs separate checks from recursive JSON discovery. A Goose alias must preserve its database and SQLite journal behavior. No fixture may fall back to the real home.

## Qualification order and acceptance

1. Qualify Pi first for complete known-bucket fixtures and usage activity times. Preserve missing models and inferred buckets as distinct states.
2. Add a contract for unavailable session times and exact unallocated totals before enabling the seven other reports.
3. Qualify Amp next for event dates. Then assess Codebuff's date fallbacks and Droid's cumulative snapshots. Treat Goose cache data as unavailable.
4. Resolve OpenCode and Hermes recorded-cost precedence and Kilo missing-price suppression before claiming public API estimates.
5. Execute the pinned binary on each recipe and its edge cases. Then check Rust parsing, storage, repeat scans, typed IPC, UI, CLI, and consent isolation.

Acceptance must include multiple models with extra tokens, unknown pricing, malformed and absent numeric fields, all-zero data, duplicate sources, and bounded dates. A missing database column can look like an empty source. Source presence therefore does not prove a successful parse. Keep unsupported or partial results explicit.

This note recommends implementation tests. It adds no new product requirement and reports no release acceptance result.
