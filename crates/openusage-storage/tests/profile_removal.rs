//! Public storage contracts with private synthetic data. No provider or OS delivery runs.
use openusage_storage::*;
use serde_json::Value;
use std::{collections::BTreeMap, fs, path::PathBuf};

const KEY: [u8; 32] = [0x48; 32];
const NOW: &str = "2026-10-18T12:00:00.000Z";
#[path = "profile_removal/fixture.rs"]
mod fixture;
use fixture::*;

fn id(n: u32) -> String {
    format!("{n:08x}-0000-4000-8000-000000000000")
}

struct Fixture {
    directory: tempfile::TempDir,
    storage: Storage,
}

impl Fixture {
    fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700)).unwrap();
        }
        let mut storage = Storage::create(&directory.path().join("data.db"), &KEY).unwrap();
        for n in 1..=2 {
            storage
                .create_profile(&id(n), "Synthetic", &id(n + 10))
                .unwrap();
        }
        Self { directory, storage }
    }

    fn path(&self) -> PathBuf {
        self.directory.path().join("data.db")
    }
}

fn request(n: u32, history: ProfileHistoryChoice) -> ProfileRemovalRequest {
    ProfileRemovalRequest {
        profile_id: id(n),
        expected_home_reference: id(n + 10),
        history,
    }
}

fn records(storage: &mut Storage) -> BTreeMap<String, Value> {
    let mut output = Vec::new();
    storage.write_application_export(&mut output, NOW).unwrap();
    let export: Value = serde_json::from_slice(&output).unwrap();
    export["tables"]
        .as_array()
        .unwrap()
        .iter()
        .map(|table| (table["name"].as_str().unwrap().to_owned(), table.clone()))
        .collect()
}

#[test]
fn archive_preserves_records_and_survives_reopen() {
    let mut fixture = populated();
    let before = records(&mut fixture.storage);
    let revision = fixture.storage.restore_state().unwrap().revision;
    let result = fixture
        .storage
        .remove_profile(request(1, ProfileHistoryChoice::Archive), &revision)
        .unwrap();
    assert!(
        matches!(result, ProfileRemovalOutcome::Archived(profile) if profile.id == id(1) && profile.archived)
    );
    let after = records(&mut fixture.storage);
    for (name, table) in &before {
        if name != "profiles" {
            assert_eq!(&after[name], table, "{name}");
        }
    }
    let mut expected_profiles = before["profiles"].clone();
    let profile_id = column(&expected_profiles, "id");
    let archived = column(&expected_profiles, "archived");
    let updated = column(&expected_profiles, "updated_at");
    for row in expected_profiles["rows"].as_array_mut().unwrap() {
        if row[profile_id] == id(1) {
            row[archived] = Value::String("1".into());
            row[updated] = after["profiles"]["rows"]
                .as_array()
                .unwrap()
                .iter()
                .find(|r| r[profile_id] == id(1))
                .unwrap()[updated]
                .clone();
        }
    }
    assert_eq!(after["profiles"], expected_profiles);
    assert!(
        !fixture
            .storage
            .list_profiles()
            .unwrap()
            .iter()
            .find(|p| p.id == id(2))
            .unwrap()
            .archived
    );
    assert_sentinels(&fixture);
    assert_eq!(
        fixture
            .storage
            .quota_history(&id(1), None)
            .unwrap()
            .entries
            .len(),
        4
    );
    assert_eq!(
        fixture
            .storage
            .activity_history(&id(1), None)
            .unwrap()
            .entries
            .len(),
        4
    );
    let snapshot = fixture.storage.backup_snapshot().unwrap();
    assert_eq!(
        validate_backup_snapshot(&snapshot.bytes, snapshot.schema_version)
            .unwrap()
            .archived_profiles,
        1
    );
    let path = fixture.path();
    drop(fixture.storage);
    let mut reopened = Storage::open(&path, &KEY).unwrap();
    assert_eq!(records(&mut reopened), after);
}

const OWNED: &[&str] = &[
    "profiles",
    "profile_identity_events",
    "profile_bindings",
    "collection_runs",
    "activity_summaries",
    "current_activity",
    "current_activity_days",
    "activity_day_history",
    "quota_snapshots",
    "quota_limits",
    "credit_snapshots",
    "current_quota_readings",
    "current_quota",
    "quota_reset_assessments",
    "alert_events",
    "operational_alert_events",
    "operational_alert_outcomes",
    "subscriptions",
    "reset_alert_overrides",
    "threshold_alert_overrides",
    "operational_alert_overrides",
];

fn column(table: &Value, name: &str) -> usize {
    table["columns"]
        .as_array()
        .unwrap()
        .iter()
        .position(|c| c["name"] == name)
        .unwrap()
}

fn expected_deleted(before: &BTreeMap<String, Value>, profile: &str) -> BTreeMap<String, Value> {
    let event_ids = ["alert_events", "operational_alert_events"]
        .into_iter()
        .flat_map(|name| {
            let table = &before[name];
            let owner = column(table, "profile_id");
            let key = column(table, "id");
            table["rows"]
                .as_array()
                .unwrap()
                .iter()
                .filter(move |r| r[owner] == profile)
                .map(move |r| r[key].clone())
        })
        .collect::<Vec<_>>();
    let mut expected = before.clone();
    for (name, table) in &mut expected {
        let index = if OWNED.contains(&name.as_str()) {
            Some(column(
                table,
                if name == "profiles" {
                    "id"
                } else {
                    "profile_id"
                },
            ))
        } else {
            None
        };
        if let Some(index) = index {
            table["rows"]
                .as_array_mut()
                .unwrap()
                .retain(|r| r[index] != profile);
        } else if [
            "notification_event_ids",
            "reset_notification_outbox",
            "threshold_alert_events",
        ]
        .contains(&name.as_str())
        {
            let index = column(
                table,
                if name == "notification_event_ids" {
                    "id"
                } else {
                    "event_id"
                },
            );
            table["rows"]
                .as_array_mut()
                .unwrap()
                .retain(|r| !event_ids.contains(&r[index]));
        }
    }
    expected
}

#[test]
fn fixture_has_every_current_table_and_retains_zero_and_missing_values() {
    let mut fixture = populated();
    for (name, table) in records(&mut fixture.storage) {
        assert!(
            !table["rows"].as_array().unwrap().is_empty(),
            "empty fixture table: {name}"
        );
    }
    let activity = fixture.storage.current_activity(&id(2)).unwrap().unwrap();
    assert_eq!(activity.summary.lifetime_tokens.as_deref(), Some("0"));
    assert_eq!(activity.summary.peak_daily_tokens, None);
    let local = fixture.storage.local_periods("pi").unwrap();
    assert_eq!(local[0].period.total_tokens, 5);
    assert_eq!(local[0].period.details.input_tokens, None);
    assert_eq!(local[0].period.estimated_value_micros, None);
}

#[test]
fn delete_removes_exact_profile_history_and_preserves_other_owners_after_reopen() {
    let mut fixture = populated();
    let before = records(&mut fixture.storage);
    let revision = fixture.storage.restore_state().unwrap().revision;
    assert_eq!(
        fixture
            .storage
            .remove_profile(request(1, ProfileHistoryChoice::Delete), &revision),
        Ok(ProfileRemovalOutcome::Deleted { profile_id: id(1) })
    );
    let after = records(&mut fixture.storage);
    assert_eq!(after, expected_deleted(&before, &id(1)));
    assert_sentinels(&fixture);
    assert!(fixture.storage.current_quota(&id(1)).unwrap().is_none());
    assert!(fixture.storage.current_activity(&id(1)).unwrap().is_none());
    assert!(fixture.storage.profile_binding(&id(1)).unwrap().is_none());
    assert_eq!(
        fixture.storage.quota_history(&id(1), None).unwrap_err(),
        ActivityError::ProfileUnavailable
    );
    assert_eq!(
        fixture.storage.activity_history(&id(1), None).unwrap_err(),
        ActivityError::ProfileUnavailable
    );
    assert_eq!(
        fixture
            .storage
            .current_activity(&id(2))
            .unwrap()
            .unwrap()
            .summary
            .peak_daily_tokens,
        None
    );
    assert_eq!(
        fixture
            .storage
            .current_quota(&id(2))
            .unwrap()
            .unwrap()
            .available_reset_credits
            .as_deref(),
        Some("0")
    );
    let snapshot = fixture.storage.backup_snapshot().unwrap();
    assert_eq!(
        validate_backup_snapshot(&snapshot.bytes, snapshot.schema_version)
            .unwrap()
            .accounts,
        3
    );
    let path = fixture.path();
    drop(fixture.storage);
    let mut reopened = Storage::open(&path, &KEY).unwrap();
    assert_eq!(records(&mut reopened), after);
    assert!(reopened.claim_notification(&time(28)).unwrap().is_some());
}

#[test]
fn fences_refuse_invalid_missing_wrong_stale_foreign_and_reopened_targets() {
    let mut fixture = populated();
    for choice in [ProfileHistoryChoice::Archive, ProfileHistoryChoice::Delete] {
        for (profile, home, error) in [
            ("BAD".into(), id(11), ProfileRemovalError::InvalidId),
            (
                id(1),
                "BAD".into(),
                ProfileRemovalError::InvalidHomeReference,
            ),
            (id(99), id(11), ProfileRemovalError::NotFound),
            (id(1), id(12), ProfileRemovalError::StaleTarget),
        ] {
            let before = records(&mut fixture.storage);
            let revision = fixture.storage.restore_state().unwrap().revision;
            assert_eq!(
                fixture.storage.remove_profile(
                    ProfileRemovalRequest {
                        profile_id: profile,
                        expected_home_reference: home,
                        history: choice
                    },
                    &revision
                ),
                Err(error)
            );
            assert_eq!(records(&mut fixture.storage), before);
        }
        let stale = fixture.storage.restore_state().unwrap().revision;
        fixture
            .storage
            .rename_profile(&id(1), "Changed after preview")
            .unwrap();
        let before = records(&mut fixture.storage);
        assert_eq!(
            fixture.storage.remove_profile(request(1, choice), &stale),
            Err(ProfileRemovalError::StaleTarget)
        );
        assert_eq!(records(&mut fixture.storage), before);
        let mut other = Storage::open(&fixture.path(), &KEY).unwrap();
        let foreign = other.restore_state().unwrap().revision;
        assert_eq!(
            fixture.storage.remove_profile(request(1, choice), &foreign),
            Err(ProfileRemovalError::StaleTarget)
        );
        let stale = fixture.storage.restore_state().unwrap().revision;
        other.set_theme(Theme::Light).unwrap();
        let before = records(&mut fixture.storage);
        assert_eq!(
            fixture.storage.remove_profile(request(1, choice), &stale),
            Err(ProfileRemovalError::StaleTarget)
        );
        assert_eq!(records(&mut fixture.storage), before);
    }
    let old = fixture.storage.restore_state().unwrap().revision;
    let path = fixture.path();
    drop(fixture.storage);
    let mut reopened = Storage::open(&path, &KEY).unwrap();
    let before = records(&mut reopened);
    assert_eq!(
        reopened.remove_profile(request(1, ProfileHistoryChoice::Delete), &old),
        Err(ProfileRemovalError::StaleTarget)
    );
    assert_eq!(records(&mut reopened), before);
}

#[test]
fn protected_environment_and_changed_home_refuse_both_choices() {
    let mut fixture = Fixture::new();
    let connection = connection(&fixture.path());
    for sql in [
        "UPDATE profiles SET environment='wsl' WHERE id=?1",
        "UPDATE profiles SET environment='native',approved_home_reference='00000042-0000-4000-8000-000000000000' WHERE id=?1",
    ] {
        connection.execute(sql, [id(1)]).unwrap();
        let before = raw_records(&fixture.path());
        for choice in [ProfileHistoryChoice::Archive, ProfileHistoryChoice::Delete] {
            let revision = fixture.storage.restore_state().unwrap().revision;
            assert_eq!(
                fixture
                    .storage
                    .remove_profile(request(1, choice), &revision),
                Err(ProfileRemovalError::StaleTarget)
            );
            assert_eq!(raw_records(&fixture.path()), before);
        }
    }
}

#[test]
fn final_statement_failure_rolls_back_all_history_and_allows_later_success() {
    let mut fixture = populated();
    let connection = connection(&fixture.path());
    connection.execute_batch("CREATE TRIGGER removal_fault BEFORE DELETE ON profiles BEGIN SELECT RAISE(ABORT,'synthetic final statement failure'); END;").unwrap();
    let before = raw_records(&fixture.path());
    let revision = fixture.storage.restore_state().unwrap().revision;
    assert_eq!(
        fixture
            .storage
            .remove_profile(request(1, ProfileHistoryChoice::Delete), &revision),
        Err(ProfileRemovalError::WriteFailed)
    );
    assert_eq!(raw_records(&fixture.path()), before);
    connection
        .execute_batch("DROP TRIGGER removal_fault")
        .unwrap();
    let revision = fixture.storage.restore_state().unwrap().revision;
    assert!(
        fixture
            .storage
            .remove_profile(request(1, ProfileHistoryChoice::Delete), &revision)
            .is_ok()
    );
}

#[test]
fn cross_profile_reference_refuses_without_partial_deletion() {
    let mut fixture = populated();
    let connection = connection(&fixture.path());
    connection
        .execute(
            "UPDATE quota_snapshots SET previous_quota_run_id=?1 WHERE id=?2",
            rusqlite::params![id(1015), id(2004)],
        )
        .unwrap();
    let before = raw_records(&fixture.path());
    let revision = fixture.storage.restore_state().unwrap().revision;
    assert_eq!(
        fixture
            .storage
            .remove_profile(request(1, ProfileHistoryChoice::Delete), &revision),
        Err(ProfileRemovalError::WriteFailed)
    );
    assert_eq!(raw_records(&fixture.path()), before);
}

#[test]
fn shared_notification_identity_refuses_without_removing_other_profile_evidence() {
    let mut fixture = populated();
    let connection = connection(&fixture.path());
    let selected: String = connection
        .query_row(
            "SELECT id FROM alert_events WHERE profile_id=?1 LIMIT 1",
            [id(1)],
            |row| row.get(0),
        )
        .unwrap();
    let other: String = connection
        .query_row(
            "SELECT id FROM operational_alert_events WHERE profile_id=?1 LIMIT 1",
            [id(2)],
            |row| row.get(0),
        )
        .unwrap();
    connection
        .execute(
            "UPDATE operational_alert_events SET id=?1 WHERE id=?2",
            rusqlite::params![selected, other],
        )
        .unwrap();
    let before = raw_records(&fixture.path());
    let revision = fixture.storage.restore_state().unwrap().revision;
    assert_eq!(
        fixture
            .storage
            .remove_profile(request(1, ProfileHistoryChoice::Delete), &revision),
        Err(ProfileRemovalError::WriteFailed)
    );
    assert_eq!(raw_records(&fixture.path()), before);
}

#[test]
fn archived_and_empty_profiles_can_be_deleted_without_inventing_history() {
    let mut fixture = Fixture::new();
    fixture.storage.set_profile_archived(&id(1), true).unwrap();
    let before = records(&mut fixture.storage);
    let revision = fixture.storage.restore_state().unwrap().revision;
    fixture
        .storage
        .remove_profile(request(1, ProfileHistoryChoice::Delete), &revision)
        .unwrap();
    assert_eq!(
        records(&mut fixture.storage),
        expected_deleted(&before, &id(1))
    );
    let revision = fixture.storage.restore_state().unwrap().revision;
    assert_eq!(
        fixture
            .storage
            .remove_profile(request(1, ProfileHistoryChoice::Delete), &revision),
        Err(ProfileRemovalError::NotFound)
    );
}

#[test]
fn backups_restore_deleted_and_archived_history_without_replaying_submissions() {
    let mut fixture = populated();
    let before = fixture.storage.backup_snapshot().unwrap();
    let homes = validate_checkpoint_restore_input(&before.bytes, before.schema_version)
        .unwrap()
        .homes;
    assert!(consume_notifications(&mut fixture.storage) >= 4);
    let consumed = records(&mut fixture.storage);
    let revision = fixture.storage.restore_state().unwrap().revision;
    fixture
        .storage
        .remove_profile(request(1, ProfileHistoryChoice::Delete), &revision)
        .unwrap();
    let after = fixture.storage.backup_snapshot().unwrap();
    let mut destination = Fixture::new();
    let revision = destination.storage.restore_state().unwrap().revision;
    destination
        .storage
        .replace_records_from_backup(
            &after.bytes,
            after.schema_version,
            &revision,
            &[RestoreHomeMapping {
                profile_id: id(2),
                managed_home_reference: id(99),
            }],
        )
        .unwrap();
    assert_eq!(destination.storage.list_profiles().unwrap().len(), 1);
    assert!(
        destination
            .storage
            .profile_binding(&id(1))
            .unwrap()
            .is_none()
    );
    let restored = records(&mut destination.storage);
    for name in [
        "accounts",
        "reset_notification_receipts",
        "operational_alert_state",
    ] {
        assert_eq!(restored[name], consumed[name], "{name}");
    }
    let revision = fixture.storage.restore_state().unwrap().revision;
    fixture
        .storage
        .replace_records_from_checkpoint(&before.bytes, before.schema_version, &revision, &homes)
        .unwrap();
    assert_eq!(fixture.storage.list_profiles().unwrap().len(), 2);
    assert_eq!(
        fixture
            .storage
            .quota_history(&id(1), None)
            .unwrap()
            .entries
            .len(),
        4
    );
    assert_eq!(consume_notifications(&mut fixture.storage), 0);
    let restored = records(&mut fixture.storage);
    assert_eq!(
        restored["reset_notification_receipts"],
        consumed["reset_notification_receipts"]
    );
    assert_eq!(
        restored["operational_alert_state"],
        consumed["operational_alert_state"]
    );
    let revision = fixture.storage.restore_state().unwrap().revision;
    fixture
        .storage
        .remove_profile(request(1, ProfileHistoryChoice::Archive), &revision)
        .unwrap();
    let archived = fixture.storage.backup_snapshot().unwrap();
    let revision = fixture.storage.restore_state().unwrap().revision;
    fixture
        .storage
        .remove_profile(request(1, ProfileHistoryChoice::Delete), &revision)
        .unwrap();
    let revision = fixture.storage.restore_state().unwrap().revision;
    fixture
        .storage
        .replace_records_from_checkpoint(
            &archived.bytes,
            archived.schema_version,
            &revision,
            &homes,
        )
        .unwrap();
    assert!(
        fixture
            .storage
            .list_profiles()
            .unwrap()
            .iter()
            .find(|p| p.id == id(1))
            .unwrap()
            .archived
    );
    assert_eq!(
        fixture
            .storage
            .activity_history(&id(1), None)
            .unwrap()
            .entries
            .len(),
        4
    );
    assert_sentinels(&fixture);
}
