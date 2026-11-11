# OpenCode producer fork qualification

L-1f11w, November 11, 2026. The actual pinned producer fork duplicates inherited
usage under new IDs. The scanner fails eight of eleven required root selections.
OpenCode stays disabled. This fixture records the failure without changing the scanner.

## Executed source

The fixture executes `Session.fork` from official OpenCode v1.19.5 at commit
`810c0eba1a7348036f8bdf61e6632d0c36c6876c`.
The [official operation](https://github.com/anomalyco/opencode/blob/810c0eba1a7348036f8bdf61e6632d0c36c6876c/packages/opencode/src/session/session.ts#L691)
runs with the actual message reader, ID generators, event bridge, replay,
projector, SQL schemas, Drizzle, and Bun SQLite.
All 181 source pins match official Git blobs. The fixture loads 154 official modules.

Five import adapters expose schema or service boundaries without the full application.
The exact storage error declaration stays pinned. Two small manifests resolve unchanged
LLM and TUI error utilities. The [fixture guide](../../tests/tools/fixtures/opencode-forks/README.md)
lists each adapter and the execution limits.
Eleven additional registry packages match the official lock, archive integrity, and extracted tree hashes.
The earlier fifteen runtime package trees and dependency link stay unchanged.
Fresh preparation and the earlier runtime tests passed.

Bubblewrap gives each producer and scanner private homes, temporary files,
processes, and network namespaces. A synthetic outside canary stays inaccessible.
The scanner receives database directories only. Observer JSON never enters its mounts.
Source hashes match before and after scanning. Temporary databases and raw output disappear.
No real account, provider, shell operation, or credential home enters this test.

## Stored identity and totals

A parent contains 173 tokens. Full forks and message-bounded forks execute the
producer operation. A separate case adds 17 tokens through the official update methods.
Another case creates two independent calls with equal token fields and timestamps.

| Case | Required tokens | Observed tokens |
| --- | ---: | ---: |
| Parent only | 173 | 173 |
| Full fork | 173 | 346 |
| Boundary after usage | 173 | 346 |
| Boundary before usage | 173 | 173 |
| Fork with new usage | 190 | 363 |
| Two independent calls | 346 | 346 |
| Durable replay | 173 | 346 |
| Copies, replay copies, reversed roots, parent plus fork | 173 | 346 |

Forked messages and parts receive new IDs. The assistant points to a new local
user message. Both sessions store null `parent_id` and null `metadata`.
Stored events carry no original-message ID or fork boundary in these cases.
The operation writes legacy `message` and `part` rows. `session_message` stays empty.

The fixture's observer records the API arguments and their known relationships.
Those facts do not establish stored lineage for a later scanner.
Equal counts, time, content, or titles cannot safely replace missing identity.
The independent-call control confirms that equal counts and time can describe distinct usage.

## Checks and remaining work

All three harness tests passed independently at root on synthetic Linux input.
Eleven root selections, two report kinds, and two repetitions produce 44 scanner runs.
Reports repeat exactly, and copies add no further usage beyond the existing fork excess.
A passing harness preserves eight qualification failures. It does not complete AT-65 or OU-DUP-004.
The [qualification record](../../tests/tools/fixtures/opencode-forks/qualification.json)
keeps required totals separate from observed reports.

The candidate remains `20.0.48-openusage.opencode-records-candidate.1`, with executable
SHA-256 `b6d6f34c7ae1f2465fcca4ea77de9d72d6488adac3198b71b541fab2b18b1a3c`.
The next assignment must establish a safe source-compatibility boundary or a producer origin contract.
No count-based lineage rule is approved.
Live producer WAL capture, historical migrations, scale, grouped storage,
native application integration, real logs, Windows, and macOS remain unqualified.
The production scanner pin and tool catalog stay unchanged.
