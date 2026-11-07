# Implementation slices

Updated: November 8, 2026. This page owns the order and boundaries of planned work. The [SRS](SRS.md) owns what each requirement says, and [status](implementation-status.md) owns what works today.

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
- Download nothing. Use what the lockfiles and local caches hold. If a slice needs more, stop and report.
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
| L-1 | Local logs with no sign-in: the pinned official `ccusage` binary, a Rust runner, the local usage store, consent, and This device views | S-09, S-10, S-11, part of S-05 | L-1a and L-1b done. See the [checklist](#build-checklist) |
| L-2 | CLI with `openusage status`, `statusline` and `report` | S-14 | Not started |
| L-3 | Sharing: Wrapped summary, heatmap PNG and SVG, a README badge, and personal records | S-19 | Not started |
| L-4 | Leaderboard service on Cloudflare Workers and D1, per the [decision](decisions/2026-11-07-leaderboard-service.md) | S-21 | Not started |
| L-5 | Leaderboard client: join with GitHub, dry run, sync, leave | S-22 | Not started |
| L-6 | Packaging: Linux packages, Windows and macOS preview builds in CI, the npm package | S-18, part of S-15 | Not started |
| L-7 | Release documents and the launch checklist | S-20 | Not started |
| G-L | Owner: claim the npm name (OQ-2), create the Cloudflare account and GitHub OAuth App, run the real-account tests, publish | G-1, G-2, G-3 | Not started |

The other slices below follow the launch, unless a launch slice needs part of one.

## Build checklist

Updated November 8, 2026. The owner asked to keep this list and to build down it with no stop between items. Each item ends with one local commit, which ticks the item. Owner items wait for the owner.

**L-0 First run**

- [x] New workspaces use the OS key store with no app password.
- [ ] Move an existing password workspace to the key store. This follows the launch.

**L-1 Local logs**

- [x] L-1a: pin, provision, run and parse `ccusage` (OU-LOG-005 to OU-LOG-007).
- [x] L-1b: store totals and differences at schema 10 (OU-LOG-003, OU-LOG-010 to OU-LOG-013).
- [ ] L-1c: scan service. Detect known folders by existence only, keep each run to the consented tools, scan after consent and then on a schedule, one run at a time, and keep a failure inside local logs (OU-LOG-001, OU-LOG-006, OU-LOG-008, OU-DUP-008, OU-NFR-021; AT-70 to AT-72).
- [ ] L-1d: the first-launch consent screen and the Settings selection (OU-LOG-001, OU-LOG-002, OU-UI-025; AT-91).
- [ ] L-1e: the This device scope in Usage, with daily, weekly, monthly and custom ranges, a breakdown by token type and model, and the difference in the detail panel (OU-LOG-011, OU-LOG-016, OU-LOG-017; AT-93).

**L-2 CLI**

- [ ] L-2a: the `openusage` binary with no webview: `openusage`, `status --json` and `statusline`, with versioned JSON, stable exit codes, and Locked without a prompt (OU-CLI-001, OU-CLI-002, OU-CLI-004 to OU-CLI-008; AT-78, AT-79).
- [ ] L-2b: `openusage report` with one consent prompt, `--yes` and nothing saved, plus `openusage app` and `openusage doctor` (OU-CLI-003, OU-DIST-004; AT-77).

**L-3 Sharing**

- [ ] L-3a: the Wrapped summary and the heatmap as SVG and PNG, made on the device, with a preview and an explicit save (OU-SHARE-001 to OU-SHARE-006; AT-82).
- [ ] L-3b: the README badge and personal records.

**L-4 Leaderboard service**

- [ ] L-4a: the Worker in `services/leaderboard/` with join, sync, leave and public ranks, strict schemas and limits, tested against SQLite with no network (OU-BOARD-003, OU-BOARD-004, OU-BOARD-007 to OU-BOARD-011).
- [ ] L-4b: the moderation policy beside the source.

**L-5 Leaderboard client**

- [ ] L-5a: `openusage board join`, `sync --dry-run`, `sync` and `leave` through one upload builder, with the token in the OS credential store (OU-BOARD-001, OU-BOARD-002, OU-BOARD-005, OU-BOARD-006; AT-86 to AT-88).
- [ ] L-5b: join, sync and leave in Settings.

**L-6 Packaging**

- [ ] L-6a: Linux packages with the bundled `ccusage` and Codex companion, and the WebKitGTK dependencies listed (OU-PLAT-002, OU-PLAT-005).
- [ ] L-6b: the npm package, with per-platform packages and no install scripts (OU-DIST-001 to OU-DIST-005, OU-DIST-009; AT-81).
- [ ] L-6c: Windows and macOS preview builds in CI (part of OU-NFR-012).

**L-7 Release documents**

- [ ] Install, upgrade, uninstall and data deletion steps, unsigned-build behavior, release notes, the SBOM, the threat model and the OU-SEC-022 statement (OU-PLAT-007, OU-DIST-006, OU-DIST-008, OU-SEC-016, OU-SEC-022).

**Owner (G-L)**

- [ ] Claim the npm name (OQ-2).
- [ ] Create the Cloudflare account and the D1 database, then deploy the service.
- [ ] Register the GitHub OAuth App and give its public client ID.
- [ ] Run the real-account tests (AT-60) and the real-machine tests (AT-38, AT-80).
- [ ] Decide the two gaps in [SC-017](srs-clarifications.md#sc-017-local-usage-store).
- [ ] Push and publish.

**After the launch track**

- [ ] The slices in [Order](#order). Stage 1 needs every mandatory requirement (SRS Section 0.4).

## Order

| Slice | Goal | Before it starts | State |
|---|---|---|---|
| S-01 | Simple screens | | Done, with one exception in [status](implementation-status.md#partial) |
| S-02 | Native journeys | S-01 | Done, with two recorded reasons in [status](implementation-status.md#failing-or-stale-tests) |
| S-03 | CI | S-02 | Done; the hosted run waits for the owner's push |
| S-03b | Rich dashboard | S-03 | Done |
| S-03d | Modern Overview | S-03b | Done, with two recorded reasons in [status](implementation-status.md#failing-or-stale-tests) |
| S-03c | Keychain by default | S-03b | Not started |
| S-04 | Reading status and evidence | S-03b | Not started |
| S-05 | Adapter layer | S-04 | Not started |
| G-1 | Owner: real-account test on Codex | S-05 | Not started |
| S-06 | Tray and background | S-05 | Not started |
| S-07 | Graphs and dashboard | S-01 | Not started |
| S-08 | Remaining gaps | S-02 | Not started |
| S-09 | `ccusage` runner | S-05, a download | Done as L-1a |
| S-10 | Local usage store | S-09 | Done as L-1b |
| S-11 | First launch and consent | S-10 | Not started |
| S-12 | This device and duplicates | S-11 | Not started |
| S-13 | Claude Code quotas | S-05 | Not started |
| G-2 | Owner: real-account test on Claude Code and local logs | S-12, S-13 | Not started |
| S-14 | CLI | S-10 | Not started |
| S-15 | Windows and macOS | S-14 | Not started |
| S-16 | WSL profiles | S-15 | Not started |
| S-17 | Other providers | S-05, OQ-3 | Not started |
| S-18 | npm package | S-15 | Not started |
| S-19 | Sharing | S-12 | Not started |
| S-20 | Release documents | S-18 | Not started |
| G-3 | Owner: real machines and first release | S-20, OQ-2 | Not started |
| S-21 | Leaderboard service | G-3 | Not started |
| S-22 | Leaderboard client | S-21 | Not started |

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
