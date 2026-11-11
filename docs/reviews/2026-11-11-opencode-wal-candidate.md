# OpenCode WAL capture candidate

L-1f11s, November 11, 2026. This local layer adds committed WAL capture after
the [strict capture candidate](2026-11-11-opencode-capture-candidate.md).
OpenCode stays disabled while producer and app qualification remain open.

## Captured database state

The Linux capture pairs each main database with its exact WAL and SHM filenames
inside the same held root. Channel names can contain dots, as the pinned
producer permits. Orphan peers and rollback journals return unavailable.
All source bytes remain under the existing joint capture limits and exact-byte
rechecks. A changing WAL or SHM can reject a capture.

The reconstruction checks the WAL header, page size, salts, rolling checksums,
and complete frames. It applies only frames through the last commit. Valid
uncommitted frames supply no usage. Reverse replay keeps the latest committed
page and respects a previous truncation. The private image then uses the
existing read-only SQLite interface. The scanner never opens a source database
through SQLite and never writes raw contents to disk.

Each reconstructed image is at most 64 MiB. Its page-offset index is at most
512 KiB. The scanner reconstructs one database at a time. Captured main, WAL,
and SHM bytes share the prior file, count, and 256 MiB limits. This layer adds
no dependency. A checkpointed WAL-mode database can work without peers.

The candidate conservatively refuses damaged, torn, or old-generation WAL
tails, even when SQLite could read a valid committed prefix. In particular, it
can reject an ordinary reused WAL file. This can reduce availability. It does not turn a failed scan into zero usage.
[SQLite file format](https://sqlite.org/fileformat.html#walformat),
[WAL recovery](https://sqlite.org/walformat.html#recovery)

## Producer evidence

The source fixture pins OpenCode v1.19.5 at
`810c0eba1a7348036f8bdf61e6632d0c36c6876c`. Its database setup enables WAL by
default. The pinned generated schema uses `session_message` and `session`.
The [fixture](../../tests/tools/fixtures/opencode-producer/pin.json) checks
fourteen source files before it creates private SQLite tables from the
generated declarations. Its message payloads are synthetic.
[Database setup](https://github.com/anomalyco/opencode/blob/810c0eba1a7348036f8bdf61e6632d0c36c6876c/packages/core/src/database/database.ts#L27),
[generated schema](https://github.com/anomalyco/opencode/blob/810c0eba1a7348036f8bdf61e6632d0c36c6876c/packages/core/src/database/schema.gen.ts#L170),
[message schema](https://github.com/anomalyco/opencode/blob/810c0eba1a7348036f8bdf61e6632d0c36c6876c/packages/schema/src/session-message.ts#L165)

This checks storage shapes, not producer-runtime execution. The current
producer can also emit metadata messages such as `agent-switched`. The strict
loader still refuses that valid shape. An unfinished assistant without token
evidence also returns unavailable. These outcomes remain explicit qualification
limits. Producer event execution, fork behavior, metadata mapping, scale,
grouped storage, native OpenCode integration, real logs, Windows, and macOS
remain unqualified. This layer does not complete AT-65 or change the production pin.


## Checks and build

All 108 adapter tests, selected Clippy, formatting, and the locked offline
release build passed. The [CLI checks](../../tests/tools/test_ccusage_opencode_wal.py)
passed all thirteen tests with the source fixture enabled. They cover the prior
26 required outcomes, sixteen format baselines, and enabled Claude Code and
Codex reports. Real SQLite fixtures cover 512, 4096, and 65536-byte pages,
multiple commits, copied roots, uncommitted spill, checkpoints, and damaged WAL
files. Source files remain unchanged. The adapter checks also cover both
checksum orders, truncate-and-grow sequences, peer replacement, missing pages,
and rejection of ordinary old-generation tails.

Apply the WAL patch after the observation and capture patches on the same
pinned ccusage source. Build with verified offline pricing, the shared target,
and `CCUSAGE_VERSION=20.0.48-openusage.opencode-wal-candidate.1`.
The lockfile remains unchanged from the capture layer.

Set `OPENUSAGE_CCUSAGE_OPENCODE_WAL_CANDIDATE` to the release binary.
Set `OPENUSAGE_CCUSAGE_OPENCODE_WAL_CANDIDATE_SHA256` to its checked digest.
Set `OPENUSAGE_OPENCODE_PRODUCER_SOURCE` to the pinned source to include the
storage-schema checks. Run from the project root:

```sh
python3 -m unittest discover -s tests/tools -p test_ccusage_opencode_wal.py -v
```

Patch [20.0.48-opencode-wal.patch](../../patches/ccusage/20.0.48-opencode-wal.patch)
has SHA-256 `361ec132cbddc0601f452be6e4694790b2cf127ab66b77e6d49e6a9462d307ee`.
The release binary has SHA-256
`1656830b342dbd4680677b761d352de70f1ebff8b45443ccb3669f0a8057cb66`.
