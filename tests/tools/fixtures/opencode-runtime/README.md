# OpenCode runtime fixture

This fixture runs the official OpenCode writer with synthetic events on Linux.
It does not run the full CLI, a provider, or a real account.
The scanner still has six qualification failures in this bounded set.

## Executed boundary

The source is `anomalyco/opencode` v1.19.5 at
`810c0eba1a7348036f8bdf61e6632d0c36c6876c`.
The [official source](https://github.com/anomalyco/opencode/tree/810c0eba1a7348036f8bdf61e6632d0c36c6876c) supplies these unchanged modules:

- Event schemas, encoding, matching, durable publication, and replay.
- `SessionProjector`, `SessionMessageUpdater`, and `SessionInput`.
- The generated database schema, migration loader, and SQLite table definitions.
- The Effect database layer, Drizzle adapter, and Bun SQLite driver.

`producer.ts` supplies fixture events through `EventV2.publish`.
It seeds one project row through the official table and database interface.
The official projector creates every session and message row.
The replay case exports the stored events in memory and calls `EventV2.replayAll` twice in a fresh database.
Repeated settlement events update one assistant row.

`bootstrap.ts` routes only `packages/core/src/location.ts` to `location.ts`.
The adapter keeps the official `@opencode/Location` service tag and schema exports.
Each publication supplies an explicit synthetic location.
This bypasses location discovery and its project, Git, and process services.
All other loaded source modules retain their original bytes.

`pin.json` records 117 loaded modules and 131 source files, including package metadata and the upstream lockfile.
It also records 15 registry packages, their locked integrity values, archive hashes, and extracted tree hashes.
The official Effect patch and both resulting files have separate hashes.
File pins use records with separate `path` and `sha256` fields.
This keeps file names separate from hash values.
The Bun 1.3.14 executable matches the project's Linux CI pin.

## Results

`producer-expected.json` records the database oracle for 15 producer cases.
`qualification.json` keeps required totals separate from observed reports.
A passing test preserves these known failures. It does not qualify OpenCode support.

| Cases | Required tokens | Candidate result in both reports |
| --- | ---: | --- |
| Completed, user peer, repeated save, system, replay, copied roots, replayed roots | 173 | 173 |
| Changed save | 190 | 190 |
| Two distinct calls | 346 | 346 |
| Reported zero | 0 | Refused: qualification failure |
| Agent switch, model switch, synthetic message, shell, compaction | 173 | Refused: qualification failure |
| Unfinished assistant, failed assistant without usage | Unknown | Refused: usage remains unavailable |

The candidate is `20.0.48-openusage.opencode-wal-reuse-candidate.1`.
Its SHA-256 is `fa337e6ffc71c56f4647feec0450b47821a7b5f8b4ec1ad7b18ff95ec5f44a8b`.
Each refusal returns exit code 1 and `OpenCode source capture is unavailable`.

The scanner executes 68 times: 17 root selections, two report kinds, and two repetitions.
Each repeated result matches exactly, including error output.
The scanner leaves every source file unchanged.

## Run

Use the pinned source checkout and a verified Bun executable.
The source needs the files in `pin.json` and this dependency link:
`node_modules -> ../opencode-runtime-deps/node_modules`.
The prepared dependencies already exist in the local ignored directory.
For a new checkout, run the separate preparation command once with an absent destination:

```sh
timeout 120 python3 tests/tools/fixtures/opencode-runtime/prepare.py \
  --source "$PWD/.tools/upstream/opencode-v1.19.5" \
  --destination "$PWD/.tools/upstream/opencode-runtime-deps"
```

Preparation downloads the exact pinned registry archives.
It checks SHA-512 integrity and SHA-256 hashes, extracts the packages, and applies the official Effect patch.
It checks each resulting tree before it creates the dependency directory.
It uses a private HOME and XDG paths. It runs no package lifecycle script.
It does not install the full monorepo or copy an existing `node_modules` tree.

Run the optional tests from the project root:

```sh
OPENUSAGE_OPENCODE_RUNTIME_SOURCE="$PWD/.tools/upstream/opencode-v1.19.5" \
OPENUSAGE_OPENCODE_RUNTIME_DEPENDENCIES="$PWD/.tools/upstream/opencode-runtime-deps" \
OPENUSAGE_OPENCODE_RUNTIME_BUN=/home/debrato/.local/bin/bun \
OPENUSAGE_CCUSAGE_OPENCODE_WAL_REUSE_CANDIDATE="$PWD/.tools/build-target/release/ccusage" \
timeout 120 python3 -m unittest discover -s tests/tools -p test_ccusage_opencode_runtime.py -v
```

Without the three runtime paths, the runtime tests skip.
Without the candidate path, the scanner comparison skips.
Tests never download or install packages.

## Isolation and limits

Bubblewrap gives each producer run a private home, XDG folders, temporary directory, process namespace, and network namespace.
A synthetic outside canary stays inaccessible. The network namespace differs from the parent namespace.
The scanner receives only the selected synthetic roots through read-only mounts.
No real credentials, provider sessions, browser data, or existing tool logs enter the fixture.
Each child has a deadline and bounded cleanup. The harness performs no automatic retry.
Temporary databases and raw output disappear after each test.

The fixture uses fresh schemas and explicit SQLite truncating checkpoints before each scan.
Historical migrations, concurrent writers, and live WAL reuse remain outside this runtime check.
The earlier WAL tests retain their separate synthetic scope.

Compaction events write a synthetic summary. No model generates or charges for that summary.
Shell events write synthetic text. No shell command runs.
Fork operations remain unexecuted because this selected writer boundary exposes no fork operation.
The full session service, CLI, provider stream, location discovery, and Git lifecycle also remain unexecuted.
