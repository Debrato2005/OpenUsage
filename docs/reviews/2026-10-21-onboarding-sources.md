# Account onboarding source review

Reviewed October 21, 2026. This review covers pinned source and published screenshots. Competitor applications were not run. No existing credential homes were read. No competitor code was copied.

## OpenQuota

Commit: `0b21b354e1a0a3f65d78900ec244c43f581542e2`.

A short welcome notice reports discovered tools. Quota rows stay visible, while details expand on request. Failed refresh preserves the last reading and adds a local Retry action. These patterns fit OpenUsage. See [the dashboard](https://github.com/deviffyy/OpenQuota/blob/0b21b354e1a0a3f65d78900ec244c43f581542e2/src/lib/Dashboard.svelte#L396-L522) and [service cache handling](https://github.com/deviffyy/OpenQuota/blob/0b21b354e1a0a3f65d78900ec244c43f581542e2/src-tauri/src/service.rs#L649-L672).

Its Codex provider reads existing credentials, refreshes tokens, and uses private usage endpoints. It does not establish isolated managed Codex accounts. OpenUsage must retain official login, separate homes, and its typed provider interface. See [authentication](https://github.com/deviffyy/OpenQuota/blob/0b21b354e1a0a3f65d78900ec244c43f581542e2/src-tauri/src/providers/codex/auth.rs#L35-L175) and [HTTP client](https://github.com/deviffyy/OpenQuota/blob/0b21b354e1a0a3f65d78900ec244c43f581542e2/src-tauri/src/providers/codex/client.rs#L9-L160).

## codex-usage-desktop

Commit: `03bf62aea9606b51b5e0709b014624db72e4c929`.

Discovery checks PATH and common installation locations. Quota requests and cached overview loads remain separate. Stale-response guards prevent old requests from replacing newer state. Its selected environment does not establish isolated multi-account onboarding. See [discovery](https://github.com/itvincent-git/codex-usage-desktop/blob/03bf62aea9606b51b5e0709b014624db72e4c929/src-tauri/src/codex_limits.rs#L1424-L1474) and [dashboard loading](https://github.com/itvincent-git/codex-usage-desktop/blob/03bf62aea9606b51b5e0709b014624db72e4c929/src/hooks/use-usage-dashboard.ts#L306-L604).

The official quota RPC path is useful. Credential parsing, private HTTP fallbacks, session scans, external forecasts, and inference-based activation conflict with OpenUsage's SRS. Do not copy those mechanisms. See [quota sources](https://github.com/itvincent-git/codex-usage-desktop/blob/03bf62aea9606b51b5e0709b014624db72e4c929/src-tauri/src/codex_limits.rs#L24-L1059).

## CodexControl

Commit: `1d90c89e097c68ab298e8fee2462cb94f957173e`.

Add starts login, shows progress and Cancel, selects the new account, then refreshes usage. This is the desired interaction pattern. Its locator checks executability but does not establish OpenUsage's provenance guarantees. See [onboarding](https://github.com/ademisler/codexcontrol/blob/1d90c89e097c68ab298e8fee2462cb94f957173e/Sources/CodexControl/App/AppModel.swift#L239-L280) and [binary discovery](https://github.com/ademisler/codexcontrol/blob/1d90c89e097c68ab298e8fee2462cb94f957173e/Sources/CodexControl/Services/CodexBinaryLocator.swift#L4-L56).

Managed homes coexist with ambient credential discovery and account switching through copied authentication files. Private HTTP and decoded token claims supply usage and identity. Error handling removes saved snapshots. Keep the short flow, but retain OpenUsage's credential isolation, official methods, and preserved stale readings. See [account switching](https://github.com/ademisler/codexcontrol/blob/1d90c89e097c68ab298e8fee2462cb94f957173e/Sources/CodexControl/Services/CodexAccountManager.swift#L251-L327), [API source](https://github.com/ademisler/codexcontrol/blob/1d90c89e097c68ab298e8fee2462cb94f957173e/Sources/CodexControl/Services/CodexAPI.swift#L134-L453), and [failure handling](https://github.com/ademisler/codexcontrol/blob/1d90c89e097c68ab298e8fee2462cb94f957173e/Sources/CodexControl/App/AppModel.swift#L450-L465).

## multi-codex

Commit: `44cb4c11cb537bfb9e92b67e72a35e7a32f7ef10`.

Private account directories and selected CODEX_HOME launchers support isolation. Live usage calls the official quota RPC and cleans up its process group. It lacks the identity bracket and account token-activity method used by OpenUsage. Local mode scans rollout files and derives identity from credentials. See [launch isolation](https://github.com/jakoes-wu/multi-codex/blob/44cb4c11cb537bfb9e92b67e72a35e7a32f7ef10/src/multi_codex/launcher.py#L64-L98), [live collection](https://github.com/jakoes-wu/multi-codex/blob/44cb4c11cb537bfb9e92b67e72a35e7a32f7ef10/src/multi_codex/usage.py#L304-L412), and [identity parsing](https://github.com/jakoes-wu/multi-codex/blob/44cb4c11cb537bfb9e92b67e72a35e7a32f7ef10/src/multi_codex/identity.py#L104-L190).

Adopt clear next actions, bounded cleanup, and visible snapshot age. Do not adopt credential inspection, shared-session attribution, or time-only reset assertions.

## Visual and license review

The review inspected [OpenQuota](https://github.com/deviffyy/OpenQuota/blob/0b21b354e1a0a3f65d78900ec244c43f581542e2/assets/screenshot.png), [Codex Usage](https://github.com/itvincent-git/codex-usage-desktop/blob/03bf62aea9606b51b5e0709b014624db72e4c929/docs/dashboard.jpg), [CodexControl](https://github.com/ademisler/codexcontrol/blob/1d90c89e097c68ab298e8fee2462cb94f957173e/site/assets/codexcontrol-demo-ui.png), and [multi-codex](https://github.com/jakoes-wu/multi-codex/blob/44cb4c11cb537bfb9e92b67e72a35e7a32f7ef10/docs/assets/social-preview.png) images. They are illustrative assets, not executed product evidence.

All four pinned repositories carry MIT licenses. Copying substantial code requires retaining the applicable notices. This change adopts interaction patterns without copying their implementations.

## Decision: bundled official companion

Bundle the existing verified Codex 0.163.0 executable in app-server mode. Retain its pinned Apache 2.0 LICENSE and NOTICE. This reuses the qualified protocol and child-process supervisor. The smaller standalone app-server binary needs separate argument and provenance qualification before substitution.

Tauri externalBin places the companion beside the application. Rust checks that fixed file before bounded discovery of an installed supported version. A present invalid companion fails closed. Add authorizes preparing this trusted source, while account sign-in retains explicit consent. No runtime download, credential import, arbitrary execution API, or shell plugin is added.

The initial bundle is qualified only on Linux x86_64. Other targets need their own pins, permissions, packaging, and native checks. Compilation does not prove an installer or cross-platform release.

Sources: [pinned license](https://github.com/openai/codex/blob/83086188ae0614bd882818ff1aa23f3194962012/LICENSE), [pinned notices](https://github.com/openai/codex/blob/83086188ae0614bd882818ff1aa23f3194962012/NOTICE), [Tauri packaging](https://v2.tauri.app/develop/sidecar/), [official account protocol](https://learn.chatgpt.com/docs/app-server).
