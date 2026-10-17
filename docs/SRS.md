# OpenUsage — Software Requirements Specification

**Document ID:** OU-SRS-001  
**Baseline:** 2.0  
**Date:** 17 October 2026  
**Status:** Final product and engineering requirements baseline; release is subject to validation  
**Product:** OpenUsage  
**License:** Apache License 2.0  
**Replaces:** OU-SRS-001, baseline 1.0  
**Supported workload:** 10 active account profiles  
**Document style:** Short, direct technical English based on ASD-STE100, Issue 9. This document has **not** received a formal STE vocabulary or conformity audit.

> **Product statement.** OpenUsage is an open-source desktop application. It observes authorized ChatGPT-plan accounts through documented Codex interfaces. It displays provider-reported quota, credit, and token-activity data. It keeps local billing records and history. It sends local alerts. It does not invent usage data, read browser sessions, or bypass provider limits.

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

1. Official authenticated provider responses are the authority for provider identity, quota, credits, and token activity.
2. User-confirmed records are the authority for local billing data.
3. Local calculations are the authority only for derived display values.
4. An absent provider value is not zero. An absent limit is not proof of unlimited access.
5. A clock reaching the expected reset time is not proof that the provider reset a quota.
6. A subscription record does not prove that the provider received a payment.
7. A test fixture does not prove that a capability works with a live provider account.
8. This document does not grant access to any provider field or interface.

### 0.3 Scope control

Baseline 2.0 changes the supported workload from at least 50 profiles to **10 active profiles**. It keeps the complete feature scope of baseline 1.0. Archived profiles do not count toward the active limit. A future change to the limit requires a new SRS baseline and new performance tests.

A release is not complete because a screen exists. A release is complete only when its mandatory behavior passes the acceptance tests.

---

## 1. Purpose and scope

### 1.1 Objectives

- Observe up to 10 independently configured account profiles.
- Support a test set with three Plus accounts and two Pro accounts.
- Display each quota bucket only when the official interface supplies it.
- Display quota use, derived remaining quota, expected reset time, and credits when available.
- Display token statistics and daily token activity when available.
- Display per-account and combined activity heatmaps.
- Keep local subscription, promotion, and payment records.
- Import local billing evidence after user review.
- Send local quota, connection, renewal, and promotion alerts.
- Support Windows, macOS, and Linux desktop installations.
- Keep prior accepted data available without network access.

### 1.2 Included features

1. Account profile registration, authentication, and identity checking.
2. Separate Codex environments for native profiles and approved Windows Subsystem for Linux (WSL) profiles.
3. Official Codex app-server reads and supported notifications.
4. Quota history, token activity, statistics, and account comparison.
5. Subscription and payment records, promotions, reminders, and receipt import.
6. System tray, local notification service, CSV/JSON export, encrypted backup, and restore.
7. Local settings, privacy controls, and complete local data deletion.
8. Browser-profile shortcuts for account navigation.

### 1.3 Excluded features

- ChatGPT web message counts, conversation history, page scraping, and cookie access.
- OpenAI Platform API-key usage, organization billing, and API credit purchases.
- Work-only usage attribution when the provider supplies only shared account data.
- A hosted service, cloud sync, remote telemetry, or a shared OpenUsage account.
- Automatic account rotation, quota evasion, or workload submission to test a limit.
- Spending earned reset credits or changing provider quota settings.
- Automated purchases, cancellation of subscriptions, or bank-mandate changes.
- Unattended monitoring after the OpenUsage application process stops.
- A built-in general web browser or a bundled Chromium rendering engine.

### 1.4 Product boundary

OpenUsage is an observer and a local record manager. It does not control the service limits of an OpenAI account. It shall use only interfaces that the provider documents and authorizes for the selected account mode.

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
| Usage source | Documented Codex app-server JSON-RPC over local stdio | No undocumented requests |
| Codex login | Provider-managed ChatGPT browser and device-code login | Avoid a custom token service |
| Account collection | Rust process supervisor; maximum two active collections | Controlled memory and request load |
| Collection cycle | 300 seconds per eligible profile, subject to backoff | Cautious provider access |
| Charts | Accessible SVG/HTML heatmaps and time-series components | Clear data labels and keyboard access |
| Notifications | OS notification interfaces through approved Tauri commands | Local alerts |
| Backup | Locally encrypted, authenticated, versioned archive | Safe portable history |
| Update distribution | Signed release artifacts; updater only after signed-update tests | Supply-chain safety |

**Decision boundary:** Do not add PostgreSQL, Redis, DuckDB, Electron, CEF, FastAPI, a hosted backend, or application telemetry without a new approved architecture decision.

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

---

## 3. Definitions and data states

| Term | Exact meaning |
|---|---|
| Provider account | An authenticated ChatGPT-plan account as reported through an official interface. |
| Profile | A local UUID, settings, and one isolated Codex home. |
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
| Confirmed payment | A local payment record that the user explicitly marked as paid. |
| Earned reset credit | A provider credit for a possible quota reset. OpenUsage only observes it. |

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

---

## 4. Profile, identity, and authentication

### 4.1 Profile records

**OU-ACCT-001:** The user shall be able to create, edit, archive, and remove a profile.

**OU-ACCT-002:** The app shall support 10 active profiles. This replaces the 50-profile requirement in baseline 1.0.

**OU-ACCT-003:** Each profile shall have an immutable UUID, editable label, environment type, and optional tags.

**OU-ACCT-004:** The UI shall show provider-returned email and plan when available.

**OU-ACCT-005:** The app shall support documented browser and device-code ChatGPT login flows.

**OU-ACCT-006:** Each profile shall use a different managed `CODEX_HOME` directory.

**OU-ACCT-007:** The app shall check account identity after login, reconnect, profile switch, and detected credential change.

**OU-ACCT-008:** Profiles with the same verified identity key shall be marked as duplicates.

**OU-ACCT-009:** The user shall manually select the profile to inspect or open.

**OU-ACCT-010:** The app shall open a configured external browser profile without reading cookies or passwords.

**OU-ACCT-011:** Windows users shall be able to configure an independent WSL profile after explicit approval.

**OU-ACCT-012:** Each profile shall offer Connect, Reconnect, Disconnect, Refresh Now, and Open Browser actions.

**OU-ACCT-013:** Removing a profile shall offer Archive History and Delete History as separate choices.

**OU-ACCT-014:** Disconnecting shall not delete history or cancel a provider subscription.

**OU-ACCT-015:** Each profile shall show Connected, Authentication required, Unauthorized, Offline, Unsupported, or Error.

**OU-ACCT-016:** Connecting a new profile shall create a new empty managed home.

**OU-ACCT-017:** OpenUsage shall not modify an existing external Codex home without explicit user approval.

**OU-ACCT-018:** The app shall not copy tokens or auth files between profile homes.

**OU-ACCT-019:** A profile cannot start normal monitoring before login completion and account-state validation.

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

**OU-ID-011:** The app shall not read provider token files or undocumented endpoints to obtain an identity key.

**OU-ID-012:** A release claiming verified multi-account aggregation shall pass a live identity-contract test.

**Release gate ID-G1:** Do not ship verified multi-account totals unless a supported official identity contract proves distinct account/workspace attribution. When this is not possible, the UI must provide the documented Not available state. The feature remains in the product.

### 4.3 Login and logout

**OU-AUTH-001:** Login shall use `account/login/start` with an official supported ChatGPT login type.

**OU-AUTH-002:** The app shall await a successful `account/login/completed` event before treating login as complete.

**OU-AUTH-003:** The app shall match login-completion notifications to the correct login request.

**OU-AUTH-004:** Failed, canceled, and expired login attempts shall not change other profiles.

**OU-AUTH-005:** The app shall offer a Cancel Login control while login is pending.

**OU-AUTH-006:** Disconnect shall use documented `account/logout` only after the user confirms the action.

**OU-AUTH-007:** Only the selected profile's Codex process and home shall receive its authentication operations.

**OU-AUTH-008:** API-key-only and unsupported auth modes shall not be treated as ChatGPT-plan usage modes.

**OU-AUTH-009:** The app shall not implement experimental external-token login in baseline 2.0.

**OU-AUTH-010:** The provider-managed login system shall own token refresh and token persistence.

---

## 5. Codex app-server integration

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

## 6. Quota, reset, and credit data

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

**OU-QUOTA-010:** The app shall label `100 - usedPercent` as Derived remaining percentage.

**OU-QUOTA-011:** The app shall reject invalid, non-finite, or out-of-range percentages from the current display.

**OU-QUOTA-012:** The app shall preserve redacted validation evidence for an invalid value.

**OU-QUOTA-013:** The app shall show a countdown only when `resetsAt` exists and is valid.

**OU-QUOTA-014:** The UI shall show reset timestamps with the selected time-zone name or UTC offset.

**OU-QUOTA-015:** After the reset time passes, the app shall show “Reset due; awaiting confirmation” until a new accepted reading confirms a new window.

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

### 6.3 Provider credits

**OU-CREDIT-001:** The UI shall display provider-returned workspace credit details only when supplied.

**OU-CREDIT-002:** The UI shall label provider credits separately from payments and OpenAI Platform API billing.

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

**OU-ACT-011:** The app shall not invent separate Work, Codex, model, request, or message counts.

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

## 9. Subscriptions, promotions, and payments

### 9.1 Data entry

**OU-BILL-001:** The user shall be able to create, edit, archive, and delete subscription records.

**OU-BILL-002:** Each subscription shall support provider, plan, tier, currency, price, interval, start, renewal, end, and auto-renew state.

**OU-BILL-003:** Promotion records shall store claim-by, activation, and benefit-end dates separately.

**OU-BILL-004:** A subscription shall support multiple discount or trial periods.

**OU-BILL-005:** Manually entered billing data shall have source `user_confirmed`.

**OU-BILL-006:** The app shall import a user-selected PDF, text, or EML billing document into a preview.

**OU-BILL-007:** The app shall parse receipt files locally.

**OU-BILL-008:** Parsed fields shall not enter confirmed billing tables before user approval.

**OU-BILL-009:** Imported dates with no time component shall remain date-only values.

**OU-BILL-010:** The app shall separately store paid, due, discount, and authorization amounts.

**OU-BILL-011:** A bank authorization or mandate amount shall not become a paid transaction.

**OU-BILL-012:** The app shall show paid transactions and projected recurring commitments separately.

**OU-BILL-013:** Currency totals shall remain separate. The app shall not perform automatic exchange conversion.

**OU-BILL-014:** The app shall show whether each deadline is user-confirmed, imported-and-confirmed, or unknown.

**OU-BILL-015:** The app shall not infer auto-renew from a bank mandate.

**OU-BILL-016:** The app shall include subscription calendar, paid-spend views, projection views, trial history, and upcoming deadlines.

**OU-BILL-017:** Deleting a local receipt shall not cancel a remote subscription.

**OU-BILL-018:** Financial displays shall show currency codes and exact decimal amounts.

**OU-BILL-019:** A payment shall have an explicit `paid`, `due`, `refunded`, or `void` state.

**OU-BILL-020:** Paid-spend reports shall count only confirmed paid transactions, less separately confirmed refunds.

**OU-BILL-021:** Paid-spend reports shall support monthly and calendar-year groupings by payment date.

**OU-BILL-022:** Projected commitment reports shall support monthly and calendar-year groupings by scheduled date.

**OU-BILL-023:** The subscription calendar shall show a cancellation deadline when the user enters one.

**OU-BILL-024:** A promotional claim date shall never substitute for a benefit-end or cancellation date.

### 9.2 Receipt import protection

**OU-IMPORT-001:** The importer shall reject files larger than 20 MiB.

**OU-IMPORT-002:** The importer shall check file contents, not only the extension.

**OU-IMPORT-003:** The importer shall reject encrypted or malformed files that it cannot parse safely.

**OU-IMPORT-004:** The importer shall never execute scripts, macros, PDF actions, or attachments.

**OU-IMPORT-005:** The importer shall not fetch network resources referenced by a document.

**OU-IMPORT-006:** The importer shall limit parsing time to 15 seconds per file.

**OU-IMPORT-007:** The app shall not use a cloud OCR or document-analysis service.

**OU-IMPORT-008:** Scanned documents without local text shall show Manual entry required.

**OU-IMPORT-009:** The import preview shall show field evidence and source filename.

**OU-IMPORT-010:** Raw receipt bytes shall not be retained by default.

**OU-IMPORT-011:** A user can choose to retain an encrypted local copy of a receipt.

**OU-IMPORT-012:** Deleting retained evidence shall not silently delete a confirmed payment record.

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

### 10.2 Notification events

**OU-ALERT-001:** The app shall support usage alerts at 80%, 90%, and 100% used.

**OU-ALERT-002:** A threshold event shall fire only when a fresh accepted reading first crosses its level.

**OU-ALERT-003:** Repeated reads in the same confirmed window shall not repeat the same threshold event.

**OU-ALERT-004:** The app shall send a reset-completed alert only after reset confirmation under Section 6.2.

**OU-ALERT-005:** Default subscription expiry and renewal reminders shall be seven days and one day before the deadline.

**OU-ALERT-006:** Default promotion claim reminders shall be seven days and one day before the deadline.

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

### 10.3 Alert identity and quiet hours

Quota threshold event keys shall include: local profile UUID, verified identity key if available, limit ID, window role, duration, confirmed window generation, and threshold. A change in expected reset time shall not create a new window generation without reset confirmation.

For a local deadline, the event key shall include: record UUID, deadline kind, deadline date/time, and reminder offset. Editing a deadline shall create a new key only when the actual deadline changes.

The default quiet period shall be **22:00–08:00** in the selected local time zone. Notifications during this period shall be queued. The app shall deliver queued reminders at 08:00 if they remain relevant. Authentication and collector-failure alerts shall also respect quiet hours unless the user changes this policy.

---

## 11. User interface and accessibility

### 11.1 Required screens

**OU-UI-001:** The app shall provide Dashboard, Accounts, Account Detail, Compare, Token Activity, Subscriptions, Calendar, Notifications, and Settings.

**OU-UI-002:** The dashboard shall show active account count, plan breakdown, quota summary, data freshness, deadlines, activity statistics, and combined heatmap.

**OU-UI-003:** Account Detail shall show identity state, plan, quota buckets, reset credits, activity, individual heatmap, billing, and errors.

**OU-UI-004:** Compare shall align quotas only when limit ID and window duration are compatible.

**OU-UI-005:** The UI shall support search by profile label, returned email, and tag.

**OU-UI-006:** The UI shall support filters by plan, connection, quota state, deadline, and reporting period.

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

### 11.2 Accessibility target

**OU-ACC-001:** Core UI behavior shall meet WCAG 2.2 AA intent.

**OU-ACC-002:** The release tests shall cover keyboard use, color contrast, zoom, screen-reader labels, and chart alternatives.

**OU-ACC-003:** The application shall not rely only on color to show data state.

**OU-ACC-004:** A provider error shall not move keyboard focus without an announced reason.

**OU-ACC-005:** Heatmap date details shall be available without hover.

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

**OU-DATA-014:** The app shall never store access tokens, refresh tokens, browser cookies, or Codex auth-file contents in SQLite.

**OU-DATA-015:** The app shall not sync the live database to cloud folders.

**OU-DATA-016:** The app shall provide complete local export and explicit local data deletion.

**OU-DATA-017:** Deleting local data shall not claim to delete OpenAI-side records.

**OU-DATA-018:** Database reads used by the dashboard shall not scan full historical snapshots when a current-state index is available.

**OU-DATA-019:** The storage service shall set `PRAGMA synchronous=FULL` for the primary database.

**OU-DATA-020:** The storage service shall set `PRAGMA busy_timeout=5000` on its database connections.

**OU-DATA-021:** SQLite writes shall use prepared statements and bounded, short transactions.

**OU-DATA-022:** A restore, backup, or export shall use a consistent SQLite snapshot.

**OU-DATA-023:** The app shall never copy only the main database file while active WAL content is required.

**OU-DATA-024:** When the app is offline, all displayed provider readings shall be labeled Stale, regardless of their age.

**OU-DATA-025:** Delete All Local Data shall remove managed OpenUsage records and managed profile homes only after explicit confirmation.

**OU-DATA-026:** Delete All Local Data shall not delete an external or WSL Codex home without separate explicit approval.

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

**OU-BACKUP-011:** Backups shall not contain raw retained receipts unless the user selects Include local evidence.

**OU-BACKUP-012:** The app shall verify a restored database before marking restore complete.

**OU-BACKUP-013:** Archive version 1 shall use authenticated chunks of at most 1 MiB each.

**OU-BACKUP-014:** Each chunk shall have a unique nonce and an authenticated chunk index and length.

**OU-BACKUP-015:** The archive shall reject missing, repeated, reordered, and truncated chunks.

**OU-BACKUP-016:** The restore parser shall enforce archive entry-count and total-size limits before extraction.

**OU-BACKUP-017:** The archive shall enforce a maximum restored size of 5 GiB and 10,000 archive-file entries. SQLite row counts shall not count as archive-file entries.

**OU-BACKUP-018:** The app shall refuse backup or restore when available free space is insufficient for safe completion.

---

## 13. Required database schema

All tables shall have explicit primary keys. Mutable business records shall store creation and update times in UTC. Foreign keys shall be enabled. Money shall be stored in integer minor units. All stored money amounts shall be nonnegative signed 64-bit integers. Payment and refund fields shall be separate. Token totals shall be stored as decimal text with checked parsing.

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
| `promotions` | UUID; subscription UUID; distinct claim, activation, benefit end; terms; evidence |
| `discount_periods` | UUID; subscription UUID; start/end; discount amount or rule; evidence |
| `payment_records` | UUID; subscription UUID; currency; paid/due/discount/refund amounts; state; date; confirmation |
| `receipt_imports` | UUID; filename; hash; parser state; confirmation state; optional encrypted evidence reference |
| `alert_policies` | UUID; event type; profile override nullable; threshold/offset; enabled; quiet-hours data |
| `alert_events` | UUID; unique event key; evidence; delivery state; scheduled and delivered times |
| `collection_runs` | UUID; profile UUID; method; start/end; outcome; redacted error code |
| `settings` | Schema version; time zone; locale; theme; monitoring state; retention policy |

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

**OU-SEC-001:** The product shall have no OpenUsage-hosted account service, telemetry endpoint, or hidden network call.

**OU-SEC-002:** Only official Codex provider operations needed for authorized login and collection may contact OpenAI.

**OU-SEC-003:** The frontend shall not receive tokens, auth-file contents, browser cookies, or the database encryption key.

**OU-SEC-004:** Provider credentials shall remain in the supported provider-managed credential environment.

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

### 14.1 Threat-model minimum

The release threat model shall cover: malicious local files; compromised browser content; malicious path links; poisoned JSON-RPC data; substituted Codex binaries; stolen backup archives; lost encryption keys; account switching; duplicate identities; corrupted updates; log leakage; and unauthorized filesystem access.

The release shall not claim protection against an attacker with full control of the user's unlocked operating-system account. It shall still use reasonable OS access controls.

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

| OU-NFR-017 | At most 450 MiB resident memory for OpenUsage app processes, excluding Codex children, after idle stabilization | Peak sampled across five idle minutes with ten cached profiles |
| OU-NFR-018 | Mean idle CPU below 5% of one logical core, excluding collection periods | Five-minute trace after warm startup |

**Performance measurement rule:** Run the benchmark on each release platform. Report the machine, operating-system release, app commit, schema version, and observed distribution. A test passes only if all specified limits pass on each reference platform.

### 15.2 Reliability rules

**OU-REL-001:** Startup shall load cached local records before starting provider collection.

**OU-REL-002:** The UI shall not block on a provider network response.

**OU-REL-003:** An interrupted collection shall leave its previously accepted measurement valid.

**OU-REL-004:** A failed database write shall not partially publish a new quota state.

**OU-REL-005:** A full disk shall produce an actionable error without deleting accepted records.

**OU-REL-006:** A corrupt import shall not alter confirmed billing records.

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

**OU-PLAT-001:** Windows packaging shall handle missing or damaged WebView2 installations with a documented recovery action.

**OU-PLAT-002:** Linux packaging shall list exact WebKitGTK runtime dependencies.

**OU-PLAT-003:** macOS packages shall use the platform's supported signing and notarization process for distributed builds.

**OU-PLAT-004:** Windows packages shall be signed before general public distribution.

**OU-PLAT-005:** Linux releases shall publish package hashes and installation dependencies.

**OU-PLAT-006:** Release binaries shall use a reproducible, locked dependency build process where supported.

**OU-PLAT-007:** The release shall include installer, upgrade, uninstall, and local-data deletion instructions.

**OU-PLAT-008:** The app shall not replace an existing user Codex installation without approval.

**OU-PLAT-009:** A built-in update check shall be opt-in and shall only read signed release metadata from a documented source.

**OU-PLAT-010:** An application update shall not add telemetry or alter the privacy boundary.

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

type DataSource = "codex_app_server" | "user_confirmed" | "local_receipt_import";

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
  source: "codex_app_server";
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

type BillingRecord = {
  subscriptionId: string;
  source: "user_confirmed" | "local_receipt_import";
  confirmedAtUtc: string;
  currency: string;                    // ISO 4217
  amountMinor: string | null;          // signed exact integer
  paymentStatus: "paid" | "due" | "refunded" | "void";
};
```

**OU-NUM-001:** A valid quota percentage is finite and within `0` through `100`, inclusive.

**OU-NUM-002:** Derived remaining percentage is `100 - usedPercent` for a valid value only.

**OU-NUM-003:** A token count shall be a nonnegative whole number representable by the supported Rust exact type.

**OU-NUM-004:** Token totals shall cross the frontend boundary as decimal strings.

**OU-NUM-005:** A provider date shall be a valid Gregorian calendar date in `YYYY-MM-DD` format.

**OU-NUM-006:** Reset times shall be interpreted as Unix seconds and stored as UTC timestamps.

**OU-NUM-007:** Monetary arithmetic shall use checked integer operations and the currency's stated scale.

**OU-NUM-010:** A money field shall not use a negative minor-unit value to encode a refund.

**OU-NUM-011:** Payment and refund values shall keep separate explicit state and transaction dates.

**OU-NUM-008:** No automatic rounding shall change the stored provider token or quota measurement.

**OU-NUM-009:** Display rounding shall not change the retained original value.

---

## 18. State machines and user workflows

### 18.1 Collection state machine

1. **Load:** Unlock the database. Load cached state. Show prior data with its age.
2. **Prepare:** Select an eligible profile. Confirm its approved home and executable.
3. **Authenticate:** Read supported account state. Apply the identity-confidence rules.
4. **Collect:** Read supported quota and activity methods. Validate each response independently.
5. **Persist:** Write accepted current state and history in one transaction.
6. **Evaluate:** Check thresholds, reset evidence, deadline policies, and duplicate event keys.
7. **Wait:** Schedule the next 300-second cycle, subject to provider backoff.
8. **Recover:** Keep old data on failure. Show the error. Do not replay missed cycles.

### 18.2 Identity conflict

1. Stop remote attribution for the affected profile.
2. Keep its old history unchanged.
3. Show the previous and current non-secret identity evidence.
4. Let the user reconnect, create a new profile, or keep the profile disconnected.
5. Resume collection only when the new binding is valid under Section 4.

### 18.3 Local receipt workflow

1. The user selects a local file.
2. Rust validates type, path, size, and content.
3. The local parser extracts candidate fields without network access.
4. The UI shows candidates and their evidence.
5. The user edits and confirms each record.
6. Rust stores the confirmed record in one transaction.
7. The app keeps or deletes the raw evidence according to user choice.

### 18.4 Backup workflow

1. The user selects data and enters a backup password.
2. The storage service creates a consistent filtered snapshot.
3. The backup service encrypts and authenticates the archive.
4. The service verifies the archive before reporting success.
5. Restore checks authentication, version, content, and required disk space.
6. Restore creates a pre-restore checkpoint and shows a preview.
7. A successful restore replaces selected records atomically.
8. Failure leaves the original records usable.

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
| AT-20 | Enter claim, benefit-end, and renewal dates | Three distinct calendar events appear |
| AT-21 | Import a local receipt | Parsed fields are previewed and need confirmation |
| AT-22 | Import a scanned or malicious document | No scripts run; unsupported scan requires manual entry |
| AT-23 | Import a bank mandate amount | It is not recorded as a paid amount |
| AT-24 | Report confirmed transactions in two currencies | Totals remain separate; projections remain separate |
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
| AT-38 | Run Windows, macOS, and Ubuntu builds | Required OS/webview functionality passes |
| AT-39 | Configure and inspect an approved WSL home | WSL/native data and credentials stay isolated |
| AT-40 | Install, upgrade, and interrupt a migration | Existing valid user data remains recoverable |
| AT-41 | Attempt path traversal and symlink escapes | Unsafe access is blocked |
| AT-42 | Scan DB, logs, backup, and export | No tokens, auth files, or browser secrets appear |
| AT-43 | Test signed release artifacts and dependencies | Signing, hashes, license notices, and SBOM pass |
| AT-44 | Test sleep, wake, offline, and restart | No catch-up burst; old state remains visible |
| AT-45 | Run live account identity contract validation | Stable identity behavior is documented or verified totals stay unavailable |
| AT-46 | Test a provider method missing in pinned version | Capability shows Unsupported; app remains usable |
| AT-47 | Check Work attribution and provider credits | Shared scope and credit types are labeled correctly |
| AT-48 | Test paid refunds and date-only terms | Money and calendar values remain correct |

| AT-49 | Test cancellation deadlines and monthly/yearly reports | Date and currency grouping are correct |
| AT-50 | Test SQLCipher with WAL and power-loss faults | Encrypted DB and transactional history remain recoverable |
| AT-51 | Attempt malformed or reordered encrypted backup chunks | Restore refuses the archive without altering live state |
| AT-52 | Measure idle application RAM and CPU at ten accounts | OU-NFR-017 and OU-NFR-018 pass |
| AT-53 | Backup a database with 2.1 million quota rows | Valid encrypted backup and restore completes; entry limits do not reject SQL rows |
| AT-54 | First accepted window reading has 95% usage | Only the 90% threshold alert appears; lower levels do not repeat |
| AT-55 | Delete all application data with an external WSL home | Managed data is deleted after approval; external home is not deleted without separate approval |

### 19.1 Release blockers

Any of these failures shall block release:

1. A secret appears in an export, log, notification, frontend payload, or backup.
2. One profile can read or change another profile's credentials without approval.
3. An unverified identity is included in a verified combined total.
4. A quota reset is claimed without accepted provider evidence.
5. A missing measurement becomes a false zero or unlimited value.
6. A transaction, migration, or restore damages the previous valid database.
7. The collector uses an undocumented endpoint, inference call, or credit-consumption method.
8. A mandatory acceptance test fails on a declared supported platform.
9. A release artifact lacks the required integrity and supply-chain checks.

---

## 20. Repository and engineering standards

```text
openusage/
├── apps/
│   └── desktop/
│       ├── src/                    # React frontend
│       └── src-tauri/              # Tauri entry point and capabilities
├── crates/
│   ├── openusage-core/            # Types, validation, aggregation
│   ├── openusage-codex/           # Official app-server protocol adapter
│   ├── openusage-storage/         # SQLCipher SQLite and migrations
│   ├── openusage-alerts/          # Scheduler and durable alerts
│   └── openusage-import/          # Local receipts and secure backup
├── docs/
│   ├── SRS.md
│   ├── architecture.md
│   ├── data-contracts.md
│   ├── identity-contract.md
│   ├── provider-capabilities.md
│   ├── threat-model.md
│   ├── security-and-privacy.md
│   ├── compatibility-matrix.md
│   ├── performance-report.md
│   └── acceptance-traceability.md
├── tests/
│   ├── fixtures/
│   ├── contract/
│   ├── integration/
│   ├── security/
│   └── accessibility/
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

**OU-ENG-010:** The public product name shall receive a separate trademark/name conflict review.

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

The app shall not substitute guessed amounts, scraped values, or requests that consume a quota merely to make a screen complete.

---

## 22. Changes from baseline 1.0

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

---

## 24. Final release rule

**The complete OpenUsage release shall implement and test every mandatory requirement in this baseline.** The application shall preserve provider truth, account isolation, local privacy, and data recovery.

If the provider does not expose a required measurement, OpenUsage shall report the documented Not available or Unsupported state. It shall not guess, scrape, or send workload requests to fill that gap.

**This is a final SRS baseline, not a statement that the application has passed release tests.**
