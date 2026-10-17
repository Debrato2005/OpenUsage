# Implementation status

Updated: October 17, 2026. The mission remains ongoing. Current milestone: native startup verified. Next slice: secure database setup and persisted settings.

## Implemented and executed

The real Tauri 2 application compiles and launches on Linux/WSL with WebKitGTK. `NATIVE-SMOKE-001` passed against its executable. It checks actual Rust bootstrap IPC, rejected commands and arguments, unavailable provider metrics, keyboard navigation, and an unchanged synthetic Codex home. A fresh native screenshot and execution receipt are retained in `docs/evidence/native-startup/`.

The renderer build and type checks pass. `npm run check` passes all four contract checks and twelve renderer checks across Chromium and WebKit. Renderer fixtures remain separate evidence.

The SQLCipher storage core creates and reopens encrypted databases with WAL, verified durability settings, and transactional migrations. Tests cover wrong-key preservation, rollback, newer schemas, path links, and encrypted main/WAL files. The passphrase key wrapper uses Argon2id and authenticated XChaCha20-Poly1305. Native key-store wiring and persistence acceptance remain in progress.

The Codex protocol crate passes seven synthetic boundary tests. It does not launch a provider process. The backup codec passes four cryptographic tests but is not wired to snapshot selection, preview, checkpoints, or live restore.

## Verification

| Check | Executed result |
|---|---|
| `npm run check` | Passed: build/type checks, 4 contract checks, 12 renderer checks |
| `cargo test --workspace --offline --locked` | Passed: 19 Rust tests |
| `cargo build -p openusage-desktop --offline --locked` | Passed, native executable produced |
| `NATIVE-SMOKE-001` | Passed, real Linux native binary and WebKitWebDriver |
| Windows, macOS, live provider, release performance | Not run |

The native startup binary SHA-256 is `799e00d997ebb8d983672b3bc05dd92a26753528e5dddd42d752799928df020b`.

## Resolved environment issues

Approved package downloads resolved sandbox DNS failures. The lockfile now includes Vite's required Lightning CSS version and removes stale unrelated dependencies. Installation scripts stayed disabled. Rust 1.99.1 was installed in a task directory after official SHA-256 checks. The owner installed GTK/WebKit development packages.

Renderer WebKit uses a documented isolated browser runtime with verified Ubuntu libraries. Native and renderer loopback tests use a process-local GLib resolver. Product security settings remain intact.

## Decisions and next work

SQLCipher 4 with WAL remains mandatory. The owner rejected the [SQLite proposal](proposals/sqlite-wal-amendment.md). Do not revisit this decision without a reproducible release-blocking issue. The original SRS remains unchanged.

Next: connect Rust-owned OS key storage and passphrase fallback to SQLCipher setup/unlock, then prove saved theme state across a native restart. An isolated real Secret Service already passed its own synthetic restart check. That resource check does not prove product key management.

## Local history

Work is isolated in `/tmp/openusage-native-acceptance-01a14ad0`, branch `codex/native-bootstrap`, with separate agent worktrees. Verified source is copied back to the primary worktree. Its Git metadata is read-only in this executor, so local commits live in the isolated clone. Inspect its log for the latest milestone and helper commits.

No existing Codex credential homes were inspected. No remote writes occurred.
