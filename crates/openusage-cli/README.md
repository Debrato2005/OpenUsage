# CLI crate

The `openusage` command (SRS Section 11A). It has no webview dependency (OU-CLI-001). People, scripts and agents run the same commands and get the same data (OU-DES-012).

## Commands

| Command | Result |
|---|---|
| `openusage` or `openusage status [--json]` | The saved quotas of each account and today's local log usage |
| `openusage statusline` | One line for tmux and shell prompts |
| `openusage report [--days N] [--yes] [--json]` | Local log totals for the last N days, 30 by default. It needs no database and saves nothing |
| `openusage board sync --dry-run --device-id UUID [--days N] [--json]` | The exact prospective leaderboard body from saved local totals, with no network |
| `openusage doctor [--json]` | One line for each check of this machine, with the fix for each failed check |
| `openusage app` | Opens the desktop app, or prints the fix when it cannot start |

`status` and `statusline` read saved data only. They open the database read-only, never prompt, never start a reading and never use the network (OU-CLI-002, OU-CLI-006). The key comes from the OS credential store through the [keys crate](../openusage-keys/README.md). A workspace with an app password is `locked`, because the CLI never asks for the password. A reset time that passed says so, and never claims a reset. With the app closed and no WAL, the read opens the file as immutable and creates no file. With a WAL, SQLite can update its shared index in `-shm`, and the database and its WAL stay unchanged ([SC-020](../../docs/srs-clarifications.md#sc-020-a-read-only-cli-and-the-wal-index)).

Quota freshness uses the saved observation time (OU-STATE-005, OU-STATE-007).
An age from zero to less than 10 minutes is `current`. At 10 minutes or more,
it is `stale`. An observation after the current clock time has `unknown`
freshness. Text and statusline show Freshness unknown. The CLI preserves the
saved quota values and observation time.

Connection state and clock freshness are independent. JSON keeps the saved
`connectionState` on the profile and clock-derived `state` on its quota.
Text and statusline show Authentication required, Unauthorized, Offline,
Unsupported, or Error when the saved profile has that state. A valid accepted
observation time appears with the connection label. Stale and Freshness unknown
marks remain separate. A missing quota has no invented values or observation time.

`report` detects the Claude Code and Codex folders by existence only. Before it reads a log, it asks once for consent on the terminal, or it takes `--yes` from a script (OU-CLI-003). With no terminal and no `--yes`, it stops with `consent_required`. Each tool gets one full daily report for all detected roots, with an empty private home. The pinned scanner receives these roots together. The CLI filters the complete report to the requested dates. It does not read sessions to invent missing daily details. See the [local logs crate](../openusage-locallogs/README.md).

When detected sources have no usage in the selected dates, text says
No local usage in this date range. With no detected source folder, the existing
no-logs message remains. Both cases keep an empty JSON `tools` array. Neither
case creates a zero measurement.

## Output

Text goes to standard output. JSON uses `schemaVersion` 2. This replaces the version 1 local totals layout. Doctor and error envelopes use the same version.

`status.local.scopes` holds separate `grouped`, `legacy`, and `unplaced` records. Each record names the tool, scope ID, date, saved time zone, and latest scan time. Each dated scope uses today in its own zone. An unplaced record has no date or zone. The CLI never adds these records together. Text and statusline label each legacy folder separately. No account identity comes from local records.

Each scope and each `report.tools` entry has a `metrics` object. It holds total, input, output, cache-write, cache-read, reasoning, and estimated value. Each measurement has `value`, `state`, `status`, `source`, and `observedAtUtc`. Values use exact decimal strings. Estimated value uses millionths of a US dollar. A known zero is `"0"` with state `current`. An absent measurement is `null` with state `not_available`. A total estimate is unavailable if any included model lacks a price.

Local measurements have `status: null` under [SC-019](../../docs/srs-clarifications.md#sc-019-local-log-totals-in-cli-json). The enclosing record has `sourceLevel: local_log`. The source names the saved `ccusage` version. Grouped measurements use their accepted record's scan time, including retained records. `lastScanAtUtc` is separate. Legacy model maxima lack a measurement time, so `observedAtUtc` stays null. The output does not substitute the latest scan time. Daily legacy reports give no reasoning count, so that value stays unavailable.

Backup and restore keep the underlying scope records unchanged. `status` reads the shared storage projection through a read-only connection. It creates no scanner process.

## Offline leaderboard preview

`openusage board sync --dry-run --device-id UUID [--days N] [--json]` prints the
exact prospective request body. The ID must be an explicit random, lowercase
version 4 UUID. It does not establish a joined identity. The command saves
nothing, reads no logs, starts no scanner, and uses no network. The workspace
opens through the same read-only key path as `status`. One read transaction
keeps the saved scopes, folders, and rows in one snapshot.

The default range is 30 days. `--days` accepts 1 through 366. Each source uses
its saved day zone. The command keeps the saved date without conversion. It
refuses a selected date that the service considers future in UTC.

Success prints compact JSON with only `deviceId` and `rows`, without an output
wrapper or added newline. Each row holds only the service aggregate fields.
The shared typed serializer is the future sync body path. The command does not
implement join, online sync, or leave. `--json` selects versioned JSON failures.

Current Claude Code and Codex history works when one folder supplies each tool,
all selected rows have complete coverage, and no grouped scope overlaps that
tool. Lower or missing scans retain model maxima. Those rows refuse preview.
Multiple folders for one tool also refuse preview because their histories can
overlap. The command never adds legacy and grouped history.

Grouped dated periods work only with complete model attribution and all four
model token categories. Missing categories never become zero. The existing
common agent parsers save missing model categories in
`local_usage_record_models.input_tokens`, `output_tokens`, `cache_write_tokens`,
and `cache_read_tokens`. Such records refuse preview. Undated periods also
refuse preview. An unknown model estimate remains `null`. Measured zero remains
`0`. No saved rows produce an empty `rows` array.

The command refuses invalid model identifiers, count limits above 10^12,
duplicate keys, more than 400 rows, or a body above 64 KiB. Failures never print
saved paths, account IDs, labels, email addresses, project names, or model text.
Select fewer days when an upload exceeds the size limits. Open the desktop app
to review incomplete or overlapping saved usage.

## Failures

Each failure prints one line to standard error: the message and its code. With `--json`, it prints `{"schemaVersion":2,"error":{"code":...,"message":...}}` to standard output. The exit code is the same in both modes (OU-CLI-005).

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
| 11 | `board_incomplete_data` | Saved local usage cannot form exact upload rows |
| 12 | `board_invalid_data` | Saved local usage violates the upload contract |
| 13 | `board_too_large` | The upload exceeds service limits |

`statusline` prints its state on standard output, such as `OpenUsage locked`, so a prompt keeps one line.

## Files

The CLI looks for `openusage-ccusage` and `openusage-desktop` beside its own executable. A debug build can name them with `OPENUSAGE_CCUSAGE` and `OPENUSAGE_DESKTOP`. The pinned digest still applies, so only the pinned `ccusage` runs.

## Tests

`cargo test -p openusage-cli` runs the commands with scripts in place of `ccusage` and the desktop app, reads a synthetic encrypted workspace, and runs the built binary to check help, exit codes and errors. The board command tests use the existing synthetic scanner report, its production parser, and a private encrypted workspace. They check exact upload rows, repeat previews, unknown estimates, empty data, pinned zones, privacy, and refusal limits. `tests/native/cli-status.mjs` reads a real key-store workspace with the `native-test` build of the CLI.
