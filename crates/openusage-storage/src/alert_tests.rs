use super::*;

pub(super) fn baseline() -> (tempfile::TempDir, Storage, QuotaReading) {
    let (directory, mut storage, mut reading) = fixture();
    reading.limits[0].primary.as_mut().unwrap().used_percent = 100;
    reading.limits[0].primary.as_mut().unwrap().resets_at_unix = Some(0);
    reading.limits[0].secondary = Some(QuotaWindow {
        used_percent: 100,
        window_duration_mins: Some(10_080),
        resets_at_unix: Some(0),
    });
    storage
        .accept_quota(reading.clone(), Some(association(&reading, None)))
        .unwrap();
    (directory, storage, reading)
}

pub(super) fn next(original: &QuotaReading, n: u32) -> QuotaReading {
    let mut reading = original.clone();
    reading.run_id = id(100 + n);
    reading.observed_at_utc = format!("2026-10-18T12:00:{n:02}.000Z");
    reading.completed_at_utc = reading.observed_at_utc.clone();
    let limit = &mut reading.limits[0];
    for window in [&mut limit.primary, &mut limit.secondary]
        .into_iter()
        .flatten()
    {
        window.used_percent = 100 - n as u8;
        window.resets_at_unix = Some(i64::from(n));
    }
    reading
}

#[test]
fn reset_history_is_atomic_once_per_window_and_survives_restart_and_archiving() {
    let (directory, mut storage, original) = baseline();
    assert!(storage.alert_history().unwrap().events.is_empty());
    let before = storage.restore_state().unwrap().revision;
    let confirmed = next(&original, 1);
    storage
        .accept_collection(Some(confirmed.clone()), None, None)
        .unwrap();
    assert_ne!(storage.restore_state().unwrap().revision, before);
    let history = storage.alert_history().unwrap();
    assert_eq!(history.events.len(), 2);
    assert!(!history.truncated);
    assert_ne!(history.events[0].id, history.events[1].id);
    for event in &history.events {
        assert_eq!(event.profile_id, original.binding.profile_id);
        assert_eq!(event.limit_id, "codex");
        assert_eq!(event.generation_run_id, confirmed.run_id);
        assert_eq!(event.evidence_at_utc, confirmed.observed_at_utc);
        assert_eq!(event.source_kind, ActivitySourceKind::TestFixture);
        assert_eq!(
            event.delivery_state,
            if cfg!(feature = "native-test") {
                crate::AlertDeliveryState::Suppressed
            } else {
                crate::AlertDeliveryState::Recorded
            }
        );
    }
    let mut repeated = next(&original, 2);
    let limit = &mut repeated.limits[0];
    for window in [&mut limit.primary, &mut limit.secondary]
        .into_iter()
        .flatten()
    {
        window.resets_at_unix = Some(1);
    }
    storage.accept_quota(repeated, None).unwrap();
    assert_eq!(storage.alert_history().unwrap(), history);
    let records = all(&storage);
    drop(storage);
    let mut storage = Storage::open(&directory.path().join("quota.db"), &KEY).unwrap();
    assert_eq!(storage.alert_history().unwrap(), history);
    assert_eq!(all(&storage), records);
    storage.rename_profile(&id(1), "Renamed profile").unwrap();
    storage.set_profile_archived(&id(1), true).unwrap();
    let renamed = storage.alert_history().unwrap();
    assert!(
        renamed
            .events
            .iter()
            .all(|event| event.profile_label == "Renamed profile")
    );
    assert_eq!(renamed.events[0].id, history.events[0].id);
    let records = all(&storage);
    assert_eq!(
        storage.accept_quota(next(&original, 3), None),
        Err(ActivityError::ProfileUnavailable)
    );
    assert_eq!(all(&storage), records);
}

#[test]
fn reset_event_write_failure_rolls_back_reading_then_retry_records_both_windows() {
    let (_directory, mut storage, original) = baseline();
    let before = all(&storage);
    storage.connection.execute_batch("CREATE TRIGGER alert_fault BEFORE INSERT ON alert_events WHEN NEW.window_role='secondary' BEGIN SELECT RAISE(ABORT,'synthetic alert failure'); END").unwrap();
    let confirmed = next(&original, 1);
    assert_eq!(
        storage.accept_quota(confirmed.clone(), None),
        Err(ActivityError::WriteFailed)
    );
    assert_eq!(all(&storage), before);
    storage
        .connection
        .execute_batch("DROP TRIGGER alert_fault")
        .unwrap();
    storage.accept_quota(confirmed.clone(), None).unwrap();
    assert_eq!(storage.alert_history().unwrap().events.len(), 2);
    assert_eq!(
        storage.accept_quota(confirmed, None),
        Err(ActivityError::AlreadyAccepted)
    );
    assert_eq!(storage.alert_history().unwrap().events.len(), 2);
}

#[test]
fn reset_history_is_bounded_without_losing_older_records_and_restore_preserves_events() {
    let (directory, mut storage, original) = baseline();
    for n in 1..=51 {
        storage.accept_quota(next(&original, n), None).unwrap();
    }
    let history = storage.alert_history().unwrap();
    assert_eq!(history.events.len(), 100);
    assert!(history.truncated);
    assert_eq!(history.events[0].generation_run_id, id(151));
    let count: i64 = storage
        .connection
        .query_row("SELECT count(*) FROM alert_events", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 102);
    let checkpoint = directory.path().join("alert-checkpoint");
    storage.create_encrypted_checkpoint(&checkpoint).unwrap();
    let recovered = storage.read_encrypted_checkpoint(&checkpoint).unwrap();
    let metadata =
        crate::validate_checkpoint_restore_input(&recovered.bytes, recovered.schema_version)
            .unwrap();
    let revision = storage.restore_state().unwrap().revision;
    storage
        .replace_records_from_checkpoint(
            &recovered.bytes,
            recovered.schema_version,
            &revision,
            &metadata.homes,
        )
        .unwrap();
    assert_eq!(storage.alert_history().unwrap(), history);
    let snapshot = storage.backup_snapshot().unwrap();
    crate::validate_backup_snapshot(&snapshot.bytes, snapshot.schema_version).unwrap();
    let mut imported = Storage::create(&directory.path().join("imported-alerts"), &KEY).unwrap();
    let revision = imported.restore_state().unwrap().revision;
    imported
        .replace_records_from_backup(
            &snapshot.bytes,
            snapshot.schema_version,
            &revision,
            &[crate::RestoreHomeMapping {
                profile_id: id(1),
                managed_home_reference: id(50),
            }],
        )
        .unwrap();
    assert_eq!(imported.alert_history().unwrap(), history);
    assert_eq!(all(&imported), all(&storage));
}

#[test]
fn reset_event_validation_refuses_forged_proof_identity_time_and_delivery() {
    let (_directory, mut storage, original) = baseline();
    storage.accept_quota(next(&original, 1), None).unwrap();
    for sql in [
        "UPDATE alert_events SET id='not-a-uuid'",
        "UPDATE alert_events SET event_key='forged'",
        "UPDATE alert_events SET profile_id='00000099-0000-4000-8000-000000000000'",
        "UPDATE alert_events SET verified_identity_key='unverified-identity'",
        "UPDATE alert_events SET generation_run_id='00000004-0000-4000-8000-000000000000'",
        "UPDATE alert_events SET evidence_at_utc='2020-01-01T00:00:00.000Z'",
        "UPDATE alert_events SET source_kind='official_artifact'",
        "UPDATE alert_events SET delivery_state='delivered'",
        "UPDATE alert_events SET window_duration_mins=7",
    ] {
        let copy = crate::backup_validation::trusted_memory_copy(
            &storage.connection,
            "main",
            crate::CURRENT_SCHEMA_VERSION,
        )
        .unwrap();
        copy.execute_batch("PRAGMA foreign_keys=OFF;PRAGMA ignore_check_constraints=ON")
            .unwrap();
        copy.execute_batch(&format!(
            "{sql} WHERE id=(SELECT id FROM alert_events ORDER BY id LIMIT 1)"
        ))
        .unwrap();
        let bytes = copy.serialize("main").unwrap();
        assert_eq!(
            crate::validate_backup_snapshot(&bytes, crate::CURRENT_SCHEMA_VERSION),
            Err(crate::BackupValidationError::InvalidDatabase),
            "{sql}"
        );
    }
}

#[test]
fn schema_four_migration_and_archive_normalization_do_not_replay_reset_history() {
    let (directory, mut storage, original) = baseline();
    let confirmed = next(&original, 1);
    storage.accept_quota(confirmed.clone(), None).unwrap();
    // Reproduce an exact v4 catalog with real encrypted quota proof, before alert recording existed.
    storage
        .connection
        .execute_batch("DROP TABLE operational_alert_events;DROP TABLE operational_alert_outcomes;DROP TABLE operational_alert_state;DROP TABLE operational_alert_overrides;DROP TABLE operational_alert_preferences;DROP TABLE threshold_alert_events;DROP TABLE threshold_alert_preferences;DROP TABLE threshold_alert_overrides;DROP TABLE reset_notification_outbox;DROP TABLE reset_notification_receipts;DROP TABLE reset_alert_overrides;DROP TABLE reset_alert_preferences;DROP TABLE alert_events;DROP TABLE notification_event_ids;PRAGMA user_version=4")
        .unwrap();
    let mut legacy =
        crate::backup_validation::trusted_memory_copy(&storage.connection, "main", 4).unwrap();
    let bytes = zeroize::Zeroizing::new(legacy.serialize("main").unwrap().to_vec());
    let bad = format!(
        "{}SELECT synthetic_missing_column FROM settings;",
        crate::MIGRATIONS[4]
    );
    let migrations = [
        crate::MIGRATIONS[0],
        crate::MIGRATIONS[1],
        crate::MIGRATIONS[2],
        crate::MIGRATIONS[3],
        bad.as_str(),
    ];
    assert_eq!(
        crate::migrate(&mut legacy, &migrations),
        Err(crate::StorageError::MigrationFailed)
    );
    crate::backup_validation::validate_connection(&legacy, 4).unwrap();
    let original_bytes = bytes.clone();
    crate::backup_validation::with_validated_snapshot::<_, crate::BackupValidationError>(
        &bytes,
        4,
        true,
        |normalized, _| {
            let count: i64 = normalized
                .query_row("SELECT count(*) FROM alert_events", [], |r| r.get(0))
                .unwrap();
            assert_eq!(count, 0);
            assert_eq!(
                crate::quota::load_snapshot(normalized, "main", &confirmed.run_id)
                    .unwrap()
                    .reset_assessments[0]
                    .generation_run_id,
                Some(confirmed.run_id.clone())
            );
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(*bytes, *original_bytes);
    drop(storage);
    let mut storage = Storage::open(&directory.path().join("quota.db"), &KEY).unwrap();
    assert!(storage.alert_history().unwrap().events.is_empty());
    let mut repeated = next(&original, 2);
    let limit = &mut repeated.limits[0];
    for window in [&mut limit.primary, &mut limit.secondary]
        .into_iter()
        .flatten()
    {
        window.resets_at_unix = Some(1);
    }
    storage.accept_quota(repeated, None).unwrap();
    assert!(storage.alert_history().unwrap().events.is_empty());
    storage.accept_quota(next(&original, 3), None).unwrap();
    assert_eq!(storage.alert_history().unwrap().events.len(), 2);
}
