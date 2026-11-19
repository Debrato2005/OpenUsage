# Compatibility matrix

Updated: November 19, 2026. No production release exists.
This matrix separates executed platform evidence from required support.
[Implementation status](implementation-status.md) owns current results. [Traceability](acceptance-traceability.md) binds each result to its artifact and scope.
[Release notes](release-notes/0.1.0.md) own package contents, signing, dependencies, and recorded runtime versions.

## Platforms

| Environment | Current evidence | Remaining acceptance |
|---|---|---|
| CachyOS, Linux x86_64 | Native app and CLI journeys with private synthetic data. Exact runtime versions appear in the release notes | Full current-build acceptance and reference performance gates remain open |
| Hosted Ubuntu 24.04 x86_64 | Linux checks and renderer CI for the commits named in status and traceability | CI does not establish native desktop or installation acceptance on Ubuntu |
| Ubuntu 24.04 LTS x64 desktop | Required Stage 1 target. Linux dependencies and package assembly exist | Real-machine AT-38 and npm installation AT-80 remain open |
| Windows 11 x64 | Required labelled preview for Stage 1. Hosted Windows Server x64 passed the CLI and desktop Rust checks | Qualified companion/scanner pins, packaged desktop builds, WebView2 recovery, and real-machine acceptance remain open |
| Windows 10 22H2 x64 | Required baseline only with applicable OS security updates | Same unexecuted Windows gates |
| macOS 14+ on Apple Silicon and Intel | Required labelled previews for Stage 1. Both architectures passed the hosted CLI and desktop Rust checks | Qualified companion/scanner pins, packaged desktop builds, and real-machine acceptance remain open |
| WSL profiles on Windows 11 | Planned with an explicitly configured supported distribution | Profile isolation, approved paths, and AT-39/AT-55 remain open |
| Other distributions or architectures | No published release qualification | Separate build, dependency, native, installation, and performance evidence required |

[SRS Section 16](SRS.md#16-platform-support-and-distribution) defines baseline systems.
Stage 1 requires Linux plus labelled Windows/macOS previews. Stage 2 requires acceptance on real Windows and macOS machines.
CI never replaces those runs. AT-80 includes the tray, notifications, and key store.
OU-DIST-007 requires a real Mac test and an ad-hoc-signed app bundle if a plain binary fails.

## Pinned executables

| Component | Current tested contract or artifact | Boundary |
|---|---|---|
| Codex companion | [0.163.0, Linux x86_64 musl pin](../crates/openusage-codex/provider-sources/codex-0.163.0-linux-x86_64.json) | Synthetic protocol/native evidence. Earlier authorized real sign-ins do not complete AT-60 |
| Local scanner | [`ccusage` 20.0.48, Linux x86_64 GNU pin](../crates/openusage-locallogs/sources/ccusage-20.0.48-linux-x86_64.json) | Enabled Claude Code and Codex fixtures only. Additional-tool AT-65 remains open |
| Scanner candidates | [Qualification checklist](implementation-slices.md#build-checklist) | Isolated experiments and qualification features. Production pins remain unchanged |

The [preview archive check](reviews/2026-11-19-preview-archive-check.md) records matching upstream digests and executable headers for Windows x64 and both Mac architectures. These companions and scanners still need source and runtime qualification.

The [Codex contract](../crates/openusage-codex/README.md) and [scanner contract](../crates/openusage-locallogs/README.md) define version and format boundaries.
A passing fixture proves its declared shape. It does not prove a current live account returns that shape.
A changed parser needs renewed field acceptance under OU-AUD-008.

## Build and distribution scope

The [CI workflow](../.github/workflows/ci.yml) defines Linux checks, Chromium/WebKit renderer checks, and hosted preview checks for Windows and macOS. The [CLI record](reviews/2026-11-19-preview-cli-process.md) and the [desktop compile record](reviews/2026-11-19-preview-desktop-compile.md) own the executed results.
[Engineering workflow](engineering-workflow.md) owns build tools and commands.
Renderer fixtures use synthetic IPC. Their browser engines do not qualify Windows WebView2 or macOS WKWebView.
Native Linux journeys use the actual application with private synthetic sources.
Older passing artifacts do not certify a later source snapshot.

The [Linux package check](reviews/2026-11-19-linux-package-acceptance.md) covers
Debian archive inspection, offline npm installation, and app startup on CachyOS.
Ubuntu installation and publication remain open.
[Install](install.md) labels steps that lack real-machine execution.

Public profiles and private desktop-to-website sync remain Stage 3 under [SRS Section 0.4](SRS.md#04-release-stages-normative).
No website, phone, or private-sync compatibility is claimed.
