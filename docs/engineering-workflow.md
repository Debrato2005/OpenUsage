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
## Local results: October 22, 2026

The selected historical root trace contained 6,873 tool-call records, 87 exact repeats and 95,223,470 bytes of tool output. Heuristics found 211 calls mentioning broad tests and 494 mentioning core documentation reads. These counts describe stored records. They do not prove that each command ran or was redundant. Four recent agent traces also passed analysis. Two contained copied parent history, so their call counts cannot establish independent agent work.

The private summary retains exact cumulative token counters, including cached inputs. These metadata fields are not billing measurements. The utilities make no model calls. We claim no percentage reduction in model tokens.

The same 26 local tool tests passed through manual checks in 1.87 seconds and the new entry point in 1.81 seconds. That difference does not establish a test speedup. Two operator commands became one, with deterministic selection, deadlines and fail-fast behavior.

Five documentation-only fixture cycles took 14 to 16 ms each. Retention reached three runs and then stayed at 2,112 bytes. Tests also covered secret-output exclusion, token snapshot handling, unsafe-link refusal, tracked private artifacts and timeout cleanup of child processes. This tooling-only change needed no application build or native acceptance run.

A mid-task disk comparison measured 86,016 additional allocated bytes. The shared `.tools` allocation stayed unchanged. No build cache or executable copy was created. Existing storage cleanup remains the single cleanup implementation.
