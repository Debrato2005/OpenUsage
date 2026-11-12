# Legacy Codex identity experiment

November 13, 2026 · L-1f11at · Private synthetic Linux x86_64 input.

This experiment is unqualified. The [patch](../../patches/ccusage/20.0.48-codex-legacy-identity.patch) corrects two independent duplicate-count cases, but it also rejects supported legacy histories. Production pins, enabled tools, application dispatch, and saved records remain unchanged. Do not use its clean numeric matrix as proof of compatibility.

## Recorded identity boundary

The experiment extends the [legacy capture candidate](2026-11-13-legacy-capture-candidate.md). The first `session_meta.payload.id` identifies a thread. Equal complete decoded record streams and exact record prefixes select one longest capture. Divergent streams with the same ID refuse. Different recorded IDs remain separate despite equal timestamps or counts.

Missing or ambiguous identity, duplicate JSON keys, unsupported lineage, subagents, replay, compaction, reverts, and unexplained cumulative decreases refuse. The code infers no account identity or response identity. Session rows change from path keys to recorded thread IDs. Production adoption needs a saved-session migration and alias review.

The [pin](../../tests/tools/fixtures/codex-legacy-identity-candidate/pin.json) owns source, executable, build, and limit records. The 538-file source manifest reconstructs with zero fuzz. The final locked offline build preserves the source lockfile and matches the tested executable. No clean final rebuild was claimed.

## Executed evidence

| Check | Result |
|---|---|
| Public commands | Twelve tests pass, with 496 scanner commands |
| Pin and source reconstruction | Two tests pass |
| Shared matrix | All 47 reports meet independent numeric expectations. Two numeric corrections and eight session-metadata changes are named |
| Inherited Codex record and snapshot checks | Nineteen and thirteen pass, including both exact raw synthetic producer comparisons |
| Pi checks | Seventeen pass. Two historical mixed-tool comparisons fail because they require old Codex path metadata |
| Historical Codex source checks | Eighty-five pass and seventy-eight fail |
| Historical CLI checks | Claude passes all sixteen. Codex fails all seven |
| Capture security controls | Both existing controls pass |
| Scoped no-deps Clippy, formatting, final build | Exit 0 |

The renamed-copy session case now keeps 4,600 tokens. The distinct equal-usage daily case keeps 9,200. These narrow corrections do not resolve the failed historical suite.

## Compatibility audit

Forty-eight additional commands compared the existing parent and experiment binaries without source edits or builds. A complete first-ID control retains 4,600 tokens in both. The experiment refuses ordinary first-ID histories with last-only counters, total-only counters, or omitted reasoning counts.

The experiment also refuses that valid control after adding unrelated `notes.zst`, an unused `1e400` value, or a 140-level unused array. The parent keeps 4,600 tokens for each. These are compatibility regressions beyond the intended identity correction.

Historical path and basename `--id` aliases fail on the complete first-ID control. The recorded UUID and `codex://threads/UUID` selectors still work. Qualification must address these aliases and prevent old path keys from becoming duplicate saved sessions.

The source failures include 46 identity-free fixtures, 29 lineage or replay scopes, two ordinary first-ID last-only histories, and one mixed test. That mixed test includes missing-parent lineage and a later CLI case with paired counters but omitted reasoning. Six CLI failures use identity-free helpers. The seventh uses recorded-ID subagent lineage. These groups describe inputs and applicable guards, not an instrumented first-failure trace.

Additional schema-shaped 0.163.0 synthetic controls also confirm the unrelated-file and unused-large-number regressions. They contain unambiguous recorded identity, but they are not producer captures.

## Remaining work

L-1f11at remains open. A separate strict legacy command could preserve ordinary parent routing while the new identity contract matures. This is a proposal, not an implemented correction. Valid history support, source discovery, metadata handling, and stored-session migration need further review before activation.

The synthetic legacy fixtures declare producer version 0.166.0. The qualified captured producer is 0.163.0 with paginated records. Legacy producer qualification remains open. Separate legacy reports still lack one joint capture. Reference performance, native application acceptance, packaging, and other platforms also remain open.

The read-only oracle has SHA-256 `2ee2b9d1251a06484f40dd5b102408d6ba50408765b8a230ef78e210b880fb18`. Private evidence remains under `.recovery/audit/l1f11at-*`, including source checks, public checks, audit probes, and the complete failure classification. The tracked test and pin preserve the experiment for the next authorized slice.
