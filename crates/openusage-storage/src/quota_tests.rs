use super::*;
const KEY: [u8; 32] = [0x69; 32];
const TIME: &str = "2026-10-18T12:00:00.000Z";
const LATER: &str = "2026-10-18T12:00:01.000Z";
fn id(n: u32) -> String {
    format!("{n:08x}-0000-4000-8000-000000000000")
}
fn evidence() -> ProfileIdentityEvidence {
    ProfileIdentityEvidence {
        email: "synthetic-quota@example.invalid".into(),
        plan: None,
    }
}
fn reading(n: u32, binding: ProfileBinding) -> QuotaReading {
    QuotaReading {
        run_id: id(n),
        binding,
        identity_evidence: evidence(),
        provider_version: "0.163.0".into(),
        source_kind: ActivitySourceKind::TestFixture,
        started_at_utc: TIME.into(),
        observed_at_utc: TIME.into(),
        completed_at_utc: TIME.into(),
        limits: vec![QuotaLimit {
            limit_id: "codex".into(),
            limit_name: Some("Synthetic limit".into()),
            primary: Some(QuotaWindow {
                used_percent: 0,
                window_duration_mins: Some(300),
                resets_at_unix: Some(253_402_300_799),
            }),
            secondary: None,
            reached_type: Some("usage".into()),
            monthly_credit_limit: Some("0000.01".into()),
            spend_control_reached: Some(false),
        }],
        available_reset_credits: Some("9223372036854775807".into()),
        reset_credit_details: Some(vec![QuotaCreditDetail {
            title: Some("Synthetic grant".into()),
            status: QuotaCreditStatus::Unknown,
            granted_at_unix: -62_135_596_800,
            expires_at_unix: None,
        }]),
    }
}
fn association(r: &QuotaReading, previous: Option<String>) -> ProfileBindingInput {
    ProfileBindingInput {
        profile_id: r.binding.profile_id.clone(),
        expected_home_reference: r.binding.home_reference.clone(),
        expected_previous_binding_id: previous,
        new_binding_id: r.binding.binding_id.clone(),
        evidence: r.identity_evidence.clone(),
        observed_at_utc: r.observed_at_utc.clone(),
    }
}
fn fixture() -> (tempfile::TempDir, Storage, QuotaReading) {
    let dir = crate::tests::private_directory();
    let mut storage = Storage::create(&dir.path().join("quota.db"), &KEY).unwrap();
    storage
        .create_profile(&id(1), "Synthetic quota", &id(2))
        .unwrap();
    let r = reading(
        4,
        ProfileBinding {
            profile_id: id(1),
            home_reference: id(2),
            binding_id: id(3),
        },
    );
    (dir, storage, r)
}
fn all(s: &Storage) -> Vec<Vec<Vec<rusqlite::types::Value>>> {
    crate::backup_validation::activity_records(&s.connection)
}

#[test]
fn quota_cache_preserves_each_window_and_credit_null_without_normalizing_strings() {
    let (_dir, mut storage, mut r) = fixture();
    r.limits[0].primary.as_mut().unwrap().window_duration_mins = None;
    r.limits[0].primary.as_mut().unwrap().resets_at_unix = None;
    r.limits[0].secondary = Some(QuotaWindow {
        used_percent: 100,
        window_duration_mins: Some(9_007_199_254_740_991),
        resets_at_unix: Some(-62_135_596_800),
    });
    r.limits[0].limit_name = None;
    r.limits[0].reached_type = None;
    r.limits[0].spend_control_reached = Some(true);
    r.limits.push(QuotaLimit {
        limit_id: "without-windows".into(),
        limit_name: None,
        primary: None,
        secondary: None,
        reached_type: None,
        monthly_credit_limit: None,
        spend_control_reached: None,
    });
    r.reset_credit_details
        .as_mut()
        .unwrap()
        .push(QuotaCreditDetail {
            title: None,
            status: QuotaCreditStatus::Available,
            granted_at_unix: 0,
            expires_at_unix: Some(0),
        });
    storage
        .accept_quota(r.clone(), Some(association(&r, None)))
        .unwrap();
    let saved = storage.current_quota(&id(1)).unwrap().unwrap();
    assert_eq!(saved.limits, r.limits);
    assert_eq!(saved.reset_credit_details, r.reset_credit_details);
    assert_eq!(saved.available_reset_credits, r.available_reset_credits);
}

#[test]
fn quota_reset_seconds_store_as_exact_utc_text_across_epoch_and_calendar_boundaries() {
    let (_dir, mut storage, mut r) = fixture();
    for (index, (unix, utc)) in [
        (-62_135_596_800, "0001-01-01T00:00:00.000Z"),
        (-2_208_988_800, "1900-01-01T00:00:00.000Z"),
        (-1, "1969-12-31T23:59:59.000Z"),
        (0, "1970-01-01T00:00:00.000Z"),
        (951_782_400, "2000-02-29T00:00:00.000Z"),
        (1_709_164_800, "2024-02-29T00:00:00.000Z"),
        (253_402_300_799, "9999-12-31T23:59:59.000Z"),
    ]
    .into_iter()
    .enumerate()
    {
        r.run_id = id(index as u32 + 10);
        r.limits[0].primary.as_mut().unwrap().resets_at_unix = Some(unix);
        let input = if index == 0 {
            Some(association(&r, None))
        } else {
            None
        };
        storage.accept_quota(r.clone(), input).unwrap();
        let history: (String, String) = storage.connection.query_row(
            "SELECT typeof(primary_resets_at_utc),primary_resets_at_utc FROM quota_limits WHERE collection_run_id=?1",
            [&r.run_id], |row| Ok((row.get(0)?, row.get(1)?)),
        ).unwrap();
        let current: String = storage.connection.query_row("SELECT resets_at_utc FROM current_quota WHERE profile_id=?1 AND window_role='primary'", [&id(1)], |row| row.get(0)).unwrap();
        assert_eq!(history, ("text".into(), utc.into()));
        assert_eq!(current, utc);
        assert_eq!(
            storage.current_quota(&id(1)).unwrap().unwrap().limits[0]
                .primary
                .as_ref()
                .unwrap()
                .resets_at_unix,
            Some(unix)
        );
    }
}

#[test]
fn quota_atomic_reassociation_cannot_bypass_source_time_or_identity_fences() {
    let (_dir, mut storage, mut r) = fixture();
    r.observed_at_utc = LATER.into();
    r.completed_at_utc = LATER.into();
    storage
        .accept_quota(r.clone(), Some(association(&r, None)))
        .unwrap();
    let before = all(&storage);
    let mut next = r.clone();
    next.run_id = id(5);
    next.binding.binding_id = id(6);
    next.observed_at_utc = TIME.into();
    next.completed_at_utc = TIME.into();
    let input = association(&next, Some(r.binding.binding_id.clone()));
    assert_eq!(
        storage.accept_quota(next, Some(input)),
        Err(ActivityError::StaleObservation)
    );
    let mut changed = r.clone();
    changed.run_id = id(7);
    changed.identity_evidence.email = "different@example.invalid".into();
    assert_eq!(
        storage.accept_quota(changed, None),
        Err(ActivityError::IdentityChanged)
    );
    assert_eq!(all(&storage), before);
}

#[test]
fn activity_reassociation_clears_quota_cache_atomically_and_retains_accepted_quota_history() {
    let (_dir, mut storage, r) = fixture();
    storage
        .accept_quota(r.clone(), Some(association(&r, None)))
        .unwrap();
    let before = all(&storage);
    let mut new_quota_binding = r.clone();
    new_quota_binding.binding.binding_id = id(6);
    new_quota_binding.observed_at_utc = LATER.into();
    let input = association(&new_quota_binding, Some(r.binding.binding_id));
    let activity = crate::ActivityReading {
        run_id: id(5),
        binding: new_quota_binding.binding.clone(),
        identity_evidence: r.identity_evidence,
        provider_version: r.provider_version,
        source_kind: r.source_kind,
        started_at_utc: TIME.into(),
        observed_at_utc: LATER.into(),
        completed_at_utc: LATER.into(),
        summary: crate::ActivitySummaryValues::default(),
        daily_usage: None,
    };
    storage.connection.execute_batch("CREATE TRIGGER fault BEFORE INSERT ON activity_summaries BEGIN SELECT RAISE(ABORT,'synthetic activity write fault'); END").unwrap();
    assert_eq!(
        storage.accept_activity_with_binding(activity.clone(), Some(input)),
        Err(ActivityError::WriteFailed)
    );
    assert_eq!(all(&storage), before);
    storage
        .connection
        .execute_batch("DROP TRIGGER fault")
        .unwrap();
    let input = association(&new_quota_binding, Some(id(3)));
    storage
        .accept_activity_with_binding(activity, Some(input))
        .unwrap();
    assert_eq!(storage.current_quota(&id(1)).unwrap(), None);
    assert!(storage.current_activity(&id(1)).unwrap().is_some());
    let after = all(&storage);
    for index in [0, 1, 7, 8, 9] {
        for row in &before[index] {
            assert!(after[index].contains(row));
        }
    }
}

#[test]
fn quota_backup_and_encrypted_checkpoint_restore_all_fields_and_history() {
    let (dir, mut storage, r) = fixture();
    storage
        .accept_quota(r.clone(), Some(association(&r, None)))
        .unwrap();
    storage
        .create_profile(&id(30), "Activity only", &id(31))
        .unwrap();
    crate::backup_validation::activity_fixture(&mut storage, &id(30), &id(31), 100);
    let expected = all(&storage);
    let saved = storage.current_quota(&id(1)).unwrap().unwrap();
    let snapshot = storage.backup_snapshot().unwrap();
    let bytes = snapshot.bytes.clone();
    crate::backup_validation::with_validated_snapshot::<_, crate::BackupValidationError>(
        &snapshot.bytes,
        crate::CURRENT_SCHEMA_VERSION,
        true,
        |source, _| {
            let copy = crate::backup_validation::trusted_memory_copy(
                source,
                "main",
                crate::CURRENT_SCHEMA_VERSION,
            )?;
            assert_eq!(crate::backup_validation::activity_records(&copy), expected);
            Ok(())
        },
    )
    .unwrap();
    let checkpoint = dir.path().join("quota-checkpoint");
    storage.create_encrypted_checkpoint(&checkpoint).unwrap();
    let encrypted_bytes = std::fs::read(&checkpoint).unwrap();
    assert!(!encrypted_bytes.starts_with(b"SQLite format 3"));
    let mut next = r.clone();
    next.run_id = id(5);
    next.observed_at_utc = LATER.into();
    next.completed_at_utc = LATER.into();
    next.limits.clear();
    next.available_reset_credits = None;
    next.reset_credit_details = None;
    let revision = storage.restore_state().unwrap().revision;
    storage.accept_quota(next, None).unwrap();
    assert_ne!(storage.restore_state().unwrap().revision, revision);
    let recovered = storage.read_encrypted_checkpoint(&checkpoint).unwrap();
    let homes =
        crate::validate_checkpoint_restore_input(&recovered.bytes, crate::CURRENT_SCHEMA_VERSION)
            .unwrap()
            .homes;
    let revision = storage.restore_state().unwrap().revision;
    storage
        .replace_records_from_checkpoint(
            &recovered.bytes,
            crate::CURRENT_SCHEMA_VERSION,
            &revision,
            &homes,
        )
        .unwrap();
    assert_eq!(all(&storage), expected);
    assert_eq!(storage.current_quota(&id(1)).unwrap(), Some(saved.clone()));
    assert_eq!(std::fs::read(&checkpoint).unwrap(), encrypted_bytes);

    let mut target = Storage::create(&dir.path().join("quota-target"), &KEY).unwrap();
    let revision = target.restore_state().unwrap().revision;
    let homes = [
        crate::RestoreHomeMapping {
            profile_id: id(1),
            managed_home_reference: id(51),
        },
        crate::RestoreHomeMapping {
            profile_id: id(30),
            managed_home_reference: id(52),
        },
    ];
    target
        .replace_records_from_backup(
            &snapshot.bytes,
            crate::CURRENT_SCHEMA_VERSION,
            &revision,
            &homes,
        )
        .unwrap();
    assert_eq!(all(&target), expected);
    assert_eq!(target.current_quota(&id(1)).unwrap(), Some(saved));
    assert_eq!(*snapshot.bytes, *bytes);
}

#[test]
fn quota_authenticated_snapshot_semantics_reject_malformed_fields_and_incomplete_caches() {
    for sql in [
        "UPDATE quota_snapshots SET available_reset_credits='01'",
        "UPDATE quota_snapshots SET available_reset_credits='9223372036854775808'",
        "UPDATE quota_snapshots SET available_reset_credits=NULL",
        "UPDATE quota_snapshots SET credit_details_state='empty'",
        "UPDATE quota_limits SET spend_control_reached=2",
        "UPDATE quota_limits SET primary_used_percent=101",
        "UPDATE quota_limits SET primary_duration_mins=-1",
        "UPDATE quota_limits SET secondary_duration_mins=5",
        "UPDATE quota_limits SET primary_resets_at_utc='10000-01-01T00:00:00.000Z'",
        "UPDATE quota_limits SET monthly_credit_limit=char(10)",
        "UPDATE credit_snapshots SET ordinal=1",
        "UPDATE credit_snapshots SET status='invented'",
        "UPDATE credit_snapshots SET expires_at_unix=-62135596801",
        "DELETE FROM current_quota",
        "UPDATE current_quota SET used_percent=1",
        "UPDATE current_quota_readings SET updated_at_utc='2026-10-18T12:00:01.000Z'",
        "DELETE FROM profile_bindings",
        "DELETE FROM quota_snapshots",
    ] {
        let (_dir, mut storage, r) = fixture();
        storage
            .accept_quota(r.clone(), Some(association(&r, None)))
            .unwrap();
        storage
            .connection
            .execute_batch("PRAGMA foreign_keys=OFF; PRAGMA ignore_check_constraints=ON")
            .unwrap();
        storage.connection.execute_batch(sql).unwrap();
        let snapshot = storage.backup_snapshot().unwrap();
        let original = snapshot.bytes.clone();
        assert_eq!(
            crate::validate_backup_snapshot(&snapshot.bytes, crate::CURRENT_SCHEMA_VERSION),
            Err(crate::BackupValidationError::InvalidDatabase),
            "{sql}"
        );
        assert_eq!(*snapshot.bytes, *original);
    }
}

#[path = "quota_migration_tests.rs"]
mod migration_tests;

#[path = "quota_reset_tests.rs"]
mod reset_tests;

#[path = "alert_tests.rs"]
mod alert_tests;
#[test]
fn quota_first_exact_nullable_packet_reopens_and_is_readable_when_archived() {
    let (dir, mut storage, r) = fixture();
    let commit = storage
        .accept_quota(r.clone(), Some(association(&r, None)))
        .unwrap();
    assert_eq!(
        storage.profile_binding(&id(1)).unwrap(),
        Some(r.binding.clone())
    );
    assert_eq!(storage.current_activity(&id(1)).unwrap(), None);
    let saved = storage.current_quota(&id(1)).unwrap().unwrap();
    assert_eq!(saved.commit, commit);
    assert_eq!(saved.limits, r.limits);
    assert_eq!(saved.available_reset_credits, r.available_reset_credits);
    assert_eq!(saved.reset_credit_details, r.reset_credit_details);
    storage.set_profile_archived(&id(1), true).unwrap();
    let exact = all(&storage);
    drop(storage);
    let storage = Storage::open(&dir.path().join("quota.db"), &KEY).unwrap();
    assert_eq!(storage.current_quota(&id(1)).unwrap(), Some(saved));
    assert_eq!(all(&storage), exact);
}
#[test]
fn quota_full_snapshot_null_empty_zero_and_error_replacements_preserve_history() {
    let (_dir, mut storage, r) = fixture();
    storage
        .accept_quota(r.clone(), Some(association(&r, None)))
        .unwrap();
    let first = all(&storage);
    let mut next = r.clone();
    next.run_id = id(5);
    next.observed_at_utc = LATER.into();
    next.completed_at_utc = LATER.into();
    next.limits.clear();
    next.available_reset_credits = Some("0".into());
    next.reset_credit_details = Some(vec![]);
    storage.accept_quota(next.clone(), None).unwrap();
    let current = storage.current_quota(&id(1)).unwrap().unwrap();
    assert!(current.limits.is_empty());
    assert_eq!(current.available_reset_credits.as_deref(), Some("0"));
    assert_eq!(current.reset_credit_details, Some(vec![]));
    assert_eq!(
        storage
            .connection
            .query_row("SELECT count(*) FROM current_quota", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    next.run_id = id(6);
    next.available_reset_credits = None;
    next.reset_credit_details = None;
    storage.accept_quota(next.clone(), None).unwrap();
    assert_eq!(
        storage
            .current_quota(&id(1))
            .unwrap()
            .unwrap()
            .available_reset_credits,
        None
    );
    let before = all(&storage);
    assert_eq!(
        storage.accept_quota(next.clone(), None),
        Err(ActivityError::AlreadyAccepted)
    );
    next.run_id = id(7);
    next.observed_at_utc = TIME.into();
    assert_eq!(
        storage.accept_quota(next, None),
        Err(ActivityError::StaleObservation)
    );
    assert_eq!(all(&storage), before);
    for index in [0, 1, 6, 7, 8, 9] {
        for row in &first[index] {
            assert!(before[index].contains(row));
        }
    }
}
#[test]
fn quota_and_activity_share_one_binding_and_reassociation_retains_both_histories() {
    let (_dir, mut storage, mut r) = fixture();
    crate::backup_validation::activity_fixture(&mut storage, &id(1), &id(2), 100);
    let old = storage.profile_binding(&id(1)).unwrap().unwrap();
    r.binding = old.clone();
    r.identity_evidence = storage.profile_binding_evidence(&old).unwrap();
    storage.accept_quota(r.clone(), None).unwrap();
    let before = all(&storage);
    let old_activity = storage.current_activity(&id(1)).unwrap();
    assert!(old_activity.is_some());
    let mut changed = r.clone();
    changed.run_id = id(8);
    changed.binding.binding_id = id(9);
    changed.identity_evidence.email = "new-synthetic@example.invalid".into();
    changed.observed_at_utc = LATER.into();
    changed.completed_at_utc = LATER.into();
    let input = association(&changed, Some(old.binding_id));
    storage.accept_quota(changed.clone(), Some(input)).unwrap();
    assert_eq!(storage.current_activity(&id(1)).unwrap(), None);
    assert_eq!(
        storage.current_quota(&id(1)).unwrap().unwrap().binding,
        changed.binding
    );
    let after = all(&storage);
    for index in [0, 1, 2, 5, 7, 8, 9] {
        for row in &before[index] {
            assert!(after[index].contains(row), "history table index {index}");
        }
    }
}
#[test]
fn quota_reassociation_rolls_back_all_current_and_history_rows_at_each_write_stage() {
    for stage in [
        "INSERT ON profile_identity_events",
        "UPDATE ON profile_bindings",
        "INSERT ON collection_runs",
        "INSERT ON quota_snapshots",
        "DELETE ON current_quota",
        "DELETE ON current_quota_readings",
        "INSERT ON current_quota_readings",
        "INSERT ON quota_limits",
        "INSERT ON current_quota",
        "INSERT ON credit_snapshots",
    ] {
        let (_dir, mut storage, r) = fixture();
        storage
            .accept_quota(r.clone(), Some(association(&r, None)))
            .unwrap();
        let before = all(&storage);
        let mut next = r.clone();
        next.run_id = id(5);
        next.binding.binding_id = id(6);
        next.observed_at_utc = LATER.into();
        next.completed_at_utc = LATER.into();
        storage.connection.execute_batch(&format!("CREATE TRIGGER fault BEFORE {stage} BEGIN SELECT RAISE(ABORT,'synthetic quota write fault'); END;")).unwrap();
        let input = association(&next, Some(r.binding.binding_id));
        let result = storage.accept_quota(next, Some(input));
        assert_eq!(result, Err(ActivityError::WriteFailed), "{stage}");
        assert_eq!(all(&storage), before, "{stage}");
    }
}
#[test]
fn quota_validation_shared_cas_and_profile_isolation_refuse_without_writes() {
    let (_dir, mut storage, r) = fixture();
    storage
        .accept_quota(r.clone(), Some(association(&r, None)))
        .unwrap();
    let before = all(&storage);
    for fault in 0..10 {
        let mut invalid = r.clone();
        invalid.run_id = id(10 + fault);
        match fault {
            0 => invalid.limits[0].primary.as_mut().unwrap().used_percent = 101,
            1 => {
                invalid.limits[0]
                    .primary
                    .as_mut()
                    .unwrap()
                    .window_duration_mins = Some(0)
            }
            2 => invalid.limits[0].primary.as_mut().unwrap().resets_at_unix = Some(i64::MAX),
            3 => invalid.available_reset_credits = Some("01".into()),
            4 => invalid.available_reset_credits = Some("9223372036854775808".into()),
            5 => {
                invalid.reset_credit_details =
                    Some(vec![
                        invalid.reset_credit_details.as_ref().unwrap()[0]
                            .clone();
                        33
                    ])
            }
            6 => invalid.limits.push(invalid.limits[0].clone()),
            7 => invalid.limits[0].monthly_credit_limit = Some("\n".into()),
            8 => invalid.observed_at_utc = "2026-10-18T12:00:00Z".into(),
            _ => invalid.available_reset_credits = None,
        };
        assert_eq!(
            storage.accept_quota(invalid, None),
            Err(ActivityError::InvalidInput)
        );
        assert_eq!(all(&storage), before);
    }
    let mut next = r.clone();
    next.run_id = id(20);
    next.binding.binding_id = id(21);
    let input = association(&next, None);
    assert_eq!(
        storage.accept_quota(next, Some(input)),
        Err(ActivityError::BindingChanged)
    );
    assert_eq!(all(&storage), before);
    storage
        .create_profile(&id(30), "Separate profile", &id(31))
        .unwrap();
    let other = reading(
        33,
        ProfileBinding {
            profile_id: id(30),
            home_reference: id(31),
            binding_id: id(32),
        },
    );
    storage
        .accept_quota(other.clone(), Some(association(&other, None)))
        .unwrap();
    assert_eq!(
        storage.current_quota(&id(1)).unwrap().unwrap().limits,
        r.limits
    );
}

#[path = "reset_notification_tests.rs"]
mod reset_notification_tests;
