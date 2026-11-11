# Native Pi candidate qualification

November 12, 2026 · L-1f11ak · Linux x86_64, synthetic input.

The qualification build routes the bounded Pi candidate through consent,
the desktop worker, grouped storage, typed IPC, and Overview. Production still
enables only Claude Code and Codex. The production scanner pin is unchanged.

## Execution boundary

The Linux-only feature enables the private native-test home. A separate
Pi descriptor uses version `20.0.48-openusage.pi-bounded-release.1` and the
SHA-256 digest in the [producer pin](../../tests/tools/fixtures/pi-producer/pin.json). Rust checks the selected descriptor before
each execution. A missing candidate descriptor cannot fall back to production.
The normal build ignores the candidate override and refuses Pi consent.

The renderer accepts known catalog IDs returned by Rust. Rust controls which
tools it offers and permits. Unknown IDs, duplicates, extra fields, and invalid
values still fail the typed contract.

The worker stores the selected descriptor's actual version. Grouped storage
accepts bounded prerelease tags and preserves existing version spellings.
Legacy validation and schemas stay unchanged. Shared cases check both storage
and renderer validation. The tagged version survives encrypted reopen, export,
backup validation, and restore.

## Native result

The private Pi fixture contains an original record and an exact copy. Both
produce 120 tokens together. An appended record raises saved and displayed
usage to 137. A malformed peer then fails the scan and preserves 137.

The saved scope keeps UTC after the display changes to New York. The independent
SQLCipher oracle checks the full version tag and both record links. Storage
keeps one current scan after growth removes every reference to the older scan.
Day and session totals both equal 137. Input counts and estimates stay null.

A native restart retains the same totals. After withdrawal, another source
change cannot enter storage. A binary with the wrong digest never executes.
Saved usage remains 137. Scans preserve all selected source hashes and the
outside canary. The ordinary native-test build neither offers nor grants Pi.

## Checks and corrections

The full staged milestone passed. It includes the renderer suite in Chromium
and WebKit, workspace tests, Clippy, IPC contracts, dependency policy, Python
tests, formatting, and native syntax. Feature tests and feature Clippy passed.
The native oracle crate's tests passed. The normal build also passed local-log
consent, backup, restore preview, restore apply, checkpoint recovery, and
application export. Setup and Settings screenshots passed review in both
themes at 720 pixels. The populated native view also passed visual review.

The first native probe correctly showed no Pi route in the baseline build.
Integration exposed the old numeric-only version check and two renderer filters.
Tests failed before those corrections. A later test wrongly required two scan
rows. The corrected oracle checks current provenance and record references.
The original failure evidence remains separate from the passing journeys.

The historical bounded release digest was recovered without changing the pin.
A source-root build with SQLite trim settings produced a different digest.
The corrected working directory and settings reproduced the existing pin.
The [engineering workflow](../engineering-workflow.md#scanner-candidate-builds) owns that build boundary. No failed command received
an automatic retry.

## Remaining gates

This is functional evidence for private synthetic Linux input. It does not
qualify real histories, other platforms, or release packaging. Native Pi timing
at the required reference scale remains open. The package still needs one
qualified scanner for its enabled tools before a production pin change.
The next comparison checks the candidate against enabled Claude Code and
Codex. No additional tool is enabled by this result.
