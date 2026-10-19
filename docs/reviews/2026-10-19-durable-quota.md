# Saved quota readings

October 19, 2026. Quota readings now persist in SQLCipher 4 with WAL. They load after unlock and restart without approving or executing a provider, including archived profiles. Failed refreshes and cancellation retain saved values. Rust controls freshness. Restart and restore return an unknown freshness state.

Quota and activity share one profile association. A changed sign-in or restored home requires review and explicit consent before saving new readings. Review and Keep saved readings neither execute the provider nor change storage. Confirmation consumes a short-lived, purpose-bound review and checks fresh provider evidence. The new association and reading commit together. Earlier history remains intact. Identity remains provisional.

Schema 3 adds quota snapshots and current values, with exact nullable credit counts and ordered credit details. UTC reset times are stored as text. The private migration connection disables foreign keys before its transaction and rebuilds only the collection-run table. It checks foreign keys before commit and restores enforcement before use. Failure rolls back. Cleanup failure discards the connection. Authenticated schema 1/2 archives remain supported.

Backup, restore, and restore undo preserve all twelve activity/quota tables. No database engine, credential, or key-management downgrade was made.

The [verification record](../evidence/native-durable-quota/verification.json) retains the executed evidence. The native Linux journey passed seven normal quota cases, a further keyboard-confirmed association read, and one activity seed. Eighteen independent all-column SQLCipher checks and two genuine window closes passed. Restart, profile isolation, failed refresh, keyboard cancellation, encrypted export, restore, and undo passed. Four reviewed captures cover dark desktop, narrow light views, and consent. JetBrains Mono loaded in each capture. The default build refused all four synthetic providers before execution.

Workspace tests passed 174 checks. One owned-child helper is intentionally ignored and invoked by its parent test. Strict workspace/all-target Clippy and formatting passed. `npm run check` passed the build, 34 contract checks, and 86 Chromium/WebKit renderer checks. Both desktop configurations compiled at `f4c2cb2`. The later `f7412bb` changes only the executed native test. All 199 checked files and ten assets matched their recorded hashes after execution.

These are synthetic Linux debug checks. They do not establish live OAuth, verified identity or combined totals, monitoring, reset confirmation, full credit/history interfaces, release performance, other platforms, or complete accessibility. The new default artifact has source-refusal qualification. Earlier real OS key-store receipts retain their original artifact scope. The mission remains ongoing. Next: inspect quota freshness and reset-state behavior against the SRS, then implement the next useful daily-use slice.
