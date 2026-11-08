# L-1f pinned formats: group B

Date: November 8, 2026. Scope: Copilot, Gemini, Antigravity, Kimi, Qwen, OpenClaw, Grok, and ZCode.

This note uses ccusage 20.0.48 source commit `8995f3b7fd7110b8f37564f7d0468acc7a7d6b20`. The [source research](2026-11-08-l1f-source-research.md) owns package provenance. This note adds format detail.

## Result and limits

Start with a Grok synthetic fixture to exercise the common session shape. Follow with ZCode to cover SQLite and required source timestamps. This order is an engineering recommendation, not qualification.

No adapter in this group preserves every source field through the pinned JSON. Missing token fields often become zero. Separate reasoning and per-model total fields disappear. A richer OpenUsage store cannot recover information that ccusage omits.

The research read public source and upstream tests. It ran no competitor executable, downloaded no package, and read no real logs or credentials. All fixture outputs below are predictions from source, not executed evidence. Run them against the pinned binary before enabling a tool.

## Common focused JSON

All eight focused reports call the same [JSON builder](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/crates/ccusage-core/src/agent_report.rs). A session report has this shape. Angle brackets describe values, not literal JSON.

```text
{
  "sessions": [{
    "sessionId": <string>,
    "inputTokens": <u64>,
    "outputTokens": <u64>,
    "cacheCreationTokens": <u64>,
    "cacheReadTokens": <u64>,
    "totalTokens": <u64>,
    "totalCost": <number>,
    "modelsUsed": [<string>],
    "modelBreakdowns": [{
      "modelName": <string>,
      "inputTokens": <u64>,
      "outputTokens": <u64>,
      "cacheCreationTokens": <u64>,
      "cacheReadTokens": <u64>,
      "cost": <number>
    }]
  }],
  "totals": {
    "inputTokens": <u64>,
    "outputTokens": <u64>,
    "cacheCreationTokens": <u64>,
    "cacheReadTokens": <u64>,
    "totalTokens": <u64>,
    "totalCost": <number>
  }
}
```

The builder adds row `credits` and `messageCount` only when their internal options exist. Some adapters never set them. The six metadata-enabled adapters add `firstActivity`, `lastActivity`, and `projectPath`, each a string or null.

| Adapter | Session metadata fields | Empty report totals | Source |
|---|---|---|---|
| Copilot | Present | Object | [Report](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/copilot/src/report.rs) |
| Gemini | Omitted | Object | [Report](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/gemini/src/report.rs) |
| Antigravity | Present | Object | [Report](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/antigravity/src/report.rs) |
| Kimi | Omitted | Object | [Report](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/kimi/src/report.rs) |
| Qwen | Present | Null | [Report](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/qwen/src/report.rs) |
| OpenClaw | Present | Object | [Report](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/openclaw/src/report.rs) |
| Grok | Present | Object | [Report](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/grok/src/report.rs) |
| ZCode | Present | Object | [Report](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/zcode/src/report.rs) |

Daily reports use `daily` and row `date` instead of `sessions` and `sessionId`. They omit the three metadata fields.

The [core types](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/crates/ccusage-core/src/types.rs) hide `extra_total_tokens` during serialization. Row totals include that count, but model breakdowns expose neither a total nor reasoning. Optional model fields are `missingPricing: true` and `storedCost: number`. Absence of the former means false inside ccusage, not proof of a complete real-world price.

The [totals builder](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/crates/ccusage-core/src/output.rs) adds `unpricedModels` only when nonempty. It adds aggregate `credits` only when positive. It emits no aggregate message count. Integer sums saturate.

### Hidden total fallback

The [shared fallback](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/crates/ccusage-core/src/utils.rs) computes a positive remainder from the source total. If output equals zero, it assigns that remainder to output. Otherwise it adds the remainder to hidden extra tokens.

Gemini, Kimi, Qwen, OpenClaw, and ZCode call this function. Thus a positive output field can contain an inferred remainder. It does not prove that the source measured output. A zero field cannot distinguish absence from explicit zero.

Inference: retaining a row remainder is useful, but it cannot supply each model's missing total or identify reasoning. Do not divide it among models. Do not call every remainder reasoning.

## Discovery and isolation

The [navigation table](2026-11-08-l1f-source-research.md#additional-tool-inputs) owns default locations. These details affect private fixtures and consent boundaries.

| Tool | Explicit root and fixture location | Boundary detail |
|---|---|---|
| Copilot | `COPILOT_HOME`: `otel/trace.jsonl` or `session-state/session-a/events.jsonl` | One trimmed root. A separate exporter variable can add a file. [Paths](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/copilot/src/paths.rs) |
| Gemini | `GEMINI_DATA_DIR`: `project/chats/session-a.json` | Recursive JSON and JSONL. Override roots split on commas. [Paths](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/gemini/src/paths.rs) |
| Antigravity | `ANTIGRAVITY_DATA_DIR`: `conversations/session-a.db` | Override accepts a root or conversation folder. A nested conversations folder takes priority. [Paths](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/antigravity/src/paths.rs) |
| Kimi | `KIMI_DATA_DIR`: `sessions/workspace/session-a/wire.jsonl` | Also accepts `sessions/workspace/session-a/agents/agent-a/wire.jsonl`. Discovery limits path depth. [Paths](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/kimi/src/paths.rs) |
| Qwen | `QWEN_DATA_DIR`: `projects/project-a/chats/session-a.jsonl` | Exact project/chats depth. Override roots split on commas. [Paths](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/qwen/src/paths.rs) |
| OpenClaw | `OPENCLAW_DIR`: `agents/main/sessions/session-a.jsonl` | Recursive JSONL also includes deleted/reset names. SQLite uses `agents/main/agent/openclaw-agent.sqlite`. [Paths](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/openclaw/src/paths.rs) |
| Grok | `GROK_HOME`: `sessions/project-a/session-a/updates.jsonl` | One trimmed root. It ignores `logs/unified.jsonl`. [Paths](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/grok/src/paths.rs) |
| ZCode | `ZCODE_HOME`: `cli/db/db.sqlite` | Comma-separated roots. It canonicalizes roots and database files. [Paths](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/zcode/src/paths.rs) |

Use an empty private home and a cleared environment. Set only the selected override. Keep `COPILOT_OTEL_FILE_EXPORTER_PATH` absent. Test another populated tool beside each fixture. Treat canonicalization as path resolution, not consent to a different target.

## Synthetic input recipes

All times below are explicit. A fixture with one accepted event should give equal first and last activity where the report exposes them. Each recipe needs a second event with another time to test the range.

The numeric fixture time is `2026-05-28T20:26:40.000Z`. Grok, ZCode, OpenClaw, Antigravity, and the separate Copilot span should expose this value in both activity fields. Qwen and the Copilot shutdown should expose `2026-11-04T09:00:00.000Z`. Gemini and Kimi omit those fields even with explicit source times.

### Grok

Write this line to `sessions/project-a/session-a/updates.jsonl`.

```json
{"timestamp":1780000000,"params":{"sessionId":"session-a","update":{"sessionUpdate":"turn_completed","usage":{"modelUsage":{"grok-4.5-build":{"inputTokens":100,"outputTokens":20,"cachedReadTokens":40,"cacheCreationTokens":5,"reasoningTokens":10,"totalTokens":120}}}},"_meta":{"eventId":"event-a"}}}
```

Write sibling `summary.json`:

```json
{"info":{"id":"session-a","cwd":"/synthetic/project"},"current_model_id":"grok-4.5-build"}
```

Expected buckets: input 55, output 20, creation 5, read 40, total 120. Output includes reasoning. The parser ignores source `totalTokens`. It always sets extra tokens to zero. Missing buckets become zero. Event/model identity suppresses duplicate events.

A positive `agentTimestampMs` takes priority over envelope seconds. Missing timestamps become the Unix epoch. Project metadata prefers summary cwd, then git root, then a decoded folder name. `costUsdTicks` uses units of 1/10,000,000,000 USD. Calculate mode uses model prices instead. [Parser](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/grok/src/parser.rs)

Inference: a focused session time alone does not prove a real start or end. Separately reported reasoning is unavailable.

### ZCode

Create `cli/db/db.sqlite` with this synthetic schema and row.

```sql
CREATE TABLE session (id TEXT PRIMARY KEY, directory TEXT, version TEXT);
CREATE TABLE model_usage (
  id TEXT PRIMARY KEY, session_id TEXT, started_at INTEGER, model_id TEXT,
  provider_id TEXT, status TEXT, input_tokens INTEGER, output_tokens INTEGER,
  cache_creation_input_tokens INTEGER, cache_read_input_tokens INTEGER,
  computed_total_tokens INTEGER
);
INSERT INTO session VALUES ('session-a', '/synthetic/project', 'fixture');
INSERT INTO model_usage VALUES (
  'usage-a', 'session-a', 1780000000000, 'GLM-5.3',
  'builtin:zai-coding-plan', 'completed', 100, 20, 5, 40, 130
);
```

The loader reads only completed rows. It requires the two tables and their basic columns. Missing cache columns become SQL zero. Missing computed total becomes input plus output. Invalid counters become zero. It opens SQLite read-only, sets query-only mode, and deduplicates IDs across roots. [Loader](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/zcode/src/loader.rs)

Expected buckets: 55, 20, 5, 40. Total 130 includes 10 hidden extra tokens. A positive millisecond `started_at` is mandatory. The directory comes from the session join, with `ZCode` as fallback. Z.ai cache creation uses the input price. An explicit other provider blocks automatic model pricing unless an override matches. Display mode returns zero cost. [Parser](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/zcode/src/parser.rs)

The [focused session snapshot](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/zcode/src/snapshots/ccusage_adapter_zcode__report__tests__focused_zcode_session_json.snap) confirms metadata fields and no per-model totals.

### Gemini

Write `project/chats/session-a.json`:

```json
{"sessionId":"session-a","startTime":"2026-11-04T09:00:00Z","messages":[{"id":"message-a","type":"gemini","model":"gemini-2.5-pro","timestamp":"2026-11-04T09:01:00Z","tokens":{"input":100,"output":20,"cached":10,"thoughts":5,"total":125}}]}
```

Expected buckets: input 90, output 20, creation 0, read 10, total 125. The remainder is 5. This direct-message path subtracts cached input only when the reported total fits its inclusive interpretation.

The parser also accepts JSONL and stats records. Stats handling subtracts cache overlap. Tool tokens add to input. Reasoning contributes hidden extra tokens and priced output. A total-only source can become output: an upstream test maps 654 total tokens to output 654.

Timestamps can use message time, session time, or file modification time. The focused session report omits all timestamps and project metadata. Missing fields and explicit zero share the same count. [Parser](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/gemini/src/parser.rs)

### Qwen

Write `projects/project-a/chats/session-a.jsonl`:

```json
{"type":"assistant","sessionId":"session-a","timestamp":"2026-11-04T09:00:00Z","model":"qwen3-coder","usageMetadata":{"promptTokenCount":100,"candidatesTokenCount":20,"cachedContentTokenCount":10,"thoughtsTokenCount":5,"totalTokenCount":135}}
```

Expected buckets: input 100, output 20, creation 0, read 10, total 135. Qwen does not subtract cache from prompt tokens. Thoughts become extra tokens. Calculate mode prices output plus extras.

The parser accepts only assistant records. It uses the explicit session ID or project-plus-filename fallback. Project metadata comes from the folder. Missing timestamps use file modification time. Missing models become `unknown`. Missing counters become zero. The fallback can assign a total remainder to output. [Parser](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/qwen/src/parser.rs)

### Kimi

Old layout, `sessions/workspace/session-a/wire.jsonl`:

```json
{"timestamp":1780000000,"message":{"type":"StatusUpdate","payload":{"message_id":"message-a","token_usage":{"input_other":100,"output":20,"input_cache_creation":5,"input_cache_read":10,"total":135}}}}
```

New layout, `sessions/workspace/session-b/agents/agent-a/wire.jsonl`:

```json
{"type":"usage.record","usageScope":"turn","time":1780000000000,"model":"kimi-code/kimi-k2.6","usage":{"inputOther":100,"output":20,"inputCacheCreation":5,"inputCacheRead":10}}
```

Expected buckets for each separate session: 100, 20, 5, 10, total 135. New session-wide cumulative usage records are skipped. The new format reads millisecond time. The old format reads seconds and can use the total fallback.

The old model comes from root `config.json`, with `kimi-for-coding` as fallback. Pricing for that default depends on event time. New model names lose the `kimi-code/` prefix. Missing times use file modification time. Missing counts become zero. The session ID comes from the path. Focused sessions omit time and project metadata. [Parser](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/kimi/src/parser.rs)

### OpenClaw

Write `agents/main/sessions/session-a.jsonl`:

```json
{"id":"message-a","type":"message","message":{"role":"assistant","model":"gpt-5.2","provider":"openai","timestamp":1780000000000,"usage":{"input":100,"output":20,"cacheRead":10,"cacheWrite":5,"totalTokens":145,"cost":{"total":999}}}}
```

Expected buckets: 100, 20, 5, 10, total 145. The hidden remainder is 10. The display model is `[openclaw] gpt-5.2`. Project metadata is the fixed label `OpenClaw`, not a directory.

The parser uses assistant messages and tracks earlier model-change records. Counts default to zero. It can infer output from total. JSONL timestamps accept milliseconds or RFC3339, with file time as fallback.

Calculate mode ignores stored cost, but its price calculation receives only the four displayed buckets. It does not add hidden extras to priced output. Inference: an estimate with a nonzero remainder can omit part of the reported usage. [Parser](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/openclaw/src/parser.rs)

For SQLite, create `transcript_events(session_id TEXT, seq INTEGER, event_json TEXT, created_at INTEGER)`. Put the same message JSON in `event_json`. Use `session-a`, sequence 1, and time 1780000000000. The reader orders by session and sequence, resets model state between sessions, and opens read-only. [SQLite reader](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/openclaw/src/sqlite.rs)

Test JSONL/SQLite overlap before claiming migration deduplication. The decorated model also needs the OpenUsage model-name boundary checked.

### Copilot

Write `session-state/session-a/events.jsonl`:

```json
{"type":"session.shutdown","id":"shutdown-a","timestamp":"2026-11-04T09:00:00Z","data":{"modelMetrics":{"gpt-5.4":{"usage":{"inputTokens":100,"outputTokens":20,"cacheReadTokens":10,"cacheWriteTokens":5,"reasoningTokens":7},"requests":{"count":1}}}}}
```

Expected buckets: 85, 20, 5, 10, total 120. Reasoning is a subset of output and does not survive as a separate JSON field. Session-state timestamps are required. The folder supplies the session ID.

For an independent OpenTelemetry fixture, write `otel/trace.jsonl`:

```json
{"type":"span","traceId":"trace-a","spanId":"span-a","name":"chat gpt-5.4","endTime":[1780000000,0],"attributes":{"gen_ai.operation.name":"chat","gen_ai.response.model":"gpt-5.4","gen_ai.conversation.id":"otel-a","gen_ai.usage.input_tokens":100,"gen_ai.usage.output_tokens":20}}
```

The parser supports spans and selected log records. Source identity sets record precedence. Missing counters become zero. Credits use separate nano-AIU snapshots, not request-cost dollars. [Parser](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/copilot/src/parser.rs)

The loader reconciles cumulative shutdowns with OpenTelemetry calls, then emits deltas. Test repeated shutdowns, restarts, and duplicate spans together. It sets project metadata to `GitHub Copilot CLI`. It emits `messageCount` only for a positive request count and sets `credits` to absent. Credit gaps can create an `unknown` model row. [Loader](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/copilot/src/loader.rs)

### Antigravity

Create `conversations/session-a.db` with these tables:

```sql
CREATE TABLE gen_metadata (idx INTEGER PRIMARY KEY, data BLOB NOT NULL);
CREATE TABLE steps (idx INTEGER PRIMARY KEY, metadata BLOB);
CREATE TABLE trajectory_metadata_blob (data BLOB);
```

A generation blob uses protobuf. This recipe defines unsigned varint fields as `v(field,value)` and length-delimited fields as `b(field,bytes)`.

```text
usage = v(2,100) + v(3,25) + v(4,5) + v(5,10)
      + v(9,5) + v(10,20) + b(11,"response-a")
timestamp = v(1,1780000000) + v(2,0)
generation_info = b(4,timestamp)
chat_model = b(4,usage) + b(9,generation_info) + b(19,"gemini-2.5-pro")
data = b(1,chat_model)
INSERT gen_metadata: idx=1, data=<data bytes>
```

Expected buckets: 100, 20, 5, 10, total 140. Reasoning contributes 5 hidden tokens. The filename supplies the session ID. Project metadata is fixed `Antigravity`.

The parser can merge generation records, step records, and retries. Identity fields include message, response, and provider message IDs. It prefers source timestamps, then matched identity time, trajectory time, and file time. Pricing uses full output, including reasoning. Missing protobuf fields default to zero. [Parser](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/antigravity/src/parser.rs)

The loader merges matching identities across databases. Its tests cover continuation rows, duplicate responses, and model IDs. A fixture containing only one generation row does not qualify those cases. [Loader](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/antigravity/src/loader.rs)

## Decisions for implementation

These are proposed checks, not new requirements or completed acceptance:

1. Keep unknown categories, per-model totals, reasoning, account identity, and unsupported session times unavailable.
2. Preserve reported row totals independently from displayed categories. Reject negative or inconsistent arithmetic without inventing a category.
3. Define how inferred category values affect the user-facing state before enabling the five total-fallback adapters.
4. Treat fixed project labels and fallback timestamps as source limitations. They do not prove a workspace or session boundary.
5. Run each recipe and its edge cases with the pinned binary, then run consent isolation and persistence acceptance.

The unresolved source limit is per-record presence and attribution. A future richer report or a separately reviewed metadata reader could retain it. Both need their own evidence and architecture review. Changing a JSON validator alone cannot solve that limit.
