# Codex account protocol boundary

This crate builds approved account requests and parses bounded responses in Rust. It starts no process and reads no credential files.

Protocol research baseline: `rust-v0.163.0`. Every fixture under `tests/fixtures` is synthetic. These fixtures do not establish live provider support or native acceptance.

Account email gives provisional identity only. A missing email gives unknown identity. The baseline supplies no approved stable identity contract.

Login completion must match the pending login ID. A successful event requires a separate account read. Logout requests require explicit caller confirmation.

Activity requests omit `threadId`. Counts leave this crate as exact decimal strings. Null values remain unavailable. Application scope remains unknown.

Sources:

- [Official app-server documentation](https://learn.chatgpt.com/docs/app-server)
- [Versioned protocol schemas](https://github.com/openai/codex/tree/rust-v0.163.0/codex-rs/app-server-protocol/schema/typescript)

The caller must enforce the process handshake, selected-profile isolation, approved executable version, timeouts, and application authorization.
