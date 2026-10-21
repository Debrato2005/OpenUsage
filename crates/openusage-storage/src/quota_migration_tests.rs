//! Synthetic legacy fixtures exercise the real SQLCipher migration and trusted-copy paths.
use super::*;
use rusqlite::{Connection, types::Value};
use std::{path::Path, sync::Arc};

const LEGACY_TABLES: &[&str] = &[
    "accounts",
    "profiles",
    "settings",
    "profile_identity_events",
    "collection_runs",
    "activity_summaries",
    "current_activity",
    "current_activity_days",
    "activity_day_history",
];

fn records(connection: &Connection) -> Vec<Vec<Vec<Value>>> {
    LEGACY_TABLES
        .iter()
        .map(|table| {
            let mut select = connection
                .prepare(&format!("SELECT * FROM {table} ORDER BY 1,2,3"))
                .unwrap();
            let count = select.column_count();
            select
                .query_map([], |row| (0..count).map(|column| row.get(column)).collect())
                .unwrap()
                .collect::<rusqlite::Result<_>>()
                .unwrap()
        })
        .collect()
}

fn legacy_v2(source: &Storage, path: &Path) -> Connection {
    let connection = Connection::open(path).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    connection
        .pragma_update(None, "key", format!("x'{}'", "69".repeat(32)))
        .unwrap();
    connection.execute_batch("PRAGMA cipher_memory_security=ON; PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; PRAGMA foreign_keys=OFF;").unwrap();
    for sql in &crate::MIGRATIONS[..2] {
        connection.execute_batch(sql).unwrap();
    }
    connection.execute("DELETE FROM settings", []).unwrap();
    for table in LEGACY_TABLES {
        let mut select = source
            .connection
            .prepare(&format!("SELECT * FROM {table}"))
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
        .execute_batch("PRAGMA user_version=2; PRAGMA foreign_keys=ON;")
        .unwrap();
    crate::backup_validation::validate_connection(&connection, 2).unwrap();
    connection
}

fn seed() -> (tempfile::TempDir, Storage) {
    let (dir, mut source, _) = fixture();
    crate::backup_validation::activity_fixture(&mut source, &id(1), &id(2), 100);
    (dir, source)
}

fn assert_legacy_unchanged(connection: &Connection, before: &[Vec<Vec<Value>>]) {
    assert_eq!(records(connection), before);
    assert_eq!(
        connection
            .pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
            .unwrap(),
        2
    );
    assert!(
        connection
            .pragma_query_value(None, "foreign_keys", |r| r.get::<_, bool>(0))
            .unwrap()
    );
    assert!(connection.is_autocommit());
    crate::backup_validation::validate_connection(connection, 2).unwrap();
}

#[test]
fn quota_v2_migration_rolls_back_every_rebuild_stage_fk_failure_and_commit_failure() {
    let (dir, source) = seed();
    let sql = crate::MIGRATIONS[2];
    let stages = [
        "INSERT INTO new_collection_runs",
        "DROP TABLE collection_runs",
        "ALTER TABLE new_collection_runs",
        "CREATE INDEX activity_summary_profile_time",
        "CREATE TABLE profile_bindings",
        "CREATE TABLE quota_snapshots",
    ];
    for (index, marker) in stages.iter().enumerate() {
        let path = dir.path().join(format!("fault-{index}"));
        let mut connection = legacy_v2(&source, &path);
        let before = records(&connection);
        let fault = format!(
            "{}SELECT synthetic_missing_column FROM settings;",
            &sql[..sql.find(marker).unwrap()]
        );
        let migrations = [crate::MIGRATIONS[0], crate::MIGRATIONS[1], fault.as_str()];
        assert_eq!(
            crate::migrate(&mut connection, &migrations),
            Err(crate::StorageError::MigrationFailed),
            "{marker}"
        );
        assert_legacy_unchanged(&connection, &before);
        drop(connection);
        let upgraded = Storage::open(&path, &KEY).unwrap();
        assert_eq!(records(&upgraded.connection), before);
        assert_eq!(
            upgraded.profile_binding(&id(1)).unwrap(),
            source.profile_binding(&id(1)).unwrap()
        );
    }
    let mut connection = legacy_v2(&source, &dir.path().join("foreign-key-fault"));
    let before = records(&connection);
    let orphan = format!("{sql} UPDATE collection_runs SET profile_id='synthetic-orphan';");
    let migrations = [crate::MIGRATIONS[0], crate::MIGRATIONS[1], orphan.as_str()];
    assert_eq!(
        crate::migrate(&mut connection, &migrations),
        Err(crate::StorageError::MigrationFailed)
    );
    assert_legacy_unchanged(&connection, &before);
    connection.commit_hook(Some(|| true)).unwrap();
    assert_eq!(
        crate::migrate_data_connection(&mut connection),
        Err(crate::StorageError::MigrationFailed)
    );
    connection.commit_hook(None::<fn() -> bool>).unwrap();
    assert_legacy_unchanged(&connection, &before);
    crate::migrate_data_connection(&mut connection).unwrap();
    assert_eq!(records(&connection), before);
    crate::backup_validation::validate_connection(&connection, crate::CURRENT_SCHEMA_VERSION)
        .unwrap();
}

#[test]
fn quota_v2_migration_begin_contention_restores_enforcement_and_allows_retry() {
    let (dir, source) = seed();
    let path = dir.path().join("busy-legacy");
    let mut connection = legacy_v2(&source, &path);
    let before = records(&connection);
    connection.busy_timeout(std::time::Duration::ZERO).unwrap();
    let blocker = Connection::open(&path).unwrap();
    blocker
        .pragma_update(None, "key", format!("x'{}'", "69".repeat(32)))
        .unwrap();
    blocker.execute_batch("BEGIN IMMEDIATE").unwrap();
    assert_eq!(
        crate::migrate_data_connection(&mut connection),
        Err(crate::StorageError::MigrationFailed)
    );
    assert_legacy_unchanged(&connection, &before);
    blocker.execute_batch("ROLLBACK").unwrap();
    crate::migrate_data_connection(&mut connection).unwrap();
    assert_eq!(records(&connection), before);
    crate::backup_validation::validate_connection(&connection, crate::CURRENT_SCHEMA_VERSION)
        .unwrap();
}

#[test]
fn quota_legacy_v2_archive_and_checkpoint_copy_keep_history_and_original_input() {
    let (dir, source) = seed();
    let legacy = legacy_v2(&source, &dir.path().join("old-live"));
    let before = records(&legacy);
    let copy = crate::backup_validation::trusted_memory_copy(&legacy, "main", 2).unwrap();
    let bytes = zeroize::Zeroizing::new(copy.serialize("main").unwrap().to_vec());
    let original = bytes.clone();
    crate::backup_validation::with_validated_snapshot::<_, crate::BackupValidationError>(
        &bytes,
        2,
        true,
        |normalized, _| {
            assert_eq!(records(normalized), before);
            assert_eq!(
                normalized
                    .query_row("SELECT count(*) FROM quota_snapshots", [], |r| r
                        .get::<_, i64>(0))
                    .unwrap(),
                0
            );
            assert_eq!(
                normalized
                    .query_row("SELECT count(*) FROM profile_bindings", [], |r| r
                        .get::<_, i64>(0))
                    .unwrap(),
                1
            );
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(*bytes, *original);
    let mut live = Storage {
        connection: legacy,
        revision_scope: Arc::new(()),
    };
    crate::migrate_data_connection(&mut live.connection).unwrap();
    let snapshot = live.backup_snapshot().unwrap();
    let expected = all(&live);
    let checkpoint = dir.path().join("legacy-2-checkpoint");
    live.create_encrypted_checkpoint(&checkpoint).unwrap();
    live.connection
        .execute(
            "ATTACH DATABASE ?1 AS old_fixture",
            [checkpoint.to_str().unwrap()],
        )
        .unwrap();
    // No quota rows exist. Restore the exact v2 collection-run catalog from trusted SQL.
    live.connection.execute_batch("PRAGMA foreign_keys=OFF; BEGIN IMMEDIATE; DROP TABLE old_fixture.threshold_alert_events; DROP TABLE old_fixture.threshold_alert_preferences; DROP TABLE old_fixture.threshold_alert_overrides; DROP TABLE old_fixture.reset_notification_outbox; DROP TABLE old_fixture.reset_notification_receipts; DROP TABLE old_fixture.reset_alert_overrides; DROP TABLE old_fixture.reset_alert_preferences; DROP TABLE old_fixture.alert_events; DROP TABLE old_fixture.quota_reset_assessments; DROP TABLE old_fixture.current_quota; DROP TABLE old_fixture.current_quota_readings; DROP TABLE old_fixture.credit_snapshots; DROP TABLE old_fixture.quota_limits; DROP TABLE old_fixture.quota_snapshots; DROP TABLE old_fixture.profile_bindings; DROP TABLE old_fixture.collection_runs;").unwrap();
    let v2 = crate::MIGRATIONS[1];
    let start = v2.find("CREATE TABLE collection_runs (").unwrap();
    let end = start + v2[start..].find(") STRICT;").unwrap() + ") STRICT;".len();
    let definition = v2[start..end].replace(
        "CREATE TABLE collection_runs",
        "CREATE TABLE old_fixture.collection_runs",
    );
    live.connection.execute_batch(&definition).unwrap();
    live.connection.execute_batch("INSERT INTO old_fixture.collection_runs SELECT * FROM main.collection_runs; CREATE INDEX old_fixture.activity_summary_profile_time ON collection_runs(profile_id,observed_at_utc,id); PRAGMA old_fixture.user_version=2; COMMIT; DETACH DATABASE old_fixture; PRAGMA foreign_keys=ON;").unwrap();
    let encrypted_original = std::fs::read(&checkpoint).unwrap();
    assert_eq!(
        live.verify_encrypted_checkpoint(&checkpoint)
            .unwrap()
            .schema_version,
        2
    );
    let recovered = live.read_encrypted_checkpoint(&checkpoint).unwrap();
    assert_eq!(recovered.schema_version, 2);
    let homes = crate::validate_checkpoint_restore_input(&recovered.bytes, 2)
        .unwrap()
        .homes;
    let revision = live.restore_state().unwrap().revision;
    live.replace_records_from_checkpoint(&recovered.bytes, 2, &revision, &homes)
        .unwrap();
    assert_eq!(std::fs::read(&checkpoint).unwrap(), encrypted_original);
    assert_eq!(records(&live.connection)[3..], before[3..]);
    assert_eq!(all(&live), expected);
    assert_eq!(snapshot.schema_version, crate::CURRENT_SCHEMA_VERSION);
}
