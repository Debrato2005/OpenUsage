# Grok qualification input

[The test](../../test_ccusage_grok.py) builds 46 private synthetic cases.
`qualification.json` records the pinned scanner's output and each required result.
It preserves qualification failures. It does not enable Grok.

Each case selects one home. Copies use separate paths within that home.
Both reports run twice with offline prices and read-only input.
The [review](../../../../docs/reviews/2026-11-12-grok-qualification.md)
owns the results, source pins, and remaining gates.
