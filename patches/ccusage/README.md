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

## Separate Pi format candidate

Apply `20.0.48-pi-format.patch` after the usage and identity patches. It maps validated records, accepts escaped JSON keys, rejects ambiguous objects and invalid shapes, and keeps assistant totals out of inferred categories. It remains local and unpublished. The [review](../../docs/reviews/2026-11-10-pi-format-candidate.md) owns digests, checks, and remaining filesystem gates.

Set `OPENUSAGE_CCUSAGE_PI_COMPLETENESS_CANDIDATE` and `OPENUSAGE_CCUSAGE_PI_COMPLETENESS_CANDIDATE_SHA256` for `test_ccusage_pi_format.py`. Its build version is `20.0.48-openusage.pi-completeness-candidate.1`. Keep the earlier candidate variables unset for this binary. No production pin or Amp patch changes.

## Separate Pi filesystem candidate

Apply `20.0.48-pi-filesystem.patch` after the usage, identity, and format patches.
It bounds Linux reads, rejects omitted or changed inputs, and adds a direct
Rustix 1.1.5 dependency already present in the lock. Package versions do not
change. The [review](../../docs/reviews/2026-11-10-pi-filesystem-candidate.md) owns
limits, platform scope, digests, checks, and remaining gates.

Set `OPENUSAGE_CCUSAGE_PI_FILESYSTEM_CANDIDATE` and
`OPENUSAGE_CCUSAGE_PI_FILESYSTEM_CANDIDATE_SHA256` for
`test_ccusage_pi_filesystem.py`. Use build version
`20.0.48-openusage.pi-filesystem-candidate.1`. Keep earlier candidate variables
unset for this binary. The candidate remains local and unpublished. Pi stays
disabled; the production pin and Amp patch are unchanged.

## Separate Pi snapshot candidate

Apply `20.0.48-pi-snapshot.patch` after the usage, identity, format, and filesystem
patches. It adds `pi snapshot --json --offline` and builds both required reports
from one captured entry set. The [review](../../docs/reviews/2026-11-10-pi-snapshot-candidate.md)
owns evidence, digests, and remaining gates. No lockfile changes in this layer.

Set `OPENUSAGE_CCUSAGE_PI_SNAPSHOT_CANDIDATE` and
`OPENUSAGE_CCUSAGE_PI_SNAPSHOT_CANDIDATE_SHA256` for
`test_ccusage_pi_snapshot.py`. Use build version
`20.0.48-openusage.pi-snapshot-candidate.1`. Keep earlier candidate variables
unset for this binary. Pi stays disabled. All candidate patches remain local
and unpublished; the production pin and Amp patch are unchanged.

## Pi bounded capture prototype

Apply `20.0.48-pi-bounded.patch` after the observation patch and its prior
layers. Build with `CCUSAGE_VERSION=20.0.48-openusage.pi-bounded-prototype.1`
and the verified pricing input. The patch uses the locked Ring 0.17.14
dependency for SHA-256. The [prototype record](../../docs/reviews/2026-11-11-pi-bounded-capture.md)
owns the capture limits and evidence.

Use the [prototype checks](../../docs/engineering-workflow.md#pi-bounded-capture-prototype)
with the two explicit binary paths. Pi stays disabled.

## OpenCode observation candidate

Apply `20.0.48-opencode-observations.patch` to the pinned ccusage source commit
above. Set `CCUSAGE_VERSION=20.0.48-openusage.opencode-observations-candidate.1`
for the build with the verified pricing input and the shared target.
The [candidate record](../../docs/reviews/2026-11-11-opencode-observation-candidate.md)
owns the source boundary, digests, and remaining qualification limits.

Set `OPENUSAGE_CCUSAGE_OPENCODE_CANDIDATE` and
`OPENUSAGE_CCUSAGE_OPENCODE_CANDIDATE_SHA256` to run
`test_ccusage_opencode_candidate.py`. OpenCode stays disabled.

## OpenCode capture candidate

Apply `20.0.48-opencode-capture.patch` after the observation patch on the same
pinned source. The [capture record](../../docs/reviews/2026-11-11-opencode-capture-candidate.md)
owns limits, digests, and remaining gates. Use its separate candidate variables
to run `test_ccusage_opencode_capture.py`. OpenCode stays disabled.
