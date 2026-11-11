# OpenCode producer runtime qualification

L-1f11u, November 11, 2026. The real OpenCode writer now runs through isolated
synthetic events. The scanner matches eleven of seventeen required outcomes.
Six valid cases fail qualification. OpenCode stays disabled.

## Executed boundary

The fixture uses OpenCode v1.19.5 at
`810c0eba1a7348036f8bdf61e6632d0c36c6876c`. Root checked all 131 pinned source
files against that official Git commit. The fixture executes 117 unchanged
source modules and fifteen integrity-bound registry packages, including the
official Effect patch. The [fixture guide](../../tests/tools/fixtures/opencode-runtime/README.md)
owns setup, commands, pins, and the exact execution boundary.

Synthetic events pass through actual durable publication, replay,
`SessionProjector`, `SessionMessageUpdater`, Drizzle, and Bun SQLite.
Only location context uses an import adapter. It retains the official service
tag and schema exports while each publication supplies a synthetic location.
The full CLI, provider stream, location discovery, Git lifecycle, and fork
operations remain unexecuted.

Bubblewrap isolates the home, XDG folders, process namespace, and network.
The outside canary stays hidden. The scanner reads only selected private
sources through read-only mounts. Each database takes an explicit truncating
checkpoint before scanning. This runtime fixture does not test concurrent
writers or live WAL. Earlier WAL evidence retains its separate scope.

## Results

All three harness tests passed independently at root. Fifteen producer cases
also passed database integrity and row checks. Seventeen root selections,
two report kinds, and two repetitions produce 68 scanner executions.
Every repeated result matches and every source hash stays unchanged.
The [record](../../tests/tools/fixtures/opencode-runtime/qualification.json)
keeps independent required totals separate from observed refusals.

| Required outcome | Result |
|---|---|
| Completed usage, user peer, repeated save, system metadata, replay, copied roots, replayed roots: 173 | Passed |
| Changed save: 190. Two distinct calls: 346 | Passed |
| Explicitly measured zero: one zero observation | Refused. Qualification failure. |
| Agent switch, model switch, synthetic message, shell, compaction: preserve 173 | Refused. Five qualification failures. |
| Unfinished or failed assistant without usage: unavailable | Correct refusal |

Passing regression tests preserve these six known failures. They do not
qualify OpenCode support. The next correction must preserve measured zero and
valid metadata while it keeps malformed or unknown usage unavailable.
Full producer forks, scale, grouped storage, native app integration, real
logs, Windows, and macOS remain unqualified.

## Rebuilt scanner

The runtime uses the unchanged WAL reuse candidate and its qualified digest.
The rebuild exposed two required inputs: the pinned compiler and the source
root Cargo configuration. A stale native SQLite library also required targeted
output cleanup. The corrected build reproduced the qualified binary exactly.
The [workflow](../engineering-workflow.md#scanner-candidate-builds) owns these
build rules. No scanner source, production pin, catalog, or dependency version
changes in this slice.
