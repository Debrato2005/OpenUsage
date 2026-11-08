# L-1f source research

Date: November 8, 2026. Scope: the pinned Linux `ccusage` 20.0.48 and the current OpenUsage source. This is a research record, not an implementation plan approval.

## Result

The exact upstream source is available through read-only web requests. An archive download is not necessary to inspect its formats. Enabling the other 16 tools still requires changes beyond folder detection. The current daily parser, session parser, scan service, and renderer cannot accept every supported report.

No dependency, archive, or executable was downloaded. No credential home or real log was inspected. No new fixture or native test ran during this research.

## Provenance

The [npm package record](https://registry.npmjs.org/@ccusage%2fccusage-linux-x64/20.0.48) names `ccusage/ccusage` as its repository. Its archive URL and SHA-512 integrity match the [OpenUsage pin](../../crates/openusage-locallogs/sources/ccusage-20.0.48-linux-x86_64.json).

The [npm attestation](https://registry.npmjs.org/-/npm/v1/attestations/@ccusage%2fccusage-linux-x64@20.0.48) contains a SLSA statement for the same package digest. Its source commit is `8995f3b7fd7110b8f37564f7d0468acc7a7d6b20`. The [v20.0.48 tag](https://api.github.com/repos/ccusage/ccusage/git/ref/tags/v20.0.48) names the same commit. This research decoded the statement but did not check its signature, certificate, or transparency proof. It did not recalculate an archive or executable digest.

All upstream links below use that exact commit. Current `main` was not used as proof of the pinned behavior.

## Additional tool inputs

Paths below are relative to the user's home. Each source link gives the exact override rules. Folder detection must preserve OU-LOG-001 and open no log before consent.

| Tool | Default root or data location | Override | Input format and important scope |
|---|---|---|---|
| [OpenCode](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/opencode/src/paths.rs) | `.local/share/opencode`, or the absolute `XDG_DATA_HOME` plus `/opencode` | `OPENCODE_DATA_DIR` | JSON in `storage/message`; SQLite in `opencode.db` or a supported alternative database. [Loader](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/opencode/src/loader.rs) |
| [Amp](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/docs/guide/amp/index.md) | `.local/share/amp` | `AMP_DATA_DIR` | `threads/**/*.json` |
| [Droid](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/docs/guide/droid/index.md) | `.factory/sessions` | `DROID_SESSIONS_DIR` | `**/*.settings.json`; the override names the sessions folder |
| [Codebuff](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/codebuff/src/paths.rs) | `.config/manicode`, `.config/manicode-dev`, `.config/manicode-staging` | `CODEBUFF_DATA_DIR` | JSON named `chat-messages.json` under `projects`; the override accepts a channel root or its `projects` folder |
| [Hermes](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/docs/guide/hermes/index.md) | `.hermes/state.db` | `HERMES_HOME` | SQLite; the override names the parent of `state.db` |
| [Pi](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/docs/guide/pi/index.md) | `.pi/agent/sessions` | `PI_AGENT_DIR` | JSONL; the override names the sessions folder, despite its name |
| [Goose](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/goose/src/paths.rs) | `.local/share/goose/sessions/sessions.db`; `Library/Application Support/goose/sessions/sessions.db`; `.local/share/Block/goose/sessions/sessions.db` | `GOOSE_PATH_ROOT` | SQLite; the override instead reads `<root>/data/sessions/sessions.db` |
| [Kilo](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/docs/guide/kilo/index.md) | `.local/share/kilo/kilo.db` | `KILO_DATA_DIR` | SQLite; the override names the parent of `kilo.db` |
| [Copilot](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/copilot/src/paths.rs) | `.copilot` | `COPILOT_HOME` | `session-state/*/events.jsonl` and recursive `otel/**/*.jsonl`; a separate exporter variable can add one file |
| [Gemini](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/docs/guide/gemini/index.md) | `.gemini/tmp` | `GEMINI_DATA_DIR` | JSON and JSONL; the override names the temporary-data root |
| [Antigravity](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/antigravity/src/paths.rs) | `.gemini/antigravity`, `.gemini/antigravity-cli`, `.gemini/antigravity-ide`, `.gemini/antigravity-backup`, `.gemini/antigravity-acp`, `.config/antigravity` | `ANTIGRAVITY_DATA_DIR` | Each default root needs `conversations/`; SQLite `.db` files contain protobuf records. The override also accepts a conversation folder. [Parser](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/antigravity/src/parser.rs) |
| [Kimi](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/kimi/src/paths.rs) | `.kimi`, `.kimi-code` | `KIMI_DATA_DIR` | `sessions/<group>/<session>/wire.jsonl` or `sessions/<workspace>/<session>/agents/<agent>/wire.jsonl` |
| [Qwen](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/qwen/src/paths.rs) | `.qwen` | `QWEN_DATA_DIR` | `projects/<project>/chats/*.jsonl` |
| [OpenClaw](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/docs/guide/openclaw/index.md) | `.openclaw`, `.clawdbot`, `.moltbot`, `.moldbot` | `OPENCLAW_DIR` | JSONL, including archived names; also `agents/<agent>/agent/openclaw-agent.sqlite` |
| [Grok](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/grok/src/paths.rs) | `.grok` | `GROK_HOME` | `sessions/**/updates.jsonl`, plus optional sibling `summary.json`; one root |
| [ZCode](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/docs/guide/zcode/index.md) | `.zcode/cli/db/db.sqlite` | `ZCODE_HOME` | SQLite `model_usage` and `session` tables; the override names `.zcode`'s replacement root |

Seven of these adapters can read SQLite. The checklist's statement about four SQLite tools does not describe this full pinned source.

## Integration limits

1. **Folder isolation needs typed rules.** [Detection](../../crates/openusage-locallogs/src/detect.rs) assumes a root and a child directory. SQLite sources need different existence checks. Goose needs a reviewed mapping because its default layout differs from its override layout. Most overrides split commas and trim whitespace. A literal folder with either property needs a refusal or another proven path strategy. Passing such a path unchanged can select unintended folders.

2. **Daily counts need a broader contract.** The [current parser](../../crates/openusage-locallogs/src/report.rs) requires four token buckets to equal every total. The [pinned core types](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/crates/ccusage-core/src/types.rs) permit additional tokens in `totalTokens`. They omit that remainder from each serialized model breakdown. They can also emit `storedCost`, which the current strict model parser rejects. The [unified report](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/crates/ccusage-adapter-all/src/report.rs) preserves reported totals. The [storage validator](../../crates/openusage-storage/src/local_logs.rs) also requires the four-bucket equality. Do not hide a remainder in output tokens or weaken total checks without a defined contract.

3. **Estimated value needs a source check.** Some adapters prefer recorded costs in the default mode, including Hermes, Kilo, OpenClaw, and Grok. OpenUsage currently calls the resulting amount an estimate at public API prices. Compare fixed `--mode calculate` output with the default before selecting a cost policy. Unknown pricing must remain unavailable. See the linked tool guides and [runner](../../crates/openusage-locallogs/src/runner.rs).

4. **Session support is separate.** Every additional tool's pinned guide documents a session command. OpenUsage's [session parser](../../crates/openusage-locallogs/src/session.rs) accepts only Claude Code and Codex. The [desktop scan service](../../apps/desktop/src-tauri/src/local_logs.rs) calls it after every daily scan. Merely adding a folder would therefore save daily data and then mark that scan failed. Each new session shape needs fixtures, or an explicit capability rule that preserves missing session data as unavailable.

5. **Presentation still names two tools.** [Contracts](../../apps/desktop/src/contracts.ts), [consent](../../apps/desktop/src/local-logs.tsx), [chart colors](../../apps/desktop/src/local-usage.tsx), and [CLI labels](../../crates/openusage-cli/src/report.rs) need a shared tool definition. The backend daily parser already lists 18 tools. Storage accepts syntactically valid tool identifiers rather than that fixed list.

6. **Combined totals need duplicate coverage.** The [OpenCode guide](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/docs/guide/opencode/index.md) describes suppression of Claude-agent rows when matching Claude usage exists. OpenUsage scans each source separately. Inference: that cross-source suppression cannot see another source during these isolated runs. Keep this limit explicit until duplicate handling has evidence.

7. **Missing counts need capability evidence.** The [Goose guide](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/docs/guide/goose/index.md) says its database has no cache columns, while the report emits zeros. A generic report shape cannot prove that a zero means measured zero. The SRS rule for missing metrics still applies.

## Acceptance criteria before L-1f is complete

- Run the pinned binary on private synthetic fixtures for all 16 tools. Cover every supported storage format, nonzero reasoning, stored costs, missing prices, and session metadata.
- Prove consent isolation with another populated tool beside each fixture. Cover commas, whitespace, links, alternate roots, Goose's layout, and repeated scans.
- Preserve exact totals and missing fields through Rust parsing, storage, backup, export, typed IPC, charts, and CLI output. Record any unresolved SRS conflict.
- Run the affected security checks, full Rust workspace tests, full renderer tests in both engines, and native consent and persistence journeys.
- Update the owning status, checklist, and traceability documents only with executed evidence. Stop after the local commit and report its Linux and synthetic-data scope.

The research establishes source locations and implementation risks. It does not qualify any additional tool for OpenUsage.
