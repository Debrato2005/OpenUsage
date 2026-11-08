# Amp duplicate handling design

Date: November 8, 2026. The initial sections record the unchanged source and baseline trial. The candidate section records the later local patch and checks. No upstream publication occurred.

## Source and build inputs

The official Git remote is `https://github.com/ccusage/ccusage.git`. A shallow fetch of the exact commit produced this checkout:

```text
/home/debrato/Projects/OpenUsage/.tools/upstream/ccusage-8995f3b7
HEAD: 8995f3b7fd7110b8f37564f7d0468acc7a7d6b20
Commit date: 2026-11-02T18:14:57Z
Subject: chore: release v20.0.48 (#2011)
```

OpenUsage ignores `.tools/`. At the baseline checkpoint, the checkout had no tracked changes. Its only new files were under `upstream-inputs/`.

The [pinned flake lock](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/flake.lock) names the [LiteLLM input](https://raw.githubusercontent.com/BerriAI/litellm/1046716b6d3d45f30d5efad42becbd440d6187ee/model_prices_and_context_window.json). The downloaded JSON is:

```text
upstream-inputs/litellm-1046716b.json
Bytes: 3176369
SHA-256: 8f8060d8ce7a40c91b995c74d6a27657543d49e3c1c53430dabd556f244392c6
NAR SHA-256: sha256-vPdRBYQk4x/35FhdgfCszMnOCwflja/N1WwS9/LExwk=
```

The NAR digest matches the lock. Nix is unavailable, so Python encoded the regular file using the [official NAR specification](https://nix.dev/manual/nix/2.24/protocols/nix-archive). The receipt is `upstream-inputs/provenance.json`.

Unchanged lock hashes:

```text
rust/Cargo.lock: cefdc05acd4531747a7e9925a1ce4af409ec0c653948eeefabd09b45b4684e17
flake.lock: 773b1951c363ff2e12f08014e4b604dcd07b9c23c7aa9daeca027038627e660f
```

## Proven source behavior

The [Amp loader](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/amp/src/loader.rs) appends all parsed files before sorting. It has no duplicate set. The [previous review](2026-11-08-ccusage-duplicate-handling.md) records the root agent's executed 162-to-324 regression.

The [parser](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/adapters/amp/src/parser.rs) supports two representations:

- Ledger events use the thread ID and a nonempty string event ID.
- Message usage uses the thread ID and a numeric or string message ID.
- A usable ledger takes precedence within one file.
- The parser retains no source revision or snapshot identity.
- Message-ID conversion currently makes numeric 7 and string `"7"` equal.
- Cache lookup uses numeric message IDs. Repeated cache entries replace earlier values.
- Existing parser tests cover fallback and precedence, but no copied-file merge.

Thus a set of normalized `LoadedEntry` values is insufficient for every conflict. Raw field presence, identity kind, and cache-source conflicts can disappear before the loader sees them.

Current [Amp plugin documentation](https://ampcode.com/docs/plugin-api) says message IDs are unique within a thread. The [external API](https://ampcode.com/api/external) documents stable message IDs and message versions. These are newer interfaces with different field names. They do not prove a revision rule for the pinned legacy ledger or local `messageId` format.

## Proposed minimal boundary

Place duplicate handling in the Amp adapter before report aggregation. Collect all selected roots in one load. Preserve narrow parser metadata instead of modifying the shared report schema.

Use a key with three parts: thread ID, representation kind, and typed source ID. Keep ledger IDs separate from message IDs. Preserve numeric/string distinction until a source contract proves equivalence. Thread identity must remain part of the key.

For each known key, compare the complete usage payload: timestamp, model, source token fields and their presence, attached cache fields, credits, and total. Suppress a repeated entry only when those values agree. Do not use estimated cost as identity. Calculate it after the accepted records are fixed.

A growing snapshot with unchanged existing events and new IDs then becomes a union. Different threads with the same event IDs remain separate. Different IDs with equal times and counts remain separate.

Do not select conflicting snapshots by filename, filesystem order, modification time, greatest total, or per-field maxima. No inspected source establishes any of these as a revision rule.

### Ambiguities

| Case | Recommended safe behavior | Remaining question |
|---|---|---|
| Same known key, changed usage or credits | Fail the Amp scan and preserve saved OpenUsage data | Which source revision, if any, is authoritative? |
| Missing IDs in overlapping snapshots | Do not infer identity from equal counters or timestamps | Can the format prove stable positions or source revisions? |
| Identical copied snapshot with missing IDs | An exact snapshot comparison could suppress the duplicate | Add and review a snapshot identity mechanism before claiming support |
| Ledger in one snapshot, messages in another | Treat an overlapping thread as unresolved | Does a ledger event map one-to-one to `toMessageId`? |
| Same message ID with conflicting cache attachments | Surface a conflict before cache lookup overwrites it | Can a source revision select one attachment? |
| Different thread IDs sharing copied history | Keep thread identities distinct | Fork-parent provenance needs a separate rule and fixtures |

An initial change can fix unchanged copies and growing snapshots with stable IDs. It must not claim all AT-65 cases. Missing-ID copies and forks need more format evidence.

Owner input is needed if the product should show partial Amp output or select a winner for conflicting records. Recommended default: fail the current Amp scan. This preserves prior accepted totals without inventing a winner.

## Determinism and regression plan

Use ordered keys for the final entries. Sort by normalized event time, thread ID, representation, and typed ID. Compare semantic report JSON across reversed root/file order and single/parallel loading. Keep the conflict result independent of traversal order.

Each test uses a private synthetic home. Add cases at the parser and loader boundaries, then assert daily and session output through the CLI.

| Synthetic case | Expected result |
|---|---|
| One 162-token thread | 162 |
| Same file copied under another name/root | 162 when stable identities agree |
| Snapshots `[A,B]` and `[A,B,C]` | A+B+C once |
| Snapshots `[A,B]` and `[B,C]` | A+B+C once |
| Repeated known event ID inside one ledger | One event when payloads agree |
| Same ID, changed output/model/time/credits/cache | Deterministic conflict |
| Same event ID in two distinct threads | Both events |
| Distinct IDs with equal usage and time | Both events |
| Missing ID, identical copy | Unqualified until snapshot identity exists |
| Missing ID, changed overlap | Unresolved, no guessed merge |
| Ledger/message representations of one thread | Unresolved until their mapping is proved |
| Reversed roots/files and parallel loading | Same accepted rows or same conflict |

Also retain upstream parser tests for missing ledgers, malformed records, total fallback, and cache attribution. A copied-file fix must not alter token or pricing semantics.

## Build preparation

Upstream [pins Rust 1.97.1](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust-toolchain.toml). The compatibility trial uses existing Rust 1.99.1. This version is an implementation choice, not an explicit owner choice.

Before the trial, the full Cargo lock had 63 registry archives absent from OpenUsage's cache inventory. That count includes optional and foreign-target packages, so it is not an Amp-only download count. Direct test dependencies include `assert_fs` and bundled SQLite through [test support](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/crates/ccusage-test-support/Cargo.toml). The shared terminal crate also needs its locked terminal-size and Unicode-width dependencies.

The baseline downloaded 23 locked crate archives into the shared cache. Their SHA-256 values matched the Cargo lock checksums. Both upstream lockfiles remain unchanged. No separate full-workspace fetch ran.

The [pricing build script](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/crates/ccusage-pricing-snapshots/build.rs) needs `CCUSAGE_PRICING_JSON_PATH`. Supply the verified file instead of enabling the optional fetch feature.

After the primary agent released the shared target, this command ran from the upstream checkout under a 600-second timeout:

```sh
env \
  PATH="/home/debrato/Projects/OpenUsage/.tools/rust/install/bin:$PATH" \
  CARGO_HOME=/home/debrato/Projects/OpenUsage/.tools/cargo-home \
  CARGO_TARGET_DIR=/home/debrato/Projects/OpenUsage/.tools/build-target \
  CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=1 \
  CARGO_PROFILE_DEV_DEBUG=1 CARGO_PROFILE_TEST_DEBUG=1 \
  CCUSAGE_PRICING_JSON_PATH=/home/debrato/Projects/OpenUsage/.tools/upstream/ccusage-8995f3b7/upstream-inputs/litellm-1046716b.json \
  cargo test --locked --manifest-path rust/Cargo.toml -p ccusage-adapter-amp
```

The run exited 0. All seven Amp unit tests passed, with no failures or ignored tests. There were no doc tests. Cargo reported 22.81 seconds. The observed run window was 12:27:44 to 12:28:13 UTC on November 8, 2026. No retry ran.

The receipt is `upstream-inputs/amp-baseline-rust1.99.1.json`. It records the toolchain, command, build environment, downloaded packages, checksum checks, lock hashes, and result. A post-run Git check found no tracked source changes.

This Linux run covers the existing Amp parser tests. It did not run a copied-log regression, compile the production CLI, or qualify a replacement pin. The pricing path was supplied, but these focused tests did not compile the pricing snapshot crate or test the embedded price table. Conflicting and missing identities remain unresolved.

A later CLI build can use the same environment with `cargo build --locked --manifest-path rust/Cargo.toml -p ccusage --bin ccusage`. This build had not run at the baseline checkpoint. The candidate section records the later build.


## Local candidate after review

The candidate now fails when selected files or folders cannot be read. Damaged JSON and usage without a thread ID also fail. Valid metadata without usage remains skippable. An absent default Amp folder returns no usage. Each explicit root must exist and be a directory. A non-UTF-8 root setting fails instead of selecting the default home.

The file and root regressions first reproduced successful partial reports. After the fix, all 28 Amp tests passed on Linux with synthetic input. The changed adapter passed Clippy with `--no-deps`. One test permission literal first failed Clippy and was corrected to octal. The baseline dependency lint limit remains unchanged.

The candidate does not change the production pin or enable Amp. Complete replay, fork, compaction, performance, platform, process-isolation, and native persistence checks remain open. The pinned Nix environment and full upstream workspace have not run.

The final candidate build took 45.43 seconds with locked, offline dependencies. It uses the verified pricing input above and version `20.0.48-openusage.amp-candidate.1`. Seven Python tests then passed with 210 scanner invocations in 17.035 seconds. They compared all sixteen recorded daily/session formats, empty roots, existing Claude/Codex output, Amp copies, growing snapshots, and failures. Synthetic source files stayed unchanged, and private homes stayed empty.

The source review closed its file, discovery, and explicit-root findings. The patch passed an apply check against the exact pinned commit and a reverse check against the candidate checkout. The production pin is unchanged. No upstream write or patch publication occurred.

The internal weekly aggregation test passed. The Amp CLI has no weekly command. The initial CLI trial included that unsupported command and failed before the matrix was corrected. This is not weekly CLI acceptance.

[Independent review](2026-11-08-amp-dedup-review.md) records its inspected source and limits. Final artifact hashes:

```text
Patch: e75d707e92154c00939a031b8cf9d8bda3dce9ef8387791a3157265cb8b98447
Binary: 5c2169e3b149de2eeeb8b92a8a3a1b2d0e69eb03a1bc49b4ae14ad976a7c0aab
```

The local metadata receipt is `upstream-inputs/amp-candidate-io-fixed.json`. The patch is in [patches/ccusage](../../patches/ccusage/README.md).

The OpenUsage staged verification plan passed whitespace and all Python tool tests in 20.78 seconds. Both scanner paths and the candidate digest were set. The manual patch gate uses the adapter tests, Clippy, formatter, CLI comparisons, and apply checks above. No production Rust code changed in this slice.
