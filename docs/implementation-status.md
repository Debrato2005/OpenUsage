# Implementation status

Updated: October 18, 2026. The mission remains ongoing. Current milestone: native profile management verified on Linux. Next slice: provider probe integration and authenticated backup inspection.

## Implemented and executed

The actual Tauri 2 application compiles and runs with WebKitGTK. Native setup protects a random database key with an unlocked OS credential store before creating the SQLCipher 4 database. A passphrase fallback wraps that key with Argon2id and authenticated XChaCha20-Poly1305 when the store is unavailable. Monitoring stays stopped.

`NATIVE-STORAGE-001` passed against the native executable and a real private GNOME Secret Service. The saved theme survived application and key-store restarts. Missing keys and missing databases showed recovery without replacement. The fallback required a passphrase after restart. A wrong passphrase preserved both database and wrapped-key hashes.

The run checked encrypted main/WAL content, rejection by unkeyed SQLite, private file modes, narrow command arguments, an isolated credential namespace, and an unchanged synthetic Codex home. Fresh screenshots and receipts are in `evidence/native-persistence/`. `NATIVE-SMOKE-001` also passed against the production configuration with an isolated session bus.

The storage core verifies SQLCipher 4, WAL, foreign keys, FULL durability, busy timeout, schema access, and transactional migrations. The Codex crate passes synthetic protocol checks but does not launch a provider process. The explicit backup action takes a consistent WAL-aware snapshot in Rust memory, encrypts it, authenticates the entire archive, and publishes it without overwriting a file. `NATIVE-BACKUP-001` passed through the actual UI. A test-side verifier authenticated the saved file, rejected wrong passwords and tampering, and read its schema and saved theme entirely in memory. Restore preview, checkpoints, and live restore remain incomplete.

The desktop now bundles the licensed Geist font, uses shared design tokens, and places accounts before backup controls. `NATIVE-VISUAL-001` passed with 46 native captures, both themes, actual desktop/narrow viewports, enlarged text, loaded-font checks, and password-error focus. Backup export passed again on the corrected UI. See `design/desktop-foundations.md` and `evidence/native-design/`.

Profile creation, renaming, archiving, and restoration now use Rust commands and SQLCipher transactions. `NATIVE-PROFILES-001` passed: ten GUI-created profiles, useful refusal of an eleventh, immutable UUIDs, private separate homes, restart, and slot reuse. File and symlink faults preserved existing records and markers. No provider login or collection occurred. See `evidence/native-profiles/`.

## Verification

| Check | Executed result |
|---|---|
| `npm run check` | Passed: build/type check, 7 contract checks, 12 renderer checks |
| `cargo test --workspace --offline` | Passed: 35 Rust tests, including 14 native service checks. |
| `cargo clippy --workspace --all-targets --offline --locked -- -D warnings` | Passed |
| Native startup | Passed with the production credential namespace and no key-store service in its private fixture |
| Native persistence | Passed again on the backup binary with a real private Secret Service |
| Native backup export | Passed again on the refined UI, with archive authentication and no plaintext disk staging |
| Native profiles | GUI lifecycle, ten-active limit, Unicode clipboard input, restart, private homes, and failure preservation passed on Linux |
| Native design | 46 captures passed font, overflow, keyboard focus, and field-error checks. Text scaling uses test-side CSS, not OS zoom. |
| Windows, macOS, live provider, release performance | Not run |

## Resolved faults

Tauri creates its webview data directory with mode 0755. Reusing that directory made the private-storage gate reject native startup. Database files now use its separate `workspace/` child with mode 0700. The diagnostic receipt is retained.

The [key-store review](reviews/2026-10-18-key-store-transport.md) rejected insecure session randomness in the first candidate library. The replacement uses OS randomness and bounded application waits. The review also fixed stale availability state and incorrect passphrase errors for database recovery faults.

## Decisions and continuation

SQLCipher 4 with WAL remains mandatory. The [SQLite proposal](proposals/sqlite-wal-amendment.md) is rejected. The original SRS remains unchanged.

Next: integrate the isolated provider probe and add authenticated backup inspection. Inspection does not establish restore preview or database content validity. Restore and full acceptance remain open. Large-history backup memory use needs separate qualification.

Local commits live in `/tmp/openusage-native-acceptance-01a14ad0`, branch `codex/native-bootstrap`. Primary Git metadata is read-only in this executor. Verified source is copied back to the primary worktree after each milestone. No existing Codex credential homes were inspected. No remote writes occurred.
