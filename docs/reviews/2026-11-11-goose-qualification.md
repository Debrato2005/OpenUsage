# Goose SQLite qualification

L-1f11ab, November 11, 2026. The pinned Goose scanner follows a database
link outside the selected root. Goose stays disabled.

## Executed reports

The [fixture](../../tests/tools/test_ccusage_goose.py) checks the production
`ccusage` 20.0.48 binary on private synthetic Linux input.
The [qualification record](../../tests/tools/fixtures/goose/qualification.json)
covers 22 SQLite cases and 88 repeated scanner runs.
Five session outcomes match. The empty database matches both views.
The daily diagnostic requires proven allocation of usage to days.
A session creation date does not supply that allocation.

| Scenario | Required session tokens | Pinned result |
| --- | ---: | ---: |
| Single row or exact row replay | 161 | 161 |
| Two distinct calls with equal counts | 322 | 322 |
| Conflicting rows with one ID | Unavailable | 161 or 179 by row order |
| Synthetic inherited counter with 17 new tokens | 178 | 339 |
| Explicit zero accumulator without a coverage rule | Unavailable | 150 from base counters |
| Positive total below input plus output | Unavailable | 150 |
| Missing model or date | 161 with unavailable details | No rows |
| Missing cache measurements | 161 with unavailable cache counts | 161 with zero cache counts |
| Invalid database or missing required column | Unavailable | Empty report |

The inherited-counter case tests known synthetic lineage.
It does not establish the official fork's counter behavior.
Negative, fractional, and text counters also produce reports.
A positive total with missing categories yields numeric zero categories.
Missing measurements must stay unavailable under
[SC-022](../srs-clarifications.md#sc-022-incomplete-metrics-from-the-other-local-tools).

Two additional tests execute both report views.
Two byte-identical databases in different default paths give 322 tokens.
A link from the selected database to a separate synthetic mount gives 161.
The link test supplies only synthetic files. Source bytes stay unchanged.
The normal explicit override accepts one root.
These results establish scanner behavior in isolation.

## Source and next gate

The pinned [path resolver](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/goose/src/paths.rs)
canonicalizes database candidates before it checks the file type.
The [loader](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/goose/src/loader.rs)
uses database path and session ID for identity, then keeps the first row.
It discards read and query failures.

The producer review pins Goose v1.59.0 at
[`5d03ef6e`](https://github.com/aaif-goose/goose/tree/5d03ef6ed07ba59826328f1aff18d9a754ff8ad2).
[Session IDs](https://github.com/aaif-goose/goose/blob/5d03ef6ed07ba59826328f1aff18d9a754ff8ad2/crates/goose/src/session/session_manager.rs#L1831)
use a UTC date and a database-local counter. IDs can overlap across stores.
The [copy operation](https://github.com/aaif-goose/goose/blob/5d03ef6ed07ba59826328f1aff18d9a754ff8ad2/crates/goose/src/session/session_manager.rs#L2895)
creates a new ID and copies the transcript. It starts without inherited counters.
The [import operation](https://github.com/aaif-goose/goose/blob/5d03ef6ed07ba59826328f1aff18d9a754ff8ad2/crates/goose/src/session/session_manager.rs#L2849)
also creates a new ID and preserves accumulated counters.
The import therefore lacks a stable usage link to the original store.

The [usage ledger](https://github.com/aaif-goose/goose/blob/5d03ef6ed07ba59826328f1aff18d9a754ff8ad2/crates/goose/src/session/session_manager.rs#L1229)
has database-local row IDs and nullable token fields. Imports omit ledger rows.
When the writer finds a gap between cumulative counters and ledger totals,
it can write a [carried-forward row](https://github.com/aaif-goose/goose/blob/5d03ef6ed07ba59826328f1aff18d9a754ff8ad2/crates/goose/src/session/session_manager.rs#L2681).
That row's timestamp records the later write. Earlier usage dates remain unavailable.
The producer review used pinned source. All executed fixtures use synthetic SQLite.

A candidate needs a complete capture from the approved root.
It must open the database and journal files relative to a held root,
refuse links and changes, and reject invalid SQLite input.
The existing [OpenCode capture](2026-11-11-opencode-capture-candidate.md)
and [WAL review](2026-11-11-opencode-wal-candidate.md) record the relevant Linux mechanisms.
Goose also needs a qualified usage scope and identity rule.
