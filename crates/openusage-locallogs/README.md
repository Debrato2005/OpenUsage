# Local logs crate

Reads local usage totals through the pinned official `ccusage` binary (SRS Section 5C). One `ccusage` release reads Claude Code, Codex, OpenCode, Amp, Gemini CLI, GitHub Copilot CLI and other tools; the parser accepts the tools it lists in `TOOLS`.

## The pin

`ccusage` 20.0.48 ships one native binary per platform in the official `@ccusage/ccusage-<platform>` npm packages. Linux x86_64 is pinned in `sources/ccusage-20.0.48-linux-x86_64.json` and in `pinned_sha256`. Other platforms stay unsupported until their binary is pinned the same way.

`bun run prepare:ccusage` downloads the package from registry.npmjs.org, checks the archive size and SHA-256, checks that the archive holds exactly the reviewed members, and writes the executable to `apps/desktop/src-tauri/binaries/openusage-ccusage-<target>`. It never runs the binary. `--offline` uses the cached archive in `.tools`.

## A run

`scan` holds one process-wide lock, checks the binary's SHA-256, then runs `ccusage daily --json --offline --by-agent --no-color --timezone <zone>` with no shell, a deadline and an output limit. `scan_sessions` runs `ccusage <tool> session --json --offline --no-color --timezone <zone>` with the same checks and limits. A mismatched digest or invalid request runs nothing.

Each run reads one consented folder of one tool. `HOME` and the working directory are an empty private directory, so the default folders of other tools do not exist for `ccusage`, and no user `ccusage` config is found. The environment holds only `HOME`, `NO_COLOR`, `TZ` and the folder variable of the tool in `FOLDER_VARIABLES`: `CLAUDE_CONFIG_DIR` for Claude Code and `CODEX_HOME` for Codex. A row of another tool rejects the run.

## The report

`parse_daily_by_agent` rejects unknown fields, unknown tools, invalid dates and model names, and any day whose per-model counts do not add up to its tool, day and report totals. Token counts are exact integers. Estimated value is in millionths of a dollar, and a model without a price keeps its tokens and has no estimate.

## The session report

`parse_sessions` reads the session report of Claude Code or Codex, because the two tools give different shapes ([SC-021](../../docs/srs-clarifications.md#sc-021-sessions-projects-hours-and-reasoning-from-the-pinned-ccusage)). A Claude Code session has its first and last activity and the project folder that Claude Code writes. A Codex session has only its last activity, and its reasoning count, which is a part of its output count. The parser rejects unknown fields, repeated session keys, a start after the end, and counts that do not add up to the model breakdowns and the report totals. A session with an unpriced model has no estimate.

## Tests

`cargo test -p openusage-locallogs` covers the parser and the runner with scripts. Set `OPENUSAGE_CCUSAGE_BIN` to the provisioned binary to also run the real pinned `ccusage` against synthetic Claude Code and Codex logs in `tests/fixtures/home`, one folder at a time. `tests/fixtures/ccusage-20.0.48-daily-by-agent-claude.json` is the real binary's output for the Claude Code folder alone, and `ccusage-20.0.48-session-claude.json` and `ccusage-20.0.48-session-codex.json` are its session reports for each folder.
