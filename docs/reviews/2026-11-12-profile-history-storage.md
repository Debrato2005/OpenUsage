# Profile history removal in storage

Date: November 12, 2026. Assignment: S-08a1. Scope: private synthetic Linux
storage tests. No real profile, provider, credential store, or log is used.

## Result

The Rust storage API now distinguishes Archive History and Delete History.
Its [architecture contract](../architecture.md#profile-removal) owns the input,
transaction, deletion boundary, and retained data. [SC-026](../srs-clarifications.md#sc-026-profile-removal-and-retained-delivery-controls)
records the requirement reading. The persistent schema remains version 13.

The caller supplies the exact profile and managed home with an opaque revision.
The operation refuses stale intent and protected environments. Fixed SQL tables
and bound owner values keep deletion within explicit profile ownership.
One transaction retains notification IDs until their constrained children are gone.
It then removes the selected profile. Foreign keys remain enabled.

## Executed storage evidence

The archive regression first failed against a public API stub. A direct test
executable diagnostic identified its expected WriteFailed panic with no compiler
error. The initial receipt retains source hashes. Its original executable digest
was not recorded, so that diagnostic is not a binary provenance claim.
A separate deletion regression failed before implementation. A fixture closure
compile error was corrected before its independent contract test passed.

The public fixture populates every current table. It includes two profiles,
multiple bindings and reading generations, history, local subscriptions, alert
outbox and receipts, shared and unreferenced accounts, global preferences,
and independent legacy and grouped local usage. True zero and missing metrics
remain distinct. Provider source kinds are synthetic records only.

Tests compare the complete application export before and after each operation.
Archive preserves every non-profile table. Delete removes exactly the selected
owned rows and keeps other records equal. Genuine SQLCipher reopen preserves
both results. Managed, external, WSL, and other-tool sentinel files remain equal.
Public current and historical reads confirm the removed profile is unavailable.

Refusal tests cover malformed IDs, missing profiles, a wrong or changed home,
WSL, local and external writes, foreign and reopened revisions, and repeat
deletion. A final-statement trigger proves full rollback after earlier deletes.
Cross-profile quota references and shared notification IDs also refuse atomically.
Archived and empty profiles can be removed without invented history.

Database backup snapshots validate after deletion and restore into a separate
private store. An earlier checkpoint restores removed history while current
submission receipts and incident state prevent notification replay. Archive
backup restoration retains archived history. These are storage snapshot tests,
not native archive-file or OS delivery acceptance.

## Primary review and integration

Review covers correctness, privacy, maintenance, tests, and requirements.
The existing UUID and profile readers are shared within the crate. No raw
connection, filesystem path, credential, or arbitrary SQL enters the public API.
The temporary event set avoids a history-sized Rust allocation. The all-table
comparison checks unrelated data instead of mirroring each deletion statement.

Ten public removal tests and the full storage suite passed. The storage suite
took 206.141 seconds. Scoped Clippy and format also passed.

Three independent source mutations disable the revision check, the home check,
and journal retention. Each named regression fails. Exact restoration passes
all ten removal tests. The restored module digest is
`85677ec5ea6f9cad73d7867e77d0d1cb1287f9fd0c44a2e33fc1f378c100cbee`.

Doctor and the staged workspace milestone passed. Workspace tests took
468.653 seconds, and workspace Clippy took 115.084 seconds. Format and
whitespace passed. The runner discarded raw compiler output and retained bounded
metadata. No dependency, schema, app, renderer, or IPC contract changed.
The renderer and native GUI suites did not run for this storage-only slice.

## Remaining gates

No renderer command calls this API. The app still needs the two removal choices,
confirmation, selected-profile process cleanup, cache cleanup, and native tests.
The existing Linux alert dispatcher holds the workspace lock across claim,
bounded submission, and completion. A later caller must preserve that ordering.
Native calls on other platforms can outlive their deadline and retain an
uncertain receipt. Removal must not claim completed cancellation from elapsed time.

Logical deletion makes no physical-erasure promise. Retained journals remain in
complete exports and backups. Earlier backups can restore old history through
the separate restore flow. Real accounts, other platforms, app acceptance,
and the broader repository audit remain open.

Private evidence uses `.recovery/audit/s08a1-*`. Only source and receipts remain
after the authorized push cleanup. No release or production activation occurs.
