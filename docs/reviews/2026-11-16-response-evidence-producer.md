# Response evidence producer review

Recorded November 16, 2026. Scope: S-04a and the producer part of OU-AUD-001/002/003.
The SRS remains unchanged. Full reading evidence remains incomplete.

Both existing Codex parsers now attach response metadata after their domain checks pass.
The crate contract owns projection version 1, privacy rules, canonical hashes, and unused-field counts.
Display serialization omits the metadata. Account identity evidence keeps its separate contract.
The existing SHA-256 dependency now applies to all targets.
Its pinned version and the lockfile stay unchanged.

Astra resolved three controlled failures: the missing producer interface, positional arrays, and object-form enum values.
The initial array projection missed changed numeric values in an accepted positional response.
The initial enum projection gave equal hashes for distinct accepted credit statuses.
Public regressions now cover both forms without changing parser acceptance.
Recursive object sorting also protects canonical hashes under the alternate JSON map feature.

All 13 public evidence tests pass, including four independent digest examples.
They cover exact integers, null and absent fields, dates, private text, dynamic identifiers, unused additions, and ignored deep content.
The alternate `preserve_order` run passes the same 13 cases.
Direct red, green, and feature runs retain agent tool output only.
The final scoped verification receipt passes dependency policy, whitespace, format, crate tests, and all-target Clippy.
Independent Spec and Standards reviews report no residual findings. They add no execution evidence.

The renderer build and all 74 public contract checks pass.
The workspace passes 604 tests, with two ignored tests. Both broad Clippy modes pass.
The native combined-collection journey passes on private synthetic Linux data.
It completes 30 oracle checks, two genuine restarts, malformed and unsupported capability cases, cancellation, reassociation, and deletion comparisons.
Scheduled monitoring and the restore race remain outside that native run.

The first native run overlapped workspace tests that replaced its executable.
The process-ownership guard correctly refused the resulting deleted executable path.
A private reproduction confirmed this cause. The sequential run passed on the identical native binary.
The workflow now requires Cargo to finish before native execution.
These are separate bounded checks. The scoped engineering receipt covers its listed commands.
The prior connection-suggestion commit passes both hosted CI jobs in run 43765735233.

This slice stores no durable reading evidence and changes no display DTO.
Source and observation context, statuses, comparison, migration, exports, and alerts remain under S-04.
Live providers and other platforms remain unqualified.
