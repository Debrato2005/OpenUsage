# SRS clarification record

## SC-001: reset confirmation with rapid new-window use

Status: proposed, not approved. No normative text or runtime reset rule changed.

SRS section 6.2 condition 4 requires a lower usage percentage before reset confirmation. Master Build Prompt D1 identifies a missed-reset case: new-window usage can already equal or exceed the prior reading. A missing percentage drop therefore cannot prove that no reset occurred.

The reviewed [Codex window schema](https://github.com/openai/codex/blob/rust-v0.163.0/codex-rs/app-server-protocol/schema/typescript/v2/RateLimitWindow.ts) contains no explicit generation identifier. The [quota response](https://github.com/openai/codex/blob/rust-v0.163.0/codex-rs/app-server-protocol/schema/typescript/v2/GetAccountRateLimitsResponse.ts) does not establish a replacement reset-proof contract.

Proposed narrow addition: distinguish an overdue expected reset from a reset whose completion evidence is unavailable. Continue bounded full-read reconciliation and preserve the latest measurement. Neither state permits a reset-completed alert or a new durable alert generation. Keep the five existing confirmation conditions until an approved provider-backed rule replaces them.

Implementation and migration remain not started. The future Rust state machine must preserve identity and bucket compatibility, stale-reading rejection, and alert deduplication across restarts. Acceptance must cover rapid new-window use, later confirmation, unknown reset times, sparse updates, clock changes, and repeated reads.

This proposal does not authorize treating elapsed time, a moved timestamp, or changed percentage alone as confirmation.


## SC-002: interruption after restore COMMIT and before acknowledgment

Status: implementation interpretation recorded for acceptance review. No normative text changed. Power-loss and native interruption acceptance remain open.

OU-BACKUP-010 requires the previous valid database to survive a failed or interrupted restore. OU-NFR-007 requires transactional writes. Before COMMIT, the previous live records survive. After COMMIT but before acknowledgment, complete restored records survive, while the encrypted pre-restore checkpoint preserves the previous records. A lost reply does not prove rollback.

The owned-process tests execute the production transaction and terminate their child before COMMIT and after COMMIT without acknowledgment. Both cases retain the original encrypted checkpoint. The user can prepare a new preview after restart rather than retry a consumed token. This interpretation does not permit partial records, deletion of the checkpoint, or automatic confirmation. See [the review](reviews/2026-10-18-restore-process-crash.md).
