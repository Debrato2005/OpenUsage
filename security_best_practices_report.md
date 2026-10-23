# OpenUsage security and push review

Date: October 23, 2026. Reviewed HEAD: `199f618093a2173cdf0e9fb1a9a6235eca87e2cb` plus the 17 existing modified files.

No secret or private-file blocker was detected for a normal source push. The repository is **not fully security-cleared**. A known dependency advisory and incomplete checks remain. This report does not qualify a production release.

## Scope and method

The review covered all 631 commits reachable from local refs, including 630 reachable from main. The cached origin/main is 21 commits behind main. Git history is not shallow. Git integrity checks found no corruption. Unreachable scratch objects remain local and were not deleted.

Gitleaks 8.30.1 scanned all refs and reported 609 commits with scanned patches. To cover historical snapshots independently, both scanners also examined all 2,128 reachable blobs and exported commit messages. These blobs total 51,946,229 bytes. Both scanners examined a separate snapshot of all 443 tracked working files. No existing provider credential home was inspected. TruffleHog 3.99.0 ran with credential verification and automatic updates disabled.

Gitleaks reported no secrets. TruffleHog reported one current URI and two historical versions of that URI. Each belongs to the synthetic rejected-login-URL test in [boundary.rs](crates/openusage-codex/tests/boundary.rs#L192). None is a real credential. No scanner allowlist was added.

Historical path checks found no private or generated paths in either main or all reachable refs. Current tracked-ignore checks found none. Historical media paths contain fonts, application icons and the logo. Automated scans cannot prove the absence of every secret, including secrets embedded in arbitrary binary content.

Manual review focused on the existing diff, renderer sinks, Tauri permissions and navigation, typed IPC, provider process isolation, credential storage, key wrapping and export publication. This is not a line-by-line audit of every historical revision.

## Security finding

### 1. Medium: known glib dependency advisory remains unresolved

[Cargo.lock:1475](Cargo.lock#L1475) locks glib 0.18.5. [RUSTSEC-2024-0429](https://rustsec.org/advisories/RUSTSEC-2024-0429.html) affects versions from 0.15.0 through versions below 0.20.0. Its iterator implementation can cause undefined behavior and crashes. Patched versions start at 0.20.0.

The source search found no affected API caller outside glib's own implementation, documentation and tests. This limits the evidence of application reachability. It does not patch the dependency or establish that every binary is unaffected. [SECURITY.md:22](SECURITY.md#L22) already records this release gate. Review a compatible upstream update or backport before release. Do not suppress the advisory merely to clear a scanner.

## Executed checks and remaining gates

| Check | Result in this review |
| --- | --- |
| Engineering doctor | Linux native packages available. Pinned Node 22.23.2 selected after the initial Node mismatch. |
| Whitespace and tracked-ignore checks | Passed. Normative SRS unchanged from cached origin/main. |
| Renderer production build and TypeScript | Passed. |
| Typed IPC contract checks | All 55 passed. |
| Python engineering/storage tools | All 26 passed. |
| Rust protocol boundary tests | All 8 passed with synthetic inputs. |
| Rust provider library tests | 9 passed, 36 failed. Thirty-five fixtures failed executable approval. One Unix socket fixture returned EPERM. |
| Rust backup/import tests | All 4 passed with synthetic archives, including wrong-password and tampering rejection. |
| Changed native harness syntax | All three files passed Node syntax checks. |
| Full renderer milestone | Blocked before browser tests. Vite cannot bind 127.0.0.1:1420: EPERM. |
| Git safety hook behavior tests | Blocked. Node spawnSync cannot start git: EPERM. Hook configuration and source were checked. |
| Full Rust workspace | The bounded 180-second attempt produced compilation output without a completed test result. Unverified. |
| Current native application journeys | Not executed. Existing copy/layout acceptance remains incomplete. |
| npm vulnerability audit | Blocked by EAI_AGAIN for registry.npmjs.org. No current clean audit result. |
| Complete Rust vulnerability audit | cargo-audit is unavailable. The specific glib advisory was checked against RustSec. |
| Live remote main | git ls-remote failed. Cached origin/main does not prove the current remote tip. |

The provider fixture failures occur before the intended supervised operations. Sandbox-visible system ancestors, including / and /tmp, have unmapped owner UID 65534. The source-path guard explicitly refuses unmapped owners. These failures do not prove an application vulnerability, but they prevent a passing isolation claim. No guard was relaxed.

The baseline and notification notice inventories together cover all 510 current registry name/version pairs. Their package checksums match Cargo.lock. Both notice text hashes and all recorded text slices match. These are source notice checks, not legal or linked-binary qualification. SECURITY.md still describes only the 486-package baseline.

The renderer search found no direct raw-HTML sink, dynamic evaluation, browser credential storage or direct network request. Tauri has a restrictive CSP, local command capabilities, disabled developer tools and denied new windows. Rust validates IPC and owns the sensitive operations. These source observations do not replace native acceptance.

## Cloudflare skill applicability

The requested Cloudflare skill is not installed locally. The official [Cloudflare skill](https://github.com/cloudflare/skills/blob/86505e44f87d801aa1864a3c4109e9a8ae5c409f/skills/cloudflare/SKILL.md) and [catalog](https://github.com/cloudflare/skills) describe Cloudflare platform workflows. This repository is a Rust/Tauri desktop app without a Cloudflare runtime. No Cloudflare security test was claimed or run. No external skill or dependency was installed. React/general frontend security guidance and the repository's native tests supplied the applicable checks.

## Owner-controlled next action

Complete the blocked checks in a suitable local environment before treating this as a clean security review. Preserve the existing hooks. Stage only the reviewed tracked edits and this report, then inspect the staged diff. A normal main-only push must remain a fast-forward. Never use force, all-refs, mirror publication or hook bypasses.

No product source, normative SRS, credential home or Git configuration changed during the review. The only added non-ignored file is this report. No staging, commit, push, publication, pull request or GitHub write API occurred.

Writing lint: 1.52 findings per 100 words. This checks wording only.
