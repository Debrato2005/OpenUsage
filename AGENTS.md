Use Linux commands directly in WSL. Check the executor and working directory first.
Never push, publish, create pull requests, or call GitHub write APIs. Commit completed slices locally.
Read docs/implementation-status.md and docs/acceptance-traceability.md before work. Check git status and preserve unrelated work.
The unchanged docs/SRS.md is normative. The user request controls execution. Record conflicts in docs/srs-clarifications.md.
Keep secrets, provider sessions, processes, storage, and exact arithmetic in Rust. Use narrow typed IPC.
Never inspect existing Codex credential homes without explicit approval. Test with temporary homes and synthetic data.
Missing metrics are not zero. Email is not a verified identity. Time alone does not confirm a reset.
Use native Tauri end-to-end tests for acceptance. Label renderer tests and synthetic fixtures clearly.
Do not claim a native build, provider capability, or platform passed without executed evidence.
Run npm run check for renderer changes. Run cargo test --workspace and cargo clippy --workspace --all-targets when Rust is available.
