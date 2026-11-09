# Local Pi filesystem candidate

Date: November 10, 2026. L-1f11f1 adds a fourth local Pi patch after the [format correction](2026-11-10-pi-format-candidate.md). Pi stays disabled. The production scanner and unpublished Amp patch are unchanged.

## Source and correction

The review compares scanner source, producer source, and synthetic results. The earlier scanner filtered out missing explicit roots and used discovery helpers that could omit unreadable entries. Six initial tests reproduced missing roots, denied directory reads, skipped links and special files, token overflow, and excessive directory depth.

Pi appends entries and can rewrite a session through a writable file descriptor. Fork creation writes a new file through one descriptor. A directory listing alone therefore cannot establish stable input. These are source findings. The actual producer did not run. [Pinned producer implementation](https://github.com/earendil-works/pi/blob/0d1793c2bc7139d1df4547fa3c0aeacb233e0534/packages/coding-agent/src/core/session-manager.ts).

The candidate opens selected roots with Linux `openat2`. Child opens stay beneath held root handles, with links refused. Missing or unreadable explicit roots, discovery errors, and special files fail the load. An absent default folder gives an empty result. Explicit lists reject empty parts and surrounding whitespace. Derived `subagent-artifacts` remain excluded. Primary `run-*` sessions remain included.

The reader captures bounded bytes, checks file metadata, then reads the bytes again and compares the complete directory manifest. This catches the controlled changes below without a retry. It uses the safe API in the locked Rustix 1.1.5 source. [Linux path-resolution contract](https://man7.org/linux/man-pages/man2/openat2.2.html).

These checks do not establish an operating-system atomic snapshot. They do not establish one shared input across separate daily and session commands. Continuous producer writes and deliberate change-and-restore races remain unqualified.

## Limits and evidence

Each load permits at most 64 roots, 32,768 directory entries, 8,192 JSONL files, depth 64, 32 MiB per file, 128 MiB of source bytes, and 100,000 physical records. Replayed records consume the record budget. The JSON parser bounds nesting. Checked token sums reject overflow after duplicate suppression.

All 64 adapter tests passed. Fourteen filesystem tests cover denied reads, links, special files, append, truncate, same-size rewrite with restored modification time, replacement, added and deleted files, and root replacement. Small injected limits test exact boundaries and rejection before file reads. Other tests cover replay budgets, JSON depth, and overflow. These checks do not measure release-scale memory or runtime.

Seven candidate Python tests passed. Sixteen filesystem cases produce 32 daily/session outcomes: baseline and repeated-root cases preserve 162 tokens. Fourteen cases return unavailable. They include FIFO, socket, sparse oversized input, and 65 explicit roots. Tests run as an unprivileged Linux user with private homes and synthetic sources. Source hashes remain unchanged. Earlier identity and format matrices, the other fifteen formats, enabled Claude/Codex, and five JSON/table views also pass.

The first full OpenUsage run passed Python checks, then failed an existing provider cooldown test after 174 desktop tests passed. The error was `ProviderUnavailable`, with no captured underlying cause. [R-2](2026-11-10-desktop-fixture-writes.md) removes a known desktop fixture-write exposure. Its diagnostics did not reproduce the original failure. Final OpenUsage Python/Rust verification and rebuilt native consent passed. Existing grouped-runner and storage checks cover report failure, atomic rejection, and retention of saved data. This slice changes no storage or production runner behavior. Missing times remain unavailable.

The initial compile required explicit path-conversion closures and removal of a duplicate import. One old path test expected missing explicit paths to disappear. Its expectation now follows strict selection. Clippy rejected decimal permission literals, which were changed to octal. Read-only navigation commands used nonexistent documentation/module paths. Corrected discovery found the owning files. No automatic retry, timeout, or interruption occurred.

## Build and remaining gates

The source is ccusage `8995f3b7fd7110b8f37564f7d0468acc7a7d6b20`. The build uses Rust 1.99.1, offline dependencies, and the verified pricing input. Cargo adds only a direct Pi dependency on already locked Rustix 1.1.5. Package versions and checksums are unchanged. The pnpm and flake locks are unchanged. Version: `20.0.48-openusage.pi-filesystem-candidate.1`.

Patch: [20.0.48-pi-filesystem.patch](../../patches/ccusage/20.0.48-pi-filesystem.patch), SHA-256 `526a888f9544000d2055fac86ee77d6245abfc90e3cd68fee3a4c4ea6c634254`. Binary SHA-256: `09164a51364596bb455942b1db768214dd552897c52197718073064791b81ad5`. Layered apply and reverse checks pass. Private metadata retains the source, patch layers, locks, pricing input, and executed checks. No extra binary copy is retained.

Selected Pi roots on other platforms return unavailable. Linux without `openat2` also fails. No permissive fallback is used. Those platforms and kernels did not run. Native consent uses the production scanner and does not qualify the candidate.

Renderer checks, other native journeys, pinned upstream Nix/Rust 1.97.1, full upstream workspace, hosted CI, real logs, and producer execution did not run. Paired-report consistency, zero-usage observations, absent times, and release-scale performance still need qualification. L-1f11 stays open. Only Claude Code and Codex are enabled.
