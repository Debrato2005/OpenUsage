# Implementation slices

Updated: November 12, 2026. This page owns the order and boundaries of planned work. The [SRS](SRS.md) owns what each requirement says, and [status](implementation-status.md) owns what works today.

## Run a slice

Start Codex:

```sh
codex --no-daemon -C /home/debrato/Projects/OpenUsage -m gpt-6-astra \
  -c 'model_reasoning_effort="xhigh"' \
  -c 'agents.default_subagent_model="gpt-6-luna"' \
  -c 'agents.default_subagent_reasoning_effort="medium"' \
  -s workspace-write -a on-request
```

Then send:

```text
Do slice S-01 in docs/implementation-slices.md. Follow its rules. Stop at its stopping condition.
```

These results come from `codex-cli` 0.166.0 on the Linux host on November 3, 2026:

| Check | Result |
|---|---|
| Model, effort, and both subagent settings | Accepted in `codex --no-daemon exec` |
| Astra subagent tool | Takes a model and an effort |
| Writes inside the repository | Allowed in the sandbox |
| TypeScript check | Passes in the sandbox |
| Writes to `.git` and the home folder | Blocked |
| Network, including loopback | Blocked, so Playwright cannot start in the sandbox |
| Cargo in the sandbox | Not tested |
| The interactive command above and its approval flow | Not tested |

Renderer tests, native journeys, and the local commit therefore need approval to run outside the sandbox.

## Rules for every slice

[AGENTS.md](../AGENTS.md) applies, including OU-DES-001 and OU-ENG-012. These rules add to it.

- Do one slice. Read the requirements it names, not the whole SRS.
- Keep existing behavior, stored data, and safety rules (OU-DES-002).
- Add a setting, screen, state, or dependency only under OU-DES-013.
- Reuse local caches where practical. Download required source and build dependencies from official sources at pinned revisions or locked versions. Check published digests when available. New dependencies still need the architecture review above. The owner approved these downloads on November 8, 2026.
- Use synthetic data and private temporary homes. Never point a test at real logs, real credentials, or the owner's accounts.
- Make each schema change a tested forward migration. Older backups shall still restore.
- Record a conflict or an unclear requirement in [SRS clarifications](srs-clarifications.md), then report it. Do not edit the SRS.

## Subagents

- Astra owns design, shared contracts, Rust storage and security code, migrations, and review of every diff.
- Luna takes bounded work with named files: inventories, mechanical test edits, fixtures, and first drafts.
- One agent owns each file. Two agents never edit the same file.
- Subagents do not start subagents, and they do not decide what a requirement means.

## Done

- The slice's tests pass, or the report names each one that did not run and why.
- The [workflow](engineering-workflow.md) checks pass. UI changes also need the full renderer suite in Chromium and WebKit. Rust changes also need the full workspace tests.
- UI changes are checked by eye in screenshots: light, dark, a 720-pixel window, and empty, populated, and error states. Empty fixtures hide layout problems, so always check real-looking data. Each screen answers one question, its main action is obvious, and nothing is clipped or repeated.
- Docs are updated as the [documentation map](README.md) lists, and the slice is marked Done below.
- The slice has one local commit. Then report, and start the next open item in the [build checklist](#build-checklist). The owner asked for this on November 8, 2026.

## Launch track

Adopted November 7, 2026 (SRS Section 0.6, SC-016). The owner wants a working product to launch for feedback, with the leaderboard in the first release. These slices come first, in this order. Each still follows the rules above and ends with one local commit.

| Slice | Goal | Existing slices it covers | State |
|---|---|---|---|
| L-0 | First run without an app password: keychain by default | S-03c | New workspaces already use the keychain without a prompt (`workspace.rs` setup); moving an existing password workspace follows the launch |
| L-1 | Local logs with no sign-in: the pinned official `ccusage` binary, a Rust runner, the local usage store, consent, and This device views | S-09, S-10, S-11, part of S-05 | L-1a to L-1e and L-1g1 to L-1g3 done. L-1f is next. L-1g4 waits for the owner. See the [checklist](#build-checklist) |
| L-2 | CLI with `openusage status`, `statusline` and `report` | S-14 | Done |
| L-3 | Sharing: Wrapped summary, heatmap PNG and SVG, a README badge, and personal records | S-19 | Done |
| L-4 | Leaderboard service on Cloudflare Workers and D1, per the [decision](decisions/2026-11-07-leaderboard-service.md) | S-21 | Done locally. Deployment waits for the owner |
| L-5 | Leaderboard client: join with GitHub, dry run, sync, leave | S-22 | Waits for the owner |
| L-6 | Packaging: Linux packages, Windows and macOS preview builds in CI, the npm package | S-18, part of S-15 | L-6a and L-6b built. Preview builds and platform pins remain open under L-6c |
| L-7 | Release documents and the launch checklist | S-20 | L-7a through L-7d built. Remaining documents and evidence are open under L-7e |
| G-L | Owner: claim the npm name (OQ-2), create the Cloudflare account and GitHub OAuth App, run the real-account tests, publish | G-1, G-2, G-3 | Not started |

The other slices below follow the launch, unless a launch slice needs part of one.

## Build checklist

Updated November 12, 2026. Each completed item has one local commit and scoped evidence. L-7e1 records the latest scoped result. The remaining work stays below. Owner items wait for the owner.

**Verification before release**

- [x] Fix portable Codex fixtures, renderer countdown clocks, and quota fixture readiness. See [traceability](acceptance-traceability.md#evidence-log) for scope and results.
- [x] The hosted renderer job for `b1b8d84` passed all 456 cases.
- [x] The hosted Linux and Renderer gates for `b8cb3cc` passed, including Pi qualification and native oracle checks. See the [record](acceptance-traceability.md#evidence-log).

**L-0 First run**

- [x] New workspaces use the OS key store with no app password.
- [ ] Move an existing password workspace to the key store. This follows the launch.

**L-1 Local logs**

- [x] L-1a: pin, provision, run and parse `ccusage` (OU-LOG-005 to OU-LOG-007).
- [x] L-1b: store totals and differences at schema 10 (OU-LOG-003, OU-LOG-010 to OU-LOG-013).
- [x] L-1c: scan service. Detect known folders by existence only, keep each run to the consented tools, scan after consent and then on a schedule, one run at a time, and keep a failure inside local logs (OU-LOG-001, OU-LOG-006, OU-LOG-008, OU-DUP-008, OU-NFR-021; AT-70 to AT-72).
- [x] L-1d: the first-launch consent screen and the Settings selection (OU-LOG-001, OU-LOG-002; part of AT-72).
- [x] L-1e: the This device scope in Usage, with 7, 30 and 90 days or a year, grouped by day, week or month, a breakdown by token type and model, and the difference in the detail panel, so first launch reaches local history with no sign-in (OU-LOG-011, OU-LOG-016, OU-LOG-017, OU-UI-025; AT-91, AT-93).
- [ ] L-1f: the other 16 tools that the pinned `ccusage` reads, with detection, isolated runs, strict parsing, and synthetic fixtures (AT-65). The [source review](reviews/2026-11-08-l1f-source-research.md) locates the exact pinned source through read-only web requests. Source inspection needs no archive download. The review also identifies count, cost, session, and folder-isolation gaps that must pass the listed acceptance criteria before this item is complete.
- [x] L-1f0: require calculated API-price estimates for every report (OU-LOG-006, OU-LOG-013, OU-LOG-014). Stored charges no longer break a scan or replace its estimate. The pinned-binary regression, full Rust checks, and native consent journey passed with synthetic logs. The [tool comparison](reviews/2026-11-08-usage-tracking-strategy.md) explains this choice. The other 16 tools still need L-1f.
- [x] L-1f1: check the resolved consent root before process launch (OU-LOG-001, OU-LOG-006, OU-DUP-008). Reject ambiguous path text and roots that now resolve elsewhere. The pinned-binary regression, full Rust checks, and native consent journey passed with synthetic logs. Spaces inside a path, Unicode names, and links resolved before consent still work. This adds no tool.
- [x] L-1f2: share one tool catalog across detection, consent, the runner, the CLI, and the renderer (OU-DES-011). It declares all 18 tools, but enables only the two already qualified. The full renderer and Rust checks passed. The rebuilt native app passed consent and read-only CLI journeys with private synthetic data on Linux. [SC-022](srs-clarifications.md#sc-022-incomplete-metrics-from-the-other-local-tools) records the approved handling of incomplete metrics for L-1f.
- [x] L-1f3: record daily and session outputs for synthetic inputs from all 16 remaining tools. Three tests ran the real pinned binary 98 times. Reports repeated exactly, empty roots returned no rows, and source files stayed unchanged. A copied Amp thread doubled 162 tokens to 324. This is a known AT-65 failure, not completed tool support. See the [fixtures](../crates/openusage-locallogs/tests/fixtures/remaining-tools/README.md) and [SC-023](srs-clarifications.md#sc-023-copied-amp-usage-in-the-pinned-scanner).
- [x] L-1f4: reject malformed Unicode timestamps without a panic and reject impossible calendar dates (OU-LOG-007). Session and hourly parsers share the UTC check. The date check covers month lengths and leap years. Synthetic regressions first reproduced both Unicode panics and acceptance of February 30. Full Rust checks and the rebuilt native consent journey passed on Linux.
- [x] L-1f5: add the strict common session parser and nullable metric types for the sixteen disabled tools. Eight focused tests cover recorded reports, incomplete model attribution, empty reports, reported zero totals, and malformed values. The diagnostic workspace run, formatting, Clippy, renderer build, IPC contracts, and rebuilt native consent journey passed on Linux. The initial workspace failure did not recur and its cause remains unconfirmed. Storage and scanner integration remain open.
- [x] L-1f6: prepare and test an unpublished Amp duplicate patch against the pinned source. Stable-ID copies count once. Conflicts and incomplete reads fail. All 28 adapter tests and 210 scanner comparisons passed. The production pin stays unchanged. Remaining AT-65 and integration gates remain open. See the [candidate record](reviews/2026-11-08-amp-dedup-design.md#local-candidate-after-review).
- [x] L-1f7: preserve independent period totals and nullable details in a strict report contract. Amp, Pi, Kilo, and ZCode use daily reports. The other twelve tools stay unplaced. Seven focused tests, full Rust checks, 98 scanner comparisons, and the rebuilt native consent journey passed with synthetic data on Linux. No tool or production pin changed.
- [x] L-1f8: store independent period totals, coherent model details, and nullable session metadata at schema 13. Preserve legacy history and zones under [SC-024](srs-clarifications.md#sc-024-legacy-local-usage-during-grouped-migration). Migration, backup, restore, full Rust checks, and nine native journeys passed on synthetic Linux data.
- [x] L-1f9: group consented roots by tool in the scanner and save each accepted result once. Validate every root and all required reports before saving. Preserve prior data when any required report fails.
- [x] L-1f10: carry the new totals and unavailable fields through desktop, CLI, export, backup, restore, and sharing. Keep unplaced usage out of dated views. Run the affected renderer and native gates.
- [x] L-1f10a: shared read results for grouped periods, unavailable details, unplaced usage, and separate legacy history.
- [x] L-1f10b: desktop and sharing views use independent totals, nullable details, and explicit saved scopes. Undated usage stays outside dated charts and shares. Original folder history remains accessible.
- [x] L-1f10c: CLI schema 2, separate saved scopes, complete per-tool reports, and shared-read export, backup, and restore round trips.
- [ ] L-1f11: qualify each tool with copied, forked, replayed, and compacted fixtures under AT-65. Complete the Amp candidate gates before any pin change. Enable only qualified tools. L-1f stays open until all sixteen pass their required checks.
- [x] L-1f11a: record Kilo copy, fork, replay, compaction, and incomplete-read results. Regression checks pass, but Kilo fails AT-65 and stays disabled. See the [qualification](reviews/2026-11-10-kilo-qualification.md).
- [x] L-1f11b: remove ZCode's fallback project label and record 19 qualification cases. Copy, fork-transcript, replay, and compaction counts match. Conflicts and incomplete reads keep ZCode disabled. See the [qualification](reviews/2026-11-10-zcode-qualification.md).
- [x] L-1f11c: record 29 Pi cases and preserve missing fields in Rust. Copy, branch, and omitted-usage defects fail AT-65. Pi stays disabled. See the [qualification](reviews/2026-11-10-pi-qualification.md).
- [x] L-1f11d: prepare a separate local Pi usage patch. Summary and standalone usage counts match. Identity and completeness gates remain open, so Pi stays disabled. See the [candidate](reviews/2026-11-10-pi-usage-candidate.md).
- [x] L-1f11e: prepare a separate Pi identity patch. Scoped IDs and proven lineage count copies once and retain distinct calls. Ambiguous and unsupported inputs reject the scan. Completeness gates remain open. See the [candidate](reviews/2026-11-10-pi-identity-candidate.md).
- [x] L-1f11f: scoped Pi format and filesystem corrections. Controlled changes and input limits pass. L-1f11h covers observation semantics. Candidate integration and release-scale performance remain qualification gates under L-1f11.
- [x] L-1f11f0: map validated Pi records, preserve escaped JSON keys, reject ambiguous shapes, and keep independent assistant totals outside missing categories. See the [format candidate](reviews/2026-11-10-pi-format-candidate.md).
- [x] L-1f11f1: bound Linux source reads and reject omitted or changed inputs. See the [filesystem candidate](reviews/2026-11-10-pi-filesystem-candidate.md).
- [x] L-1f11g: build Pi daily and session views from one captured entry set and require a strict snapshot envelope. See the [candidate](reviews/2026-11-10-pi-snapshot-candidate.md).
- [x] L-1f11h: preserve reported zero observations and reject absent required usage, totals, or times. Seventy synthetic cases and saved-history checks passed. See the [candidate](reviews/2026-11-10-pi-observations-candidate.md).
- [x] L-1f11i: run the pinned Pi writer and fourteen controlled schedules. Stable operations and twelve overlap rejections pass. SC-025 corrects the earlier classification of two valid prefixes. Pi stays disabled. See the [original review](reviews/2026-11-10-pi-producer-qualification.md).
- [x] L-1f11j: select the captured-record boundary under SC-025. Correct the earlier producer-completion qualification assumption and test a separate completion protocol. See the [decision](decisions/2026-11-10-pi-capture-boundary.md).
- [x] L-1f11k: qualify the pinned Pi candidate through the grouped Rust scanner and storage path under SC-025. See the [executed evidence](acceptance-traceability.md#evidence-log).
- [x] L-1f11l: measure the pinned Linux Pi candidate through the grouped scan and save path. See the [performance report](performance-report.md#pi-candidate-on-linux).
- [x] L-1f11m: test the Pi input cap and whether separate reports can preserve identity. Select a [bounded capture design](decisions/2026-11-11-pi-large-history.md) for a prototype. Pi stays disabled.
- [x] L-1f11n: build the [bounded Pi prototype](reviews/2026-11-11-pi-bounded-capture.md). Test exact counts and safe refusal on synthetic Linux histories. Pi stays disabled.
- [x] L-1f11o: measure release Pi capture and enabled Claude Code history through native consent on synthetic Linux input. See the [performance report](performance-report.md#linux-release-local-history). Reference and platform gates remain open.
- [x] L-1f11p: record 26 [OpenCode qualification cases](reviews/2026-11-11-opencode-qualification.md). Fourteen fail the required observation. OpenCode stays disabled.
- [x] L-1f11q: preserve OpenCode usage without model or provider labels and keep explicit zero observations. The [candidate](reviews/2026-11-11-opencode-observation-candidate.md) corrects three cases. Capture qualification remains open.
- [x] L-1f11r: reject conflicting and incomplete explicit OpenCode captures on Linux. All 26 required outcomes pass in the [capture candidate](reviews/2026-11-11-opencode-capture-candidate.md). Producer, WAL, scale, and app integration gates remain open.
- [x] L-1f11s: preserve committed OpenCode WAL pages through private in-memory reconstruction. The [WAL candidate](reviews/2026-11-11-opencode-wal-candidate.md) passes thirteen CLI tests. Ordinary reused tails, producer execution, metadata, scale, and app integration remain open.
- [x] L-1f11t: accept ordinary reused WAL tails only with a matching committed boundary. The [reuse candidate](reviews/2026-11-11-opencode-wal-reuse-candidate.md) passes sixteen CLI tests. Producer execution, metadata, scale, and app integration remain open.
- [x] L-1f11u: execute the pinned real OpenCode writer with isolated synthetic events. The [runtime qualification](reviews/2026-11-11-opencode-runtime-qualification.md) records seventeen selections and six qualification failures. Correct zero and metadata before further enablement work.
- [x] L-1f11v: preserve measured OpenCode zero and validate five non-usage metadata types. The [record candidate](reviews/2026-11-11-opencode-records-candidate.md) passes all seventeen actual writer outcomes and strict refusal guards. Forks, scale, and app integration remain open.
- [x] L-1f11w: execute actual pinned OpenCode forks, bounded forks, new fork usage, and durable replay. The [fork qualification](reviews/2026-11-11-opencode-fork-qualification.md) preserves eight failures across eleven selections. Stored fork origin is absent in the tested legacy format. OpenCode stays disabled.
- [x] L-1f11x: restrict a separate OpenCode candidate to the pinned fresh v2 format. The [source boundary](reviews/2026-11-11-opencode-v2-boundary.md) preserves seventeen writer outcomes and refuses legacy or mixed captures as a whole. All 166 adapter tests and fourteen CLI tests passed. General OpenCode qualification remains open.
- [x] L-1f11y: record [Codebuff qualification](reviews/2026-11-11-codebuff-qualification.md). Eight of thirty cases match both reports. The reviewed CLI format does not establish per-call token measurements. Codebuff stays disabled.
- [x] L-1f11z: record [Droid qualification](reviews/2026-11-11-droid-qualification.md). Eight of twenty-eight session outcomes match. Filename identity and lock-time ordering fail qualification. Droid stays disabled.
- [x] L-1f11aa: record [Hermes qualification](reviews/2026-11-11-hermes-qualification.md). Nine of twenty-eight session outcomes match. Conflicts, usage coverage, and stored cost prevent enablement. Hermes stays disabled.
- [x] L-1f11ab: record [Goose qualification](reviews/2026-11-11-goose-qualification.md). Five of twenty-two session outcomes match. A selected database link reaches a synthetic unselected source. Goose stays disabled.
- [x] L-1f11ac: record [Gemini qualification](reviews/2026-11-11-gemini-qualification.md). Eleven of thirty-six cases match both reports. Copies, conflicting IDs, missing counts, and capture failures prevent enablement. Gemini stays disabled.
- [x] L-1f11ad: record [Qwen qualification](reviews/2026-11-11-qwen-qualification.md). Ten of thirty-four cases match both reports. Count fingerprints lose distinct calls and duplicate inherited forks. Counter overlap and capture failures also prevent enablement. Qwen stays disabled.
- [x] L-1f11ae: record [Kimi qualification](reviews/2026-11-11-kimi-qualification.md). Nine of thirty-eight cases match both reports. Copied forks, skipped request scopes, and ambiguous new-format identities prevent enablement. Kimi stays disabled.
- [x] L-1f11af: record [OpenClaw qualification](reviews/2026-11-12-openclaw-qualification.md). Fourteen of fifty-two cases match both reports. Distinct events collapse, inherited usage repeats, and unavailable metrics remain numeric. OpenClaw stays disabled.
- [x] L-1f11ag: record [Copilot qualification](reviews/2026-11-12-copilot-qualification.md). Five of forty-eight cases match both reports. Copies, conflicting records, ignored checkpoints, and partial capture prevent enablement. Copilot stays disabled.
- [x] L-1f11ah: record [Grok qualification](reviews/2026-11-12-grok-qualification.md). Twenty of forty-six session outcomes match. Conflicting IDs, overlapping parent/child usage, incomplete records, and partial capture prevent enablement. Grok stays disabled.
- [x] L-1f11ai: record [Antigravity qualification](reviews/2026-11-12-antigravity-qualification.md). Seventeen of thirty-seven session outcomes match. Conflicting maxima, missing metrics, partial reads, and an unqualified writer contract prevent enablement. Antigravity stays disabled.
- [x] L-1f11aj: reconcile all sixteen additional-tool records with the catalog and candidate gates. The [gate review](reviews/2026-11-12-additional-tool-gates.md) selects Pi's native application path next. The production pin and enabled tools stay unchanged.
- [x] L-1f11ak: exercise the pinned Pi candidate through a native qualification build, including consent, worker scans, exact persistence and display, provenance, refusal, and withdrawal. Keep production support unchanged.
- [x] L-1f11al: compare the shared bounded release candidate with the production scanner for enabled Claude Code and Codex. Keep production support unchanged.
- [x] L-1f11am: qualify captured Codex event identity with private producer output before an isolated scanner correction.
- [x] L-1f11an: correct the tested Codex paginated format in a separate candidate with explicit record identity and lineage.
- [x] L-1f11ao: derive both Codex reports from one captured input in a separate candidate.
- [x] L-1f11ap: parse the Codex candidate envelope into existing typed grouped records without scanner activation.
- [x] L-1f11aq: qualify the combined Codex candidate through native consent, grouped storage, history, and failure handling.
- [x] R-8: review saved CLI connection-state display with public commands before any correction.
- [x] R-6: correct CLI text for an empty selected date range after a public-command regression.
- [x] R-7: reject future quota observations as Current after a public-command regression.
- [x] R-5: bound leaderboard request collection at 64 KiB. Twenty-four local service tests passed, including overflow cancellation, byte boundaries, stream errors, and the earlier aggregate and concurrency checks. Hosted Workers remain unverified.
- [x] R-4: enforce hourly limits and the five-device cap under concurrent requests. Fourteen local service tests passed, including token rotation, hour resets, and database failures. Hosted D1 remains unverified.
- [x] R-3: keep a leaderboard estimate unavailable when any included row lacks it. Token totals, measured zero, ordering, and period filters stay intact. Eight local service tests passed. Hosted D1 remains unverified.
- [x] R-2: reuse the bounded writer in desktop synthetic fixtures. The inherited-writer regression, 22 provider tests, full Rust, and native provider-lifecycle passed. The original cooldown failure remains unexplained. See the [review](reviews/2026-11-10-desktop-fixture-writes.md).
- [x] R-1: use bounded test-fixture writes to prevent inherited writable handles from blocking execution. Full Rust and native provider-lifecycle checks passed. See the [investigation](reviews/2026-11-10-executable-fixture-race.md).
- [x] L-1g1: sessions, projects and the reasoning count from the `ccusage` session report of each tool, at schema 11 (OU-LOG-017, [SC-021](srs-clarifications.md#sc-021-sessions-projects-hours-and-reasoning-from-the-pinned-ccusage)).
- [x] L-1g2: hourly ranges for Claude Code from `ccusage blocks`, at schema 12 (OU-LOG-017, SC-021).
- [x] L-1g3: custom date ranges (OU-LOG-017).
- [ ] L-1g4: session quota movement (OU-LOG-018). Waits for the owner's OQ-1 decision, because a local session names no provider account (SC-021).

**Profile lifecycle**

- [x] S-08a1: guarded archive and scoped deletion in storage, with retained data and rollback checks.
- [x] S-08a2: explicit history choices in the app, selected work cleanup, and synthetic Linux native acceptance. See the [record](reviews/2026-11-12-profile-removal-app.md). Other platforms and real-provider removal remain unverified.

**L-2 CLI**

- [x] L-2a: the `openusage` binary with no webview, with `report` (one consent prompt, `--yes`, nothing saved), `app` and `doctor`, stable error codes and exit codes, and help for each command (OU-CLI-001, OU-CLI-003, OU-CLI-005, OU-CLI-008, OU-DIST-004; part of AT-77).
- [x] L-2b: `openusage`, `status --json` and `statusline` from saved data, with versioned JSON and Locked without a prompt (OU-CLI-002, OU-CLI-004, OU-CLI-006, OU-CLI-007; AT-78, AT-79).

**L-3 Sharing**

- [x] L-3a: the Wrapped summary and the heatmap as SVG and PNG, made on the device, with a preview and an explicit save (OU-SHARE-001 to OU-SHARE-006; AT-82).
- [x] L-3b: the README badge and personal records.

**L-4 Leaderboard service**

- [x] L-4a: the Worker in `services/leaderboard/` with join, sync, leave and public ranks, strict schemas and limits, tested against SQLite with no network (OU-BOARD-003, OU-BOARD-004, OU-BOARD-007 to OU-BOARD-011).
- [x] L-4b: the moderation policy beside the source.

**L-5 Leaderboard client**

- [ ] L-5a: `openusage board join`, `sync --dry-run`, `sync` and `leave` through one upload builder, with the token in the OS credential store (OU-BOARD-001, OU-BOARD-002, OU-BOARD-005, OU-BOARD-006; AT-86 to AT-88). Downloads are allowed under the slice rules. The HTTPS client still needs an architecture decision because Rust currently declares no HTTP client.
- [ ] L-5b: join, sync and leave in Settings.

**L-6 Packaging**

- [x] L-6a: Linux packages with the bundled `ccusage` and Codex companion, and the WebKitGTK dependencies listed (OU-PLAT-002, OU-PLAT-005).
- [x] L-6b: the npm package, with per-platform packages and no install scripts (OU-DIST-001 to OU-DIST-005, OU-DIST-009; AT-81).
- [ ] L-6c: Windows and macOS preview builds in CI (part of OU-NFR-012). The owner approved required build downloads on November 8. Obtain each official toolchain archive. Check its digest. Each platform also needs qualified companion pins. This host has only the Linux Rust standard library. Hosted builds and real-machine tests remain open.

**L-7 Release documents**

- [x] L-7a: install, upgrade, uninstall and data deletion steps, unsigned-build behavior, and the OU-SEC-022 statement (OU-PLAT-007, OU-DIST-006, OU-DIST-008, OU-SEC-022).
- [x] L-7b: the threat model (SRS Section 14.1).
- [x] L-7c: the SBOM (OU-SEC-016, part of OU-DIST-005).
- [x] L-7d: the release notes, with the signing state of each build (OU-PLAT-003, OU-PLAT-004).
- [ ] L-7e: complete the remaining Section 20 documents and evidence. These cover data and identity contracts, provider capabilities, security and privacy, compatibility, performance, and provider evidence. Record unexecuted platform and live-account gates explicitly.

- [x] L-7e1: document current data contracts, exact values, missing metrics, source ownership, all 42 tables, and format versions. Link planned SRS gaps without claiming implementation. [Data contracts](data-contracts.md) owns the result.

**Owner (G-L)**

- [ ] Claim the npm name `openusage` and the `@openusage` scope (OQ-2).
- [ ] Create the Cloudflare account and the D1 database, then deploy the service.
- [ ] Register the GitHub OAuth App and give its public client ID.
- [ ] Run the real-account tests (AT-60) and the real-machine tests (AT-38, AT-80).
- [ ] Decide the two gaps in [SC-017](srs-clarifications.md#sc-017-local-usage-store).
- [ ] Approve an HTTPS client with TLS for the leaderboard client (L-5), for example `ureq` with `native-tls`, which can use the OpenSSL that SQLCipher already builds.
- [x] The owner approved build downloads under the slice rules. Record the toolchain digests and companion pins for Windows and macOS during L-6c.
- [ ] Push and publish.

**After the launch track**

- [ ] The slices in [Order](#order). Stage 1 needs every mandatory requirement (SRS Section 0.4). The next launch item is L-1f. S-03c remains the first item after the launch track with no owner gate.

### Completed assignment: L-1f7

Goal: finish the typed report contract that storage integration needs. Follow OU-LOG-007, OU-LOG-013, OU-LOG-017, and [approved SC-022](srs-clarifications.md#sc-022-incomplete-metrics-from-the-other-local-tools).

Scope: the local-log parser, catalog policy, and synthetic report fixtures. Start with the [existing session parser](../crates/openusage-locallogs/src/agent_report.rs) and the [incomplete-metrics proposal](reviews/2026-11-08-incomplete-local-metrics.md). That proposal is design input, not completed implementation.

Acceptance:

- Preserve independently reported totals when model details are incomplete. Do not derive a total from missing buckets or assign a remainder to reasoning.
- Check daily reports for Amp, Pi, Kilo, and ZCode against the recorded fixtures. Keep the other twelve tools unplaced under the catalog policy.
- Reject unknown shapes, malformed dates and counts, and inconsistent totals. Keep unavailable buckets, times, projects, and estimates absent.
- Pass focused fixtures and the full Rust workspace checks. Update the owning documents with the executed scope.

Stopping condition: the contract and checks are complete, with one local commit and a Graphify update. Storage integration starts in L-1f8. This slice enables no tool and changes no production pin.

### L-1f8 storage contract (complete)

Goal: store independent period totals and nullable details without losing accepted history. Follow OU-LOG-003, OU-LOG-010 to OU-LOG-013, OU-LOG-017, and [SC-022](srs-clarifications.md#sc-022-incomplete-metrics-from-the-other-local-tools).

Scope: Rust storage, a forward migration, backup compatibility, and synthetic storage fixtures. [SC-024](srs-clarifications.md#sc-024-legacy-local-usage-during-grouped-migration) governs legacy history. The [storage proposal](reviews/2026-11-08-incomplete-local-metrics.md) remains design input.

Acceptance:

- Keep period totals separate from model details. Replace each accepted snapshot coherently, without adding independent model maxima.
- Preserve older backups, exact accepted history, and lower-scan differences. Keep absent categories, estimates, projects, and session times absent.
- Keep one unplaced period per accounting scope. Never assign it an activity date or include it in dated views.
- Pass migration, backup, restore, full Rust workspace, and affected native checks.

Stopping condition: one verified storage slice, owning docs, a local commit, and a Graphify update. Grouped scans start in L-1f9.

### L-1f9: grouped scans (complete)

Goal: group consented roots by tool and save each complete result once. Follow OU-LOG-001, OU-LOG-003, OU-LOG-006 to OU-LOG-013, OU-LOG-017, OU-DUP-008, SC-021, SC-022, and SC-024.

Scope: Rust scanner grouping, required report validation, atomic storage, and synthetic fixtures. Preserve the working Claude Code and Codex paths while the new consumers are prepared. Keep the Amp candidate unpublished and the production pin unchanged.

Acceptance:

- Group only the selected roots of one tool. Check resolved roots and consent again before saving.
- Use the pinned scanner and the approved period rules. Keep session and hourly readings separate from period totals.
- Save complete results with their scope and root membership. A failed required report must preserve accepted values.
- Test copied roots, empty reports, malformed data, revoked consent, changed paths, and zones with synthetic inputs.
- Pass the selected Rust and native checks. Enable no extra tool in this slice.

Stopping condition: one verified scanner slice, owning docs, a local commit, and a Graphify update. Consumer integration follows in L-1f10.

### L-1f10: consumer integration

Goal: carry independent totals and unavailable fields through desktop, CLI, export, backup, restore, and sharing. Follow SC-022 and SC-024, with the owning local usage, data safety, CLI, and sharing requirements. Keep legacy history separate and preserve the working Claude Code and Codex paths.

Acceptance:

- Use independent totals. Preserve nullable details, model attribution, and session times through each boundary.
- Keep unplaced usage out of date filters, charts, heatmaps, streaks, hourly sums, and dated shares.
- Preserve legacy source details and zones. Never sum legacy records with grouped snapshots or claim a combined history without proof.
- Preserve working Claude Code and Codex session and hourly views. Show missing fields as unavailable. Infer no account identity.
- Pass affected Rust, renderer, contract, and native checks. Verify export, backup, restore, and share privacy. Enable no extra tool.

Stopping condition: one verified consumer slice, owning docs, a local commit, and a Graphify update. AT-65 qualification follows in L-1f11.

### L-1f10a: shared read contract (complete)

Goal for L-1f10a: give Rust consumers one read result that preserves independent totals, unavailable details, legacy source history, and unplaced usage. Follow SC-022 and SC-024. Change no schema, scanner pin, consent rule, or enabled tool.

Acceptance:

- Keep grouped periods, unplaced periods, and legacy source records in separate typed collections. Do not infer a combined total.
- Sum independent period totals with exact integers. A missing category or model count remains unavailable across the selected range.
- Use each scope's pinned zone for dated sessions. Keep sessions without an activity end outside date filters. Never add session totals to period totals.
- Preserve lower-scan differences, root membership, legacy source zones, and reported zero. Read all parts in one transaction, including through a read-only connection.
- Pass the focused and full Rust checks and the affected native read-only journey. Keep consumer integration open.

Stopping condition: one verified read-contract slice, owning docs, a local commit, and a Graphify update. Desktop and sharing integration follows in L-1f10b.

### L-1f10b: desktop and sharing (complete)

Goal: connect desktop and sharing views to the shared read results. Follow SC-022 and SC-024 and the local usage, UI, and sharing requirements. The CLI and remaining data round trips follow in L-1f10c. Keep the Amp candidate unpublished and the production pin unchanged.

Acceptance:

- Carry independent totals and unavailable details through Rust IPC and the renderer. Keep model attribution and nullable session times intact.
- Keep dated data, unplaced data, and legacy source history separate. Use each scope's pinned zone. Show no combined historical total without proof.
- Keep the old source values accessible in the existing detail panel. Preserve the working session and hourly views.
- Exclude unplaced usage from dated charts, records, heatmaps, and shares. Preserve missing days and share privacy.
- Pass affected Rust and contract checks, the full renderer milestone, and the affected native journeys. Enable no extra tool.

Stopping condition: one verified desktop and sharing slice, owning docs, a local commit, and a Graphify update. CLI integration follows in L-1f10c.

### L-1f10c: CLI and round trips (complete)

Goal: connect CLI output to independent totals and unavailable fields. Complete the remaining shared-read checks across export, backup, and restore. Follow SC-022, SC-024, and OU-CLI-001 to OU-CLI-008. Keep the Amp candidate unpublished and the scanner pin unchanged.

Acceptance:

- Saved status and statusline use read-only data. Preserve each source zone, separate legacy and grouped accounting, and keep undated usage outside today's totals.
- Version the JSON changes. Keep absent measurements null with their state, source, status, and observation time. Never derive account identity from local metadata.
- Report keeps one consent prompt, accepts --yes, and saves nothing. Reuse complete scan rules where they apply. Enable no extra tool.
- Round trips preserve independent totals, nullable details, retained records, scan evidence, and legacy history in shared read results.
- Pass the selected verification plan and affected native CLI, export, backup, and restore checks with private homes and synthetic data.

Stopping condition: one verified CLI and round-trip slice, owning docs, a local commit, and a Graphify update. Per-tool qualification follows in L-1f11.

### R-1: executable fixture reliability (complete)

Goal: resolve the intermittent startup failure in the synthetic cleanup-fault test before tool qualification. The failure returns StartFailed before CleanupFailed. Production cleanup logic stays unchanged unless evidence proves a defect there.

Acceptance:

- Use private temporary homes and synthetic providers. Inspect no real logs, accounts, or credential homes.
- Capture the underlying process or reader-thread startup error. Compare runtime evidence with the fixture and process-launch source.
- Fix the proven cause and add a focused regression. Keep deadlines, child ownership, cleanup assertions, and security checks intact.
- Run doctor, the selected verification plan, and affected native checks. Record all diagnostic trials and any unexecuted gates.

Stopping condition: one verified reliability fix, owning docs, a local commit, and a Graphify update. Then continue L-1f11 with bounded per-tool qualification.

### L-1f11a: Kilo qualification record (complete, AT-65 failed)

Goal: qualify Kilo using the pinned source, actual scanner reports, and synthetic fixtures. Keep all sixteen candidates disabled while qualification is incomplete. The Amp candidate stays unpublished, and the production scanner pin stays unchanged.

Acceptance:

- Compare the source's identity and count rules with independent expected totals and actual daily/session reports.
- Check copied roots, forked records, replays, compaction, empty inputs, missing fields, and conflicting records. Never invent event IDs, times, reasoning counts, or account identity.
- Preserve independent period totals and unavailable detail fields. Keep consent and root isolation intact.
- Fix defects within OpenUsage's boundary with focused regressions. Record upstream defects and candidate fixes without changing the production pin or enabling a tool that fails AT-65.
- Run doctor, the selected verification plan, and affected acceptance checks. Record failures and unexecuted gates with their platform and data scope.

Stopping condition: one bounded Kilo qualification result, owning docs, a local commit, and a Graphify update. Then continue the next candidate.

### L-1f11b: ZCode project correction and qualification record (complete, enablement failed)

Goal: qualify ZCode with source inspection and private synthetic fixtures. Compare independent totals with actual daily and session reports. Keep missing measurements unavailable under SC-022.

Acceptance:

- Check copied roots, forks, replays, compaction, empty inputs, unknown prices, missing fields, and incomplete reads in the supported source format.
- Compare format and identity claims with primary source. Do not invent event IDs, dates, reasoning counts, or account identity.
- Preserve consent and root isolation. Keep unavailable fields through the existing Rust contract.
- Fix defects within OpenUsage's boundary. Record upstream gaps and required candidate fixes. Keep unqualified tools disabled and the production pin unchanged.
- Run doctor, the selected verification plan, and affected acceptance checks. Record failures and unexecuted gates.

Stopping condition: one bounded ZCode qualification result, owning docs, a local commit, and a Graphify update. Then continue the next open slice.

### L-1f11c: Pi qualification record (complete, AT-65 failed)

Goal: qualify Pi with source inspection and private synthetic fixtures. Compare independent totals with actual daily and session reports. Preserve unavailable measurements under SC-022.

Acceptance:

- Check copied roots with and without entry IDs, forks, replay, compaction, empty inputs, missing fields, and incomplete reads.
- Verify the producer's ID and lineage rules from primary source. Do not infer duplicate identity from paths, equal counts, or time alone.
- Preserve consent isolation, independent totals, and unavailable details through Rust parsing. Keep the other twelve tools unplaced.
- Fix defects within OpenUsage's boundary. Record upstream gaps and required candidate fixes. Keep unqualified tools disabled and the production pin unchanged.
- Run doctor, the selected verification plan, and affected acceptance checks. Record failures and unexecuted gates.

Stopping condition: one bounded Pi qualification result, owning docs, a local commit, and a Graphify update. Then continue the next open slice.

### L-1f11d: local Pi usage candidate (complete within scope)

Goal: make a local Pi scanner candidate preserve usage on compaction, branch summaries, and standalone usage entries. Use the pinned source and the recorded producer contract. This slice does not change production support.

Acceptance:

- Preserve independent reported totals for those record types. Treat context size as context, never as a new call.
- Keep absent models, timestamps, token categories, and prices unavailable. Do not infer reasoning or account identity.
- Add focused adapter tests and run the candidate against synthetic comparisons. Preserve the current pinned-binary results as the baseline.
- Keep the Amp candidate and production pin unchanged. Record remaining identity, lineage, incomplete-read, and format gates.
- Run doctor, the selected verification plan, and affected acceptance checks. Record failures and unexecuted checks.

Stopping condition: one verified local candidate change, owning docs, a local commit, and a Graphify update. Then continue the next unresolved qualification issue.

### L-1f11e: local Pi identity candidate (complete within scope)

Goal: make the local Pi candidate count copied ID-bearing records once while retaining distinct calls. Check forks against proven session lineage. Preserve the usage correction from L-1f11d.

Acceptance:

- Use the producer's scoped entry IDs and session lineage. Different sessions with coincident short IDs remain separate unless a source link proves shared history.
- Count renamed copies once and retain distinct IDs with equal usage. Reject conflicting usage and ambiguous lineage without a successful partial total.
- Cover active and inactive branch forks, full clones, multiple roots, reversed input order, and missing legacy identity. Declare unsupported cases explicitly.
- Preserve missing metrics, source isolation, and saved history after a rejected scan. Keep all candidates disabled and the production scanner pin and the Amp patch unchanged.
- Run doctor, focused adapter and CLI comparisons, the selected OpenUsage verification plan, and affected acceptance checks.

Stopping condition: one verified local identity correction, owning docs, a local commit, and a Graphify update. Then continue the next unresolved qualification issue.

### L-1f11f: Pi completeness and format corrections (complete within scope)

Goal: prevent the Pi candidate from accepting partial totals after omitted, changed, or unsupported source inputs. Preserve the identity and usage corrections.

Acceptance:

- Trace discovery and parsing from the consented roots through the recorded reports. Triangulate each gap against producer source and synthetic behavior.
- Reject incomplete selected roots and file reads. Read no extra parent path or real source. Preserve saved history when a required scan fails.
- Check coherent input under controlled changes and bounded handling of large or malformed records. Do not add automatic retries.
- Test escaped JSON fields, unsupported versions and shapes, absent times, and missing metrics. Use explicit unavailable outcomes where the contract lacks reliable data.
- Keep all candidates disabled, the production scanner pin unchanged, and the Amp patch unpublished. Run doctor, focused checks, the selected verification plan, and affected native acceptance.

Stopping condition: one verified local completeness change, owning docs, a local commit, and a Graphify update. Then continue the next unresolved qualification issue.

### L-1f11f0: Pi format candidate (complete within scope)

Goal: make the Pi report mapper preserve every supported usage record after source validation. Discovery and coherent filesystem reads follow in L-1f11f1.

Acceptance:

- Reproduce escaped JSON field names, independent header metadata, invalid optional fields, and ambiguous duplicate keys against the current candidate.
- Parse each supported source record without a byte-filter shortcut. Reject invalid shapes without a partial total. Do not infer token categories from an independent total.
- Preserve source IDs, explicit unavailable times, nullable OpenUsage details, and all prior valid copy/fork results.
- Keep candidates disabled, the production pin unchanged, and the Amp patch unpublished. Run doctor, focused adapter/CLI checks, the selected OpenUsage plan, and native consent.

Stopping condition: one verified local format correction, owning docs, a local commit, and a Graphify update. Then start L-1f11f1 for discovery, coherent reads, and input bounds.

### L-1f11f1: Pi filesystem candidate (complete within scope)

Goal: return a complete coherent Pi source snapshot or an explicit unavailable result. Preserve the usage, identity, and format corrections.

Acceptance:

- Reproduce omitted explicit roots and directory read errors using private synthetic inputs. Discovery errors must not become empty or partial totals.
- Do not follow source links outside selected roots. Check directory and file replacement during controlled reads, with no automatic retry.
- Bound file counts, source bytes, nesting, and cumulative token arithmetic. Reject unsupported or special files before reading their contents.
- Keep absent timestamps unavailable and preserve the last saved usage on failed scans. Document supported platform scope and any remaining acceptance gates.
- Keep candidates disabled, the production pin unchanged, and the Amp patch unpublished. Run doctor, focused adapter/CLI checks, the selected OpenUsage plan, and affected native acceptance.

Stopping condition: one verified local filesystem correction, owning docs, a local commit, and a Graphify update. Then continue the next unresolved qualification issue.

### L-1f11g: Pi report consistency (complete within scope)

Goal: make the Pi required-report boundary reject an inconsistent scan. Preserve the usage, identity, format, and filesystem corrections.

Acceptance:

- Trace producer writes, candidate report generation, and OpenUsage's required-report contract. Compare source findings with independent synthetic expectations.
- Check stable reports, append, rewrite, and replacement between required commands. Do not invent a common timestamp or assume unchanged input from equal totals.
- Preserve the last saved usage on a rejected scan. Keep missing measurements unavailable and existing Claude/Codex behavior intact.
- Correct the boundary where the existing contract permits it. Record any remaining snapshot requirement explicitly before candidate enablement.
- Keep candidates disabled, the production pin unchanged, and the Amp patch unpublished. Run doctor, focused checks, the selected verification plan, and affected native acceptance.

Stopping condition: one verified report-consistency result, owning docs, a local commit, and a Graphify update. Then continue the next unresolved qualification issue.

### L-1f11h: Pi observation semantics (complete within scope)

Goal: qualify the Pi candidate's handling of zero usage and absent times. Preserve the completed single-capture report boundary and saved history.

Acceptance:

- Compare the pinned producer contract, candidate source, and independent synthetic expectations for explicit zero, absent usage, absent categories, and absent or invalid times.
- Keep empty sources, measured zero, and unavailable measurements distinct. Never infer a timestamp, token category, reasoning count, or account identity.
- Correct supported cases within the approved requirements. Reject cases that cannot yield a reliable required report. Preserve saved data and document any unresolved requirement conflict.
- Preserve valid copy/fork results and the single-capture report boundary. Keep the other twelve tools unplaced, all candidates disabled, the production pin unchanged, and the Amp patch unpublished.
- Run doctor, focused adapter/parser comparisons, the selected verification plan, and affected native acceptance. Record failures, interruptions, and unexecuted checks.

Stopping condition: one bounded observation-semantics result, updated owning docs and handoffs, a local commit, and a Graphify update. Report the next open slice and stop.

### L-1f11i: Pi producer qualification (investigation complete, capture scope clarified by SC-025)

Goal: qualify the Pi candidate against pinned producer execution and controlled concurrent writes. Use private synthetic sessions without real accounts or provider calls.

Acceptance:

- Run the pinned producer's session writer with synthetic append, summary, branch, and fork operations. Compare its files with independent usage expectations.
- Exercise bounded, controlled writes during candidate collection. Each result must contain a reliable complete report or return unavailable without replacement data.
- Preserve zero observations, missing measurements, source isolation, duplicate handling, and the single-capture report boundary. Preserve saved history after rejected scans.
- Keep Pi disabled, the other twelve tools unplaced, the production scanner pin unchanged, and the Amp patch unpublished. Record any unresolved contract or runtime gap.
- Run doctor, focused comparisons, the selected verification plan, and affected native checks. Record failures and unexecuted checks without automatic retries.

Stopping condition: one bounded producer-execution result, updated owning docs and handoffs, one local commit, and a Graphify update. Report the next open slice and stop.

### L-1f11j: Pi capture boundary (decision complete)

Goal: determine whether Pi can expose a reliable write-completion boundary. Resolve the incomplete-prefix gate without weakening SC-022 or saved-history rules.

Acceptance:

- Compare the pinned producer's write contract with the two failed L-1f11i schedules. State what a complete scan can prove.
- Evaluate a bounded completion protocol, such as coordinated publication or locking. Test a local candidate only if its guarantees cover the recorded failures.
- Distinguish support for the unmodified producer from a proposed producer integration. Do not infer completion from delays, unchanged bytes, or equal totals.
- Preserve source isolation, exact counts, unavailable measurements, and saved history. Keep Pi disabled, the other twelve tools unplaced, and all production pins unchanged.
- Record the supported path or remaining dependency. Run doctor, relevant focused checks, the selected verification plan, and affected native checks for any correction.

Stopping condition: one supported-boundary decision with executed evidence, updated docs and handoffs, one local commit, and a Graphify update. Report and stop.

### L-1f11k: Pi grouped scanner and storage integration (complete within scope)

Goal: verify the Pi candidate through the grouped Rust scanner and storage path at the captured-record boundary in SC-025.

Acceptance:

- Use the actual digest-pinned candidate and private producer fixtures. Carry its snapshot through the grouped Rust runner, strict parser, and storage APIs.
- Check empty input, measured zero, a valid complete-record prefix, absent categories, and the later completed write. Never claim producer idleness or invent a missing metric.
- Reject a partial or malformed record, changed consent, failed source read, and conflicting identity. Preserve accepted totals and original zones after failure or lower scans.
- Preserve copy/fork handling, source isolation, and the shared input for required reports. Keep Pi disabled, the other twelve tools unplaced, the production pin unchanged, and Amp unpublished.
- Run doctor, focused integration checks, the selected verification plan, and affected native acceptance. Record remaining qualification and release gates accurately.

Stopping condition: one verified scanner/storage integration result, updated docs and handoffs, one local commit, and a Graphify update. Report and stop.

### L-1f11l: Pi scale test (complete on synthetic Linux)

Goal: measure the pinned Linux Pi candidate through the grouped scan and save
path with large, private synthetic histories.

Acceptance:

- Record the root, file, entry, and byte counts for each workload. Check exact
  totals at representative sizes and safe refusal at the existing limits.
- Measure scanner wall time, CPU, and peak memory separately from the app.
  Compare with the runner deadline and output bound.
- Preserve higher saved history after a failed scan. Keep Pi disabled and the
  production pin unchanged.

Stopping condition: one Linux performance record, an updated handoff, and one
local commit. Report and stop.

### L-1f11m: Pi large histories (complete)

Goal: evaluate how a bounded Pi scan could serve large histories under
OU-NFR-020.

Acceptance:

- Compare the pinned candidate limits with the proposed 1 GiB first-history
  target. Identify which limit prevents a complete observation.
- Evaluate a bounded approach that preserves copy and fork handling, private
  source selection, exact totals, and one captured input for required reports.
- Test the deciding assumption with private synthetic input. Record the limits
  of the result. Keep Pi disabled and preserve the production pin.

Stopping condition: one evidence-backed decision, an updated handoff, and one
local commit. Report and stop.

### L-1f11n: Pi bounded capture prototype (complete)

Goal: keep one capture across all approved roots under OU-NFR-020. Bound raw
input and identity state. Check copy, fork, and conflict handling before
aggregation. Compare the previous candidate on the same input. Test exact
reports, safe refusal, and saved history at the larger byte limit. Keep Pi
disabled and preserve the production pin. See the [prototype record](reviews/2026-11-11-pi-bounded-capture.md).

### L-1f11ak: native Pi candidate qualification

Goal: exercise the pinned bounded Pi release candidate through the native
application without changing production support.

Acceptance:

- Use an explicit qualification build and a digest-pinned candidate descriptor.
  Production detection, consent, execution gates, and binary pins stay unchanged.
- Use the existing consent, worker, grouped storage, and typed view paths.
  Persist the actual candidate version and verify it after reopen.
- With private synthetic input, check exact saved and displayed totals, copies,
  malformed-input refusal, saved-data retention, and consent withdrawal.
  Preserve unavailable details and the pinned source zone.
- Prove private-home and selected-root isolation. Do not read real histories,
  provider sessions, or credentials.
- Run doctor, the staged plan, full affected Rust and renderer checks, and the
  rebuilt native journey. Record platform scope and every remaining gate.

Stopping condition: one verified native candidate result, updated owning docs,
a local commit, and an AST Graphify update. Report before the next bounded step.

### L-1f11al: shared candidate regression qualification

Goal: compare one bounded release candidate with the production scanner for
both enabled tools before any production pin change.

Acceptance:

- Use private synthetic Claude Code and Codex fixtures and independent counts.
- Compare each supported report, copied roots, missing values, and failures.
  Preserve exact values and unchanged source hashes.
- Check both executable digests and record commands, source pins, and scope.
- Record mismatches and remaining packaging and performance gates. Keep the
  enabled catalog and production pin unchanged.
- Run doctor and the selected verification plan. Keep native and platform
  claims within their executed source scope.

Stopping condition: one comparison result, updated owning docs, one local
commit, and an AST Graphify update. Report before the next bounded step.

### L-1f11am: Codex captured-event identity

Goal: establish the pinned Codex producer's identity contract for copied and
distinct equal-usage records before a scanner correction.

Acceptance:

- Read the pinned producer and consumer boundaries. Record source fingerprints.
  Source types alone do not prove writer execution.
- Use private synthetic producer output. Compare renamed copies, distinct
  sessions, replay, conflicts, and declared lineage with independent counts.
- Never use time, equal counts, email, or folder names as identity proof.
  Keep uncertain observations unavailable.
- If the contract permits a correction, keep it in a separate candidate and
  check both reports. Preserve the Pi candidate and production pins.
- Run doctor and the selected verification plan. Run affected native and
  workspace checks for application changes. Record unexecuted gates.

Stopping condition: one verified producer-boundary result or isolated candidate
correction, updated owning docs, one local commit, and an AST Graphify update.
Report before the next bounded step.

### L-1f11an: Codex record-identity candidate

Goal: correct accounting for the tested Codex paginated format in a separate
candidate, while preserving the production scanner and Pi candidate pins.

Acceptance:

- Use the pinned producer contract and explicit record IDs and lineage.
  Do not use paths, time, or equal counts as identity proof.
- Count each response once across its usage record and counter. Preserve copies,
  distinct equal-usage responses, measured zero, and unavailable fields.
- Preserve 12,650 tokens for the full captured fork. Reject conflicting identity
  without a successful partial result. Declare the missing-prefix boundary
  under SC-025 before choosing whether own records remain available.
- Keep unsupported formats outside the qualified boundary. Preserve existing
  history and production gates. Add no scanner fallback that guesses identity.
- Build from pinned source with the shared Cargo target and a bounded deadline.
  Check source, executable, pricing, and compiler fingerprints.
- Run focused candidate tests, producer comparisons, and the selected project
  checks. Run affected workspace and native checks for application changes.

Stopping condition: one isolated candidate correction with executed evidence,
updated owning docs, one local commit, and an AST Graphify update. Report before
the next bounded step.

### L-1f11ao: Codex combined capture

Goal: derive both required Codex reports from one captured input in the separate
candidate, before application integration.

Acceptance:

- Add one bounded JSON command for the tested paginated format. Reuse the strict
  loader and report logic. Do not execute two independent scans inside it.
- Preserve full captured fork totals, copies, distinct responses, measured zero,
  unavailable attribution, and strict refusal without successful partial output.
- Check both reports against independent expectations and their separate
  report controls. Test a controlled source change around capture.
- Preserve the prior candidate oracle, production pin, enabled catalog, and
  legacy history. Keep unsupported formats explicit.
- Build with the shared target and pinned inputs. Run focused source and
  candidate regressions, doctor, and the selected project checks.

Stopping condition: one isolated combined-capture candidate with executed
evidence, updated owning docs, one local commit, and an AST Graphify update.
Report before the next bounded step.

### L-1f11ap: Codex combined report consumer

Goal: parse the candidate envelope into existing typed grouped records before
any scanner activation or application dispatch change.

Acceptance:

- Parse the pinned envelope and Codex-specific sections strictly. Preserve exact
  totals, reported categories, reasoning, session identity, and available times.
- Keep required null values distinct from missing fields. Preserve measured
  zero and unavailable model attribution or estimates. Never add session totals
  to daily totals or invent a start time or project.
- Refuse unknown, missing, duplicate, invalid, excessive, or inconsistent fields.
  Check section totals and model attribution with independent synthetic fixtures.
- Return existing grouped types. Keep the production scanner pin, enabled
  catalog, scan dispatch, and legacy history unchanged.
- Run focused parser tests, doctor, and affected project checks. Record native
  and application integration as later gates.

Stopping condition: one Rust report contract with executed evidence, owning
docs, one local commit, and an AST Graphify update. Report before the next step.

### L-1f11aq: native Codex candidate qualification

Goal: exercise the pinned combined-capture candidate through the desktop app
with private synthetic homes before any production pin change.

Acceptance:

- Add an isolated Linux qualification feature and pinned candidate descriptor.
  Keep ordinary Codex consent and the production scanner behavior unchanged.
  Missing or mismatched candidates must fail without a scanner fallback.
- Use consent, worker scans, one grouped capture, the typed consumer, and one
  complete save. Preserve legacy records, keys, original zones, and separate
  historical views under SC-024. Save the actual candidate version as provenance.
- Check copies, distinct responses, the full fork, reported zero, unavailable
  fields, growth, genuine reopen, refused input, withdrawal, and wrong digests
  through a native journey. Inspect both saved and displayed values.
- Run affected backup, export, restore, and persistence checks. The ordinary
  build must keep its production descriptor and existing Codex support.
- Run the full workspace and renderer milestone and affected native journeys.
  Record source, build, executable, and platform scope. Keep release activation
  and reference-scale performance as separate gates.

Stopping condition: one native qualification result with evidence, owning docs,
one local commit, and an AST Graphify update. Report before the next step.

### R-6 and R-7: CLI state accuracy

Goal: report absence and freshness without claims that saved evidence cannot support.

Acceptance:

- Reproduce both findings with public command tests and private synthetic homes.
  Distinguish an empty selected range from the absence of source logs.
- Treat an observation after the current clock time as not Current. Check the
  exact freshness boundary and preserve saved quota values and observation times.
- Keep JSON shapes, read-only access, consent, exact arithmetic, and unavailable
  metrics intact. Use the existing states unless the SRS requires a clarification.
- Run doctor, affected crate and project checks, and the native CLI journey.
  Record scope and preserve all unrelated work.

Stopping condition: executed evidence, owning docs, one commit, an AST Graphify
update, and the authorized push. Report before the next assignment.

### R-8: CLI connection-state display

Goal: check that CLI quota output keeps known connection failures visible.

Acceptance:

- Read OU-STATE-008 and OU-STATE-009 and the existing desktop state precedence.
  Compare public JSON, text, and statusline with private saved quota fixtures.
- Reproduce a concrete failure before a correction. Preserve accepted values,
  observation times, source, missing metrics, and read-only storage.
- Keep connection and freshness evidence distinct. Record any normative conflict
  in clarifications without changing the SRS.
- Run affected project checks and the native CLI journey for changed behavior.
  Keep the production scanner pin and catalog unchanged.

Stopping condition: one bounded review or correction with executed evidence,
owning docs, one commit, an AST Graphify update, and the authorized push.
Report before the next assignment.

### L-7e1: data-contract documentation

Status: Done for the implemented contract snapshot, November 12, 2026.
The [data contract](data-contracts.md) describes current records and interfaces.
Source checks cover 32 local references, 37 files, 42 tables, version markers,
and numeric examples. This work changes no implementation or schema.

Goal: document stored values and exported representations from current source
under SRS Sections 12, 17, and 20, including OU-NUM-001 through OU-NUM-009.

Acceptance:

- Map profile, association, reading, local-usage, subscription, and delivery ownership.
- Preserve exact units, missing values, dates, provenance, and retention boundaries.
- Separate database, archive, export, and CLI versions. Link canonical definitions.
- Record planned evidence and DTO gaps. Leave the SRS unchanged.
- Check references, the current catalog, examples, and prose. Update owning docs.

Stopping condition: one reviewed document, current CI attribution, selected checks,
one local commit, and the authorized push. Report before the next assignment.
L-7e stays open for its other documents and acceptance evidence.

## Order

| Slice | Goal | Before it starts | State |
|---|---|---|---|
| S-01 | Simple screens | | Done, with one exception in [status](implementation-status.md#partial) |
| S-02 | Native journeys | S-01 | Done, with two recorded reasons in [status](implementation-status.md#failing-or-stale-tests) |
| S-03 | CI | S-02 | Hosted Linux and Renderer passed for `cd99e35`. Other platforms follow in S-15 |
| S-03b | Rich dashboard | S-03 | Done |
| S-03d | Modern Overview | S-03b | Done, with two recorded reasons in [status](implementation-status.md#failing-or-stale-tests) |
| S-03c | Keychain by default | S-03b | New workspaces use the key store (L-0). The move of a password workspace follows the launch |
| S-04 | Reading status and evidence | S-03b | Not started |
| S-05 | Adapter layer | S-04 | Not started |
| G-1 | Owner: real-account test on Codex | S-05 | Not started |
| S-06 | Tray and background | S-05 | Not started |
| S-07 | Graphs and dashboard | S-01 | Not started |
| S-08 | Remaining gaps | S-02 | Profile removal passed in scoped S-08a1/S-08a2. Other gaps remain open |
| S-09 | `ccusage` runner | S-05, a download | Done as L-1a |
| S-10 | Local usage store | S-09 | Done as L-1b |
| S-11 | First launch and consent | S-10 | Partly done as L-1d and L-1e. AT-72's file trace and the OU-NFR-020 measurement did not run |
| S-12 | This device and duplicates | S-11 | This device done as L-1e and L-1g1 to L-1g3. Quota movement waits for OQ-1. Duplicates not started |
| S-13 | Claude Code quotas | S-05 | Not started |
| G-2 | Owner: real-account test on Claude Code and local logs | S-12, S-13 | Not started |
| S-14 | CLI | S-10 | Done as L-2 |
| S-15 | Windows and macOS | S-14 | Preview builds and platform pins remain open under L-6c. Required build downloads are approved |
| S-16 | WSL profiles | S-15 | Not started |
| S-17 | Other providers | S-05, OQ-3 | Not started |
| S-18 | npm package | S-15 | Linux packages built as L-6a and L-6b. Publishing waits for the owner |
| S-19 | Sharing | S-12 | Built as L-3. The check in a GitHub README did not run |
| S-20 | Release documents | S-18 | L-7a through L-7d built. See L-7e for the remaining documents and evidence |
| G-3 | Owner: real machines and first release | S-20, OQ-2 | Not started |
| S-21 | Leaderboard service | G-3 | Built as L-4. Deployment waits for the owner |
| S-22 | Leaderboard client | S-21 | Waits for the owner (L-5) |

## Foundation

### S-01 Simple screens

Every screen meets Section 2.3. Later slices reuse its one mark and one detail panel.

- **Requirements:** OU-DES-003 to OU-DES-008.
- **Tests:** AT-85.
- **Build:** Inventory the visible text in `apps/desktop/src/`. Remove taglines, helper text, and secondary labels, and keep what OU-DES-008 protects. Add one mark component and one detail panel, and open the panel from every number. Show only facts the app already has.
- **Subagents:** One Luna agent per renderer spec updates its text assertions. The specs hold 432 of them. Change only text removed on purpose, and never weaken a check on a missing, stale, or error state.
- **Stop:** AT-85 is recorded for every screen, the renderer suite passes in both engines, and contract checks pass. Native journeys wait for S-02.

These layout decisions come from screenshots of the app on November 3, 2026. OU-UI-001 to OU-UI-006 still hold.

- **One panel:** A side panel on the native `<dialog>` replaces the per-card disclosures. Every number uses it, and Account Detail (OU-UI-003) is its account view. Rows are compact name and value pairs. Escape closes it and focus returns. It fills a narrow window.
- **Header:** Logo, three pages, and a monitoring mark only when monitoring is not running. Theme and display scale move to Settings. The clock goes.
- **Overview:** Summary, accounts, and token activity at full width. Show one number once. The selected-account panel moves to the side panel. Activity scope stays here, because OU-UI-002 needs the combined heatmap on the dashboard and SC-010 needs Profile sum to stay an explicit choice. The share donut appears beside the heatmap in Profile sum and Verified combined, so it follows the same selection and dates. The quota donut moves to Usage.
- **Accounts table:** One column per window, as Compare does, so cells do not repeat window names. A connected account shows no status. Any other state shows one mark and its one action. Plan, last update, Refresh, and View usage move to the panel or the row menu. Selecting a row opens the panel.
- **Usage:** One scope control. One empty state. No heading repeats the selected account. Compare loses its footnote.
- **Settings:** No heading repeats its only row. Every row looks like what it does.
- **Manage accounts:** Match the accounts table, and remove the loaded message and `Remaining (derived)`. It keeps its own list, because that list owns sign-in, focus, and provider actions.
- **Everywhere:** A missing value says Not available, never a dash. One empty state per view, with one action. One primary color in both themes. Normal word spacing in values such as `60% left`.
- **Narrow windows:** At 720 pixels, no action or column is clipped, the header keeps one row, and panels use the full width.

### S-02 Native journeys

Native acceptance is mandatory, and S-01 changes the screens these journeys drive.

- **Tests:** Every journey in `tests/native/`, on Linux.
- **Build:** Fix `profiles` and `restore-checkpoint`. Update the stale journeys that [status](implementation-status.md#failing-or-stale-tests) lists. Run the journeys not yet run on this build.
- **Notes:** `.tools/build-target` is absent, so the first build is a full one. The `verifierDigest` pin is host-specific: swap it for a run and restore it. `heatmap-window` needs a window manager.
- **Subagents:** Luna updates one journey at a time. Astra diagnoses failures. Run journeys one after another.
- **Stop:** Every journey passes or has a recorded reason, and each failure that remains names an app defect.

### S-03 CI

Later slices need a gate that nobody can forget.

- **Requirements:** OU-ENG-002 to OU-ENG-004. OU-NFR-012 for Linux; S-15 adds Windows and macOS.
- **Build:** Add workflows under `.github/workflows/` that run the same commands as `scripts/engineering.py`.
- **Stop:** The commands pass locally. The hosted run is recorded as not run, because only the owner pushes.

### S-03b Rich dashboard

On November 3, 2026 the owner tested the S-01 build and said its UI was much worse than the approved reference dashboard. This slice restores that hierarchy without breaking Section 2.3. The mockups are in the owner's private canvas, artboards A and C.

- **Requirements:** OU-DES-003 to OU-DES-008, OU-UI-001 to OU-UI-006. See SC-013.
- **Build:** A left sidebar holds the brand, the three pages, a local-storage note and the version. Overview's top row holds the selected account's card, with its quota windows and stat tiles in place, then account share, then token activity with its peak, lowest and active days. Below it, the accounts table shows Status, Plan, Quota left, Last updated and one action per row. Choosing a row selects that account's card. Each number still opens the detail panel. The summary cards go, because the account card and the table show the same facts.
- **Stop:** The renderer suite passes in both engines, the affected native journeys pass, and screens are checked by eye in light, dark, 720 and 360 pixels with populated and empty data.

### S-03c Keychain by default

The owner asked to replace the app password with OAuth. OAuth cannot protect the local encryption key, so this slice removes the password the right way. See SC-014.

- **Requirements:** OU-CRYPT, OU-SEC as they apply to key protection.
- **Build:** New workspaces use the system keychain without a prompt, as today. Settings gains one action that moves a password-protected workspace's key into the keychain. The app password stays only as the fallback when no keychain exists.
- **Stop:** Rust tests cover the move, failure keeps the old protection, and a native journey moves a synthetic workspace.

### S-04 Reading status and evidence

Every number can say where it came from.

- **Requirements:** OU-STATE-010, OU-STATE-011, OU-AUD-001 to OU-AUD-003, OU-AUD-009, OU-AUD-010, OU-ALERT-019, OU-RESET-005, OU-RESET-006, OU-DATA-027 for evidence.
- **Tests:** AT-56 to AT-59.
- **Build:** Add a new schema with `reading_evidence`, and add status and source to the typed readings. Codex readings are `official`. Show status and evidence in the S-01 detail panel, and carry them through exports and backups. Give readings from before that schema the `official` status and no evidence record, and record that reading of OU-AUD-001 as a clarification.
- **Subagents:** Luna writes fixtures for each status. Astra owns the migration and the evidence writer.
- **Stop:** The tests pass, and an older database and an older backup both open with their numbers unchanged.

### S-05 Adapter layer

One data path holds every source. Codex behavior does not change.

- **Requirements:** OU-SRC-001 to OU-SRC-004, OU-SRC-006, OU-SRC-009, OU-SRC-010, OU-AUD-008, OU-DES-009, OU-DES-011, OU-ENG-011.
- **Tests:** AT-61 with a synthetic Unofficial adapter, AT-62.
- **Build:** Add `crates/openusage-sources/` with the adapter declaration, and put the existing Codex path behind it. Create `docs/provider-evidence.md` with an empty summary format.
- **Subagents:** Luna writes the synthetic adapter and its fixtures.
- **Stop:** All existing Rust tests pass unchanged, and adding the synthetic adapter touched no UI or storage code.

### G-1 Owner: real-account test on Codex

This runs after S-05, because OU-AUD-008 voids a test when a parser changes.

Run AT-60 for Codex under OU-AUD-006. Raw captures stay private, and a summary goes in `docs/provider-evidence.md` under OU-AUD-007.

## Everyday desktop

### S-06 Tray and background

- **Requirements:** OU-UI-008, OU-UI-021, OU-UI-024, OU-MON-013, OU-MON-014, OU-MON-016, OU-MON-017, OU-PROC-013, OU-ALERT-005, OU-ALERT-014, OU-ALERT-015.
- **Tests:** AT-89, AT-90.
- **Notes:** `tray-icon`, `muda`, and `libappindicator` are in `Cargo.lock` and the local cache. `tauri-plugin-autostart` is not, so write Start at login as a small Rust module.
- **Stop:** The tests pass on Linux, with Windows and macOS recorded as not run.

### S-07 Graphs and dashboard

- **Requirements:** OU-UI-026, OU-UI-027.
- **Tests:** AT-83, AT-92.
- **Build:** Rust prepares every value. Save the layout in `settings`. Gaps stay gaps.
- **Stop:** The tests pass, and marks and errors stay visible on every shown card.

### S-08 Remaining gaps

- **Requirements:** OU-ACCT-013, OU-ALERT-007, OU-ALERT-016, OU-MON-012, OU-STATE-009, OU-DATA-024, OU-UI-010.
- **Tests:** AT-44, plus tests for each requirement.
- **Stop:** Each item is done, or the report names what blocks it. OU-ALERT-007 stays under SC-009.

### S-08a1: profile removal storage contract

Status: Done within storage scope, November 12, 2026. Public storage tests,
parent guard mutations, full workspace tests, and Clippy passed. The
[record](reviews/2026-11-12-profile-history-storage.md) owns evidence and limits.
App integration remains a separate assignment.

Goal: define and test the storage boundary for OU-ACCT-013 before app integration.

Acceptance:

- Read the requirement and existing profile, history, binding, alert, backup,
  restore, and process-ownership contracts. Map records to their actual owner.
- Define separate archive and delete-history behavior. Keep other profiles,
  independent local usage, shared configuration, protected homes, and credentials
  outside the selected profile's history operation.
- Use a narrow typed storage API with explicit profile and home fences.
  Reproduce its behavior through public storage tests before implementation.
- Test atomic refusal, complete scoped deletion, archive preservation, genuine
  reopen, and affected backup and restore behavior with private synthetic data.
- Record any normative conflict in clarifications. Leave the SRS unchanged.
  Run doctor and the selected project checks. Mark app and native integration
  explicitly unverified until their later slice.

Stopping condition: one reviewed storage contract with executed evidence,
owning docs, one commit, an AST Graphify update, and the authorized push.
Report before app integration.

### S-08a2: profile removal in the app

Status: Done within private synthetic Linux app scope, November 12, 2026.
Full workspace and renderer checks, 18 selected native variants, and four
independent app guard mutations passed. The [record](reviews/2026-11-12-profile-removal-app.md)
owns evidence and limits. Real providers and other platforms remain unverified.

Goal: offer Archive History and Delete History for the selected profile under
OU-ACCT-013 without access to another profile or a protected home.

Acceptance:

- Use narrow typed Rust commands and a one-use confirmation bound to the exact
  profile, home, history choice, and opaque storage revision. Keep home paths
  and revisions in Rust. A stale or consumed request requires a new preview.
- Stop selected-profile monitoring and provider operations before applying the
  choice. Keep notification claim, submission, completion, and removal ordered
  under the workspace lock. Clear selected-profile caches and stale previews.
- Use the common dialog controls for the two explicit choices and destructive
  consent. Preserve keyboard focus, narrow layouts, and both themes. Add no
  tagline or general helper text. Explain retained data only in the confirmation.
- Prove other profiles, independent usage, homes, credentials, local subscription
  scope, and delivery journals keep the storage contract. Use private synthetic
  native fixtures. Test stale and repeated confirmation and late worker results.
- Run doctor, affected security checks, full workspace and renderer checks,
  and the changed native journeys. Keep other platforms and real-provider
  acceptance open unless executed.

Stopping condition: one reviewed app flow with executed native evidence,
owning docs, one commit, an AST Graphify update, and the authorized push.
Report the result before the next feature.

## Many tools

### S-09 `ccusage` runner

- **Before it starts:** Outside the sandbox, fetch the pinned Linux x86_64 package and record its version and hash, as `scripts/provision-companion.py` does for Codex. Version 20.0.47 was current on November 2, 2026.
- **Requirements:** OU-LOG-005 to OU-LOG-009, OU-DUP-004. This closes OQ-4 for synthetic logs.
- **Tests:** AT-65, AT-70, AT-71.
- **Build:** Rust runs the checked binary with fixed arguments, no shell, and a minimal environment. Set the home and log folders to fixtures in every test, so it can never read real logs.
- **Subagents:** Luna writes forked, replayed, compacted, and copied log fixtures.
- **Stop:** The tests pass, and the arguments and offline behavior are recorded.

### S-10 Local usage store

- **Requirements:** OU-LOG-003, OU-LOG-004, OU-LOG-010 to OU-LOG-015, OU-DUP-008, OU-DATA-027 for summaries, OU-DATA-028, OU-SEC-021, OU-NFR-021.
- **Tests:** AT-66 to AT-69, AT-73, AT-74, AT-94.
- **Build:** Add the local usage tables from Section 13 as a forward migration. Keep logs under Unknown account until the owner settles OQ-1.
- **Stop:** The tests pass, and saved totals survive deleted logs.

### S-11 First launch and consent

- **Requirements:** OU-LOG-001, OU-LOG-002, OU-UI-025, OU-CAP-008, OU-NFR-020.
- **Tests:** AT-72, AT-91.
- **Stop:** No log opens before consent, and history appears with no sign-in.

### S-12 This device and duplicates

- **Requirements:** OU-LOG-016 to OU-LOG-018, OU-HEAT-026, OU-HEAT-027, OU-AUD-004, OU-AUD-005, OU-DUP-001 to OU-DUP-003, OU-DUP-005 to OU-DUP-007, OU-ID-013.
- **Tests:** AT-63, AT-64, AT-75, AT-76, AT-93.
- **Stop:** Provider and local totals are never added, and no number needs a source label.

### S-13 Claude Code quotas

- **Requirements:** OU-SRC-011 to OU-SRC-015.
- **Tests:** AT-84.
- **Build:** Add one adapter through the S-05 layer. Tests use a temporary Claude Code settings file, never the real one.
- **Stop:** The test passes, and no UI or storage code changed for this source.

### G-2 Owner: real-account test on Claude Code and local logs

Run AT-60 for Claude Code, and for local logs from Codex and Claude Code, as Section 5A.4 requires.

## Release

### S-14 CLI

- **Requirements:** OU-CLI-001 to OU-CLI-008, OU-DES-010, OU-DES-012, OU-NFR-019.
- **Tests:** AT-77 to AT-79.
- **Build:** Add `crates/openusage-cli/`, sharing `openusage-storage`. It has no webview dependency.
- **Stop:** The tests pass with desktop libraries absent.

### S-15 Windows and macOS

- **Requirements:** OU-NFR-012, OU-PLAT-001, OU-PLAT-008.
- **Tests:** AT-38 stays with G-3.
- **Build:** Make the code build on both systems. Pin a Codex companion and a `ccusage` binary for each. Extend CI.
- **Stop:** This host cannot run either system, so record the result as unverified until a hosted build or a real machine runs it.

### S-16 WSL profiles

- **Requirements:** OU-ACCT-011, OU-ENV-006, OU-LOG-019.
- **Tests:** AT-39, AT-55.
- **Stop:** The code is complete, and both tests are recorded for the owner's Windows PC.

### S-17 Other providers

Repeat this slice once per provider, from the OQ-3 list.

- **Requirements:** OU-SRC-005, OU-SRC-007, OU-SRC-008, OU-ACCT-020, OU-ENG-011.
- **Stop:** One adapter, its fixtures, and its declaration are added, with its terms review recorded.

### S-18 npm package

- **Requirements:** OU-DIST-001 to OU-DIST-009, OU-PLAT-002 to OU-PLAT-007, OU-PLAT-009, OU-PLAT-010.
- **Tests:** AT-81. AT-80 stays with G-3.
- **Stop:** `npm pack` output passes AT-81. Nothing is published.

### S-19 Sharing

- **Requirements:** OU-SHARE-001 to OU-SHARE-006.
- **Tests:** AT-82.
- **Stop:** Images pass the test, and the SVG renders in a GitHub README preview.

### S-20 Release documents

Write these last, so they describe what exists.

- **Build:** Data contracts, identity contract, provider capabilities, threat model, security and privacy, compatibility matrix, and performance report. OU-SEC-022 stays stated in [SECURITY.md](../SECURITY.md).
- **Subagents:** Luna drafts from the code. Astra reviews each draft against the code and writes the threat model.
- **Stop:** All seven exist. The identity contract stays a draft until the owner approves it.

### G-3 Owner: real machines and first release

Run AT-38 and AT-80 on a real Windows PC and Mac, including OU-DIST-007. Claim the npm name (OQ-2), then publish. Agents never publish.

## Stage 2

### S-21 Leaderboard service

- **Requirements:** OU-BOARD-003, OU-BOARD-004, OU-BOARD-006 to OU-BOARD-011.
- **Build:** Add `services/leaderboard/`.
- **Stop:** It passes with synthetic data locally. The owner chooses hosting and deploys it.

### S-22 Leaderboard client

- **Requirements:** OU-BOARD-001, OU-BOARD-002, OU-BOARD-005.
- **Tests:** AT-86 to AT-88.
- **Stop:** The tests pass against a local service.

## Outside these slices

- Verified identity and verified totals need an approved identity contract: OU-ACCT-008, OU-ID-*.
- Browser sign-in waits on SC-005.
- Credential renewal and revocation need live accounts.
- OU-BOARD-012 is Stage 3.
