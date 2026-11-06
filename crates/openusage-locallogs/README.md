# Local logs crate

Reads local usage totals through the pinned official `ccusage` binary (SRS Section 5C). One `ccusage` release reads Claude Code, Codex, OpenCode, Amp, Gemini CLI, GitHub Copilot CLI and other tools; the parser accepts the tools it lists in `TOOLS`.

## The pin

`ccusage` 20.0.48 ships one native binary per platform in the official `@ccusage/ccusage-<platform>` npm packages. Linux x86_64 is pinned in `sources/ccusage-20.0.48-linux-x86_64.json` and in `pinned_sha256`. Other platforms stay unsupported until their binary is pinned the same way.

`bun run prepare:ccusage` downloads the package from registry.npmjs.org, checks the archive size and SHA-256, checks that the archive holds exactly the reviewed members, and writes the executable to `apps/desktop/src-tauri/binaries/openusage-ccusage-<target>`. It never runs the binary. `--offline` uses the cached archive in `.tools`.

## A run

`scan` holds one process-wide lock, checks the binary's SHA-256, then runs `ccusage daily --json --offline --by-agent --no-color --timezone <zone>` with no shell, an empty environment apart from `HOME`, `NO_COLOR` and `TZ`, a deadline and an output limit. A mismatched digest or invalid request runs nothing.

## The report

`parse_daily_by_agent` rejects unknown fields, unknown tools, invalid dates and model names, and any day whose per-model counts do not add up to its tool, day and report totals. Token counts are exact integers. Estimated value is in millionths of a dollar, and a model without a price keeps its tokens and has no estimate.

## Tests

`cargo test -p openusage-locallogs` covers the parser and the runner with scripts. Set `OPENUSAGE_CCUSAGE_BIN` to the provisioned binary to also run the real pinned `ccusage` against synthetic Claude Code and Codex logs in `tests/fixtures/home`.
