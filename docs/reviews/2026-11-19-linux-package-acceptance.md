# Linux package acceptance

November 19, 2026. The source was `ce809054a81cde773cce8acf08144ffbe09db765`.
The build used CachyOS x86_64 on Linux 7.3.3, WebKitGTK 2.54.2, GTK 3.24.52,
Rust 1.99.1, Node 22.23.2, Bun 1.3.14, npm 10.9.8, and Tauri CLI 2.12.4.
The locked release builds used the local cache and the pinned Codex 0.163.0
and `ccusage` 20.0.48 archives. [Build commands](../../CONTRIBUTING.md#linux-package)

## Checks

- The Debian package lists `libwebkit2gtk-4.1-0` and `libgtk-3-0`. It contains
  the app, both executables, and their license notices. The sidecar hashes
  match the pins. The app matches the build output after accounting for
  Tauri's Debian bundle marker.
- Both npm tarballs installed offline into a private prefix with scripts
  disabled. The installed files kept their executable modes and hashes. The
  checksum list verifies both tarballs and the 426-component CycloneDX SBOM.
- The installed CLI passed `doctor`. In an empty home, `statusline` and
  `doctor --audit-history --json` returned exit code 8. `report` refused to
  read without consent. With `--yes`, it reported exactly 2,450 tokens for the
  synthetic Claude Code day and left the source files unchanged.
- `NATIVE-SMOKE-001` passed on the desktop app installed by npm with a private
  display, home, and D-Bus session. It checked startup, keyboard access to
  privacy details, and refusal of invalid IPC.

| File | SHA-256 |
|---|---|
| `OpenUsage_0.1.0_amd64.deb` | `e81aaa079eb3ef612e9d765e9fa0a8de9c6ccfa5118dc415d5454624d87ffdbb` |
| `openusage-0.1.0.tgz` | `6e4b4e1db799c9c088ecc8c7c05b8fd9677060cbe58801bb72e52953ddf7c4e9` |
| `openusage-linux-x64-0.1.0.tgz` | `003f26e7bc3cbf0cb06a869f918177921e19face673a0c57fc91150afe1a0eb5` |
| `openusage-0.1.0-linux-x64.cdx.json` | `6f8fa65f5c03e4e409cd6248832af4c9561d4e72cad79c297fac453df97bfda6` |

Debian installation on Ubuntu, real accounts, SBOM schema validation, and
release provenance remain open. [Compatibility matrix](../compatibility-matrix.md)
