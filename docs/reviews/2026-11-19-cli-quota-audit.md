# Saved CLI quota audit

November 19, 2026. S-04e2 covers saved quota status under OU-CLI-002/004,
OU-STATE-010/011, and OU-AUD-001/002.

`openusage status --json` reads values and audit metadata through the shared
Rust quota view. Each profile's saved quota and evidence come from one
read-only SQLCipher transaction. The output keeps schema version 2.
The [CLI contract](../../crates/openusage-cli/README.md#output)
defines complete evidence, historical gaps, and absent quota.

## Checks

The existing command test first failed on the missing audit field. All 25
focused CLI checks passed after the change. The storage migration check
preserved the older quota and returned a historical gap with no reading status.
The full workspace passed 620 tests with two intentional ignores. Rust format,
both Clippy modes, the frontend build, and native journey syntax passed.

## Native Linux

The compiled CLI and desktop passed the private `cli-status` journey with
GNOME Secret Service, SQLCipher, and a synthetic Codex account. The CLI matched
the saved desktop audit, source version, and observation time. It returned 75%
and 60% remaining for the fixture's two windows. Text status and statusline
kept their existing behavior for connected and signed-out profiles.

The journey checked source file and executable hashes. The database, WAL, and
file set stayed unchanged after the CLI read. SQLite can update the shared WAL
index. Removing the private key store produced the locked result and exit code
7 without a prompt.

The ordinary desktop build passed the provider source gate. It rejected all
six synthetic provider executables before launch.

## Scope

Acceptance covers synthetic Linux. SC-031, live-provider comparisons,
other platforms, and the remaining alert and reset rules stay open.
