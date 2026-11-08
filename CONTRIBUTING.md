# Contributing to OpenUsage

Keep changes small and verify the affected workflow. Preserve existing local data, unrelated work and the normative [SRS](docs/SRS.md). Commit locally by default. Push a branch only when the owner explicitly asks; do not publish, open pull requests or change a remote without owner approval.

The [documentation map](docs/README.md) shows which doc owns each fact and what to update. Update docs in the same change as the work.

## Build from source

With native prerequisites installed and dependencies cached, build the frontend and desktop binary:

```sh
bun run build
CARGO_BUILD_JOBS=1 cargo rustc --locked --offline -p openusage-desktop --bin openusage-desktop -- -C link-arg=-Wl,--threads=1
```

This Linux/LLD command limits linker workers as well as Cargo jobs. Cargo applies the additional flag only to the selected binary. See the [Cargo command documentation](https://doc.rust-lang.org/cargo/commands/cargo-rustc.html) and [LLD options](https://github.com/llvm/llvm-project/blob/main/lld/docs/ld.lld.1).

### Linux companion build

A provisioned build includes the pinned official Codex companion and its license notices. Rust checks the companion before execution. Account sign-in keeps its own consent. Manual paths remain in Settings for troubleshooting.

Prepare the reviewed archive, then enable the companion configuration:

```sh
bun run build
bun run prepare:companion --offline
bun run prepare:ccusage --offline
TAURI_CONFIG="$(cat apps/desktop/src-tauri/tauri.companion.conf.json)" \
  CARGO_BUILD_JOBS=1 cargo rustc --locked --offline \
  -p openusage-desktop --bin openusage-desktop -- -C link-arg=-Wl,--threads=1
```

The archive cache defaults to `.tools` in this checkout. Use `--cache-dir /absolute/cache` for a shared cache. Omit `--offline` only to download the exact pinned archive during build preparation. The script checks archive and executable hashes without launching the executable. It refuses unsupported targets.

The Tauri override stages `openusage-codex` beside the application and copies LICENSE and NOTICE under `licenses/codex`. Keep those files together. The ordinary development build can instead discover a supported installed version. It does not produce a companion bundle by default.

### Linux package

After `bun run build`, `prepare:companion` and `prepare:ccusage`, build the Debian package from `apps/desktop` with the pinned Tauri CLI:

```sh
cd apps/desktop
tauri build --bundles deb --config src-tauri/tauri.companion.conf.json \
  --config '{"build":{"beforeBuildCommand":""}}' -- --locked
```

The package is `target/release/bundle/deb/OpenUsage_<version>_amd64.deb`. It puts `openusage-desktop`, `openusage-codex` and `openusage-ccusage` side by side in `/usr/bin`, and the notices in `/usr/lib/OpenUsage/licenses`. It depends on `libwebkit2gtk-4.1-0` and `libgtk-3-0` (OU-PLAT-002). The CLI comes with the npm package. After you assemble the npm packages, add the package to the same `SHA256SUMS`, so the release publishes one list of digests (OU-PLAT-005):

```sh
(cd target/release/bundle/deb && sha256sum OpenUsage_<version>_amd64.deb) >> /absolute/empty/folder/SHA256SUMS
```

### npm packages

After a release build, assemble the npm packages for Linux x86_64 in an empty folder:

```sh
cargo build --release --locked --offline -p openusage-cli -p openusage-desktop
python3 scripts/sbom.py --out /absolute/openusage.cdx.json
python3 scripts/package-npm.py --out /absolute/empty/folder \
  --sbom /absolute/openusage.cdx.json \
  --cli target/release/openusage --desktop target/release/openusage-desktop \
  --ccusage apps/desktop/src-tauri/binaries/openusage-ccusage-x86_64-unknown-linux-gnu \
  --codex apps/desktop/src-tauri/binaries/openusage-codex-x86_64-unknown-linux-gnu
```

Use your shared Cargo target in place of `target` when you set one. The script checks both pinned binaries against their source records, then writes two packages: `openusage`, which holds only a launcher, and `@openusage/linux-x64`, which holds the CLI, the desktop app, `ccusage`, the Codex companion and their notices. Neither package has an install script, and nothing downloads at install time or at run time (OU-DIST-003). It packs both with `npm pack`, copies the SBOM beside them, and writes `SHA256SUMS`.

`scripts/sbom.py` writes a CycloneDX 1.5 SBOM of the Linux release. It lists the crates of the CLI and the desktop app, the npm packages in the renderer, and the pinned `ccusage` and Codex binaries. It reads only the lockfiles, the Cargo cache and `node_modules`, so run `bun install` first. Set `SOURCE_DATE_EPOCH` to fix its time. Otherwise it uses the time of the HEAD commit. It never publishes: publishing with npm provenance is an owner step.

This path currently covers Linux x86_64 only. A native build does not prove an installer or another platform. See the [source review and architecture decision](docs/reviews/2026-10-21-onboarding-sources.md).

## Normal Git workflow

Use the project root. Do not create a second publication checkout or develop inside recovery copies. Recovery copies are private backups.

The repository retains privacy-filtered development history. `Original-Commit` trailers link filtered commits to their local originals. Raw account data, test databases and diagnostic captures remain private.

Install [Gitleaks](https://github.com/gitleaks/gitleaks/releases) once from its official releases. The reviewed local version is 8.30.1. Make `gitleaks` available on `PATH`, or set `GITLEAKS_BIN` to its executable. The hook also recognizes the project-local copy under `.tools/security`. No package dependency or background service is added.

Enable the tracked hooks once per clone:

```sh
git config --local core.hooksPath .githooks
git config --local push.default simple
git config --local remote.origin.push refs/heads/main:refs/heads/main
```

Git does not enable repository hooks automatically when cloning. See the [Git hook documentation](https://git-scm.com/docs/githooks) and [push configuration](https://git-scm.com/docs/git-config#Documentation/git-config.txt-remoteltnamegtpush).

For routine work on `main`, review the diff, then use ordinary commands:

```sh
git add .
git diff --cached --stat
git commit -m "Describe the change"
git push
```

The final command is for the repository owner after review. Agents must not execute it. The local default sends only `main` to `origin/main`. It does not publish temporary branches or tags. Do not publish all branches, mirror repositories or force updates.

Pre-commit checks staged paths and runs Gitleaks. Pre-push checks outgoing history, requires a normal main-to-main fast-forward, and refuses deletion of `main` and other branch uploads. When the owner asks to publish one branch, set `OPENUSAGE_PUSH_BRANCH=<branch>` for that push; only that branch, under its own name, is accepted, and its commits since `origin/main` are scanned. Missing scanners and timeouts fail the operation with an explanation. Secret values are redacted. These checks reduce risk but cannot prove that no secret exists. Review findings rather than disabling the hooks. Remote branch deletions require the owner's approval and a fresh preservation check.

Recovery and build outputs stay ignored. Never force-add private data to work around an ignore rule. Keep source fixtures synthetic. If a real credential appears in published history, revoke or rotate it with its provider before reviewing history remediation.

Run the small hook suite with the installed scanner:

```sh
node --test tests/tools/git-safety.test.mjs
```

Tests use disposable local repositories and nonfunctional synthetic tokens. They do not contact a remote or validate credentials.

## Bounded development checks

Use one primary agent and one assignment with a clear stopping condition. Follow the [engineering workflow](docs/engineering-workflow.md). Run `python3 scripts/engineering.py doctor` when the environment changes. Use `verify` to inspect the changed-file plan, then `verify --run` for bounded checks. Select relevant renderer specs explicitly. Full native and security acceptance remain required at the affected integration milestone. Stop after the verified local commit and report.

## Keep build storage bounded

Follow the [artifact retention policy](docs/artifact-retention.md). Before a large build, check repository storage:

```sh
python3 scripts/storage.py check --warn-gib 10
```

Preview cleanup before applying it:

```sh
python3 scripts/storage.py clean
python3 scripts/storage.py clean --apply
```

`clean` is a dry run unless `--apply` is supplied. Cleanup is limited to scratch outputs owned by the storage tool's controlled markers. Do not substitute broad recursive deletion, follow symlinks into other locations or remove user databases, credentials, backups, source history or required evidence. Review any refusal instead of bypassing it. On Windows, use `python` in place of `python3` when that is the installed launcher.

Use one shared, absolute `CARGO_TARGET_DIR` at the repository's `.tools/build-target`, and one dependency installation. Do not copy Cargo targets or `node_modules` into agent worktrees. Use the repository's dev/test profiles with incremental caching disabled and line-level debug information. The standalone native verifier declares the same profiles because nested Cargo workspaces do not inherit them. Build it with that same target directory. Enable full debugging information only for a specific investigation, then retire the disposable output through the documented policy.

Normal native verification records receipts; it does not retain another executable. `scripts/freeze-native-artifact.py SOURCE LABEL` records provenance only. Supply the producing `--source-workspace`, `--build-command` and `--test-reference`. Use explicit `--retain` only for active debugging, release qualification or security evidence, within the documented count and size caps. Do not patch verified executable bytes or overwrite an explicitly retained artifact. Reusing the shared build path is normal: seal the receipt and finish its tests before building another configuration. Qualify the resulting binary by its own hash.

After integrating a worktree, preserve its commits with a local branch or tag, confirm that it has no unique tracked, untracked or ignored data, and remove it with `git worktree remove PATH`. Never force removal to discard unique work. Keep one authoritative copy of essential evidence; do not duplicate entire historical evidence trees for each slice.

## Check storage tooling

The tooling uses the Python standard library:

```sh
python3 -m unittest discover -s tests/tools -p 'test_storage.py'
```

The `CI` workflow runs the full check set on Linux; see the [engineering workflow](docs/engineering-workflow.md). The storage-policy workflow runs these tests and a read-only storage check on Linux, Windows and macOS. It performs no native build, artifact upload or remote publication. A workflow definition is not evidence that those platforms have passed; retain the actual results when it runs.

## Native reset-notification checks

Linux notification tests use `python3-dbus` and `python3-gi` to run an isolated synthetic Freedesktop service. They do not show real desktop notifications or read existing credential homes. The explicit `native-test` feature must be enabled for both the desktop application and the standalone fixture verifier to dispatch synthetic events. Normal production must refuse fixture queues, including queues imported from a test workspace. Keep one shared target for both configurations, record each hash, and restore the default production configuration afterward. Never point a test build at real workspaces or provider credential homes.
