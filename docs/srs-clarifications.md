# SRS clarification record

## SC-001: reset confirmation with rapid new-window use

Status: proposed, not approved. No normative text or runtime reset rule changed.

SRS section 6.2 condition 4 requires a lower usage percentage before reset confirmation. Master Build Prompt D1 identifies a missed-reset case: new-window usage can already equal or exceed the prior reading. A missing percentage drop therefore cannot prove that no reset occurred.

The reviewed [Codex window schema](https://github.com/openai/codex/blob/rust-v0.163.0/codex-rs/app-server-protocol/schema/typescript/v2/RateLimitWindow.ts) contains no explicit generation identifier. The [quota response](https://github.com/openai/codex/blob/rust-v0.163.0/codex-rs/app-server-protocol/schema/typescript/v2/GetAccountRateLimitsResponse.ts) does not establish a replacement reset-proof contract.

Proposed narrow addition: distinguish an overdue expected reset from a reset whose completion evidence is unavailable. Continue bounded full-read reconciliation and preserve the latest measurement. Neither state permits a reset-completed alert or a new durable alert generation. Keep the five existing confirmation conditions until an approved provider-backed rule replaces them.

Implementation and migration remain not started. The future Rust state machine must preserve identity and bucket compatibility, stale-reading rejection, and alert deduplication across restarts. Acceptance must cover rapid new-window use, later confirmation, unknown reset times, sparse updates, clock changes, and repeated reads.

This proposal does not authorize treating elapsed time, a moved timestamp, or changed percentage alone as confirmation.
