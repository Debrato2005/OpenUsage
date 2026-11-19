# Desktop preview compile check

November 19, 2026. L-6c2 covers part of OU-NFR-012.
The same three hosted preview runners check the desktop Rust crate after the
locked CLI process tests. Each compile uses Tauri development mode and a local
development URL. This removes the need for bundled frontend assets in this check.

## Local verification

The workflow check first found no desktop compile step. After the change,
it checked the three runner targets and confirmed that the new configuration
belongs to one step. The locked desktop compile passed on Linux with default
features disabled and an explicit path to absent frontend assets.

## Scope

Hosted Windows and macOS results remain pending. The compile check does not
launch the desktop or build a package. The [compatibility matrix](../compatibility-matrix.md)
owns companion qualification, packaging, and real-machine acceptance.
