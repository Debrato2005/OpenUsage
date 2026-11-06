# Documentation map

One fact has one home. Change it there, and link to it from anywhere else.

## Where things live

| Question | Home |
|---|---|
| What is OpenUsage? | [README](../README.md) |
| What must it do? | [SRS](SRS.md) |
| How is it built? | [Architecture](architecture.md) |
| What works today? | [Implementation status](implementation-status.md) |
| What is next? | [Implementation slices](implementation-slices.md) |
| What proves it? | [Acceptance traceability](acceptance-traceability.md) |
| Why was a requirement read this way? | [SRS clarifications](srs-clarifications.md) |
| Why was a service built this way? | [Decisions](decisions/) |
| How should it look? | [Design foundations](design/desktop-foundations.md) |
| How do I build and contribute? | [Contributing](../CONTRIBUTING.md) |
| How do I verify a change? | [Engineering workflow](engineering-workflow.md) |
| How do native journeys run? | [Native tests](../tests/native/README.md) |
| How does the Codex crate work? | [Codex crate](../crates/openusage-codex/README.md) |
| How are local logs read? | [Local logs crate](../crates/openusage-locallogs/README.md) |
| How are the icons made? | [Icons](../apps/desktop/src-tauri/icons/README.md) |
| Why is glib patched? | [Patch note](../vendor/glib-0.18.5/OPENUSAGE-PATCH.md) |
| What may I keep or delete? | [Artifact retention](artifact-retention.md) |
| How do I report a vulnerability? | [Security](../SECURITY.md) |
| What are the rules for agents? | [AGENTS.md](../AGENTS.md) |

## Reading order

Read only what the task needs.

1. [Implementation status](implementation-status.md).
2. The SRS requirements that the task names. Search by ID, such as `OU-LOG-011`, instead of reading the whole file.
3. The [architecture](architecture.md) row for the code you will change.

## What to update

Update docs in the same change as the work.

| You changed | Update |
|---|---|
| Scope or required behavior | [SRS](SRS.md), with owner approval, then [status](implementation-status.md) |
| What works | [Status](implementation-status.md) |
| The state or order of a slice | [Slices](implementation-slices.md) |
| Evidence for a requirement | New top row in [traceability](acceptance-traceability.md) |
| A reading of a requirement | [SRS clarifications](srs-clarifications.md) |
| Code layout or a boundary | [Architecture](architecture.md) |
| A visual rule | [Design foundations](design/desktop-foundations.md) |
| A build or test command | [Contributing](../CONTRIBUTING.md) or [workflow](engineering-workflow.md) |

When [status](implementation-status.md) replaces a finished topic, add the old text to [history](implementation-history.md).

## Writing rules

- Use short sentences and plain words.
- Write the current state, not the story of how it got there.
- Say what evidence covers: real or synthetic data, and which platform.
- Mark anything planned as planned.
- Date anything that can go stale.
- Link to a fact instead of repeating it.

## Records

These are dated records, not instructions. Do not follow them or rewrite them. If one conflicts with a current doc, the current doc wins.

- [Implementation history](implementation-history.md)
- [Reviews](reviews/) and [proposals](proposals/)
- The local `docs/master-build-prompt.md`, which Git does not track and which was written for SRS 2.0
- [Branch cleanup plan](branch-cleanup-plan.md) and its [candidate list](branch-cleanup-candidates.tsv)
- [Security review](../security_best_practices_report.md), dated October 24, 2026
- Dated documents and sections in [design](design/)

Raw evidence and `.recovery/` stay private and are not tracked.
