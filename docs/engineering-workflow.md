# Bounded engineering workflow

Use one primary agent and one bounded assignment. Name the user outcome, acceptance criteria and stopping condition before edits. Add another agent only for independent work with separate file ownership. Stop after verification, a local commit and the results report. Do not start another feature automatically.

## Commands

Use Python 3.12 and Git on Linux, macOS or Windows. Windows uses `python` instead of `python3`. No extra Python package, service or AI API is required.

```sh
python3 scripts/engineering.py doctor
python3 scripts/engineering.py verify
python3 scripts/engineering.py verify --run --timeout 600
python3 scripts/engineering.py verify --renderer-test tests/renderer/overview.spec.ts --run
python3 scripts/engineering.py verify --base main --milestone
python3 scripts/engineering.py trace /absolute/path/to/selected-session.jsonl
```

`doctor` reports tool versions, the pinned Node match, Linux native dependencies and configuration flags. It does not install software, inspect credentials or prove native readiness. Activate the required Node version before renderer checks.

`verify` defaults to a JSON plan for committed-to-working-tree changes and untracked files. Use `--staged` for the index, or `--base REF` for branch changes. Plans select tool tests, renderer specs and affected Rust packages. Unknown changes retain a manual gate. Review the plan before `--run`.

The plan covers every Rust workspace member, including the CLI, keys, and local logs. A member manifest also selects the dependency-policy check. A regression test compares this coverage with the workspace manifest. A source change in an unknown crate keeps an explicit manual gate.

Leaderboard source, tests, migrations, and configuration select `bun run test:leaderboard`. These tests use SQLite with a local D1 stand-in and a GitHub stub. Hosted D1 and deployment remain separate checks.

The runner stops at the first failure. One total deadline covers all checks, followed by bounded child cleanup. It never retries. It uses one Cargo target with incremental builds disabled. Missing renderer selections refuse execution. Use `--milestone` for applicable full renderer/workspace regression. Native acceptance, SQLCipher/restore, authentication isolation and SRS gates remain separate requirements.

Logs retain commands, timing, exit codes and fixed diagnostic counters. They discard raw child output. For full diagnostics, rerun the indicated command locally. Logs remain ignored and use the existing ownership-controlled retention: three completed runs, seven days, 16 MiB. No executable copies are retained.

`trace` reads only the selected regular JSONL file. It emits counts and owner-scoped token high-water marks without messages, arguments, identifiers or raw outputs. Copied parent history is flagged. Do not add token snapshots or interpret them as billing. Repeats and category matches indicate review candidates, not proven waste.

`scripts/dependency-policy.py` checks offline that every locked crate comes from crates.io with a checksum and every package in `bun.lock` from registry.npmjs.org with a sha512 integrity. Bun leaves the registry field empty only for that default registry. Any other lockfile, such as `package-lock.json`, fails the check. Registry or scope overrides in `bunfig.toml` or `.npmrc`, and non-registry specs in `package.json`, fail as well. Local crates must be workspace members, the patched glib or a lock's own crate. `verify` runs it when a manifest or lockfile changes, and runs `cargo fmt --all --check` when Rust changes.

The `CI` workflow in `.github/workflows/ci.yml` runs the full set on Linux for pull requests and `main` in two parallel jobs of at most 60 minutes. Linux checks runs dependency policy, whitespace, tool tests, Git safety, renderer build and TypeScript check, IPC contracts, Rust format, Clippy and workspace tests, and native oracle tests. Renderer installs only Node, Bun and Playwright's browsers and runs the renderer suite in both engines. Apt retries stalled downloads, because the runner's package mirror alone once took 74 minutes. It installs Rust 1.99.1, Node 22.23.2, Bun 1.3.14 and Gitleaks 8.30.1 from their official archives and checks their pinned digests. The Git safety tests and hooks find Gitleaks through `GITLEAKS_BIN`. Bun installs the frozen lockfile and runs package scripts; Vite, Playwright and the contract tests still run on Node. `package.json` names `playwright-core` at the same version as `@playwright/test`, so the axe checks and the test runner share one Playwright. An empty `trustedDependencies` keeps dependency lifecycle scripts blocked, as `ignore-scripts` did under npm. Native GUI journeys stay a local gate. Windows and macOS join in S-15.

CI tests the combined Codex and Pi qualification features. It does not run the
private candidate executable or native GUI journeys. The native Codex journey
and its ordinary-build check remain local gates.

Reuse `scripts/storage.py check` and `clean` for storage measurement and bounded cleanup. Cleanup previews by default. Existing secret hooks remain mandatory. CI runs Python tool tests, whitespace and tracked-ignore checks with a five-minute deadline. Hosted Windows/macOS results remain unverified until CI runs.

## Pi candidate integration

On Linux, set `OPENUSAGE_PI_QUALIFICATION_BIN` to the absolute path of the
[observation candidate](reviews/2026-11-10-pi-observations-candidate.md#build-and-remaining-gates).
Set `OPENUSAGE_PI_PRODUCER_SOURCE` to the absolute path of the
[pinned writer source](../tests/tools/fixtures/pi-producer/README.md).
Run as an unprivileged user. The test needs Node, Python, and Bubblewrap.

```sh
timeout 180 cargo test --locked -p openusage-storage --features pi-qualification --test pi_grouped -- --ignored
```

Use the shared Cargo target. The test checks the candidate digest and the
writer source pins. It runs the writer offline in private fixtures. The
feature exposes the grouped runner only for Pi qualification. The desktop and
CLI use the normal catalog gate.

The separate [native qualification build](../tests/native/README.md#native-pi-candidate)
uses the bounded release candidate through the application. Production support
stays unchanged.

### Pi scale test

Use the same candidate and writer paths. Run from the project root with the
shared Cargo target.

```sh
timeout 600 python3 tests/tools/pi_scale.py
```

The test writes three samples per workload. It checks exact totals through the
grouped scanner and SQLCipher, then checks refusal at the selected limits.
Only run metadata remains in the private diagnostic directory.

### Pi large-history experiment

Use the same candidate and writer paths. The private fixture uses 1 GiB of
logical source bytes. Run from the project root.

```sh
timeout 180 python3 tests/tools/pi_large_history.py
```

The experiment checks the joint byte limit and compares copy, conflict, and
fork outcomes across root selections. It retains only run metadata.

### Pi bounded capture prototype

Set `OPENUSAGE_PI_OBSERVATION_BIN` to the observation candidate. Set
`OPENUSAGE_PI_QUALIFICATION_BIN` to the [bounded prototype](reviews/2026-11-11-pi-bounded-capture.md).
Use the same pinned writer path and shared Cargo target. Run from the project
root on Linux.

```sh
timeout 450 python3 tests/tools/pi_bounded_capture.py
```

The harness checks both digests. It compares the complete reports and child
memory on a shared fixture. It also checks two 1 GiB histories and uses the
grouped scanner to save one in SQLCipher. Only run metadata remains.

Use `OPENUSAGE_PI_QUALIFICATION_VARIANT=bounded` for the grouped integration
test with the prototype. Leave this variable unset for the observation candidate.

### Release local-history measurements

Build the bounded Pi source with `--release` and
`CCUSAGE_VERSION=20.0.48-openusage.pi-bounded-release.1`. Use the same verified
pricing input and shared target. Set `OPENUSAGE_PI_QUALIFICATION_BIN` to that
binary and keep the pinned writer path.

```sh
timeout 600 python3 tests/tools/pi_release_capture.py
```

The harness checks the release digest, limits CPU affinity to four slots, and
collects three samples for each 1 GiB workload. It runs the grouped storage
test in release mode and checks refusal after one added byte.

The [native benchmark](../tests/native/README.md#native-local-history-performance)
measures first history through the enabled Claude Code consent path.

## Shared candidate comparison


Set `OPENUSAGE_SHARED_CANDIDATE` to the bounded Pi release candidate.
Set `OPENUSAGE_SHARED_CANDIDATE_RESULTS` to a new private JSON path.
The test refuses an existing result file and an executable off its pin.
Run the command from the project root:

```sh
python3 -m unittest discover -s tests/tools -p test_ccusage_shared_candidate.py -v
```

The live test uses private synthetic input. Its seven harness checks also run
without an executable. A successful comparison means the two artifacts match
within this matrix. It does not mean either scanner meets every independent
expectation. The [compatibility record](reviews/2026-11-12-shared-scanner-compatibility.md) holds those disagreements.

## Codex producer capture


On Linux, run the producer harness with the reviewed Codex executable and
Bubblewrap available. Select a new private output path.

```sh
python3 tests/tools/codex_producer.py --mode contract --output /absolute/private/new-capture.json
```

This starts a local synthetic Responses fixture inside an isolated network
namespace. It uses fresh homes and no real provider account. The output holds
raw synthetic rollouts and must stay private. A new capture needs a reviewed
projection before the scanner comparison can use it.

The historical comparison needs the exact private receipt referenced by
`tests/tools/fixtures/codex-producer/capture.json`. Set
`OPENUSAGE_CODEX_CAPTURE_RECEIPT` to that receipt. Set
`OPENUSAGE_CODEX_CAPTURE_RESULTS` to a new private results path. Then run:

```sh
python3 -m unittest discover -s tests/tools -p test_codex_producer.py -v
```

Without those variables, the source and projection checks still run. The live
scanner comparison stays skipped. The [producer contract record](reviews/2026-11-12-codex-producer-contract.md) owns the
executed scope and remaining gates.

## Codex record candidate checks

Set `OPENUSAGE_CODEX_RECORD_CANDIDATE` to the absolute candidate oracle path from
its [fixture pin](../tests/tools/fixtures/codex-records-candidate/pin.json).
The tests check its digest before execution. For raw producer comparisons,
set `OPENUSAGE_CODEX_CAPTURE_RECEIPT` to the exact private receipt referenced by
the producer fixture. Select new private result paths with
`OPENUSAGE_CODEX_RECORD_RESULTS` and `OPENUSAGE_CODEX_COMPAT_RESULTS`.

```sh
python3 -m unittest discover -s tests/tools -p test_ccusage_codex_candidate.py -v
```

Without the executable variable, only pin checks run. Without the receipt,
the raw capture comparison stays skipped. The [candidate record](reviews/2026-11-12-codex-record-identity-candidate.md)
owns executed results and the missing-parent decision. The command qualifies
individual captures. It does not establish joint capture across two invocations.

## Codex combined capture checks

Set `OPENUSAGE_CODEX_SNAPSHOT_CANDIDATE` to the absolute oracle path from its
[fixture pin](../tests/tools/fixtures/codex-snapshot-candidate/pin.json).
Set `OPENUSAGE_CODEX_CAPTURE_RECEIPT` to the exact private producer receipt for
raw capture comparisons. Select a new private output path with
`OPENUSAGE_CODEX_SNAPSHOT_RESULTS`.

```sh
python3 -m unittest discover -s tests/tools -p test_ccusage_codex_snapshot.py -v
```

The controlled trace runs on Linux x86_64 and needs permission to trace its own
private child. Without the executable variable, only pin checks run. Without
the receipt, the raw producer comparison stays skipped. These checks qualify
the isolated scanner command. The [record](reviews/2026-11-12-codex-combined-capture.md)
owns its executed scope and remaining application gates.

## Codex consumer fixtures

The [fixture generator](../tests/tools/codex_snapshot_fixture.py) uses the pinned
snapshot oracle and the exact private producer receipt. Select a new private
output path. It checks independent totals, source fingerprints, and the oracle
before it writes report projections.

```sh
python3 tests/tools/codex_snapshot_fixture.py --capture-receipt /absolute/private/capture.json --output /absolute/private/new-reports.json
```

Review the output before replacing the tracked Rust fixture. The default Python
suite checks its scanner, producer, and generator pins and excludes message
bodies. Rust tests preserve original report bytes for decimal and duplicate-field
checks. The [consumer record](reviews/2026-11-12-codex-snapshot-consumer.md)
owns executed evidence. These tests do not activate application scanning.

## Claude JSON candidate checks

Set `OPENUSAGE_CLAUDE_JSON_CANDIDATE` to the absolute read-only oracle path in
its [pin](../tests/tools/fixtures/claude-json-candidate/pin.json).
Set `OPENUSAGE_CLAUDE_JSON_RESULTS` to a new private JSON path.
Run from the project root:

```sh
python3 -m unittest discover -s tests/tools -p test_ccusage_claude_json.py -v
```

The tests check the executable digest before running. All inputs and homes are
private and synthetic. Without an executable selection, source and harness
checks still run. The live cases stay skipped. The new matrix permits only the
three expected Claude whitespace improvements. It records remaining independent
failures separately. The [candidate record](reviews/2026-11-12-claude-json-candidate.md)
owns executed results and limits.

The older Codex mixed-tool equality method expects unchanged Claude reports.
It is a historical diagnostic for its own artifact. Keep its test and pins
unchanged. The new matrix covers the changed Claude contract. A final Codex
selection retains all record and pin checks and the raw-receipt skip.

## Scanner candidate builds

Run Cargo from the candidate source root so its `.cargo/config.toml` applies.
Prefix `PATH` and set `RUSTC` to the pinned toolchain in `.tools/rust/install/bin`.
Keep the shared Cargo target and verified pricing input. Record the compiler's
verbose version, including LLVM, with the build receipt.

The pinned `sqlite3-src` 0.7.1 reads `SQLITE_*` settings without declaring them
as Cargo inputs. A change of those settings can leave a stale native library.
After such a change, run this command from the candidate source root with the
pinned environment:

```sh
cargo clean --release -p sqlite3-src -p sqlite3-sys --manifest-path rust/Cargo.toml
```

This removes generated SQLite outputs only. Then build with the corrected
inputs and check the expected binary digest. A November 11 rebuild reproduced
the qualified digest after this targeted cleanup. The
[producer qualification record](reviews/2026-11-11-opencode-runtime-qualification.md)
records the source and execution scope.

The pinned Pi bounded release used a different build boundary. Run Cargo from
the OpenUsage root with the candidate's absolute manifest path. The historical
build omitted the 47 `SQLITE_OMIT_*` settings in the candidate source config.
This reproduces the existing bounded release digest. A source-root build with
those settings produces a different binary and must not replace the pin.

Before another source-root scanner build, remove the generated release outputs
for `sqlite3-src` and `sqlite3-sys` with the command above. Keep source, prices,
and the required Pi oracle. Record the working directory and effective SQLite
settings with each receipt.
