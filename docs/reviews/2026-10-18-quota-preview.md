# Provisional quota preview

October 18, 2026. The selected profile can request a full quota preview through its approved Codex executable. Monitoring stays stopped. Results are provisional, unsaved, and excluded from combined totals. This slice does not satisfy accepted-history requirement OU-QUOTA-020.

## Provider and process boundary

Rust initializes one owned app-server process, reads the account, requests full rate limits, then reads the account again. It refuses unknown identity, unsupported scope, changed descriptive evidence, or relevant account updates during the read. Email comparison detects changes. It never establishes a verified identity. Provider account IDs, email, credentials, credits, raw frames, and process details do not cross the UI boundary.

The multi-limit map takes precedence, including an empty map. Legacy fallback requires a usable returned ID. Duplicate IDs, inconsistent IDs, invalid percentages, unsafe durations, out-of-range reset times, and unbounded text fail the read. Missing windows and timestamps stay unavailable. Sparse notifications do not replace the full response. Rust preserves integer percentages and derives remaining capacity with checked arithmetic.

The [pinned account contract](https://github.com/openai/codex/blob/83086188ae0614bd882818ff1aa23f3194962012/codex-rs/app-server-protocol/src/protocol/v2/account.rs) and [read implementation](https://github.com/openai/codex/blob/83086188ae0614bd882818ff1aa23f3194962012/codex-rs/app-server/src/request_processors/account_processor.rs) govern the parser. The application requests no inference, reserve opt-in, reset consumption, or undocumented endpoint.

The native wrapper checks the selected profile, approved source, managed home, generation, and exact request token before publication. Cancellation cannot target a successor. Ordinary connection-check cancellation cannot interrupt a quota request. Quota reads share the supervisor's two process slots. Restore, reapproval, profile mutation, and genuine window close quiesce owned processes. Waiting does not retain the workspace lock.

Observation time comes from the full provider response. A delayed completion call cannot make an old response appear fresh. No database, WAL, protected key, or profile record changes during a preview.

## Product and design

The profile actions remain above the result. Each preview identifies its profile, source observation time, and identity limitation. The first limit opens by default. Other limits and source details use disclosure. Missing values never display as zero. Reset times are expected times in UTC and do not confirm a reset.

The user selected coss/ui for all components. This slice adopts its Meter, Button, Input, and Textarea composition across existing workflows. The [adoption record](../design/coss-ui-adoption.md) describes the reviewed MIT source, pinned dependencies, token bridge, and remaining migration groups.

Native screenshot review replaced the original green used-value meter with a neutral remaining-capacity meter. Its label and percentage sit above the track. The first coss capture exposed a wrapped percent sign at 200% text. The final style keeps the numeric value together. Independent source review also corrected an undefined autofill foreground token.

A targeted renderer regression found colliding numeric React keys between sign-in and quota children. Unique sibling key prefixes prevent duplicate quota controls after remount. Generation checks also prevent a late canceled connection check from overwriting a new quota result. Failure traces remain separate from the passing regression.

The backup and sign-in harnesses originally inspected borders and outlines on bare input elements. coss renders these states on its control wrappers. Updated assertions inspect the visible wrapper while retaining focus, complete selection, invalid-state association, and border-width checks. The earlier failed receipts remain available. These changes do not relax the product checks.

## Qualification limits

Native fixtures are labeled synthetic. Real authenticated quota collection and OAuth remain unexecuted. The signed official executable regression performs a read-only account check in a fresh managed home. It does not establish official live quotas.

SQLCipher 4, WAL, protected keys, authenticated backups, and the normative SRS remain unchanged. Accepted persistence needs a separate transactional history slice with profile binding and coordinated backup compatibility. Reset confirmation, verified totals, scheduled collection, alerts, cross-platform behavior, and full screen-reader acceptance remain open.
