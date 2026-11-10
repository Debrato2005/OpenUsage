# Pi bounded capture prototype

November 11, 2026 · L-1f11n · Linux x86_64, synthetic input.

The prototype implements the [large-history decision](../decisions/2026-11-11-pi-large-history.md).
It reads one file at a time into a reusable buffer. It keeps session headers,
record IDs and parents, record fingerprints, and mapped usage entries in memory.
It does not write log content or fingerprints to disk. It checks the selected
roots, the file stamps, and the file digests again before it returns a capture.

The record digest covers every field except `parentId`. The scanner checks the
parent separately. This permits a fork to change a parent link after it removes
a label. Type and length markers distinguish arrays, objects, strings, and
numbers. The tests cover key order, escaped strings, signed floating-point
zero, and the difference between an absent field and null. The prototype
relies on SHA-256 collision resistance for content comparisons.

The scanner uses one index across all selected roots. It checks parent paths
against the manifest and refuses missing parents or conflicting records. It
builds both reports from the reconciled entries after the capture passes.
The grouped runner uses its existing consent and save path.

## Bounds

| Limit | Prototype value |
|---|---:|
| Total source bytes | 1 GiB |
| Source file bytes | 32 MiB |
| Nonblank JSONL line bytes | 1 MiB |
| Physical records, including copies and replays | 100,000 |
| Accounted string bytes for the index and mapped entries | 128 MiB |
| Selected roots, directory entries, JSONL files, depth | 64, 32,768, 8,192, 64 |

The string budget charges for copies the mapper can allocate. It does not
measure the process's resident memory. Record and file counts bound the other
index entries. The grouped runner keeps its existing timeout and output cap.
A failed scan returns no report. The capture checks follow
[SC-025](../srs-clarifications.md#sc-025-captured-local-records-and-producer-completion).

## Build and scope

The [patch](../../patches/ccusage/20.0.48-pi-bounded.patch) applies after the
observation layer on ccusage `8995f3b7fd7110b8f37564f7d0468acc7a7d6b20`.
The offline debug build used Rust 1.99.1, the shared target, and the verified
pricing input. Ring 0.17.14 was already in the lockfile. This layer adds one
direct dependency on it. The [test pin](../../tests/tools/fixtures/pi-producer/pin.json)
holds the prototype version and executable digest.

The [performance report](../performance-report.md#pi-bounded-capture-prototype)
holds the measurements. The [traceability log](../acceptance-traceability.md#evidence-log)
holds the executed checks. Pi stays disabled and the production pin stays in
place. Release builds, histories above the record and line limits, and the
consent-to-display target still need qualification.
