# OpenUsage security and push review

Date: October 24, 2026. Base: `e4984f1` with a clean worktree before this assignment.

The known glib iterator flaw now has a local upstream backport. Its optimized regression failed before the fix and passed afterward. **Push readiness remains blocked** until the required environment-dependent checks pass. This review does not qualify a production release.

## 1. Medium: GLib iterator memory safety

The original `glib 0.18.5` source passed an immutable reference to a C function that changes the referenced pointer. Optimization can then leave a null pointer for the string conversion. [RUSTSEC-2024-0429](https://rustsec.org/advisories/RUSTSEC-2024-0429.html) describes the defect.

The root [Cargo patch](Cargo.toml#L8) now selects the local source for every glib consumer. The [iterator](vendor/glib-0.18.5/src/variant_iter.rs#L118) contains exactly the upstream two-line fix. The original version, API and MIT license remain intact. No advisory suppression or version disguise is added.

The source archive SHA-256 matches the former lockfile checksum. Comparison covered all 121 archive files. Only the iterator differs. [Patch provenance](vendor/glib-0.18.5/OPENUSAGE-PATCH.md) records the archive, upstream revision, fixed-file hash and removal condition.

The [regression](apps/desktop/src-tauri/tests/glib_variant.rs#L8) covers all affected iterator entry points, Unicode, empty values, exhaustion, mixed directions and type refusal. The test profile compiles glib at optimization level 2. An isolated Linux probe with the same test aborted against the original source, then passed against the fixed source. Full application and native acceptance remain separate gates.

## 2. Publication and secret boundaries

Gitleaks 8.30.1 scanned all reachable local refs and reported 610 commits with scanned patches and no leaks. Git integrity checks passed. The current source snapshot also passed Gitleaks. TruffleHog 3.99.0 scanned that snapshot with credential verification and updates disabled. No existing provider credential home was inspected.

The final snapshot contained 567 files. TruffleHog reported only the synthetic rejected-login URI in `crates/openusage-codex/tests/boundary.rs`. An earlier snapshot also flagged a Git commit hash in the previous review's Cloudflare skill URL. That value matched the cited commit exactly. The current review replaces that obsolete section. No real credential was detected, and no scanner allowlist was added.

Source review found the existing renderer without raw HTML, dynamic evaluation, browser credential storage or direct network calls. Tauri retains its restrictive CSP, local capability list, typed commands, external-navigation refusal and new-window refusal. Rust continues to own credentials, processes, keys and persistence. These observations do not establish the absence of every vulnerability.

The notice inventories match all 509 remaining registry package checksums and both notice-text hashes. The local glib source includes its original MIT license. Two pre-existing Rust formatting defects were corrected. The normative SRS is unchanged.

## 3. Executed checks

| Check | Current result |
| --- | --- |
| Doctor with pinned Node 22.23.2 | Passed toolchain and Linux package checks. |
| Optimized iterator probe | Original aborted. Patched source passed. Synthetic Linux scope. |
| Cargo metadata | One glib resolution, from the patched local manifest. Vendor excluded from workspace membership. |
| Renderer build and TypeScript | Passed. |
| IPC contracts | All 55 passed. |
| Python engineering/storage tools | All 26 passed. |
| Rust formatting and whitespace | Passed after formatting corrections. |
| Strict workspace Clippy, all targets | Passed with `-D warnings`. |
| History and source secret scans | No real secret detected within the stated scope. |
| Full Rust milestone | Compiled, then failed in the provider test suite. Diagnostic run: 9 passed, 36 failed. |
| Backup/import tests | All 4 passed with synthetic archives. |
| Storage suite | Incomplete: reached its 150-second deadline. No complete suite result. |
| Desktop package iterator regression | Passed with the workspace glib patch and optimized test profile. |
| Current native acceptance | Smoke attempted against the current binary. Blocked before app launch by Node Git spawning. |
| Local staging and commit | Staging failed: `.git/index.lock` is on a read-only filesystem. Nothing was staged or committed. |

## 4. Remaining environment gates

The renderer milestone cannot start its web server. A direct check confirmed `listen EPERM` on 127.0.0.1:1420. Browser checks and rendered acceptance remain unverified.

All four Git-hook behavior cases fail before their assertions because Node cannot spawn Git: `spawnSync git EPERM`. No hook or executable approval guard was relaxed.

The npm audit failed because this environment cannot resolve `registry.npmjs.org`. Cargo-audit is unavailable, so a complete current Rust advisory scan remains unverified. The specific glib advisory and upstream fix were read through the web tool. Firecrawl could not run because its account had no credits.

A live remote read failed because this environment cannot resolve `github.com`. Cached `origin/main` equals the base commit. That cache does not prove the live remote tip. Refresh the remote and require a fast-forward before an owner push.

The provider diagnostic run reproduced 35 executable-approval failures and one Unix-socket EPERM failure. The sandbox maps the filesystem root and `/tmp` to UID 65534. The source-path guard refuses unmapped owners before the fixture operations. This prevents an isolation pass. No guard was relaxed.

## Owner commands after the gates pass

Keep the existing hooks enabled. Review and commit only the named source, test, vendor and documentation paths. Then fetch `origin/main` and require it to be an ancestor of local `main`. Use an explicit main-to-main push without tags. Do not use force, all-refs publication, mirror publication or hook bypasses.

After the required checks pass outside this sandbox, use the project root:

```sh
git add -- Cargo.toml Cargo.lock SECURITY.md security_best_practices_report.md \
  docs/implementation-status.md docs/acceptance-traceability.md \
  apps/desktop/src-tauri/src/main.rs apps/desktop/src-tauri/src/workspace.rs \
  apps/desktop/src-tauri/tests/glib_variant.rs vendor/glib-0.18.5
git diff --cached --check && git diff --cached --stat
git commit -m "Backport GLib iterator memory-safety fix"
git fetch origin main &&
  git merge-base --is-ancestor origin/main main &&
  git -c push.followTags=false push origin refs/heads/main:refs/heads/main
```

No push, publication, pull request or GitHub write API occurred.

Writing lint: 1.42 findings per 100 words before this score line. This checks wording only.
