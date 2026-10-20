#![cfg(unix)]

use openusage_storage::{Storage, Theme, keywrap::wrap_key};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    time::{SystemTime, UNIX_EPOCH},
};

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
            let bytes = metadata.is_file().then(|| fs::read(&path).unwrap());
            output.insert(
                path.strip_prefix(root).unwrap().into(),
                (metadata.permissions().mode(), bytes),
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

fn assert_unchanged(
    before: &BTreeMap<PathBuf, (u32, Option<Vec<u8>>)>,
    after: &BTreeMap<PathBuf, (u32, Option<Vec<u8>>)>,
    allow_live_read_metadata: bool,
    label: &str,
) {
    let mut after = after.clone();
    if allow_live_read_metadata {
        let shm = PathBuf::from("live.db-shm");
        if let (Some(original), Some(current)) = (before.get(&shm), after.get(&shm)) {
            assert_eq!(original.0, current.0, "Shared-memory permissions changed.");
            after.insert(shm, original.clone());
        }
    }
    let changed: Vec<_> = before
        .keys()
        .chain(after.keys())
        .filter(|path| before.get(*path) != after.get(*path))
        .collect();
    assert!(changed.is_empty(), "{label}: {changed:?}");
}

fn execute(binary: &str, database: &Path, envelope: &Path, request: &Value) -> Output {
    let mut child = Command::new(binary)
        .args([database, envelope])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(&serde_json::to_vec(request).unwrap())
        .unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn current_schema_is_explicit_and_mismatch_refusal_preserves_the_synthetic_workspace() {
    let root = Temporary(std::env::temp_dir().join(format!(
        "openusage-verifier-schema-{}-{}",
        std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
    )));
    fs::DirBuilder::new().mode(0o700).create(&root.0).unwrap();
    let live = root.0.join("live.db");
    let checkpoint = root.0.join("checkpoint.db");
    let envelope = root.0.join("database-key.v1");
    let key = [0x58; 32];
    let passphrase = "OpenUsage synthetic verifier schema passphrase";
    let mut wrapped = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(&envelope)
        .unwrap();
    wrapped
        .write_all(&wrap_key(&key, passphrase.as_bytes()).unwrap())
        .unwrap();
    drop(wrapped);
    let id = "00000001-0000-4000-8000-000000000000";
    let home = root.0.join("profiles").join(id);
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&home)
        .unwrap();
    let mut storage = Storage::create(&live, &key).unwrap();
    assert_eq!(storage.metadata().unwrap().schema_version, 4);
    storage.set_theme(Theme::Dark).unwrap();
    storage
        .create_profile(id, "Synthetic current-schema profile", id)
        .unwrap();
    storage.create_encrypted_checkpoint(&checkpoint).unwrap();
    assert!(!fs::read(&live).unwrap().starts_with(b"SQLite format 3\0"));
    assert!(
        !fs::read(&checkpoint)
            .unwrap()
            .starts_with(b"SQLite format 3\0")
    );
    assert!(
        !fs::read(PathBuf::from(format!("{}-wal", live.display())))
            .unwrap()
            .is_empty()
    );
    let request = json!({
        "passphrase": passphrase,
        "expectedSchemaVersion": 4,
        "expected": {"theme":"dark", "accounts":"0", "profiles":[{
            "id":id, "label":"Synthetic current-schema profile", "archived":false,
            "environment":"native", "connectionState":"authentication_required"
        }]},
        "homeRoot": root.0,
        "homeMode":"fresh",
        "forbiddenHomes":[]
    });
    for (binary, path) in [
        (
            env!("CARGO_BIN_EXE_openusage-native-checkpoint-verifier"),
            &checkpoint,
        ),
        (env!("CARGO_BIN_EXE_openusage-native-live-verifier"), &live),
    ] {
        let before = snapshot(&root.0);
        let accepted = execute(binary, path, &envelope, &request);
        assert!(
            accepted.status.success(),
            "{binary}: {}",
            String::from_utf8_lossy(&accepted.stderr)
        );
        let report: Value = serde_json::from_slice(&accepted.stdout).unwrap();
        assert_eq!(report["schemaVersion"], 4);
        for field in [
            "verified",
            "readOnly",
            "selectedSemanticFieldsMatched",
            "originalProtectedKeyUsed",
            "encryptedPagesAuthenticated",
        ] {
            assert_eq!(report[field], true, "{binary}: {field}");
        }
        assert_eq!(report["plaintextDiskStaging"], false);
        let after = snapshot(&root.0);
        if path == &live {
            let shm = PathBuf::from("live.db-shm");
            assert_eq!(
                report["sharedMemoryMetadataChanged"],
                before[&shm] != after[&shm]
            );
        }
        assert_unchanged(
            &before,
            &after,
            path == &live,
            "Successful verification changed synthetic files",
        );
        let before = after;
        let mut mismatch = request.clone();
        mismatch["expectedSchemaVersion"] = json!(1);
        let refused = execute(binary, path, &envelope, &mismatch);
        assert!(
            !refused.status.success(),
            "A schema-4 database must refuse expected schema 1."
        );
        assert!(String::from_utf8_lossy(&refused.stderr).contains("schema version"));
        assert!(refused.stdout.is_empty());
        assert_unchanged(
            &before,
            &snapshot(&root.0),
            path == &live,
            "Schema mismatch changed synthetic files",
        );
        let before = snapshot(&root.0);
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
            let refused = execute(binary, path, &envelope, &malformed);
            assert!(
                !refused.status.success(),
                "The expected schema must be an explicit u32."
            );
            assert!(String::from_utf8_lossy(&refused.stderr).contains("expected schema version"));
            assert!(refused.stdout.is_empty());
            assert_unchanged(
                &before,
                &snapshot(&root.0),
                false,
                "Invalid schema request changed synthetic files",
            );
        }
    }
}
