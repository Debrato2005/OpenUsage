# Threat model

This is the release threat model that SRS Section 14.1 asks for. Each row names the threat, the control in the code, and the gap that stays open. [Status](implementation-status.md) and [traceability](acceptance-traceability.md) hold the executed evidence.

## Out of scope

OpenUsage does not protect against an attacker with full control of your unlocked operating-system account. Such an attacker can read your files, use your OS credential store, and run code as you. OpenUsage still uses the access controls of the OS: private folder and file modes, the OS credential store, and no listening network port.

## Assets

- Provider sessions and credentials, in provider-managed homes inside the workspace.
- The database key, in the OS credential store or wrapped with your password.
- The encrypted database: accounts, quota and activity history, local log totals, sessions and hours. A Claude Code project folder name encodes a path on your device.
- Backup archives and exports that you save.

## Threats and controls

| Threat | Controls | Open gaps |
|---|---|---|
| Malicious local files | Rust opens workspace files with mode 0600 in a 0700 folder and refuses symbolic links (`workspace.rs`, `openusage-storage`). Restore checks the authentication tag of each archive chunk, with limits of 5 GiB and 10,000 entries (`openusage-import`). Local data deletion proves ownership and path identity first, and refuses on a platform where it cannot. | Windows has no ownership proof yet, so deletion there is not available. |
| Compromised browser content | React gets only registered typed commands and no file or process access. The content security policy allows only scripts from the app, no network connections, and `blob:` images for share previews. Rust parses every command argument into strict types. Secrets never reach React. Provider web pages never become commands (OU-SEC-020). | None known in the code. Native acceptance runs on Linux only. |
| Malicious path links | The app opens only the fixed Codex sign-in page `https://auth.openai.com/codex/device`, after Rust compares the exact string. Log text never becomes a path to open (OU-SEC-021). | The SRS rule to show an unrecognized link before it opens (OU-SEC-019) has no other link to apply to yet. |
| Poisoned JSON-RPC data | The Codex client in `openusage-codex` limits a message to 1 MiB and a response to 4 MiB. It parses each message into strict types, limits list sizes, and refuses quota integers above 2^53 − 1. A value that does not parse stays missing. | Only Codex 0.163.0 is qualified. |
| Poisoned adapter responses | Each reading passes format, range and identity checks before the store (OU-DES-009). Missing values stay missing, and time alone does not confirm a reset. | The general adapter layer is not built. Codex is the only provider source. |
| Malicious log content | `ccusage` output is parsed by a strict parser with an output limit. Only token counts, model names, dates, session IDs and times, Claude Code project folder names, the estimated value and the `ccusage` version are kept. React shows them as text, and share images hold no project name. | `ccusage` itself reads the raw logs. See the next row and [Bundled ccusage](../SECURITY.md#bundled-ccusage). |
| Substituted Codex or `ccusage` binaries | Rust checks the SHA-256 digest of the Codex companion and of `ccusage` before each start, and refuses a mismatch. The npm assembly refuses binaries off their pins. | `ccusage` runs with your rights and is not sandboxed. Only the Linux x86_64 binaries are pinned. |
| A compromised npm package | The packages have no install scripts and download nothing. `SHA256SUMS` lists every packed file. The binaries carry pinned digests. | npm provenance and a build attestation wait for the owner to publish. `scripts/sbom.py` writes the SBOM. |
| Fake leaderboard submissions | Every view says Self-reported. Joining needs a GitHub account. The Worker checks strict schemas and limits, and hides a day above 2 billion tokens until a review. The [moderation policy](../services/leaderboard/MODERATION.md) gives the review steps. | The service cannot prove any total (OU-BOARD-011). It is not deployed, and no client sends to it. |
| Stolen backup archives | Archives use XChaCha20-Poly1305 with a new random salt and nonce prefix for each archive. The key comes from your backup password through Argon2id with 64 MiB, 3 passes and 1 lane. Each 1 MiB chunk has its own tag, so a change makes the restore fail. | A weak backup password stays open to guessing offline. The app asks for at least 12 characters. |
| Lost encryption keys | The database key stays in the OS credential store, or it is wrapped with your password using Argon2id and XChaCha20-Poly1305. The app says that it cannot recover a lost backup password. | A lost key or password loses the database. An encrypted backup is the only recovery. |
| Account switching | Each account has its own provider-managed home. OpenUsage never copies credentials between accounts and never reads another Codex home. | Session renewal and revocation are not complete. |
| Duplicate identities | An email gives only provisional identity, and a missing email gives unknown identity. Provisional identities are never added into one total. | The SRS stable identity contract is not approved yet. |
| Corrupted updates | OpenUsage has no updater. All pending migrations run in one database transaction, and a database from a newer version is refused, not changed. Restore makes a checkpoint first. | The opt-in update check (OU-PLAT-009) is not built. |
| Log leakage | The app writes no log file and no telemetry. Secrets stay out of errors, exports and test evidence. The leaderboard keeps no request logs, IP addresses or GitHub tokens. | A crash report from the OS or WebKit is outside the app's control. |
| Unauthorized filesystem access | Workspace folders are 0700 and files are 0600. The renderer has no file access. `ccusage` gets an empty private home and one folder variable for each run. | `ccusage` can still read any file that you can read. The Windows folder modes are not qualified. |

## Review

Review this model before each release, and when a new source, process, network path or file format joins the app.
