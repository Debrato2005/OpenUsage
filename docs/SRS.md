# OpenUsage — Software Requirements Specification

**Document ID:** OU-SRS-001  
**Baseline:** 3.0  
**Date:** 2 November 2026  
**Status:** Adopted 2 November 2026; OU-DES-003, OU-DES-005 and OU-DES-008 amended by the owner on 6 November 2026 ([SC-013](srs-clarifications.md#sc-013-rich-dashboard-under-section-23)). Normative product and engineering requirements; release is subject to validation.  
**Product:** OpenUsage  
**License:** Apache License 2.0  
**Replaces:** OU-SRS-001, baseline 2.0  
**Supported workload:** 10 active account profiles, plus tools read only from local logs  
**Document style:** Short, direct technical English based on ASD-STE100, Issue 9. This document has **not** received a formal STE vocabulary or conformity audit.

> **Product statement.** OpenUsage shows what you have used and what you have left across your AI coding tools. It is free and open source. Every number has a source. It never invents usage data or bypasses provider limits.

---

## 0. How to use this specification

### 0.1 Requirement terms

- **Shall** means a mandatory requirement. A test must verify it.
- **Shall not** means a mandatory prohibition.
- **Can** describes a capability of another system. It does not create a product requirement.
- **Not available** is a valid result when the provider does not supply a usable metric.
- **Unsupported** is a valid result when the documented interface or authentication mode does not support a capability.
- **Acceptance test** means an executable test or a recorded manual test with a clear expected result.

All requirements with an `OU-` identifier are mandatory for the complete release. All other rules in a table marked **Normative** are also mandatory. Examples are not provider observations.

### 0.2 Authority and interpretation

1. Official authenticated provider responses are the authority for provider identity, quota, credits, and token activity. An Unofficial source is never the authority. A local log is the authority only for what that log recorded on this device.
2. User-confirmed records are the authority for local billing data.
3. Local calculations are the authority only for derived display values.
4. An absent provider value is not zero. An absent limit is not proof of unlimited access.
5. A clock reaching the expected reset time is not proof that the provider reset a quota.
6. A subscription record does not prove that the provider received a payment.
7. A test fixture does not prove that a capability works with a live provider account.
8. This document does not grant access to any provider field or interface.
9. Agreement between two sources shows agreement. It does not prove that a value is true.
10. A leaderboard value is self-reported. It is never evidence of provider usage.

### 0.3 Scope control

Baseline 3.0 keeps the limit of **10 active profiles** from baseline 2.0. It widens the product from Codex to many AI coding tools. It removes promotions, payments, and receipt import. Archived profiles and tools read only from local logs do not count toward the active limit. A future change to the limit requires a new SRS baseline and new performance tests.

A release is not complete because a screen exists. A release is complete only when its mandatory behavior passes the acceptance tests.

### 0.4 Release stages (Normative)

| Stage | Content |
|---|---|
| 1. First public release | CLI and desktop app on Linux, with labelled Windows and macOS previews. All requirements, including Section 14A except OU-BOARD-012 |
| 2. Platform release | Windows and macOS after acceptance on real machines |
| 3. Public website | OU-BOARD-012 |

### 0.5 Owner decisions, 2 November 2026

1. The product is an everyday tracker for developers across many AI coding tools. The goal is wide adoption.
2. The name stays OpenUsage.
3. Unofficial sources are permitted only when they are auditable. It must be possible to know that a reading is accurate and not duplicated.
4. Promotions, payments, and receipt import are removed.
5. Quotas shall cover as many tools as possible.
6. Real-account testing covers Codex and Claude Code. The owner tests on a real Windows PC and a real Mac.
7. Stage 1 covers Windows, macOS, and Linux at once. Amended 7 November 2026: see Section 0.6.
8. One install supplies the CLI and the desktop app. A public website follows later.
9. First launch uses one consent screen with one action.
10. An opt-in leaderboard follows soon after Stage 1. Amended 7 November 2026: see Section 0.6.
11. WSL profiles stay. The CLI, statusline output, and customizable dashboard are in Stage 1.
12. Tools stay separate by default. A sum across tools is optional and carries a May overlap mark.
13. The product follows the simplicity rules in Section 2.3.
14. Permitted access is sign-in inside OpenUsage and local logs. Pasted keys and the saved credentials of other tools are excluded.
15. Paid code signing is optional, because anyone must be able to install and run the product for free.
16. The limit stays at 10 active profiles. Tools read only from local logs do not count.
17. Start at login exists and is off by default.
18. An Unofficial adapter with no real-account test can ship. It stays off by default and stays Unverified.
19. Customizable dashboard means reorder and hide. Moving a card to the top is pinning.
20. A Matched reading shows no mark. The leaderboard states Self-reported once, in its title.
21. `openusage report` works on a machine with no desktop app.
22. Subscriptions keep one renewal alert. The subscription calendar is removed.
23. Profile tags and browser shortcuts are removed.
24. There is no floating widget. Editor plugins stay outside Stage 1.


### 0.6 Owner decisions, 7 November 2026

1. The first release ships a working, usable product for feedback. Visual design can change later.
2. The opt-in leaderboard, Section 14A except OU-BOARD-012, is in the first release. Its service follows the [architecture decision of 7 November 2026](decisions/2026-11-07-leaderboard-service.md).
3. The first release ships on Linux. Windows and macOS builds ship as labelled previews until the owner's real-machine tests pass.
4. The first release includes local logs without sign-in, sharing, the CLI with statusline output, and npm installation.

### 0.7 Open items

- **OQ-1:** Choose how to split logs by account. The `ccusage` Codex adapter has no account handling. On the owner's Linux host on 2 November 2026, `creator_account_id` was filled in all 86 readable logs since October 2026 and absent from 7 older logs. 1,323 compressed logs were not read.
- **OQ-2:** Claim the npm name `openusage`. The registry reports it as unpublished since 14 June 2026.
- **OQ-3:** List the providers with documented quota interfaces, and complete a terms review for each other provider.
- **OQ-4:** Run the pinned `ccusage` binary. Its arguments, offline behavior, and per-folder use are not yet executed by this project.

---

## 1. Purpose and scope

### 1.1 Objectives

- Observe up to 10 independently configured account profiles.
- Track many AI coding tools through source adapters and local logs.
- Show the source and status of every number.
- Keep token history after tools delete their local logs.
- Give developers a free one-command install and a useful first launch.
- Support a test set with three Plus accounts and two Pro accounts.
- Display each quota bucket only when the official interface supplies it.
- Display quota use, derived remaining quota, expected reset time, and credits when available.
- Display token statistics and daily token activity when available.
- Display per-account and combined activity heatmaps.
- Keep local subscription records.
- Send local quota, connection, and renewal alerts.
- Support Windows, macOS, and Linux desktop installations.
- Keep prior accepted data available without network access.

### 1.2 Included features

1. Account profile registration, authentication, and identity checking.
2. Separate Codex environments for native profiles and approved Windows Subsystem for Linux (WSL) profiles.
3. Source adapters at Official, Local log, and Unofficial levels, including official Codex app-server reads and supported notifications.
4. Quota history, token activity, statistics, account comparison, and audit evidence for each reading.
5. Subscription records and one renewal alert.
6. System tray, local notification service, CSV/JSON export, encrypted backup, and restore.
7. Local settings, privacy controls, and complete local data deletion.
8. Local log analytics by tool, model, session, and project, with estimated API-equivalent value.
9. A command-line interface with statusline output.
10. Shareable Wrapped summaries and heatmap images.
11. A customizable dashboard.
12. An opt-in public leaderboard and, later, public web profiles.

### 1.3 Excluded features

- Web message counts, stored conversation content, page scraping, and cookie access for any provider.
- Use of credentials that another tool saved, and user-pasted provider keys or tokens.
- Promotions, discount periods, payment records, paid-spend reports, projections, trial history, and receipt import.
- Profile tags, browser-profile shortcuts, a subscription calendar, and a floating widget.
- Editor plugins in Stage 1. They can use `openusage status --json` later.
- OpenAI Platform API-key usage, organization billing, and API credit purchases.
- Work-only usage attribution when the provider supplies only shared account data.
- Cloud sync of the local database, remote telemetry, and any hosted service other than the opt-in Leaderboard Service in Section 14A.
- Automatic account rotation, quota evasion, or workload submission to test a limit.
- Spending earned reset credits or changing provider quota settings.
- Automated purchases, cancellation of subscriptions, or bank-mandate changes.
- Unattended monitoring after the OpenUsage application process stops.
- A built-in general web browser or a bundled Chromium rendering engine.

### 1.4 Product boundary

OpenUsage is an observer and a local record manager. It does not control the service limits of a provider account. It shall use only interfaces at the source levels in Section 5A. An interface that a provider does not document is permitted only at the Unofficial level, with every safeguard in Sections 5A and 5B.

---

## 2. Fixed technical decisions

| Component | Required choice | Reason |
|---|---|---|
| Desktop shell | Tauri 2 | Native process boundary and small deployment footprint |
| Native core | Rust, stable toolchain, Tokio | Safe process and task management |
| Frontend | React, TypeScript, Vite | Typed UI and portable webview rendering |
| UI system | Tailwind CSS and accessible primitives | Consistent controls |
| Main database | SQLite with SQLCipher 4 | Local transactions and encrypted data at rest |
| Database access | `rusqlite` behind one Rust storage service | Controlled writes and short transactions |
| Database mode | Write-ahead log (WAL), local disk only | Responsive reads during collection |
| Token arithmetic | Rust `u128` where valid; decimal strings at UI boundary | No JavaScript precision loss |
| Money arithmetic | Signed 64-bit integer minor units, with checked operations | Exact amounts for supported currencies |
| Usage sources | Rust source adapters (Section 5A). The Codex adapter uses documented Codex app-server JSON-RPC over local stdio | Every number has a declared source |
| Token analytics | Pinned, bundled `ccusage` binary run by the Rust supervisor (Section 5C) | Reuse maintained log parsers for many tools |
| Command line | Rust `openusage` binary with no webview dependency (Section 11A) | Terminal use on any supported machine |
| Codex login | Provider-managed ChatGPT browser and device-code login | Avoid a custom token service |
| Account collection | Rust process supervisor; maximum two active collections | Controlled memory and request load |
| Collection cycle | 300 seconds per eligible profile, subject to backoff | Cautious provider access |
| Charts | Accessible SVG/HTML heatmaps and time-series components | Clear data labels and keyboard access |
| Notifications | OS notification interfaces through approved Tauri commands | Local alerts |
| Backup | Locally encrypted, authenticated, versioned archive | Safe portable history |
| Distribution | One-command npm package with published hashes and build provenance (Section 16A); in-app updater only after update-signature tests | Free installation and supply-chain safety |

**Decision boundary:** Do not add PostgreSQL, Redis, DuckDB, Electron, CEF, FastAPI, a hosted backend, or application telemetry without a new approved architecture decision. The opt-in Leaderboard Service in Section 14A is the only planned hosted backend. It still needs its own approved architecture decision before construction.

### 2.1 Rendering engines

| Operating system | Default Tauri engine |
|---|---|
| Windows | Microsoft Edge WebView2 (Chromium-based) |
| macOS | WKWebView |
| Linux | WebKitGTK |

**OU-ARCH-001:** The UI shall work in all three engines.

**OU-ARCH-002:** The app shall not download or bundle CEF to replace the default Tauri engine.

**OU-ARCH-003:** Native and security-sensitive operations shall execute in Rust.

**OU-ARCH-004:** The frontend shall call only registered, typed Tauri commands.

**OU-ARCH-005:** Rust shall reject unauthorized commands and invalid arguments before any file or process operation.

**OU-ARCH-006:** The application shall use one active primary database per local operating-system user.

**OU-ARCH-007:** The app shall not place the live database in a network share or a cloud-synced folder.

**OU-ARCH-008:** Production builds shall not enable development tools, remote debugging ports, or arbitrary shell access.

### 2.2 Supported account capacity

**OU-CAP-001:** The app shall permit at most 10 active profiles.

**OU-CAP-002:** An archived profile shall retain its local history unless the user deletes it.

**OU-CAP-003:** Archived profiles shall not run scheduled collections or count toward the 10-profile limit.

**OU-CAP-004:** The app shall display a clear error when the user attempts to create an eleventh active profile.

**OU-CAP-005:** The user shall be able to archive an active profile to free a slot.

**OU-CAP-006:** The database schema shall use stable IDs and shall not use a ten-column account layout.

**OU-CAP-007:** The complete acceptance tests shall operate with 10 active profiles, including at least two provider plans.

**OU-CAP-008:** A tool that OpenUsage reads only from local logs shall not count as an active profile.

### 2.3 Simplicity rules (Normative)

**Purpose:** OpenUsage shows what you have used and what you have left.

**Standard:** The quality bar is the simplicity of Linear's website and Apple Notes.

**OU-DES-001:** Every decision shall improve user, developer, and agent experience together. A choice that helps one and harms another shall be redesigned.

**OU-DES-002:** A change shall not break existing behavior, stored data, or a safety rule.

**OU-DES-003:** The UI shall have no taglines or helper text. A number may carry one short context line that says what it measures, such as its account, window, reset time, pace, or comparison.

**OU-DES-004:** If a screen needs such text to make sense, the layout shall change.

**OU-DES-005:** A number in its normal state shall show the number, its name, and at most one short context line.

**OU-DES-006:** A number in an abnormal state shall show one mark, using the word that this document gives that state, such as Stale, Incomplete, Unverified, Disputed, Duplicate?, or May overlap. A missing number shall show Not available.

**OU-DES-007:** Every number shall open one detail panel. Source, time, scope, coverage, derivation, and reasons belong there. Where another requirement says to show or label these facts, the detail panel satisfies it for a number in its normal state.

**OU-DES-008:** Errors, consent, destructive confirmations, accessible names, and required notices are not helper text. They shall remain.

**OU-DES-009:** All data shall follow one path: adapter, checks, reading, store, view. A feature shall not add a second path.

**OU-DES-010:** Each job shall have one way: one reading contract, one detail panel, one binary for the CLI, and one verification command.

**OU-DES-011:** Adding a tool shall need one adapter module, its fixtures, and its declaration. It shall not need a change to UI or storage code.

**OU-DES-012:** People, scripts, and agents shall use the same commands and receive the same data.

**OU-DES-013:** A new setting, screen, state, or dependency shall exist only when a good default cannot replace it.

---

## 3. Definitions and data states

| Term | Exact meaning |
|---|---|
| Provider | The company or service that operates an AI coding tool or its quota. |
| Tool | An AI coding product, such as Codex or Claude Code. |
| Provider account | An authenticated account with a provider, as reported by a source adapter. |
| Profile | A local UUID, settings, and one isolated credential environment for one provider account. For Codex, this is one isolated Codex home. |
| Verified identity key | A stable account/workspace identity value from an approved official interface, when supplied. |
| Provisional identity | An account description, such as email, that does not prove a stable account/workspace key. |
| Quota bucket | One provider limit ID and window role, with duration and optional reset time. |
| Full snapshot | A complete accepted response from a documented read method. |
| Sparse update | An event that contains only some fields. |
| Provider date | The date string returned for a daily token bucket. |
| Source scope | The provider-defined population to which a metric applies. |
| Observed time | UTC time when OpenUsage accepted a response. |
| Current state | The most recent accepted value within the freshness threshold. |
| History | Prior accepted readings. History samples are not an event log of all provider activity. |
| Earned reset credit | A provider credit for a possible quota reset. OpenUsage only observes it. |
| Source adapter | One Rust module that reads one source and returns readings. |
| Source level | Official, Local log, or Unofficial. See Section 5A. |
| Reading status | Official, Matched, Unverified, or Disputed. See Section 3.2. |
| Local log | A file that a tool writes on this device. |
| Format fingerprint | A hash of the field names and value types that an adapter uses. It contains no values. |
| Evidence record | The stored record of how a reading was accepted. See Section 5B. |
| Estimated value | Token use priced at public API rates. It is not money that the user paid. |
| Detail panel | The one view that opens from a number. See OU-DES-007. |
| Mark | One short word beside a number in an abnormal state. See OU-DES-006. |

### 3.1 Metric states (Normative)

| State | When it applies | UI behavior |
|---|---|---|
| `current` | An accepted value is within its freshness period | Show value and observation time |
| `stale` | An accepted value is older than its freshness period | Show last value with Stale label |
| `not_available` | No usable provider value is known | Show “Not available”; do not show zero |
| `not_applicable` | Provider explicitly states that the metric does not apply | Show “Not applicable” |
| `unsupported` | Current official method/auth mode cannot supply the metric | Show “Unsupported” and reason |
| `failed` | The latest read failed | Show error; keep any last accepted value separately |
| `identity_unverified` | Source profile lacks a stable verified identity key | Show profile data; restrict identity-sensitive aggregation |

**OU-STATE-001:** Every remote metric shall have its own state and observation time.

**OU-STATE-002:** Failure shall never replace a valid measurement with zero or null.

**OU-STATE-003:** The UI shall distinguish an unknown value from a reported zero.

**OU-STATE-004:** The UI shall distinguish provider measurements from local calculations.

**OU-STATE-005:** Quota data shall be Current for 10 minutes after accepted collection.

**OU-STATE-006:** Activity data shall be Current for 30 minutes after accepted collection.

**OU-STATE-007:** Both freshness timers shall use observation time, not UI-render time.

**OU-STATE-008:** Connection errors shall remain visible until the next successful read or explicit dismissal.

**OU-STATE-009:** Offline status shall override the Current label for remote metrics and show the last accepted timestamp.

### 3.2 Reading status (Normative)

| Status | When it applies | UI behavior |
|---|---|---|
| `official` | An Official adapter supplied the reading | No mark |
| `matched` | An Unofficial reading agreed with an independent source under OU-AUD-004 | No mark |
| `unverified` | An Unofficial reading passed its checks and has no match | Unverified mark |
| `disputed` | Two sources disagree beyond tolerance | Disputed mark, under OU-AUD-005 |

A reading that fails its checks is not accepted. Its metric state is `not_available`.

**OU-STATE-010:** Every accepted reading shall have one reading status.

**OU-STATE-011:** The UI shall not call a reading true, accurate, or verified.

---

## 4. Profile, identity, and authentication

### 4.1 Profile records

**OU-ACCT-001:** The user shall be able to create, edit, archive, and remove a profile.

**OU-ACCT-002:** The app shall support 10 active profiles. This replaces the 50-profile requirement in baseline 1.0.

**OU-ACCT-003:** Each profile shall have an immutable UUID, editable label, and environment type.

**OU-ACCT-004:** The UI shall show provider-returned email and plan when available.

**OU-ACCT-005:** The app shall support documented browser and device-code ChatGPT login flows for Codex profiles.

**OU-ACCT-006:** Each Codex profile shall use a different managed `CODEX_HOME` directory.

**OU-ACCT-007:** The app shall check account identity after login, reconnect, profile switch, and detected credential change.

**OU-ACCT-008:** Profiles with the same verified identity key shall be marked as duplicates.

**OU-ACCT-009:** The user shall manually select the profile to inspect or open.

**OU-ACCT-011:** Windows users shall be able to configure an independent WSL profile after explicit approval.

**OU-ACCT-012:** Each profile shall offer Connect, Reconnect, Disconnect, and Refresh Now actions.

**OU-ACCT-013:** Removing a profile shall offer Archive History and Delete History as separate choices.

**OU-ACCT-014:** Disconnecting shall not delete history or cancel a provider subscription.

**OU-ACCT-015:** Each profile shall show Connected, Authentication required, Unauthorized, Offline, Unsupported, or Error.

**OU-ACCT-016:** Connecting a new profile shall create a new empty managed home.

**OU-ACCT-017:** OpenUsage shall not modify an existing external Codex home without explicit user approval.

**OU-ACCT-018:** The app shall not copy tokens or auth files between profile homes.

**OU-ACCT-019:** A profile cannot start normal monitoring before login completion and account-state validation.

**OU-ACCT-020:** A profile for a provider other than OpenAI shall use a sign-in method that OU-SRC-007 permits.

### 4.2 Identity-confidence rules

Official `account/read` examples can return an email and plan without a stable account/workspace ID. OpenUsage shall not assume that email is a universal identity key.

| Confidence | Evidence | Allowed action |
|---|---|---|
| `verified` | A stable identifier from an approved official identity interface | Attribute snapshots; allow distinct-account aggregation |
| `provisional` | Authenticated email or equivalent descriptive field, but no stable key | Show profile values; mark identity warning; exclude from verified combined totals |
| `unknown` | No usable identity evidence | Show connection problem; do not accept new metric attribution |
| `conflict` | Current identity contradicts the profile's accepted binding | Stop writes and collection; request user action |

**OU-ID-001:** Every accepted remote reading shall be linked to the collecting profile UUID.

**OU-ID-002:** A remote reading shall also contain the verified identity key when the provider supplies one.

**OU-ID-003:** The app shall not create a verified identity key from an email address alone.

**OU-ID-004:** The app shall exclude provisional and unknown identities from verified distinct-account totals by default.

**OU-ID-005:** The UI shall explain why an account is excluded from a combined total.

**OU-ID-006:** A verified duplicate identity shall contribute only once to a combined total.

**OU-ID-007:** A change in verified identity shall stop attribution until the user confirms a new binding.

**OU-ID-008:** The app shall never reassign old readings to a new provider identity during reconnect.

**OU-ID-009:** The UI shall not describe provisional combined values as provider-verified totals.

**OU-ID-010:** If no documented stable identity is available, verified aggregate metrics shall show Not available with the reason.

**OU-ID-011:** The app shall not read provider token files or undocumented endpoints to obtain a verified identity key.

**OU-ID-012:** A release claiming verified multi-account aggregation shall pass a live identity-contract test.

**OU-ID-013:** An account ID from an Unofficial adapter or a local log is not a verified identity key. It can support only the duplicate rules in Section 5D.

**Release gate ID-G1:** Do not ship verified multi-account totals unless a supported official identity contract proves distinct account/workspace attribution. When this is not possible, the UI must provide the documented Not available state. The feature remains in the product.

### 4.3 Login and logout

This section applies to the Codex adapter.

**OU-AUTH-001:** Login shall use `account/login/start` with an official supported ChatGPT login type.

**OU-AUTH-002:** The app shall await a successful `account/login/completed` event before treating login as complete.

**OU-AUTH-003:** The app shall match login-completion notifications to the correct login request.

**OU-AUTH-004:** Failed, canceled, and expired login attempts shall not change other profiles.

**OU-AUTH-005:** The app shall offer a Cancel Login control while login is pending.

**OU-AUTH-006:** Disconnect shall use documented `account/logout` only after the user confirms the action.

**OU-AUTH-007:** Only the selected profile's Codex process and home shall receive its authentication operations.

**OU-AUTH-008:** API-key-only and unsupported auth modes shall not be treated as ChatGPT-plan usage modes.

**OU-AUTH-009:** The app shall not implement experimental external-token login in baseline 3.0.

**OU-AUTH-010:** The provider-managed login system shall own token refresh and token persistence.

---

## 5. Codex app-server integration

This section applies to the Codex adapter. Section 5A applies to every adapter.

### 5.1 Allowed methods

| Method or event | Direction | Required use |
|---|---|---|
| `initialize` and required handshake | App → Codex | Establish a compatible protocol session |
| `account/read` | App → Codex | Read account and plan |
| `account/login/start` | App → Codex | Start ChatGPT login |
| `account/login/cancel` | App → Codex | Cancel pending login |
| `account/login/completed` | Codex → App | Confirm login result |
| `account/updated` | Codex → App | Detect auth and plan changes |
| `account/rateLimits/read` | App → Codex | Read quota and credit data |
| `account/rateLimits/updated` | Codex → App | Detect changed quota fields |
| `account/usage/read` | App → Codex | Read token summary and daily buckets |
| `account/logout` | App → Codex | User-requested logout |

**OU-PROTO-001:** The app shall use JSON-RPC over a local child-process stdio transport.

**OU-PROTO-002:** The app shall use a pinned, tested Codex executable version or an approved installed version.

**OU-PROTO-003:** Release documentation shall state the exact Codex versions that passed contract tests.

**OU-PROTO-004:** The app shall check the executable path, version, and source before launching it.

**OU-PROTO-005:** The app shall not download or run an unverified executable.

**OU-PROTO-006:** The app shall use a fixed argument set. The UI shall not supply arbitrary shell arguments.

**OU-PROTO-007:** The app shall not send inference or model-run requests to test limits.

**OU-PROTO-008:** The app shall not call `account/rateLimitResetCredit/consume`.

**OU-PROTO-009:** The app shall not call `account/sendAddCreditsNudgeEmail`.

**OU-PROTO-010:** The app shall not use private HTTP endpoints or browser-session extraction.

**OU-PROTO-011:** The adapter shall reject malformed, oversized, or incompatible responses.

**OU-PROTO-012:** An unsupported method shall affect only its capability. Other screens shall remain usable.

**OU-PROTO-013:** The adapter shall record a redacted reason for each failed request.

**OU-PROTO-014:** The repository shall include versioned protocol fixtures from documented shapes.

### 5.2 Child-process rules

**OU-PROC-001:** The Rust supervisor shall own all Codex child processes.

**OU-PROC-002:** At most two profiles shall perform active provider collection at one time.

**OU-PROC-003:** The app shall not run two collections for the same profile at the same time.

**OU-PROC-004:** A stopped or crashed child shall not change previously accepted data.

**OU-PROC-005:** The supervisor shall stop owned children when OpenUsage exits.

**OU-PROC-006:** The app shall close unused child processes after their pending operations complete.

**OU-PROC-007:** The app shall not kill unrelated user-owned Codex processes.

**OU-PROC-008:** Each process shall receive only its selected profile's approved environment.

**OU-PROC-009:** A read timeout shall stop that read and record a failure state.

**OU-PROC-010:** Process start, login, and read timeouts shall be configurable only through reviewed Rust constants.

**OU-PROC-011:** Initial defaults shall be 15 seconds for process readiness and 30 seconds for each provider read.

**OU-PROC-012:** A timeout shall not cause an unbounded retry loop.

**OU-PROC-013:** On application sleep or restart, the scheduler shall run at most one catch-up collection per eligible profile.

**OU-PROC-014:** The app shall measure child-process CPU and memory separately from the frontend in performance tests.

### 5.3 Credential and path isolation

**OU-ENV-001:** New native profile homes shall be inside an application-owned local directory.

**OU-ENV-002:** Managed profile-home names shall derive from local UUIDs, not email addresses.

**OU-ENV-003:** The app shall block path traversal, unsafe symlinks, and path escapes before writes.

**OU-ENV-004:** The app shall set restrictive file permissions on managed homes.

**OU-ENV-005:** The app shall treat external and WSL paths as read-only before explicit modification approval.

**OU-ENV-006:** Each WSL profile shall name an approved distribution and Linux home path.

**OU-ENV-007:** The app shall block automatic conversion of a native profile into a WSL profile.

**OU-ENV-008:** Secrets shall never pass to the React frontend.

---

## 5A. Source adapters

### 5A.1 Source levels (Normative)

| Level | Meaning | Default |
|---|---|---|
| Official | An interface that the provider documents for this use | On when the user connects the account |
| Local log | Files that a tool writes on this device | On after first-launch consent |
| Unofficial | An interface that the provider does not document for this use | Off |

### 5A.2 Rules

**OU-SRC-001:** Each source shall have one Rust adapter. An adapter shall return only readings in the contract of Section 17.

**OU-SRC-002:** Each adapter shall declare its provider, tool, level, metrics, sign-in method, allowed hosts or folders, format fingerprints, and tolerances.

**OU-SRC-003:** An Official adapter shall cite the provider document that describes its interface.

**OU-SRC-004:** An Unofficial adapter shall stay off until the user enables it for that provider.

**OU-SRC-005:** An Unofficial adapter shall not ship before a recorded review of the provider's terms. It shall not ship when those terms forbid it.

**OU-SRC-006:** The project shall disable an adapter when the provider asks.

**OU-SRC-007:** Permitted access is sign-in inside OpenUsage through the provider's login page. Local logs need no sign-in. Browser cookies, browser-session extraction, page scraping, the saved credentials of another tool, and pasted keys or tokens are prohibited.

**OU-SRC-008:** A session from sign-in inside OpenUsage shall stay in Rust. It shall use the OS credential store, or the passphrase fallback in Section 12.2 when no store is available.

**OU-SRC-009:** An adapter shall contact only its declared hosts. It shall not hide its client identity or bypass a provider control.

**OU-SRC-010:** Every adapter shall obey the collection limits in Section 10.1. A failure shall affect only that adapter's metrics.

### 5A.3 Claude Code adapter

**OU-SRC-011:** The adapter shall read the documented `rate_limits` status-line fields. Its level is Official.

**OU-SRC-012:** Installation shall show the exact Claude Code setting change and require consent. It shall preserve an existing status line and shall offer Undo.

**OU-SRC-013:** Capture shall keep only quota fields.

**OU-SRC-014:** These fields exist only during a session, after its first response, and for some plans. An absent window shall stay Not available or Stale. It shall not become zero.

**OU-SRC-015:** The documented fields carry no account ID. These readings shall keep `identity_unverified`.

### 5A.4 Stage 1 sources (Normative)

| Source | Level | Real-account test required for Stage 1 |
|---|---|---|
| Codex app-server | Official | Yes |
| Claude Code status line | Official | Yes |
| Local logs through `ccusage` | Local log | Yes, for Codex and Claude Code |
| Other providers | Official when documented, otherwise Unofficial | No |

---

## 5B. Audit

**OU-AUD-001:** Each accepted reading shall have one evidence record: adapter and version, source level, observation time, format fingerprint, check results, reading status, comparison result, and a hash of the redacted response.

**OU-AUD-002:** Evidence shall never contain tokens, cookies, email addresses, prompts, code, or full paths. The app shall not edit evidence after it writes it.

**OU-AUD-003:** A change in a field that an adapter uses shall give `not_available`. A new field that the adapter does not use shall not fail the reading. The app shall record it.

**OU-AUD-004:** An Unofficial reading is Matched only when all of these are true:

1. Its adapter version has a passing real-account test.
2. The second source uses a different interface or data store.
3. Both readings cover the same account and window.
4. Both readings were observed within 300 seconds.
5. Both values are within the declared tolerance. The default is 1 percentage point and 60 seconds for a reset time.

**OU-AUD-005:** A difference beyond tolerance shall give `disputed`. An Official value stays primary. Two Unofficial values shall both show.

**OU-AUD-006:** A real-account test shall compare at least five readings with the provider's own usage display, at two or more usage levels. It shall record adapter version, tool version, date, operating system, and largest difference.

**OU-AUD-007:** Raw test captures shall stay private. A summary shall be published in `docs/provider-evidence.md`. Anyone can submit a summary that contains no account IDs and no raw responses.

**OU-AUD-008:** A change to a used field or its parser shall void the test for that field.

**OU-AUD-009:** Official and Matched readings shall alert normally. An alert from an Unverified reading shall say Unverified. A Disputed reading shall send no alert.

**OU-AUD-010:** Exports shall include each reading's status and source.

---

## 5C. Local logs

### 5C.1 Consent and privacy

**OU-LOG-001:** First launch shall show one consent screen. It shall list detected tools, all selected, with one action. The app shall open no log before consent. Detection shall check only whether known folders exist.

**OU-LOG-002:** The user shall be able to change this selection in Settings.

**OU-LOG-003:** The app shall store only date, tool, model, token counts by type, estimated value, session key, project name, and log account key. It shall never store a prompt, response, tool output, or code.

**OU-LOG-004:** Project names shall stay in the encrypted database, backups, and exports that the user requests. The app shall not upload them.

### 5C.2 Running ccusage

**OU-LOG-005:** The package shall bundle one pinned `ccusage` binary for each platform. Rust shall check its hash before each run. The app shall not run another copy or download one.

**OU-LOG-006:** Each run shall use fixed arguments, no shell, JSON output, offline mode, a minimal environment, time and output limits, and one run at a time.

**OU-LOG-007:** Rust shall parse the output strictly and record the `ccusage` version in the evidence record. An unknown shape shall give Not available for that scan.

**OU-LOG-008:** A `ccusage` failure shall affect only local log data.

**OU-LOG-009:** A `ccusage` upgrade shall pass the fixture comparison in AT-65 before release.

### 5C.3 Totals

**OU-LOG-010:** The first scan shall pin the time zone for day boundaries. A later change to the display time zone shall not move saved days.

**OU-LOG-011:** Saved totals shall survive log deletion. A scan can raise a day total. A lower result shall not replace a saved total. The app shall save it as a difference, and the detail panel shall show it.

**OU-LOG-012:** A repeated scan of the same logs shall give the same totals.

**OU-LOG-013:** An unknown model shall keep its tokens and show Not available for estimated value.

**OU-LOG-014:** Estimated value shall not appear as spend. It shall not be added to money amounts.

**OU-LOG-015:** The app shall group logs by account only when a log records an account ID. A profile link needs a provider-issued ID or an explicit user link. Other logs shall appear under Unknown account. The app shall never infer an account from time, email, or folder name.

### 5C.4 Views

**OU-LOG-016:** Usage shall have one source control: Provider or This device. The control sets the scope, so numbers need no source label.

**OU-LOG-017:** This device shall offer hourly, daily, weekly, monthly, and custom ranges. It shall break down tokens by input, output, cached, reasoning, and total, and by model, session, and project.

**OU-LOG-018:** A session shall show its quota movement when saved readings exist for its start and end. The app shall not convert a quota percentage to tokens.

**OU-LOG-019:** WSL logs shall use the folder of an approved WSL profile.

---

## 5D. Duplicates

**OU-DUP-001:** Two readings describe one account only when both carry the same provider-issued account ID from the same provider. The app shall then show one account with two sources.

**OU-DUP-002:** Email, label, folder, and matching numbers are not proof. Without a shared ID, both accounts stay, with a Duplicate? mark.

**OU-DUP-003:** The app shall never add provider token totals to local log totals.

**OU-DUP-004:** Within one tool, the app shall rely on the replay and fork handling in `ccusage`. AT-65 shall qualify this handling.

**OU-DUP-005:** The app shall compare weekly totals when one log account matches one provider account. A local total above the provider total shall give a Disputed mark. A local total below it is normal, because other devices can also use the account.

**OU-DUP-006:** Tools shall stay separate by default. An optional sum shall carry a May overlap mark and shall never be called a total.

**OU-DUP-007:** The app shall never add quota percentages or credits across accounts or tools.

**OU-DUP-008:** The app shall scan one folder once, by resolved path, whatever links lead to it.

---

## 6. Quota, reset, and credit data

Sections 6.1 and 6.3 name Codex fields. Other adapters shall apply the same validation, display, and history rules to the fields that they supply.

### 6.1 Quota collection

**OU-QUOTA-001:** The collector shall use `account/rateLimits/read`.

**OU-QUOTA-002:** The collector shall prefer `rateLimitsByLimitId` when it is present.

**OU-QUOTA-003:** The collector shall use legacy `rateLimits` only as a fallback and shall not double-count it.

**OU-QUOTA-004:** Every quota record shall retain limit ID, name, role, duration, used percentage, reset time, observation time, and source.

**OU-QUOTA-005:** A 300-minute window shall be labeled Five-hour.

**OU-QUOTA-006:** A 10,080-minute window shall be labeled Seven-day.

**OU-QUOTA-007:** Other positive durations shall retain their provider duration and optional provider name.

**OU-QUOTA-008:** Daily, monthly, and model-specific labels shall appear only when supported by returned identity or duration data.

**OU-QUOTA-009:** The app shall show the provider-used percentage unchanged when it is valid.

**OU-QUOTA-010:** The UI shall call `100 - usedPercent` Remaining. The detail panel shall state that it is derived.

**OU-QUOTA-011:** The app shall reject invalid, non-finite, or out-of-range percentages from the current display.

**OU-QUOTA-012:** The app shall preserve redacted validation evidence for an invalid value.

**OU-QUOTA-013:** The app shall show a countdown only when `resetsAt` exists and is valid.

**OU-QUOTA-014:** The UI shall show reset timestamps with the selected time-zone name or UTC offset.

**OU-QUOTA-015:** After the reset time passes, the app shall show a Reset due mark until a new accepted reading confirms a new window.

**OU-QUOTA-016:** A sparse `account/rateLimits/updated` event shall never delete an omitted field.

**OU-QUOTA-017:** After a sparse update, the collector shall request a bounded full read before changing credit or reset-completion state.

**OU-QUOTA-018:** The UI shall show the provider's rate-limit-reached type when returned.

**OU-QUOTA-019:** Each quota bucket shall show its last successful collection time and current state.

**OU-QUOTA-020:** The app shall retain accepted quota history until the user deletes it or applies an explicit retention action.

**OU-QUOTA-021:** The app shall not add used percentages from different windows.

**OU-QUOTA-022:** The app shall distinguish a missing quota bucket from an explicit not-applicable result.

### 6.2 Quota reset confirmation

**Normative reset rule:** A clock alone never confirms a completed quota reset.

A reset is **confirmed** only after an accepted full provider snapshot shows new-window evidence. New-window evidence requires all of these conditions:

1. The previous accepted bucket has a valid `resetsAt` value.
2. The new full snapshot was observed at or after that prior reset time.
3. The new bucket has a later `resetsAt` value.
4. The new `usedPercent` is lower than the previous accepted `usedPercent`.
5. The bucket identity, role, and duration are compatible.

If any condition is missing, the app shall not announce reset completion. It shall keep the new measurement and show “Reset status not confirmed” when needed. This rule is conservative. It can omit a real reset alert rather than report an unverified reset.

**OU-RESET-001:** A confirmed reset shall create one new local alert-window generation.

**OU-RESET-002:** Reset confirmation shall not rely on a decrease in usage alone.

**OU-RESET-003:** Reset confirmation shall not rely on a changed timestamp alone.

**OU-RESET-004:** The app shall not repeat an earlier reset-completion alert after restart.

**OU-RESET-005:** Only an Official or Matched reading can confirm a reset.

**OU-RESET-006:** A window that a source stops reporting after its reset time is not reset evidence, and it is not zero.

### 6.3 Provider credits

**OU-CREDIT-001:** The UI shall display provider-returned workspace credit details only when supplied.

**OU-CREDIT-002:** The UI shall label provider credits separately from subscription prices and API billing.

**OU-CREDIT-003:** The authoritative earned reset-credit count shall be `availableCount` when returned.

**OU-CREDIT-004:** Optional credit rows shall show title, status, grant time, and expiry when returned.

**OU-CREDIT-005:** A null credit-detail array shall mean details are not available, not that the count is zero.

**OU-CREDIT-006:** An empty credit-detail array shall not override a separate authoritative count.

**OU-CREDIT-007:** The app shall not provide a control to spend earned reset credits.

**OU-CREDIT-008:** The app shall show the provider-returned effective monthly credit limit when it exists.

**OU-CREDIT-009:** The app shall show the provider-returned spend-control state when it exists.

**OU-CREDIT-010:** The app shall never describe provider workspace credits as confirmed subscription payments.

---

## 7. Token activity and statistics

This section applies to provider-reported activity. Section 5C applies to local log activity.

**OU-ACT-001:** The collector shall use `account/usage/read` when the authentication mode supports it.

**OU-ACT-002:** The UI shall display `summary.lifetimeTokens` when present.

**OU-ACT-003:** The UI shall display `summary.peakDailyTokens` when present.

**OU-ACT-004:** The UI shall display `summary.longestRunningTurnSec` when present.

**OU-ACT-005:** The UI shall display `summary.longestStreakDays` when present.

**OU-ACT-006:** The UI shall display `summary.currentStreakDays` when present.

**OU-ACT-007:** The app shall keep provider `dailyUsageBuckets[].startDate` unchanged.

**OU-ACT-008:** The app shall store each accepted token total as an exact nonnegative decimal integer.

**OU-ACT-009:** The UI shall distinguish account-wide activity from application-specific activity.

**OU-ACT-010:** If the provider gives shared Codex/Work activity, the UI shall label it Shared account activity.

**OU-ACT-011:** The app shall not invent separate Work, Codex, model, request, or message counts. Model values from local logs shall stay in the This device scope under OU-LOG-016.

**OU-ACT-012:** Null summary fields and null daily buckets shall be handled independently.

**OU-ACT-013:** A repeated daily total for one profile, provider date, and source scope shall replace the prior current total.

**OU-ACT-014:** An accepted replacement shall be atomic with its observation metadata.

**OU-ACT-015:** The app shall retain historical summary snapshots without treating them as event history.

**OU-ACT-016:** The UI shall show active days and daily totals for the known coverage period.

**OU-ACT-017:** The UI shall label incomplete or unknown daily-bucket coverage.

**OU-ACT-018:** The app shall not convert a provider date-only value by applying a time-zone offset.

**OU-ACT-019:** Provider token integers shall be parsed without passing through an imprecise JavaScript number.

**OU-ACT-020:** The UI shall show Not available for invalid integers or missing metrics.

**OU-ACT-021:** The app shall use separate freshness times for activity and quota reads.

**OU-ACT-022:** The app shall not equate quota percentage with a token count.

---

## 8. Heatmaps and account comparisons

### 8.1 Heatmap behavior

**OU-HEAT-001:** Every account detail screen shall show an individual daily token heatmap.

**OU-HEAT-002:** The dashboard shall show a combined heatmap for eligible selected identities.

**OU-HEAT-003:** Both heatmaps shall support Daily, Weekly, and Cumulative modes.

**OU-HEAT-004:** The combined view shall include account-selection controls.

**OU-HEAT-005:** All eligible connected, distinct verified identities shall be selected by default.

**OU-HEAT-006:** Each cell shall show provider date, exact token count, source scope, and coverage state through an accessible detail view.

**OU-HEAT-007:** A missing day shall not display as a verified zero-token day.

**OU-HEAT-008:** A provider-reported zero shall display as zero.

**OU-HEAT-009:** Combined daily totals shall sum only distinct identities with compatible dates and scopes.

**OU-HEAT-010:** Incompatible or identity-unverified profiles shall be excluded from verified totals with a stated reason.

**OU-HEAT-011:** Weekly mode shall use ISO weeks, Monday through Sunday.

**OU-HEAT-012:** Partial weeks shall carry an Incomplete label.

**OU-HEAT-013:** Cumulative mode shall sum available daily totals from the selected period start.

**OU-HEAT-014:** A cumulative heatmap total shall not be labeled Lifetime tokens without complete lifetime coverage.

**OU-HEAT-015:** Combined daily peak shall be the maximum of the combined daily sums.

**OU-HEAT-016:** Combined daily peak shall not sum separate account peak values.

**OU-HEAT-017:** Combined active-day counts shall use the union of dates with verified positive activity.

**OU-HEAT-018:** Combined streak values shall show Not available when missing days can change the result.

**OU-HEAT-019:** Combined longest task shall use the maximum compatible reported account value.

**OU-HEAT-020:** Combined lifetime tokens shall sum only compatible provider-reported lifetime totals from distinct verified identities.

**OU-HEAT-021:** Every combined metric shall show member count, exclusions, and provenance.

**OU-HEAT-022:** The UI shall allow a date range, account comparison, trend view, and contribution breakdown.

**OU-HEAT-023:** Charts shall have keyboard navigation and a non-color representation.

**OU-HEAT-024:** Charts shall provide an equivalent text table.

**OU-HEAT-025:** Activity export shall include provider date, profile, identity confidence, scope, observation time, and inclusion rule.

**OU-HEAT-026:** A heatmap shall show either provider-reported activity or local log activity. It shall not add the two.

**OU-HEAT-027:** A local log heatmap shall keep the day boundary from OU-LOG-010.

### 8.2 Aggregation algorithm (Normative)

1. Take the latest accepted daily bucket for each profile, provider date, and scope.
2. Remove archived or unselected profiles.
3. Group profiles only when they have the same verified identity key.
4. Choose one source profile per verified identity. Do not add duplicate profiles.
5. Exclude identities with incompatible metric scopes or calendar definitions.
6. Sum exact integer token values for each provider date.
7. Mark days incomplete when selected eligible identities have missing data.
8. Compute weekly sums from compatible reported dates.
9. Compute cumulative values only for the selected reporting period.
10. Keep provider lifetime totals separate from historical cumulative values.

Selection of a representative profile for duplicate identities shall use the newest accepted compatible reading. A tie shall use the lexicographically smaller profile UUID. The UI shall list excluded duplicates.

---

## 8A. Sharing

**OU-SHARE-001:** The app shall create Wrapped summaries and heatmap images locally, as PNG and SVG, with no network access.

**OU-SHARE-002:** An image shall contain no email address, account label, project name, path, or device name.

**OU-SHARE-003:** The app shall show a preview. Saving shall be an explicit action.

**OU-SHARE-004:** Missing days shall stay distinct from zero. Estimated value shall say Estimated.

**OU-SHARE-005:** An image shall show the OpenUsage name and repository address. The user can remove them.

**OU-SHARE-006:** The SVG shall render in a GitHub README.

---

## 9. Subscriptions

### 9.1 Data entry

**OU-BILL-001:** The user shall be able to create, edit, archive, and delete subscription records.

**OU-BILL-002:** Each subscription shall support provider, plan, tier, currency, price, interval, start, renewal, end, and auto-renew state.

**OU-BILL-005:** Manually entered billing data shall have source `user_confirmed`.

**OU-BILL-013:** Currency totals shall remain separate. The app shall not perform automatic exchange conversion.

**OU-BILL-014:** The app shall show whether each deadline is user-confirmed or unknown.

**OU-BILL-016:** The app shall show upcoming subscription deadlines.

**OU-BILL-018:** Financial displays shall show currency codes and exact decimal amounts.

Section 22.1 lists the billing requirements that baseline 3.0 removes. The former Section 9.2, receipt import protection, is removed.

---

## 10. Monitoring, refresh, and alerts

### 10.1 Collection schedule

**OU-MON-001:** Eligible profiles shall have a nominal 300-second collection cycle while the process runs.

**OU-MON-002:** A normal collection shall not start less than 300 seconds after the previous scheduled start for the same profile.

**OU-MON-003:** At most two collections shall run at the same time.

**OU-MON-004:** The scheduler shall stagger profile work to avoid a simultaneous ten-profile burst.

**OU-MON-005:** Refresh Now shall respect an active request, provider backoff, and a 30-second per-profile manual-refresh cooldown.

**OU-MON-006:** A provider update event shall schedule one bounded full read, not an unlimited event-triggered loop.

**OU-MON-007:** A transient failure shall use exponential backoff with jitter.

**OU-MON-008:** Transient backoff delays shall be 30, 60, 120, 240, 480, 960, 1,920, and 3,600 seconds before further retries.

**OU-MON-009:** Random jitter shall change a delay by no more than 20 percent.

**OU-MON-010:** An explicit provider retry time shall override a shorter local retry delay.

**OU-MON-011:** Persistent invalid authentication shall pause collection until reconnect.

**OU-MON-012:** Offline mode shall preserve accepted data and suppress immediate repeated requests.

**OU-MON-013:** Wake or restart shall trigger at most one immediate eligible read per profile.

**OU-MON-014:** Missed intervals shall not be replayed.

**OU-MON-015:** Pausing monitoring shall stop scheduled provider reads but retain local viewing functions.

**OU-MON-016:** The tray shall not claim that monitoring continues after application exit.

**OU-MON-017:** The app shall offer Start at login as a setting. The default shall be off.

### 10.2 Notification events

**OU-ALERT-001:** The app shall support usage alerts at 80%, 90%, and 100% used.

**OU-ALERT-002:** A threshold event shall fire only when a fresh accepted reading first crosses its level.

**OU-ALERT-003:** Repeated reads in the same confirmed window shall not repeat the same threshold event.

**OU-ALERT-004:** The app shall send a reset-completed alert only after reset confirmation under Section 6.2.

**OU-ALERT-005:** The app shall send one renewal alert, one day before a subscription renews or expires.

**OU-ALERT-007:** Confirmed invalid credentials shall produce one actionable authentication alert.

**OU-ALERT-008:** Three consecutive transient failures shall produce one collection-failure alert.

**OU-ALERT-009:** The user shall be able to enable or disable each alert type.

**OU-ALERT-010:** The user shall be able to set per-profile overrides and quiet hours.

**OU-ALERT-011:** An alert shall identify the profile, event, evidence time, and next action.

**OU-ALERT-012:** The app shall store a durable event key before notification delivery.

**OU-ALERT-013:** Restart shall not repeat an alert with a completed durable event key.

**OU-ALERT-014:** After wake, the app shall recheck overdue local deadlines once.

**OU-ALERT-015:** The app shall not repeatedly send the same past-due reminder.

**OU-ALERT-016:** The notification center shall show delivery state and the source of the event.

**OU-ALERT-017:** If the first reading in a confirmed window exceeds several thresholds, the app shall send only the highest applicable threshold alert.

**OU-ALERT-018:** After that first alert, lower satisfied thresholds shall be marked delivered for the window to prevent later duplicate alerts.

**OU-ALERT-019:** Alert behavior shall follow the reading status under OU-AUD-009.

### 10.3 Alert identity and quiet hours

Quota threshold event keys shall include: local profile UUID, verified identity key if available, limit ID, window role, duration, confirmed window generation, and threshold. A change in expected reset time shall not create a new window generation without reset confirmation.

For a local deadline, the event key shall include: record UUID, deadline kind, deadline date/time, and reminder offset. Editing a deadline shall create a new key only when the actual deadline changes.

The default quiet period shall be **22:00–08:00** in the selected local time zone. Notifications during this period shall be queued. The app shall deliver queued reminders at 08:00 if they remain relevant. Authentication and collector-failure alerts shall also respect quiet hours unless the user changes this policy.

---

## 11. User interface and accessibility

### 11.1 Required screens

**OU-UI-001:** The app shall provide three primary pages: Overview, Usage, and Settings. These pages or their dialogs shall provide accounts, account detail, tools, compare, token activity, subscriptions, notifications, and sharing.

**OU-UI-002:** The dashboard shall show active account count, plan breakdown, quota summary, data freshness, deadlines, activity statistics, and combined heatmap.

**OU-UI-003:** Account Detail shall show identity state, plan, quota buckets, reset credits, activity, individual heatmap, subscription, and errors.

**OU-UI-004:** Compare shall align quotas only when limit ID and window duration are compatible.

**OU-UI-005:** The UI shall support search by profile label and returned email.

**OU-UI-006:** The UI shall support filters by tool, plan, connection, quota state, reading status, deadline, and reporting period.

**OU-UI-007:** The app shall provide Light, Dark, and System themes.

**OU-UI-008:** The tray shall contain Open Dashboard, Refresh Now, Monitoring Status, and Quit.

**OU-UI-009:** The user shall select a time zone from recognized IANA time-zone names.

**OU-UI-010:** The UI shall format numbers and dates for the selected locale without changing stored values.

**OU-UI-011:** The app shall export selected data as CSV or JSON with source, units, scope, status, and coverage.

**OU-UI-012:** All primary functions shall work with a keyboard alone.

**OU-UI-013:** Every interactive control shall have an accessible name and visible focus state.

**OU-UI-014:** The app shall supply non-color chart labels and text-table alternatives.

**OU-UI-015:** Errors shall state the problem and a user action that can resolve it when known.

**OU-UI-016:** Each page shall show the selected profile or reporting scope.

**OU-UI-017:** A data card shall show the latest accepted observation time or Not available.

**OU-UI-018:** Quota reset links shall lead only to documented official provider pages.

**OU-UI-019:** The app shall not show synthetic readings in a connected production view.

**OU-UI-020:** A clearly marked demonstration mode may use test data only when the user enables it explicitly.

**OU-UI-021:** Minimize to tray shall leave monitoring active only while the OpenUsage process continues.

**OU-UI-022:** Quit shall stop monitoring and terminate only OpenUsage-owned child processes.

**OU-UI-023:** The Settings view shall include a guarded Delete All Local OpenUsage Data action.

**OU-UI-024:** The tray shall show the lowest remaining quota and any mark on it.

**OU-UI-025:** When logs exist, first launch shall reach local history with no account sign-in.

**OU-UI-026:** The user shall be able to reorder and hide Overview cards, and to reset the layout. A mark, Not available, and an error shall stay visible on any shown card.

**OU-UI-027:** Quota history graphs shall use saved readings for any range. Provider token graphs shall start at daily, because the provider supplies daily buckets.

### 11.2 Accessibility target

**OU-ACC-001:** Core UI behavior shall meet WCAG 2.2 AA intent.

**OU-ACC-002:** The release tests shall cover keyboard use, color contrast, zoom, screen-reader labels, and chart alternatives.

**OU-ACC-003:** The application shall not rely only on color to show data state.

**OU-ACC-004:** A provider error shall not move keyboard focus without an announced reason.

**OU-ACC-005:** Heatmap date details shall be available without hover.

---

## 11A. Command line

| Command | Result |
|---|---|
| `openusage` | Current quotas and today's usage |
| `openusage status --json` | The same data as versioned JSON |
| `openusage statusline` | One line for tmux and shell prompts |
| `openusage report` | Local log totals, with no database needed |
| `openusage app` | Opens the desktop app |
| `openusage doctor` | Checks the environment and prints each fix |

**OU-CLI-001:** The CLI shall have no webview dependency.

**OU-CLI-002:** Except for `openusage report` and the Stage 2 `openusage board` commands, the CLI shall read saved data only. It shall not write the database, start collection, or use the network.

**OU-CLI-003:** `openusage report` shall ask once for consent, accept `--yes` for scripts, and save nothing.

**OU-CLI-004:** JSON output shall give each value with its state, status, source, and observation time. A missing value shall be null with its state, never zero. The schema shall be versioned.

**OU-CLI-005:** Each failure shall have one line of text, a stable code, and a documented exit code. JSON mode shall return the same code as JSON.

**OU-CLI-006:** A locked database shall give Locked and an exit code. `openusage statusline` shall never prompt.

**OU-CLI-007:** Output shall contain no secrets, prompts, or code.

**OU-CLI-008:** `--help` shall be complete for every command.

---

## 12. Local data storage

### 12.1 Storage principles

**OU-DATA-001:** SQLCipher-backed SQLite shall be the only application-record database.

**OU-DATA-002:** SQLite shall run in WAL mode on a local file system.

**OU-DATA-003:** The app shall enable and verify foreign-key enforcement on each database connection.

**OU-DATA-004:** One Rust storage service shall coordinate all writes.

**OU-DATA-005:** Database changes shall use versioned, transactional migrations.

**OU-DATA-006:** A failed migration shall keep a recoverable prior database state.

**OU-DATA-007:** All remote readings shall store profile UUID, source method, raw scope, normalized unit, and observation time.

**OU-DATA-008:** The app shall keep a small current-state table separate from historical snapshots.

**OU-DATA-009:** A current-state update and its history entry shall share one transaction.

**OU-DATA-010:** Repeated daily bucket reads shall replace the current bucket without summing the values.

**OU-DATA-011:** History shall remain until the user chooses deletion or an explicit retention rule.

**OU-DATA-012:** Default history retention shall be unlimited.

**OU-DATA-013:** The app shall not silently compact, delete, or aggregate away raw accepted quota history.

**OU-DATA-014:** The app shall never store access tokens, refresh tokens, browser cookies, or provider auth-file contents in SQLite.

**OU-DATA-015:** The app shall not sync the live database to cloud folders.

**OU-DATA-016:** The app shall provide complete local export and explicit local data deletion.

**OU-DATA-017:** Deleting local data shall not claim to delete provider-side or Leaderboard Service records.

**OU-DATA-018:** Database reads used by the dashboard shall not scan full historical snapshots when a current-state index is available.

**OU-DATA-019:** The storage service shall set `PRAGMA synchronous=FULL` for the primary database.

**OU-DATA-020:** The storage service shall set `PRAGMA busy_timeout=5000` on its database connections.

**OU-DATA-021:** SQLite writes shall use prepared statements and bounded, short transactions.

**OU-DATA-022:** A restore, backup, or export shall use a consistent SQLite snapshot.

**OU-DATA-023:** The app shall never copy only the main database file while active WAL content is required.

**OU-DATA-024:** When the app is offline, all displayed provider readings shall be labeled Stale, regardless of their age.

**OU-DATA-025:** Delete All Local Data shall remove managed OpenUsage records and managed profile homes only after explicit confirmation.

**OU-DATA-026:** Delete All Local Data shall not delete an external or WSL Codex home without separate explicit approval.

**OU-DATA-027:** Local log summaries and reading evidence shall follow the same encryption, backup, export, and deletion rules as other application records.

**OU-DATA-028:** Delete All Local Data shall not delete the logs of another tool.

### 12.2 Encryption and unlock behavior

**OU-CRYPT-001:** The app shall generate a random 256-bit database key when it creates a database.

**OU-CRYPT-002:** SQLCipher shall encrypt the primary SQLite database, including its SQLite-managed WAL content.

**OU-CRYPT-003:** The app shall protect the database key with an OS credential store when that store is available.

**OU-CRYPT-004:** Windows shall use the current user's protected credential service.

**OU-CRYPT-005:** macOS shall use Keychain.

**OU-CRYPT-006:** Linux shall use Secret Service when an unlocked compatible keyring is available.

**OU-CRYPT-007:** If no supported key store is available, the app shall require a user passphrase at each unlock.

**OU-CRYPT-008:** The passphrase fallback shall derive a key-encryption key with Argon2id and a unique random salt.

**OU-CRYPT-009:** The passphrase shall not be saved to disk or logs.

**OU-CRYPT-010:** Monitoring shall not start until the application database is unlocked.

**OU-CRYPT-011:** A missing or invalid key shall not cause automatic database reset.

**OU-CRYPT-012:** The app shall show a recovery message when a protected key cannot be accessed.

**OU-CRYPT-013:** Opening a database shall verify that SQLCipher can read its encrypted schema before migration.

**OU-CRYPT-014:** Exports shall be treated as potentially sensitive and shall require an explicit save action.

**OU-CRYPT-015:** The fallback passphrase shall wrap the random database key with authenticated XChaCha20-Poly1305 encryption.

**OU-CRYPT-016:** The wrapped-key file shall store its format version, unique salt, KDF parameters, and unique nonce.

**OU-CRYPT-017:** The app shall never write the unwrapped database key to a plain file.

**Security limit:** OpenUsage controls its database and its own files. It does not claim to encrypt the provider-owned Codex credential files or the operating-system swap file.

### 12.3 Backup and restore

**OU-BACKUP-001:** The app shall create a versioned encrypted archive of selected non-secret local data.

**OU-BACKUP-002:** Backup encryption shall use Argon2id for password derivation and XChaCha20-Poly1305 for authenticated encryption.

**OU-BACKUP-003:** Each archive shall have a unique random salt and nonce.

**OU-BACKUP-004:** Backup archives shall exclude provider credentials and browser data.

**OU-BACKUP-005:** A backup shall contain a schema version and authenticated file manifest.

**OU-BACKUP-006:** Restore shall check authentication and archive structure before it changes live data.

**OU-BACKUP-007:** Restore shall show a preview and require user confirmation.

**OU-BACKUP-008:** Restore shall create a reversible pre-restore checkpoint.

**OU-BACKUP-009:** Tampered archives shall fail without changing the current database.

**OU-BACKUP-010:** A failed or interrupted restore shall preserve the previous valid database.

**OU-BACKUP-012:** The app shall verify a restored database before marking restore complete.

**OU-BACKUP-013:** Archive version 1 shall use authenticated chunks of at most 1 MiB each.

**OU-BACKUP-014:** Each chunk shall have a unique nonce and an authenticated chunk index and length.

**OU-BACKUP-015:** The archive shall reject missing, repeated, reordered, and truncated chunks.

**OU-BACKUP-016:** The restore parser shall enforce archive entry-count and total-size limits before extraction.

**OU-BACKUP-017:** The archive shall enforce a maximum restored size of 5 GiB and 10,000 archive-file entries. SQLite row counts shall not count as archive-file entries.

**OU-BACKUP-018:** The app shall refuse backup or restore when available free space is insufficient for safe completion.

---

## 13. Required database schema

All tables shall have explicit primary keys. Mutable business records shall store creation and update times in UTC. Foreign keys shall be enabled. Money shall be stored in integer minor units. All stored money amounts shall be nonnegative signed 64-bit integers. Token totals shall be stored as decimal text with checked parsing. An estimated API-equivalent value is not money under this rule. It shall be stored as labeled decimal text.

| Table | Required fields and rules |
|---|---|
| `accounts` | Local UUID; verified identity key nullable; identity confidence; returned email/plan nullable; first and last observed UTC |
| `profiles` | UUID; account UUID nullable; name; type native/WSL; approved home reference; connection state; archived flag; timestamps |
| `profile_identity_events` | UUID; profile UUID; old/new identity evidence; state; observation time; no secrets |
| `current_quota` | Profile UUID; limit ID; role; current duration, percentage, reset UTC, source time, state; unique profile/limit/role |
| `quota_snapshots` | UUID; profile UUID; identity key nullable; bucket fields; time; state; schema version; source |
| `credit_snapshots` | UUID; profile UUID; observed UTC; provider credit type; count and returned metadata |
| `activity_summaries` | UUID; profile UUID; five nullable stats; source scope; observed UTC |
| `current_activity_days` | Profile UUID; provider date; scope; exact token text; observed UTC; unique profile/date/scope |
| `activity_day_history` | UUID; profile/date/scope; prior and accepted exact token text; observed UTC; collection run |
| `subscriptions` | UUID; profile/account association; plan; amount; currency; interval; dates; auto-renew; evidence |
| `alert_policies` | UUID; event type; profile override nullable; threshold/offset; enabled; quiet-hours data |
| `alert_events` | UUID; unique event key; evidence; delivery state; scheduled and delivered times |
| `collection_runs` | UUID; profile UUID; method; start/end; outcome; redacted error code |
| `settings` | Schema version; time zone; locale; theme; monitoring state; retention policy; dashboard layout; start-at-login state |
| `source_adapters` | Adapter ID; version; provider; tool; level; enabled state; consent time |
| `reading_evidence` | UUID; reading reference; adapter version; level; format fingerprint; check results; reading status; comparison result; redacted response hash; observed UTC |
| `log_sources` | UUID; tool; resolved folder reference; consent state; pinned time zone; last scan UTC |
| `local_usage_days` | UUID; log source; tool; model; local date; token text by type; estimated value text nullable; scanner version; coverage; unique source/date/model |
| `local_usage_sessions` | Session key; log source; project name nullable; log account key nullable; start and end UTC; token text by type; estimated value text nullable |
| `local_usage_differences` | UUID; log source; date; model; saved and new token text; scan UTC |
| `log_accounts` | UUID; tool; log account key; linked profile nullable; link kind |

### 13.1 Database constraints (Normative)

- Foreign keys shall reject orphaned child records.
- All enumerated states shall have `CHECK` constraints or equivalent validated storage types.
- All profile IDs shall be UUIDs.
- Date-only values shall use `YYYY-MM-DD` and pass calendar validation.
- UTC timestamps shall use an unambiguous stored UTC format.
- Money amounts shall use an ISO 4217 currency code with the currency's defined minor-unit scale.
- Token decimal strings shall contain digits only and represent nonnegative exact integers.
- `alert_events.event_key` shall be unique.
- `current_quota(profile_id, limit_id, window_role)` shall be unique.
- `current_activity_days(profile_id, provider_date, source_scope)` shall be unique.
- Indexes shall support latest quota, activity date range, alert due date, and subscription deadline reads.
- An accepted remote measurement shall never be written under a conflicting provider identity.

---

## 14. Security and privacy

**OU-SEC-001:** The desktop app and CLI shall have no telemetry endpoint or hidden network call. The opt-in Leaderboard Service is the only OpenUsage-hosted service.

**OU-SEC-002:** Only declared adapter operations for authorized sign-in and collection may contact a provider. Only a joined Leaderboard client may contact the Leaderboard Service.

**OU-SEC-003:** The frontend shall not receive tokens, auth-file contents, browser cookies, or the database encryption key.

**OU-SEC-004:** Provider credentials shall remain in the supported provider-managed credential environment. A session from sign-in inside OpenUsage shall follow OU-SRC-008.

**OU-SEC-005:** The app shall isolate managed home directories by profile and file permission.

**OU-SEC-006:** Every local command and RPC field shall pass length, type, range, and path validation.

**OU-SEC-007:** Tauri capability permissions shall follow least privilege.

**OU-SEC-008:** The frontend shall not have unrestricted filesystem or process execution access.

**OU-SEC-009:** The app shall use a restrictive content security policy. Remote scripts and remote UI content shall be blocked.

**OU-SEC-010:** The user shall approve changes to external homes, logout, deletion, and restore.

**OU-SEC-011:** Logs and support diagnostics shall redact tokens, personal identifiers, and sensitive document content by default.

**OU-SEC-012:** Every export shall warn before including account email or identifiable activity.

**OU-SEC-013:** Backups shall use authenticated encryption, per-archive randomness, and tamper checks.

**OU-SEC-014:** The app shall not run quota-testing inference, consume provider credits, or rotate accounts for workloads.

**OU-SEC-015:** Dependencies shall use pinned versions or lockfiles and receive vulnerability checks.

**OU-SEC-016:** The release shall include a vulnerability disclosure policy and a software bill of materials.

**OU-SEC-017:** The app shall prevent local data leakage through temporary plaintext database copies.

**OU-SEC-018:** Sensitive temporary export files shall be removed after their purpose ends when the app owns the file.

**OU-SEC-019:** The app shall display external links before opening an unrecognized domain.

**OU-SEC-020:** The app shall not accept provider web pages as commands or application instructions.

**OU-SEC-021:** The app shall treat log contents as untrusted data. Log text shall never become a command, a path to open, or rendered markup.

**OU-SEC-022:** The documentation shall state that the bundled `ccusage` process runs with the rights of the user and is not sandboxed. The app shall not claim otherwise.

### 14.1 Threat-model minimum

The release threat model shall cover: malicious local files; compromised browser content; malicious path links; poisoned JSON-RPC data; poisoned adapter responses; malicious log content; substituted Codex or `ccusage` binaries; a compromised npm package; fake leaderboard submissions; stolen backup archives; lost encryption keys; account switching; duplicate identities; corrupted updates; log leakage; and unauthorized filesystem access.

The release shall not claim protection against an attacker with full control of the user's unlocked operating-system account. It shall still use reasonable OS access controls.

---

## 14A. Opt-in leaderboard

**OU-BOARD-001:** The leaderboard shall be off by default. The app and CLI shall not contact the service before the user joins.

**OU-BOARD-002:** Joining shall use GitHub sign-in. The client token shall stay in the OS credential store, and the user shall be able to revoke it.

**OU-BOARD-003:** An upload shall contain only date, tool, model, token counts, estimated value, and a random device ID.

**OU-BOARD-004:** An upload shall never contain prompts, code, paths, project names, session content, email addresses, provider account IDs, or device names.

**OU-BOARD-005:** `openusage board sync --dry-run` shall print the exact upload through the same code path as a real sync.

**OU-BOARD-006:** Only local log totals shall upload. Provider readings shall not upload.

**OU-BOARD-007:** The leaderboard title shall say Self-reported. Leaderboard values shall never become readings in the app.

**OU-BOARD-008:** The leaderboard shall rank tokens and estimated value over 7 days, 30 days, and all time.

**OU-BOARD-009:** A repeated sync shall replace the same device, date, tool, and model. A profile shall sum its devices.

**OU-BOARD-010:** Leaving shall remove the user's data from public view at once and from storage within 30 days.

**OU-BOARD-011:** The service source shall be public, with a published moderation policy. The service can flag implausible totals but cannot prove any total. The app shall work fully without the service. The service needs an approved architecture decision before construction.

**OU-BOARD-012:** In Stage 3, a joined user can have a public web profile. It shall show only the data that OU-BOARD-003 permits.

---

## 15. Performance, reliability, and operating conditions

### 15.1 Reference conditions

The reference computer shall have: four CPU cores, 8 GiB RAM, a local SSD, a current supported 64-bit operating system, and a release build. Tests shall use 10 active profiles and 365 provider-date buckets per profile. Quota-history stress tests shall also use 2.1 million synthetic rows.

| ID | Mandatory target | Measurement |
|---|---|---|
| OU-NFR-001 | Cached dashboard ready in ≤ 1.5 seconds at the 95th percentile | 30 cold-process launches, database already unlocked |
| OU-NFR-002 | Local account switching and heatmap mode changes in ≤ 200 ms at the 95th percentile | 100 actions, excluding provider network time |
| OU-NFR-003 | No routine scheduled starts closer than 300 seconds for one profile | Scheduler trace |
| OU-NFR-004 | No more than two active profile collections | Concurrency test |
| OU-NFR-005 | No catch-up burst after wake | Sleep/resume test |
| OU-NFR-006 | Prior accepted values survive network, auth, and process failure | Fault injection |
| OU-NFR-007 | Migrations and accepted current-state writes are transactional | Interrupted-write tests |
| OU-NFR-008 | No floating-point arithmetic for exact token or cash totals | Property-based tests |
| OU-NFR-009 | Time changes, DST, null fields, and malformed payloads fail safely | Boundary tests |
| OU-NFR-010 | Normal operation needs no administrator privilege | Platform installation tests |
| OU-NFR-011 | Core functions meet WCAG 2.2 AA intent | Accessibility tests |
| OU-NFR-012 | CI shall build and test supported Windows, macOS, and Linux releases | CI matrix |
| OU-NFR-013 | Repo shall include build instructions, licenses, security policy, and redacted diagnostics | Release review |
| OU-NFR-014 | The UI shall remain usable while a provider read is pending | UI responsiveness test |
| OU-NFR-015 | The app shall not keep more than two active collecting child processes | Process test |
| OU-NFR-016 | A failed backup or update shall not corrupt the previous valid database | Recovery tests |

| OU-NFR-017 | At most 450 MiB resident memory for OpenUsage app processes, excluding Codex and `ccusage` children, after idle stabilization | Peak sampled across five idle minutes with ten cached profiles |
| OU-NFR-018 | Mean idle CPU below 5% of one logical core, excluding collection periods | Five-minute trace after warm startup |
| OU-NFR-019 | `openusage statusline` completes in ≤ 200 ms at the 95th percentile | 100 runs with a cached database; proposed target |
| OU-NFR-020 | First local history appears in ≤ 60 seconds after consent | Reference machine with 1 GiB of synthetic logs; proposed target |
| OU-NFR-021 | A local log scan does not make the UI miss OU-NFR-002 | Interaction trace during a scan; proposed target |

**Performance measurement rule:** Run the benchmark on each release platform. Report the machine, operating-system release, app commit, schema version, and observed distribution. A test passes only if all specified limits pass on each reference platform.

### 15.2 Reliability rules

**OU-REL-001:** Startup shall load cached local records before starting provider collection.

**OU-REL-002:** The UI shall not block on a provider network response.

**OU-REL-003:** An interrupted collection shall leave its previously accepted measurement valid.

**OU-REL-004:** A failed database write shall not partially publish a new quota state.

**OU-REL-005:** A full disk shall produce an actionable error without deleting accepted records.

**OU-REL-007:** The app shall preserve its database during an interrupted application update.

**OU-REL-008:** A clock rollback shall not create false reset-completion notifications.

---

## 16. Platform support and distribution

### 16.1 Baseline platforms

- Windows 11 x64 with an available supported WebView2 runtime.
- Windows 10 22H2 x64 only when the installation receives applicable operating-system security updates.
- macOS 14 or later on Apple Silicon and Intel Macs, subject to the tested release matrix.
- Ubuntu 24.04 LTS x64 with the required WebKitGTK and desktop notification packages.
- Windows 11 with an explicitly configured supported WSL distribution for optional WSL profiles.

Other Linux distributions are not release-verified until a published test matrix names them. Release notes shall list exact tested versions and architectures. The app shall show Unsupported environment when a required runtime is missing.

Stage 1 shall run on Linux. Windows and macOS builds shall ship as labelled previews until they pass acceptance on real machines. CI does not replace these runs.

**OU-PLAT-001:** Windows packaging shall handle missing or damaged WebView2 installations with a documented recovery action.

**OU-PLAT-002:** Linux packaging shall list exact WebKitGTK runtime dependencies.

**OU-PLAT-003:** Paid macOS signing and notarization are optional. Release notes shall state whether a macOS build has them.

**OU-PLAT-004:** Paid Windows code signing is optional. Release notes shall state whether a Windows build has it.

**OU-PLAT-005:** Every release shall publish package hashes. Linux releases shall also publish installation dependencies.

**OU-PLAT-006:** Release binaries shall use a reproducible, locked dependency build process where supported.

**OU-PLAT-007:** The release shall include installer, upgrade, uninstall, and local-data deletion instructions.

**OU-PLAT-008:** The app shall not replace an existing user Codex installation without approval.

**OU-PLAT-009:** A built-in update check shall be opt-in and shall only read verified release metadata from a documented source.

**OU-PLAT-010:** An application update shall not add telemetry or alter the privacy boundary.

---

## 16A. Distribution

**OU-DIST-001:** `npm install -g openusage` shall install the CLI and the desktop app.

**OU-DIST-002:** Installation and use shall need no payment and no OpenUsage account.

**OU-DIST-003:** The package shall use per-platform packages with no install scripts and no runtime downloads. It shall contain the CLI, the desktop app, the pinned Codex companion, `ccusage`, and notices. Each platform needs its own pinned and qualified copies.

**OU-DIST-004:** The CLI shall work when desktop libraries are missing. `openusage app` shall then print the fix.

**OU-DIST-005:** Each release shall publish hashes, npm provenance, a build attestation, and an SBOM.

**OU-DIST-006:** The documentation shall state what an unsigned build does on each operating system.

**OU-DIST-007:** A real Mac test shall cover the tray, notifications, and Keychain. If a plain binary fails, the package shall ship an ad-hoc-signed app bundle.

**OU-DIST-008:** Uninstalling shall keep user data. The documentation shall state how to delete it.

**OU-DIST-009:** One documented command shall build the product from source.

---

## 17. Data contracts and numeric rules

```typescript
type MetricState =
  | "current"
  | "stale"
  | "not_available"
  | "not_applicable"
  | "unsupported"
  | "failed"
  | "identity_unverified";

type IdentityConfidence = "verified" | "provisional" | "unknown" | "conflict";

type SourceLevel = "official" | "local_log" | "unofficial";

type ReadingStatus = "official" | "matched" | "unverified" | "disputed";

type DataSource = string;                // registered adapter ID, or "user_confirmed"

type QuotaWindow = {
  profileId: string;                   // immutable local UUID
  verifiedIdentityKey: string | null;  // official stable key, when available
  identityConfidence: IdentityConfidence;
  limitId: string;
  limitName: string | null;
  windowRole: "primary" | "secondary" | string;
  windowDurationMins: number | null;
  usedPercent: number | null;
  resetsAtUtc: string | null;
  reachedType: string | null;
  observedAtUtc: string;
  state: MetricState;
  source: DataSource;
  sourceLevel: SourceLevel;
  readingStatus: ReadingStatus;
  evidenceId: string;
};

type ActivityBucket = {
  profileId: string;
  verifiedIdentityKey: string | null;
  providerDate: string;                // original YYYY-MM-DD
  tokens: string;                      // exact nonnegative integer
  sourceScope: string;
  observedAtUtc: string;
  state: MetricState;
};

type SummaryStats = {
  profileId: string;
  sourceScope: string;
  lifetimeTokens: string | null;
  peakDailyTokens: string | null;
  longestRunningTurnSec: number | null;
  longestStreakDays: number | null;
  currentStreakDays: number | null;
  observedAtUtc: string;
};

type LocalUsageDay = {
  logSourceId: string;
  tool: string;
  model: string;
  localDate: string;                   // YYYY-MM-DD in the pinned time zone
  inputTokens: string | null;
  cachedInputTokens: string | null;
  outputTokens: string | null;
  reasoningTokens: string | null;
  totalTokens: string;
  estimatedValueUsd: string | null;    // decimal text, never money
  scannerVersion: string;
  coverage: "complete" | "logs_gone" | "partial";
};
```

**OU-NUM-001:** A valid quota percentage is finite and within `0` through `100`, inclusive. An adapter can declare a higher limit only for a documented over-limit measure, such as a spend limit.

**OU-NUM-002:** Derived remaining percentage is `100 - usedPercent` for a valid value only.

**OU-NUM-003:** A token count shall be a nonnegative whole number representable by the supported Rust exact type.

**OU-NUM-004:** Token totals shall cross the frontend boundary as decimal strings.

**OU-NUM-005:** A provider date shall be a valid Gregorian calendar date in `YYYY-MM-DD` format.

**OU-NUM-006:** Reset times shall be interpreted as Unix seconds and stored as UTC timestamps.

**OU-NUM-007:** Monetary arithmetic shall use checked integer operations and the currency's stated scale.

**OU-NUM-008:** No automatic rounding shall change the stored provider token or quota measurement.

**OU-NUM-009:** Display rounding shall not change the retained original value.

---

## 18. State machines and user workflows

### 18.1 Collection state machine

1. **Load:** Unlock the database. Load cached state. Show prior data with its age.
2. **Prepare:** Select an eligible profile. Confirm its approved home and executable.
3. **Authenticate:** Read supported account state. Apply the identity-confidence rules.
4. **Collect:** Read supported quota and activity methods. Validate each response independently. Run the checks in Section 5B.
5. **Persist:** Write accepted current state, history, and evidence in one transaction.
6. **Evaluate:** Check thresholds, reset evidence, deadline policies, and duplicate event keys.
7. **Wait:** Schedule the next 300-second cycle, subject to provider backoff.
8. **Recover:** Keep old data on failure. Show the error. Do not replay missed cycles.

### 18.2 Identity conflict

1. Stop remote attribution for the affected profile.
2. Keep its old history unchanged.
3. Show the previous and current non-secret identity evidence.
4. Let the user reconnect, create a new profile, or keep the profile disconnected.
5. Resume collection only when the new binding is valid under Section 4.

### 18.3 Local log workflow

1. Check which known folders exist. Open no file.
2. Show one consent screen.
3. Check the `ccusage` hash and run it with fixed arguments.
4. Validate the output.
5. Store summaries in one transaction under OU-LOG-011.
6. Show history.


### 18.4 Backup workflow

1. The user selects data and enters a backup password.
2. The storage service creates a consistent filtered snapshot.
3. The backup service encrypts and authenticates the archive.
4. The service verifies the archive before reporting success.
5. Restore checks authentication, version, content, and required disk space.
6. Restore creates a pre-restore checkpoint and shows a preview.
7. A successful restore replaces selected records atomically.
8. Failure leaves the original records usable.

### 18.5 Unofficial adapter workflow

1. The user enables one provider.
2. The app shows the access method and the risk, and requires consent.
3. The user signs in inside OpenUsage.
4. Rust reads the source and runs the checks in Section 5B.
5. The app shows the reading with its mark.

### 18.6 Leaderboard workflow

1. The user joins with GitHub.
2. The app shows the exact upload.
3. The user confirms.
4. Later syncs replace the same rows.
5. Leaving removes the user's data.

---

## 19. Acceptance tests

Every test shall record: build version, platform, fixture or account type, steps, expected result, actual result, and pass/fail status. Synthetic fixtures are permitted in automated tests. Live-provider claims require separate live checks.

| Test ID | Test | Expected result |
|---|---|---|
| AT-01 | Connect three Plus and two Pro fixtures | Five separate profiles and their reported plans appear |
| AT-02 | Run with 10 configured profiles | Dashboard and collector obey the ten-profile limit |
| AT-03 | Add an eleventh active profile | Action fails with a useful capacity message |
| AT-04 | Archive one active profile | One slot becomes available; history remains |
| AT-05 | Log in and cancel a browser or device-code flow | Only the selected profile changes |
| AT-06 | Attempt a read under another profile's home | Access is blocked; prior data remains |
| AT-07 | Use two profiles with one verified identity | Combined totals count the identity once |
| AT-08 | Use email-only provider identity | Verified combined total excludes it and states why |
| AT-09 | Change the bound identity after reconnect | New attribution stops; old history stays unchanged |
| AT-10 | Read 300-minute, 10,080-minute, and custom quotas | Labels and durations are correct |
| AT-11 | Return 25% used with a valid reset time | UI shows 75% derived remaining and exact expected time |
| AT-12 | Move test clock past reset with no new response | No reset-complete claim or alert |
| AT-13 | Send a sparse quota update without credit fields | Prior credit state remains until a full read |
| AT-14 | Return null summary and daily buckets | UI shows Not available, not zero |
| AT-15 | Repeat one daily bucket in later snapshots | Current daily value is replaced, not added |
| AT-16 | Aggregate compatible daily buckets | Daily, ISO-week, and cumulative totals match exact calculations |
| AT-17 | Put account peaks on different dates | Combined daily peak equals max of combined days |
| AT-18 | Remove one middle daily bucket | Coverage warning appears; missing day is not zero |
| AT-19 | Supply incompatible provider date scopes | Incompatible accounts are excluded with reason |
| AT-25 | Cross 80%, then repeat polls | One threshold alert for that confirmed window |
| AT-26 | Move reset time but give no full confirmation | No reset-complete alert; no new alert generation |
| AT-27 | Disable alerts and test quiet hours | Disabled events do not deliver; quiet events queue |
| AT-28 | Restart with a delivered durable event | No duplicate alert |
| AT-29 | Cause auth failure, 429, malformed JSON, and crash | Old readings remain; bounded errors and backoff apply |
| AT-30 | Test expired credentials on one profile | Other profiles continue unchanged |
| AT-31 | Export CSV and JSON | Units, scope, source, times, and redaction are correct |
| AT-32 | Create, tamper with, and restore a backup | Tampered restore fails; valid restore succeeds |
| AT-33 | Lose the OS key-store reference | No database reset; recovery guidance appears |
| AT-34 | Run 10 profiles with 365 days of activity | Local UI meets Section 15 latency targets |
| AT-35 | Run a 2.1-million-row quota history fixture | Dashboard uses current-state path; latency targets pass |
| AT-36 | Run keyboard and screen-reader tests | All main functions and chart alternatives work |
| AT-37 | Inspect protocol requests | Only approved methods are used; no inference or credit use |
| AT-38 | Run Windows, macOS, and Ubuntu builds on real machines | Required OS/webview functionality passes |
| AT-39 | Configure and inspect an approved WSL home | WSL/native data and credentials stay isolated |
| AT-40 | Install, upgrade, and interrupt a migration | Existing valid user data remains recoverable |
| AT-41 | Attempt path traversal and symlink escapes | Unsafe access is blocked |
| AT-42 | Scan DB, logs, backup, and export | No tokens, auth files, or browser secrets appear |
| AT-43 | Test release artifacts and dependencies | Hashes, build provenance, license notices, and SBOM pass; signing state matches release notes |
| AT-44 | Test sleep, wake, offline, and restart | No catch-up burst; old state remains visible |
| AT-45 | Run live account identity contract validation | Stable identity behavior is documented or verified totals stay unavailable |
| AT-46 | Test a provider method missing in pinned version | Capability shows Unsupported; app remains usable |
| AT-47 | Check Work attribution and provider credits | Shared scope and credit types are labeled correctly |

| AT-50 | Test SQLCipher with WAL and power-loss faults | Encrypted DB and transactional history remain recoverable |
| AT-51 | Attempt malformed or reordered encrypted backup chunks | Restore refuses the archive without altering live state |
| AT-52 | Measure idle application RAM and CPU at ten accounts | OU-NFR-017 and OU-NFR-018 pass |
| AT-53 | Backup a database with 2.1 million quota rows | Valid encrypted backup and restore completes; entry limits do not reject SQL rows |
| AT-54 | First accepted window reading has 95% usage | Only the 90% threshold alert appears; lower levels do not repeat |
| AT-55 | Delete all application data with an external WSL home | Managed data is deleted after approval; external home is not deleted without separate approval |
| AT-56 | Give fixtures for each reading status | Status, mark, alerts, and reset confirmation follow Section 3.2, OU-AUD-009, and OU-RESET-005 |
| AT-57 | Change a field that an adapter uses | Not available; no guessed value; the real-account test for that field is void |
| AT-58 | Add a field that an adapter does not use | Reading stays; change is recorded |
| AT-59 | Scan evidence records | No tokens, cookies, emails, prompts, code, or full paths |
| AT-60 | Run a real-account test on Codex and Claude Code | Results and largest difference are recorded |
| AT-61 | Use an Unofficial adapter with no test | It stays Unverified and off by default |
| AT-62 | Trace file and network access during collection | No cookie store or other tool's credentials are read; only declared hosts are contacted |
| AT-63 | Read one account through two adapters | One account with two sources |
| AT-64 | Use two accounts with one email and no shared ID | Both stay, with a Duplicate? mark |
| AT-65 | Run forked, replayed, compacted, and copied log fixtures | Tokens count once; results match the recorded `ccusage` outputs |
| AT-66 | Scan the same logs twice | Totals are identical |
| AT-67 | Delete logs and rescan | Saved totals stay; the difference is saved |
| AT-68 | Change the display time zone | Saved days do not move |
| AT-69 | Use an unknown model | Tokens stay; estimated value is Not available |
| AT-70 | Change the `ccusage` binary | It does not run; the rest of the app works |
| AT-71 | Run `ccusage` with the network blocked; give oversized and malformed output | Results are unchanged, or the scan fails safely with limits applied |
| AT-72 | Trace first launch | No log opens before consent; an unselected tool is never read |
| AT-73 | Plant marker strings and markup in log prompts | Markers appear in no database, export, image, or upload; no markup renders |
| AT-74 | Scan logs with and without an account ID | Unknown account appears; no inferred account |
| AT-75 | Compare provider and local totals | They are never added; a higher local total gives Disputed |
| AT-76 | Sum two tools | May overlap appears; the word total does not |
| AT-77 | Run the CLI without desktop libraries | All CLI commands work; `openusage app` prints the fix |
| AT-78 | Run CLI commands on locked and missing data | Exit codes are correct; null stays null; statusline never prompts |
| AT-79 | Compare JSON output with the UI | Both show the same values, states, and statuses |
| AT-80 | Install with npm on Ubuntu and on a real Windows PC and Mac | The CLI, app, tray, notifications, and key store work |
| AT-81 | Inspect the package | No install scripts; hashes, provenance, and SBOM pass |
| AT-82 | Create and preview a share image | No private fields; no network use; missing days stay distinct |
| AT-83 | Reorder and hide cards, then restart | Layout stays; marks and errors stay visible |
| AT-84 | Install, keep, and undo the Claude Code status line | The user's own status line survives; an absent window is not zero |
| AT-85 | Review every screen against Section 2.3 | No tagline, helper text, or secondary label; each number opens one detail panel |
| AT-86 | Keep the leaderboard off and trace network use | The service is never contacted |
| AT-87 | Compare dry run with a real upload | Identical bytes; only permitted fields |
| AT-88 | Sync twice, then leave | Rows are replaced, not added; leaving removes the data |
| AT-89 | Set a renewal for tomorrow; restart and wake the app | One renewal alert; no repeat |
| AT-90 | Use the tray and Start at login | The tray shows the lowest remaining quota and its mark; Start at login is off by default and works when enabled |
| AT-91 | Launch for the first time with logs and no account | History appears with no sign-in; log-only tools use no profile slot |
| AT-92 | Open quota and token graphs at each range | Quota history covers any range; provider tokens start at daily; gaps are not zero |
| AT-93 | Switch between Provider and This device | Scope changes with no per-number label; ranges, breakdowns, and session quota movement are correct |
| AT-94 | Back up, restore, export, and delete local usage data | Summaries and evidence follow the same rules; the logs of other tools stay untouched |

### 19.1 Release blockers

Any of these failures shall block release:

1. A secret appears in an export, log, notification, frontend payload, or backup.
2. One profile can read or change another profile's credentials without approval.
3. An unverified identity is included in a verified combined total.
4. A quota reset is claimed without accepted provider evidence.
5. A missing measurement becomes a false zero or unlimited value.
6. A transaction, migration, or restore damages the previous valid database.
7. A collector uses an undocumented interface outside the Unofficial level and its safeguards, or uses an inference call or credit-consumption method.
8. A mandatory acceptance test fails on a declared supported platform.
9. A release artifact lacks the required integrity and supply-chain checks.
10. A number in an abnormal state appears without its mark.
11. The app reads browser cookies or the saved credentials of another tool.
12. Prompt, response, or code text enters the database, a share image, an export, or an upload.
13. Usage data leaves the device without the opt-in in Section 14A.
14. Provider token totals and local log totals are added together.
15. A cross-tool sum appears without its May overlap mark.

---

## 20. Repository and engineering standards

```text
openusage/
├── apps/
│   └── desktop/
│       ├── src/                    # React frontend
│       └── src-tauri/              # Tauri entry point and capabilities
├── crates/
│   ├── openusage-codex/           # Official app-server protocol adapter
│   ├── openusage-storage/         # SQLCipher SQLite, migrations, and alerts
│   ├── openusage-import/          # Secure backup
│   ├── openusage-sources/         # Planned: source adapters, checks, and evidence
│   └── openusage-cli/             # Planned: command-line binary
├── services/
│   └── leaderboard/               # Planned for Stage 2
├── docs/
│   ├── README.md                  # Map of all documentation
│   ├── SRS.md
│   ├── implementation-status.md
│   ├── architecture.md
│   ├── data-contracts.md
│   ├── identity-contract.md
│   ├── provider-capabilities.md
│   ├── threat-model.md
│   ├── security-and-privacy.md
│   ├── compatibility-matrix.md
│   ├── performance-report.md
│   ├── provider-evidence.md
│   └── acceptance-traceability.md
├── tests/
│   ├── contract/
│   ├── native/
│   ├── renderer/
│   └── tools/
├── AGENTS.md
├── LICENSE
├── SECURITY.md
├── CONTRIBUTING.md
└── README.md
```

**OU-ENG-001:** The project shall use Rust and JavaScript/TypeScript lockfiles.

**OU-ENG-002:** CI shall run Rust format, lint, tests, and dependency-policy checks.

**OU-ENG-003:** CI shall run TypeScript type checks, frontend tests, and UI accessibility checks.

**OU-ENG-004:** CI shall run database migration and provider-contract tests.

**OU-ENG-005:** Each feature pull request shall cite its requirement IDs and tests.

**OU-ENG-006:** Test-only provider fixtures shall be clearly marked and shall not enter production screens.

**OU-ENG-007:** Release notes shall list tested provider versions, operating systems, and architectures.

**OU-ENG-008:** Release assets shall include licenses, notices, SBOM, checksums, and installation instructions.

**OU-ENG-009:** Users shall be able to build the application without a proprietary OpenUsage service.

**OU-ENG-011:** A pull request that adds an adapter shall include synthetic fixtures, format fingerprints, declared tolerances, allowed hosts, and a terms review.

**OU-ENG-012:** No agent shall push, publish, or call a GitHub write API. Publication stays with the owner.

**OU-ENG-010:** The public product name is OpenUsage. The owner approved this name on 2 November 2026.

---

## 21. Provider-dependent capability policy

OpenUsage cannot require the provider to publish a specific metric or identifier. The implementation must support the following results.

| Capability | When the provider returns valid data | When the provider does not |
|---|---|---|
| Five-hour or weekly quota | Show exact returned values | Not available |
| Other quota windows | Show named or duration-based windows | Not available |
| Credit balance or earned resets | Show provider data | Not available |
| Token lifetime summary | Show provider data | Not available |
| Daily token history | Show returned date buckets | Not available; keep prior accepted history |
| Request or message count | Show only if documented and returned | Not available |
| Separate Work activity | Show only if provider supplies a valid attribution | Shared account activity or Not available |
| Stable account/workspace key | Enable verified aggregation for distinct keys | Exclude from verified aggregate; explain why |
| Unsupported auth mode | Do not send an invalid usage read | Unsupported |
| Provider protocol change | Use tested compatible contract only | Unsupported until tested |
| Claude Code quota | Show documented status-line values during a session | Keep the last reading as Stale; never show zero |
| Account ID in a local log | Group by account under OU-LOG-015 | Show Unknown account |
| Token data for a tool | Show local log estimates | Not available |
| Quota for a tool with no accepted source | Show the reading with its status | Not available |

The app shall not substitute guessed amounts, scraped values, or requests that consume a quota merely to make a screen complete.

---

## 22. Baseline changes

### 22.1 Changes from baseline 2.0

| Item | Baseline 2.0 | Baseline 3.0 |
|---|---|---|
| Purpose | Observe ChatGPT-plan Codex accounts | Show what developers used and have left across many tools |
| Sources | Documented Codex app-server only | Official, Local log, and Unofficial levels |
| Trust | Provider authority | Same, plus a status and evidence for each reading |
| Duplicates | Identity rules for Codex | Same, plus rules for accounts, logs, and tools |
| Token analytics | Provider daily buckets | Same, plus local logs through bundled `ccusage` |
| Interface | Desktop app | Desktop app and CLI from one install |
| Design | Labels on each value | One context line per number; one mark; one detail panel |
| Sharing | Export and backup | Same, plus Wrapped and share images |
| Hosted service | None | Opt-in leaderboard only, from Stage 2 |
| Distribution | Paid signing required | Free npm install; hashes and provenance; paid signing optional |
| Billing | Subscriptions, promotions, payments, receipts, calendar | Subscriptions and one renewal alert |
| Accounts | Profile tags and browser shortcuts | Removed |

**Removed requirements:** OU-ACCT-010, OU-BILL-003, OU-BILL-004, OU-BILL-006, OU-BILL-007, OU-BILL-008, OU-BILL-009, OU-BILL-010, OU-BILL-011, OU-BILL-012, OU-BILL-015, OU-BILL-017, OU-BILL-019, OU-BILL-020, OU-BILL-021, OU-BILL-022, OU-BILL-023, OU-BILL-024, OU-IMPORT-001 through OU-IMPORT-012, OU-ALERT-006, OU-BACKUP-011, OU-REL-006, OU-NUM-010, OU-NUM-011.

**Removed tests:** AT-20, AT-21, AT-22, AT-23, AT-24, AT-48, AT-49.

Baseline 2.0 remains in Git history at commit `5f1c2c7`. No other baseline 2.0 requirement is removed. All retained requirement IDs keep their meaning, except where this baseline amends their text.

### 22.2 Changes from baseline 1.0

| Item | Baseline 1.0 | Baseline 2.0 |
|---|---|---|
| Profile capacity | At least 50 profiles | At most 10 active profiles; archived history retained |
| Concurrency | Three collectors by default | Two concurrent collectors, fixed release cap |
| Database | Plain SQLite baseline | SQLCipher-encrypted SQLite with WAL |
| Data access | SQLite access not finalized | Rust `rusqlite` storage service |
| Current quota read | General snapshot tables | Separate indexed current-state and history tables |
| UI rendering | Tauri 2 platform webviews | Same; explicit CEF/Chromium-bundling exclusion |
| Identity | Verify official account/workspace identity | Explicit confidence states and aggregation gate |
| Reset | Provider-confirmed reset rule | Deterministic conservative evidence criteria |
| Receipt import | Local parsed preview | Explicit 20 MiB limit, 15-second parser time limit, no cloud OCR |
| Performance test | 50 profiles | 10 active profiles and 2.1M history rows |
| Encryption | Encrypted backup | Encrypted live app database and encrypted backup |
| Backup | Authenticated archive | Explicit Argon2id + XChaCha20-Poly1305 scheme |
| Provider coupling | Documented methods | Pinned capability matrix, subprocess checks, release gate |

No other baseline 1.0 feature is removed. A provider limitation shall produce an honest supported state rather than a fabricated reading.

---

## 23. References

1. OpenAI, **Codex App Server**: https://developers.openai.com/codex/app-server/ — account, authentication, quota, credit, and activity methods. Checked 17 October 2026.
2. OpenAI, **Codex source**: https://github.com/openai/codex — implementation and generated protocol contracts. Version pinning remains mandatory.
3. OpenAI, **Sign in with ChatGPT for open-source tools**: https://developers.openai.com/siwc/token-sharing-open-source — authorization scope and restrictions.
4. Tauri, **Process Model**: https://v2.tauri.app/concept/process-model/ — native core and per-platform webview engines.
5. SQLite, **Write-Ahead Logging**: https://sqlite.org/wal.html — WAL coordination and single-writer behavior.
6. Zetetic, **SQLCipher API**: https://www.zetetic.net/sqlcipher/sqlcipher-api/ — encrypted SQLite interface and key behavior.
7. ASD Simplified Technical English Maintenance Group, **ASD-STE100**, Issue 9, 15 January 2025: https://asd-ste100.org/ — writing principles; this document makes no formal conformity claim.
8. W3C, **WCAG 2.2**: https://www.w3.org/TR/WCAG22/ — accessibility target.
9. ISO/IEC/IEEE 29148: requirements-engineering organization guide. This document does not claim certification.
10. Anthropic, **Customize your status line**: https://code.claude.com/docs/en/statusline — documented `rate_limits` fields. Checked 2 November 2026.
11. ccusage: https://github.com/ccusage/ccusage — MIT-licensed local log analysis. Version 20.0.47 was current on 2 November 2026. The release pin remains mandatory.
12. tokenmaxxing: https://tokenmaxxing.sh — studied for dry-run preview and aggregate-only upload. Checked 2 November 2026.
13. tokscale: https://github.com/junhoyeo/tokscale — studied for day-total and pricing rules. Checked 2 November 2026.

---

## 24. Final release rule

**Each release stage in Section 0.4 shall implement and test every mandatory requirement in that stage.** The application shall preserve provider truth, account isolation, local privacy, and data recovery.

If the provider does not expose a required measurement, OpenUsage shall report the documented Not available or Unsupported state. It shall not guess, scrape pages, or send workload requests to fill that gap.

**This is a final SRS baseline, not a statement that the application has passed release tests.**
