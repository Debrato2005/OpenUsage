# OpenCode v2 source boundary

L-1f11x, November 11, 2026. A separate candidate restricts OpenCode capture to
the tested fresh v2 storage format. Unsupported legacy or mixed input returns
unavailable for the complete selected group. Legacy fork accounting remains
unqualified. OpenCode stays disabled.

## Source contract

The [fork qualification](2026-11-11-opencode-fork-qualification.md) found copied
usage under new IDs without a stored origin. Source review also found a second
path to those counts. Legacy step-finish events increase shared session totals.
The usage migration sums legacy assistant data into those totals.
Filtering legacy messages while retaining aggregate fallback can still double count.
Filtering both can discard valid legacy usage in a mixed capture.

The actual v2 writer keeps its shared session totals at zero.
It stores measured usage in `session_message`. Its inspected service has no fork
or legacy-import operation. Replay preserves message IDs.
The synthetic analysis executed the actual fork and v2 projectors in one database.
It also executed the seven exact SQL statements from the usage migration.
The historical migration engine and full CLI did not execute.
All eighteen source files used by the analysis match official Git blobs.
The source remains OpenCode v1.19.5 at
`810c0eba1a7348036f8bdf61e6632d0c36c6876c`.

The selected boundary follows current captured records under
[SC-025](../srs-clarifications.md#sc-025-captured-local-records-and-producer-completion).
It does not require complete durable history or infer writer completion.
A future legacy contract needs stable usage origins that forks, replay, imports,
and migrations preserve. Its aggregates also need explicit origin coverage.
Historical records without that evidence remain unavailable.

## Candidate behavior

Each database must contain the four exact critical table declarations from the
pinned fresh schema. Root independently compared all four declarations with
an actual producer database. Exact column names also match.
The pinned SQLite build omits schema PRAGMAs and declared-type introspection.
The guard therefore uses known declarations without a permissive fallback.
Different historical declarations remain unsupported until qualified.

The complete selected group returns unavailable for:

- Discovered JSON, any legacy message or part row, or a `session_v2` or `session_entry` table.
- Missing or changed critical schema, invalid identities, or conflicting sequence positions.
- Nonzero or invalid shared session token or cost aggregates.
- Unknown message shapes, legacy payload fields, or missing required measurements.

The candidate removes aggregate fallback and the unqualified fork-column algorithm.
It does not infer identity from titles, time, equal content, or equal counts.
Stable v2 message IDs count canonical copies once. Conflicting payloads or
positions return unavailable in either root order. Distinct calls keep their counts.
Model variants and annotations remain part of canonical identity.

V2 assistants need all five explicit nonnegative integer token buckets.
An independent `tokens.total` field is outside this pinned format.
Measured zero remains a row. Empty metadata and zero session compatibility
fields create no measurement. Missing assistant usage stays unavailable.

The capture bounds, source rechecks, read-only SQLite VFS, and WAL reconstruction
retain their earlier behavior. The production scanner pin and catalog stay unchanged.
No dependency or lockfile changes in this layer.

## Checks and limits

All 85 OpenCode adapter tests and 81 unified-adapter tests passed without ignores.
The candidate retires 47 loader tests and three capture tests for the unsupported
legacy contract. Nine new v2 tests cover supported copies, distinct calls,
whole-group refusal, schema changes, identity conflicts, zero, overflow, date
filtering, and legacy-shaped payloads. The 28 remaining capture tests retain WAL, corruption, source changes, and isolation
checks. Their positive database fixtures now use v2.
Unified helpers now use the pinned schema, valid IDs, sequence positions, and
explicit buckets. Their expected totals and assertions stay unchanged.

Selected strict Clippy, formatting, patch application checks, and the locked offline
release build passed. Dependency Clippy stopped on an unchanged pricing warning
in `ccusage-core`. Selected Clippy used `--no-deps` without warning suppression.

All fourteen [independent CLI tests](../../tests/tools/test_ccusage_opencode_v2.py)
passed on synthetic Linux input. All seventeen actual v2 outcomes retain their
recorded reports. Eleven actual legacy root selections return unavailable across
44 repeated runs. Twenty-three unsupported-source cases refuse alone and in both
orders beside valid usage. Measured zero, metadata, variants, empty sources,
committed WAL, an open transaction, links, and invalid WAL passed.
Fifteen other format baselines and enabled Claude Code and Codex reports stay unchanged.
Every scanner test checks private homes and unchanged source bytes.

The earlier record and fork fixtures retain their original results.
Legacy and mixed-format users receive unavailable from this experiment.
Fresh-schema compatibility does not qualify historical migrations, every OpenCode
version, full CLI/provider behavior, scale, grouped storage, or native application integration.
Real logs, real accounts, arbitrary concurrent writers, Windows, and macOS remain unqualified.
This layer does not complete AT-65 or OU-DUP-004 for OpenCode.

## Build and run

Apply [20.0.48-opencode-v2.patch](../../patches/ccusage/20.0.48-opencode-v2.patch) after the five earlier OpenCode layers on the same pinned
ccusage source. Use the [candidate build rules](../engineering-workflow.md#scanner-candidate-builds)
and `CCUSAGE_VERSION=20.0.48-openusage.opencode-v2-candidate.1`.

Set `OPENUSAGE_CCUSAGE_OPENCODE_V2_CANDIDATE` and its `_SHA256` variable.
Keep the runtime and fork dependency paths from the
[fork fixture guide](../../tests/tools/fixtures/opencode-forks/README.md).
Run from the project root:

```sh
python3 -m unittest discover -s tests/tools -p test_ccusage_opencode_v2.py -v
```

The patch has SHA-256 `08d001e53c7f18eb90cc005f194e271082e0853b3932c26143102397aa83060e`.
The release binary has SHA-256
`7cbd2baf33d08c42e7df2b80478826a009c3552c0c51f90b2de1fd81407c0cc1`.
