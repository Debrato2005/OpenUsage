# Antigravity qualification input

[The test](../../test_ccusage_antigravity.py) builds 37 private synthetic cases.
`qualification.json` records the pinned scanner's output and each required result.
It preserves qualification failures. It does not enable Antigravity.

The SQLite tables and protobuf fields follow the consumer, not a verified writer.
Both reports run twice with offline prices and read-only input.
The [review](../../../../docs/reviews/2026-11-12-antigravity-qualification.md)
owns the results, source pins, and remaining gates.
