# Contributing to OpenUsage

Keep changes small and verify the affected workflow. Preserve existing local data, unrelated work and the normative [SRS](docs/SRS.md). This development session permits local commits only: do not push, publish, open pull requests or change a remote.

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

Use one shared, absolute `CARGO_TARGET_DIR` at the repository's `.tools/build-target`, and one dependency installation. Do not copy Cargo targets or `node_modules` into agent worktrees. Use the repository's dev/test profiles with incremental caching disabled and line-level debug information. Enable full debugging information only for a specific investigation, then retire the disposable output through the documented policy.

Normal native verification records receipts; it does not retain another executable. `scripts/freeze-native-artifact.py SOURCE LABEL` records provenance only. Supply the producing `--source-workspace`, `--build-command` and `--test-reference`. Use explicit `--retain` only for active debugging, release qualification or security evidence, within the documented count and size caps. Do not alter an already verified binary in place.

After integrating a worktree, preserve its commits with a local branch or tag, confirm that it has no unique tracked, untracked or ignored data, and remove it with `git worktree remove PATH`. Never force removal to discard unique work. Keep one authoritative copy of essential evidence; do not duplicate entire historical evidence trees for each slice.

## Check storage tooling

The tooling uses the Python standard library:

```sh
python3 -m unittest discover -s tests/tools -p 'test_storage.py'
```

The storage-policy workflow runs these tests and a read-only storage check on Linux, Windows and macOS. It performs no native build, artifact upload or remote publication. A workflow definition is not evidence that those platforms have passed; retain the actual results when it runs.
