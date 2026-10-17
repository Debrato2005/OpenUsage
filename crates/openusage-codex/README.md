# Codex account protocol boundary

This crate builds approved account requests and parses bounded responses in Rust. Its optional one-shot probe starts an isolated provider process.

Protocol research baseline: `rust-v0.163.0`. Every fixture under `tests/fixtures` is synthetic. These fixtures do not establish live provider support or native acceptance.

Account email gives provisional identity only. A missing email gives unknown identity. The baseline supplies no approved stable identity contract.

Login completion must match the pending login ID. A successful event requires a separate account read. Logout requests require explicit caller confirmation.

Activity requests omit `threadId`. Counts leave this crate as exact decimal strings. Null values remain unavailable. Application scope remains unknown.

Sources:

- [Official app-server documentation](https://learn.chatgpt.com/docs/app-server)
- [Versioned protocol schemas](https://github.com/openai/codex/tree/rust-v0.163.0/codex-rs/app-server-protocol/schema/typescript)

## Isolated provider probe

`session::probe_account` takes an approved absolute executable path and an existing private managed directory. It creates fresh temporary homes and removes them after the probe. It never opens an existing provider home.

The blocking probe checks the executable version, completes `initialize` and `initialized`, then sends `account/read` with `refreshToken: false`. Readiness permits 15 seconds. Account reads permit 30 seconds. Each response permits 1 MiB, with at most 32 unrelated notifications and 4 MiB per request. The probe discards notifications without actions.

The child receives a cleared environment and fixed private HOME, CODEX_HOME, XDG, and temporary directories. Fixed options select file-only credentials and disable analytics, feedback, update checks, and telemetry exporters. Provider stderr is discarded. Failures expose typed reasons without provider messages.

On Unix, the probe owns a separate child process group. Completion, failure, and normal Rust cleanup terminate that group and reap the child. Other platforms return `UnsupportedPlatform`. The caller must approve executable provenance and enforce application authorization.

Run the example with reviewed absolute paths:

```text
cargo run --manifest-path crates/openusage-codex/Cargo.toml --example isolated_probe -- /approved/codex /private/managed-parent
```

The real Linux probe passed with Codex CLI 0.163.0 on October 18, 2026. It returned an unauthenticated account and unknown identity. The child, process group, and temporary homes were absent afterward. Unit transport fixtures remain synthetic. Native integration, authenticated collection, and a persistent process supervisor remain unimplemented.

Additional pinned sources:

- [File-only credential storage](https://github.com/openai/codex/blob/rust-v0.163.0/codex-rs/login/src/auth/storage.rs)
- [Provider configuration schema](https://github.com/openai/codex/blob/rust-v0.163.0/codex-rs/core/config.schema.json)
- [Server notification schema](https://github.com/openai/codex/blob/rust-v0.163.0/codex-rs/app-server-protocol/schema/typescript/ServerNotification.ts)
