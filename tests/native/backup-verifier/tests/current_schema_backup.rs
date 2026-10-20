#![cfg(unix)]

use openusage_import::{Entry, Manifest, encrypt};
use openusage_storage::{Storage, Theme};
use rusqlite::Connection;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    io::{Cursor, Write},
    os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    time::{SystemTime, UNIX_EPOCH},
};
use zeroize::Zeroizing;

struct Temporary(PathBuf);
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn snapshot(root: &Path) -> BTreeMap<PathBuf, (u32, Option<Vec<u8>>)> {
    fn visit(root: &Path, path: &Path, output: &mut BTreeMap<PathBuf, (u32, Option<Vec<u8>>)>) {
        for entry in fs::read_dir(path).unwrap() {
            let path = entry.unwrap().path();
            let metadata = fs::symlink_metadata(&path).unwrap();
            assert!(!metadata.file_type().is_symlink());
            output.insert(
                path.strip_prefix(root).unwrap().into(),
                (
                    metadata.permissions().mode(),
                    metadata.is_file().then(|| fs::read(&path).unwrap()),
                ),
            );
            if metadata.is_dir() {
                visit(root, &path, output);
            }
        }
    }
    let mut output = BTreeMap::new();
    visit(root, root, &mut output);
    output
}

fn archive(path: &Path, schema: u32, payload: &[u8], password: &[u8]) {
    let manifest = Manifest {
        schema_version: schema,
        entries: vec![Entry {
            name: "records.sqlite".into(),
            size: payload.len() as u64,
        }],
    };
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(path)
        .unwrap();
    encrypt(
        &manifest,
        &mut Cursor::new(payload),
        &mut file,
        password,
        u64::MAX,
    )
    .unwrap();
    file.sync_all().unwrap();
}

fn execute(path: &Path, staging: &Path, input: &[u8]) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_openusage-native-backup-verifier"))
        .args([path.as_os_str(), staging.as_os_str(), "dark".as_ref()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(input).unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn exported_schema_four_requires_matching_explicit_manifest_and_database_versions() {
    let root = Temporary(std::env::temp_dir().join(format!(
            "openusage-backup-schema-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        )));
    fs::DirBuilder::new().mode(0o700).create(&root.0).unwrap();
    let staging = root.0.join("staging");
    fs::DirBuilder::new().mode(0o700).create(&staging).unwrap();
    let live = root.0.join("live.db");
    let mut storage = Storage::create(&live, &[0x58; 32]).unwrap();
    storage.set_theme(Theme::Dark).unwrap();
    let snapshot_data = storage.backup_snapshot().unwrap();
    assert_eq!(snapshot_data.schema_version, 4);
    assert!(!fs::read(&live).unwrap().starts_with(b"SQLite format 3\0"));
    let password = "OpenUsage synthetic backup schema password";
    let valid = root.0.join("schema4.oubackup");
    archive(&valid, 4, &snapshot_data.bytes, password.as_bytes());
    let wrong_manifest = root.0.join("manifest1.oubackup");
    archive(
        &wrong_manifest,
        1,
        &snapshot_data.bytes,
        password.as_bytes(),
    );
    let mut database = Connection::open_in_memory().unwrap();
    database
        .pragma_update(None, "temp_store", "MEMORY")
        .unwrap();
    database
        .deserialize_read_exact(
            "main",
            Cursor::new(snapshot_data.bytes.as_slice()),
            snapshot_data.bytes.len(),
            false,
        )
        .unwrap();
    database.pragma_update(None, "user_version", 1).unwrap();
    let old_schema = Zeroizing::new(database.serialize("main").unwrap().to_vec());
    let wrong_database = root.0.join("database1.oubackup");
    archive(&wrong_database, 4, &old_schema, password.as_bytes());
    drop(database);
    let before = snapshot(&root.0);
    let request = json!({"password":password, "expectedSchemaVersion":4});
    let accepted = execute(&valid, &staging, &serde_json::to_vec(&request).unwrap());
    assert!(
        accepted.status.success(),
        "Current-schema export verification failed: {}",
        String::from_utf8_lossy(&accepted.stderr)
    );
    let report: Value = serde_json::from_slice(&accepted.stdout).unwrap();
    assert_eq!(
        report,
        json!({"authenticated":true,"wrongPasswordRejected":true,"tamperRejected":true,"schemaVersion":4,"profiles":0,"plaintextDiskStaging":false})
    );
    assert!(
        snapshot(&root.0) == before,
        "Successful export verification changed synthetic files or modes."
    );
    let mut mismatch = request.clone();
    mismatch["expectedSchemaVersion"] = json!(1);
    for (path, input) in [
        (&valid, mismatch),
        (&wrong_manifest, request.clone()),
        (&wrong_database, request.clone()),
    ] {
        let refused = execute(path, &staging, &serde_json::to_vec(&input).unwrap());
        assert!(
            !refused.status.success(),
            "A schema mismatch must be refused."
        );
        assert!(refused.stdout.is_empty());
        assert!(String::from_utf8_lossy(&refused.stderr).contains("schema version"));
        assert!(
            snapshot(&root.0) == before,
            "Schema refusal changed synthetic files or modes."
        );
    }
    for invalid in [
        None,
        Some(json!(null)),
        Some(json!("4")),
        Some(json!(-1)),
        Some(json!(4.5)),
        Some(json!(4294967296_u64)),
    ] {
        let mut malformed = request.clone();
        match invalid {
            Some(value) => {
                malformed["expectedSchemaVersion"] = value;
            }
            None => {
                malformed
                    .as_object_mut()
                    .unwrap()
                    .remove("expectedSchemaVersion");
            }
        }
        let refused = execute(&valid, &staging, &serde_json::to_vec(&malformed).unwrap());
        assert!(
            !refused.status.success(),
            "The expected schema must be an explicit u32."
        );
        assert!(String::from_utf8_lossy(&refused.stderr).contains("expected schema version"));
        assert!(refused.stdout.is_empty());
        assert!(
            snapshot(&root.0) == before,
            "Invalid schema input changed synthetic files or modes."
        );
    }
    let refused = execute(&valid, &staging, b"not a JSON request");
    assert!(!refused.status.success());
    assert!(
        snapshot(&root.0) == before,
        "Malformed JSON changed synthetic files or modes."
    );
    assert!(fs::read_dir(&staging).unwrap().next().is_none());
}
