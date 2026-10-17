# Acceptance traceability

Updated: October 18, 2026. Native startup and encrypted persistence passed on Linux. The full mandatory acceptance suite remains incomplete.

| Requirement or check | Implementation and executed evidence | Status |
|---|---|---|
| NATIVE-SMOKE-001 | Real production native binary, bootstrap and command rejection, keyboard privacy access, unavailable readings, private session bus. Receipt and screenshot in `evidence/native-secure-startup/`. | passed on Linux |
| NATIVE-STORAGE-001 | Real private Secret Service, OS-protected setup, saved theme after application/key-store restart, fallback unlock, wrong-passphrase preservation, missing-key/database recovery, encrypted WAL and private modes. See `evidence/native-persistence/`. | passed on Linux |
| OU-ARCH-004, OU-ARCH-005, OU-SEC-006 through OU-SEC-009 | Narrow typed commands, argument rejection, main-window capabilities, CSP and navigation checks. Native journeys and Rust checks passed. Full security scope remains incomplete. | in progress |
| Renderer checks | `npm run check`: build/type check, four synthetic contract checks, twelve renderer checks across Chromium and WebKit | passed |
| OU-DATA-001 through OU-DATA-006, OU-DATA-019, OU-DATA-020, OU-CRYPT-001, OU-CRYPT-002, OU-CRYPT-011, OU-CRYPT-013 | SQLCipher/WAL core and native encrypted persistence passed. Migration rollback and newer-schema preservation passed Rust checks. Full platform coverage remains open. | in progress |
| OU-CRYPT-007 through OU-CRYPT-009, OU-CRYPT-015 through OU-CRYPT-017 | Authenticated passphrase envelope, real native fallback setup/restart/unlock, wrong-passphrase preservation. Owned Rust key/KDF buffers use zeroization. | Linux journey passed |
| OU-CRYPT-003 through OU-CRYPT-006, AT-33 | Linux Secret Service and native missing-key recovery passed. Windows Credential Manager and macOS Keychain code remain unexecuted. | in progress |
| OU-BACKUP-001 through OU-BACKUP-018 | Authenticated bounded archive codec passed four Rust checks. Native save, preview, checkpoint, and live restore remain incomplete. | in progress |
| OU-PROTO-011, OU-PROTO-014, identity and scope boundaries | Seven synthetic Rust protocol checks. No provider subprocess or live contract ran. | in progress |
| AT-36 | Native keyboard startup and renderer accessibility checks passed. Main workflows, screen-reader checks, and chart alternatives remain incomplete. | in progress |
| AT-01 through AT-55 | Default status for mandatory acceptance tests without a completed native journey | not started |
| AT-34, AT-35, AT-52, AT-53 and performance targets | No release-platform benchmark or ten-profile stress run | not started |
| Windows/macOS platform acceptance and AT-45 live identity | No executed evidence | not started |

Synthetic protocol checks, codec checks, renderer fixtures, and OS resource checks do not establish full native acceptance. The two named native journeys above have executed Linux evidence.

See [the checkpoint](implementation-status.md) and [the archived rejected amendment](proposals/sqlite-wal-amendment.md).
