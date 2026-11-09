> Public source copy: raw acceptance evidence remains private. Historical test statements keep their original scope and do not certify this source snapshot.

# Implementation status

Updated: November 9, 2026. This page holds the current state only. [History](implementation-history.md) holds past work, and [traceability](acceptance-traceability.md) holds exact evidence.

OpenUsage is in development against [SRS baseline 3.0](SRS.md). There is no release. Baseline 3.0 was adopted on November 2, 2026. [Not built](#not-built) lists what is missing, and [Current assignment](#current-assignment) gives the launch state.

## What works

The app runs on Linux only. Results below use synthetic data unless they say otherwise.

| Area | State |
|---|---|
| Storage | SQLCipher 4 with WAL at schema 12, with OS key store and passphrase fallback |
| Accounts | Up to 10 Codex profiles, device-code sign-in, sign-out, and archive |
| Quotas | Reported windows, reset countdowns, conservative reset confirmation, and history |
| Token activity | Provider daily buckets, heatmaps, and Individual and Profile sum views |
| Comparison | Windows align only when limit ID, role, and duration match |
| Alerts | Threshold, reset, failure, and sign-in alerts, with quiet hours and history |
| Subscriptions | User-entered records |
| Data safety | CSV and JSON export, application export, encrypted backup, restore, checkpoints, and local deletion |
| Interface | A sidebar with Overview, Usage, and Settings in light and dark themes. Overview is the November 6 modern dashboard: hero, eight answers, pastel account cards, daily and weekly charts, and the year heatmap (S-03d). |

Verified combined totals stay unavailable, because no approved identity contract exists.

Real accounts: two authorized Codex accounts signed in and saved readings on Linux on an earlier build. The formal real-account test, AT-60, has not run.

Latest scoped result, November 9, 2026: L-1f7 passed seven period tests, full Rust workspace checks, 98 pinned-scanner comparisons, and the rebuilt native consent journey. All data was synthetic on Linux. The renderer build and all 56 IPC contracts passed. The full renderer suite did not run for this parser-only slice. [Traceability](acceptance-traceability.md) holds the exact scope.

The wider native results below are from L-1g2 and earlier. They do not certify the current build. These ran on Linux with synthetic data and the real pinned `ccusage` 20.0.48:

- 426 workspace tests passed, with 1 intentional ignore. The verify plan and Clippy passed.
- 14 native journeys passed at schema 12 on a private display. [Failing or stale tests](#failing-or-stale-tests) names the 12 that did not run.
- The full Chromium and WebKit renderer suite passed after the countdown fixture fix on November 8.
- The last run of all native journeys was on November 4, after S-03b: 46 of 48 passed.

## Not built

New in baseline 3.0:

- Source adapters (5A), audit (5B), and duplicates (5D).
- Customizable dashboard, Start at login, and graphs: OU-UI-026, OU-UI-027, OU-MON-017.
- The other 16 tools that the pinned `ccusage` reads (L-1f, AT-65), and session quota movement (L-1g4, OU-LOG-018).
- The leaderboard client (L-5): OU-BOARD-001, OU-BOARD-002, OU-BOARD-005, OU-BOARD-006.
- Moving a password workspace's key to the OS key store (S-03c).

Kept from baseline 2.0:

- System tray: OU-UI-008, OU-UI-021, OU-UI-024, OU-MON-016.
- One renewal alert: OU-ALERT-005, OU-ALERT-014, OU-ALERT-015.
- WSL profile configuration: OU-ACCT-011, OU-ENV-006, AT-39.
- Per-profile Delete History: OU-ACCT-013.
- Wake handling: OU-PROC-013, OU-MON-013.

## Partial

- Local logs (5C), L-1f7, November 9, 2026: independent period reports preserve reported totals and unavailable details for all sixteen disabled tools. The four dated candidates use strict daily reports. The other twelve use full session reports for one unplaced period. The [crate contract](../crates/openusage-locallogs/README.md#additional-period-reports) owns the parser rules. Storage and scanner integration remain open.

- Quota fixture readiness, November 9, 2026: the fixture saves its runtime path before it exposes its PID. The controlled regression and original Rust cancellation test passed. The full workspace checks passed, with Clippy completed separately after the verification runner ended with signal 15. [Traceability](acceptance-traceability.md) holds the execution limits. The production code is unchanged.

- Local logs (5C), L-1f6, November 8, 2026: a local ccusage patch fixes equal stable-ID Amp copies and rejects conflicting or incomplete scans. All 28 adapter tests and 210 scanner comparisons passed on Linux with synthetic input. The [candidate record](reviews/2026-11-08-amp-dedup-design.md#local-candidate-after-review) holds exact scope. The production pin is unchanged, Amp stays disabled, and the patch remains unpublished.

- Codex fixture portability: the Rust fixture test and two native journeys now copy tracked synthetic input into private homes. They no longer depend on an ignored `.codex` directory in the checkout. Secret exclusions and hooks stay intact.

- Local logs (5C), L-1f5, November 8, 2026: the common session parser reads recorded reports for all sixteen disabled tools. It keeps independent totals and unavailable metrics under [SC-022](srs-clarifications.md#sc-022-incomplete-metrics-from-the-other-local-tools). It rejects malformed shapes, metadata, counts, and inconsistent totals. Storage and process integration remain open. No additional tool is enabled.

- Local logs (5C), L-1f4, November 8, 2026: The parsers require real calendar days, including leap-year rules. Session and hourly timestamps must be ASCII before byte slicing. Malformed Unicode returns a report error instead of a panic. The full Rust checks and rebuilt native consent journey passed with synthetic logs on Linux.

- Local logs (5C), L-1f3, November 8, 2026: record daily and session outputs for synthetic inputs from all 16 remaining tools. Three tests ran the real pinned binary 98 times. Reports repeated exactly, empty roots returned no rows, and source files stayed unchanged. A copied Amp thread doubled 162 tokens to 324. This is a known AT-65 failure, not completed tool support. See [SC-023](srs-clarifications.md#sc-023-copied-amp-usage-in-the-pinned-scanner).

- Local logs (5C), L-1f2, November 8, 2026: one catalog now defines all 18 tools for Rust, the CLI, and the renderer. Only Claude Code and Codex are enabled. Consent and every report runner reject disabled entries. The owner approved reliable totals with unavailable fields under [SC-022](srs-clarifications.md#sc-022-incomplete-metrics-from-the-other-local-tools). The remaining adapters, storage changes, and fixtures are not complete.

- Local logs (5C), L-1f1, November 8, 2026: the runner rejects commas, trailing whitespace, non-UTF-8 paths, and roots that no longer equal the resolved path saved with consent. The regression first reproduced a read of synthetic decoy logs through a comma in an empty root's name. After the fix, invalid paths never start the scanner. The native journey passed with spaces and Unicode in its home path. A saved root replaced with a link to an unselected folder then failed safely and preserved saved usage. The other 16 tools remain unfinished.

- Local logs (5C), L-1f0, November 8, 2026: every report now uses `--mode calculate` with the existing pinned offline prices. Zero and large stored charges no longer make the strict parser reject the report or replace its API-price estimate. Unknown models keep their tokens and no estimate. The pinned-binary regression and native consent journey passed on Linux with synthetic logs. This change adds no tool and makes no account association.

- Distribution (16A), L-6b, November 8, 2026: `scripts/package-npm.py` assembles `openusage`, which holds only a launcher, and `@openusage/linux-x64`, which holds the CLI, the desktop app, the pinned `ccusage`, the pinned Codex companion and their notices. It checks both pins, packs both with `npm pack`, and writes `SHA256SUMS`. Neither package has an install script. Nothing is published: the npm name, provenance and a build attestation wait for the owner and for L-6c.

- Release documents, L-7d, November 8, 2026: the draft [release notes](release-notes/0.1.0.md) state the signing of each package, the Linux dependencies, and the exact tested versions: CachyOS on Linux 7.3.1 x86_64, WebKitGTK 2.54.2, GTK 3.24.52, Codex 0.163.0 and `ccusage` 20.0.48. They say that Ubuntu 24.04, Windows and macOS are not tested yet.

- Release documents, L-7c, November 8, 2026: `scripts/sbom.py` writes a CycloneDX 1.5 SBOM of the Linux release from `cargo tree`, `Cargo.lock`, `bun.lock` and the two pin records, with no download. Crates that only run at build time get the scope `excluded`. Two runs gave the same bytes. `package-npm.py --sbom` publishes it beside the packages in `SHA256SUMS`. No schema validator ran, because none is on this host.

- Release documents, L-7a, November 8, 2026: [Install](install.md) gives the steps to install, upgrade, uninstall and delete data. It also tells what an unsigned build does. [Security](../SECURITY.md#bundled-ccusage) states that `ccusage` runs with the user's rights and is not sandboxed. A private-home run on Linux confirmed the Linux folders. The macOS and Windows steps come from the code and were not run on a real machine.

- Distribution (16A), L-6a, November 8, 2026: the production Tauri config now bundles `ccusage` beside the Codex companion, with both notices and package metadata. The Debian package puts the app, the companion and `ccusage` in `/usr/bin` and depends on `libwebkit2gtk-4.1-0` and `libgtk-3-0`. It was built and inspected on Linux but not installed, because this host has no `dpkg`. No RPM or AppImage is built.

- Leaderboard (14A), L-4a, November 8, 2026: the Cloudflare Worker and D1 schema in `services/leaderboard` join with GitHub, sync strict rows, leave, and rank over 7 days, 30 days and all time under the title Self-reported. Its [moderation policy](../services/leaderboard/MODERATION.md) sits beside the source (L-4b). The service is not deployed, `wrangler.toml` has not run with wrangler, and no client calls it yet.

- Sharing (8A), L-3a, November 8, 2026: Share on This device previews a Wrapped card and a year heatmap from saved local totals, and saves each as PNG or SVG in Downloads/OpenUsage only on an explicit save. Rust builds the SVG with escaped text and checks the PNG. The images show no email, account label, project, path or device name, a day with no saved usage is an outline, the estimate says Estimated, and the OpenUsage name and address can be removed.

- Sharing (8A), L-3b, November 8, 2026: Share also makes a README badge with the 365-day total, as SVG only. Usage, This device, shows three personal records for the chosen dates: active days, the longest streak and the busiest day, each with its detail panel. The badge has not run in a real GitHub README (OU-SHARE-006).

- Command line (11A), L-2a, November 8, 2026: the `openusage` binary in `crates/openusage-cli` has `report`, `doctor` and `app`. Each command has help, stable error codes and documented exit codes. `report` asks once for consent or takes `--yes`, runs the pinned `ccusage` once for each detected folder, prints exact totals as text or versioned JSON, and saves nothing. No package installs the CLI yet.

- Command line (11A), L-2b, November 8, 2026: `openusage`, `status --json` and `statusline` read the saved quotas and today's local usage from a read-only database open, with the key from the OS credential store. They never prompt: a password workspace or a locked key store gives `locked` with exit code 7. The key store code moved from the desktop app into `crates/openusage-keys`, which both use.

- Local logs (5C), L-1a, November 7, 2026: the official `ccusage` 20.0.48 Linux x86_64 binary is pinned and provisioned, and `crates/openusage-locallogs` runs it with checked digest, fixed offline arguments, an empty environment and limits, then parses its report strictly. On Linux the real binary read synthetic Claude Code and Codex logs twice with identical exact totals. This executes the arguments and the offline mode of OQ-4. L-1c ran it once for each folder.

- Local logs (5C), L-1b, November 8, 2026: schema 10 stores consented log sources, daily totals and differences (SRS Section 13). A scan can raise a saved total but never lowers it, and a lower result is saved as a difference. The first saved scan pins the zone of each source. Backup, checkpoint, restore and application export carry the new tables. [SC-017](srs-clarifications.md#sc-017-local-usage-store) records how the store reads these rules.

- Local logs (5C), L-1c, November 8, 2026: the desktop app detects the Claude Code and Codex folders by existence only, saves the consent per tool, and scans each granted folder after start, after consent and every 10 minutes. Each run gets an empty private `HOME` and only the folder variable of its tool, so `ccusage` cannot see a tool that the user did not choose. Runs happen outside the workspace lock, and a failed run keeps the saved totals. `load_local_logs` and `save_local_log_consent` are the IPC commands. The other 16 tools that `ccusage` reads are not scanned yet.

- Local logs (5C), L-1d, November 8, 2026: the welcome screen lists the detected tools, all selected, and its one action creates the workspace and saves the choice. A workspace with no saved choice shows one Local logs dialog after unlock. Settings, Privacy and data, changes the choice and shows a failed scan. Privacy details state that the bundled `ccusage` is not sandboxed.

- Local logs (5C), L-1e, November 8, 2026: Usage has one source control, Provider or This device (OU-LOG-016). This device shows the saved totals for 7, 30 or 90 days or a year, grouped by day, week or month: a card for each tool, stacked bars and a model table. Each total opens the detail panel with the token types, the estimate at public API prices, the source, the day zone, and any saved difference (OU-LOG-011). Overview shows the last 30 days in a This device section, also with no account, so first launch reaches local history with no sign-in (OU-UI-025). Rust sums the exact counts. The L-1g1 entry covers the rest of OU-LOG-017.

- Local logs (5C), L-1g1, November 8, 2026: each scan also runs the `ccusage` session report of the granted tool, and schema 11 saves each session in `local_usage_sessions` (SC-021). A Claude Code session keeps its project folder and its start and end times. A Codex session keeps its end time and its reasoning count, a part of output. A lower scan keeps the saved counts. This device shows a Projects table and a Sessions table, and a count that a tool does not report shows as Not available. Share images read no session. Since L-1g3, a Custom chip sets any range of at most 366 days that ends by today, and Rust checks it again. Since L-1g2, each scan of Claude Code also saves its UTC hours from `ccusage blocks` at schema 12, and an Hourly grouping shows them for ranges of up to 7 days, at their local start times. Codex reports no hours, so the chart key shows it as Not available. Session quota movement (L-1g4) waits for OQ-1.

- Simplicity (S-01, Section 2.3): every screen has been reviewed. One exception to OU-DES-007 remains: quota numbers in Manage accounts open their reading through View usage in the row, not through the detail panel. See SC-012 for unknown freshness.
- Confirmed invalid-credential detection: OU-ALERT-007, under SC-009.
- Offline behavior: OU-MON-012, OU-STATE-009, OU-DATA-024.
- Locale formatting: OU-UI-010. Interface numbers use `en-US`.
- Identity conflict and verified duplicates: OU-ACCT-008, OU-ID-*. These wait for an approved identity contract.
- Notification center delivery state: OU-ALERT-016.

## Blocked

- Real credential renewal and revocation, and live identity validation, need authorized live accounts.
- Browser sign-in stays out of the working build until callback isolation is resolved. See SC-005.

## Not yet run

- [Hosted CI for `b1b8d84`](https://github.com/Debrato2005/OpenUsage/actions/runs/42483593712) passed all 456 renderer tests. The Linux job passed lint, then failed the synthetic quota test at its temporary-path assertion. Native oracle checks were skipped. The local fixture now saves its runtime path before it signals readiness, and both focused regressions pass. The complete hosted gate still needs a later run. OU-NFR-012 also needs Windows and macOS, planned for S-15.
- Windows and macOS have never run. The owner has a real Windows PC and Mac for this.
- Windows and macOS need their own pinned Codex companion. Only Linux x86_64 is qualified.
- Sleep and wake, packaging, and release-scale performance are unmeasured.

## Failing or stale tests

- L-1g2, November 8, 2026: 24 journeys now expect schema 12. 14 journeys ran on the native-test build on a private display and passed: `local-log-consent`, `share-images`, `cli-status`, `backup`, `backup-inspection`, `restore-preview`, `restore-apply`, `restore-checkpoint`, `application-export`, `local-data-deletion`, `persistence`, `subscriptions`, `activity-persistence` and `account-signout`. These 12 did not run on this build: `activity-history`, `activity-history-export`, `alert-history`, `collection`, `companion-onboarding`, `device-login`, `first-account-read`, `monitoring`, `onboarding-discovery`, `operational-alerts`, `quota-read` and `reset-notifications`.
- S-03d, November 6, 2026: the slice updated the 19 journeys that touch Overview and ran them on a private display. 17 passed, 16 on the native-test build and `provider-keyring` on the companion build. `reference-visual` passes every Overview check and keeps its recorded digit-width reason on Usage. `heatmap-window` keeps its window-manager reason. The other journeys did not run on this build.

As of November 3, 2026, on Linux. 43 journeys ran on the native-test build and 4 on the production build. No failure names an app defect.

- `heatmap-window`: its calendar checks pass. The title-bar checks need an X11 window manager, and this host has none.
- `reference-visual`: every layout check passes. This host's font hinting rounds each glyph to whole pixels, so a tabular `0` is 1 pixel wider than other digits at 20 pixels. The journey records it as a violation.
- `companion-onboarding` did not run. It requests a real OpenAI device code, so it waits for owner approval.
- `activity-read` was removed. It checked unsaved previews, and every completed read has been saved since October 18. `activity-persistence` and `collection` cover saved reads.

## Missing release documents

SRS Section 20 requires these documents, which do not exist yet: data contracts, identity contract, provider capabilities, security and privacy, compatibility matrix, performance report, and provider evidence. The [threat model](threat-model.md) exists since L-7b, and `scripts/sbom.py` writes the SBOM since L-7c.

## Open items

[SRS Section 0.7](SRS.md) lists four: how to split logs by account, the npm name, the provider and terms review, and the first `ccusage` run.

## Current assignment

The next assignment is [L-1f8](implementation-slices.md#next-assignment-l-1f8): store independent period totals and nullable details. L-1f0 through L-1f7 are complete within their recorded scope. Only Claude Code and Codex are enabled. The Amp patch is a local candidate, and the production scanner pin is unchanged.

The [build checklist](implementation-slices.md#build-checklist) defines L-1f8 through L-1f11 and their order. [SC-022](srs-clarifications.md#sc-022-incomplete-metrics-from-the-other-local-tools) governs missing metrics. The [storage proposal](reviews/2026-11-08-incomplete-local-metrics.md) remains design input. Resolve its migration questions before changing stored history. [SC-023](srs-clarifications.md#sc-023-copied-amp-usage-in-the-pinned-scanner) holds the Amp qualification limits. OQ-1 and the owner release gates remain open.

The exact working checkpoint, if one exists, is in the local `.recovery/active-checkpoint.json`.
