//! Check authenticated current-schema snapshots without files, migrations, or live storage.
//! The caller must authenticate the archive before supplying its database image.
//! Validation needs a second image-sized RAM buffer. The 5 GiB format limit is not a memory guarantee.
//! Future time-zone or locale writers must extend the current UTC/en-US settings guard.
//! Stored identity confidence is checked for consistency, not treated as fresh provider verification.

use crate::{CURRENT_SCHEMA_VERSION, MAX_BACKUP_SNAPSHOT_SIZE, MIGRATIONS};
use rusqlite::{Connection, Row, config::DbConfig, ffi, limits::Limit};
use std::{
    fmt,
    time::{Duration, Instant},
};
use zeroize::Zeroizing;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BackupContentSummary {
    pub active_profiles: u32,
    pub archived_profiles: u64,
    pub accounts: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BackupValidationError {
    UnsupportedSchema,
    UnsupportedContent,
    InvalidDatabase,
    Unavailable,
}

impl fmt::Display for BackupValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::UnsupportedSchema => "This backup uses an unsupported database schema.",
            Self::UnsupportedContent => "This version cannot check these database settings.",
            Self::InvalidDatabase => "The backup database did not pass its content checks.",
            Self::Unavailable => "The backup database content check is unavailable.",
        })
    }
}

impl std::error::Error for BackupValidationError {}

type Result<T> = std::result::Result<T, BackupValidationError>;

/// Validate only the current application schema and return counts without identifiers or home references.
/// No restore is approved or applied. A later restore must stop monitoring, regardless of saved state.
pub fn validate_backup_snapshot(
    bytes: &[u8],
    manifest_schema_version: u32,
) -> Result<BackupContentSummary> {
    if manifest_schema_version != CURRENT_SCHEMA_VERSION {
        return Err(BackupValidationError::UnsupportedSchema);
    }
    validate_size(bytes.len() as u64)?;
    if !bytes.starts_with(b"SQLite format 3\0") || bytes[18..20] != [1, 1] {
        return Err(BackupValidationError::InvalidDatabase);
    }
    let size = bytes
        .len()
        .checked_add(20)
        .ok_or(BackupValidationError::Unavailable)?;
    let mut image = Zeroizing::new(Vec::new());
    image
        .try_reserve_exact(size)
        .map_err(|_| BackupValidationError::Unavailable)?;
    image.extend_from_slice(bytes);
    // SQLite recommends extra readable padding when a serialized image has uncertain contents.
    image.resize(size, 0);
    // Declared after image: all paths close SQLite before the borrowed buffer is wiped.
    let connection = memory_connection()?;
    let database_size =
        i64::try_from(bytes.len()).map_err(|_| BackupValidationError::Unavailable)?;
    let buffer_size = i64::try_from(image.len()).map_err(|_| BackupValidationError::Unavailable)?;
    // SQLite borrows this stable, private allocation until connection close. READONLY prevents writes;
    // omitting FREEONCLOSE/RESIZEABLE preserves Rust ownership. No pointer or connection escapes.
    let status = unsafe {
        ffi::sqlite3_deserialize(
            connection.handle(),
            c"main".as_ptr(),
            image.as_mut_ptr(),
            database_size,
            buffer_size,
            ffi::SQLITE_DESERIALIZE_READONLY,
        )
    };
    if status != ffi::SQLITE_OK {
        return Err(if status == ffi::SQLITE_NOMEM {
            BackupValidationError::Unavailable
        } else {
            BackupValidationError::InvalidDatabase
        });
    }
    let checked = validate_connection(&connection, manifest_schema_version);
    // Every statement is local to validate_connection and has finished before this explicit close.
    connection
        .close()
        .map_err(|_| BackupValidationError::Unavailable)?;
    checked
}

fn validate_size(size: u64) -> Result<()> {
    if !(100..=MAX_BACKUP_SNAPSHOT_SIZE).contains(&size) {
        return Err(BackupValidationError::InvalidDatabase);
    }
    Ok(())
}

fn memory_connection() -> Result<Connection> {
    let connection =
        Connection::open_in_memory().map_err(|_| BackupValidationError::Unavailable)?;
    for (option, enabled) in [
        (DbConfig::SQLITE_DBCONFIG_DEFENSIVE, true),
        (DbConfig::SQLITE_DBCONFIG_TRUSTED_SCHEMA, false),
        (DbConfig::SQLITE_DBCONFIG_ENABLE_TRIGGER, false),
        (DbConfig::SQLITE_DBCONFIG_ENABLE_VIEW, false),
        (DbConfig::SQLITE_DBCONFIG_ENABLE_FKEY, true),
    ] {
        if connection
            .set_db_config(option, enabled)
            .map_err(|_| BackupValidationError::Unavailable)?
            != enabled
        {
            return Err(BackupValidationError::Unavailable);
        }
    }
    let mut extensions = 1;
    // Disable both extension-loading entry points on this fresh connection and check the result.
    let status = unsafe {
        ffi::sqlite3_db_config(
            connection.handle(),
            ffi::SQLITE_DBCONFIG_ENABLE_LOAD_EXTENSION,
            0,
            &mut extensions,
        )
    };
    if status != ffi::SQLITE_OK || extensions != 0 {
        return Err(BackupValidationError::Unavailable);
    }
    for (limit, value) in [
        (Limit::SQLITE_LIMIT_SQL_LENGTH, 64 * 1024),
        (Limit::SQLITE_LIMIT_COLUMN, 32),
        (Limit::SQLITE_LIMIT_EXPR_DEPTH, 32),
        (Limit::SQLITE_LIMIT_COMPOUND_SELECT, 8),
        (Limit::SQLITE_LIMIT_FUNCTION_ARG, 16),
        (Limit::SQLITE_LIMIT_ATTACHED, 0),
        (Limit::SQLITE_LIMIT_TRIGGER_DEPTH, 0),
        (Limit::SQLITE_LIMIT_WORKER_THREADS, 0),
    ] {
        connection
            .set_limit(limit, value)
            .map_err(|_| BackupValidationError::Unavailable)?;
        if connection
            .limit(limit)
            .map_err(|_| BackupValidationError::Unavailable)?
            != value
        {
            return Err(BackupValidationError::Unavailable);
        }
    }
    connection
        .execute_batch(
            "PRAGMA cipher_memory_security=ON; PRAGMA temp_store=MEMORY; PRAGMA query_only=ON;",
        )
        .map_err(|_| BackupValidationError::Unavailable)?;
    let memory_security: String = connection
        .pragma_query_value(None, "cipher_memory_security", |row| row.get(0))
        .map_err(|_| BackupValidationError::Unavailable)?;
    let temporary: i64 = connection
        .pragma_query_value(None, "temp_store", |row| row.get(0))
        .map_err(|_| BackupValidationError::Unavailable)?;
    let query_only: bool = connection
        .pragma_query_value(None, "query_only", |row| row.get(0))
        .map_err(|_| BackupValidationError::Unavailable)?;
    if memory_security != "1" || temporary != 2 || !query_only {
        return Err(BackupValidationError::Unavailable);
    }
    // ponytail: a 60-second elapsed limit bounds checks; qualify larger histories before changing it.
    let started = Instant::now();
    connection
        .progress_handler(
            10_000,
            Some(move || started.elapsed() > Duration::from_secs(60)),
        )
        .map_err(|_| BackupValidationError::Unavailable)?;
    Ok(connection)
}

fn validate_connection(
    connection: &Connection,
    manifest_version: u32,
) -> Result<BackupContentSummary> {
    let version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(sql_error)?;
    if version != i64::from(manifest_version) {
        return Err(BackupValidationError::UnsupportedSchema);
    }
    require_schema(connection)?;
    let integrity: String = connection
        .query_row("PRAGMA integrity_check(1)", [], |row| row.get(0))
        .map_err(sql_error)?;
    if integrity != "ok" {
        return Err(BackupValidationError::InvalidDatabase);
    }
    let mut foreign_keys = connection
        .prepare("PRAGMA foreign_key_check")
        .map_err(sql_error)?;
    if foreign_keys
        .query([])
        .map_err(sql_error)?
        .next()
        .map_err(sql_error)?
        .is_some()
    {
        return Err(BackupValidationError::InvalidDatabase);
    }
    let settings_supported = validate_settings(connection)?;
    let (accounts, accounts_supported) = validate_accounts(connection)?;
    let (active_profiles, archived_profiles) = validate_profiles(connection)?;
    if !settings_supported || !accounts_supported {
        return Err(BackupValidationError::UnsupportedContent);
    }
    Ok(BackupContentSummary {
        active_profiles,
        archived_profiles,
        accounts,
    })
}

fn require_schema(connection: &Connection) -> Result<()> {
    let reference = memory_connection()?;
    reference
        .pragma_update(None, "query_only", false)
        .map_err(|_| BackupValidationError::Unavailable)?;
    // Only trusted application SQL builds the reference. No migration runs against the input.
    for migration in MIGRATIONS {
        reference
            .execute_batch(migration)
            .map_err(|_| BackupValidationError::Unavailable)?;
    }
    const CATALOG: &str = "SELECT type,name,tbl_name,sql FROM sqlite_schema ORDER BY type,name";
    let mut expected = reference
        .prepare(CATALOG)
        .map_err(|_| BackupValidationError::Unavailable)?;
    let mut expected_rows = expected
        .query([])
        .map_err(|_| BackupValidationError::Unavailable)?;
    let mut actual = connection.prepare(CATALOG).map_err(sql_error)?;
    let mut actual_rows = actual.query([]).map_err(sql_error)?;
    while let Some(row) = expected_rows
        .next()
        .map_err(|_| BackupValidationError::Unavailable)?
    {
        let actual = actual_rows
            .next()
            .map_err(sql_error)?
            .ok_or(BackupValidationError::InvalidDatabase)?;
        for column in 0..4 {
            if row.get_ref(column).map_err(sql_error)?
                != actual.get_ref(column).map_err(sql_error)?
            {
                return Err(BackupValidationError::InvalidDatabase);
            }
        }
    }
    if actual_rows.next().map_err(sql_error)?.is_some() {
        return Err(BackupValidationError::InvalidDatabase);
    }
    Ok(())
}

fn text<'row>(row: &'row Row<'_>, column: usize) -> Result<&'row str> {
    row.get_ref(column)
        .map_err(sql_error)?
        .as_str()
        .map_err(|_| BackupValidationError::InvalidDatabase)
}

fn optional_text<'row>(row: &'row Row<'_>, column: usize) -> Result<Option<&'row str>> {
    let value = row.get_ref(column).map_err(sql_error)?;
    if value == rusqlite::types::ValueRef::Null {
        Ok(None)
    } else {
        value
            .as_str()
            .map(Some)
            .map_err(|_| BackupValidationError::InvalidDatabase)
    }
}

fn require(condition: bool) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(BackupValidationError::InvalidDatabase)
    }
}

fn canonical_uuid(value: &str) -> bool {
    if value.len() != 36 {
        return false;
    }
    uuid::Uuid::parse_str(value).is_ok_and(|id| id.hyphenated().encode_lower(&mut [0; 36]) == value)
}

fn safe_text(value: &str) -> bool {
    !value.chars().any(char::is_control)
}

fn validate_profiles(connection: &Connection) -> Result<(u32, u64)> {
    let mut statement = connection.prepare("SELECT id,account_id,name,environment,approved_home_reference,connection_state,archived,created_at,updated_at FROM profiles").map_err(sql_error)?;
    let mut rows = statement.query([]).map_err(sql_error)?;
    let mut active = 0u32;
    let mut archived = 0u64;
    while let Some(row) = rows.next().map_err(sql_error)? {
        require(canonical_uuid(text(row, 0)?))?;
        require(optional_text(row, 1)?.is_none_or(canonical_uuid))?;
        let label = text(row, 2)?;
        require(
            label.len() <= 1024
                && label == label.trim()
                && safe_text(label)
                && (1..=100).contains(&label.chars().count()),
        )?;
        require(matches!(text(row, 3)?, "native" | "wsl"))?;
        require(canonical_uuid(text(row, 4)?))?;
        require(matches!(
            text(row, 5)?,
            "connected"
                | "authentication_required"
                | "unauthorized"
                | "offline"
                | "unsupported"
                | "error"
        ))?;
        match row.get::<_, i64>(6).map_err(sql_error)? {
            0 if active < 10 => active += 1,
            1 => {
                archived = archived
                    .checked_add(1)
                    .ok_or(BackupValidationError::InvalidDatabase)?
            }
            _ => return Err(BackupValidationError::InvalidDatabase),
        }
        require(utc_timestamp(text(row, 7)?) && utc_timestamp(text(row, 8)?))?;
    }
    Ok((active, archived))
}

fn validate_accounts(connection: &Connection) -> Result<(u64, bool)> {
    let mut statement = connection.prepare("SELECT id,verified_identity_key,identity_confidence,returned_email,returned_plan,first_observed_at,last_observed_at FROM accounts").map_err(sql_error)?;
    let mut rows = statement.query([]).map_err(sql_error)?;
    let mut count = 0u64;
    let mut supported = true;
    while let Some(row) = rows.next().map_err(sql_error)? {
        require(canonical_uuid(text(row, 0)?))?;
        let key = optional_text(row, 1)?;
        let confidence = text(row, 2)?;
        require(matches!(confidence, "verified" | "provisional" | "unknown"))?;
        require(key.is_some() == (confidence == "verified"))?;
        // No stable-identity writer exists yet. Do not invent its grammar or claim verified provenance.
        if key.is_some_and(|value| value.len() > 1024) {
            supported = false;
        } else {
            require(key.is_none_or(|value| !value.trim().is_empty() && safe_text(value)))?;
        }
        // Match the current openusage-codex::parse_account limits in UTF-8 bytes.
        for (column, maximum) in [(3, 254), (4, 64)] {
            require(optional_text(row, column)?.is_none_or(|value| {
                value.len() <= maximum && !value.trim().is_empty() && safe_text(value)
            }))?;
        }
        require(utc_timestamp(text(row, 5)?) && utc_timestamp(text(row, 6)?))?;
        count = count
            .checked_add(1)
            .ok_or(BackupValidationError::InvalidDatabase)?;
    }
    Ok((count, supported))
}

fn validate_settings(connection: &Connection) -> Result<bool> {
    let mut statement = connection.prepare("SELECT id,timezone,locale,theme,monitoring,retention_policy,created_at,updated_at FROM settings").map_err(sql_error)?;
    let mut rows = statement.query([]).map_err(sql_error)?;
    let row = rows
        .next()
        .map_err(sql_error)?
        .ok_or(BackupValidationError::InvalidDatabase)?;
    require(row.get::<_, i64>(0).map_err(sql_error)? == 1)?;
    let timezone = text(row, 1)?;
    let locale = text(row, 2)?;
    let mut supported = true;
    for value in [timezone, locale] {
        if value.len() > 1024 {
            supported = false;
        } else {
            require(!value.is_empty() && safe_text(value))?;
        }
    }
    require(matches!(text(row, 3)?, "system" | "light" | "dark"))?;
    require(matches!(text(row, 4)?, "stopped" | "paused" | "active"))?;
    require(text(row, 5)? == "unlimited")?;
    require(utc_timestamp(text(row, 6)?) && utc_timestamp(text(row, 7)?))?;
    supported &= timezone == "UTC" && locale == "en-US";
    require(rows.next().map_err(sql_error)?.is_none())?;
    Ok(supported)
}

fn utc_timestamp(value: &str) -> bool {
    let bytes = value.as_bytes();
    if !(20..=30).contains(&bytes.len())
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || bytes[10] != b'T'
        || bytes[13] != b':'
        || bytes[16] != b':'
        || bytes.last() != Some(&b'Z')
    {
        return false;
    }
    if bytes.len() > 20
        && (bytes[19] != b'.'
            || bytes.len() < 22
            || !bytes[20..bytes.len() - 1].iter().all(u8::is_ascii_digit))
    {
        return false;
    }
    let number = |start: usize, end: usize| -> Option<u32> {
        bytes[start..end].iter().try_fold(0, |value, digit| {
            digit
                .is_ascii_digit()
                .then(|| value * 10 + u32::from(digit - b'0'))
        })
    };
    let (Some(year), Some(month), Some(day), Some(hour), Some(minute), Some(second)) = (
        number(0, 4),
        number(5, 7),
        number(8, 10),
        number(11, 13),
        number(14, 16),
        number(17, 19),
    ) else {
        return false;
    };
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
        2 => 28,
        _ => return false,
    };
    year > 0 && day > 0 && day <= days && hour < 24 && minute < 60 && second < 60
}

fn sql_error(error: rusqlite::Error) -> BackupValidationError {
    match error {
        rusqlite::Error::SqliteFailure(error, _)
            if matches!(
                error.code,
                rusqlite::ErrorCode::OutOfMemory | rusqlite::ErrorCode::OperationInterrupted
            ) =>
        {
            BackupValidationError::Unavailable
        }
        _ => BackupValidationError::InvalidDatabase,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Storage;
    use rusqlite::params;

    const KEY: [u8; 32] = [0x64; 32];
    const WHEN: &str = "2024-02-29T12:34:56.123Z";

    fn id(number: u32) -> String {
        format!("{number:08x}-0000-4000-8000-000000000000")
    }

    fn fixture() -> Connection {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .pragma_update(None, "foreign_keys", false)
            .unwrap();
        for migration in MIGRATIONS {
            connection.execute_batch(migration).unwrap();
        }
        connection
            .pragma_update(None, "user_version", CURRENT_SCHEMA_VERSION)
            .unwrap();
        connection
    }

    fn add_profile(connection: &Connection, number: u32) {
        connection.execute(
            "INSERT INTO profiles (id,name,environment,approved_home_reference,connection_state,created_at,updated_at) VALUES (?1,'Synthetic','native',?1,'authentication_required',?2,?2)",
            params![id(number), WHEN],
        ).unwrap();
    }

    fn image(connection: &Connection) -> Zeroizing<Vec<u8>> {
        Zeroizing::new(connection.serialize("main").unwrap().to_vec())
    }

    fn reject_mutation(sql: &str) {
        let connection = fixture();
        add_profile(&connection, 1);
        connection.execute_batch(sql).unwrap();
        let bytes = image(&connection);
        let original = bytes.clone();
        assert_eq!(
            validate_backup_snapshot(&bytes, CURRENT_SCHEMA_VERSION),
            Err(BackupValidationError::InvalidDatabase),
            "{sql}"
        );
        assert_eq!(*bytes, *original);
    }

    #[test]
    fn active_wal_snapshot_validates_counts_without_files_or_input_changes() {
        let temporary = crate::tests::private_directory();
        let path = temporary.path().join("live.db");
        let mut storage = Storage::create(&path, &KEY).unwrap();
        storage
            .connection
            .pragma_update(None, "wal_autocheckpoint", 0)
            .unwrap();
        storage.connection.execute(
            "INSERT INTO accounts VALUES (?1,'synthetic-stable-id','verified','synthetic@example.invalid','synthetic-plan',?2,?2)",
            params![id(99), WHEN],
        ).unwrap();
        storage
            .create_profile(&id(1), &"😀".repeat(100), &id(1))
            .unwrap();
        storage
            .create_profile(&id(2), "Archived synthetic", &id(2))
            .unwrap();
        storage.set_profile_archived(&id(2), true).unwrap();
        storage
            .connection
            .execute(
                "UPDATE profiles SET account_id=?1 WHERE id=?2",
                params![id(99), id(1)],
            )
            .unwrap();
        assert!(path.with_extension("db-wal").metadata().unwrap().len() > 0);
        let snapshot = storage.backup_snapshot().unwrap();
        let original = snapshot.bytes.clone();
        let mut before: Vec<_> = std::fs::read_dir(temporary.path())
            .unwrap()
            .map(|entry| {
                let path = entry.unwrap().path();
                (
                    path.file_name().unwrap().to_owned(),
                    std::fs::read(path).unwrap(),
                )
            })
            .collect();
        before.sort_by(|left, right| left.0.cmp(&right.0));
        assert_eq!(
            validate_backup_snapshot(&snapshot.bytes, snapshot.schema_version).unwrap(),
            BackupContentSummary {
                active_profiles: 1,
                archived_profiles: 1,
                accounts: 1
            }
        );
        assert_eq!(*snapshot.bytes, *original);
        let mut after: Vec<_> = std::fs::read_dir(temporary.path())
            .unwrap()
            .map(|entry| {
                let path = entry.unwrap().path();
                (
                    path.file_name().unwrap().to_owned(),
                    std::fs::read(path).unwrap(),
                )
            })
            .collect();
        after.sort_by(|left, right| left.0.cmp(&right.0));
        assert_eq!(after, before);
    }

    #[test]
    fn altered_schema_and_broken_record_invariants_are_rejected() {
        for sql in [
            "CREATE VIEW unexpected AS SELECT load_extension('untrusted')",
            "DROP TRIGGER active_profiles_limit; CREATE TRIGGER active_profiles_limit BEFORE INSERT ON profiles BEGIN SELECT 1; END",
            "UPDATE profiles SET account_id='00000099-0000-4000-8000-000000000000'",
            "UPDATE profiles SET approved_home_reference='../external-home'",
            "UPDATE profiles SET id='00000001-0000-4000-8000-00000000000A'",
            "UPDATE profiles SET name='  padded label  '",
            "UPDATE profiles SET name=CAST(x'ff' AS TEXT)",
            "UPDATE profiles SET name='unsafe' || char(10)",
            "UPDATE profiles SET created_at='2023-02-29T12:34:56Z'",
            "UPDATE profiles SET updated_at='2024-01-01T00:00:00+00:00'",
            "DELETE FROM settings",
            "PRAGMA ignore_check_constraints=ON; UPDATE settings SET theme='invented'",
            "UPDATE settings SET timezone='Asia/Kolkata'; UPDATE profiles SET approved_home_reference='../external-home'",
        ] {
            reject_mutation(sql);
        }
        let connection = fixture();
        connection.execute_batch("UPDATE settings SET timezone='UTC',locale='en-US'; INSERT INTO accounts VALUES ('00000099-0000-4000-8000-000000000000',replace(hex(zeroblob(1025)),'0','x'),'verified',NULL,NULL,'2024-01-01T00:00:00Z','2024-01-01T00:00:00Z')").unwrap();
        assert_eq!(
            validate_backup_snapshot(&image(&connection), CURRENT_SCHEMA_VERSION),
            Err(BackupValidationError::UnsupportedContent)
        );
        connection.execute_batch("UPDATE accounts SET verified_identity_key='synthetic-key',returned_email=replace(hex(zeroblob(255)),'0','x')").unwrap();
        assert_eq!(
            validate_backup_snapshot(&image(&connection), CURRENT_SCHEMA_VERSION),
            Err(BackupValidationError::InvalidDatabase)
        );
        connection.execute_batch("DELETE FROM accounts").unwrap();
        let connection = fixture();
        connection
            .set_db_config(DbConfig::SQLITE_DBCONFIG_ENABLE_TRIGGER, false)
            .unwrap();
        for number in 1..=11 {
            add_profile(&connection, number);
        }
        assert_eq!(
            validate_backup_snapshot(&image(&connection), CURRENT_SCHEMA_VERSION),
            Err(BackupValidationError::InvalidDatabase)
        );
    }

    #[test]
    fn unsupported_versions_and_settings_are_distinct_from_invalid_contents() {
        let connection = fixture();
        let bytes = image(&connection);
        assert_eq!(
            validate_backup_snapshot(&bytes[..bytes.len() - 1], CURRENT_SCHEMA_VERSION),
            Err(BackupValidationError::InvalidDatabase)
        );
        assert_eq!(
            validate_backup_snapshot(&bytes, CURRENT_SCHEMA_VERSION + 1),
            Err(BackupValidationError::UnsupportedSchema)
        );
        connection
            .pragma_update(None, "user_version", CURRENT_SCHEMA_VERSION + 1)
            .unwrap();
        assert_eq!(
            validate_backup_snapshot(&image(&connection), CURRENT_SCHEMA_VERSION),
            Err(BackupValidationError::UnsupportedSchema)
        );
        connection
            .pragma_update(None, "user_version", CURRENT_SCHEMA_VERSION)
            .unwrap();
        for sql in [
            "UPDATE settings SET timezone='Asia/Kolkata'",
            "UPDATE settings SET timezone='UTC',locale='fr-FR'",
            "UPDATE settings SET locale='en-US',timezone=replace(hex(zeroblob(513)),'0','x')",
        ] {
            connection.execute_batch(sql).unwrap();
            assert_eq!(
                validate_backup_snapshot(&image(&connection), CURRENT_SCHEMA_VERSION),
                Err(BackupValidationError::UnsupportedContent)
            );
        }
        connection
            .execute_batch("UPDATE settings SET timezone='UTC',locale='en-US',monitoring='active'")
            .unwrap();
        assert_eq!(
            validate_backup_snapshot(&image(&connection), CURRENT_SCHEMA_VERSION)
                .unwrap()
                .active_profiles,
            0
        );
        for bytes in [b"not a database".as_slice(), &[0u8; 100]] {
            assert_eq!(
                validate_backup_snapshot(bytes, CURRENT_SCHEMA_VERSION),
                Err(BackupValidationError::InvalidDatabase)
            );
        }
        assert_eq!(validate_size(MAX_BACKUP_SNAPSHOT_SIZE), Ok(()));
        assert_eq!(
            validate_size(MAX_BACKUP_SNAPSHOT_SIZE + 1),
            Err(BackupValidationError::InvalidDatabase)
        );
    }
}
