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
