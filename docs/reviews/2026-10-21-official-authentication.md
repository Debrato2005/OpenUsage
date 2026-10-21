# Official authentication decision

Reviewed October 21, 2026. Keep the local, open-source Codex-managed authentication baseline. Real authorized usage remains unverified. This review grants no account-enforcement guarantee.

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

The executed production-companion probe occupied both ports with its own listeners. It received `/cancel` on port 1455, then observed login refusal. No real login, external callback, inference request, or existing credential home was involved. The probe reaped its owned provider. See [the result](../evidence/official-authentication-review/callback-conflict-result.json) and [source hashes](../evidence/official-authentication-review/source-receipt.json).

Do not ship the browser draft with only a port precheck. That leaves a race and cannot guarantee another login remains intact. The draft remains outside the working build. Keep official device-code login while a supported callback-isolation solution remains unresolved. This is an unmet browser requirement, not a waiver of OU-ACCT-005.

## Remaining real-account gate

The post-code failure investigation found a concrete protocol mismatch: the pinned provider's [outgoing notification serializer](https://github.com/openai/codex/blob/83086188ae0614bd882818ff1aa23f3194962012/codex-rs/app-server/src/outgoing_message.rs#L904) adds `emittedAtMs`. OpenUsage rejected that envelope. The correction explicitly accepts an optional unsigned timestamp while retaining strict fields and selected-attempt matching. The RED/GREEN regression, official anonymous notification shape, production challenge/cancellation, and synthetic two-account native journeys passed. See [the bounded evidence](../evidence/native-account-signin/). A successful real user login remains unverified.

Complete official sign-in, initial supported reads, encrypted persistence, restart, refresh, and second-account isolation. Then check renewal, cancellation, and revocation with authorized sessions. Never retain passwords, challenges, tokens, or raw authentication responses as evidence. Browser isolation, protected provider storage, and sign-out remain open. SQLCipher 4/WAL and the normative SRS are unchanged.
