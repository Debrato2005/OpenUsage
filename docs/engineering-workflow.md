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

The runner stops at the first failure. One total deadline covers all checks, followed by bounded child cleanup. It never retries. It uses one Cargo target with incremental builds disabled. Missing renderer selections refuse execution. Use `--milestone` for applicable full renderer/workspace regression. Native acceptance, SQLCipher/restore, authentication isolation and SRS gates remain separate requirements.

Logs retain commands, timing, exit codes and fixed diagnostic counters. They discard raw child output. For full diagnostics, rerun the indicated command locally. Logs remain ignored and use the existing ownership-controlled retention: three completed runs, seven days, 16 MiB. No executable copies are retained.

`trace` reads only the selected regular JSONL file. It emits counts and owner-scoped token high-water marks without messages, arguments, identifiers or raw outputs. Copied parent history is flagged. Do not add token snapshots or interpret them as billing. Repeats and category matches indicate review candidates, not proven waste.

Reuse `scripts/storage.py check` and `clean` for storage measurement and bounded cleanup. Cleanup previews by default. Existing secret hooks remain mandatory. CI runs Python tool tests, whitespace and tracked-ignore checks with a five-minute deadline. Hosted Windows/macOS results remain unverified until CI runs.
