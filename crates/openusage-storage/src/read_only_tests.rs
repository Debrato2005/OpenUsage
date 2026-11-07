//! The read-only open that the CLI uses: no migration, no write, and the latest committed data.

use crate::{LogConsent, Storage, StorageError};

const KEY: [u8; 32] = [61; 32];
const NOW: &str = "2026-11-08T08:00:00.000Z";

/// File names with their length and a digest, so a failure prints a short list.
fn tree(dir: &std::path::Path) -> Vec<(String, usize, u64)> {
    use std::hash::{DefaultHasher, Hash, Hasher};
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap())
        .map(|entry| {
            let bytes = std::fs::read(entry.path()).unwrap();
            let mut hasher = DefaultHasher::new();
            bytes.hash(&mut hasher);
            (
                entry.file_name().to_string_lossy().into_owned(),
                bytes.len(),
                hasher.finish(),
            )
        })
        .collect();
    files.sort();
    files
}

#[test]
fn read_only_open_reads_committed_data_and_never_writes() {
    let dir = crate::tests::private_directory();
    let path = dir.path().join("cli.db");
    let mut writer = Storage::create(&path, &KEY).unwrap();
    writer
        .set_log_source(
            "claude",
            "/home/synthetic/.claude",
            LogConsent::Granted,
            NOW,
        )
        .unwrap();
    // With the app open, a reader sees committed rows that are still in the WAL.
    let reader = Storage::open_read_only(&path, &KEY).unwrap();
    assert_eq!(reader.log_sources().unwrap().len(), 1);
    drop(reader);
    drop(writer);
    // With the app closed, the reader opens the database and leaves every file unchanged.
    let before = tree(dir.path());
    let mut reader = Storage::open_read_only(&path, &KEY).unwrap();
    assert_eq!(reader.log_sources().unwrap()[0].tool, "claude");
    assert!(
        reader
            .set_log_source("codex", "/home/synthetic/.codex", LogConsent::Granted, NOW)
            .is_err()
    );
    drop(reader);
    assert_eq!(tree(dir.path()), before);
    assert!(matches!(
        Storage::open_read_only(&path, &[62; 32]),
        Err(StorageError::UnlockFailed)
    ));
    // Only the desktop app migrates, so another schema version is refused.
    let writer = Storage::open(&path, &KEY).unwrap();
    writer
        .connection
        .pragma_update(None, "user_version", 9)
        .unwrap();
    drop(writer);
    assert!(matches!(
        Storage::open_read_only(&path, &KEY),
        Err(StorageError::UnsupportedSchema)
    ));
    assert!(matches!(
        Storage::open_read_only(&dir.path().join("missing.db"), &KEY),
        Err(StorageError::OpenFailed)
    ));
}
