# OpenCode capture candidate

L-1f11r, November 11, 2026. This local candidate adds strict capture after the
[observation patch](../../patches/ccusage/20.0.48-opencode-observations.patch).
All 26 cases from the [qualification record](2026-11-11-opencode-qualification.md)
now return their required total or refusal. These are synthetic consumer fixtures
on Linux. OpenCode stays disabled.

## Capture boundary

Explicit selected roots use held directory handles and Linux `openat2`.
The capture refuses links, hard links, special files, missing roots, unreadable
inputs, malformed records, conflicting IDs, and unresolved fork boundaries.
It checks the complete manifest, root identity, file identity, and captured bytes
before it returns a report. Controlled replacements and rewrites return an error.
This does not prove completion by an arbitrary concurrent writer.

Matching IDs count once only when their validated source records agree.
Fork suppression also requires matching copied records and consistent lineage
across selected roots. Partial token categories need an independent total.
Present counts must be unsigned integers. Duplicate JSON keys reject the capture.
Missing time keeps the established unplaced scope under SC-022.

SQLite reads only captured rollback-journal database bytes through a private
read-only virtual filesystem. It never opens the source database through SQLite.
The interface rejects writes, outside attachments, temporary files, and sidecars.
The backing bytes outlive the connection, which closes before the interface unregisters.
The build omits several SQLite PRAGMAs, so the safety boundary does not depend
on those PRAGMAs. WAL headers and journal, WAL, or shared-memory peers return
unavailable. Committed WAL content is never silently omitted.

| Joint limit | Value |
|---|---|
| Selected roots | 64 |
| Directory entries | 32,768 |
| Source files | 8,192 |
| Directory depth | 64 |
| One database | 64 MiB |
| One JSON file or SQLite value/row | 1 MiB |
| Captured source bytes | 256 MiB |
| Record visits | 100,000 |
| Canonical identity bytes | 64 MiB |

An explicit empty root now reaches the strict loader. The scanner marks that
source as detected for the table title. It still returns no usage rows. Explicit totals retain
unassigned remainders outside output. This intentionally replaces the earlier
fallback for fully attributed records. OpenUsage already discards OpenCode
token buckets and estimates under SC-022. Covered aggregates do not contribute
to overflow checks after message evidence replaces them.

## Checks and remaining gates

All 96 adapter tests, selected Clippy, and the release build passed.
The [CLI comparison](../../tests/tools/test_ccusage_opencode_capture.py) passed
all nine tests. It checks the 26 required outcomes in both report modes,
all sixteen recorded format baselines, and enabled Claude Code and Codex reports.
It also checks invalid database types, views, counts, duplicate keys, links,
FIFOs, journal peers, committed and checkpointed WAL refusal, and the exact
1 MiB JSON boundary. Source hashes remain unchanged and private homes stay empty.

The adapter checks include direct refusal of writes, outside attachments,
temporary opens, and deletes through the private SQLite interface. They also
check controlled source changes, joint limits, nested forks, native-session
links, date windows, aggregate fallback, and final-total overflow.
Two existing fixtures now include complete zero buckets and matching fork-copy
costs. Their linkage and fork assertions remain. Parser snapshots record the
intentional removal of output inference.

Apply the capture patch after the observation patch on ccusage
`8995f3b7fd7110b8f37564f7d0468acc7a7d6b20`. Build with verified offline pricing,
the shared Cargo target, and
`CCUSAGE_VERSION=20.0.48-openusage.opencode-capture-candidate.1`.
Rustix 1.1.5 and sqlite3-sys 0.18.2 become direct dependencies already present
in the lockfile. No package version or source changes.

Set `OPENUSAGE_CCUSAGE_OPENCODE_CAPTURE_CANDIDATE` to the release binary.
Set `OPENUSAGE_CCUSAGE_OPENCODE_CAPTURE_CANDIDATE_SHA256` to its checked digest.
Run `python3 -m unittest discover -s tests/tools -p test_ccusage_opencode_capture.py -v`
from the project root.

Producer execution, WAL support, release-scale capture, grouped storage and
native OpenCode integration, real logs, Windows, and macOS remain unqualified.
This patch does not complete AT-65 or replace the production scanner pin.

Patch [20.0.48-opencode-capture.patch](../../patches/ccusage/20.0.48-opencode-capture.patch)
has SHA-256 `2d4ac19a5fdfb52c238b67ca3104f2a1a807caeb959ae89694119f5af2143ca7`.
The release binary has SHA-256
`530b00f2abb452847b3fbf81e59f14b1e68eab3bd086c017e6b8d537904e33b9`.
The resulting lockfile has SHA-256
`73b42993adc34213b165429a5498e87707f36f3fb1ddde63dc443a8b3fbe16fa`.
