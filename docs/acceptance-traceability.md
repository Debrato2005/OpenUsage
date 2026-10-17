# Acceptance traceability

Updated: October 17, 2026. Native startup passed on Linux. The full mandatory acceptance suite remains incomplete.

| Requirement or check | Implementation and executed evidence | Status |
|---|---|---|
| NATIVE-SMOKE-001 | Real Tauri binary, native bootstrap and command rejection, keyboard privacy access, unavailable readings, isolated Codex home. Receipt and screenshot in `evidence/native-startup/`. | passed |
| OU-ARCH-004, OU-ARCH-005, OU-SEC-006 through OU-SEC-009 | Native command registration, argument checks, main-window capability, CSP, navigation restrictions. Native startup and one Rust navigation test passed. Full platform/security scope remains incomplete. | in progress |
| Renderer build and startup checks | `npm run check`: build/type check, four synthetic contract checks, twelve renderer checks across Chromium and WebKit | passed |
| OU-DATA-001 through OU-DATA-006, OU-DATA-019, OU-DATA-020, OU-CRYPT-002, OU-CRYPT-011, OU-CRYPT-013 | SQLCipher/WAL core and four storage tests passed. Native setup and recovery journeys remain unexecuted. | in progress |
| OU-CRYPT-007 through OU-CRYPT-009, OU-CRYPT-015 through OU-CRYPT-017 | Authenticated passphrase key envelope. Three synthetic Rust tests passed. Native fallback UI and lifecycle remain in progress. | in progress |
| OU-CRYPT-003 through OU-CRYPT-006 | Platform key-store integration under implementation. A separate private Linux Secret Service passed a resource check. | in progress |
| OU-BACKUP-001 through OU-BACKUP-018 | Authenticated bounded archive codec. Four Rust crypto tests passed. Snapshot selection, preview, checkpoints, space measurement, and live restore remain incomplete. | in progress |
| OU-PROTO-011, OU-PROTO-014, identity and scope boundaries | Seven synthetic Rust contract tests passed against documented versioned shapes. No provider subprocess or live contract ran. | in progress |
| AT-36 | Native keyboard startup path and renderer accessibility checks passed. Main workflows, screen-reader checks, and chart alternatives remain incomplete. | in progress |
| AT-01 through AT-55 | Default status for mandatory acceptance tests without a specific completed native journey | not started |
| AT-34, AT-35, AT-52, AT-53 and performance targets | No release-platform benchmark or ten-profile stress run | not started |
| Windows/macOS platform acceptance and AT-45 live identity | No executed evidence | not started |

Contract tests, codec tests, renderer fixtures, and operating-system resource checks do not establish full native acceptance. Only `NATIVE-SMOKE-001` currently passed as a complete native journey.

See [the checkpoint](implementation-status.md) and [the archived rejected amendment](proposals/sqlite-wal-amendment.md).
