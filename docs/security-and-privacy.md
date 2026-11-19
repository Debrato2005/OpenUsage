# Security and privacy

Updated: November 19, 2026. OpenUsage is unreleased. This reference describes current boundaries and their limits.
[SRS Section 14](SRS.md#14-security-and-privacy) is normative.
The [threat model](threat-model.md) owns threats, controls, and open gaps. [Traceability](acceptance-traceability.md) owns executed evidence.

## Data and access

| Data or action | Current boundary | Canonical reference |
|---|---|---|
| Provider sign-in | Separate managed homes for profiles. Rust owns sessions and child processes. Browser sign-in remains blocked | [Codex contract](../crates/openusage-codex/README.md), [status](implementation-status.md#blocked) |
| Database and keys | SQLCipher with WAL. The OS credential store holds the database key, with a protected passphrase fallback | [Data contracts](data-contracts.md), [keys contract](../crates/openusage-keys/README.md) |
| Local log access | Detection checks known folders. The desktop uses saved consent from Settings. `openusage report` asks on each run or accepts `--yes` and saves nothing | [Local logs contract](../crates/openusage-locallogs/README.md), [CLI reference](../crates/openusage-cli/README.md#commands) |
| Retained local usage | Counts, dates, tools, models, estimates, session identifiers/times, and Claude Code project folders. No prompts, responses, code, or tool output | [Local records](data-contracts.md#local-usage-and-subscriptions) |
| Reading evidence | Versioned redacted hashes, checks, and observation context. Historical gaps remain explicit | [Evidence contract](data-contracts.md#reading-evidence) |
| Export and backup | Explicit user actions. Selected exports control identifying fields. Backups use authenticated encryption | [Export contract](data-contracts.md#exports-backups-and-privacy), [backup codec](../crates/openusage-import/src/lib.rs) |
| Sharing and leaderboard | Share images omit account and project identifiers. Public participation uses explicit enrollment and approved upload content | [Share contract](data-contracts.md#local-share-images), [service contract](../services/leaderboard/README.md) |

Provider credential storage remains incomplete. The database key store does not establish complete provider renewal, revocation, or OS session storage.
[Implementation status](implementation-status.md) lists these remaining gates.

## Renderer and process boundary

Rust owns secrets, persistence, exact arithmetic, provider processes, and scanner processes.
The renderer receives narrow typed results through [registered commands](../apps/desktop/src-tauri/capabilities/main-window.json).
The [content security policy](../apps/desktop/src-tauri/tauri.conf.json) blocks remote scripts and remote page content.
Neither provider pages nor log text become commands or rendered markup.

The bundled `ccusage` runs with your user rights and is not sandboxed.
It can read any file that you can read. Its checked digest, fixed offline arguments, private home, and limits do not create a sandbox.
[SECURITY.md](../SECURITY.md#bundled-ccusage) owns this required disclosure.

OpenUsage has no telemetry endpoint. Provider collection uses the approved companion.
The opt-in leaderboard client uses Rust transport with declared service configuration.
Offline arguments and source review do not replace a runtime network trace.
Live GitHub, hosted service acceptance, and deployment remain open.
Private sync and the responsive website remain [Stage 3](SRS.md#14b-private-sync-stage-3) requirements, without implemented support.

## Recovery and deletion

[Install](install.md) owns data locations, removal steps, and retained exports and backups.
Uninstalling keeps user data. Profile history deletion, complete local deletion, and public leaderboard departure are separate actions.
Local deletion does not delete provider records or public leaderboard records.
Earlier exports and backups remain until their owner removes them.
Logical deletion makes no physical secure-erasure promise.

A lost encryption key or password can make stored data unrecoverable.
An attacker with full control of an unlocked OS account remains outside the threat model.
Linux synthetic tests qualify only their recorded boundaries. Windows/macOS, live accounts, and full release acceptance remain open.
Report vulnerabilities through the [security policy](../SECURITY.md#report-a-concern). Keep raw captures and secrets private.
