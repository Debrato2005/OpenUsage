# ccusage Amp candidate

This patch targets ccusage commit `8995f3b7fd7110b8f37564f7d0468acc7a7d6b20`, version 20.0.48. The bundled binary still uses the official pin. The patch is local and unpublished.

Matching event or typed message IDs within one thread count once when their usage fields agree. All selected roots take part. The merge preserves each ledger cache attachment before token normalization. Conflicting usage, missing-ID overlaps, and mixed ledger/message snapshots fail the scan. Different thread IDs stay separate. Copied fork history remains unqualified.

The candidate now fails when selected files or folders cannot be read. Damaged JSON and usage without a thread ID also fail. Valid metadata without usage remains skippable. An absent default Amp folder returns no usage. Each explicit root must exist and be a directory. A non-UTF-8 root setting fails instead of selecting the default home.

The patch adds no dependency and changes no lockfile or report schema. The upstream MIT license is in [the existing notice](../../packaging/licenses/ccusage/LICENSE). [Research and build inputs](../../docs/reviews/2026-11-08-amp-dedup-design.md) give the source and pricing provenance.

Apply `20.0.48-amp-duplicates.patch` to a clean checkout of the exact commit with `git apply`. Use the shared Cargo target and locked pricing input described in the research note. Set `CCUSAGE_VERSION=20.0.48-openusage.amp-candidate.1` for the candidate CLI build.

Run the adapter tests and Clippy for the changed crate. Dependency Clippy under Rust 1.99.1 reports an existing needless borrow in upstream `ccusage-core/src/pricing.rs`. The changed adapter passes with `--no-deps`. The pinned Nix environment, full upstream workspace, other platforms, and performance limits remain unverified.

Run OpenUsage's candidate checks with an explicit binary and its SHA-256:

```sh
OPENUSAGE_CCUSAGE_CANDIDATE=/absolute/path/to/ccusage \
OPENUSAGE_CCUSAGE_CANDIDATE_SHA256=<expected-sha256> \
python3 -m unittest discover -s tests/tools -p test_ccusage_amp_candidate.py -v
```

The checks require the official pinned binary as a reference. They use private synthetic homes, check source files for changes, and compare all sixteen recorded formats. They also compare existing Claude/Codex reports and Amp JSON/table output. They test copies, multiple roots, growing snapshots, conflicts, and missing IDs. They do not qualify all AT-65 cases or promote a production pin.

## Separate Pi candidate

`20.0.48-pi-usage.patch` adds summary and standalone usage to the same pinned source. It does not include the Amp changes. The [Pi candidate record](../../docs/reviews/2026-11-10-pi-usage-candidate.md) owns its evidence and remaining gates. Apply the patch to the exact source commit and set `CCUSAGE_VERSION=20.0.48-openusage.pi-candidate.1` when building.

Run `test_ccusage_pi_candidate.py` with `OPENUSAGE_CCUSAGE_PI_CANDIDATE` and `OPENUSAGE_CCUSAGE_PI_CANDIDATE_SHA256`. Its separate variables prevent accidental selection for the Amp checks. This candidate also remains local and unpublished.

## Separate Pi identity candidate

Apply `20.0.48-pi-identity.patch` after `20.0.48-pi-usage.patch` on the same pinned source. It requires source IDs and proven parent links, counts copies once, and rejects ambiguous input. Version 1 and headerless data are unsupported. This patch is local and unpublished; it does not replace the production pin or Amp candidate. The [review](../../docs/reviews/2026-11-10-pi-identity-candidate.md) owns the source evidence, digests, checks, and remaining gates.

Set `OPENUSAGE_CCUSAGE_PI_IDENTITY_CANDIDATE` and `OPENUSAGE_CCUSAGE_PI_IDENTITY_CANDIDATE_SHA256` to run `test_ccusage_pi_identity.py`. Use version `20.0.48-openusage.pi-identity-candidate.1` for its build. Keep the earlier usage-candidate variables unset for this binary. Its stricter identity boundary intentionally rejects their headerless fixtures.
