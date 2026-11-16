# Selected reading evidence export review

Recorded November 16, 2026. Scope: S-04d, part of OU-AUD-001/002/010, OU-DATA-022/027, and OU-SEC-012/018.

Latest readings, quota history, and activity history append eleven audit fields to CSV and JSON.
JSON uses schema version 2.
Metric availability stays separate from reading status.
A complete record supplies saved adapter metadata, source level, status, hashes, checks, comparison, and unused-field counts.
The [data contract](../data-contracts.md#selected-reading-audit-fields) owns field order and absence rules.

All three routes use the existing typed evidence reader inside their deferred transaction.
The IPC details route and exports share one validation implementation.
Each retained daily value keeps the evidence of its own observation.
Historical gaps supply no invented audit fields.
Absent readings retain explicit audit unavailability.
Unexplained missing evidence refuses the complete export.
Existing file publication, consent, exact values, aliases, limits, and source preservation remain in force.

The public Storage regression first failed on a missing audit column, then passed on all three routes.
It compares different activity and quota hashes and positive unused-field counts.
A retained-day case compares two observations with different hashes.
The real schema-13 migration case checks historical gaps and missing current readings without invented metadata.
The corruption case refuses latest and activity-history exports and preserves the saved snapshot.
Twelve existing export regressions pass, including row limits, archived history, prior bindings, and concurrent WAL snapshots.
The file-writer regression first failed on schema version 1, then passed with version 2.
All six writer tests pass, including private-path and publication controls.
Twelve evidence integration cases and 46 affected renderer cases pass.
The frontend and 76 IPC checks pass.

Independent Spec review reports no scoped findings.
Standards review closes one stale documentation item after a focused recheck.
These reviews establish no execution evidence.

This scope changes no database schema, provider activation, alert policy, or reset policy.
SC-031 remains open. Historical compliance and full S-04 remain incomplete.
The uniform status contract, other evidence clients, and real-provider qualification retain separate gates.


The first native activity-history export failed its independent oracle comparison after both formats wrote 146 rows.
All base fields and JSON audit text matched.
Only adapter and projection revisions differed in type.
The oracle returned SQLite integers. The export contract requires scalar strings.
The correction converts those two expected cells after integer checks.
It preserves every other field comparison and the canonical JSON checks.
Product Rust and the frozen application remain unchanged.
The failed run stays private evidence.

The corrected activity-history journey passes on native-test binary `143c92611855fca3324be71217917279bd495251364dc087ace991d8f5f96815`.
It compares 146 rows from 22 activity observations, including two associations and seven retained daily observations.
All three selected routes pass CSV and schema-2 JSON audit checks.
Four independent reads compare all 44 encrypted table rowsets.
Consent, aliases, identifier choice, exact values, archived restart, and source preservation pass.
Root inspection accepts the retained export capture.
The harness correction adds no product change or weaker field comparison.

All 617 workspace tests pass with two intentional ignores.
Both Clippy modes, format, frontend, and 76 IPC checks pass.
The standalone current-schema oracle test also passes.
These are separate bounded checks. The combined verification runner did not execute this set.
The full renderer suite retains its prior commit scope. This Rust export change runs 46 affected renderer cases.

The corrected quota-history journey passes 374-row CSV/JSON parity across 22 saved observations.
It checks schema 2, all 34 columns, Official status, fixture provenance, archived access, and restart.
The first invocation omitted its required fixture variable and stopped before setup.
Its preserved log establishes no product failure or native acceptance.
The corrected invocation supplies the reviewed quota fixture.
That helper enforces an owned private root. Unlike the activity helper, it has no purpose-marker parameter.
No source edit or rebuild corrected this invocation.

The selected-export mode in the monitoring harness also passes.
It checks three GUI files, explicit consent, profile selection, identifiers, exact strings, nulls, CSV quoting, and formula-prefix refusal.
Independent source checks preserve all 44 table rowsets, database, WAL, protected key, and private homes.
A genuine restart retains private defaults and saved records.
This mode establishes no scheduled-monitoring interval acceptance.

All three changed native journeys use the same unchanged native-test application.
They use private synthetic Linux homes, providers, and encrypted workspaces.
They establish no real-provider or other-platform qualification.

The ordinary-build source gate passes on binary `912aecafa0eac4adb31ca6f8841d6ea5587fca81c5be8368b0f81aed215edb38`.
It refuses all six reviewed test executables before process launch.
Build receipts retain provenance and hashes without executable copies.
