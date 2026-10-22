> Public source copy: raw acceptance evidence remains private. Historical test statements keep their original scope and do not certify this source snapshot.

# Official authentication decision

Reviewed October 21, 2026. Keep the local, open-source Codex-managed authentication baseline. One real authorized account now passes official login and quota/activity reads in the native app. Full authentication acceptance remains incomplete. This review grants no account-enforcement guarantee.

## Supported baseline

OpenAI explicitly permits local or open-source applications to continue with app-server authentication. Commercial or hosted use is outside that permission. Managed ChatGPT mode owns OAuth, persistence, and renewal. Browser and device-code login are documented. Both account quota and token-activity reads are documented. API-key-only authentication cannot supply the latter. See [the official account interface](https://learn.chatgpt.com/docs/app-server#auth-endpoints).

OpenUsage identifies itself as `openusage` / `OpenUsage`, with its actual version. Its companion runs in a separate persistent CODEX_HOME for each account. It does not depend on another Codex client remaining open. It does not import credentials, borrow an OAuth client identity, or call private provider endpoints. Its protocol exposes no inference or limit-reset mutation. Existing supervision serializes operations for each profile. Monitoring retains conservative polling and provider backoff.

## Sign in with ChatGPT comparison

The [quickstart](https://developers.openai.com/siwc/quickstart) separates identity scopes from Responses API permission. The [app-server integration](https://developers.openai.com/siwc/token-sharing-open-source/codex-app-server) configures an OAuth access token as a Responses provider. That configuration does not establish Codex-service authorization for the two required usage reads. Compatibility remains unproven, rather than unsupported by assumption.

Do not migrate, borrow a client ID, or request inference permission merely to test quota access. Do not implement experimental external-token login. SIWC would transfer token handling to OpenUsage and needs a separate capability and license review. No SIWC SDK code was copied or installed.

[OpenCode's published flow](https://opencode.ai/docs/providers/#openai) offers Connect, OpenAI, and browser authentication. Its simplicity is useful. Its inference support does not prove OpenUsage's usage-read compatibility. No OpenCode credentials or implementation were imported.

## Persistent renewal and storage

The pinned [quota and activity handlers](https://github.com/openai/codex/blob/83086188ae0614bd882818ff1aa23f3194962012/codex-rs/app-server/src/request_processors/account_processor.rs#L1192) call the provider's authentication manager. Its [managed refresh path](https://github.com/openai/codex/blob/83086188ae0614bd882818ff1aa23f3194962012/codex-rs/login/src/auth/manager.rs#L2387) checks token age or expiry and persists replacements. OpenUsage must not force refresh on every reading or duplicate that logic. Source confirmation does not prove renewal with a real session.

The [credential backend](https://github.com/openai/codex/blob/83086188ae0614bd882818ff1aa23f3194962012/codex-rs/login/src/auth/storage.rs#L243) scopes its key to the canonical CODEX_HOME. Explicit keyring mode reports failure. Auto mode can fall back to a file. OpenUsage currently forces provider-managed file storage and clears the child environment, including the session bus. OS-backed provider storage therefore remains incomplete. Implement explicit protected storage with narrow session-bus access and recovery before claiming that requirement passed. Never copy existing authentication files into another home.

## Reproduced browser isolation defect

Pinned browser login defaults to fixed callback ports. On an occupied preferred port, [the official listener](https://github.com/openai/codex/blob/83086188ae0614bd882818ff1aa23f3194962012/codex-rs/login/src/server.rs#L620) sends `/cancel` to the existing listener. The [app-server options](https://github.com/openai/codex/blob/83086188ae0614bd882818ff1aa23f3194962012/codex-rs/app-server/src/request_processors/account_processor.rs#L584) expose no callback-port override. [Upstream issue 27618](https://github.com/openai/codex/issues/27618) separately reports the fixed-port limitation.

The executed production-companion probe occupied both ports with its own listeners. It received `/cancel` on port 1455, then observed login refusal. No real login, external callback, inference request, or existing credential home was involved. The probe reaped its owned provider. See the result (private evidence omitted) and source hashes (private evidence omitted).

Do not ship the browser draft with only a port precheck. That leaves a race and cannot guarantee another login remains intact. The draft remains outside the working build. Keep official device-code login while a supported callback-isolation solution remains unresolved. This is an unmet browser requirement, not a waiver of OU-ACCT-005.

## Remaining real-account gate

October 21 follow-up: the user completed official login. The next real collection exposed the provider's initial routing snapshot, which OpenUsage incorrectly rejected before reading any measurements. The corrected startup barrier preserves later account-change rejection. A Rust-owned collection and the native app then accepted real quota and activity using saved authorization. The user confirmed the readings. See the source-backed correction (private evidence omitted). Genuine native cache restart passed with identical saved-reading digests. A second real account remains pending.

The post-code failure investigation found a concrete protocol mismatch: the pinned provider's [outgoing notification serializer](https://github.com/openai/codex/blob/83086188ae0614bd882818ff1aa23f3194962012/codex-rs/app-server/src/outgoing_message.rs#L904) adds `emittedAtMs`. OpenUsage rejected that envelope. The correction explicitly accepts an optional unsigned timestamp while retaining strict fields and selected-attempt matching. The RED/GREEN regression, official anonymous notification shape, production challenge/cancellation, and synthetic two-account native journeys passed. See the bounded evidence (private evidence omitted). That earlier slice did not establish real OAuth; the follow-up above records the successful real-account check.

Complete second-account isolation and the remaining authorized-session lifecycle checks. Then check renewal, cancellation, and revocation with authorized sessions. Never retain passwords, challenges, tokens, or raw authentication responses as evidence. Browser isolation, protected provider storage, and sign-out remain open. SQLCipher 4/WAL and the normative SRS are unchanged.


## October 22: selected-account sign-out

The user confirmed two real accounts and their saved readings after restart. This supersedes the earlier second-account pending note. The new sign-out journey uses synthetic accounts. It does not establish real OAuth revocation. Protected provider storage, renewal, revocation and safe browser callback isolation remain open.

Per-account Sign out uses the official [account/logout method](https://developers.openai.com/codex/app-server) after explicit consent. Rust reserves only the selected profile, checks the approved source and held home, and waits outside the workspace lock. A strict empty acknowledgement and a signed-out account read precede the saved status change. Cached readings and history remain. Other profiles retain their authorization and records. OpenAI can attempt session revocation. OpenUsage does not promise that revocation succeeded.

The pinned Linux companion passed a separate local keyring probe with internet sockets blocked. Two fresh synthetic credentials stayed isolated across child restarts. Exact deletion preserved the second and unrelated entries. This proves local keyring feasibility only. Product provider storage still uses isolated provider-managed files. Protected provider storage, real token renewal and revocation remain open.


## October 22: protected provider storage on Linux

New empty Linux profiles use the pinned official companion with strict OS credential storage when a validated desktop bus and unlocked store are available. A private, non-secret marker preserves that choice. Existing file-backed profiles keep their sessions. Protected profiles refuse an unavailable store instead of falling back to files. Authentication, tokens and refresh remain provider-managed. No credential enters React, SQLCipher, exports or backups.

Confirmed local deletion removes exact provider entries before removing marked homes. Cleanup includes archived and unmapped retained homes. It reads no secrets, refuses locked or ambiguous matches and preserves unrelated entries. An unavailable store leaves the deletion blocker and managed homes for a confirmed retry. Each OS-store operation has a five-second deadline.

The native check uses the production build and pinned official companion with nonfunctional synthetic API-key entries in a fresh private OS store. It tests retrieval, restart, selected logout, unavailable-store refusal and exact cleanup. It does not establish ChatGPT OAuth persistence, renewal, real revocation or usage collection through this backend. Existing real-account file-backed acceptance retains its recorded scope. macOS and Windows provider keyring support remain open.

Native verification found that immediate provider termination left official temporary helper symlinks. Strict local deletion refused them. The source-backed correction closes stdin and permits one second for normal cleanup before the existing group-kill fallback. Cancellation skips this grace period. Exit observation preserves the owned PID before reaping, and forced reaping has its own deadline. Unknown links remain refused. Older helper remnants still require a successful authorized provider operation before deletion.

Sources: [official credential storage](https://learn.chatgpt.com/docs/auth#credential-storage), pinned [direct store and home-derived namespace](https://github.com/openai/codex/blob/83086188ae0614bd882818ff1aa23f3194962012/codex-rs/login/src/auth/storage.rs), [EOF shutdown](https://github.com/openai/codex/blob/83086188ae0614bd882818ff1aa23f3194962012/codex-rs/app-server-transport/src/transport/stdio.rs#L117), and [temporary helper lifecycle](https://github.com/openai/codex/blob/83086188ae0614bd882818ff1aa23f3194962012/codex-rs/arg0/src/lib.rs#L307). Private evidence: `provider-keyring-production-eof-native`.
