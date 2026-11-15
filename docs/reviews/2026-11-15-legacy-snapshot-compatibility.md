# Legacy Codex snapshot compatibility

November 15, 2026 · L-1f11au · Private synthetic Linux x86_64 input.

The isolated candidate passes the compatibility controls that blocked the earlier
[identity experiment](2026-11-13-legacy-codex-identity.md). Ordinary commands retain
the parent behavior. The explicit `codex legacy-snapshot --json --offline` command
returns both strict reports from one captured event set. This resolves the scoped
routing regression. It does not qualify legacy production support.

## Command and identity boundary

The [patch](../../patches/ccusage/20.0.48-codex-legacy-snapshot.patch) restores
`aggregate.rs` and `loader.rs` to the compatibility parent byte-for-byte.
Ordinary Codex reports keep path, basename, filename, recorded-ID, and
`codex://threads/ID` selectors. Last-only counters, total-only counters, omitted
reasoning counts, missing identity, and lineage retain their ordinary parent behavior.
Unrelated `notes.zst`, an unused `1e400`, and an unused 140-level array also retain
that behavior.

The strict command uses the first recorded `session_meta.payload.id` as its thread
identity. Equal decoded streams and exact prefixes select one longest capture.
Divergent streams with the same ID refuse. Distinct IDs remain separate despite
equal counts and timestamps. No account identity or response identity is inferred.

The strict envelope contains `schemaVersion`, `tool`, `historyMode`, `daily`, and
`session`. Its `historyMode` is `legacy`. The existing paginated consumer rejects
this extra field. Strict session rows use recorded IDs and an empty directory.
There is no fallback to ordinary parsing after strict refusal.

## Executed checks

The [pin](../../tests/tools/fixtures/codex-legacy-snapshot-candidate/pin.json) owns
source, executable, build, and limit records. The tested executable is
`d2447fed2661a9473816b537ca8dfb51230620030fe39f2b2187a5f5edacba2a`.

| Check executed on November 15 | Result |
|---|---|
| Candidate suite | 12 tests pass, no skips, 213 scanner commands |
| Source reconstruction | All 538 manifest entries match. The patch applies with zero fuzz and no offsets |
| Ordinary shared matrix | All 47 Claude Code and Codex reports match the compatibility parent, including output hashes |
| Ordinary counter and metadata cases | Nine variants pass across three report routes |
| Ordinary session selectors | Five selectors match the parent |
| Strict identity controls | Copies count once. Distinct threads count separately. Exact prefixes select the longest capture |
| Strict refusal controls | Conflicts, invalid counters, duplicate keys, lineage, unsafe paths, report modifiers, compressed faults, and the joint count limit refuse |
| Strict capture timing | A change during reading refuses without output. A change after capture leaves both reports at 4,600 tokens |
| Inherited paginated suite | 13 tests pass, no skips, 95 scanner commands, including the raw synthetic producer comparison |
| Explicit ordinary-count comparison | Six commands confirm the two retained defects below |

The two suites and the final comparison execute 314 scanner commands in total.
Each scanner command has a 20-second deadline. Each suite has a 180-second deadline.
Inputs and homes are private and synthetic. Tests check source preservation, except
for the deliberate capture mutations. They also check that scanner homes stay empty.

## Retained ordinary count defects

Compatibility does not establish correct identity counts in ordinary reports.
The strict command corrects these two synthetic cases:

| Case | Ordinary parent and candidate | Strict daily and session |
|---|---:|---:|
| Renamed copy, session report | 9,200 | 4,600 |
| Distinct threads with equal usage, daily report | 4,600 | 9,200 |

These ordinary defects remain visible. No test redefines their expected correct
counts or treats parent equality as full qualification.

## Remaining limits

The strict command still refuses last-only counters, total-only counters, omitted
reasoning counts, unrelated `.zst` files, unused large numbers, and deeply nested
unused values. These are unresolved compatibility limits, not new support rules.
Missing or ambiguous identity, lineage, subagents, replay, compaction, reverts,
and unexplained counter changes also remain outside this strict candidate.

The synthetic legacy fixtures declare producer version 0.166.0. The pinned raw
producer capture uses 0.163.0 with paginated history. Its passing comparison does
not qualify the legacy producer or history mode.

Stored-session migration remains open. Old path keys and recorded IDs must not
become two saved sessions for one history. Source discovery, metadata compatibility,
reference performance, production integration, native acceptance, and packaging
remain open. Other platforms and real histories remain unverified.

No scanner source change or build was needed for this review. Earlier saved source
checks report 163 Codex library tests, 99 parser tests, 16 Claude CLI tests, and seven
Codex CLI tests passing. Earlier formatting and scoped Clippy checks also passed.
Those records were inspected, not rerun. The pinned final build reused the shared
target. No clean final rebuild is claimed. Production pins and enabled tools remain
unchanged.

## Reproduction and private evidence

Run the candidate suite with `OPENUSAGE_CODEX_LEGACY_SNAPSHOT_CANDIDATE` set to the
oracle path in the pin:

```sh
timeout 180s python3 -m unittest discover -s tests/tools -p test_ccusage_codex_legacy_snapshot.py -v
```

The inherited suite uses `OPENUSAGE_CODEX_SNAPSHOT_CANDIDATE` and
`OPENUSAGE_CODEX_SNAPSHOT_SHA256` for that same oracle. It also needs
`OPENUSAGE_CODEX_CAPTURE_RECEIPT` set to the exact pinned private synthetic receipt.

```sh
timeout 180s python3 -m unittest discover -s tests/tools -p test_ccusage_codex_snapshot.py -v
```

Private metadata remains in these retention-managed directories:

- `.recovery/diagnostics/legacy-snapshot-compatibility-eac989022d6742f68ce42e5f7214dcf1/result.json`
- `.recovery/diagnostics/legacy-snapshot-inherited-controls-be7fc9df21674132bcf37e2f90d7784c/result.json`
- The second directory also contains `paginated-matrix.json` with report hashes and exact totals.

The earlier source checks remain in `.recovery/audit/l1f11au-source-checks.json`.
The earlier build receipts remain in `.recovery/audit/l1f11au-build.json` and
`.recovery/audit/l1f11au-final-build.json`. Raw producer data stays private.
