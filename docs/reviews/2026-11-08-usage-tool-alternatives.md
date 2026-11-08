# Usage tool alternatives

Date: November 8, 2026. Scope: primary documentation and selected source files. No tool was installed or executed. No dependency, archive, credential, browser profile, or real log was read locally.

This record compares alternatives with the existing OpenUsage constraints. It proposes patterns, not new requirements. The separate [L-1f research](2026-11-08-l1f-source-research.md) covers the pinned `ccusage` input contract.

## Source snapshots

These are current source commits observed through the GitHub API, not tested releases.

| Tool | Verified repository and commit | Main role |
|---|---|---|
| CodexBar | [steipete/CodexBar, `7447bc77`](https://github.com/steipete/CodexBar/tree/7447bc77c2497323d9c41e40164ce705533493c8) | Provider quotas, reset windows, account management, and selected local history |
| Tokscale | [junhoyeo/tokscale, `82613d2f`](https://github.com/junhoyeo/tokscale/tree/82613d2f5e96654ce3729892afbb0025979f1c75) | Broad usage history, terminal reports, live quotas, sharing, and a public leaderboard |
| Tokentop | [tokentopapp/tokentop, `bca33680`](https://github.com/tokentopapp/tokentop/tree/bca336801232f4ac54710db436282bb62d0e8a44) | Live terminal views from provider polling and agent session plugins |

## Comparison

| Question | CodexBar | Tokscale | Tokentop |
|---|---|---|---|
| Local tokens or live limits? | Both, through separate paths | Both; the current source has a Usage tab and provider quota commands | Both; provider plugins and agent plugins have separate jobs |
| Multi-account evidence? | Managed Codex homes, provider IDs, workspace checks, and account-specific history | Multiple saved Codex accounts; separate identity and credential-source handling | The inspected Codex plugin selects one credential source; no general multi-account ledger was established in this review |
| Cost meaning? | Local estimates and provider billing remain distinct source types | Public pricing, provider-specific prices, recorded amounts, and some inferred token sources need distinct interpretation | Session costs use model prices; provider capabilities distinguish actual costs from estimates |
| Public sharing? | No leaderboard appeared in the inspected core path | Explicit submission and optional scheduled submission to a public service | No upload service appeared in the inspected core path; notification plugins can extend behavior |
| Installation advertised? | macOS app; Linux CLI and Qt desktop app | npm entry point with a native Rust core | Standalone binaries, Homebrew, Scoop, or npm with Bun |

The following sections give the evidence and limits for this table.

## CodexBar

The [README](https://github.com/steipete/CodexBar/blob/7447bc77c2497323d9c41e40164ce705533493c8/README.md) describes a macOS menu app and a Linux Qt desktop app. The Linux desktop needs its CLI and Qt runtime. Its advertised provider paths include OAuth, provider CLIs, API keys, browser sessions, and selected local files. These are different trust boundaries. It does not offer every metric through every source.

The [Codex provider guide](https://github.com/steipete/CodexBar/blob/7447bc77c2497323d9c41e40164ce705533493c8/docs/codex.md) separates quota retrieval from local token history. Its automatic path tries a personal access token, OAuth, then `codex app-server` where supported. OAuth uses the WHAM usage endpoint. A selected managed workspace disables an unscoped CLI fallback. External OAuth sources default off. Managed accounts use separate homes and workspace checks. Shared native credentials remain the provider CLI's responsibility for renewal.

Inspected [identity code](https://github.com/steipete/CodexBar/blob/7447bc77c2497323d9c41e40164ce705533493c8/Sources/CodexBarCore/Providers/Codex/CodexIdentity.swift) prefers a provider account ID. It also contains an email-only fallback for partial or legacy authentication. [History ownership](https://github.com/steipete/CodexBar/blob/7447bc77c2497323d9c41e40164ce705533493c8/Sources/CodexBar/Providers/Codex/CodexHistoryOwnership.swift) keeps separate provider-ID and email-hash forms. OpenUsage must not treat that fallback as verified identity.

The inspected [request-identity code](https://github.com/steipete/CodexBar/blob/7447bc77c2497323d9c41e40164ce705533493c8/Sources/CodexBarCore/Vendored/CostUsage/CostUsageScanner%2BRequestIdentity.swift) uses session-scoped response IDs where available. It also tracks fallback row keys and reconciles mirrored requests across files. This is a useful example of explicit duplicate evidence, not proof that equal token totals identify one request.

The [Claude guide](https://github.com/steipete/CodexBar/blob/7447bc77c2497323d9c41e40164ce705533493c8/docs/claude.md) documents separate OAuth, web, CLI, and local-cost paths. It preserves incomplete and unpriced coverage. It also describes account binding checks before history fragments move between account scopes. These source and coverage distinctions fit OpenUsage better than copying its complete fallback stack.

## Tokscale

The [README](https://github.com/junhoyeo/tokscale/blob/82613d2f5e96654ce3729892afbb0025979f1c75/README.md) lists many local formats, remote usage caches, project/session reports, sharing, and live subscription quotas. Its npm command includes a native Rust core. Coverage does not mean every source supplies measured tokens: for example, the README labels Freebuff usage as estimated from transcripts. Such estimates cannot replace OpenUsage's reported counts.

The inspected [Codex quota code](https://github.com/junhoyeo/tokscale/blob/82613d2f5e96654ce3729892afbb0025979f1c75/crates/tokscale-cli/src/commands/usage/codex.rs) stores multiple accounts and calls WHAM usage with their credentials. Account matching prefers an account ID, then an identical ID token, then an identical access token. Those fallback keys identify credentials, not independently verified people. Refresh writes target Tokscale's own credential store. The source explicitly notes that copying a refresh grant still creates a renewal conflict risk with the original CLI. Separate files alone do not remove that risk.

The [Codex parser](https://github.com/junhoyeo/tokscale/blob/82613d2f5e96654ce3729892afbb0025979f1c75/crates/tokscale-core/src/sessions/codex.rs) reads current and archived sessions and uses in-file session identity. The [Claude parser](https://github.com/junhoyeo/tokscale/blob/82613d2f5e96654ce3729892afbb0025979f1c75/crates/tokscale-core/src/sessions/claudecode.rs) carries message and request IDs for duplicate checks. The [OpenCode/Claude bridge](https://github.com/junhoyeo/tokscale/blob/82613d2f5e96654ce3729892afbb0025979f1c75/crates/tokscale-core/src/sessions/opencode_claude_agent.rs) claims Claude sessions only after OpenCode counts rows carrying native session IDs. This is source-specific attribution. It does not associate an arbitrary local session with a subscription account.

[Offline documentation](https://github.com/junhoyeo/tokscale/blob/82613d2f5e96654ce3729892afbb0025979f1c75/docs/offline.md) states that `--offline` blocks quota refresh and implicit remote history sync. It uses existing logs, caches, and prices. Missing remote history remains missing. This is an application policy, not an operating-system sandbox. Without that mode, reports can sync certain provider caches and fetch prices.

The [privacy policy source](https://github.com/junhoyeo/tokscale/blob/82613d2f5e96654ce3729892afbb0025979f1c75/packages/frontend/src/app/%28main%29/privacy/page.tsx) documents explicit uploads. Submitted data includes token counts, estimated costs, dates, tools, models, message counts, session timing, MCP server names, and device labels. GitHub identity and submitted profiles are public-facing service data. This is broader than OpenUsage's approved leaderboard payload. The inspection did not independently trace every submission field through its serializer.

## Tokentop

The [README](https://github.com/tokentopapp/tokentop/blob/bca336801232f4ac54710db436282bb62d0e8a44/README.md) describes eight agent integrations, eleven providers, file watching, local SQLite storage, and budget alerts. Those counts describe its documentation at this commit. They do not mean eleven local log parsers. Installation options include packaged binaries and package managers. The npm route needs Bun.

The inspected [Codex plugin](https://github.com/tokentopapp/tokentop/blob/bca336801232f4ac54710db436282bb62d0e8a44/src/plugins/providers/codex.ts) declares quota support but no token-usage or actual-cost capability. It tries OpenCode credentials before a Codex file and sends an account ID with its WHAM request. This clean capability distinction is useful. Automatic cross-application credential discovery does not match OpenUsage's consent boundary.

The inspected [pricing client](https://github.com/tokentopapp/tokentop/blob/bca336801232f4ac54710db436282bb62d0e8a44/src/pricing/models-dev.ts) fetches `models.dev/api.json` with a one-hour cache. Thus “local” does not mean “no network.” The README's no-telemetry claim is documented behavior, not a network audit from this research.

The [aggregator](https://github.com/tokentopapp/tokentop/blob/bca336801232f4ac54710db436282bb62d0e8a44/src/agents/aggregator.ts) groups by session, provider, and model. It can suppress a Claude session through an explicit proxy session ID. It also has a heuristic that matches project, model/provider overlap, and activity within 120 seconds. That heuristic does not meet OpenUsage's evidence rule for a confirmed duplicate. The code uses JavaScript numbers, so its arithmetic path also differs from OpenUsage's Rust exact-count boundary.

## Patterns suited to OpenUsage

1. Keep provider quotas and local history separate. Use the existing Provider and This device scopes. A current credential does not identify the owner of old logs.
2. Give each adapter explicit capabilities, source evidence, missing-field states, and fixture coverage. Broad support lists cannot replace those contracts.
3. Bind live readings to provider-issued account and workspace IDs. Keep local rows under Unknown account unless their source or an explicit user link supplies the association.
4. Deduplicate with source-specific request/session evidence. Preserve unresolved overlaps instead of using equal values, email, or nearby timestamps as proof.
5. Keep the pinned `ccusage` path while qualifying its formats. Use CodexBar for quota and account design references, Tokscale for parser fixtures, and Tokentop for capability patterns.

These patterns fit the existing [architecture](../architecture.md), [SRS](../SRS.md), and [launch checklist](../implementation-slices.md). None justifies silently replacing the pinned engine, importing other tools' credentials, adding a network dependency, or expanding the leaderboard payload.
