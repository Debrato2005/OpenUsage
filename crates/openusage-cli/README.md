# CLI crate

The `openusage` command (SRS Section 11A). It has no webview dependency (OU-CLI-001). People, scripts and agents run the same commands and get the same data (OU-DES-012).

## Commands

| Command | Result |
|---|---|
| `openusage report [--days N] [--yes] [--json]` | Local log totals for the last N days, 30 by default. It needs no database and saves nothing |
| `openusage doctor [--json]` | One line for each check of this machine, with the fix for each failed check |
| `openusage app` | Opens the desktop app, or prints the fix when it cannot start |

`openusage`, `openusage status --json` and `openusage statusline` read saved data. They come in L-2b.

`report` detects the Claude Code and Codex folders by existence only. Before it reads a log, it asks once for consent on the terminal, or it takes `--yes` from a script (OU-CLI-003). With no terminal and no `--yes`, it stops with `consent_required`. Each folder gets one run of the pinned `ccusage` with an empty private home, as in the [local logs crate](../openusage-locallogs/README.md).

## Output

Text goes to standard output. `--json` prints versioned JSON with `schemaVersion` 1. Token counts and the estimate, in millionths of a US dollar, are decimal text. A missing estimate is `null`, never zero (OU-CLI-004). Local log totals have `sourceLevel` `local_log` and a `status` of `null`, as [SC-019](../../docs/srs-clarifications.md#sc-019-local-log-totals-in-cli-json) records.

## Failures

Each failure prints one line to standard error: the message and its code. With `--json`, it prints `{"schemaVersion":1,"error":{"code":...,"message":...}}` to standard output. The exit code is the same in both modes (OU-CLI-005).

| Exit | Code | Meaning |
|---|---|---|
| 0 | | Success |
| 1 | `doctor_found_problems` | `doctor` found a check that needs a fix |
| 2 | `usage` | An unknown command or option |
| 3 | `consent_required` | `report` needs consent, or `--yes` |
| 4 | `ccusage_unavailable` | The pinned `ccusage` is missing, changed, or not pinned for this platform |
| 5 | `scan_failed` | A `ccusage` run failed. Nothing was saved |
| 6 | `app_unavailable` | The desktop app is missing or stopped at start |

## Files

The CLI looks for `openusage-ccusage` and `openusage-desktop` beside its own executable. A debug build can name them with `OPENUSAGE_CCUSAGE` and `OPENUSAGE_DESKTOP`. The pinned digest still applies, so only the pinned `ccusage` runs.

## Tests

`cargo test -p openusage-cli` runs the commands with scripts in place of `ccusage` and the desktop app, and runs the built binary to check help, exit codes and errors.
