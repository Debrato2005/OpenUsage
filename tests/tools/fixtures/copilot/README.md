# Copilot qualification input

[The test](../../test_ccusage_copilot.py) builds 48 private synthetic cases.
`qualification.json` records the pinned scanner's output and each required result.
It preserves qualification failures. It does not enable Copilot.

Each case supplies one home. Copied origins use separate paths within that home.
The scanner runs both reports twice with offline prices and read-only input.
The [review](../../../../docs/reviews/2026-11-12-copilot-qualification.md)
owns the results, source pins, and remaining gates.
