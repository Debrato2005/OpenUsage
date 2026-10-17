# Implementation status

Updated: October 18, 2026. The mission remains ongoing. Current milestone: native backup content checks verified on Linux. Next slice: encrypted checkpoints and native provider supervision.

## Implemented and executed

The actual Tauri 2 application compiles and runs with WebKitGTK. Native setup protects a random database key with an unlocked OS credential store before creating the SQLCipher 4 database. A passphrase fallback wraps that key with Argon2id and authenticated XChaCha20-Poly1305 when the store is unavailable. Monitoring stays stopped.

`NATIVE-STORAGE-001` passed against the native executable and a real private GNOME Secret Service. The saved theme survived application and key-store restarts. Missing keys and missing databases showed recovery without replacement. The fallback required a passphrase after restart. A wrong passphrase preserved both database and wrapped-key hashes.

The run checked encrypted main/WAL content, rejection by unkeyed SQLite, private file modes, narrow command arguments, an isolated credential namespace, and an unchanged synthetic Codex home. Fresh screenshots and receipts are in `evidence/native-persistence/`. `NATIVE-SMOKE-001` also passed against the production configuration with an isolated session bus.

The storage core verifies SQLCipher 4, WAL, foreign keys, FULL durability, busy timeout, schema access, and transactional migrations. The Codex crate runs an isolated provider probe and bounded checks for managed profiles. Its real Codex 0.163.0 disposable probe passed in fresh temporary homes, with no login or inference. Native provider collection remains incomplete. The explicit backup action takes a consistent WAL-aware snapshot in Rust memory, encrypts it, authenticates the entire archive, and publishes it without overwriting a file. `NATIVE-BACKUP-001` passed through the actual UI. A test-side verifier authenticated the saved file, rejected wrong passwords and tampering, and read its schema and saved theme entirely in memory. The checkpoint core is implemented. Native restore preview and live restore remain incomplete.

The desktop now bundles the licensed Geist font, uses shared design tokens, and places accounts before backup controls. `NATIVE-VISUAL-001` passed with 46 native captures, both themes, actual desktop/narrow viewports, enlarged text, loaded-font checks, and password-error focus. Backup export passed again on the corrected UI. See `design/desktop-foundations.md` and `evidence/native-design/`.

Profile creation, renaming, archiving, and restoration now use Rust commands and SQLCipher transactions. `NATIVE-PROFILES-001` passed: ten GUI-created profiles, useful refusal of an eleventh, immutable UUIDs, private separate homes, restart, and slot reuse. File and symlink faults preserved existing records and markers. No provider login or collection occurred. See `evidence/native-profiles/`.

`NATIVE-BACKUP-INSPECTION-001` passed through the actual Tauri UI. Rust lists bounded candidates from the managed backup folder and authenticates the entire selected archive before returning metadata. Wrong passwords, tampering, truncation, trailing bytes, symlinks, hardlinks, and unsafe file modes were rejected. Inspection preserved workspace and source hashes and left no staging files. The receipt explicitly leaves database contents unchecked and restore unavailable. Six native screenshots cover errors, successful inspection, and both narrow themes. See `evidence/native-backup-inspection/`.

That earlier inspection milestone now has content validation. `NATIVE-BACKUP-CONTENT-001` passed on the rebuilt application. A GUI export of three profiles, including one archived profile, returned the expected counts after validation. Authenticated non-database bytes and broken foreign keys showed failed database checks; future manifest and database versions showed unsupported contents. Metadata remained visible, with no record counts for either warning state. Nine native captures show the resulting views. Workspace and archive hashes stayed unchanged. See `evidence/native-backup-content/` and the [content review](reviews/2026-10-18-backup-content-validation.md).

Two further Rust cores passed their integrated checks. Encrypted checkpoints capture active WAL records, authenticate disk pages, and reopen with the existing protected key. They preserve existing targets and leave the live connection usable after failure. The provider supervisor limits checks to two active profiles, rejects duplicates, owns cancellation, and reports cleanup failures. Executable transport fixtures passed; native app lifecycle and live supervised profiles remain unverified. These are prerequisites for the next native slices.

## Verification

| Check | Executed result |
|---|---|
| `npm run check` | Passed: build/type check, 10 contract checks, 12 renderer checks |
| `cargo test --workspace --offline --locked` | Passed: 57 Rust tests, including 19 native service checks. |
| `cargo clippy --workspace --all-targets --offline --locked -- -D warnings` | Passed |
| Native startup | Passed with the production credential namespace and no key-store service in its private fixture |
| Native persistence | Passed again on the backup binary with a real private Secret Service |
| Native backup export | Passed again on the refined UI, with archive authentication and no plaintext disk staging |
| Native profiles | GUI lifecycle, ten-active limit, Unicode clipboard input, restart, private homes, and failure preservation passed on Linux |
| Native backup inspection | Full archive authentication, private source checks, failure preservation, narrow themes, and password/error focus passed on Linux |
| Native backup content | Valid counts, invalid records, unsupported versions, retained metadata, private staging, and source preservation passed on Linux |
| Checkpoint core | Four real SQLCipher checks passed: active WAL, original key, unsafe targets, cleanup/retry, and disk page authentication |
| Provider supervisor core | Synthetic executable checks passed for two slots, duplicate rejection, cancellation, owned shutdown, home identity, and cleanup errors |
| Native design | 46 captures passed font, overflow, keyboard focus, and field-error checks. Text scaling uses test-side CSS, not OS zoom. |
| Windows, macOS, live provider, release performance | Not run |

## Resolved faults

Tauri creates its webview data directory with mode 0755. Reusing that directory made the private-storage gate reject native startup. Database files now use its separate `workspace/` child with mode 0700. The diagnostic receipt is retained.

The [key-store review](reviews/2026-10-18-key-store-transport.md) rejected insecure session randomness in the first candidate library. The replacement uses OS randomness and bounded application waits. The review also fixed stale availability state and incorrect passphrase errors for database recovery faults.

Saving appearance disabled its select control and lost keyboard focus. The UI now restores focus after the save when that control initiated the change and focus fell to the document body. Actual pointer and keyboard changes both passed against the rebuilt native binary.

## Decisions and continuation

SQLCipher 4 with WAL remains mandatory. The [SQLite proposal](proposals/sqlite-wal-amendment.md) is rejected. The original SRS remains unchanged.

The isolated probe was rerun after the supervisor integration. Its real-process receipt matches five current source hashes in `crates/openusage-codex/tests/evidence/real-isolated-probe.json`. Synthetic supervision evidence is beside it in `supervision-core.json`. A Rust approval reference records a caller's source decision; it does not prove executable authenticity. Native approval, application shutdown hooks, and live supervision remain incomplete. Windows is unsupported by this transport; macOS remains untested.

Next: finish native restore preview with durable checkpoints, then implement confirmed replacement and reversal. Native provider approval and lifecycle integration remain separate work. Current-schema content checks run only in isolated memory and do not approve or apply a restore. Other timezone/locale settings remain unsupported until their writers and validators are implemented. Restore and full acceptance remain open. Large-history backup memory use needs separate qualification.

Local commits live in `/tmp/openusage-native-acceptance-01a14ad0`, branch `codex/native-bootstrap`. Primary Git metadata is read-only in this executor. Verified source is copied back to the primary worktree after each milestone. No existing Codex credential homes were inspected. No remote writes occurred.
