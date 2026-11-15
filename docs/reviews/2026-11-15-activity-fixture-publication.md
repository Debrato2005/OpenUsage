# Synthetic activity configuration publication

November 15, 2026. Test-only Rust helper with private synthetic Linux providers.

[Hosted CI for f4e492b](https://github.com/Debrato2005/OpenUsage/actions/runs/43697578148)
failed `synthetic_activity_observation_precedes_the_held_final_account_response`
with `Session(UnexpectedExit)`. The renderer job passed. The hosted output
contains no child stderr, so the exact hosted cause remains unconfirmed.

The `activity_mode` helper used `fs::write` to replace a JSON configuration.
That operation truncates the existing file before it writes the new contents.
A local probe held this empty interval while the actual synthetic Python
provider read the file. The child exited with `JSONDecodeError`. This reproduces
a failure on the hosted test path, but does not prove its exact hosted cause.

The helper now writes a complete temporary sibling and publishes it atomically.
A deterministic regression opens the previous file before the update. It
requires that reader to retain the old complete JSON and a new reader to
receive the new complete JSON. The regression failed against the original
helper and passed after the change.

All 69 Codex crate tests pass, including the original hosted test. Formatting
and scoped Clippy with warnings denied pass. No production protocol, provider
parser, timeout, safety assertion, or fixture Python source changed. The full
workspace result and replacement hosted result remain separate evidence.
Native GUI and Windows/macOS checks did not run for this test-helper correction.
