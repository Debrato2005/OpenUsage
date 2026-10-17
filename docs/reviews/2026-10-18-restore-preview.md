# Restore preview and checkpoint review

The native preview command authenticates the whole archive and validates its supported database records. It saves a durable encrypted checkpoint before it returns the comparison. This phase changes no application records and exposes no Apply action. AT-32 remains partial.

Rust retains the authenticated archive and its key for the selected preview. The renderer receives an opaque preview ID, a checkpoint ID, byte size, and exact record counts. It receives no database key, managed-home path, or plaintext database image. Cancelling, replacing a preview, or changing live records clears the retained archive. The checkpoint remains available as a private recovery artifact.

The checkpoint uses SQLCipher 4 and the current protected database key. ATTACH inherits key material inside SQLCipher; Rust does not retrieve it. The export includes committed WAL records. The checkpoint core commits, checks page authentication and database integrity, detaches, and synchronizes the encrypted file and its parent directory. Native publication refuses an existing destination. The live database keeps WAL mode.

Preparation checks free space on the actual staging, checkpoint, and live-workspace filesystems. Its checkpoint estimate includes journal and page overhead. The estimate does not guarantee an output size. Storage handles actual write failures and retains an unpublished private pending file if SQLite still owns an attachment.

Tauri can call process exit without running ordinary Rust destructors. The application now clears its shared archive cache during the Exit event. The cache lock stays separate from long storage operations, and a closed cache refuses a late worker result. Forced termination can skip Rust cleanup. The implementation does not claim unconditional key zeroization. See the [official event reference](https://docs.rs/tauri/latest/tauri/enum.RunEvent.html#variant.Exit) and [application source](https://github.com/tauri-apps/tauri/blob/dev/crates/tauri/src/app.rs).

The UI uses the existing font, spacing, contrast, and focus tokens. A semantic table compares checkpoint and backup counts. At enlarged text sizes, each row presents two labeled values. An em-based container query uses the available text space. Explicit table roles preserve its semantic structure when CSS changes the layout. Native DOM checks do not establish screen-reader acceptance. Password fields clear before IPC. Error messages identify the affected field or receive focus. The UI explains that the checkpoint stays encrypted with the workspace key.

`NATIVE-RESTORE-PREVIEW-001` passed on Linux. The journey exported GUI state A, changed profiles and appearance to state B, and prepared a preview of A. Its encrypted checkpoint retained exact state B and opened read-only with the original protected key. Cancellation and normal window close preserved B, the key envelope, archive, and synthetic home markers. Restart and unlock retained B and its checkpoints. Invalid archives created no checkpoint. Live writes and failed replacement cleared earlier preview tokens.

Five native captures cover desktop, both narrow themes, password errors, and 200% text scaling. The first capture exposed broken table headings at enlarged text sizes. The final build uses grouped values and passed whole-word, overflow, focus, and keyboard checks. Scaling uses test-side CSS, not OS zoom. The receipt and images are in `../evidence/native-restore-preview/`.

Integrated checks passed: 66 Rust tests, strict Clippy, formatting, 12 contract checks, and 12 renderer checks. The default configuration also passed a native startup smoke. Both native binaries are debug builds; signed release and other platforms remain unqualified.

Large-history memory use remains unqualified. Validation still needs two image-sized Rust buffers plus SQLite resources. The mandatory 5 GiB archive limit and SQLCipher architecture remain unchanged. Confirmed replacement and checkpoint recovery are the next slices.
