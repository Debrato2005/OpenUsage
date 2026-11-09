# Remaining tool fixtures

These synthetic inputs record the Linux x86_64 contracts of the pinned `ccusage` 20.0.48 binary. They cover the 16 tools that OpenUsage does not enable yet. The [first](../../../../../docs/reviews/2026-11-08-l1f-formats-a.md) and [second](../../../../../docs/reviews/2026-11-08-l1f-formats-b.md) source notes give the format references and known limits.

`inputs.json` holds JSON records and SQL statements. The test creates SQLite files in a private temporary folder. It encodes the Antigravity protobuf fields there as well. `expected.json` holds the reviewed daily and session outputs from the pinned binary. No real log, credential, prompt, or response is included.

Run the comparisons with the provisioned binary:

```sh
OPENUSAGE_CCUSAGE_BIN="$PWD/apps/desktop/src-tauri/binaries/openusage-ccusage-x86_64-unknown-linux-gnu" \
  python3 -m unittest discover -s tests/tools -p test_ccusage_formats.py -v
```

The test checks the binary hash before execution. Every report gets an empty private home, one explicit source root, and fixed offline arguments. The tests compare both reports twice for each tool, check empty roots, and compare source hashes before and after. Together with the copied-root cases, these checks run the scanner 104 times. A copied Amp thread reports 324 tokens for two identical copies of 162 tokens, in both reports. This records an upstream defect and does not qualify Amp for AT-65. Qwen's empty session report has null totals. The test keeps that difference explicit.

These comparisons establish a format baseline, not release acceptance for a tool. The expected reports retain upstream gaps and stored charges. Normalizers must handle them under [SC-022](../../../../../docs/srs-clarifications.md#sc-022-incomplete-metrics-from-the-other-local-tools). Forks, replay, compaction, copied roots, missing fields, alternate formats, storage, and native application journeys still need their own checks under AT-65.

The Rust [period tests](../../agent_periods.rs) read these recorded reports and construct malformed variants in memory. They check independent daily totals for Amp, Pi, Kilo, and ZCode, and one unplaced total for each other tool. Model attribution, nullable details, and reported zero remain separate from missing data. These tests do not rerun the scanner or qualify duplicate handling.

The copied-root test uses two roots for Pi, Kilo, and Amp. Pi and Amp report 324 tokens, against 162 for one root. Kilo reports 173 for both one root and its copy. Source hashes stay unchanged. The minimal Pi fixture has no entry ID. The copy has a different path-derived project, which participates in the pinned scanner identity. This remains an AT-65 qualification limit. No tool is enabled by these tests.

The Kilo result above uses an ID embedded in the JSON payload. Its own legacy writer instead stores IDs in database columns. The [Kilo qualification](../../../../../docs/reviews/2026-11-10-kilo-qualification.md) covers that layout in 16 cases, recorded in `kilo-qualification.json`. Run `test_ccusage_kilo.py` through the same unittest command. It checks copies, forks, replay, compaction, incomplete reads, and missing metrics in 32 reports. Several cases deliberately preserve failing scanner behavior. Kilo remains disabled, and AT-65 remains open.

The [ZCode qualification](../../../../../docs/reviews/2026-11-10-zcode-qualification.md) records 19 cases and 38 reports in `zcode-qualification.json`. Run `test_ccusage_zcode.py` through the same command. It models the separate usage ledger and transcript tables. Copy, replay, fork-transcript, and compaction cases match their expected counts. Conflicting IDs and incomplete reads remain qualification failures. The Rust regression also checks missing projects, known activity times, and unavailable estimates and token details. ZCode stays disabled.

The [Pi qualification](../../../../../docs/reviews/2026-11-10-pi-qualification.md) records 29 cases and 58 reports in `pi-qualification.json`. Run `test_ccusage_pi.py` through the same command. It covers stable entry IDs, copies, branches, replay, non-message usage, missing fields, and incomplete reads. Copies can double counts, distinct calls can collapse, and summary or standalone usage can disappear. The Rust regression preserves unavailable fields and reported activity times. Pi stays disabled, and AT-65 remains failed.

The separate [Pi usage candidate](../../../../../docs/reviews/2026-11-10-pi-usage-candidate.md) records five corrected or added cases in `pi-usage-candidate.json`. Its five JSON/table views are in `pi-usage-candidate-cli.json`. The explicit candidate test preserves the production baseline and checks that only the scoped usage cases change. These outputs do not qualify the remaining identity or completeness failures and do not replace the production pin.

## Pi identity candidate

`pi-identity-candidate.json` records 40 synthetic cases and 80 daily/session outcomes from the separate local identity candidate. Fifteen cases are explicit unavailable outcomes, not scanner reports or zero totals. The other 25 include a valid empty tree. `test_ccusage_pi_identity.py` checks the explicit binary digest, independent expected totals, reversed roots, unchanged sources, isolated homes, other tools, and JSON/table views. The [review](../../../../../docs/reviews/2026-11-10-pi-identity-candidate.md) owns its source evidence and remaining gates.

The original Pi matrix remains the production baseline. Its fresh-child source fixture now has a null parent entry ID; this correction leaves the recorded pinned output unchanged. OpenUsage's parser test reads the accepted candidate reports and preserves missing details, estimates, and model attribution. No tool or production pin changes.

## Pi format candidate

`pi-format-candidate.json` records nineteen cases and 38 daily/session outcomes from the third local Pi patch. Five cases have reports. Fourteen are explicit unavailable outcomes, not zero totals. `test_ccusage_pi_format.py` also checks the earlier identity matrix, other tools, empty roots, unchanged source hashes, and isolated homes. The [review](../../../../../docs/reviews/2026-11-10-pi-format-candidate.md) owns source evidence, digests, and remaining gates. No production pin or enabled tool changes.

## Pi filesystem candidate

`pi-filesystem-candidate.json` records sixteen cases and 32 daily/session
outcomes from the fourth local Pi patch. Baseline and repeated-root cases retain
162 tokens. Fourteen cases return unavailable. The explicit candidate test also
checks the prior format and identity matrices, other tools, source hashes, and
private-home isolation. Permission checks require an unprivileged Linux process.
The [review](../../../../../docs/reviews/2026-11-10-pi-filesystem-candidate.md)
owns limits, source evidence, digests, and remaining gates. No production pin or
enabled tool changes.

## Pi snapshot candidate

`pi-snapshot-candidate.json` records five cases from the fifth local Pi patch.
Four return envelopes: baseline, independent total with absent categories,
unattributed usage, and empty input. Malformed input returns unavailable.
The explicit candidate test also applies the snapshot command to the previous
identity, format, and filesystem matrices. Source hashes stay unchanged.
Rust tests preserve nullable details and reject incomplete, inconsistent, or
duplicate-field envelopes. The [review](../../../../../docs/reviews/2026-11-10-pi-snapshot-candidate.md)
owns the controlled-write evidence and remaining gates. Pi stays disabled.

## Pi observation candidate

`pi-observation-candidate.json` records seventy cases from the sixth local Pi
patch. Thirty return reports and forty return unavailable. Explicit zero has
rows, empty sources have none, and missing required measurements reject the scan.
The test checks independent totals, row counts, and reported entry times before
it compares recorded output. The inherited tests preserve the prior matrices,
other tools, private homes, and source hashes. Rust tests cover nullable fields,
zero records, and saved-history retention. The
[review](../../../../../docs/reviews/2026-11-10-pi-observations-candidate.md)
owns source evidence, digests, failures, and remaining gates. Pi stays disabled.
