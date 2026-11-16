# Architecture

OpenUsage is a Tauri 2 desktop app. Rust owns everything sensitive. React only displays typed results. [SRS Section 2](SRS.md) fixes the technology.

[Data contracts](data-contracts.md) maps record ownership, exact values, missing
metrics, interface versions, and planned evidence fields.

[The draft identity contract](identity-contract.md) describes attribution limits,
association review, duplicate handling, and the gates for verified aggregation.

## Data path

All data follows one path (OU-DES-009).

| Step | Job |
|---|---|
| Adapter | Reads one source |
| Checks | Validate format, range, and identity |
| Reading | One typed result with a state and a status |
| Store | SQLCipher database with one write coordinator |
| View | React, through typed commands |

Today Codex is the only source, and its path has no general adapter layer. The planned `openusage-sources` crate adds that layer without adding a second path.

## Reading evidence

The Codex parsers prepare immutable response metadata in Rust.
The desktop binds that metadata to the actual method, run, association, provider version, executable origin, and observation time.
Storage requires matching context for every new activity or quota acceptance, including delayed observations.
A combined collection keeps one evidence record for each accepted method.
All evidence writes share the transaction for association, readings, history, caches, and alerts.

Schema 14 adds complete evidence and explicit markers for historical gaps.
The evidence lookup distinguishes a complete record, a historical gap, an unknown run, and corrupt absence.
SQL rejects updates. Existing deletion cascades remove evidence with its collection run.
Backup validation and trusted copies preserve both tables.
The read-only command loads one record through a typed profile, method, and run target.
The existing Overview detail panel requests evidence only after Show. No provider process runs for this read.
The [evidence access contract](data-contracts.md#evidence-access) owns request, response, cache, and absence rules.
All three selected export routes use the same typed evidence reader inside their deferred snapshot.
The [selected audit contract](data-contracts.md#selected-reading-audit-fields) owns the scalar fields and schema version.
The renderer receives only the existing file receipt.
The [data contract](data-contracts.md#reading-evidence) owns field meanings and version rules.
The [open historical conflict](srs-clarifications.md#sc-031-evidence-for-historical-readings) approves no SRS exception.

## Code map

| Path | Owns |
|---|---|
| `apps/desktop/src/` | React UI with three pages: Overview, Usage, Settings |
| `apps/desktop/src-tauri/src/` | Typed commands, monitoring, local log consent and scans, share images, notifications, key handling, provider control, export, backup, restore |
| `crates/openusage-codex/` | Codex protocol, response parsing and evidence projections, approved executable checks, owned child processes |
| `crates/openusage-storage/` | SQLCipher 4 with WAL, migrations, a read-only open for the CLI, quota, activity, alerts, subscriptions, local log totals, sessions and hours, exports |
| `crates/openusage-import/` | Encrypted backup archive format |
| `crates/openusage-locallogs/` | Pinned `ccusage` binary checks, folder detection, the hardened runner, and the strict daily, session and hourly report parsers for local logs |
| `crates/openusage-cli/` | The `openusage` command, with no webview dependency |
| `crates/openusage-keys/` | Separate OS credential slots and Linux enrollment coordination |
| `crates/openusage-board/` | Bounded public transport, origin-bound enrollment, and departure sessions |
| `services/leaderboard/` | The opt-in leaderboard Worker and its D1 schema |
| `packaging/` | The npm launcher, its README, and third-party notices for the packages |
| `tests/contract/` | IPC contract checks |
| `tests/renderer/` | Playwright in Chromium and WebKit with synthetic IPC |
| `tests/native/` | Journeys against the real app |
| `tests/tools/` | Script tests |
| `scripts/` | Verification, storage cleanup, Git safety, companion preparation |
| `vendor/glib-0.18.5/` | Local security backport, explained in [SECURITY.md](../SECURITY.md) |

The [local tool catalog](../crates/openusage-locallogs/sources/tools.json) defines tool names, order, colors, enabled state, roots, folder variables, and report capabilities. Rust embeds it at compile time. The renderer imports it at build time. Detection, consent, the runner, and both interfaces use this one definition. Adapter and fixture checks must pass before a tool is enabled.

The catalog also declares supported provider connections. Codex declares the existing OpenAI connection. Scanner activation keeps its separate gate.

The local logs crate owns the [additional period contract](../crates/openusage-locallogs/README.md#additional-period-reports) in `agent_period.rs`. The [grouped runner](../crates/openusage-locallogs/README.md#grouped-scans) collects all required reports before storage.

The disabled Pi path requires one candidate snapshot command. `agent_snapshot.rs` validates its envelope and both reports before the grouped runner returns data. The candidate derives both views from the same captured entries. A failed command or report returns no accepted scan and preserves saved history.

The Linux-only `pi-native-qualification` feature also enables `native-test`.
Detection uses its private log home. A separate descriptor selects the bounded
Pi candidate and checks its digest before each execution. The production
descriptor keeps its original pin. The normal build does not allow Pi consent
or execution.

The isolated [Codex candidate](reviews/2026-11-12-codex-combined-capture.md)
uses ordered scanner patches. Individual reports select the strict paginated
loader before legacy replay. The snapshot command uses strict capture directly
and derives both JSON reports from the same event vector. The
[candidate pin](../tests/tools/fixtures/codex-snapshot-candidate/pin.json) owns
source and executable fingerprints. The isolated native feature tests application
parsing and dispatch before production activation.

`codex_snapshot.rs` parses the combined candidate envelope into the existing
grouped types. It reuses strict envelope and model types, rejects duplicate
model keys, and preserves required null fields. The [crate contract](../crates/openusage-locallogs/README.md#codex-candidate-snapshot)
owns count and estimate rules. This pure parser starts no process and changes
no application dispatch.

The isolated Linux Codex feature selects one pinned candidate snapshot. It uses
the existing typed grouped save and read paths. Ordinary builds keep source
scans with the production scanner. The first grouped scan pins its own zone.
Legacy history remains separate under SC-024.

Schema 13 adds six tables for grouped scopes, scans, root membership, records, model details, and lower-scan differences. Legacy tables stay unchanged. One tool has one stable scope. Only day records enter dated queries. Period and session totals stay separate. Every accepted record points to the scan that produced it. A lower scan keeps the whole saved record and its model details. An equal or higher scan replaces them together.
 A complete scan with an absent saved period records an observed zero difference. Missing measurements remain null.

The writer checks every root's tool and consent in the save transaction. Scope zones stay pinned. Backup checks enforce the same bounds and metric rules as a write. Restore copies only declared fields. Migration adds empty grouped tables and leaves all legacy records intact. No legacy model sum becomes a new scanner total.

Grouped provenance accepts existing numeric version spellings and bounded
SemVer prerelease tags. Both Rust storage and the typed renderer contract use
shared accepted and rejected cases. Tags must fit 128 ASCII bytes. Legacy
validation and schemas stay unchanged. The worker saves the version from the
selected descriptor. Reopen, export, backup validation, and restore use the
same grouped checks.

See [SC-024](srs-clarifications.md#sc-024-legacy-local-usage-during-grouped-migration) for the legacy-history rule. `local_periods.rs` owns these Rust APIs. The desktop worker has a grouped path for candidate tools. They remain disabled. Claude Code and Codex keep their source views, with one transaction for all required reports. Desktop and CLI views use the shared read result.

The shared reader in `local_usage_read.rs` returns grouped dated data, unplaced data, and legacy source rows as separate collections. It returns no grand total across those collections. Each grouped scope keeps its scan evidence and its zone. The reader selects dated sessions by their reported last activity in that zone. It keeps untimed sessions outside the selected dates. Session totals never enter period totals.

A range with no saved periods has no total. A saved zero stays zero. Optional categories and estimates remain unavailable if any included period lacks them. Incomplete model attribution leaves model totals unavailable for the whole selected range. All sums use exact Rust integers. The result preserves source zones and lower-scan differences. One read transaction gives a consistent view without a database write.

Desktop and sharing views use the shared read result. Saved history can select a dated tool scope, an undated scope, or one legacy folder. Combined history needs one generation, one source per legacy tool, and common day boundaries. Otherwise its total is unavailable. The detail panel keeps each original folder value and zone accessible.

Token categories, model totals, reasoning counts, and session times cross IPC as nullable values. The renderer accepts saved records from the full catalog without enabling a scanner. Undated usage stays outside charts, records, and share images. Legacy sessions and hours retain their source filter. Missing days and saved zero days remain distinct in charts and shared heatmaps.

Share images use the same selected dated scope and its zone. Both PNG and SVG saves check the scope again. Images exclude folder paths, projects, and identity fields. An incomplete model breakdown has no top model. Backup and export retain the stored fields under the current schema.

The CLI reads today's value separately for each saved scope and zone. Legacy, grouped, and unplaced records never enter one combined total. Its [version 2 JSON](../crates/openusage-cli/README.md#output) gives each local measurement an exact value or null, its state, source, status, and observation time. Legacy maxima have no per-record time. Their latest scan time stays separate. `report` scans all detected roots of each enabled tool together, filters the full daily report, and saves nothing.

## Owned scanner shutdown

The [local-log runner](../crates/openusage-locallogs/README.md#cancellation-and-cleanup)
uses one-way cancellation separate from consent. Unix capture checks a bounded
nonblocking output read between cancellation and deadline checks. Cleanup signals
only the retained child and uses a two-second reap budget. Failed reports do
not enter storage.

`shutdown.rs` handles `ExitRequested` once. It signals local scanning and
monitoring before it joins workers outside the GUI thread and workspace lock.
Final exit follows cleanup. A cleanup failure gives a generic diagnostic and
nonzero exit code. The [native record](reviews/2026-11-13-owned-scanner-shutdown.md)
covers Linux only. Tray, autostart, and wake handling remain open.

## Profile removal

The Rust-only `Storage::remove_profile` API accepts a profile UUID, its expected
managed home UUID, a history choice, and an opaque `StorageRevision`.
One immediate transaction checks the revision, exact native profile, and home.
The API refuses a revision after any intervening write or storage reopen.
It also refuses a revision from another connection. A wrong home or environment also refuses the operation.

Archive changes the selected profile archive flag. Delete removes its bindings,
provider readings and history, alert history and outbox, overrides, and local
subscription rows. It then removes the profile. A temporary ID table scopes
shared notification identities without a history-sized Rust allocation.
Foreign keys remain enabled. A cross-profile dependency or statement failure
rolls back the entire operation. No persistent schema change is required.

Both choices preserve unrelated profiles, shared account records, workspace
settings, global alert preferences, independent log sources and local usage,
and durable submission and incident journals. The journals remain in complete
exports and backups. The operation never accesses homes, credential files,
provider sessions, processes, or remote subscriptions. Earlier backups can
restore old history. [SC-026](srs-clarifications.md#sc-026-profile-removal-and-retained-delivery-controls)
records this reading of the requirements.

The desktop adds three strict commands: `prepare_profile_removal`,
`apply_profile_removal`, and `cancel_profile_removal`. One cached Rust approval
binds its random UUID to the exact profile, home, choice, and storage revision.
React receives only the approval UUID, profile UUID, label, and history choice.
Replacing, cancelling, or consuming the approval prevents its reuse.

Apply checks consent and the revision before process cleanup. Under the workspace
lock, it revokes the selected monitoring grant, joins that profile's provider
operations, and clears its pending requests, reviews, and accepted session caches.
Other profiles retain their work. The same lock orders notification claims,
bounded submission, completion, and removal. Storage checks the revision again
inside its immediate transaction. Failed cleanup never starts record deletion.
The outgoing view is prepared before commit, so a later read cannot turn a
successful deletion into a reported read failure.

The app dialog offers Archive History and Delete History with explicit consent.
A stale or consumed request requires a new review. Restore and whole-workspace
deletion clear pending profile consent. [The native record](reviews/2026-11-12-profile-removal-app.md)
owns executed Linux scope. Real accounts and other platforms remain unverified. WSL removal stays
unavailable. Native calls on other platforms can outlive their timeout.

## Settings leaderboard worker

The desktop owns one blocking Rust worker for leaderboard operations.
It starts idle and makes no automatic service request.
The collapsed Settings section does not request status.
Opening it reads local enrollment metadata only.
Missing build configuration leaves that metadata unchecked rather than absent.
An explicit refresh inspects the credential store without contacting the service.

One operation holds the shared credential guard through authorization, preview, transport, and local cleanup.
The worker keeps the device flow, access token, service token, and retained upload in Rust.
React receives public challenges, operation IDs, exact upload previews, and safe results through strict commands.
The browser command accepts an operation ID and opens only the fixed GitHub device page.
The renderer cannot supply a URL, credential, or replacement upload body.

Sync gets one owned body from the shared storage builder before it releases the workspace lock.
The preview waits up to five minutes for explicit confirmation.
Confirmation identifies the retained operation and cannot replace its bytes.
An expired, cancelled, or consumed preview requires a new operation.
No storage or worker lock spans an HTTP request.

Cancellation and public dispatch use the same mutex.
Cancellation first prevents public mutation and discards late authorization.
Dispatch first settles the real result and finishes local saving or cleanup.
Quit closes admission, signals all owned workers, and joins leaderboard work before application exit.
Transport deadlines bound requests. They do not establish one total shutdown budget.
Local deletion holds an idle admission lease and the shared credential guard.
The lease prevents new work. It clears cached enrollment metadata after credential preparation succeeds.
Earlier preflight refusals preserve the confirmed receipt.
Fresh setup in the same process cannot reuse an earlier login or receipt.

The shared ClientContext owns ordinary build configuration and the separate native-test fixture checks.
DepartureSession owns fresh departure and exact-token local cleanup for CLI and desktop callers.
Local forgetting takes the guard before credential erasure and constructs no HTTP client.
It does not require service configuration.


## Own-rank reads

Overview opens Self-reported rank through the existing detail panel.
Launch, navigation, and the access button's initial render make no rank request.
Opening the panel requests one read. Period, metric, and Refresh controls request later reads.
One pending queue survives dialog and Overview remounts.
Closed selections skip dispatch, and obsolete replies cannot change the visible result.
Rank failure leaves provider capacity and local value available.

The existing Rust worker owns a separate typed read job and reply.
The job uses the same configured client, credential guard, admission check, and shutdown path as enrollment.
It reads only an existing enrollment. It never joins, uploads, or changes saved usage.
No GUI, storage, or control lock spans HTTP.
The public reply carries no credential, URL, private device code, or provider identity.
The worker drops read credentials before it sends the reply.

An active read blocks enrollment changes and local deletion.
A late cancellation of a finished enrollment operation preserves that receipt and leaves the read alone.
Quit drops queued replies, settles the active bounded request, and joins the owned worker.
A stopped receiver returns a safe stopped result.
The read does not replace the last enrollment receipt or persist a rank snapshot.


## Boundaries

- **Leaderboard credentials:** the [separate vault](../crates/openusage-keys/README.md#leaderboard-credential-storage) preserves the database key slot. The [session coordinator](../crates/openusage-board/README.md#serialized-enrollment) owns pending and active records, origin binding, expected-value replacement, and the shared Linux guard. CLI and Settings callers keep that guard across each complete operation.
- **Departure recovery:** the [shared departure session](../crates/openusage-board/src/departure.rs) holds the shared guard while fresh GitHub authorization removes public data. The service's numeric identity and exact token witness control separate remote and local effects. Explicit local forgetting constructs no HTTP client. The [service contract](../services/leaderboard/README.md#api) owns transactional checks and scheduled rate-counter cleanup.
- **Local enrollment erasure:** the [desktop coordinator](../apps/desktop/src-tauri/src/leaderboard_credentials.rs) owns the guard through local deletion. The existing durable intent precedes erasure of the selected slot. It has no HTTP client or public departure path. Linux requires a usable credential store even for password workspaces. Other-platform enrollment remains unqualified.
- **Secrets:** provider sessions, credentials, and the database key stay in Rust, provider-managed homes, or the OS credential store. They never reach React.
- **Processes:** the Rust supervisor owns every child process. At most two collections run at once.
- **Numbers:** Rust does exact arithmetic. Token counts cross IPC as decimal strings. A missing value stays missing.
- **Renderer:** React calls only registered typed commands. It has no direct file or process access. Its content security policy allows `blob:` images only so that a share preview can show the Rust-made SVG as an image, which runs no script, and draw it as PNG.
- **Network:** The official Codex child process makes provider requests. The separate [leaderboard client](../crates/openusage-board/README.md) uses the approved [HTTP policy](decisions/2026-11-13-leaderboard-http-client.md). Missing build settings leave it inert. Explicit CLI, Settings, and Overview actions use it. Private website sync remains unbuilt.
- **Storage:** one service coordinates all writes. Current state and history are separate.
- **Read snapshots:** `Storage::leaderboard_upload` owns the complete saved-data upload transaction on read-write and query-only connections. Concurrent writes cannot mix source, scope, and row generations. The method returns owned bytes and ends the transaction before transport. The separate generic `read_snapshot` still accepts query-only callbacks.

## Planned

| Piece | Location | SRS |
|---|---|---|
| Source adapters and evidence | `crates/openusage-sources/` | 5A, 5B, 5D |
| Local logs through bundled `ccusage` | An adapter in the same crate | 5C |
| One-command install | npm packages | 16A |
| Private sync and responsive companion | Not built; architecture decision required | 14B, 14C, Stage 3 |

Adding a tool shall take one adapter module, its fixtures, and its declaration (OU-DES-011).
