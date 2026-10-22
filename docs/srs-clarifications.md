> Public source copy: raw acceptance evidence remains private. Historical test statements keep their original scope and do not certify this source snapshot.

# SRS clarification record

## SC-001: reset confirmation with rapid new-window use

Status: proposed, not approved. No normative text or runtime reset rule changed.

SRS section 6.2 condition 4 requires a lower usage percentage before reset confirmation. Master Build Prompt D1 identifies a missed-reset case: new-window usage can already equal or exceed the prior reading. A missing percentage drop therefore cannot prove that no reset occurred.

The reviewed [Codex window schema](https://github.com/openai/codex/blob/rust-v0.163.0/codex-rs/app-server-protocol/schema/typescript/v2/RateLimitWindow.ts) contains no explicit generation identifier. The [quota response](https://github.com/openai/codex/blob/rust-v0.163.0/codex-rs/app-server-protocol/schema/typescript/v2/GetAccountRateLimitsResponse.ts) does not establish a replacement reset-proof contract.

Proposed narrow addition: distinguish an overdue expected reset from a reset whose completion evidence is unavailable. Continue bounded full-read reconciliation and preserve the latest measurement. Neither state permits a reset-completed alert or a new durable alert generation. Keep the five existing confirmation conditions until an approved provider-backed rule replaces them.

Implementation and migration remain not started. The future Rust state machine must preserve identity and bucket compatibility, stale-reading rejection, and alert deduplication across restarts. Acceptance must cover rapid new-window use, later confirmation, unknown reset times, sparse updates, clock changes, and repeated reads.

This proposal does not authorize treating elapsed time, a moved timestamp, or changed percentage alone as confirmation.


## SC-002: interruption after restore COMMIT and before acknowledgment

Status: implementation interpretation recorded for acceptance review. No normative text changed. Power-loss and native interruption acceptance remain open.

OU-BACKUP-010 requires the previous valid database to survive a failed or interrupted restore. OU-NFR-007 requires transactional writes. Before COMMIT, the previous live records survive. After COMMIT but before acknowledgment, complete restored records survive, while the encrypted pre-restore checkpoint preserves the previous records. A lost reply does not prove rollback.

The owned-process tests execute the production transaction and terminate their child before COMMIT and after COMMIT without acknowledgment. Both cases retain the original encrypted checkpoint. The user can prepare a new preview after restart rather than retry a consumed token. This interpretation does not permit partial records, deletion of the checkpoint, or automatic confirmation. See [the review](reviews/2026-10-18-restore-process-crash.md).


## SC-003: transient device-code challenge display

Status: the user approved the narrow interpretation on October 18, 2026. No normative SRS text changed.

OU-ACCT-005 requires device-code login, while section 19.1(1) broadly prohibits secret frontend payloads. The approved scope permits only the selected profile's validated verification URL and human user code in transient typed IPC after explicit consent. The code remains a sensitive challenge. OAuth tokens, provider login identifiers, device authorization secrets, credentials, and database keys stay in Rust or the provider.

Codes must stay out of logs, notifications, toasts, exports, backups, application persistence, and retained real test evidence. Terminal states and approval invalidation clear the challenge. Clipboard retention remains under operating-system control. See the [exact approved interpretation](proposals/device-code-display-interpretation.md). Synthetic native checks passed. Real OAuth remains unexecuted.

October 21 implementation update: the user's explicit per-account Sign in action supplies selected-profile consent. A separate Get sign-in code confirmation is no longer required. The named pending dialog retains Cancel; the fixed official verification page opens automatically. Production anonymous challenge/cancellation passed without retaining a code. Successful real OAuth remains unverified. See the evidence (private evidence omitted).

## SC-004: trusted companion preparation through Add

Status: user-directed implementation interpretation, October 21, 2026. The normative SRS stays unchanged.

The user requires normal onboarding without manual executable paths or technical approval panels. Add or Connect now authorizes preparation of the exact reviewed official companion. Rust retains the source pin, held-file checks, isolated homes, and process controls required by OU-PROTO-002/004/005. Sign-in retains explicit consent for the selected account. A present invalid companion fails closed. No credential import or runtime download is permitted.

Technical review remains available in Settings for troubleshooting. Test fixtures still require the explicit test-build approval path and cannot pass normal production preparation. See the [source review and decision](reviews/2026-10-21-onboarding-sources.md). This change does not grant another profile access to an active sign-in or saved reading. Scoped native Linux qualification passed. Real OAuth and release-platform acceptance remain open.

## SC-005: browser callback isolation remains unresolved

Status: confirmed implementation restriction, October 21, 2026. No normative requirement changed or waived.

OU-ACCT-005 requires browser login. OU-AUTH-007 restricts authentication operations to the selected profile. The pinned provider can cancel another listener on its fixed callback port. A probe reproduced that behavior against owned listeners. No supported app-server port override was found. A precheck leaves a race and does not establish isolation. The browser draft stays outside the working build while this conflict remains unresolved. Official device-code login remains available. See [the source review and executed result](reviews/2026-10-21-official-authentication.md).

## SC-006: initial account snapshot before measurements

Status: source-backed implementation correction, October 21, 2026. No normative requirement changed.

The pinned official provider sends an initial routing snapshot through `account/updated`. It is not by itself evidence of an account switch. Rust accepts one validated matching snapshot before the measurement baseline, supported by a full account response and routing context. A deferred snapshot requires another full account check before measurement requests. The snapshot never establishes identity, freshness, or reset evidence. Mismatches, duplicate snapshots, malformed envelopes, and all later account-update notifications still invalidate the collection. Message and read deadlines remain bounded. See the source and executed evidence (private evidence omitted).

## SC-007: three primary pages with contextual account management

Status: explicit user-directed navigation correction, October 21, 2026. The normative SRS stays unchanged.

OU-UI-001 lists Accounts and other functional views. The user requires only Overview, Usage and Settings in primary navigation. Account selection, sign-in, management and details remain accessible through Overview, Usage and contextual dialogs. This changes navigation placement, not the required capabilities. Unimplemented Compare, Subscriptions, Calendar and Notifications functions remain acceptance gaps; removing their navigation entries does not waive them.

Show each returned quota window. Do not infer a five-hour or weekly limit from the plan name. Refresh must preserve the current Overview route. See native evidence (private evidence omitted).


## SC-008: consumed lower thresholds and desktop delivery

Status: implementation interpretation, October 22, 2026. No normative text changed.

OU-ALERT-018 consumes lower satisfied thresholds after the first reading selects the highest threshold. The saved Covered state means that no separate OS notification was submitted for that lower level. It does not claim OS delivery. The consumed key survives restore and Undo even if the higher notification fails. Later crossings remain separate under OU-ALERT-002. The normative SRS is unchanged.

## SC-009: authentication-required evidence

The approved app-server interface can report `AuthenticationRequired`. For a profile with an accepted reading from the same home and association, OpenUsage can report that sign-in is required. This response does not establish `invalid_grant` or confirmed invalid credentials. Missing identity, unsupported methods, malformed responses, cancellation and stale operations do not qualify as authentication proof.

OU-ALERT-007 remains open for confirmed invalid-credential evidence through a supported interface. The added sign-in alert is a narrower capability, not a change to that requirement. The normative SRS is unchanged.

## SC-010: approved unverified selected-profile activity

The owner approved [the narrow amendment](proposals/user-curated-profile-activity.md) on October 22, 2026. An explicit Profile sum view may calculate observed subtotals and shares from the same pinned provider contract and matching provider dates. Its visible Unverified label and details retain unknown identity, application scope and day-definition limits. This does not certify compatibility or distinct accounts. Known duplicate identities and known contract differences are excluded. Verified combined totals retain every existing identity, scope and live-contract gate. The normative SRS file remains unchanged.
