> Public source copy: raw acceptance evidence remains private. Historical test statements keep their original scope and do not certify this source snapshot.

# Implementation status

Updated: November 2, 2026. This page holds the current state only. [History](implementation-history.md) holds past work, and [traceability](acceptance-traceability.md) holds exact evidence.

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
| Interface | Overview, Usage, and Settings in light and dark themes |

Verified combined totals stay unavailable, because no approved identity contract exists.

Real accounts: two authorized Codex accounts signed in and saved readings on Linux on an earlier build. The formal real-account test, AT-60, has not run.

Last full results, all from November 2, 2026, before the docs update:

- 389 Rust tests passed, with one intentional child test ignored.
- All 410 renderer cases passed: 205 each in Chromium and WebKit.
- 55 contract checks passed.
- 11 native journeys passed on a private display.

## Not built

New in baseline 3.0:

- Simplicity rules (Section 2.3). Current screens have not been reviewed against them, so they still carry secondary labels.
- Source adapters (5A), audit (5B), local logs (5C), and duplicates (5D).
- Sharing (8A), the CLI (11A), and distribution (16A).
- Customizable dashboard, Start at login, first launch without sign-in, and graphs: OU-UI-025 to OU-UI-027, OU-MON-017.
- Leaderboard (14A), planned for Stage 2.

Kept from baseline 2.0:

- System tray: OU-UI-008, OU-UI-021, OU-UI-024, OU-MON-016.
- One renewal alert: OU-ALERT-005, OU-ALERT-014, OU-ALERT-015.
- WSL profile configuration: OU-ACCT-011, OU-ENV-006, AT-39.
- Per-profile Delete History: OU-ACCT-013.
- Wake handling: OU-PROC-013, OU-MON-013.

## Partial

- Confirmed invalid-credential detection: OU-ALERT-007, under SC-009.
- Offline behavior: OU-MON-012, OU-STATE-009, OU-DATA-024.
- Locale formatting: OU-UI-010. Interface numbers use `en-US`.
- Identity conflict and verified duplicates: OU-ACCT-008, OU-ID-*. These wait for an approved identity contract.
- Notification center delivery state: OU-ALERT-016.

## Blocked

- Real credential renewal and revocation, and live identity validation, need authorized live accounts.
- Browser sign-in stays out of the working build until callback isolation is resolved. See SC-005.
- Hosted CI runs only the storage policy workflow. OU-NFR-012 and OU-ENG-002 to OU-ENG-004 are not met.

## Not yet run

- Windows and macOS have never run. The owner has a real Windows PC and Mac for this.
- Windows and macOS need their own pinned Codex companion. Only Linux x86_64 is qualified.
- Sleep and wake, packaging, and release-scale performance are unmeasured.

## Failing or stale tests

As of November 2, 2026, on Linux:

- Failing: `profiles` and `restore-checkpoint`. Both fail with element not interactable, the same as before the redesign.
- Stale: `overview-workspace`, `quota-history`, `activity-history`, `application-export`, `smoke`, `restore-preview`, `restore-apply`, and `theme-select`.
- Needs a window manager: `heatmap-window`.
- Not run on this build: monitoring, collection, device login, and production fixture refusal.

## Missing release documents

SRS Section 20 requires these documents, which do not exist yet: data contracts, identity contract, provider capabilities, threat model, security and privacy, compatibility matrix, performance report, and provider evidence. No SBOM exists.

## Open items

[SRS Section 0.6](SRS.md) lists four: how to split logs by account, the npm name, the provider and terms review, and the first `ccusage` run.

## Next

The owner chooses each assignment. This is the suggested order:

1. Run the real-account test on Codex.
2. Review current screens against Section 2.3 and fix them.
3. Build the tray and graphs from existing data.
4. Run `ccusage`, then settle how to split logs by account.
5. Build the adapter layer and local logs.
6. Add Claude Code quotas.
7. Build the CLI.
8. Run on Windows and macOS, then package with npm.
9. Build sharing, then launch.

The exact working checkpoint, if one exists, is in the local `.recovery/active-checkpoint.json`.
