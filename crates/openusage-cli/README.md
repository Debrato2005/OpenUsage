# CLI crate

The `openusage` command (SRS Section 11A). It has no webview dependency (OU-CLI-001). People, scripts and agents run the same commands and get the same data (OU-DES-012).

## Commands

| Command | Result |
|---|---|
| `openusage` or `openusage status [--json]` | The saved quotas of each account and today's local log usage |
| `openusage statusline` | One line for tmux and shell prompts |
| `openusage report [--days N] [--yes] [--json]` | Local log totals for the last N days, 30 by default. It needs no database and saves nothing |
| `openusage doctor [--json]` | One line for each check of this machine, with the fix for each failed check |
| `openusage app` | Opens the desktop app, or prints the fix when it cannot start |

`status` and `statusline` read saved data only. They open the database read-only, never prompt, never start a reading and never use the network (OU-CLI-002, OU-CLI-006). The key comes from the OS credential store through the [keys crate](../openusage-keys/README.md). A workspace with an app password is `locked`, because the CLI never asks for the password. A quota reading older than 10 minutes is `stale` (OU-STATE-005). A reset time that passed says so, and never claims a reset. With the app closed and no WAL, the read opens the file as immutable and creates no file. With a WAL, SQLite can update its shared index in `-shm`, and the database and its WAL stay unchanged ([SC-020](../../docs/srs-clarifications.md#sc-020-a-read-only-cli-and-the-wal-index)).

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
| 7 | `locked` | The key store is locked or unavailable, the workspace uses an app password, or a deletion did not finish |
| 8 | `no_workspace` | No saved data on this device |
| 9 | `unsupported_schema` | The saved data has another schema version. Open the desktop app once |
| 10 | `read_failed` | The saved data could not be read |

`statusline` prints its state on standard output, such as `OpenUsage locked`, so a prompt keeps one line.

## Files

The CLI looks for `openusage-ccusage` and `openusage-desktop` beside its own executable. A debug build can name them with `OPENUSAGE_CCUSAGE` and `OPENUSAGE_DESKTOP`. The pinned digest still applies, so only the pinned `ccusage` runs.

## Tests

`cargo test -p openusage-cli` runs the commands with scripts in place of `ccusage` and the desktop app, reads a synthetic encrypted workspace, and runs the built binary to check help, exit codes and errors. `tests/native/cli-status.mjs` reads a real key-store workspace with the `native-test` build of the CLI.
