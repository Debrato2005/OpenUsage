#![cfg(all(target_os = "linux", feature = "native-test"))]
//! Private CLI fixture setup. All data and credentials are synthetic.
use openusage_keys::{KeyVault, OsVault};
use openusage_storage::{LocalUsageRow, LocalUsageScan, LogConsent, Storage};
use std::{os::unix::fs::PermissionsExt, path::PathBuf};

#[test]
#[ignore = "Requires the private board CLI harness, never a real keyring"]
fn seed_private_cli_workspace() {
    let root = openusage_keys::private_keyring_fixture().unwrap();
    let data = PathBuf::from(std::env::var_os("XDG_DATA_HOME").unwrap());
    assert!(data.starts_with(&root) && data != root);
    let workspace = data.join("app.openusage.desktop/workspace");
    std::fs::create_dir_all(&workspace).unwrap();
    std::fs::set_permissions(&workspace, std::fs::Permissions::from_mode(0o700)).unwrap();
    let key = [0x43; 32];
    OsVault::new().unwrap().protect(&key).unwrap();
    let mut storage = Storage::create(&workspace.join("openusage.db"), &key).unwrap();
    let now = jiff::Timestamp::now()
        .strftime("%Y-%m-%dT%H:%M:%S.000Z")
        .to_string();
    let today = jiff::Timestamp::now()
        .to_zoned(jiff::tz::TimeZone::UTC)
        .date()
        .to_string();
    let source = storage
        .set_log_source(
            "claude",
            "/synthetic/project-not-for-upload",
            LogConsent::Granted,
            &now,
        )
        .unwrap();
    storage
        .save_local_usage_scan(&LocalUsageScan {
            log_source_ids: &[source.id],
            scanner_version: "20.0.48",
            time_zone: "UTC",
            since: None,
            scanned_at_utc: &now,
            rows: &[LocalUsageRow {
                local_date: today,
                tool: "claude".into(),
                model: "synthetic-model".into(),
                input_tokens: 31,
                output_tokens: 7,
                cache_read_tokens: 5,
                cache_write_tokens: 3,
                total_tokens: 46,
                estimated_value_usd_micros: Some(12345),
            }],
        })
        .unwrap();
}
