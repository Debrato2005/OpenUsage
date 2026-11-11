# OpenCode validated record candidate

L-1f11v, November 11, 2026. This layer corrects all six failures from the
[real writer qualification](2026-11-11-opencode-runtime-qualification.md).
All seventeen required outcomes now match. OpenCode stays disabled while
fork, scale, and app qualification remain open.

## Observation rules

A completed assistant can report all five token buckets as zero without an
independent total. A private marker preserves that measured observation only
after strict validation. JSON cannot supply the marker. Missing or invalid
buckets retain their earlier refusal. Empty aggregate placeholders remain empty.

Unified daily summaries now retain zero observations from the strict OpenCode
loader. Empty or metadata-only sources still return no row. Other agents keep
their existing filter. The first integration test exposed this separate
summary filter after the source parser already kept the zero entry.

The loader validates agent switches, model switches, synthetic messages,
shell records, and compaction records against the pinned producer shapes.
It rejects unexpected usage fields, invalid fields, and contradictory row
identities before it skips a non-usage record. Arbitrary values inside the
schema-defined metadata object remain annotations. They create no usage.

Canonical dedup retains those annotations and original model objects, including
variants. Matching copied records count once. A conflicting variant or changed
metadata under the same ID returns unavailable in either root order.

## Checks

All 126 OpenCode adapter tests and 81 unified adapter tests passed, with selected
Clippy, formatting, and the locked offline release build. The expanded tests
use private homes and temporary folders. Two unified test helpers now state
complete zero cache and reasoning buckets. Their expected totals stay unchanged.

The [candidate checks](../../tests/tools/test_ccusage_opencode_records.py)
passed all 21 tests independently at root. They cover the seventeen actual
producer outcomes, 26 legacy cases, sixteen format baselines, enabled tools,
WAL and source isolation, annotations, conflicting variants, and invalid records.
Measured zero has one row in both reports. Unfinished or failed assistants
without usage remain unavailable. The original runtime failure record stays intact.

The v2 consumer fixture now omits identity and role fields that belong in SQL
columns, as the real projector does. Every recorded report from the pinned
production scanner remains unchanged after this fixture correction.

The [candidate record](../../crates/openusage-locallogs/tests/fixtures/remaining-tools/opencode-records-runtime-candidate.json)
keeps the new observed totals. Source files remain unchanged after scanning.
The producer fixture still checkpoints before each scan. Actual producer forks,
live writer capture, scale, grouped storage, native app integration, real logs,
Windows, and macOS remain unqualified. This does not complete AT-65 or change
the production scanner pin.

## Build and run

Apply the record patch after the observation, capture, WAL, and reuse patches
on the same pinned ccusage source. Use the
[candidate build rules](../engineering-workflow.md#scanner-candidate-builds),
verified pricing, and
`CCUSAGE_VERSION=20.0.48-openusage.opencode-records-candidate.1`.
No dependency or lockfile changes in this layer.

Set `OPENUSAGE_CCUSAGE_OPENCODE_RECORDS_CANDIDATE` and its `_SHA256` variable.
Keep the three runtime paths and the separate schema-source path from the
earlier fixture guides. Run from the project root:

```sh
python3 -m unittest discover -s tests/tools -p test_ccusage_opencode_records.py -v
```

Patch [20.0.48-opencode-records.patch](../../patches/ccusage/20.0.48-opencode-records.patch)
has SHA-256 `841390f01f0c6668e0089c6d73f8cfa1bb99b9631ccf76114edfbf3e83ce45bf`.
The release binary has SHA-256
`b6d6f34c7ae1f2465fcca4ea77de9d72d6488adac3198b71b541fab2b18b1a3c`.
