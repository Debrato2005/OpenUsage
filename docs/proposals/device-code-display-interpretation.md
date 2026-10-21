# SC-003: device-code challenge display

Status: approved by the user on October 18, 2026. The normative SRS remains unchanged.

OU-ACCT-005 requires documented device-code login. Section 19.1(1) prohibits secrets in frontend payloads and other outputs. The [pinned Codex contract](https://github.com/openai/codex/blob/83086188ae0614bd882818ff1aa23f3194962012/codex-rs/app-server-protocol/src/protocol/v2/account.rs#L141-L150) returns a verification URL and a human-facing user code. The [provider implementation](https://github.com/openai/codex/blob/83086188ae0614bd882818ff1aa23f3194962012/codex-rs/login/src/device_code_auth.rs#L169-L248) retains the device authorization identifier and handles token exchange and persistence.

Approved scope: after explicit consent for the selected profile, typed Rust IPC may deliver its validated URL and user code for transient display and manual copy. The code is a sensitive challenge. It is not an OAuth bearer credential. A disclosed code can still enable sign-in abuse.

Provider login IDs, device authorization secrets, OAuth tokens, authentication files, and database keys stay in Rust or the provider. This interpretation permits no other secret payload.

Keep codes out of logs, notifications, toasts, exports, backups, database records, localStorage, and other application persistence. Clear challenge state after cancellation, completion, failure, timeout, component removal, or approval invalidation. Clipboard retention remains under operating-system control.

Use labeled synthetic challenges for retained test traces and screenshots. Verify exact attempt binding and cancellation races. Real codes must stay out of retained evidence. Credential isolation, provider ownership of credentials, and the release gate remain mandatory. Real sign-in qualification remains unexecuted.
