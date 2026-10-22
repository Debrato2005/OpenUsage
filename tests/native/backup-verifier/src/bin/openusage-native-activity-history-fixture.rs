//! Bounded synthetic summary snapshots in a marker-owned, closed native workspace.
use openusage_storage::{
    ActivityDayValue, ActivityReading, ActivitySourceKind, ActivitySummaryValues,
    CURRENT_SCHEMA_VERSION, ProfileBinding, ProfileBindingInput, Storage, keywrap::unwrap_key,
};
use serde_json::{Value, json};
use std::{
    env, fs,
    io::{self, Read},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
};
use zeroize::Zeroizing;

fn uuid(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(index, byte)| {
            if [8, 13, 18, 23].contains(&index) {
                byte == b'-'
            } else {
                byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)
            }
        })
}
fn directory(path: &Path) {
    let metadata = fs::symlink_metadata(path).unwrap();
    assert!(
        metadata.is_dir()
            && !metadata.file_type().is_symlink()
            && metadata.uid() == unsafe { libc::geteuid() }
            && metadata.mode() & 0o777 == 0o700
    );
    assert_eq!(path.canonicalize().unwrap(), path);
}
fn regular(path: &Path) -> fs::File {
    let before = fs::symlink_metadata(path).unwrap();
    assert!(before.is_file() && !before.file_type().is_symlink());
    let file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .unwrap();
    let metadata = file.metadata().unwrap();
    assert!(
        metadata.is_file()
            && metadata.uid() == unsafe { libc::geteuid() }
            && metadata.mode() & 0o777 == 0o600
            && metadata.nlink() == 1
    );
    file
}
fn private_bytes(path: &Path) -> Vec<u8> {
    let mut file = regular(path);
    assert!(file.metadata().unwrap().len() <= 4096);
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).unwrap();
    bytes
}
fn main() {
    let args: Vec<_> = env::args_os().collect();
    assert_eq!(
        args.len(),
        3,
        "Supply the private fixture root and selected profile UUID."
    );
    let root = PathBuf::from(&args[1]);
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .unwrap();
    assert!(
        root.is_absolute()
            && root.parent() == Some(repository.join(".recovery/tmp").as_path())
            && root
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("n-")
    );
    directory(&root);
    let profile = args[2].to_str().unwrap();
    assert!(uuid(profile));
    directory(&root.join("control"));
    let marker: Value = serde_json::from_slice(&private_bytes(
        &root.join("control/activity-history-fixture.json"),
    ))
    .unwrap();
    let initial_run = marker["initialRunId"].as_str().unwrap();
    assert!(uuid(initial_run));
    assert_eq!(
        marker,
        json!({"purpose":"openusage-native-activity-history-v1","profileId":profile,"initialRunId":initial_run})
    );
    let workspace = root.join("data/app.openusage.desktop/workspace");
    for path in [
        root.join("data"),
        root.join("data/app.openusage.desktop"),
        workspace.clone(),
        workspace.join("profiles"),
    ] {
        directory(&path);
    }
    let mut input = Zeroizing::new(String::new());
    io::stdin().take(4097).read_to_string(&mut input).unwrap();
    assert!(input.len() <= 4096);
    let mut request: Value = serde_json::from_str(&input).unwrap();
    assert!(request.as_object().is_some_and(|value| value.len() == 1));
    let password = match request.get_mut("passphrase").unwrap().take() {
        Value::String(value) => Zeroizing::new(value),
        _ => panic!("Invalid synthetic password."),
    };
    let envelope = Zeroizing::new(private_bytes(&workspace.join("database-key.v1")));
    let key = unwrap_key(&envelope, password.as_bytes()).unwrap();
    let database = workspace.join("openusage.db");
    drop(regular(&database));
    for suffix in ["-wal", "-shm"] {
        let path = workspace.join(format!("openusage.db{suffix}"));
        match fs::symlink_metadata(&path) {
            Ok(_) => drop(regular(&path)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(_) => panic!("Cannot check the synthetic database sidecar."),
        }
    }
    let mut storage = Storage::open(&database, &key).unwrap();
    assert_eq!(
        storage.metadata().unwrap().schema_version,
        CURRENT_SCHEMA_VERSION
    );
    assert_eq!(CURRENT_SCHEMA_VERSION, 8);
    assert!(
        storage
            .list_profiles()
            .unwrap()
            .iter()
            .any(|item| item.id == profile && !item.archived)
    );
    let saved = storage.current_activity(profile).unwrap().unwrap();
    assert_eq!(saved.source_kind, ActivitySourceKind::TestFixture);
    assert_eq!(saved.provider_version, "0.163.0");
    assert_eq!(saved.commit.collection_run_id, initial_run);
    let revision = storage.restore_state().unwrap().revision;
    let mappings = storage.restore_home_mappings(&revision).unwrap();
    let mapped = mappings
        .iter()
        .find(|item| item.profile_id == profile)
        .unwrap();
    assert!(uuid(&mapped.managed_home_reference));
    assert_eq!(saved.binding.home_reference, mapped.managed_home_reference);
    directory(
        &workspace
            .join("profiles")
            .join(&mapped.managed_home_reference),
    );
    let original_evidence = storage.profile_binding_evidence(&saved.binding).unwrap();
    let mut alternate = saved.binding.binding_id.clone();
    let prefix = if alternate.starts_with('a') { "b" } else { "a" };
    alternate.replace_range(..1, prefix);
    let mut binding = saved.binding.clone();
    let mut identity = original_evidence;
    for number in 0..21u32 {
        let association = if number == 10 {
            let previous = binding.binding_id.clone();
            binding = ProfileBinding {
                binding_id: alternate.clone(),
                ..binding
            };
            identity.email = "activity-history-reassociation@example.invalid".into();
            Some(ProfileBindingInput {
                profile_id: profile.into(),
                expected_home_reference: binding.home_reference.clone(),
                expected_previous_binding_id: Some(previous),
                new_binding_id: binding.binding_id.clone(),
                evidence: identity.clone(),
                observed_at_utc: saved.commit.observed_at_utc.clone(),
            })
        } else {
            None
        };
        let summary = match number % 3 {
            0 => ActivitySummaryValues {
                lifetime_tokens: Some("0".into()),
                peak_daily_tokens: None,
                longest_running_turn_sec: Some("0".into()),
                current_streak_days: Some("0".into()),
                longest_streak_days: None,
            },
            1 => ActivitySummaryValues {
                lifetime_tokens: None,
                peak_daily_tokens: Some("0".into()),
                longest_running_turn_sec: None,
                current_streak_days: None,
                longest_streak_days: Some("0".into()),
            },
            _ => ActivitySummaryValues {
                lifetime_tokens: Some("9223372036854775807".into()),
                peak_daily_tokens: Some("9007199254740993".into()),
                longest_running_turn_sec: None,
                current_streak_days: Some("0".into()),
                longest_streak_days: Some("0".into()),
            },
        };
        let daily_usage = match number % 3 {
            0 => None,
            1 => Some(vec![]),
            _ => Some(vec![ActivityDayValue {
                provider_date: "2026-10-20".into(),
                tokens: "0".into(),
            }]),
        };
        storage
            .accept_activity_with_binding(
                ActivityReading {
                    run_id: format!("{:08x}-bbbb-4bbb-8bbb-bbbbbbbbbbbb", number + 60_000),
                    binding: binding.clone(),
                    identity_evidence: identity.clone(),
                    provider_version: saved.provider_version.clone(),
                    source_kind: ActivitySourceKind::TestFixture,
                    started_at_utc: saved.commit.observed_at_utc.clone(),
                    observed_at_utc: saved.commit.observed_at_utc.clone(),
                    completed_at_utc: saved.commit.observed_at_utc.clone(),
                    summary,
                    daily_usage,
                },
                association,
            )
            .unwrap();
    }
    println!(
        "{}",
        json!({"syntheticAcceptedSnapshotsAdded":21,"sameObservationTimestamp":true,"productionStorageApi":true,"sourceKind":"test_fixture","schemaVersion":CURRENT_SCHEMA_VERSION,"originalBindingId":saved.binding.binding_id,"currentBindingId":binding.binding_id,"initialRunId":initial_run})
    );
}
