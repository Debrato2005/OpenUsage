# Provider capabilities

Updated: November 19, 2026. This reference describes implemented sources. It does not certify a release or live provider behavior.
[Implementation status](implementation-status.md) owns the current state. [Acceptance traceability](acceptance-traceability.md) owns executed evidence.
[SRS Sections 5, 5A, 5C, and 21](SRS.md#5-codex-app-server-integration) define required behavior.

## Source matrix

| Source | Level | Implemented scope | Qualification boundary |
|---|---|---|---|
| Codex app-server | Official | Account and plan reads, device-code sign-in, explicit sign-out, quota windows, reset credits, and token activity | Linux synthetic protocol and native evidence. Formal AT-60 remains open |
| Claude Code logs through `ccusage` | Local log | Consented local tokens, model estimates, sessions, project folders, and hourly activity | Pinned Linux scanner with synthetic input. Formal real-log acceptance remains open |
| Codex logs through `ccusage` | Local log | Consented local tokens, model estimates, and sessions | Pinned Linux scanner with synthetic input. Formal real-log acceptance remains open |
| Claude Code status line | Official, planned | No implemented quota adapter | S-13 and AT-84 remain open |
| Other providers | Official when documented, otherwise Unofficial | Planned | Adapter declarations, terms review, and qualification remain open |

The [tool catalog](../crates/openusage-locallogs/sources/tools.json) enables only Claude Code and Codex.
Its other sixteen entries remain disabled. Candidate parsers, grouped storage, and synthetic scanner experiments do not enable them.
The [qualification checklist](implementation-slices.md#build-checklist) owns their individual gates.

## Codex boundaries

The [Codex crate](../crates/openusage-codex/README.md) owns the protocol baseline and synthetic fixture scope.
Its [request types](../crates/openusage-codex/src/lib.rs) restrict operations to account and usage methods.
Browser sign-in remains blocked under [SC-005](srs-clarifications.md#sc-005-browser-callback-isolation-remains-unresolved).
Unsupported authentication or a missing method does not establish a usable measurement.

[Quota parsing](../crates/openusage-codex/src/quota.rs) preserves reported windows, optional reset times, and optional reset-credit details.
A window need not last five hours or one week. Missing values remain unavailable.
Time alone never confirms a reset. [Data contracts](data-contracts.md#provider-quota-and-activity) define saved values and reset assessment.

Activity preserves returned summaries and daily buckets. It does not request thread estimates or invent separate Work attribution.
Email gives provisional identity only. Verified combined totals remain unavailable under the [draft identity contract](identity-contract.md).
The explicitly Unverified profile sum has its separate approved scope.

## Local-log boundaries

The [local logs contract](../crates/openusage-locallogs/README.md) owns scanner arguments, retained fields, exact counts, and missing metrics.
Claude Code supplies project folders and session start/end times. Codex supplies session end times and reasoning counts, but no project or hourly report.
Reasoning remains part of output and never adds another token total.

Local estimates use pinned offline API prices. They are neither provider bills nor subscription spend.
Unknown models keep tokens and no estimate. Provider and local measurements remain separate.
A detected tool proves no account identity or provider quota connection.
Account splitting and session quota movement remain open under OQ-1.

## Acceptance still required

[Provider evidence](provider-evidence.md) defines the redacted AT-60 summary and records the missing formal comparisons.
The [compatibility matrix](compatibility-matrix.md) separates Linux evidence from required Windows/macOS previews and Stage 3.
New or changed used-field parsers require fresh field qualification under OU-AUD-008.
