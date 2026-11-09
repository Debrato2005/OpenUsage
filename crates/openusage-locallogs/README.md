# Local logs crate

Reads local usage totals through the pinned official `ccusage` binary (SRS Section 5C).

The shared catalog in `sources/tools.json` defines each tool's name, order, color, enabled state, folder variable, root mode, known roots, and report capabilities. Rust embeds this file at compile time. The renderer imports it at build time. Neither reads a catalog from the user's disk. Detection, consent, process arguments, and report selection use its enabled entries. A declaration alone does not enable a tool. Claude Code and Codex remain the only enabled tools. The other 16 entries await adapter and fixture checks.

## The pin

`ccusage` 20.0.48 ships one native binary per platform in the official `@ccusage/ccusage-<platform>` npm packages. Linux x86_64 is pinned in `sources/ccusage-20.0.48-linux-x86_64.json` and in `pinned_sha256`. Other platforms stay unsupported until their binary is pinned the same way.

`bun run prepare:ccusage` downloads the package from registry.npmjs.org, checks the archive size and SHA-256, checks that the archive holds exactly the reviewed members, and writes the executable to `apps/desktop/src-tauri/binaries/openusage-ccusage-<target>`. It never runs the binary. `--offline` uses the cached archive in `.tools`.

## A run

`scan` holds one process-wide lock, checks the binary's SHA-256, then runs `ccusage daily --json --offline --by-agent --no-color --timezone <zone>` with no shell, a deadline and an output limit. `scan_sessions` runs `ccusage <tool> session --json --offline --no-color --timezone <zone>` with the same checks and limits. `scan_hours` runs `ccusage blocks --json --offline --no-color -n 1 --timezone <zone>` for Claude Code, the only tool with hours enabled in the catalog. A mismatched digest or invalid request runs nothing.

Every report also receives `--mode calculate`. For the enabled tools, estimates use token prices from the pinned offline data instead of stored charges. A zero stored charge does not make the estimate zero. An unknown model keeps its tokens and has no estimate (OU-LOG-013, OU-LOG-014). This claim does not extend to the 16 tools that remain disabled. The [source review](../../docs/reviews/2026-11-08-l1f-formats-a.md) found exceptions to `--mode calculate` in some of those tools.

A desktop legacy run reads one consented folder of one tool. The CLI uses `scan_daily_roots` for one full daily report across all detected roots of that tool. Both paths hold the same lock and validate the same paths, digest, arguments, deadline, and output limit. `HOME` and the working directory are an empty private directory, so the default folders of other tools do not exist for `ccusage`, and no user `ccusage` config is found. The environment holds only `HOME`, `NO_COLOR`, `TZ` and the folder variable from the catalog: `CLAUDE_CONFIG_DIR` for Claude Code and `CODEX_HOME` for Codex. A row of another tool rejects the run.

Before any process starts, the folder must be an absolute UTF-8 path that still equals its resolved path. Commas and trailing whitespace are refused, because ccusage can interpret them as a list of roots or another root. Spaces inside the path and Unicode names remain valid. Detection resolves links before consent; a saved root that later becomes a link to another folder fails the scan. This checks the root before launch. It does not sandbox ccusage.

## The report

`parse_daily_by_agent` rejects unknown fields, unknown tools, invalid dates and model names, and any day whose per-model counts do not add up to its tool, day and report totals. Token counts are exact integers. Estimated value is in millionths of a dollar, and a model without a price keeps its tokens and has no estimate.

## The session report

`parse_sessions` reads the session report of Claude Code or Codex, because the two tools give different shapes ([SC-021](../../docs/srs-clarifications.md#sc-021-sessions-projects-hours-and-reasoning-from-the-pinned-ccusage)). A Claude Code session has its first and last activity and the project folder that Claude Code writes. A Codex session has only its last activity, and its reasoning count, which is a part of its output count. The parser rejects unknown fields, repeated session keys, a start after the end, and counts that do not add up to the model breakdowns and the report totals. A session with an unpriced model has no estimate.

## Additional session reports

`parse_agent_sessions` reads focused session JSON for the sixteen disabled tools. It keeps the reported session and report totals separate from model attribution. This parser does not start a process, save data, or enable a tool.

The catalog holds each report's metadata policy. Only Pi and ZCode retain activity times. Pi, Qwen, Grok, and ZCode retain reported projects, with Pi's fallback `unknown` omitted. Other fields remain unavailable under [SC-022](../../docs/srs-clarifications.md#sc-022-incomplete-metrics-from-the-other-local-tools). Token categories and estimates remain unavailable because the pinned report loses their presence and derivation. A model total is available only when all tokens in its enclosing session or report have model attribution. A remainder never becomes reasoning or an invented model.

The parser rejects unknown fields, invalid metadata, repeated keys, excessive counts, and totals that disagree with their rows. It accepts Qwen's null empty totals but rejects missing totals. Empty reports differ from sessions with a reported zero. All sixteen recorded fixtures and malformed-report cases exercise this boundary. Grouped storage and the process path are implemented. Duplicate qualification remains open.

## Additional period reports

`parse_agent_periods` returns independent period totals and nullable model details for the sixteen disabled tools. Amp, Pi, Kilo, and ZCode use daily reports. The other twelve tools use full session reports for one explicit unplaced period. Daily reports from unplaced tools are rejected. Session totals must never be added to period totals.

The daily parser checks the redundant day and tool fields, strict calendar dates, ordered days, model names, bounded counts, and report totals. An incomplete model breakdown leaves model totals unavailable within that scope. Token categories and estimates stay unavailable under SC-022. No remainder becomes reasoning, a model, or an activity time. Empty reports differ from a reported zero.

`UsagePeriod` distinguishes a scanner day from unplaced usage. The caller owns the pinned zone and consented scan scope. This contract does not change storage, run a scanner, or enable a tool.

## Grouped scans

`scan_grouped_with_check` prepares the sixteen candidate tools. They remain disabled. The desktop worker groups available, granted roots by tool. It checks consent and resolved paths before each report and before the save. It stores each complete group once, with its root membership. A changed selection, invalid root, failed report, or failed write preserves accepted data.

The runner sorts roots and removes repeated paths. A `comma_list` adapter receives one bounded list of validated roots. The pinned [Goose](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/goose/src/paths.rs), [Copilot](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/copilot/src/paths.rs), and [Grok](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/grok/src/paths.rs) adapters accept one root. Their `single` mode rejects a request with more than one root. These capabilities do not prove duplicate handling.

Amp, Pi, Kilo, and ZCode require daily and session reports. The other twelve tools use one session report for separate unplaced and session records. Session totals are never added to period totals. The group has one process lock, one deadline, and one combined output bound. Every process checks the binary digest and uses fixed offline arguments and an empty private home.

The first grouped day scan pins its zone. Unplaced usage has no day zone. Old source records keep their zones under SC-024. Claude Code and Codex keep their existing source views. Their required daily, session, and hourly reports now commit together. A failed later report cannot update an earlier report. Desktop, sharing, and CLI consumers read saved scopes without adding legacy history to grouped records.

## The hourly report

`parse_hours` reads one-hour blocks. Each block must start and end on a whole UTC hour, in order, with counts that add up to its total. A gap must be empty and gives no row. The parser keeps no cost, because a block does not mark an unpriced model.

## Tests

`cargo test -p openusage-locallogs` covers the parser and runner with scripts. Set `OPENUSAGE_CCUSAGE_BIN` to the provisioned binary to run the real scanner against synthetic logs in private temporary homes. Claude Code input lives in `tests/fixtures/home/.claude`. Codex input lives in `tests/fixtures/codex` and becomes a private `.codex` folder only at runtime. The `.codex` exclusion and secret hooks stay intact. `tests/fixtures/ccusage-20.0.48-daily-by-agent-claude.json` is the real binary's output for the Claude Code folder alone. `ccusage-20.0.48-session-claude.json` and `ccusage-20.0.48-session-codex.json` are its session reports for each folder, and `ccusage-20.0.48-blocks-claude.json` is its hourly report for Claude Code.

The pinned-binary test also writes zero and large stored costs into its private Claude Code fixtures. Daily and session reports must keep the recorded calculated estimates, and hourly counts must stay unchanged. The native local-log consent journey checks the saved estimates with the same stored-cost cases.

The runner test uses a marker to prove that invalid paths never start a child. The pinned-binary test places populated decoy folders beside empty literal roots with commas or trailing whitespace. It checks refusal for daily, session, and hourly runs, and exact repeated results for normal spaces, Unicode, and resolved links. The native journey also replaces a saved root with a link to an unselected folder. The scan must fail, the workspace must stay ready, and saved usage must stay unchanged.

The [remaining-tool fixtures](tests/fixtures/remaining-tools/README.md) record daily and session contracts for all 16 disabled tools. They supply a format baseline, not release acceptance.

The parsers require real calendar days, including leap-year rules. Session and hourly timestamps must be ASCII before byte slicing. Malformed Unicode returns a report error instead of a panic.
