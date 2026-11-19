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

Both Mac jobs found a restore compile error. Darwin reports
available inodes as `u32`, and the restore request uses `u64`. The correction
compares both values losslessly in `u128`.

The Windows job found the fixed-drive constant in the wrong module. The
guard now imports `DRIVE_FIXED` from `WindowsProgramming` and keeps the
existing `GetDriveTypeW` check. [First hosted run](https://github.com/Debrato2005/OpenUsage/actions/runs/44539690073)

All 620 workspace tests and Clippy passed on Linux after the fixes. The
compiled desktop passed the private native restore-apply journey with the
checkpoint, live-database, and backup-fixture verifiers. Dependency policy
passed with the existing lockfiles.

The corrected hosted result remains pending. The compile check leaves desktop
launch and package acceptance to the [compatibility matrix](../compatibility-matrix.md).
