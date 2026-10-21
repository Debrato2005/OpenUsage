# Security

OpenUsage is in development. There is no supported production release yet. Linux execution has recorded, limited acceptance evidence; Windows/macOS, signing, complete session renewal/revocation, and the full SRS acceptance matrix remain incomplete. A source snapshot is not a security certification.

## Report a concern

Use the repository's **Security → Report a vulnerability** option if private reporting is enabled. Otherwise, open an issue asking the maintainer for a private reporting channel, without exploit details, account information, or secrets. Do not post passwords, authorization codes, OAuth tokens, credential files, database keys, private backups, or unredacted diagnostics in a public issue.

Describe the affected commit, operating system, expected and observed behavior, and a minimal reproduction with synthetic data. Keep any sensitive material local until a private channel is agreed. No response-time guarantee is offered for this unreleased project.

## Security boundaries

- Rust owns provider processes, credentials, encryption keys, persistence and exact usage arithmetic. The renderer receives narrow typed data.
- Production storage requires SQLCipher 4 with WAL. A working OS credential store is preferred; an explicit protected fallback is required when it is unavailable. Provider OS credential storage is still incomplete.
- Accounts use separate provider-managed authorization homes. Never copy credentials between accounts or read an unrelated Codex home.
- Only validated official provider interfaces and pinned executables are permitted. Read-only usage collection must respect cooldowns and provider backoff. OpenUsage does not promise immunity from provider enforcement.
- Missing measurements remain unavailable. Email is not verified identity, elapsed time does not confirm a reset, and provisional identities must not be combined into totals.
- Backups require authenticated encryption; restore requires explicit consent. Secrets must stay out of logs, exports, screenshots and public test evidence.

## Known dependency qualification

The locked Linux GTK chain includes `glib 0.18.5`, affected by [RUSTSEC-2024-0429](https://rustsec.org/advisories/RUSTSEC-2024-0429.html). The inspected OpenUsage and dependency sources had no caller of the affected `Variant::array_iter_str` entry point. That scoped result is not a fix or proof about every binary. A compatible upstream update or reviewed backport remains a release requirement; do not suppress the advisory or replace the dependency with an incompatible version just to clear a scanner.

The locked `proc-macro-error`, `zvariant`, `zbus_names` and `zvariant_derive` versions also have maintenance notices. Maintenance notices are not, by themselves, confirmed exploits. Recheck the exact lockfiles and review upstream migrations before release.

Vulnerability scans are point-in-time checks. Native system libraries, the separate official companion, platform behavior and previously unknown vulnerabilities require their own qualification. See [implementation status](docs/implementation-status.md) for remaining product and release work.
