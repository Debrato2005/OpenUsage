# Saved quota readings

October 19, 2026. Quota readings now persist in SQLCipher 4 with WAL. They load after unlock and restart without approving or executing a provider, including archived profiles. Failed refreshes and cancellation retain saved values. Rust controls freshness. Restart and restore return an unknown freshness state.

Quota and activity share one profile association. A changed sign-in or restored home requires review and explicit consent before saving new readings. Review and Keep saved readings neither execute the provider nor change storage. Confirmation consumes a short-lived, purpose-bound review and checks fresh provider evidence. The new association and reading commit together. Earlier history remains intact. Identity remains provisional.

Schema 3 adds quota snapshots and current values, with exact nullable credit counts and ordered credit details. UTC reset times are stored as text. The private migration connection disables foreign keys before its transaction and rebuilds only the collection-run table. It checks foreign keys before commit and restores enforcement before use. Failure rolls back. Cleanup failure discards the connection. Authenticated schema 1/2 archives remain supported.

Backup, restore, and restore undo preserve all twelve activity/quota tables. No database engine, credential, or key-management downgrade was made.

The [verification record](../evidence/native-durable-quota/verification.json) retains the executed evidence. The native Linux journey passed seven normal quota cases, a further keyboard-confirmed association read, and one activity seed. Eighteen independent all-column SQLCipher checks and two genuine window closes passed. Restart, profile isolation, failed refresh, keyboard cancellation, encrypted export, restore, and undo passed. Four reviewed captures cover dark desktop, narrow light views, and consent. JetBrains Mono loaded in each capture. The default build refused all four synthetic providers before execution.

Workspace tests passed 174 checks. One owned-child helper is intentionally ignored and invoked by its parent test. Strict workspace/all-target Clippy and formatting passed. `npm run check` passed the build, 34 contract checks, and 86 Chromium/WebKit renderer checks. Both desktop configurations compiled at `f4c2cb2`. The later `f7412bb` changes only the executed native test. All 199 checked files and ten assets matched their recorded hashes after execution.

These are synthetic Linux debug checks. They do not establish live OAuth, verified identity or combined totals, monitoring, reset confirmation, full credit/history interfaces, release performance, other platforms, or complete accessibility. The new default artifact has source-refusal qualification. Earlier real OS key-store receipts retain their original artifact scope. The mission remains ongoing.

## Overdue expectations and cache errors

The next slice adds a Rust-derived `resetDue` flag to saved windows. An elapsed valid reset time shows “Reset due; awaiting confirmation” beside its original UTC time. Saved values remain unchanged. Cache-load errors now show Freshness unknown for retained quota and activity values, including daily rows. A successful reload restores Rust's state. There is no schema change or new dependency.

The native deadline check passed after 47 seconds through the existing sixty-second cache interval. No provider ran and all twelve tables and live storage hashes stayed unchanged. Restart without approval retained the due message. The full journey passed eight normal quota reads, another confirmed association read, twenty independent SQLCipher checks, restore and undo, and four reviewed captures. The default configuration refused all four fixtures. Workspace tests passed 176 checks, strict Clippy and formatting. The frontend passed 34 contracts and 88 renderer checks. The [reset verification record](../evidence/native-quota-reset-due/verification.json) binds source `dffda70`, both compiled configurations, and the unchanged reused read-only verifier.

Two failed native attempts remain retained. Setup timed out during concurrent linking. A separate attempt later stalled during WebDriver path entry after saving activity. No owned native processes remained. The same artifact and unchanged timeouts then passed with command timing only. The longest command took 1,394 ms and path entry took at most 257 ms. The isolated stall's cause remains unconfirmed. These figures do not qualify product performance.

This is partial OU-QUOTA-015. Positive reset confirmation still requires all five SRS conditions. Persistent alert generations remain unimplemented.

## Observation-time freshness

Quota and activity now capture a Rust monotonic timestamp with each validated measurement, before the final account check. Successful publication retains that timestamp instead of starting a new freshness period. The anchor stays in Rust memory. Restart still gives unknown freshness. No database or IPC field changes.

Both delayed-publication service checks failed before this fix. The existing held-response core checks prove that observation precedes the final account response. The expiry check also keeps a reading stale when its wall-clock age is zero. All 176 workspace checks, strict Clippy, and formatting passed.

The rebuilt native journey passed eight normal quota cases, explicit association, activity collection, twenty SQLCipher checks, restart, restore, and undo. Four fresh captures were reviewed. The default build refused all four fixtures. All 199 source hashes and ten assets matched. The 49 frontend/config/test files match the preceding 34-contract/88-renderer qualification, so those checks were reused. See the [observation verification record](../evidence/native-observation-anchor/verification.json), source `8fad7d1`.

The default debug link took 14 minutes 38 seconds during host memory pressure. It finished without cancellation or compiler changes. Native execution followed both completed builds.

A subsequent Linux/LLD build limited the final link to one worker with `cargo rustc`. It finished in 39.17 seconds and produced identical binary bytes. The existing default native qualification therefore applies. The [README recipe](../../README.md) cites the official Cargo and LLD options. This is an uncontrolled host observation, not a benchmark. Next: implement conservative reset confirmation and persistent window generations.

## Reset confirmation

Schema 4 saves each reset assessment with its accepted quota snapshot. Rust requires all five SRS conditions before it creates a generation. Repeated readings retain that generation. An unconfirmed timestamp change cannot clear an unresolved deadline. Provider version changes also retain the deadline and generation, without confirming a reset.

Migration initializes old readings as baselines without replaying reset events. Backup validation checks packet links and recomputes assessments. Equal observation timestamps use the stored unique predecessor chain. These checks establish internal consistency, not provider origin. The original schema files and normative SRS remain unchanged.

The native Linux journey passed 23 quota reads, explicit reassociation, a separate activity seed, and 29 SQLCipher checks across all thirteen tables. Six negative reset cases created no generation. Full confirmation created one generation. Repeated reads, restart, encrypted restore, and undo preserved it. The real cache interval showed an overdue reset without provider execution or database writes. Six captures were reviewed.

The rebuilt app also passed native backup-content checks, isolated OS key-store persistence, and default-production refusal of all four fixtures. There are 21 reviewed captures across these journeys. Rust passed 184 workspace checks, strict Clippy, and formatting. The frontend passed 34 contract and 90 renderer checks. The verification record binds both desktop builds, both test tools, 202 source hashes, and ten assets. Two later harness corrections changed no product source.

Initial failures remain retained. A long temporary path prevented private D-Bus startup. A shorter private path resolved it. Two harness assertions also needed correction: stale interface copy and an isolation check that included its intentionally changed profile. The unchanged binaries passed after these corrections.

OS alert delivery, delivered-event deduplication, monitoring, real OAuth, and other-platform qualification remain incomplete.

See the [reset confirmation verification record](../evidence/native-quota-reset-confirmation/verification.json). Saved views use one Rust-owned status field. Generation identifiers stay in Rust.

## Confirmation button contrast

The global control color rule overrode coss foreground utilities. Enabled confirmation text measured 1.06:1 contrast. Removing that declaration restored the component colors without adding styles or dependencies. The full renderer check passed: 34 contracts and 90 Chromium/WebKit checks, including enabled confirmation contrast in both themes. Rust passed 184 workspace checks, formatting, and strict Clippy.

The rebuilt native application passed quota persistence, keyboard consent, encrypted restore and undo. Native backup-content checks and default fixture refusal also passed. All eighteen screenshots were reviewed. The consent button now uses its coss foreground color. This is a focused contrast correction, not full accessibility qualification. SQLCipher 4 with WAL and the normative SRS remain unchanged.

See the [native confirmation receipt](../evidence/native-button-foreground/quota/result.json) and [enabled confirmation capture](../evidence/native-button-foreground/quota/quota-association-consent-enabled-light-desktop.png). Source: `d92e88e`.
