# Usage tracking strategy after the tool review

Date: November 8, 2026. This review compares public documentation and selected source files. It does not certify another product's runtime behavior.

## Recommendation

Keep the pinned `ccusage` engine for local logs. Keep provider quotas on the existing account path. Complete reliable local reporting before the leaderboard client. OpenUsage can combine local history, account quotas, and optional sharing without requiring a public account for its private features.

Use tokenmaxxing as a reference for joining, previewing an upload, and sharing. Use CodexBar for quota and account scopes. Use Tokscale for parser fixtures and Tokentop for adapter capabilities. The [alternatives review](2026-11-08-usage-tool-alternatives.md) cites their source commits and limits.

## What the tools do

| Tool | Main job | Best use in OpenUsage |
|---|---|---|
| [tokenmaxxing](https://tokenmaxxing.sh/) | Runs ccusage locally, then publishes usage summaries through profiles and a leaderboard | Simple optional sharing, source selection, and upload previews |
| [ccusage](https://ccusage.com/) | Reads retained local logs and produces reports by date, tool, model, and session | Keep it as the pinned parser engine, with offline pricing and fixture tests |
| CodexBar | Retrieves provider quotas and manages account scopes, with separate local history | Model the source, account, and workspace of each live reading explicitly |
| Tokscale | Combines broad local history, live quota commands, terminal reports, and public sharing | Study source-specific duplicate tests and format support |
| Tokentop | Displays provider polling and local agent sessions in a terminal | Give each source explicit capabilities and missing-field states |

The last three rows rely on the linked alternatives review. All five projects have different scopes. A large supported-tools list does not mean every tool supplies tokens, quotas, costs, and account identity.

## tokenmaxxing findings

The [official site](https://tokenmaxxing.sh/) describes public profiles, multiple devices, source filters, and an optional service that syncs every five minutes. The [privacy policy](https://tokenmaxxing.sh/privacy) lists daily usage, OAuth profile basics, and device hostnames. It states that prompts, project names, paths, and conversation content are not uploaded. Public identity belongs to its service account. It is not proof of the provider account that produced each session.

The [npm record](https://registry.npmjs.org/@851-labs%2ftokenmaxxing/latest) reported version 0.7.8 during this review. It declares a preinstall script and platform packages. Source inspection used commit `0a02787ac766bf21a72a45609da949e4ee754595`, returned by the [main reference](https://api.github.com/repos/851-labs/tokenmaxxing/git/ref/heads/main). This review did not prove that the published binary was built from that commit.

The inspected [runner](https://github.com/851-labs/tokenmaxxing/blob/0a02787ac766bf21a72a45609da949e4ee754595/apps/cli/src/ccusage/runner.ts) invokes `ccusage@^20.0.33` through Bun or npx. It selects one source and uses `--mode calculate`. That cost mode fits OpenUsage's API-price estimates. Runtime package resolution does not fit OpenUsage's pinned-binary rule.

The inspected [sync path](https://github.com/851-labs/tokenmaxxing/blob/0a02787ac766bf21a72a45609da949e4ee754595/apps/cli/src/commands/sync.ts) uploads parsed daily reports, command metadata, device metadata, and session counts. Current session reports stay local. A session failure can leave a daily report usable with partial status. OpenUsage should adopt that distinction when its source capabilities support it.

Its [server aggregation](https://github.com/851-labs/tokenmaxxing/blob/0a02787ac766bf21a72a45609da949e4ee754595/apps/api/src/usage/ccusage.ts) distributes unreported day tokens across models by token weight. It also supplies zeros for missing token fields and distributes some day costs across models. These are useful facts about its comparison model, but they do not meet OpenUsage's requirements for exact attribution and missing values. OpenUsage must preserve an unknown breakdown rather than invent one.

For our leaderboard, keep the existing strict upload contract. Build one allowlisted payload for both preview and send. Do not forward a parser's whole JSON report. The [service decision](../decisions/2026-11-07-leaderboard-service.md) already excludes device names, projects, paths, and provider account IDs. Keep the Self-reported title and an explicit opt-in.

## ccusage findings

The [official report guide](https://ccusage.com/guide/all-reports) supports unified and focused local reports. It documents one cross-tool duplicate case for OpenCode's Claude agent. The same guide describes limitations around midnight. One scan of one consented folder cannot see another tool's copies, so OpenUsage must not claim complete cross-tool deduplication.

The [Codex guide](https://ccusage.com/guide/codex/) describes reconstruction from retained session logs, including cumulative counts and compaction records. It explicitly distinguishes these local totals from account-wide usage. Selecting several log roots changes which files are read. It does not verify who owned every event.

The [cost-mode guide](https://ccusage.com/guide/cost-modes) distinguishes stored costs from calculated prices. `auto` can use a stored cost, including zero. `calculate` uses token prices. `display` can show zero when stored cost data is absent. OpenUsage's estimate must use `calculate`, and unknown prices must stay unavailable.

The pinned version remains 20.0.48. Its [core types](https://github.com/ccusage/ccusage/blob/8995f3b7fd7110b8f37564f7d0468acc7a7d6b20/rust/crates/ccusage-core/src/types.rs) retain some extra tokens in totals without serializing their model breakdown. They also permit `storedCost`. Current online documentation can describe later behavior. The [pinned-source review](2026-11-08-l1f-source-research.md) therefore remains the starting point for fixture qualification.

## Account and duplicate policy

Keep historical local logs under Unknown account unless the source supplies an account ID or the user explicitly links them. A current sign-in cannot prove the ownership of old logs. A folder, email, or nearby timestamp cannot prove it either. This follows OU-LOG-015 and OU-DUP-001 to OU-DUP-003. OQ-1 remains open for a future linking design.

Keep Provider and This device separate. Do not add their counts together. Keep tools separate by default, and preserve May overlap for an optional cross-tool sum. Session quota movement needs a defensible account link and saved readings at both ends. It cannot come from dividing a token count by an assumed quota.

Use provider-issued account and workspace IDs for live quotas. Do not copy the other tools' browser-cookie access, cross-application credential imports, or token refresh behavior. Those paths conflict with OpenUsage's current scope. The alternative tools do not remove our identity requirements.

## Bounded next change

The first implementation change fixes API-price estimates in the existing runner under OU-LOG-006, OU-LOG-013, and OU-LOG-014. Every report uses fixed `--mode calculate` with the existing offline mode. Synthetic logs carry both zero and large stored costs. Their token counts, calculated estimates, unknown prices, sessions, and hours must stay correct.

Acceptance needs the pinned-binary regression, full Rust workspace checks, and the native consent journey with saved values checked independently. This change qualifies no additional tool. The remaining L-1f work covers report capabilities, exact totals with unknown breakdowns, isolated folders, and fixtures for all 16 tools.

## The download rule

The [build checklist](../implementation-slices.md#rules-for-every-slice) says to use the lockfiles and local caches and to download nothing during a slice. This is a repository rule, not a tool limitation. It does not stop public web research. No package, archive, or tool installation is needed for this comparison or the estimate fix. The user asked what the rule means, not to remove it, so it stays unchanged.
