# Durable reading evidence review

Recorded November 16, 2026. Scope: S-04b, the durable part of OU-AUD-001/002/003, and affected backup and deletion boundaries.
Full S-04 remains partial. The SRS remains unchanged.

Every new Codex activity or quota acceptance now requires response metadata with matching method, run, association, provider version, origin, and observation time.
This includes delayed observations through non-live methods.
One combined collection keeps two independent method records in its existing transaction.
Failures preserve the prior association, readings, caches, history, and alert effects.
The [data contract](../data-contracts.md#reading-evidence) owns fields, versions, privacy, and lookup states.

Schema 14 adds two tables. Migration preserves historical values and adds explicit evidence-gap markers.
It invents no hashes, checks, adapter revisions, comparisons, or earlier status records.
The [SC-031 conflict](../srs-clarifications.md#sc-031-evidence-for-historical-readings) remains open and approves no normative exception.
New missing evidence remains invalid.

Astra resolved stale synthetic context, private directory modes, fixture inventories, and independent oracle schema limits.
The fixture correction retains exact comparisons across all 44 tables and both synthetic owners.
The review also found that UPDATE guards alone permitted SQL replacement.
Two controlled tests reproduced complete-record and historical-marker replacement before the fix.
Duplicate-insert guards now reject replacement and ignored duplicates. Explicit deletion remains available.
Delete-then-insert by a caller with SQL access stays outside this record guard.
The project makes no claim against a malicious unlocked operating-system account.

Nine public storage tests cover context rejection, rollback, replay, immutable records, corruption, migration, backup, checkpoint, restore, export, and deletion.
The round trip compares complete metadata for both methods and preserves the exact count 9007199254740993.
The original schema-13 backup restores only the explicit historical gap.
The backup and checkpoint equality helpers now include both evidence tables.

Independent Spec review found no actionable defect within this slice.
Standards review found the replacement and inventory gaps. Its focused source recheck reports both resolved.
These source reviews add no execution evidence.

The producer remains separate from account identity proof.
Official interface classification preserves synthetic executable provenance.
No display command, uniform status DTO, selected reading export, new adapter, or all-status alert behavior ships in this slice.
Real-provider qualification, historical compliance, and other platforms remain open.

## Executed checks

The final workspace passes 613 tests, with two intentional ignores.
All 538 Chromium and WebKit renderer cases, 74 IPC checks, and the renderer build pass.
Both workspace Clippy modes pass. Workspace and standalone Rust formatting pass.
The standalone oracle package passes both integration tests after the fixture corrections.
The Python tool suite runs 98 tests, with 46 explicit skips.
These are separate bounded checks, not one combined runner result.

The first collection run exposed a profile-only comparison that omitted evidence ownership through collection runs.
The first populated application export stopped at an old schema assertion in its synthetic seeder.
Both original failures remain private. Corrected runs use separate result directories.
The correction preserves other owners and rejects evidence mutations in 33 isolated synthetic comparisons.
No application workaround masks either test failure.

Nine native Linux journeys pass on the same native-test binary:
collection, application export, backup, backup inspection, restore preview, restore apply, checkpoint recovery, persistence, and local data deletion.
The collection journey completes 30 oracle checks and two genuine restarts.
It checks separate method hashes, retained evidence, cancellation, reassociation, unsupported capabilities, and selected-profile deletion.
Application export matches all 44 tables through three exports and five oracle checks.

The restore-preview, apply, and checkpoint journeys check their documented profile and settings fields.
They do not establish equality for every stored provider field.
The public storage round trip supplies the separate exact evidence checks.
The additional pending-collection restore and Undo journey passes.
Its exact oracle compares all 44 tables, including complete provider evidence, with only the documented restore changes.
It checks pending-work cancellation and refused restore tokens before successful restore and Undo.
The normal 300-second monitoring interval remains outside this mode.
Other changed native journeys update current schema expectations only and remain unexecuted for this slice.
Cargo stays idle throughout native runtime. Each runner checks the unchanged application hash.


All ten native journeys use the unchanged binary with SHA-256 `50513621495525ab7ddfc0904f64baaf97282be6167fe392fac95a79a563554a`.
The result directories, original failures, source hashes, and command receipts remain private.
Normal verification retains metadata. No executable copy is retained after the guarded target cleanup.
