# Legacy JSONL capture candidate

November 13, 2026 · L-1f11as · Private synthetic Linux x86_64 input.

The [successor patch](../../patches/ccusage/20.0.48-claude-capture.patch) makes five legacy reports refuse malformed peers. These are Claude daily, session, and blocks, plus Codex daily and session. No production scanner pin or enabled tool changes.

## Source boundary

The candidate extends the [Claude JSON candidate](2026-11-12-claude-json-candidate.md). Invalid UTF-8, malformed JSONL, required file-read failures, and truncated Codex compressed input now reach the report caller. Failed reports emit no JSON payload. Well-formed records still follow the existing typed schema and object guards. Blank lines carry no records.

The common syntax check uses `IgnoredAny`, so unused large numbers and nested metadata do not require value conversion. The patch does not change discovery, date pruning, replay planning, provider identity, or atomic filesystem capture. Those limits remain explicit in the [pin](../../tests/tools/fixtures/claude-capture-candidate/pin.json).

The final patch covers seven source files, including two successor-only test corrections. Parent tests and pins remain unchanged. The 537-record source manifest matches the reconstructed source with zero fuzz. The final locked offline build preserves the parent's Cargo.lock bytes and reproduces the qualified executable digest.

## Executed evidence

| Boundary | Result |
|---|---|
| New public-command and reconstruction checks | Eleven tests pass across selected runs, covering 422 scanner commands |
| Inherited Codex record and snapshot checks | Nineteen and thirteen tests pass, including both previously skipped raw synthetic producer comparisons |
| Inherited Pi snapshot and observation checks | Nineteen tests pass |
| Source adapters | Claude 58, Codex 163, and common 19 tests pass |
| Source CLI integration | Claude 16 and Codex 7 tests pass |
| Scoped no-deps Clippy, formatting, final build | Exit 0 |

The initial public-command check failed all five malformed-peer expectations against the parent oracle. The new candidate passes them. Valid controls include final records without newlines, whitespace, escaped keys, wrong usage paths, and unused metadata. Serial reads, unreadable peers, corrupt compressed peers, and a valid compressed frame followed by a truncated frame are covered.

The exact synthetic producer receipt matches the digest in the tracked producer fixture. Both inherited comparisons checked each captured file's digest and length. This does not involve the owner's real accounts or logs.

Two old source tests expected successful partial results. Their successor versions retain valid controls and require refusal for malformed JSON or truncated compressed input. The first source runs recorded 56/57 Claude and 162/163 Codex results before those expectation corrections.

A broader Clippy invocation stopped on an unchanged `needless_borrows_for_generic_args` warning in `ccusage-core/src/pricing.rs:1781`. The final affected-adapter check uses `--no-deps`. This is a scoped result, not a clean full upstream lint result.

## Remaining gates

| Legacy matrix case | Expected tokens | Candidate tokens |
|---|---:|---:|
| Codex renamed copy, session | 4,600 | 9,200 |
| Codex distinct equal usage, daily | 9,200 | 4,600 |

The 47-report matrix changes only the five malformed-peer outcomes relative to its parent. These two identity failures remain. Atomic capture, discovery, reference performance, production integration, native application acceptance, packaging, and other platforms remain unverified.

The required read-only oracle has SHA-256 `b38a905ef951015cd981e068ea73cd608d97f86db9d35b33b831737cf116a187`. The pin owns all other digests, build inputs, and boundaries. Private run metadata remains under `.recovery/audit/l1f11as-*`. No new desktop or real-provider acceptance is claimed for this scanner-only slice.
