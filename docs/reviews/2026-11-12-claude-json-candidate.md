# Claude JSON formatting candidate

November 12, 2026 · L-1f11ar · Synthetic Linux x86_64 input.

The [isolated patch](../../patches/ccusage/20.0.48-claude-json.patch) preserves
Claude usage across valid JSONL whitespace and escaped field names. It retains
the existing typed count and identity rules. Production scanning, the catalog,
and the SRS remain unchanged. This result does not qualify a production pin change.

## Source boundary

The [pinned upstream README](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/claude/README.md)
describes separate daily and full-entry loaders. Both checked the exact bytes
`"usage":{` before typed parsing. Spaces, tabs, or an escaped usage key bypassed
that filter and silently lost otherwise valid observations.

The patch removes those two filters. Existing typed parsing still validates
the record. A narrow map visitor then requires an object at the usage path
that the typed parser selected. It skips unrelated values with `IgnoredAny`.
Arrays and scalars cannot become token objects. An object at another path
cannot authorize them. Escaped keys retain their decoded meaning.

The full-entry path uses `message.usage`. The daily parser can also select
`data.message.message.usage`. AgentProgress remains daily-only. The patch adds
no report, schema, storage path, dependency, account association, or enabled tool.
Each JSONL line must still hold one complete JSON record. This does not add
multiline JSON support.

The [candidate pin](../../tests/tools/fixtures/claude-json-candidate/pin.json)
owns version `20.0.48-openusage.3`, executable and toolchain digests, prices,
build inputs, and the ordered patch chain. Its source manifest covers 537
records. Only the Claude loader files and their new test module differ from
the prior Codex snapshot source. Parent reconstruction applies the patch to
the exact prior bytes and matches the resulting source.

## Executed checks

The parent [public-command suite](../../tests/tools/test_ccusage_claude_json.py)
passes nine tests and 302 scanner commands with private synthetic input.
The shared fixture retains 14,950 tokens across compact, spaced, tabbed,
CRLF, escaped-key, and reordered records. Every category and complete report
matches the compact control. Repeated reports match. Prompt-like strings,
wrong-path usage objects, and non-object usage create no observations.

Unknown metadata with `1e400` or a 140-level array preserves the previously
accepted session and block reports. These controls remove the unused
`parentUuid:null` field to isolate the historical typed-only path. Existing
daily and null-fallback limitations stay unchanged.

The 47-report legacy matrix completes with three intentional Claude whitespace
corrections. The other reports match production. Source and executable hashes
stay unchanged. Seven independent expectations still fail: five malformed-peer
reports and two legacy Codex identity cases. Those failures remain qualification
blockers. Agreement with the old scanner does not establish correctness.

The final source checks pass 57 Claude adapter tests, eight filtered CLI unit
tests, all 16 Claude CLI integration tests, scoped Clippy, and formatting.
The parent Codex selection passes 18 tests with one raw-capture skip. The
Codex snapshot suite passes 12 tests with one raw-capture skip. Pi snapshot
and observation suites pass nine and ten tests. The two skipped cases require
the exact private producer receipt. This candidate does not claim that comparison.

## Review corrections and diagnostic limits

The first removal of the byte filters admitted Serde positional arrays.
Failing controls led to the object-shape guard. The first guard parsed the
whole line into `Value`. Independent review found that it erased valid session
and block observations when unused metadata exceeded that parser's limits.
Four public controls reproduced that regression. The final path-only visitor
passes them without changing the historical null or daily parser.

An initial parent metadata control retained `parentUuid:null`. It reached the
old whole-value fallback and did not isolate the new regression. That failed
control remains separate from the corrected four-case evidence.

The historical Codex mixed-tool equality method also failed on the intentional
Claude whitespace correction. It expects every report to equal the old oracle.
Its test and pin remain unchanged. A separate 19-test selection retains the
Codex record and pin checks, including the visible raw-capture skip. The new
47-report matrix owns the changed Claude comparison contract.

Earlier timing samples cover the superseded whole-value guard. They do not
measure the final visitor. Reference-scale performance remains unexecuted.
The final build uses the shared target, pinned compiler, locked offline inputs,
verified prices, and the source-root SQLite settings. It reuses compiled
outputs. Cargo's temporary lockfile reordering is restored before the final
locked build, which preserves the original bytes.

## Remaining gates

Malformed peers still permit partial legacy reports. Complete capture, producer
qualification, reference performance, production integration, native application
acceptance, packaging, and other platforms remain open. The existing Codex
snapshot path keeps its separate stricter capture contract. This slice does
not replace legacy scanning with that path.

No application source changed. No new desktop build, renderer suite, workspace
milestone, or native journey ran for this candidate-only slice. Earlier app
results keep their original scope. Private receipts use the `l1f11ar` prefix
under `.recovery/audit/`. Raw fixture data and superseded diagnostics remain
private. The broader repository audit remains incomplete.
