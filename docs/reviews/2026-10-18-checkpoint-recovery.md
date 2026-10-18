# Native checkpoint recovery

The desktop can now recover records from a saved encrypted checkpoint. The user selects a local candidate, prepares a fresh preview, and explicitly approves replacement. Rust saves another encrypted checkpoint before returning that preview. Restart invalidates prior preview tokens and requires a new review.

The candidate list contains private file metadata, not authenticated restore history. It scans at most 1,024 entries and returns at most 100 candidates. Dates describe file modification times in UTC. Missing directories stay absent. The UI does not label a candidate as the last applied restore.

Preparation opens the selected checkpoint with the existing protected SQLCipher key. It authenticates encrypted pages and validates supported records in memory. The cache retains descriptors, home mappings, size, and schema metadata. It does not retain the plaintext database image while the user considers the preview. Apply rereads and revalidates the encrypted source before starting replacement.

Checkpoint recovery uses the exact original home mappings. Private directories need owner read, write, and search permissions. Rust checks retained descriptors before preparation, after checkpoint publication, and before the write transaction. It reads no credential contents. Missing, replaced, or unusable homes stop recovery. A live home reference cannot move to a different profile. Archive imports still require fresh homes.

The original and imported homes remain on disk. Restored profiles require authentication, and monitoring stays stopped. The transaction checks its revision under the write lock and validates the resulting records before commit. The checkpoint created before recovery can recover the earlier state again.

`NATIVE-CHECKPOINT-RECOVERY-001` passed on the actual Tauri application. The journey first restored backup A, restarted, and selected the checkpoint containing B. Explicit confirmation recovered B into its original three marker homes. Two imported empty homes remained private and intact. Restart retained the selected B fields, original protected key, and WAL. Stale previews, corrupt checkpoints, and replaced homes preserved current records. Independent read-only checks authenticated pages and compared the expected original mappings.

Seven captures cover desktop, narrow views, and enlarged text. Review confirmed the catalog width, focus, comparison hierarchy, and clear approval text. At 200% text size, the comparison becomes labeled Current/Checkpoint pairs. Natural labels remain whole, keyboard Tab reaches the unchecked box, and Apply remains disabled. Scaling uses temporary test-side CSS. It does not establish OS zoom or complete screen-reader acceptance.

The final debug recovery-to-focus measurement was 126.5 ms, including driver and polling time. It does not qualify release performance. All recorded source, harness, executable, encrypted file, and image hashes passed the artifact audit. The earlier normal-size run remains preserved separately.

The integrated checks passed: 81 Rust tests, strict Clippy, formatting, 15 IPC contract checks, and 12 renderer checks. The default production configuration also compiled and passed its native startup smoke. Both executables are debug builds. SQLCipher 4 with WAL and the unchanged SRS remain mandatory. Large-history memory use, process interruption, live providers, other platforms, and signed releases need separate qualification.
