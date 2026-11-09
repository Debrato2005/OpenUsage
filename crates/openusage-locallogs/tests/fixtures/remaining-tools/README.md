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
