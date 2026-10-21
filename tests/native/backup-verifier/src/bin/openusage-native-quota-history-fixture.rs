//! Seeds bounded synthetic history through the production storage API in an owned native-test workspace.
use openusage_storage::{QuotaReading, Storage, keywrap::unwrap_key};
use serde_json::Value;
use std::{
    env, fs,
    io::{self, Read},
    path::Path,
};
use zeroize::Zeroizing;

fn main() {
    let args: Vec<_> = env::args_os().collect();
    assert_eq!(
        args.len(),
        3,
        "Supply the owned fixture root and synthetic profile UUID."
    );
    let root = Path::new(&args[1]);
    assert!(root.is_absolute() && root.parent() == Some(Path::new("/tmp")));
    assert!(
        root.file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("openusage-native-quota-history-")
    );
    assert!(!fs::symlink_metadata(root).unwrap().file_type().is_symlink());
    let workspace = root.join("data/app.openusage.desktop/workspace");
    let mut input = Zeroizing::new(String::new());
    io::stdin().take(4097).read_to_string(&mut input).unwrap();
    assert!(input.len() <= 4096);
    let mut request: Value = serde_json::from_str(&input).unwrap();
    let password = match request.get_mut("passphrase").unwrap().take() {
        Value::String(value) => Zeroizing::new(value),
        _ => panic!("Invalid synthetic passphrase."),
    };
    let envelope = Zeroizing::new(fs::read(workspace.join("database-key.v1")).unwrap());
    let key = unwrap_key(&envelope, password.as_bytes()).unwrap();
    let mut storage = Storage::open(&workspace.join("openusage.db"), &key).unwrap();
    let profile = args[2].to_str().unwrap();
    let saved = storage.current_quota(profile).unwrap().unwrap();
    let evidence = storage.profile_binding_evidence(&saved.binding).unwrap();
    for number in 0..21u32 {
        let mut limits = saved.limits.clone();
        if let Some(window) = limits.first_mut().and_then(|limit| limit.primary.as_mut()) {
            window.used_percent = u8::try_from(number).unwrap();
        }
        storage
            .accept_quota(
                QuotaReading {
                    run_id: format!("{:08x}-aaaa-4aaa-8aaa-aaaaaaaaaaaa", number + 50_000),
                    binding: saved.binding.clone(),
                    identity_evidence: evidence.clone(),
                    provider_version: saved.provider_version.clone(),
                    source_kind: saved.source_kind,
                    started_at_utc: saved.commit.observed_at_utc.clone(),
                    observed_at_utc: saved.commit.observed_at_utc.clone(),
                    completed_at_utc: saved.commit.observed_at_utc.clone(),
                    limits,
                    available_reset_credits: None,
                    reset_credit_details: None,
                },
                None,
            )
            .unwrap();
    }
    println!(
        "{{\"syntheticAcceptedSnapshotsAdded\":21,\"sameObservationTimestamp\":true,\"productionStorageApi\":true}}"
    );
}
