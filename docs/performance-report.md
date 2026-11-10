# Performance report

## Pi candidate on Linux

L-1f11l, November 10, 2026. This section records one synthetic scanner and
storage benchmark. Pi stays disabled.

The host ran CachyOS, Linux 7.3.3-1-cachyos, on an x86_64 Intel Core Ultra 7
255HX with 20 logical CPUs and 15 GiB RAM. The fixtures ran on Btrfs on local
NVMe storage. OpenUsage used base commit `8b3d104a`, the test harness in this
change, Rust 1.99.1, and SQLCipher schema 13. The [pinned Pi
candidate](reviews/2026-11-10-pi-observations-candidate.md#build-and-remaining-gates)
is a debug build.

The [scale test](engineering-workflow.md#pi-scale-test) takes the record shape
from the pinned Pi writer. It generates private sessions with distinct IDs.
Each usage record has a 512-byte synthetic message and 120 tokens. The
file-size and aggregate-size cases add blank bytes to isolate their limits.
Directory entries include ignored files and nested directories. Record counts
exclude session headers.

Each workload has three samples. The grouped timer includes the digest check,
child run, and Rust report parser. The save timer covers the SQLCipher
transaction. Workspace creation and database reopen checks run outside both
timers. The harness checks exact period and session totals after reopen.

Each sample also runs the same child command directly with a private empty home
and offline arguments. The direct run writes stdout to a temporary file to
count bytes. Linux `wait4` reports user CPU, system CPU, and peak resident set
size (RSS) for the child. The benchmark uses freshly generated files without
clearing the OS cache. [wait4](https://man7.org/linux/man-pages/man2/wait4.2.html)
· [getrusage](https://man7.org/linux/man-pages/man2/getrusage.2.html)

### Accepted inputs

| Workload | Roots | JSONL files | Directory entries | Records | Input bytes | Exact tokens |
|---|---:|---:|---:|---:|---:|---:|
| Year | 4 | 1,000 | 1,000 | 10,000 | 9,417,000 | 1,200,000 |
| Record cap | 16 | 4,000 | 4,000 | 100,000 | 93,528,000 | 12,000,000 |
| File and entry caps | 64 | 8,192 | 32,768 | 8,192 | 8,503,296 | 983,040 |
| File size and depth caps | 1 | 1 | 65 | 1 | 33,554,432 | 120 |
| Aggregate byte cap | 4 | 5 | 5 | 5 | 134,217,728 | 600 |

Times are the minimum and maximum of three runs. Child CPU time is user time
plus system time. RSS is the highest value in the three runs.

| Workload | Child wall (s) | Child CPU (s) | Child peak RSS (KiB) | Output bytes | Grouped scan (s) | Save (s) | Scan and save (s) |
|---|---:|---:|---:|---:|---:|---:|---:|
| Year | 0.397–0.423 | 0.393–0.418 | 81,480 | 1,213,687 | 0.478–0.593 | 0.678–0.698 | 1.164–1.290 |
| Record cap | 2.949–3.076 | 2.944–3.069 | 672,820 | 3,436,143 | 3.083–3.170 | 2.654–2.674 | 5.756–5.844 |
| File and entry caps | 0.759–0.803 | 0.754–0.798 | 94,020 | 6,461,159 | 0.935–1.004 | 5.149–5.502 | 6.153–6.506 |
| File size and depth caps | 0.208–0.265 | 0.206–0.262 | 85,592 | 2,630 | 0.274–0.298 | 0.002–0.003 | 0.277–0.301 |
| Aggregate byte cap | 0.624–0.647 | 0.622–0.630 | 183,876 | 11,046 | 0.689–0.773 | 0.006–0.007 | 0.695–0.779 |

The runner sets a 120-second timeout and a 67,108,864-byte output cap. The
longest grouped scan was 3.170 seconds. The largest output was 6,461,159
bytes, about 10% of the cap. The 100,000-record workload had the highest
scanner memory use. The 8,192-file workload took the longest to save.

### Refused inputs

Each refusal preserved the saved scan ID, zone, day and session totals, and
absence of differences. The aggregate overflow removed one usage record and
kept the prior 600 tokens when the new 480-token input exceeded the byte cap.

| Boundary | Refused request | Result |
|---|---|---|
| Roots | 65 roots | Invalid request |
| Directory entries | 32,769 entries | Candidate refused |
| Files | 8,193 JSONL files | Candidate refused |
| Depth | 65 nested levels | Candidate refused |
| File size | 33,554,433 bytes in one file | Candidate refused |
| Aggregate size | 134,217,729 bytes across five files | Candidate refused |
| Records | 100,001 records | Candidate refused |
| Output | Cap set to 3,436,142 bytes for a 3,436,143-byte report | Output too large |
| Deadline | Timeout set to 1 ms on the 100,000-record workload | Timeout |

### Release scope

The 128 MiB cap blocks a single 1 GiB Pi group. The proposed first-history
target in OU-NFR-020 remains open. These measurements used a debug candidate,
a different host from the SRS reference machine, and synthetic inputs. The
release build, 1 GiB reference test, UI latency under load, real Pi histories,
Windows, and macOS remain unmeasured.
