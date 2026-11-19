# Saved history audit

November 19, 2026. S-04e1 covers part of OU-STATE-010/011,
OU-AUD-001/002, and OU-DES-007/009/010/012.

Quota and activity history read each page and its audit records in one
deferred transaction. The existing cursor, original binding, source kind,
and exact values stay intact. The two snapshot panels use the shared audit
labels. The [data contract](../data-contracts.md#provider-quota-and-activity)
owns absence and validity rules.

## Checks

Both history contracts fail their first IPC check before the audit fields
are added. The updated contracts reject another profile's page, unsupported
status, private fields, and invalid observation metadata.

The workspace run passes 620 tests with two intentional ignores. All 548
renderer cases pass in Chromium and WebKit. All 77 IPC checks pass.
The focused storage run passes all 14 evidence tests. It checks schema 13
migration and restore, original values, and trusted gaps in both histories.
Rust format and both Clippy modes pass.

## Native Linux

Each history journey checks its first 20 readings and the 2 older readings.
Activity compares each entry with the independent encrypted-copy oracle.
Quota matches an older snapshot to its detailed evidence and preserves both
history export formats. Both journeys check archived history after restart.
Light and dark captures show the new audit rows at 720 pixels.

Overview evidence disclosure passes on the same native-test build.
The ordinary build rejects all six synthetic provider executables before launch.
The source gate preserves the private workspace and protected key.

## Scope

This slice covers desktop history. SC-031, other evidence clients, alerts,
and reset policies remain open. Acceptance uses synthetic data on Linux.
