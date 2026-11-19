# Historical evidence check

November 19, 2026. S-04f1 adds an explicit check for the historical evidence
requirement in [SC-031](../srs-clarifications.md#sc-031-evidence-for-historical-readings).

`openusage doctor --audit-history` uses the shared read-only workspace path.
It validates and counts evidence across every saved provider quota and activity
run in one transaction, including archived profiles. It reports counts without
profile or run identifiers. The [CLI contract](../../crates/openusage-cli/README.md#output)
records the exit behavior.

## Local checks

The public process test first returned a usage error for the new flag. It
returned `no_workspace` after the change. All 25 focused CLI checks and all 14
storage evidence checks passed on Linux with synthetic data. The storage tests
cover complete evidence, schema-13 migration gaps, and unexplained absence.
The full workspace passed 620 tests with two intentional ignores. Rust format,
both Clippy modes, and native journey syntax passed.

The compiled CLI passed against a private native Linux workspace. It counted
the fixture's two complete readings and zero historical gaps. Its output
matched the expected version-2 JSON. The test checked the database and WAL
hashes after the read and confirmed exit code 7 after key-store shutdown.

## Recovery boundary

Two source responses can normalize to the same saved values and produce
different hashes. Recovery needs evidence bound to the original run and
observation. The [producer test](../../crates/openusage-codex/tests/response_evidence.rs)
covers this distinction. This check reports gaps. Historical acceptance
remains open.
