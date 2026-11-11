# Additional-tool qualification gates

L-1f11aj, November 12, 2026. Select Pi for the next native application gate.
Keep the production scanner pin and enabled catalog unchanged.

## Reviewed records

The catalog has a record for each of its sixteen disabled tools.
Each linked record owns its source contract, observations, and remaining gaps.
Passing fixture comparisons preserve failure evidence; they do not enable a tool.

| Tool | Evidence home |
| --- | --- |
| Amp | [Stable-ID candidate](2026-11-08-amp-dedup-design.md), [independent review](2026-11-08-amp-dedup-review.md) |
| OpenCode | [Baseline](2026-11-11-opencode-qualification.md), [v2 boundary](2026-11-11-opencode-v2-boundary.md) |
| Pi | [Baseline](2026-11-10-pi-qualification.md), [bounded capture](2026-11-11-pi-bounded-capture.md) |
| Kilo | [Qualification](2026-11-10-kilo-qualification.md) |
| ZCode | [Qualification](2026-11-10-zcode-qualification.md) |
| Codebuff | [Qualification](2026-11-11-codebuff-qualification.md) |
| Droid | [Qualification](2026-11-11-droid-qualification.md) |
| Hermes | [Qualification](2026-11-11-hermes-qualification.md) |
| Goose | [Qualification](2026-11-11-goose-qualification.md) |
| Gemini | [Qualification](2026-11-11-gemini-qualification.md) |
| Qwen | [Qualification](2026-11-11-qwen-qualification.md) |
| Kimi | [Qualification](2026-11-11-kimi-qualification.md) |
| OpenClaw | [Qualification](2026-11-12-openclaw-qualification.md) |
| Copilot | [Qualification](2026-11-12-copilot-qualification.md) |
| Grok | [Qualification](2026-11-12-grok-qualification.md) |
| Antigravity | [Qualification](2026-11-12-antigravity-qualification.md) |

## Candidate choice

| Candidate | Executed boundary | Next material gate |
| --- | --- | --- |
| Pi bounded release | Pinned writer operations, shared capture, grouped storage, and synthetic large histories | Native consent, worker, save, and display using the candidate |
| OpenCode fresh v2 | Pinned writer and projectors, strict schema boundary, SQLite and WAL capture | Scale, grouped storage, and native integration |
| Amp stable-ID candidate | Checked copies, conflicts, discovery, and read failures | Remaining identity, fork, format, capture bounds, and application evidence |

Pi has the shortest remaining path to an application result among these candidates.
The [performance report](../performance-report.md#linux-release-local-history)
keeps its release scanner results separate from the native Claude Code result.
The Claude Code benchmark does not qualify Pi in the desktop app.

[SC-025](../srs-clarifications.md#sc-025-captured-local-records-and-producer-completion)
accepts complete validated captured records without proving producer completion.
It replaces the older Pi prefix-failure assumption. Do not add an idle-time rule
or revive the stronger publication requirement.

## Native boundary to build

The [desktop worker](../../apps/desktop/src-tauri/src/local_logs.rs) still uses
the production binary descriptor. Detection, consent, and scan planning apply
the normal enabled-tool gate. The [grouped runner](../../crates/openusage-locallogs/src/grouped.rs)
exposes a Pi qualification entry point behind a feature. The app does not call it.

The [save path](../../apps/desktop/src-tauri/src/workspace/local_logs.rs) currently
records the production version constant for grouped scans. A candidate journey
must retain the actual pinned candidate version. It must not record a candidate
scan as production `20.0.48`.

L-1f11ak will exercise a qualification build with one explicit pinned Pi descriptor.
It must use the normal consent, worker, grouped save, and typed view paths.
The production build keeps its current gates and pin. No runtime variable may
make that build run an unqualified tool.

The native journey must prove exact saved and displayed counts, copied usage,
refusal with saved-data retention, consent withdrawal, and source isolation.
It must check scanner provenance after reopen. Full affected Rust and renderer
checks remain mandatory. This review ran doctor and checked the catalog-to-record
mapping and source pins. It ran no new scanner or native journey.
