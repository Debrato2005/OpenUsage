# OpenCode fork fixture

This fixture executes the official `Session.fork` service with synthetic data on Linux.
The scanner fails fork qualification. OpenCode stays disabled.

## Executed boundary

The source is `anomalyco/opencode` v1.19.5 at `810c0eba1a7348036f8bdf61e6632d0c36c6876c`.
The [official fork operation](https://github.com/anomalyco/opencode/blob/810c0eba1a7348036f8bdf61e6632d0c36c6876c/packages/opencode/src/session/session.ts#L691) executes unchanged.
The fixture also executes the official message reader, identifier generators, event bridge, durable replay, projector, SQL schemas, Drizzle, and Bun SQLite.

Each parent has one completed assistant message with 173 tokens.
The fixture calls a full fork and two forks with an exclusive message boundary.
It adds 17 new tokens to one fork through the official update methods.
A separate control creates two distinct calls with identical token fields and timestamps.
No provider or shell command runs.

The fork generates new message and part IDs.
It changes the assistant's `parentID` to the fork's user message ID.
Both sessions store null `parent_id` and null `metadata`.
Stored events have no original-message ID or fork-boundary field.
The legacy operation writes `message` and `part`; `session_message` stays empty.

`producer-expected.json` records canonical stored metadata.
Its `operation` field records observer context from API calls. It is not a stored lineage field.
The scanner receives only database directories. It cannot read the observer JSON.
No title, timestamp, equal content, or matching count establishes identity.

## Results

`qualification.json` keeps required totals separate from observed reports.
Both report kinds return the same totals.

| Case | Required tokens | Observed tokens |
| --- | ---: | ---: |
| Parent only | 173 | 173 |
| Full fork or boundary after usage | 173 | 346 |
| Boundary before usage | 173 | 173 |
| Fork with 17 new tokens | 190 | 363 |
| Two independent calls | 346 | 346 |
| Replay, copies, reversed roots, parent plus fork roots | 173 | 346 |

The scanner runs 44 times: eleven root selections, two report kinds, and two repetitions.
Repeated reports match exactly. Source files stay unchanged.
A passing test preserves eight qualification failures. It does not approve tool support.
The candidate version and executable digest are in `pin.json`.

## Import adapters

The fixture replaces five module boundaries:

- Location exports retain the official schema and service tag. The fixture supplies a synthetic instance context.
- Core session exports use the official session schema directly.
- Core project exports use the official project schema directly.
- Snapshot exports use the official file-diff schema directly.
- Storage exports retain the exact pinned `NotFoundError` declaration span.

LLM and TUI error imports resolve to unchanged official utility modules through two fixture manifests.
The fork, message reader, event bridge, projector, database, and identifier implementations remain unchanged.
The full CLI, providers, Git lifecycle, location discovery, and snapshot operations do not execute.

## Run

The setup uses the existing runtime dependencies. It preserves their fifteen package trees and the source dependency link.
Run the separate preparation command with an absent destination:

```sh
timeout 240 python3 tests/tools/fixtures/opencode-forks/prepare.py \
  --source "$PWD/.tools/upstream/opencode-v1.19.5" \
  --runtime-dependencies "$PWD/.tools/upstream/opencode-runtime-deps" \
  --destination "$PWD/.tools/upstream/opencode-forks-deps"
```

Preparation checks locked SHA-512 integrity, archive hashes, extracted trees, and official source hashes.
It reads pinned local Git objects first, then downloads absent official source files with four workers.
It runs no install script and reads no package-manager configuration.
Tests never download or install dependencies.

```sh
OPENUSAGE_OPENCODE_RUNTIME_SOURCE="$PWD/.tools/upstream/opencode-v1.19.5" \
OPENUSAGE_OPENCODE_RUNTIME_DEPENDENCIES="$PWD/.tools/upstream/opencode-runtime-deps" \
OPENUSAGE_OPENCODE_RUNTIME_BUN=/home/debrato/.local/bin/bun \
OPENUSAGE_OPENCODE_FORKS_DEPENDENCIES="$PWD/.tools/upstream/opencode-forks-deps" \
OPENUSAGE_CCUSAGE_OPENCODE_RECORDS_CANDIDATE="$PWD/.tools/build-target/release/ccusage" \
timeout 120 python3 -m unittest discover -s tests/tools -p test_ccusage_opencode_forks.py -v
```

Without the four producer paths, the runtime tests skip. Without the scanner path, only its comparison skips.
Bubblewrap supplies private HOME, XDG paths, temporary files, processes, and network namespaces.
A synthetic outside canary stays inaccessible.
Each child has a deadline and bounded cleanup. No automatic retry runs.
Temporary databases and raw output disappear after each test.
Fresh schemas and truncating checkpoints keep historical migration and concurrent-writer qualification outside this test.
