use super::*;
use crate::QuotaResetState;

fn next(r: &QuotaReading, n: u32, used: u8, reset: Option<i64>) -> QuotaReading {
    let mut r = r.clone();
    r.run_id = id(100 + n);
    r.observed_at_utc = format!("2026-10-18T12:00:{n:02}.000Z");
    r.completed_at_utc = r.observed_at_utc.clone();
    r.limits[0].primary.as_mut().unwrap().used_percent = used;
    r.limits[0].primary.as_mut().unwrap().resets_at_unix = reset;
    r
}
fn baseline() -> (tempfile::TempDir, Storage, QuotaReading) {
    let (dir, mut storage, mut r) = fixture();
    r.limits[0].primary.as_mut().unwrap().used_percent = 80;
    r.limits[0].primary.as_mut().unwrap().resets_at_unix = Some(0);
    storage
        .accept_quota(r.clone(), Some(association(&r, None)))
        .unwrap();
    (dir, storage, r)
}

#[test]
fn quota_reset_confirms_once_and_keeps_generation_and_pending_deadline_after_reopen() {
    let (dir, mut storage, r) = baseline();
    let initial = storage.current_quota(&id(1)).unwrap().unwrap();
    assert_eq!(
        initial.reset_assessments[0].state,
        QuotaResetState::NotConfirmed
    );
    assert_eq!(initial.reset_assessments[0].pending_reset_at_unix, Some(0));
    assert_eq!(initial.reset_assessments[0].generation_run_id, None);
    let confirmed = next(&r, 1, 20, Some(1_893_456_000));
    storage.accept_quota(confirmed.clone(), None).unwrap();
    let saved = storage.current_quota(&id(1)).unwrap().unwrap();
    assert_eq!(saved.reset_assessments[0].state, QuotaResetState::Confirmed);
    assert_eq!(
        saved.reset_assessments[0].generation_run_id,
        Some(confirmed.run_id.clone())
    );
    assert_eq!(
        saved.reset_assessments[0].pending_reset_at_unix,
        Some(1_893_456_000)
    );
    let repeated = next(&confirmed, 2, 25, Some(1_893_456_000));
    storage.accept_quota(repeated.clone(), None).unwrap();
    assert_eq!(
        storage
            .current_quota(&id(1))
            .unwrap()
            .unwrap()
            .reset_assessments,
        saved.reset_assessments
    );
    let mut uncertain = next(&repeated, 3, 30, Some(2_208_988_800));
    uncertain.provider_version = "different-reviewed-version".into();
    storage.accept_quota(uncertain.clone(), None).unwrap();
    let current = storage.current_quota(&id(1)).unwrap().unwrap();
    assert_eq!(current.limits, uncertain.limits);
    assert_eq!(
        current.reset_assessments[0].state,
        QuotaResetState::NotConfirmed
    );
    assert_eq!(
        current.reset_assessments[0].generation_run_id,
        Some(confirmed.run_id)
    );
    assert_eq!(
        current.reset_assessments[0].pending_reset_at_unix,
        Some(1_893_456_000)
    );
    let before = all(&storage);
    drop(storage);
    let storage = Storage::open(&dir.path().join("quota.db"), &KEY).unwrap();
    assert_eq!(storage.current_quota(&id(1)).unwrap(), Some(current));
    assert_eq!(all(&storage), before);
}

#[test]
fn quota_reset_unconfirmed_moves_and_missing_reset_never_clear_pending_due() {
    let (_dir, mut storage, r) = baseline();
    let mut current = r;
    for (n, used, reset) in [
        (1, 85, Some(1_893_456_000)),
        (2, 10, None),
        (3, 5, Some(2_208_988_800)),
    ] {
        current = next(&current, n, used, reset);
        storage.accept_quota(current.clone(), None).unwrap();
        let saved = storage.current_quota(&id(1)).unwrap().unwrap();
        assert_eq!(saved.limits, current.limits);
        assert_eq!(saved.reset_assessments[0].pending_reset_at_unix, Some(0));
        assert_eq!(saved.reset_assessments[0].generation_run_id, None);
        assert_eq!(
            saved.reset_assessments[0].state,
            QuotaResetState::NotConfirmed
        );
    }
}

#[test]
fn quota_reset_every_missing_rule_or_incompatible_bucket_preserves_measurement_without_generation()
{
    for fault in 0..10 {
        let (_dir, mut storage, mut r) = fixture();
        r.limits[0].primary.as_mut().unwrap().used_percent = 80;
        r.limits[0].primary.as_mut().unwrap().resets_at_unix =
            Some(if fault == 0 { 1_893_456_000 } else { 0 });
        if fault == 1 {
            r.limits[0].primary.as_mut().unwrap().resets_at_unix = None;
        }
        storage
            .accept_quota(r.clone(), Some(association(&r, None)))
            .unwrap();
        let mut changed = next(&r, 1, 20, Some(2_208_988_800));
        match fault {
            2 => changed.limits[0].primary.as_mut().unwrap().resets_at_unix = Some(0),
            3 => changed.limits[0].primary.as_mut().unwrap().used_percent = 80,
            4 => {
                changed.limits[0]
                    .primary
                    .as_mut()
                    .unwrap()
                    .window_duration_mins = None
            }
            5 => {
                changed.limits[0]
                    .primary
                    .as_mut()
                    .unwrap()
                    .window_duration_mins = Some(301)
            }
            6 => changed.limits[0].limit_id = "different".into(),
            7 => changed.limits[0].secondary = changed.limits[0].primary.take(),
            8 => changed.provider_version = "different-reviewed-version".into(),
            9 => changed.source_kind = ActivitySourceKind::OfficialArtifact,
            _ => {}
        }
        storage.accept_quota(changed.clone(), None).unwrap();
        let saved = storage.current_quota(&id(1)).unwrap().unwrap();
        assert_eq!(saved.limits, changed.limits, "fault {fault}");
        assert_eq!(
            saved.reset_assessments[0].state,
            QuotaResetState::NotConfirmed,
            "fault {fault}"
        );
        assert_eq!(
            saved.reset_assessments[0].generation_run_id, None,
            "fault {fault}"
        );
        if fault == 8 || fault == 9 {
            assert_eq!(saved.reset_assessments[0].pending_reset_at_unix, Some(0));
        }
    }
}

#[test]
fn quota_reset_uses_immediately_previous_full_snapshot_including_an_empty_map() {
    let (_dir, mut storage, r) = baseline();
    let mut empty = next(&r, 1, 80, Some(0));
    empty.limits.clear();
    storage.accept_quota(empty.clone(), None).unwrap();
    let changed = next(&r, 2, 20, Some(1_893_456_000));
    storage.accept_quota(changed.clone(), None).unwrap();
    let saved = storage.current_quota(&id(1)).unwrap().unwrap();
    assert_eq!(
        saved.reset_assessments[0].state,
        QuotaResetState::NotConfirmed
    );
    assert_eq!(saved.reset_assessments[0].generation_run_id, None);
    let previous: String = storage
        .connection
        .query_row(
            "SELECT previous_quota_run_id FROM quota_snapshots WHERE id=?1",
            [&changed.run_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(previous, empty.run_id);
}

#[test]
fn quota_reset_assessment_write_failure_rolls_back_packet_and_generation() {
    let (_dir, mut storage, r) = baseline();
    let before = all(&storage);
    storage.connection.execute_batch("CREATE TRIGGER reset_fault BEFORE INSERT ON quota_reset_assessments BEGIN SELECT RAISE(ABORT,'synthetic reset assessment failure'); END").unwrap();
    assert_eq!(
        storage.accept_quota(next(&r, 1, 20, Some(1_893_456_000)), None),
        Err(ActivityError::WriteFailed)
    );
    assert_eq!(all(&storage), before);
}

#[test]
fn quota_reset_authenticated_import_recomputes_proof_and_rejects_forged_links_and_pending_state() {
    for sql in [
        "UPDATE quota_reset_assessments SET state='confirmed',generation_run_id=collection_run_id WHERE collection_run_id='00000004-0000-4000-8000-000000000000'",
        "UPDATE quota_reset_assessments SET pending_reset_at_utc='1970-01-01T00:00:00.000Z' WHERE state='confirmed'",
        "UPDATE quota_reset_assessments SET generation_run_id='00000004-0000-4000-8000-000000000000' WHERE state='confirmed'",
        "DELETE FROM quota_reset_assessments WHERE state='confirmed'",
        "UPDATE quota_reset_assessments SET window_role='secondary' WHERE state='confirmed'",
        "UPDATE quota_snapshots SET previous_quota_run_id=id WHERE previous_quota_run_id IS NOT NULL",
        "UPDATE quota_snapshots SET previous_quota_run_id=NULL WHERE previous_quota_run_id IS NOT NULL",
    ] {
        let (_dir, mut storage, r) = baseline();
        storage
            .accept_quota(next(&r, 1, 20, Some(1_893_456_000)), None)
            .unwrap();
        storage
            .connection
            .execute_batch("PRAGMA foreign_keys=OFF; PRAGMA ignore_check_constraints=ON")
            .unwrap();
        storage.connection.execute_batch(sql).unwrap();
        let snapshot = storage.backup_snapshot().unwrap();
        let bytes = snapshot.bytes.clone();
        assert_eq!(
            crate::validate_backup_snapshot(&snapshot.bytes, crate::CURRENT_SCHEMA_VERSION),
            Err(crate::BackupValidationError::InvalidDatabase),
            "{sql}"
        );
        assert_eq!(*snapshot.bytes, *bytes);
    }
    // A coordinated rewrite must not skip a newer accepted full measurement.
    let (_dir, mut storage, r) = baseline();
    let middle = next(&r, 1, 85, Some(1_893_456_000));
    let latest = next(&middle, 2, 20, Some(2_051_222_400));
    storage.accept_quota(middle.clone(), None).unwrap();
    storage.accept_quota(latest.clone(), None).unwrap();
    let original = storage.backup_snapshot().unwrap();
    crate::validate_backup_snapshot(&original.bytes, crate::CURRENT_SCHEMA_VERSION).unwrap();
    storage
        .connection
        .execute(
            "UPDATE quota_snapshots SET previous_quota_run_id=NULL WHERE id=?1",
            [&middle.run_id],
        )
        .unwrap();
    storage
        .connection
        .execute(
            "UPDATE quota_reset_assessments SET pending_reset_at_utc=?2 WHERE collection_run_id=?1",
            rusqlite::params![middle.run_id, unix_to_utc(1_893_456_000)],
        )
        .unwrap();
    storage
        .connection
        .execute(
            "UPDATE quota_snapshots SET previous_quota_run_id=?2 WHERE id=?1",
            rusqlite::params![latest.run_id, r.run_id],
        )
        .unwrap();
    storage.connection.execute("UPDATE quota_reset_assessments SET state='confirmed',generation_run_id=collection_run_id,pending_reset_at_utc=?2 WHERE collection_run_id=?1", rusqlite::params![latest.run_id, unix_to_utc(2_051_222_400)]).unwrap();
    let forged = storage.backup_snapshot().unwrap();
    let bytes = forged.bytes.clone();
    assert_eq!(
        crate::validate_backup_snapshot(&forged.bytes, crate::CURRENT_SCHEMA_VERSION),
        Err(crate::BackupValidationError::InvalidDatabase)
    );
    assert_eq!(*forged.bytes, *bytes);
}

#[test]
fn quota_reset_confirmation_and_latched_due_survive_encrypted_backup_restore_and_undo() {
    let (dir, mut storage, r) = baseline();
    let confirmed = next(&r, 1, 20, Some(1_893_456_000));
    storage.accept_quota(confirmed.clone(), None).unwrap();
    let checkpoint = dir.path().join("confirmed-reset-checkpoint");
    storage.create_encrypted_checkpoint(&checkpoint).unwrap();
    let confirmed_records = all(&storage);
    let confirmed_value = storage.current_quota(&id(1)).unwrap();
    storage
        .accept_quota(next(&confirmed, 2, 30, Some(2_208_988_800)), None)
        .unwrap();
    let snapshot = storage.backup_snapshot().unwrap();
    let snapshot_bytes = snapshot.bytes.clone();
    let unresolved_records = all(&storage);
    let unresolved_value = storage.current_quota(&id(1)).unwrap();
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
    assert_eq!(all(&storage), confirmed_records);
    assert_eq!(storage.current_quota(&id(1)).unwrap(), confirmed_value);
    let revision = storage.restore_state().unwrap().revision;
    storage
        .replace_records_from_checkpoint(
            &snapshot.bytes,
            crate::CURRENT_SCHEMA_VERSION,
            &revision,
            &homes,
        )
        .unwrap();
    assert_eq!(all(&storage), unresolved_records);
    assert_eq!(storage.current_quota(&id(1)).unwrap(), unresolved_value);
    assert_eq!(*snapshot.bytes, *snapshot_bytes);
    let mut imported = Storage::create(&dir.path().join("imported-reset"), &KEY).unwrap();
    let revision = imported.restore_state().unwrap().revision;
    imported
        .replace_records_from_backup(
            &snapshot.bytes,
            crate::CURRENT_SCHEMA_VERSION,
            &revision,
            &[crate::RestoreHomeMapping {
                profile_id: id(1),
                managed_home_reference: id(50),
            }],
        )
        .unwrap();
    assert_eq!(all(&imported), unresolved_records);
    assert_eq!(imported.current_quota(&id(1)).unwrap(), unresolved_value);
}

fn legacy_v3(source: &Storage) -> rusqlite::Connection {
    let connection = rusqlite::Connection::open_in_memory().unwrap();
    for sql in &crate::MIGRATIONS[..3] {
        connection.execute_batch(sql).unwrap();
    }
    connection.execute("DELETE FROM settings", []).unwrap();
    for table in [
        "accounts",
        "profiles",
        "settings",
        "profile_identity_events",
        "collection_runs",
        "activity_summaries",
        "current_activity",
        "current_activity_days",
        "activity_day_history",
        "profile_bindings",
        "quota_snapshots",
        "quota_limits",
        "credit_snapshots",
        "current_quota_readings",
        "current_quota",
    ] {
        let columns = if table == "quota_snapshots" {
            "id,profile_id,binding_id,available_reset_credits,credit_details_state"
        } else {
            "*"
        };
        let mut select = source
            .connection
            .prepare(&format!("SELECT {columns} FROM {table}"))
            .unwrap();
        let count = select.column_count();
        let slots = (1..=count)
            .map(|n| format!("?{n}"))
            .collect::<Vec<_>>()
            .join(",");
        let mut insert = connection
            .prepare(&format!("INSERT INTO {table} VALUES ({slots})"))
            .unwrap();
        let mut rows = select.query([]).unwrap();
        while let Some(row) = rows.next().unwrap() {
            let values = (0..count)
                .map(|i| row.get_ref(i).map(rusqlite::types::ToSqlOutput::Borrowed))
                .collect::<rusqlite::Result<Vec<_>>>()
                .unwrap();
            insert.execute(rusqlite::params_from_iter(values)).unwrap();
        }
    }
    connection
        .execute_batch("PRAGMA user_version=3; PRAGMA foreign_keys=ON")
        .unwrap();
    crate::backup_validation::validate_connection(&connection, 3).unwrap();
    connection
}

#[test]
fn quota_reset_schema3_migration_rolls_back_and_old_archives_start_without_replayed_generations() {
    let (_dir, mut storage, r) = baseline();
    let confirmed = next(&r, 1, 20, Some(1_893_456_000));
    storage.accept_quota(confirmed.clone(), None).unwrap();
    let mut legacy = legacy_v3(&storage);
    let bytes = zeroize::Zeroizing::new(legacy.serialize("main").unwrap().to_vec());
    let original = bytes.clone();
    let fault = format!(
        "{}SELECT synthetic_missing_column FROM settings;",
        crate::MIGRATIONS[3]
    );
    let migrations = [
        crate::MIGRATIONS[0],
        crate::MIGRATIONS[1],
        crate::MIGRATIONS[2],
        fault.as_str(),
    ];
    assert_eq!(
        crate::migrate(&mut legacy, &migrations),
        Err(crate::StorageError::MigrationFailed)
    );
    crate::backup_validation::validate_connection(&legacy, 3).unwrap();
    assert!(
        legacy
            .pragma_query_value(None, "foreign_keys", |row| row.get::<_, bool>(0))
            .unwrap()
    );
    crate::migrate_data_connection(&mut legacy).unwrap();
    let saved = crate::quota::load_snapshot(&legacy, "main", &confirmed.run_id).unwrap();
    assert_eq!(saved.limits, confirmed.limits);
    assert_eq!(
        saved.reset_assessments[0].state,
        QuotaResetState::NotConfirmed
    );
    assert_eq!(saved.reset_assessments[0].generation_run_id, None);
    assert_eq!(
        saved.reset_assessments[0].pending_reset_at_unix,
        Some(1_893_456_000)
    );
    crate::backup_validation::with_validated_snapshot::<_, crate::BackupValidationError>(
        &bytes,
        3,
        true,
        |copy, _| {
            let saved = crate::quota::load_snapshot(copy, "main", &confirmed.run_id).unwrap();
            assert_eq!(
                saved.reset_assessments[0].state,
                QuotaResetState::NotConfirmed
            );
            assert_eq!(saved.reset_assessments[0].generation_run_id, None);
            assert_eq!(saved.limits, confirmed.limits);
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(*bytes, *original);
}
