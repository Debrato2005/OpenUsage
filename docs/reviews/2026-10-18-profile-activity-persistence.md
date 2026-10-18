# Saved profile activity

October 18, 2026. Read activity now saves a validated reading before it reports success. Saved activity loads after unlock, including archived profiles, without another executable approval. Three summary values remain visible first. Other values and saved daily rows stay under Activity details. JetBrains Mono and the shared coss controls remain in use.

SQLCipher 4 with WAL remains mandatory. Migration 2 adds local association events, collection runs, summary snapshots, current daily totals, daily history, and a current-reading pointer. Rust keeps account descriptions as provisional evidence. Email never becomes a verified identity key. Profiles do not merge by email, and these readings contribute no verified combined totals.

The native coordinator holds its request, approval, generation, and physical-home checks through the storage commit. Each accepted reading stores its provenance, summary, daily replacements, and history in one transaction. Repeated daily totals replace the previous total without adding it again. Null or empty daily responses retain earlier dates and their observation times. Independent missing summary fields remain unavailable.

The first requested read can establish an empty local association before the metric transaction. This alone never produces an accepted reading.

Rust computes freshness for the summary and each retained row. A restart or restore returns unknown freshness until a new successful read. A failed request preserves saved values and marks their status stale. An exact request check prevents an obsolete failure from changing a later success. The renderer reloads that status after errors. A background cache read cannot steal focus from a manual result.

Backup and restore copy all six activity tables through trusted SQL and bound values. Valid schema 1 images first copy into a hardened private memory database and receive trusted migrations. Archive SQL never runs. Source archives and checkpoints remain unchanged. Future schemas remain unsupported. A real disk-full test preserves every previous record and allows retry. Process-crash tests cover all six tables before and after commit.

The [artifact audit](../evidence/native-activity-persistence-verification/artifact-audit.json) binds source `a94d49b`, 185 source and configuration files, ten built assets, and separate immutable native-test and default-production executables. The main native journey passed eight committed reads, 17 independent storage or archive checks, and three genuine window closes with restart. Exact values, two-profile isolation, cancellation, failure preservation, fresh-home refusal, and complete six-table backup/checkpoint round trips passed. Thirteen captures cover those flows. A focused three-capture supplement verifies the latest missing-daily notice beside retained rows. The original journey's notice capture showed middle rows only.

The default production executable separately passed setup with a real private GNOME key store. Daemon/application restart, encrypted main/WAL checks, private permissions, and unkeyed SQLite refusal passed. Both fresh captures were reviewed. It also refused all four synthetic providers before execution. These are Linux debug artifacts. The preceding optimized executable retains its earlier scope.

All 149 Rust workspace checks, strict Clippy, and formatting passed. One owned child entry remains intentionally ignored and executes through its parent. The exact integrated renderer sources match the successful 29-contract and 38-renderer check across Chromium and WebKit. The integrated frontend build also passed.

Three early test failures changed only SHM bytes. Their database and WAL hashes matched. SQLite documents SHM as coordination state, including reader marks. The corrected checks preserve exact database/WAL bytes and inspect logical records. Original failures remain retained. See [SQLite WAL format](https://www.sqlite.org/walformat.html#the_wal_index_or_shm_file).

The normative SRS is unchanged. Actual activity values use labeled synthetic fixtures. Real OAuth, live activity, verified identity, scheduling, history browsing, broader accessibility, other platforms, and performance acceptance remain incomplete. Current-day IPC is bounded to 4096 rows with an explicit truncation flag. History retention has no such bound.

Fresh homes restored from an archive cannot reuse an old association. A separate explicit reassociation workflow remains necessary. The actual retained schema 1 archive GUI regression is the next slice. Existing schema 1 qualification here is Rust-level compatibility.
