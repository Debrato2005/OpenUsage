# ccusage duplicate handling review

Date: November 8, 2026. Scope: copied Amp threads, copied Goose databases, and available upstream remedies.

## Decision

No reviewed newer release or existing mode fixes the Amp case. Keep Amp unqualified for AT-65 until ccusage suppresses copied usage and a replacement pin passes the fixtures.

The official [latest release](https://api.github.com/repos/ccusage/ccusage/releases/latest) is still `v20.0.48`, published November 2, 2026, at 18:22:40 UTC. The [recent releases](https://api.github.com/repos/ccusage/ccusage/releases?per_page=10) contain no newer prerelease.

Current `main` was `5a3d96eacc3c20aea11d7011271d4858e135574c`, dated November 8, 2026, at 03:23:19 UTC. The [comparison with the pin](https://github.com/ccusage/ccusage/compare/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20...5a3d96eacc3c20aea11d7011271d4858e135574c) contains 42 commits. It changes neither Amp nor Goose adapters, their shared report code, nor their discovery code.

## Amp

The root agent reported an executed Linux regression against the real pinned binary. One synthetic thread counted 162 tokens. A byte-for-byte copy with unchanged thread and event IDs raised both daily and session totals to 324. The [regression](../../tests/tools/test_ccusage_formats.py) records this failure.

The current [loader](https://github.com/ccusage/ccusage/blob/5a3d96eacc3c20aea11d7011271d4858e135574c/rust/adapters/amp/src/loader.rs) appends entries from every discovered JSON file. It sorts by timestamp but has no duplicate set. The [parser](https://github.com/ccusage/ccusage/blob/5a3d96eacc3c20aea11d7011271d4858e135574c/rust/adapters/amp/src/parser.rs) retains thread and event/message IDs but emits each eligible event. Ledger precedence only selects between two representations within one thread file.

Both [focused reports](https://github.com/ccusage/ccusage/blob/5a3d96eacc3c20aea11d7011271d4858e135574c/rust/adapters/amp/src/lib.rs) and the [unified loader](https://github.com/ccusage/ccusage/blob/5a3d96eacc3c20aea11d7011271d4858e135574c/rust/crates/ccusage-adapter-all/src/loader.rs) use the same Amp loader. Switching daily/session, focused/unified, cost mode, date filters, or parallel loading does not add identity checks. This is a source conclusion, not a separate execution of every option.

The [path selector](https://github.com/ccusage/ccusage/blob/5a3d96eacc3c20aea11d7011271d4858e135574c/rust/adapters/amp/src/paths.rs) removes identical configured root strings. It does not suppress identical files or events within a root. Thus one selected root does not prevent the reproduced failure.

### Related unmerged work

[Pull request 1472](https://github.com/ccusage/ccusage/pull/1472) proposed server-backed Amp loading. It is closed and unmerged. Its final head is `f3d6653985561c7397444c78dbdd90bc77f9019f`.

Its [loader patch](https://github.com/ccusage/ccusage/pull/1472/files) filters newly appended server entries against local usage keys. It leaves the original local vector intact. Therefore it does not fix two local copies. It also adds authenticated Amp CLI discovery, which expands the offline source boundary. It is not a drop-in remedy for this release.

## Goose

The current [Goose loader](https://github.com/ccusage/ccusage/blob/5a3d96eacc3c20aea11d7011271d4858e135574c/rust/adapters/goose/src/loader.rs) suppresses duplicates by database path plus session ID. The [path selector](https://github.com/ccusage/ccusage/blob/5a3d96eacc3c20aea11d7011271d4858e135574c/rust/adapters/goose/src/paths.rs) canonicalizes candidate paths. Aliases resolving to one path count once. Physical copies at distinct candidate paths remain distinct.

Inference: identical sessions in two copied databases can count twice. This research did not execute that fixture.

A nonempty `GOOSE_PATH_ROOT` selects one `data/sessions/sessions.db`. This supported mode avoids simultaneous discovery of default locations. It does not deduplicate separate consented roots if OpenUsage scans and adds them independently. Restricting the scope to one database cannot establish general copied-log acceptance.

## Required follow-up

[OU-DUP-004 and AT-65](../SRS.md) place within-tool replay and fork handling in ccusage and require copied logs to count once. A local heuristic based on equal totals or session names would create another duplicate engine and could discard distinct usage.

Recommended upstream work:

- Suppress exact Amp event copies before aggregation, across files and roots.
- Test identical, overlapping, older, and changed copies, with and without stable event IDs.
- Define conflicting-ID behavior and preserve distinct events with equal token values.
- Add Goose copied-database tests and resolve identity scope before changing its key.
- Qualify the resulting pin against all AT-65 fixtures and consent boundaries.

No archive, package, or executable was downloaded. No implementation changed, upstream message was sent, or new binary ran during this research. Current-main behavior and Goose overlap remain source conclusions. The root agent's Amp run is separate executed evidence.
