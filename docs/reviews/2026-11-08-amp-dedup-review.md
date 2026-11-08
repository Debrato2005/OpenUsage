# Amp duplicate candidate review

Date: November 8, 2026. Scope: independent source review of the local candidate, before its I/O error fixes.

## Result

Require file-read and directory-discovery errors to fail the Amp scan before qualification. Both paths can currently return successful partial totals. These behaviors come from the pinned source. The new merge does not introduce them, but they conflict with the candidate's conservative whole-scan contract.

No additional false merge, fabricated cache join, or lost count was found in the inspected stable-ID merge path. This conclusion applies to equal source payloads within one thread. It does not qualify anonymous replay, mixed representations, or copied fork history across thread IDs.

The primary agent reported 22 passing Amp tests and 150 CLI fixture invocations. This reviewer did not execute them, run Cargo, or run either scanner. The candidate remains unbundled and unpublished. [Candidate tests](../../tests/tools/test_ccusage_amp_candidate.py), [SC-023](../srs-clarifications.md#sc-023-copied-amp-usage-in-the-pinned-scanner)

## Inspected artifact

The local checkout HEAD is `8995f3b7fd7110b8f37564f7d0468acc7a7d6b20`, the pinned ccusage 20.0.48 source. Graphify ran before the source review. The review covered the candidate parser, merge module, loader, loader tests, Amp report path, and shared discovery and JSON helpers.

The reviewed [patch](../../patches/ccusage/20.0.48-amp-duplicates.patch) had SHA-256:

```text
623175a80abc9d7129c3726fc5d9b27e8cfe69e08098a5580cc72b186b198c8c
```

`git apply --reverse --check` succeeded against the local candidate. This was a read-only correspondence check. It did not apply or remove the patch. The primary agent then started the required fixes below. Those later edits are outside this review snapshot.

## Required fixes

| Finding | Concrete synthetic case | Required result |
|---|---|---|
| **P1: File failures disappear before merge.** The [candidate loader](/home/debrato/Projects/OpenUsage/.tools/upstream/ccusage-8995f3b7/rust/adapters/amp/src/loader.rs:26) converts read errors to `None`. The [snapshot reader](/home/debrato/Projects/OpenUsage/.tools/upstream/ccusage-8995f3b7/rust/adapters/amp/src/parser.rs:105) also converts malformed JSON to `None`. | One valid snapshot contains event A with 162 tokens. A second snapshot contains A and new event B, but its JSON is truncated. The candidate can return A's 162 tokens successfully. An unreadable second file has the same omission path. | Propagate a failure before report output. Cover malformed input alone, malformed input beside valid data, and a direct read error. Check focused and unified commands, both loading modes, nonzero exit, and no partial stdout. |
| **P1: Discovery failures disappear before file reads.** The candidate still calls the [shared walker](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/common/src/lib.rs#L21-L47). It ignores `read_dir`, directory-entry, and file-type errors. | A selected root has a readable snapshot A and an inaccessible `threads/archive` directory containing B. The walker omits the directory and the candidate can report only A. A file-read fix cannot detect a file that discovery never returns. | Use checked Amp discovery or an equivalent error path. Distinguish an intentionally absent source from a present source that cannot be inspected. A test must create an actual error under the executor's permissions. |

The [pinned loader](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/amp/src/loader.rs) and [pinned parser](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/amp/src/parser.rs) already skipped these file failures. The required change follows the selected fail-whole-scan policy. It is not evidence that the new identity merge corrupts a record.

Valid metadata without a thread ID can remain skippable. Do not turn every unrelated JSON document into a usage failure. Separately retain the documented pinned behavior for non-usage records and unavailable metrics. The [lenient helpers](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/common/src/jsonl.rs) still skip malformed nested records or normalize missing fields. This patch does not establish complete source-format validation.

## Merge reasoning

The [merge module](/home/debrato/Projects/OpenUsage/.tools/upstream/ccusage-8995f3b7/rust/adapters/amp/src/parser/dedup.rs:168) groups by normalized thread ID. It keeps ledger and message representations separate. Numeric message ID `7` and string ID `"7"` remain separate keys. Distinct IDs with equal counts remain separate. Equal known IDs suppress a copy only after their retained source payloads agree. Estimated cost does not select identity or a winner.

Ledger cache handling uses each snapshot's local messages before combining snapshots. The [attachment check](/home/debrato/Projects/OpenUsage/.tools/upstream/ccusage-8995f3b7/rust/adapters/amp/src/parser/dedup.rs:97) distinguishes a missing join from an attached message with missing cache fields. It rejects changed attachments across snapshots. Only messages referenced by that snapshot's ledger enter the merged cache map. Thus an unreferenced later cache message cannot fill an earlier missing join.

For each accepted event, the later [numeric parser](/home/debrato/Projects/OpenUsage/.tools/upstream/ccusage-8995f3b7/rust/adapters/amp/src/parser.rs:154) receives the same token fields and cache values as the source snapshot. Identical copies collapse before aggregation. Growing snapshots form a union of stable IDs. Conflicting values return an error before any merged entries reach the report.

File paths and known identities use ordered collections. The parser sorts entries by time, thread, and normalized output ID. The [parallel reader](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/common/src/lib.rs#L124-L171) restores the original file-index order. Numeric/string IDs can later render as the same text, but the merge already retains both entries. Their stable map order remains intact when the final sort keys tie.

These are source deductions. The [loader tests](/home/debrato/Projects/OpenUsage/.tools/upstream/ccusage-8995f3b7/rust/adapters/amp/src/loader/tests.rs:78) cover copies, growing and partial overlaps, typed IDs, changed payloads, missing cache joins, and report ordering. The tests do not replace the remaining qualification gates.

## Approved failures and remaining limits

Missing stable IDs across snapshots, mixed ledger/message snapshots, changed usage, and changed cache attachments intentionally fail. Do not resolve them with file order, modification time, equal totals, or per-field maxima. Those choices can discard distinct usage or combine conflicting revisions. [Design boundary](2026-11-08-amp-dedup-design.md#proposed-minimal-boundary)

The anonymous-record rule is broader than detected overlap. Any second snapshot for that thread fails when the group contains anonymous usage, even if the second snapshot is empty. Likewise, conflicting unreferenced cache messages can fail one ledger snapshot. These are conservative failures, not false merges. Describe and test them so future maintenance does not mistake them for accidental behavior.

One anonymous snapshot still preserves every accepted occurrence. Repeated anonymous entries inside that file can therefore count twice. Different thread IDs also remain separate, including copied fork prefixes. These cases remain unqualified under [OU-DUP-004 and AT-65](../SRS.md#5d-duplicates). The candidate fixes stable-ID copies. It does not establish all replay, fork, or compaction semantics.

Derived `PartialEq` deliberately compares retained source fields before numeric fallback. This protects absent-versus-zero checks. It can also reject changes to unused fields inside a raw usage object. Keep that conservative behavior explicit. A future parser-field change must review its effect on identity equality. The candidate also retains all parsed snapshots before merging, so large-root memory and deadline behavior need measurement.

## Acceptance gaps

1. Execute the file and discovery failure regressions after the fixes. Recheck that valid metadata without an ID remains skippable.
2. Exercise typed message IDs, mixed kinds, anonymous overlaps, and cache conflicts through the CLI. Add focused weekly output and zero, unknown-price, and nonzero-price cases.
3. Check tied timestamps with multiple models and credits under reversed file/root order. Compare complete JSON, not only token totals.
4. Qualify copied, forked, replayed, and compacted histories separately. Keep unsupported cases explicit. Future OpenUsage calls must supply the same tool's consented roots together.
5. Before replacing the pin, record candidate provenance and run the affected fixture, process-isolation, and native persistence gates. This review does not authorize bundling or publication.

No implementation files changed during this review. No real logs or credentials were read.

## Addendum: I/O and explicit-root fixes

This source review closes the two required I/O findings above. It also closes a root-selection gap found during the follow-up. No additional false merge or privacy regression was found in these edits. The merge module is unchanged. These conclusions concern the inspected source, not a rebuilt CLI or native integration.

The primary agent reported failing regressions before each fix. It then reported 28 passing Amp tests and a passing changed-crate Clippy check. This reviewer read the final source and tests, but ran no Cargo command or scanner. The previous 150 CLI invocations cover the earlier candidate. They do not certify these later edits. The patch artifact was stale during this review.

The reviewed source files have these SHA-256 values:

```text
loader.rs  7bb9ee3a42d40833bd0b63d04f9c16204c2d803aaa8ffa11209138a9d092e36e
parser.rs  8a7a2df5efbab4cf9a12447aca3dabf14d4f5e33d202d753f97b7e81b7c4a1aa
paths.rs   c64da19284f5fde813d97b079782a213390c52bef045552288aba374c057938d
```

The [loader](/home/debrato/Projects/OpenUsage/.tools/upstream/ccusage-8995f3b7/rust/adapters/amp/src/loader.rs:22) now collects snapshot results as `Result<Vec<_>>`. A failed read prevents the merge and report. Its Amp-specific walker propagates directory, directory-entry, and file-type errors. It permits `NotFound` only for the top-level `threads` directory. Recursive directory failures remain errors. Directory entries that are symbolic links remain outside traversal, as before.

The [snapshot reader](/home/debrato/Projects/OpenUsage/.tools/upstream/ccusage-8995f3b7/rust/adapters/amp/src/parser.rs:105) propagates file and JSON errors with fixed messages. Those errors include neither source contents nor filesystem paths. A parsed document with usage but no thread ID also fails. A valid metadata object without usage or identity remains skippable. The `lib.rs` edit only removes the old walker import.

The additional root gap came from the inherited `path.is_dir()` filter. It could silently remove an invalid explicit component before checked discovery. The inherited `env::var` branch could also substitute the default home after a non-UTF-8 override. Both paths could bypass the intended scope of an explicit selection.

The final [path reader](/home/debrato/Projects/OpenUsage/.tools/upstream/ccusage-8995f3b7/rust/adapters/amp/src/paths.rs:7) uses `var_os` and rejects a non-UTF-8 override. Every nonempty explicit component must pass directory metadata checks. A missing or ordinary-file component therefore fails even beside a valid root. An empty override selects no roots. With no override, only a missing default root counts as absent. Other default-root errors fail. Error messages do not expose the supplied path.

The new [unit cases](/home/debrato/Projects/OpenUsage/.tools/upstream/ccusage-8995f3b7/rust/adapters/amp/src/loader/tests.rs:290) cover malformed snapshots alone and beside valid data, missing identity, denied files and subdirectories, and skippable metadata. They also cover invalid explicit roots and a non-UTF-8 override beside a populated synthetic default home. The [CLI cases](../../tests/tools/test_ccusage_amp_candidate.py) assert no partial stdout, a failure exit, unchanged source files, and no `private-content` marker in stderr. Permission tests need an executor that actually receives permission denial.

Correction to acceptance item 2 above: `amp weekly` is unsupported. Amp uses [STANDARD_AGENT_REPORTS](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/crates/ccusage-cli/src/types.rs#L215-L219), which contains daily, monthly, and session. Internal weekly aggregation tests do not establish a focused weekly CLI command. Qualify the supported focused commands and unified reports without adding a new command requirement.

The bounded remaining gates for these fixes are:

1. Rebuild the candidate CLI and run the expanded failure matrix against its recorded hash. Include supported focused commands and unified daily output.
2. Retain explicit checks for empty override, absent default root, valid roots without `threads`, and rejected invalid selected roots. Keep default-home sentinel data private and synthetic.
3. Regenerate the patch and check it against the reviewed source. Record new build evidence before any pin replacement.

The earlier limits for anonymous replay, cross-thread forks, nested parser leniency, and native integration remain. This addendum does not qualify those cases or authorize publication.
