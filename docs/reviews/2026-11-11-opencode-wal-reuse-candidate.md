# OpenCode WAL reuse candidate

L-1f11t, November 11, 2026. This layer corrects ordinary WAL reuse after a
checkpoint. It follows the [WAL candidate](2026-11-11-opencode-wal-candidate.md).
OpenCode stays disabled while producer and app qualification remain open.

## Committed frontier

SQLite can reuse a WAL file without shortening it. Old-generation frames can
remain after the current generation. The candidate accepts those leftovers
only when captured shared-memory metadata matches the current committed boundary.
[SQLite WAL recovery](https://sqlite.org/walformat.html#recovery)

The capture requires identical, initialized, checksum-valid shared-memory
headers. It matches the WAL version, page size, checksum order, salts, last
committed frame, database page count, and frame checksum. It still checks
every current-generation frame. A later commit beyond the captured boundary
returns unavailable. Valid uncommitted frames supply no usage.

Missing or inconsistent shared-memory metadata keeps reuse unavailable. Torn
files, current-generation checksum failures, and a reappearance of current
salts after old frames also return unavailable. The obsolete payload bytes
cannot be independently checked after their original prefix was overwritten.
Their exclusion relies on the matched committed boundary. It does not prove
that old payload bytes remain intact.

The prior capture limits, exact-byte rechecks, in-memory reconstruction, and
read-only SQLite interface remain in force. No source writes, disk copies, or
new dependencies take part. Producer-runtime execution, metadata mapping,
fork behavior, scale, native OpenCode integration, real logs, Windows, and
macOS remain unqualified. This layer does not complete AT-65 or change the
production scanner pin.

## Checks and build

All 115 adapter tests, selected Clippy, formatting, and the locked offline
release build passed. The [CLI checks](../../tests/tools/test_ccusage_opencode_wal_reuse.py)
passed all sixteen tests. They preserve the prior matrix, format comparisons,
enabled tools, and pinned storage-schema fixture. Four real checkpoint/reuse
cycles at each of three page sizes leave 36 obsolete frames and exactly 241
committed tokens. A real uncommitted spill keeps 190 tokens while thirteen old
frames remain. Missing, mismatched, and stale shared-memory headers, a later
current-generation commit, damaged current frames, and torn tails return
unavailable. Source hashes remain unchanged.

Apply this patch after the observation, capture, and WAL patches on the same
pinned ccusage source. Build with verified offline pricing and the shared
target. Set `CCUSAGE_VERSION=20.0.48-openusage.opencode-wal-reuse-candidate.1`.
The lockfile remains unchanged.

Set `OPENUSAGE_CCUSAGE_OPENCODE_WAL_REUSE_CANDIDATE` to the release binary.
Set its `_SHA256` variable to the checked digest. Keep
`OPENUSAGE_OPENCODE_PRODUCER_SOURCE` set to the pinned source to include its
storage-schema fixture. Run from the project root:

```sh
python3 -m unittest discover -s tests/tools -p test_ccusage_opencode_wal_reuse.py -v
```

Patch [20.0.48-opencode-wal-reuse.patch](../../patches/ccusage/20.0.48-opencode-wal-reuse.patch)
has SHA-256 `a10d4f822e911ff88708c6a76cec59617002e430ce6d1a74f62f8ea836ce88d0`.
The release binary has SHA-256
`fa337e6ffc71c56f4647feec0450b47821a7b5f8b4ec1ad7b18ff95ec5f44a8b`.
