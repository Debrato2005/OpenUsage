//! Test-only reset proof in a marker-owned synthetic workspace, through public storage APIs.
use openusage_storage::{
    ActivitySourceKind, CURRENT_SCHEMA_VERSION, ProfileBinding, ProfileBindingInput,
    ProfileIdentityEvidence, QuotaLimit, QuotaReading, QuotaWindow, Storage, keywrap::unwrap_key,
};
use serde_json::{Value, json};
use std::{
    env, fs,
    io::{self, Read},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
};
use zeroize::Zeroizing;

fn private_file(path: &Path) -> Vec<u8> {
    let mut file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .expect("Cannot open the synthetic private file.");
    let metadata = file.metadata().unwrap();
    assert!(
        metadata.is_file()
            && metadata.uid() == unsafe { libc::geteuid() }
            && metadata.mode() & 0o777 == 0o600
            && metadata.nlink() == 1,
        "The synthetic file must be private and owned."
    );
    assert!(
        metadata.len() <= 4096,
        "The synthetic private file exceeds its limit."
    );
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).unwrap();
    bytes
}
fn directory(path: &Path) {
    let metadata = fs::symlink_metadata(path).unwrap();
    assert!(
        metadata.is_dir()
            && !metadata.file_type().is_symlink()
            && metadata.uid() == unsafe { libc::geteuid() }
            && metadata.mode() & 0o777 == 0o700,
        "The synthetic directory must be private and owned."
    );
}
fn reading(
    binding: &ProfileBinding,
    evidence: &ProfileIdentityEvidence,
    number: u8,
) -> QuotaReading {
    let (observed, used, reset) = match number {
        1 => ("2026-10-22T11:59:00.000Z", 80, 1_792_670_400),
        2 => ("2026-10-22T12:01:00.000Z", 10, 1_792_674_000),
        3 => ("2026-10-22T12:02:00.000Z", 15, 1_792_674_000),
        4 => ("2026-10-22T12:03:00.000Z", 20, 1_792_674_000),
        _ => panic!("Invalid synthetic observation."),
    };
    QuotaReading {
        run_id: format!("b1000000-0000-4000-8000-{number:012}"),
        binding: binding.clone(),
        identity_evidence: evidence.clone(),
        provider_version: "0.163.0".into(),
        source_kind: ActivitySourceKind::TestFixture,
        started_at_utc: observed.into(),
        observed_at_utc: observed.into(),
        completed_at_utc: observed.into(),
        limits: vec![QuotaLimit {
            limit_id: "synthetic-reset-window".into(),
            limit_name: Some("Synthetic reset proof".into()),
            primary: Some(QuotaWindow {
                used_percent: used,
                window_duration_mins: Some(300),
                resets_at_unix: Some(reset),
            }),
            secondary: None,
            reached_type: None,
            monthly_credit_limit: None,
            spend_control_reached: None,
        }],
        available_reset_credits: None,
        reset_credit_details: None,
    }
}
fn main() {
    let args: Vec<_> = env::args_os().collect();
    assert!(
        args.len() == 4,
        "Supply the private fixture root, selected profile UUID and seed/followup mode."
    );
    let root = PathBuf::from(&args[1]);
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .unwrap();
    assert!(
        root.is_absolute()
            && root.canonicalize().unwrap() == root
            && root.parent() == Some(repository.join(".recovery/tmp").as_path())
            && root
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("n-"),
        "Only a repository-private native fixture is permitted."
    );
    directory(&root);
    let profile = args[2].to_str().unwrap();
    assert!(
        profile.len() == 36
            && profile.bytes().enumerate().all(|(index, byte)| {
                if [8, 13, 18, 23].contains(&index) {
                    byte == b'-'
                } else {
                    byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)
                }
            }),
        "A canonical synthetic profile UUID is required."
    );
    directory(&root.join("control"));
    let mode = args[3].to_str().unwrap();
    assert!(matches!(mode, "seed" | "followup"));
    let marker: Value =
        serde_json::from_slice(&private_file(&root.join("control/alert-fixture.json"))).unwrap();
    assert_eq!(
        marker,
        json!({"purpose":"openusage-native-alert-history-v1","profileId":profile})
    );
    let workspace = root.join("data/app.openusage.desktop/workspace");
    directory(&workspace);
    let home = workspace.join("profiles").join(profile);
    directory(&home);
    assert!(
        fs::read_dir(&home).unwrap().next().is_none(),
        "The synthetic home must contain no provider files."
    );
    let mut input = Zeroizing::new(String::new());
    io::stdin().take(4097).read_to_string(&mut input).unwrap();
    assert!(input.len() <= 4096);
    let mut request: Value = serde_json::from_str(&input).expect("Invalid synthetic request.");
    assert!(request.as_object().is_some_and(|object| object.len() == 1));
    let password = match request
        .get_mut("passphrase")
        .expect("Missing synthetic password.")
        .take()
    {
        Value::String(value) => Zeroizing::new(value),
        _ => panic!("Invalid synthetic password."),
    };
    let envelope = Zeroizing::new(private_file(&workspace.join("database-key.v1")));
    let key = unwrap_key(&envelope, password.as_bytes()).expect("Cannot unlock the synthetic key.");
    let database = workspace.join("openusage.db");
    let metadata = fs::symlink_metadata(&database).unwrap();
    assert!(
        metadata.is_file()
            && !metadata.file_type().is_symlink()
            && metadata.uid() == unsafe { libc::geteuid() }
            && metadata.mode() & 0o777 == 0o600
            && metadata.nlink() == 1
    );
    let mut storage = Storage::open(&database, &key).expect("Cannot open synthetic storage.");
    assert_eq!(
        storage.metadata().unwrap().schema_version,
        CURRENT_SCHEMA_VERSION
    );
    let profiles = storage.list_profiles().unwrap();
    assert!(
        profiles
            .iter()
            .any(|item| item.id == profile && !item.archived)
    );
    let evidence = ProfileIdentityEvidence {
        email: "alert-synthetic@example.invalid".into(),
        plan: None,
    };
    let binding = ProfileBinding {
        profile_id: profile.into(),
        home_reference: profile.into(),
        binding_id: "a1000000-0000-4000-8000-000000000001".into(),
    };
    if mode == "seed" {
        assert!(storage.alert_history().unwrap().events.is_empty());
        assert!(storage.current_quota(profile).unwrap().is_none());
        let first = reading(&binding, &evidence, 1);
        let association = ProfileBindingInput {
            profile_id: profile.into(),
            expected_home_reference: profile.into(),
            expected_previous_binding_id: None,
            new_binding_id: binding.binding_id.clone(),
            evidence: evidence.clone(),
            observed_at_utc: first.observed_at_utc.clone(),
        };
        storage.accept_quota(first, Some(association)).unwrap();
        assert!(storage.alert_history().unwrap().events.is_empty());
        storage
            .accept_quota(reading(&binding, &evidence, 2), None)
            .unwrap();
        assert_eq!(storage.alert_history().unwrap().events.len(), 1);
        storage
            .accept_quota(reading(&binding, &evidence, 3), None)
            .unwrap();
    } else {
        let before = storage.alert_history().unwrap();
        assert_eq!(before.events.len(), 1);
        storage
            .accept_quota(reading(&binding, &evidence, 4), None)
            .unwrap();
        assert_eq!(
            storage.alert_history().unwrap(),
            before,
            "A follow-up must not replay the confirmed generation."
        );
    }
    let history = storage.alert_history().unwrap();
    assert!(!history.truncated);
    assert_eq!(history.events.len(), 1);
    let event = &history.events[0];
    assert_eq!(event.profile_id, profile);
    assert_eq!(
        event.generation_run_id,
        "b1000000-0000-4000-8000-000000000002"
    );
    println!(
        "{}",
        json!({"synthetic":true,"productionStorageApi":true,"schemaVersion":CURRENT_SCHEMA_VERSION,"mode":mode,"profileId":profile,"eventId":event.id,"generationRunId":event.generation_run_id,"eventCount":1,"evidenceAtUtc":event.evidence_at_utc,"deliveryState":"recorded"})
    );
}
