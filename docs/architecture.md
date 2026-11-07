# Architecture

OpenUsage is a Tauri 2 desktop app. Rust owns everything sensitive. React only displays typed results. [SRS Section 2](SRS.md) fixes the technology.

## Data path

All data follows one path (OU-DES-009).

| Step | Job |
|---|---|
| Adapter | Reads one source |
| Checks | Validate format, range, and identity |
| Reading | One typed result with a state and a status |
| Store | SQLCipher database with one write coordinator |
| View | React, through typed commands |

Today Codex is the only source, and its path has no general adapter layer. The planned `openusage-sources` crate adds that layer without adding a second path.

## Code map

| Path | Owns |
|---|---|
| `apps/desktop/src/` | React UI with three pages: Overview, Usage, Settings |
| `apps/desktop/src-tauri/src/` | Typed commands, monitoring, local log consent and scans, notifications, key handling, provider control, export, backup, restore |
| `crates/openusage-codex/` | Codex protocol, response parsing, approved executable checks, owned child processes |
| `crates/openusage-storage/` | SQLCipher 4 with WAL, migrations, a read-only open for the CLI, quota, activity, alerts, subscriptions, local log totals, exports |
| `crates/openusage-import/` | Encrypted backup archive format |
| `crates/openusage-locallogs/` | Pinned `ccusage` binary checks, folder detection, the hardened runner, and the strict report parser for local logs |
| `crates/openusage-cli/` | The `openusage` command, with no webview dependency |
| `crates/openusage-keys/` | The database key in the OS credential store, for the app and the CLI |
| `tests/contract/` | IPC contract checks |
| `tests/renderer/` | Playwright in Chromium and WebKit with synthetic IPC |
| `tests/native/` | Journeys against the real app |
| `tests/tools/` | Script tests |
| `scripts/` | Verification, storage cleanup, Git safety, companion preparation |
| `vendor/glib-0.18.5/` | Local security backport, explained in [SECURITY.md](../SECURITY.md) |

## Boundaries

- **Secrets:** provider sessions, credentials, and the database key stay in Rust, provider-managed homes, or the OS credential store. They never reach React.
- **Processes:** the Rust supervisor owns every child process. At most two collections run at once.
- **Numbers:** Rust does exact arithmetic. Token counts cross IPC as decimal strings. A missing value stays missing.
- **Renderer:** React calls only registered typed commands. It has no direct file or process access.
- **Network:** OpenUsage declares no HTTP client in Rust. The official Codex child process makes provider requests.
- **Storage:** one service coordinates all writes. Current state and history are separate.

## Planned

| Piece | Location | SRS |
|---|---|---|
| Source adapters and evidence | `crates/openusage-sources/` | 5A, 5B, 5D |
| Local logs through bundled `ccusage` | An adapter in the same crate | 5C |
| `openusage` CLI | `crates/openusage-cli/` | 11A |
| One-command install | npm packages | 16A |
| Opt-in leaderboard | `services/leaderboard/` | 14A |

Adding a tool shall take one adapter module, its fixtures, and its declaration (OU-DES-011).
