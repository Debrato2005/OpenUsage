> Public source copy: raw acceptance evidence remains private. Historical test statements keep their original scope and do not certify this source snapshot.

# Implementation status

Updated: November 6, 2026. This page holds the current state only. [History](implementation-history.md) holds past work, and [traceability](acceptance-traceability.md) holds exact evidence.

OpenUsage is in development against [SRS baseline 3.0](SRS.md). There is no release. Baseline 3.0 was adopted on November 2, 2026, and most of its new work has not started.

## What works

The app runs on Linux only. Results below use synthetic data unless they say otherwise.

| Area | State |
|---|---|
| Storage | SQLCipher 4 with WAL at schema 9, with OS key store and passphrase fallback |
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

Last full results, from November 4, 2026, after S-03b, on Linux with synthetic data:

- 46 of 48 native journeys passed on a private display. [Failing tests](#failing-or-stale-tests) names the other two. `quota-read` failed once at a reset-confirmation read and passed when rerun alone.
- All 422 renderer cases passed: 211 each in Chromium and WebKit.
- 55 contract checks, 30 tool tests and the dependency policy passed.
- On November 3, 389 Rust tests passed, with one intentional child test ignored, and Rust format, Clippy and the native verifier crate's tests passed. S-03b changed no Rust.

## Not built

New in baseline 3.0:

- Source adapters (5A), audit (5B), local logs (5C), and duplicates (5D).
- Sharing (8A), the CLI (11A), and distribution (16A).
- Customizable dashboard, Start at login, first launch without sign-in, and graphs: OU-UI-025 to OU-UI-027, OU-MON-017.
- Leaderboard (14A), now in the first release (SRS Section 0.6).

Kept from baseline 2.0:

- System tray: OU-UI-008, OU-UI-021, OU-UI-024, OU-MON-016.
- One renewal alert: OU-ALERT-005, OU-ALERT-014, OU-ALERT-015.
- WSL profile configuration: OU-ACCT-011, OU-ENV-006, AT-39.
- Per-profile Delete History: OU-ACCT-013.
- Wake handling: OU-PROC-013, OU-MON-013.

## Partial

- Local logs (5C), L-1a, November 7, 2026: the official `ccusage` 20.0.48 Linux x86_64 binary is pinned and provisioned, and `crates/openusage-locallogs` runs it with checked digest, fixed offline arguments, an empty environment and limits, then parses its report strictly. On Linux the real binary read synthetic Claude Code and Codex logs twice with identical exact totals. Nothing stores or shows these totals yet, and folder selection by consent (OU-LOG-001) is not built. This executes OQ-4's arguments and offline mode; per-folder use is still open.

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

- The Linux CI workflow first ran on GitHub on November 6, 2026 and failed at Test Git safety. The second run passed every code check, then hit the 90-minute job limit during the renderer tests because the runner's package mirror was slow. The split into two parallel jobs waits for the owner's push. OU-NFR-012 also needs Windows and macOS, planned for S-15.
- Windows and macOS have never run. The owner has a real Windows PC and Mac for this.
- Windows and macOS need their own pinned Codex companion. Only Linux x86_64 is qualified.
- Sleep and wake, packaging, and release-scale performance are unmeasured.

## Failing or stale tests

- S-03d, November 6, 2026: the 19 journeys that touch Overview were updated and run on a private display. 17 passed, 16 on the native-test build and `provider-keyring` on the companion build. `reference-visual` passes every Overview check and keeps its recorded digit-width reason on Usage; `heatmap-window` keeps its window-manager reason. The other journeys did not run on this build.

As of November 3, 2026, on Linux. 43 journeys ran on the native-test build and 4 on the production build. No failure names an app defect.

- `heatmap-window`: its calendar checks pass. The title-bar checks need an X11 window manager, and this host has none.
- `reference-visual`: every layout check passes. This host's font hinting rounds each glyph to whole pixels, so a tabular `0` is 1 pixel wider than other digits at 20 pixels. The journey records it as a violation.
- `companion-onboarding` did not run. It requests a real OpenAI device code, so it waits for owner approval.
- `activity-read` was removed. It checked unsaved previews, and every completed read has been saved since October 18. `activity-persistence` and `collection` cover saved reads.

## Missing release documents

SRS Section 20 requires these documents, which do not exist yet: data contracts, identity contract, provider capabilities, threat model, security and privacy, compatibility matrix, performance report, and provider evidence. No SBOM exists.

## Open items

[SRS Section 0.7](SRS.md) lists four: how to split logs by account, the npm name, the provider and terms review, and the first `ccusage` run.

## Current assignment

S-03d, the modern Overview, passed the renderer suite and its native journeys. Local commits wait for the owner's push. On November 7, 2026 the owner adopted a launch track with the leaderboard in the first release; L-0, the keychain by default, is next. [Implementation slices](implementation-slices.md#launch-track) hold the order. [Implementation slices](implementation-slices.md) hold the order.

The exact working checkpoint, if one exists, is in the local `.recovery/active-checkpoint.json`.
