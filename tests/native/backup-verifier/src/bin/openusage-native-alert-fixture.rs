//! Test-only quota proofs in a marker-owned synthetic workspace, through public storage APIs.
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
fn canonical_uuid(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(index, byte)| {
            if [8, 13, 18, 23].contains(&index) {
                byte == b'-'
            } else {
                byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)
            }
        })
}
fn reading(
    binding: &ProfileBinding,
    evidence: &ProfileIdentityEvidence,
    number: u8,
    times: Option<&Value>,
) -> QuotaReading {
    let (observed, used, reset) = match number {
        1 => ("2026-10-22T11:59:00.000Z", 80, 1_792_670_400),
        2 => ("2026-10-22T12:01:00.000Z", 10, 1_792_674_000),
        3 => ("2026-10-22T12:02:00.000Z", 15, 1_792_674_000),
        4 => ("2026-10-22T12:03:00.000Z", 20, 1_792_674_000),
        _ => panic!("Invalid synthetic observation."),
    };
    let mut result = QuotaReading {
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
    };
    if let Some(times) = times {
        let observations = times["observations"]
            .as_array()
            .expect("Synthetic observation array required.");
        assert_eq!(observations.len(), 4);
        let observed = observations[usize::from(number - 1)].as_str().unwrap();
        result.started_at_utc = observed.into();
        result.observed_at_utc = observed.into();
        result.completed_at_utc = observed.into();
        let reset = times[if number == 1 {
            "priorResetUnix"
        } else {
            "nextResetUnix"
        }]
        .as_i64()
        .unwrap();
        result.limits[0].primary.as_mut().unwrap().resets_at_unix = Some(reset);
        // Keep synthetic provenance. Only an explicitly compiled native-test
        // storage feature may enqueue these packets; production must refuse them.
        assert!(cfg!(feature = "native-test"));
    }
    result
}

fn threshold_readings(
    storage: &mut Storage,
    profile: &str,
    home_reference: &str,
    evidence: &ProfileIdentityEvidence,
    mode: &str,
    times: &Value,
) {
    // This helper cannot grant synthetic packets production provenance or bypass
    // the production dispatch gate. Only the explicit test build may call it.
    assert!(cfg!(feature = "native-test"));
    let first_high = mode.starts_with("threshold-high-");
    let seed = mode.ends_with("seed");
    let reseed = mode.ends_with("reseed");
    let values: &[u8] = match (first_high, seed) {
        (true, true) => &[99, 95],
        (false, true) => &[99, 79, 80, 90, 100],
        (true, false) => &[85, 95],
        (false, false) => &[79, 100],
    };
    let observations = times["observations"].as_array().unwrap();
    let runs = times["runIds"].as_array().unwrap();
    assert_eq!(observations.len(), 7);
    assert_eq!(runs.len(), 7);
    let now = times["acceptedAt"].as_str().unwrap();
    let binding = ProfileBinding {
        profile_id: profile.into(),
        home_reference: home_reference.into(),
        binding_id: profile.into(),
    };
    let before = storage.alert_history().unwrap();
    if seed {
        assert!(storage.current_quota(profile).unwrap().is_none());
        assert!(
            before
                .events
                .iter()
                .all(|event| event.profile_id != profile)
        );
    }
    for (step, used) in values.iter().enumerate() {
        let index = if seed { step } else { step + 5 };
        let prior = seed && step == 0;
        let mut packet = reading(&binding, evidence, if prior { 1 } else { 2 }, None);
        packet.run_id = runs[index].as_str().unwrap().into();
        let observed = observations[index].as_str().unwrap();
        packet.started_at_utc = observed.into();
        packet.observed_at_utc = observed.into();
        packet.completed_at_utc = observed.into();
        let window = packet.limits[0].primary.as_mut().unwrap();
        window.used_percent = *used;
        window.resets_at_unix = Some(
            times[if prior {
                "priorResetUnix"
            } else {
                "nextResetUnix"
            }]
            .as_i64()
            .unwrap(),
        );
        let association = prior.then(|| ProfileBindingInput {
            profile_id: profile.into(),
            expected_home_reference: home_reference.into(),
            expected_previous_binding_id: None,
            new_binding_id: binding.binding_id.clone(),
            evidence: evidence.clone(),
            observed_at_utc: observed.into(),
        });
        storage.accept_live_quota(packet, association, now).unwrap();
        if prior {
            assert!(
                storage
                    .alert_history()
                    .unwrap()
                    .events
                    .iter()
                    .all(|event| event.profile_id != profile),
                "An initial reading must not manufacture a confirmed generation."
            );
        }
    }
    let history = storage.alert_history().unwrap();
    if !seed {
        assert_eq!(
            history, before,
            "A decline and re-cross must not replay consumed levels."
        );
    }
    let events: Vec<_> = history
        .events
        .iter()
        .filter(|event| event.profile_id == profile)
        .collect();
    assert_eq!(
        events.len(),
        if reseed {
            1
        } else if first_high {
            3
        } else {
            4
        }
    );
    let mut thresholds: Vec<_> = events
        .iter()
        .filter_map(|event| {
            event
                .threshold_percent
                .map(|threshold| (threshold, event.delivery_state.as_str()))
        })
        .collect();
    thresholds.sort_unstable();
    if reseed {
        assert!(
            thresholds.is_empty(),
            "Restoring before the reset proof must not revive consumed provider-window thresholds."
        );
    } else if seed {
        assert_eq!(
            thresholds,
            if first_high {
                vec![(80, "covered"), (90, "queued")]
            } else {
                vec![(80, "queued"), (90, "queued"), (100, "queued")]
            }
        );
    }
    let generation = &events[0].generation_run_id;
    assert!(
        events
            .iter()
            .all(|event| event.generation_run_id == *generation
                && event.source_kind == ActivitySourceKind::TestFixture)
    );
    println!(
        "{}",
        json!({"synthetic":true,"productionStorageApi":true,"explicitNativeTestLiveContext":true,"schemaVersion":CURRENT_SCHEMA_VERSION,"mode":mode,"profileId":profile,"generationRunId":generation,"eventCount":events.len(),"thresholds":thresholds})
    );
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
        canonical_uuid(profile),
        "A canonical synthetic profile UUID is required."
    );
    directory(&root.join("control"));
    let mode = args[3].to_str().unwrap();
    assert!(matches!(
        mode,
        "seed"
            | "followup"
            | "notification-seed"
            | "notification-followup"
            | "threshold-high-seed"
            | "threshold-high-followup"
            | "threshold-steps-seed"
            | "threshold-steps-followup"
            | "threshold-high-reseed"
            | "threshold-steps-reseed"
    ));
    let notification_mode = mode.starts_with("notification-") || mode.starts_with("threshold-");
    let marker: Value =
        serde_json::from_slice(&private_file(&root.join("control/alert-fixture.json"))).unwrap();
    assert_eq!(
        marker,
        json!({"purpose":if notification_mode { "openusage-native-reset-notifications-v1" } else { "openusage-native-alert-history-v1" },"profileId":profile})
    );
    let workspace = root.join("data/app.openusage.desktop/workspace");
    directory(&workspace);
    let mut input = Zeroizing::new(String::new());
    io::stdin().take(4097).read_to_string(&mut input).unwrap();
    assert!(input.len() <= 4096);
    let mut request: Value = serde_json::from_str(&input).expect("Invalid synthetic request.");
    assert!(
        request
            .as_object()
            .is_some_and(|object| object.len() == if notification_mode { 2 } else { 1 })
    );
    let fixture_times = notification_mode.then(|| {
        request
            .get("notificationTimes")
            .expect("Missing controlled synthetic times.")
            .clone()
    });
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
    // Archive restore intentionally assigns fresh managed homes. Read the
    // current Rust-owned mapping rather than assuming that it is the profile ID.
    let revision = storage.restore_state().unwrap().revision;
    let mappings = storage.restore_home_mappings(&revision).unwrap();
    let home_reference = &mappings
        .iter()
        .find(|mapping| mapping.profile_id == profile)
        .expect("The synthetic profile must have a managed home.")
        .managed_home_reference;
    assert!(canonical_uuid(home_reference));
    let homes = workspace.join("profiles");
    directory(&homes);
    assert_eq!(homes.canonicalize().unwrap(), homes);
    let home = homes.join(home_reference);
    directory(&home);
    assert_eq!(home.canonicalize().unwrap(), home);
    assert!(
        fs::read_dir(&home).unwrap().next().is_none(),
        "The synthetic home must contain no provider files."
    );
    let evidence = ProfileIdentityEvidence {
        email: "alert-synthetic@example.invalid".into(),
        plan: None,
    };
    if mode.starts_with("threshold-") {
        threshold_readings(
            &mut storage,
            profile,
            home_reference,
            &evidence,
            mode,
            fixture_times.as_ref().unwrap(),
        );
        return;
    }
    let binding = ProfileBinding {
        profile_id: profile.into(),
        home_reference: home_reference.into(),
        binding_id: "a1000000-0000-4000-8000-000000000001".into(),
    };
    if mode.ends_with("seed") {
        assert!(storage.alert_history().unwrap().events.is_empty());
        assert!(storage.current_quota(profile).unwrap().is_none());
        let first = reading(&binding, &evidence, 1, fixture_times.as_ref());
        let association = ProfileBindingInput {
            profile_id: profile.into(),
            expected_home_reference: home_reference.into(),
            expected_previous_binding_id: None,
            new_binding_id: binding.binding_id.clone(),
            evidence: evidence.clone(),
            observed_at_utc: first.observed_at_utc.clone(),
        };
        storage.accept_quota(first, Some(association)).unwrap();
        assert!(storage.alert_history().unwrap().events.is_empty());
        storage
            .accept_quota(
                reading(&binding, &evidence, 2, fixture_times.as_ref()),
                None,
            )
            .unwrap();
        assert_eq!(storage.alert_history().unwrap().events.len(), 1);
        storage
            .accept_quota(
                reading(&binding, &evidence, 3, fixture_times.as_ref()),
                None,
            )
            .unwrap();
    } else {
        let before = storage.alert_history().unwrap();
        assert_eq!(before.events.len(), 1);
        storage
            .accept_quota(
                reading(&binding, &evidence, 4, fixture_times.as_ref()),
                None,
            )
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
        json!({"synthetic":true,"productionStorageApi":true,"schemaVersion":CURRENT_SCHEMA_VERSION,"mode":mode,"profileId":profile,"eventId":event.id,"generationRunId":event.generation_run_id,"eventCount":1,"evidenceAtUtc":event.evidence_at_utc,"deliveryState":event.delivery_state.as_str()})
    );
}
