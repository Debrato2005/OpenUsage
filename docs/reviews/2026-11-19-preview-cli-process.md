# Preview CLI process checks

November 19, 2026. L-6c1 checks part of OU-NFR-012 and AT-77 to AT-79.
The [workflow](../engineering-workflow.md#commands) defines the runner scope.

## Empty reports and scanner approval

The Linux regression first failed with `ccusage_unavailable` on an empty
synthetic home without a scanner pin. The fix checks the pin when a scan is
needed. Both an absent pin and a mismatched pin still return exit code 4 for
detected roots after consent. The synthetic scanner marker remains absent.

The existing public CLI tests clear the child environment and select a private
home on each target. The Windows fixture sets `USERPROFILE` and `LOCALAPPDATA`.
The two process tests check help, no-workspace behavior, report consent, stable
error output, and offline leaderboard preview.

## Local checks

All 25 focused CLI checks passed on Linux. The workspace passed 620 tests
with two intentional ignores. Rust format and both Clippy modes passed.
Local workflow checks verified YAML, runner targets, read-only permissions,
the pinned Rust version, and shell syntax. All 16 engineering tool tests passed.

The Linux native CLI journey passed against the rebuilt desktop and CLI.
It checked consent, the pinned scanner, no-usage output, exact saved quota,
read-only workspace files, and exit code 7 after key-store shutdown.

## Hosted preview checks

Both public CLI tests passed on Apple Silicon in the [first hosted run](https://github.com/Debrato2005/OpenUsage/actions/runs/44493637393).
On Windows the build stopped before the tests. Git Bash's Perl lacked
`Locale::Maketext::Simple`, which vendored OpenSSL requires. The Windows job
now checks native Perl and the required modules before passing its path to
OpenSSL. The locked CLI command stays the same.

Intel Mac and the corrected Windows run need their own results. The
[compatibility matrix](../compatibility-matrix.md) owns platform qualification.
