# Security

OpenUsage is in development. There is no supported production release yet. Linux execution has recorded, limited acceptance evidence. Windows and macOS execution, complete session renewal and revocation, and the full SRS acceptance matrix remain incomplete. A source snapshot is not a security certification.

## Report a concern

Use the repository's **Security → Report a vulnerability** option if private reporting is enabled. Otherwise, open an issue asking the maintainer for a private reporting channel, without exploit details, account information, or secrets. Do not post passwords, authorization codes, OAuth tokens, credential files, database keys, private backups, or unredacted diagnostics in a public issue.

Describe the affected commit, operating system, expected and observed behavior, and a minimal reproduction with synthetic data. Keep any sensitive material local until a private channel is agreed. No response-time guarantee is offered for this unreleased project.

## Security boundaries

- Rust owns provider processes, credentials, encryption keys, persistence and exact usage arithmetic. The renderer receives narrow typed data.
- Production storage requires SQLCipher 4 with WAL. A working OS credential store is preferred; an explicit protected fallback is required when it is unavailable. Provider OS credential storage is still incomplete.
- Accounts use separate provider-managed authorization homes. Never copy credentials between accounts or read an unrelated Codex home.
- Today OpenUsage uses only validated official provider interfaces and pinned executables. Read-only usage collection must respect cooldowns and provider backoff. OpenUsage does not promise immunity from provider enforcement.
- The [SRS](docs/SRS.md) plans two more source levels. Local logs need consent, and OpenUsage stores only usage numbers from them. Unofficial sources stay off by default and show their status.
- Browser cookies, page scraping, pasted keys and the saved credentials of other tools are never permitted.
- Missing measurements remain unavailable. Email is not verified identity, elapsed time does not confirm a reset, and provisional identities must not be combined into totals.
- Backups require authenticated encryption; restore requires explicit consent. Secrets must stay out of logs, exports, screenshots and public test evidence.

## Bundled ccusage

OpenUsage reads local logs with the bundled `ccusage` (OU-SEC-022). The `ccusage` process runs with your user rights and is not sandboxed. It can read any file that you can read. OpenUsage does not claim otherwise.

OpenUsage limits what it gives the process:

- It starts `ccusage` only for the tools that you selected.
- It checks the SHA-256 digest of the binary before each run, and refuses a binary that differs from its pin.
- It clears the environment. The process gets an empty private home, one folder variable for one tool, `NO_COLOR` and `TZ`.
- It runs `ccusage` with `--offline`, so `ccusage` does not get prices from the network.
- It stops the process after a time limit or an output limit.
- It treats the output as untrusted data and keeps only usage numbers.

These limits reduce what a fault in `ccusage` can do by accident. They are not a sandbox.

## Known dependency qualification

The Linux GTK chain requires `glib 0.18.5`. The root Cargo patch selects a [local source backport](vendor/glib-0.18.5/OPENUSAGE-PATCH.md) of the upstream fix for [RUSTSEC-2024-0429](https://rustsec.org/advisories/RUSTSEC-2024-0429.html). The source archive checksum matches the previous lockfile. Only the two upstream pointer-mutability changes differ from that archive. An optimized synthetic iterator regression reproduced the original abort and passed after the fix. The version stays 0.18.5, and no advisory suppression is added. Full dependency scanning and native desktop acceptance remain separate gates.

The locked `proc-macro-error`, `zvariant`, `zbus_names` and `zvariant_derive` versions also have maintenance notices. Maintenance notices are not, by themselves, confirmed exploits. Recheck the exact lockfiles and review upstream migrations before release.

Vulnerability scans are point-in-time checks. Native system libraries, the separate official companion, platform behavior and previously unknown vulnerabilities require their own qualification. See [implementation status](docs/implementation-status.md) for remaining product and release work.

## Dependency notices

The [baseline notices](apps/desktop/public/licenses/rust-dependencies.txt) cover 486 original registry packages. The [notification supplement](apps/desktop/public/licenses/notification-dependencies.json) covers 24 additional packages. These inventories retain the original archive and license hashes. The glib backport includes its original MIT [license](vendor/glib-0.18.5/LICENSE) and separate patch provenance. It replaces one registry source without adding a new package version. These source inventories do not identify packages linked into a released binary. Artifact-specific attribution, MPL source availability and system-library qualification remain release gates.
