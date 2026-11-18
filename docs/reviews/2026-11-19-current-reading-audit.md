# Current reading audit

November 19, 2026. S-04e0 covers part of OU-AUD-001/002, OU-STATE-007/010/011,
OU-DATA-018/027, and OU-DES-007/009/010/012.

Saved desktop quota and activity responses carry compact audit metadata.
Current activity and its daily rows share one read transaction.
Each retained day keeps the audit and provenance of its own run.
Complete evidence and historical gaps follow the [data contract](../data-contracts.md#provider-quota-and-activity).
A corrupt audit record fails the read, and a missing reading stays absent.

Chart projection uses dates and exact counts. It assigns no reading status
to an aggregate. The renderer uses the same observation checks for complete
compact and detailed evidence. Status and executable origin remain distinct.

## Checks

IPC checks reproduce and reject unsupported statuses, invented historical
status, invalid observation metadata, and inconsistent context for one run.
The review also caches the prepared evidence statement for repeated daily reads.

The workspace run passes 620 tests with two intentional ignores. The activity
snapshot test commits a newer run during the read. Each response retains
one generation. A schema 13 database and backup preserve historical values.
The full renderer suite passes 548 cases across Chromium and WebKit.
All 77 IPC tests pass. After a lint-only correction to the test assertions,
all 14 storage evidence tests, Rust format, workspace Clippy, and native-test
Clippy pass.

## Native Linux

The summary details journey matches compact audit fields to the saved evidence
records for quota, activity, and daily rows. Keyboard disclosure and saved values
pass. Light and dark captures show wrapped evidence hashes at 360 pixels and
200% test-side text size.

The saved daily chart and year heatmap journeys pass on the same native-test
binary. The first heatmap run passed the calendar checks and stopped at the
window-manager check. A fresh private Xvfb session with Openbox passes native
maximize, minimize, restore, and close. The environment record identifies Openbox.
The journey's generic scope string still names WSLg and Weston.

The ordinary build refuses all six synthetic provider executables before launch.
Its source-gate checks preserve the private workspace and protected key.
Build receipts retain hashes and source metadata.

## Scope

The checks use synthetic data on Linux. Live providers, other platforms,
performance at the history limit, and other evidence clients remain open.
[SC-031](../srs-clarifications.md#sc-031-evidence-for-historical-readings)
still governs the historical evidence conflict. This slice adds no new
adapter or status and does not complete S-04.
