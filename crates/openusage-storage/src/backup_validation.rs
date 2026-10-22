//! Check authenticated supported snapshots without files or live storage.
//! Old schemas migrate only after validation and a trusted-column copy into private memory.
//! The caller must authenticate the archive before supplying its database image.
//! Validation copies the image into RAM; legacy normalization needs another private database.
//! The 5 GiB format limit is not a memory guarantee.
//! Settings accept recognized IANA time zones and the supported en-US locale.
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

/// Validate a supported application schema and return counts without identifiers or home references.
/// No restore is approved or applied. A later restore must stop monitoring, regardless of saved state.
pub fn validate_backup_snapshot(
    bytes: &[u8],
    manifest_schema_version: u32,
) -> Result<BackupContentSummary> {
    validate_snapshot(bytes, manifest_schema_version, false)
}

/// Apply the same content checks, then require native profiles for the current restore capability.
/// WSL records remain valid for inspection but need a separate approval flow before restore.
pub fn validate_native_restore_snapshot(
    bytes: &[u8],
    manifest_schema_version: u32,
) -> Result<BackupContentSummary> {
    validate_snapshot(bytes, manifest_schema_version, true)
}

fn validate_snapshot(
    bytes: &[u8],
    manifest_schema_version: u32,
    native_only: bool,
) -> Result<BackupContentSummary> {
    with_validated_snapshot(bytes, manifest_schema_version, native_only, |_, summary| {
        Ok(summary)
    })
}

pub(crate) fn with_validated_snapshot<T, E: From<BackupValidationError>>(
    bytes: &[u8],
    manifest_schema_version: u32,
    native_only: bool,
    operation: impl FnOnce(&Connection, BackupContentSummary) -> std::result::Result<T, E>,
) -> std::result::Result<T, E> {
    if !supported_schema_version(manifest_schema_version) {
        return Err(BackupValidationError::UnsupportedSchema.into());
    }
    validate_size(bytes.len() as u64)?;
    if !bytes.starts_with(b"SQLite format 3\0") || bytes[18..20] != [1, 1] {
        return Err(BackupValidationError::InvalidDatabase.into());
    }
    let encoded_page_size = u16::from_be_bytes([bytes[16], bytes[17]]);
    let page_size = if encoded_page_size == 1 {
        65536
    } else {
        u64::from(encoded_page_size)
    };
    let page_count = u64::from(u32::from_be_bytes(
        bytes[28..32]
            .try_into()
            .map_err(|_| BackupValidationError::InvalidDatabase)?,
    ));
    if !(512..=65536).contains(&page_size)
        || !page_size.is_power_of_two()
        || page_count == 0
        || page_size.checked_mul(page_count) != Some(bytes.len() as u64)
    {
        return Err(BackupValidationError::InvalidDatabase.into());
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
        return Err((if status == ffi::SQLITE_NOMEM {
            BackupValidationError::Unavailable
        } else {
            BackupValidationError::InvalidDatabase
        })
        .into());
    }
    let checked = validate_connection(&connection, manifest_schema_version).and_then(|summary| {
        if native_only {
            let has_other_environment: bool = connection
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM profiles WHERE environment != 'native')",
                    [],
                    |row| row.get(0),
                )
                .map_err(sql_error)?;
            if has_other_environment {
                return Err(BackupValidationError::UnsupportedContent);
            }
        }
        Ok(summary)
    });
    let checked = checked.map_err(E::from).and_then(|summary| {
        if manifest_schema_version == CURRENT_SCHEMA_VERSION {
            operation(&connection, summary)
        } else {
            // Never migrate the deserialized input or execute its catalog SQL.
            let mut normalized = trusted_memory_copy(&connection, "main", manifest_schema_version)?;
            crate::migrate_data_connection(&mut normalized)
                .map_err(|_| E::from(BackupValidationError::Unavailable))?;
            let normalized_summary = validate_connection(&normalized, CURRENT_SCHEMA_VERSION)?;
            if normalized_summary != summary {
                return Err(BackupValidationError::InvalidDatabase.into());
            }
            normalized
                .pragma_update(None, "query_only", true)
                .map_err(|_| E::from(BackupValidationError::Unavailable))?;
            let result = operation(&normalized, summary);
            normalized
                .close()
                .map_err(|_| BackupValidationError::Unavailable)?;
            result
        }
    });
    // Close the borrowed source and wipe its image before a returned target transaction commits.
    connection
        .close()
        .map_err(|_| BackupValidationError::Unavailable)?;
    checked
}

pub(crate) fn supported_schema_version(version: u32) -> bool {
    // A later migration must add explicit content/copy support before becoming restorable.
    matches!(version, 1..=9) && version <= CURRENT_SCHEMA_VERSION
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

pub(crate) fn validate_connection(
    connection: &Connection,
    manifest_version: u32,
) -> Result<BackupContentSummary> {
    validate_schema_connection(connection, manifest_version, "main")
}

pub(crate) fn validate_schema_connection(
    connection: &Connection,
    manifest_version: u32,
    schema: &str,
) -> Result<BackupContentSummary> {
    if !supported_schema_version(manifest_version) {
        return Err(BackupValidationError::UnsupportedSchema);
    }
    if !matches!(schema, "main" | "openusage_checkpoint") {
        return Err(BackupValidationError::Unavailable);
    }
    let version: i64 = connection
        .pragma_query_value(Some(schema), "user_version", |row| row.get(0))
        .map_err(sql_error)?;
    if version != i64::from(manifest_version) {
        return Err(BackupValidationError::UnsupportedSchema);
    }
    require_schema(connection, schema, manifest_version)?;
    let integrity: String = connection
        .query_row(&format!("PRAGMA {schema}.integrity_check(1)"), [], |row| {
            row.get(0)
        })
        .map_err(sql_error)?;
    if integrity != "ok" {
        return Err(BackupValidationError::InvalidDatabase);
    }
    let mut foreign_keys = connection
        .prepare(&format!("PRAGMA {schema}.foreign_key_check"))
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
    let settings_supported = validate_settings(connection, schema)?;
    let (accounts, accounts_supported) = validate_accounts(connection, schema)?;
    let (active_profiles, archived_profiles) = validate_profiles(connection, schema)?;
    if manifest_version >= 2 {
        validate_activity(connection, schema, manifest_version)?;
        if manifest_version >= 3 {
            crate::quota_validation::validate_quota(connection, schema)?;
            if manifest_version >= 4 {
                crate::quota_resets::validate(connection, schema)
                    .map_err(|_| BackupValidationError::InvalidDatabase)?;
            }
            if manifest_version >= 5 {
                crate::alerts::validate(connection, schema, manifest_version)?;
            }
            if manifest_version >= 6 {
                crate::reset_notifications::validate(connection, schema, manifest_version)?;
                if manifest_version >= 7 {
                    crate::threshold_alerts::validate(connection, schema)?;
                }
                if manifest_version >= 8 {
                    crate::operational_alerts::validate(connection, schema)?;
                }
            }
        }
    }
    if manifest_version >= 9 {
        crate::subscriptions::validate(connection, schema)?;
    }
    if !settings_supported || !accounts_supported {
        return Err(BackupValidationError::UnsupportedContent);
    }
    Ok(BackupContentSummary {
        active_profiles,
        archived_profiles,
        accounts,
    })
}

fn require_schema(connection: &Connection, schema: &str, version: u32) -> Result<()> {
    let reference = memory_connection()?;
    reference
        .pragma_update(None, "query_only", false)
        .map_err(|_| BackupValidationError::Unavailable)?;
    // Only trusted application SQL builds the reference. No migration runs against the input.
    for migration in &MIGRATIONS[..version as usize] {
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
    let mut actual = connection
        .prepare(&CATALOG.replace("sqlite_schema", &format!("{schema}.sqlite_schema")))
        .map_err(sql_error)?;
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

// These table/column lists are application-owned, never read from an input catalog.
const LEGACY_TABLES: &[(&str, &str)] = &[
    (
        "accounts",
        "id,verified_identity_key,identity_confidence,returned_email,returned_plan,first_observed_at,last_observed_at",
    ),
    (
        "profiles",
        "id,account_id,name,environment,approved_home_reference,connection_state,archived,created_at,updated_at",
    ),
    (
        "settings",
        "id,timezone,locale,theme,monitoring,retention_policy,created_at,updated_at",
    ),
];
const ACTIVITY_TABLES: &[(&str, &str)] = &[
    (
        "collection_runs",
        "id,profile_id,binding_id,started_at_utc,completed_at_utc,observed_at_utc,source_method,provider_version,source_kind,source_scope,raw_scope,normalized_unit,coverage,identity_confidence,verified_identity_key,outcome",
    ),
    (
        "activity_summaries",
        "id,profile_id,binding_id,collection_run_id,lifetime_tokens,peak_daily_tokens,longest_running_turn_sec,current_streak_days,longest_streak_days,daily_state",
    ),
    (
        "current_activity",
        "profile_id,binding_id,summary_id,collection_run_id,created_at_utc,updated_at_utc",
    ),
    (
        "current_activity_days",
        "profile_id,provider_date,source_scope,binding_id,collection_run_id,tokens,observed_at_utc,created_at_utc,updated_at_utc",
    ),
    (
        "activity_day_history",
        "id,profile_id,provider_date,source_scope,binding_id,collection_run_id,prior_tokens,accepted_tokens,observed_at_utc",
    ),
];

const QUOTA_TABLES: &[(&str, &str)] = &[
    (
        "profile_bindings",
        "profile_id,binding_id,created_at_utc,updated_at_utc",
    ),
    (
        "quota_snapshots",
        "id,profile_id,binding_id,available_reset_credits,credit_details_state",
    ),
    (
        "quota_limits",
        "collection_run_id,profile_id,binding_id,limit_id,limit_name,reached_type,monthly_credit_limit,spend_control_reached,primary_used_percent,primary_duration_mins,primary_resets_at_utc,secondary_used_percent,secondary_duration_mins,secondary_resets_at_utc",
    ),
    (
        "credit_snapshots",
        "collection_run_id,profile_id,binding_id,ordinal,title,status,granted_at_unix,expires_at_unix",
    ),
    (
        "current_quota_readings",
        "profile_id,binding_id,collection_run_id,created_at_utc,updated_at_utc",
    ),
    (
        "current_quota",
        "profile_id,limit_id,window_role,binding_id,collection_run_id,used_percent,window_duration_mins,resets_at_utc,created_at_utc,updated_at_utc",
    ),
];

/// Copy only validated known fields into a fresh private RAM database of the original version.
/// The checkpoint/manifest version remains unchanged; callers may then migrate this private copy.
pub(crate) fn trusted_memory_copy(
    source: &Connection,
    schema: &str,
    version: u32,
) -> Result<Connection> {
    if !matches!(schema, "main" | "openusage_checkpoint") || !supported_schema_version(version) {
        return Err(BackupValidationError::UnsupportedSchema);
    }
    let mut target = memory_connection()?;
    target
        .execute_batch(
            "PRAGMA query_only=OFF; PRAGMA page_size=4096; PRAGMA max_page_count=1310720;",
        )
        .map_err(sql_error)?;
    let page_size: i64 = target
        .pragma_query_value(None, "page_size", |r| r.get(0))
        .map_err(sql_error)?;
    let page_limit: i64 = target
        .pragma_query_value(None, "max_page_count", |r| r.get(0))
        .map_err(sql_error)?;
    require(page_size.checked_mul(page_limit) == Some(MAX_BACKUP_SNAPSHOT_SIZE as i64))?;
    for migration in &MIGRATIONS[..version as usize] {
        target.execute_batch(migration).map_err(sql_error)?;
    }
    target
        .pragma_update(None, "user_version", version)
        .map_err(sql_error)?;
    {
        let tx = target.transaction().map_err(sql_error)?;
        tx.execute("DELETE FROM settings", []).map_err(sql_error)?;
        for (table, columns) in LEGACY_TABLES {
            copy_fields(source, &tx, schema, table, columns, columns)?;
        }
        if version >= 2 {
            copy_activity_records(source, &tx, schema, false)?;
        }
        tx.commit().map_err(sql_error)?;
    }
    validate_connection(&target, version)?;
    Ok(target)
}

pub(crate) fn copy_activity_records(
    source: &Connection,
    target: &Connection,
    schema: &str,
    restore: bool,
) -> Result<()> {
    if !matches!(schema, "main" | "openusage_checkpoint") {
        return Err(BackupValidationError::Unavailable);
    }
    // Parent links may sort after their children. Insert all event IDs before restoring links.
    copy_fields(
        source,
        target,
        schema,
        "profile_identity_events",
        "id,profile_id,home_reference,previous_binding_id,returned_email,returned_plan,identity_confidence,verified_identity_key,observed_at_utc",
        "id,profile_id,home_reference,NULL,returned_email,returned_plan,identity_confidence,verified_identity_key,observed_at_utc",
    )?;
    let mut parents = source.prepare(&format!("SELECT id,previous_binding_id FROM {schema}.profile_identity_events WHERE previous_binding_id IS NOT NULL")).map_err(sql_error)?;
    let mut rows = parents.query([]).map_err(sql_error)?;
    let mut update = target
        .prepare("UPDATE profile_identity_events SET previous_binding_id=?2 WHERE id=?1")
        .map_err(sql_error)?;
    while let Some(row) = rows.next().map_err(sql_error)? {
        update
            .execute(rusqlite::params![text(row, 0)?, text(row, 1)?])
            .map_err(sql_error)?;
    }
    for (table, columns) in ACTIVITY_TABLES {
        copy_fields(source, target, schema, table, columns, columns)?;
    }
    let version: u32 = source
        .pragma_query_value(Some(schema), "user_version", |r| r.get(0))
        .map_err(sql_error)?;
    if version >= 3 {
        for (table, columns) in QUOTA_TABLES {
            copy_fields(source, target, schema, table, columns, columns)?;
        }
    }
    if version >= 4 {
        copy_fields(
            source,
            target,
            schema,
            "quota_reset_assessments",
            "collection_run_id,profile_id,binding_id,limit_id,window_role,state,pending_reset_at_utc,generation_run_id",
            "collection_run_id,profile_id,binding_id,limit_id,window_role,state,pending_reset_at_utc,generation_run_id",
        )?;
        let mut statement = source.prepare(&format!("SELECT id,previous_quota_run_id FROM {schema}.quota_snapshots WHERE previous_quota_run_id IS NOT NULL")).map_err(sql_error)?;
        let mut rows = statement.query([]).map_err(sql_error)?;
        while let Some(row) = rows.next().map_err(sql_error)? {
            target
                .execute(
                    "UPDATE quota_snapshots SET previous_quota_run_id=?2 WHERE id=?1",
                    rusqlite::params![text(row, 0)?, text(row, 1)?],
                )
                .map_err(sql_error)?;
        }
    }
    let target_version: u32 = target
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .map_err(sql_error)?;
    if target_version >= 8 {
        if version >= 8 {
            copy_fields(source, target, schema, "notification_event_ids", "id", "id")?;
        } else if version >= 5 {
            let mut ids = source
                .prepare(&format!("SELECT id FROM {schema}.alert_events"))
                .map_err(sql_error)?;
            let mut rows = ids.query([]).map_err(sql_error)?;
            while let Some(row) = rows.next().map_err(sql_error)? {
                target
                    .execute(
                        "INSERT INTO notification_event_ids VALUES(?1)",
                        [text(row, 0)?],
                    )
                    .map_err(sql_error)?;
            }
        }
    }
    if version >= 5 {
        copy_fields(
            source,
            target,
            schema,
            "alert_events",
            crate::alerts::ALERT_COLUMNS,
            crate::alerts::ALERT_COLUMNS,
        )?;
    }
    if version >= 7 {
        for (table, columns) in [
            (
                "threshold_alert_events",
                crate::threshold_alerts::DETAIL_COLUMNS,
            ),
            (
                "threshold_alert_preferences",
                crate::threshold_alerts::PREFERENCE_COLUMNS,
            ),
            (
                "threshold_alert_overrides",
                crate::threshold_alerts::OVERRIDE_COLUMNS,
            ),
        ] {
            copy_fields(source, target, schema, table, columns, columns)?;
        }
    }
    if version >= 8 {
        crate::operational_alerts::copy_records(source, target, schema, restore)?;
    }
    if version >= 9 {
        copy_fields(
            source,
            target,
            schema,
            "subscriptions",
            crate::subscriptions::COLUMNS,
            crate::subscriptions::COLUMNS,
        )?;
    }
    if version >= 6 {
        crate::reset_notifications::copy_records(source, target, schema, restore, version)?;
    }
    Ok(())
}

pub(crate) fn copy_fields(
    source: &Connection,
    target: &Connection,
    schema: &str,
    table: &str,
    columns: &str,
    selected: &str,
) -> Result<()> {
    let mut select = source
        .prepare(&format!("SELECT {selected} FROM {schema}.{table}"))
        .map_err(sql_error)?;
    let count = select.column_count();
    let slots = (1..=count)
        .map(|n| format!("?{n}"))
        .collect::<Vec<_>>()
        .join(",");
    let mut insert = target
        .prepare(&format!("INSERT INTO {table}({columns}) VALUES ({slots})"))
        .map_err(sql_error)?;
    let mut rows = select.query([]).map_err(sql_error)?;
    while let Some(row) = rows.next().map_err(sql_error)? {
        let values = (0..count)
            .map(|i| row.get_ref(i).map(rusqlite::types::ToSqlOutput::Borrowed))
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(sql_error)?;
        insert
            .execute(rusqlite::params_from_iter(values))
            .map_err(sql_error)?;
    }
    Ok(())
}

fn validate_activity(connection: &Connection, schema: &str, version: u32) -> Result<()> {
    use crate::activity::{activity_date, activity_decimal, activity_utc_timestamp};
    let valid_text =
        |s: &str, maximum: usize| !s.trim().is_empty() && s.len() <= maximum && safe_text(s);
    let mut statement = connection.prepare(&format!("SELECT id,profile_id,home_reference,previous_binding_id,returned_email,returned_plan,identity_confidence,verified_identity_key,observed_at_utc FROM {schema}.profile_identity_events")).map_err(sql_error)?;
    let mut rows = statement.query([]).map_err(sql_error)?;
    while let Some(row) = rows.next().map_err(sql_error)? {
        for col in [0, 1, 2] {
            require(canonical_uuid(text(row, col)?))?;
        }
        require(
            optional_text(row, 3)?
                .is_none_or(|id| canonical_uuid(id) && Some(id) != text(row, 0).ok()),
        )?;
        require(
            valid_text(text(row, 4)?, 254)
                && optional_text(row, 5)?.is_none_or(|v| valid_text(v, 64)),
        )?;
        require(
            text(row, 6)? == "provisional"
                && optional_text(row, 7)?.is_none()
                && activity_utc_timestamp(text(row, 8)?),
        )?;
    }
    let mut statement = connection.prepare(&format!("SELECT id,profile_id,binding_id,started_at_utc,completed_at_utc,observed_at_utc,source_method,provider_version,source_kind,source_scope,raw_scope,normalized_unit,coverage,identity_confidence,verified_identity_key,outcome FROM {schema}.collection_runs")).map_err(sql_error)?;
    let mut rows = statement.query([]).map_err(sql_error)?;
    while let Some(row) = rows.next().map_err(sql_error)? {
        for col in [0, 1, 2] {
            require(canonical_uuid(text(row, col)?))?;
        }
        for col in [3, 4, 5] {
            require(activity_utc_timestamp(text(row, col)?))?;
        }
        require(text(row, 3)? <= text(row, 5)? && text(row, 5)? <= text(row, 4)?)?;
        let activity = text(row, 6)? == "account/usage/read"
            && text(row, 9)? == crate::activity::ACTIVITY_SOURCE_SCOPE
            && text(row, 11)? == "tokens";
        let quota = version >= 3
            && text(row, 6)? == crate::QUOTA_SOURCE_METHOD
            && text(row, 9)? == crate::QUOTA_SOURCE_SCOPE
            && text(row, 11)? == "percent";
        require(
            (activity || quota)
                && valid_text(text(row, 7)?, 64)
                && matches!(text(row, 8)?, "official_artifact" | "test_fixture"),
        )?;
        require(
            optional_text(row, 10)?.is_none()
                && text(row, 12)? == "unknown"
                && text(row, 13)? == "provisional"
                && optional_text(row, 14)?.is_none()
                && text(row, 15)? == "accepted",
        )?;
    }
    let mut statement = connection.prepare(&format!("SELECT id,profile_id,binding_id,collection_run_id,lifetime_tokens,peak_daily_tokens,longest_running_turn_sec,current_streak_days,longest_streak_days,daily_state FROM {schema}.activity_summaries")).map_err(sql_error)?;
    let mut rows = statement.query([]).map_err(sql_error)?;
    while let Some(row) = rows.next().map_err(sql_error)? {
        for col in 0..4 {
            require(canonical_uuid(text(row, col)?))?;
        }
        require(text(row, 0)? == text(row, 3)?)?;
        for col in 4..9 {
            require(optional_text(row, col)?.is_none_or(activity_decimal))?;
        }
        require(matches!(
            text(row, 9)?,
            "unavailable" | "empty" | "returned"
        ))?;
    }
    let mut statement = connection.prepare(&format!("SELECT profile_id,binding_id,summary_id,collection_run_id,created_at_utc,updated_at_utc FROM {schema}.current_activity")).map_err(sql_error)?;
    let mut rows = statement.query([]).map_err(sql_error)?;
    while let Some(row) = rows.next().map_err(sql_error)? {
        for col in [0, 1] {
            require(canonical_uuid(text(row, col)?))?;
        }
        require(
            optional_text(row, 2)?.is_none_or(canonical_uuid)
                && optional_text(row, 2)? == optional_text(row, 3)?,
        )?;
        require(activity_utc_timestamp(text(row, 4)?) && activity_utc_timestamp(text(row, 5)?))?;
    }
    let mut statement = connection.prepare(&format!("SELECT profile_id,provider_date,source_scope,binding_id,collection_run_id,tokens,observed_at_utc,created_at_utc,updated_at_utc FROM {schema}.current_activity_days")).map_err(sql_error)?;
    let mut rows = statement.query([]).map_err(sql_error)?;
    while let Some(row) = rows.next().map_err(sql_error)? {
        for col in [0, 3, 4] {
            require(canonical_uuid(text(row, col)?))?;
        }
        require(
            activity_date(text(row, 1)?)
                && text(row, 2)? == crate::activity::ACTIVITY_SOURCE_SCOPE
                && activity_decimal(text(row, 5)?),
        )?;
        for col in 6..9 {
            require(activity_utc_timestamp(text(row, col)?))?;
        }
    }
    let mut statement = connection.prepare(&format!("SELECT id,profile_id,provider_date,source_scope,binding_id,collection_run_id,prior_tokens,accepted_tokens,observed_at_utc FROM {schema}.activity_day_history")).map_err(sql_error)?;
    let mut rows = statement.query([]).map_err(sql_error)?;
    while let Some(row) = rows.next().map_err(sql_error)? {
        for col in [0, 1, 4, 5] {
            require(canonical_uuid(text(row, col)?))?;
        }
        require(
            activity_date(text(row, 2)?) && text(row, 3)? == crate::activity::ACTIVITY_SOURCE_SCOPE,
        )?;
        require(
            optional_text(row, 6)?.is_none_or(activity_decimal)
                && activity_decimal(text(row, 7)?)
                && activity_utc_timestamp(text(row, 8)?),
        )?;
    }
    // Each binding has at most one parent. Every acyclic chain must reach a NULL root.
    let cyclic: bool = connection.query_row(&format!("WITH RECURSIVE rooted(id) AS (
        SELECT id FROM {schema}.profile_identity_events WHERE previous_binding_id IS NULL
        UNION SELECT e.id FROM {schema}.profile_identity_events e JOIN rooted p ON e.previous_binding_id=p.id
    ) SELECT EXISTS(SELECT 1 FROM {schema}.profile_identity_events e WHERE NOT EXISTS(SELECT 1 FROM rooted r WHERE r.id=e.id))"), [], |r|r.get(0)).map_err(sql_error)?;
    require(!cyclic)?;
    // Foreign keys bind profile/run/binding IDs; these checks bind their accepted value/time semantics.
    let invalid: bool = connection.query_row(&format!("SELECT
        EXISTS(SELECT 1 FROM {schema}.activity_day_history GROUP BY collection_run_id HAVING count(*)>4096)
        OR EXISTS(SELECT 1 FROM {schema}.collection_runs r LEFT JOIN {schema}.activity_summaries s ON s.collection_run_id=r.id WHERE r.source_method='account/usage/read' AND s.id IS NULL)
        OR EXISTS(SELECT 1 FROM {schema}.activity_summaries s JOIN {schema}.collection_runs r ON r.id=s.collection_run_id WHERE r.source_method!='account/usage/read')
        OR EXISTS(SELECT 1 FROM {schema}.activity_summaries s WHERE (s.daily_state='returned') != EXISTS(SELECT 1 FROM {schema}.activity_day_history h WHERE h.collection_run_id=s.collection_run_id))
        OR EXISTS(SELECT 1 FROM {schema}.activity_day_history h JOIN {schema}.collection_runs r ON r.id=h.collection_run_id WHERE h.observed_at_utc!=r.observed_at_utc)
        OR EXISTS(SELECT 1 FROM {schema}.current_activity_days d LEFT JOIN {schema}.current_activity c ON c.profile_id=d.profile_id LEFT JOIN {schema}.activity_day_history h ON h.collection_run_id=d.collection_run_id AND h.provider_date=d.provider_date AND h.source_scope=d.source_scope WHERE c.binding_id IS NULL OR c.binding_id!=d.binding_id OR h.id IS NULL OR d.tokens!=h.accepted_tokens OR d.observed_at_utc!=h.observed_at_utc)"), [], |r|r.get(0)).map_err(sql_error)?;
    require(!invalid)
}

pub(crate) fn text<'row>(row: &'row Row<'_>, column: usize) -> Result<&'row str> {
    row.get_ref(column)
        .map_err(sql_error)?
        .as_str()
        .map_err(|_| BackupValidationError::InvalidDatabase)
}

pub(crate) fn optional_text<'row>(row: &'row Row<'_>, column: usize) -> Result<Option<&'row str>> {
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

pub(crate) fn require(condition: bool) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(BackupValidationError::InvalidDatabase)
    }
}

pub(crate) fn canonical_uuid(value: &str) -> bool {
    if value.len() != 36 {
        return false;
    }
    uuid::Uuid::parse_str(value).is_ok_and(|id| id.hyphenated().encode_lower(&mut [0; 36]) == value)
}

fn safe_text(value: &str) -> bool {
    !value.chars().any(char::is_control)
}

fn validate_profiles(connection: &Connection, schema: &str) -> Result<(u32, u64)> {
    let mut statement = connection.prepare(&format!("SELECT id,account_id,name,environment,approved_home_reference,connection_state,archived,created_at,updated_at FROM {schema}.profiles")).map_err(sql_error)?;
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

fn validate_accounts(connection: &Connection, schema: &str) -> Result<(u64, bool)> {
    let mut statement = connection.prepare(&format!("SELECT id,verified_identity_key,identity_confidence,returned_email,returned_plan,first_observed_at,last_observed_at FROM {schema}.accounts")).map_err(sql_error)?;
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

fn validate_settings(connection: &Connection, schema: &str) -> Result<bool> {
    let mut statement = connection.prepare(&format!("SELECT id,timezone,locale,theme,monitoring,retention_policy,created_at,updated_at FROM {schema}.settings")).map_err(sql_error)?;
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
    supported &= crate::valid_time_zone(timezone) && locale == "en-US";
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

pub(crate) fn sql_error(error: rusqlite::Error) -> BackupValidationError {
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
pub(crate) fn activity_fixture(storage: &mut crate::Storage, profile: &str, home: &str, seed: u32) {
    use crate::{
        ActivityDayValue, ActivityReading, ActivitySourceKind, ActivitySummaryValues,
        ProfileBindingInput, ProfileIdentityEvidence,
    };
    let id = |n| format!("{n:08x}-0000-4000-8000-000000000000");
    let evidence = ProfileIdentityEvidence {
        email: "synthetic-history@example.invalid".into(),
        plan: Some("synthetic".into()),
    };
    let mut previous = None;
    for generation in 0..2 {
        let binding = storage
            .associate_profile_binding(ProfileBindingInput {
                profile_id: profile.into(),
                expected_home_reference: home.into(),
                expected_previous_binding_id: previous,
                new_binding_id: id(seed + generation),
                evidence: evidence.clone(),
                observed_at_utc: "2024-02-29T10:00:00.000Z".into(),
            })
            .unwrap();
        for index in 0..2 {
            let tokens = match (generation, index) {
                (0, 0) => "9007199254740993",
                (0, 1) => "9223372036854775807",
                _ => "0",
            };
            storage
                .accept_activity(ActivityReading {
                    run_id: id(seed + 10 + generation * 2 + index),
                    binding: binding.clone(),
                    identity_evidence: evidence.clone(),
                    provider_version: "0.163.0-synthetic".into(),
                    source_kind: ActivitySourceKind::TestFixture,
                    started_at_utc: "2024-02-29T11:00:00.000Z".into(),
                    observed_at_utc: format!("2024-02-29T12:00:0{index}.000Z"),
                    completed_at_utc: "2024-02-29T13:00:00.000Z".into(),
                    summary: ActivitySummaryValues {
                        lifetime_tokens: Some(tokens.into()),
                        ..Default::default()
                    },
                    daily_usage: if generation == 1 && index == 0 {
                        None
                    } else {
                        Some(vec![ActivityDayValue {
                            provider_date: "2024-02-29".into(),
                            tokens: tokens.into(),
                        }])
                    },
                })
                .unwrap();
        }
        previous = Some(binding.binding_id);
    }
}

#[cfg(test)]
pub(crate) fn activity_records(connection: &Connection) -> Vec<Vec<Vec<rusqlite::types::Value>>> {
    [
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
        "quota_reset_assessments",
        "alert_events",
        "reset_alert_preferences",
        "reset_alert_overrides",
        "reset_notification_outbox",
        "reset_notification_receipts",
        "threshold_alert_events",
        "threshold_alert_preferences",
        "threshold_alert_overrides",
        "notification_event_ids",
        "operational_alert_outcomes",
        "operational_alert_state",
        "operational_alert_events",
        "operational_alert_preferences",
        "operational_alert_overrides",
    ]
    .into_iter()
    .map(|table| {
        let order = if table == "notification_event_ids" {
            "1"
        } else {
            "1,2"
        };
        let mut statement = connection
            .prepare(&format!("SELECT * FROM {table} ORDER BY {order}"))
            .unwrap();
        let count = statement.column_count();
        statement
            .query_map([], |row| (0..count).map(|i| row.get(i)).collect())
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap()
    })
    .collect()
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
        assert_eq!(
            validate_native_restore_snapshot(&snapshot.bytes, snapshot.schema_version),
            validate_backup_snapshot(&snapshot.bytes, snapshot.schema_version)
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
    fn wsl_profiles_are_inspectable_but_require_a_future_restore_approval_flow() {
        let connection = fixture();
        add_profile(&connection, 1);
        connection
            .execute_batch("UPDATE profiles SET environment='wsl',archived=1")
            .unwrap();
        let bytes = image(&connection);
        let original = bytes.clone();
        assert_eq!(
            validate_backup_snapshot(&bytes, CURRENT_SCHEMA_VERSION).unwrap(),
            BackupContentSummary {
                active_profiles: 0,
                archived_profiles: 1,
                accounts: 0,
            }
        );
        assert_eq!(
            validate_native_restore_snapshot(&bytes, CURRENT_SCHEMA_VERSION),
            Err(BackupValidationError::UnsupportedContent)
        );
        assert_eq!(*bytes, *original);
        connection
            .execute_batch("UPDATE profiles SET approved_home_reference='../unsafe'")
            .unwrap();
        assert_eq!(
            validate_native_restore_snapshot(&image(&connection), CURRENT_SCHEMA_VERSION),
            Err(BackupValidationError::InvalidDatabase)
        );
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
            "UPDATE settings SET timezone='Invalid/Zone'",
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
    #[test]
    fn legacy_snapshot_is_validated_before_private_migration_and_keeps_original_bytes() {
        let legacy = Connection::open_in_memory().unwrap();
        legacy.execute_batch(MIGRATIONS[0]).unwrap();
        legacy.pragma_update(None, "user_version", 1).unwrap();
        add_profile(&legacy, 1);
        let bytes = image(&legacy);
        let original = bytes.clone();
        with_validated_snapshot::<_, BackupValidationError>(&bytes, 1, true, |copy, summary| {
            assert_eq!(summary.active_profiles, 1);
            assert_eq!(
                copy.pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
                    .unwrap(),
                CURRENT_SCHEMA_VERSION
            );
            assert!(activity_records(copy).iter().all(Vec::is_empty));
            assert_eq!(
                copy.query_row("SELECT approved_home_reference FROM profiles", [], |r| {
                    r.get::<_, String>(0)
                })
                .unwrap(),
                id(1)
            );
            Ok(())
        })
        .unwrap();
        assert_eq!(*bytes, *original);
        assert_eq!(
            legacy
                .pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
                .unwrap(),
            1
        );
        for sql in [
            "CREATE TABLE untrusted(id TEXT)",
            "DROP TRIGGER active_profiles_limit; CREATE TRIGGER active_profiles_limit BEFORE INSERT ON profiles BEGIN SELECT 1; END",
        ] {
            let bad = Connection::open_in_memory().unwrap();
            bad.execute_batch(MIGRATIONS[0]).unwrap();
            bad.pragma_update(None, "user_version", 1).unwrap();
            bad.execute_batch(sql).unwrap();
            let bytes = image(&bad);
            let original = bytes.clone();
            assert_eq!(
                validate_backup_snapshot(&bytes, 1),
                Err(BackupValidationError::InvalidDatabase)
            );
            assert_eq!(*bytes, *original);
        }
        assert_eq!(
            validate_backup_snapshot(&bytes, 0),
            Err(BackupValidationError::UnsupportedSchema)
        );
    }

    #[test]
    fn current_activity_history_is_checked_and_copied_without_numeric_or_home_changes() {
        let dir = crate::tests::private_directory();
        let mut storage = Storage::create(&dir.path().join("live"), &KEY).unwrap();
        storage
            .create_profile(&id(1), "Synthetic history", &id(1))
            .unwrap();
        activity_fixture(&mut storage, &id(1), &id(1), 100);
        let expected = activity_records(&storage.connection);
        let snapshot = storage.backup_snapshot().unwrap();
        let original = snapshot.bytes.clone();
        with_validated_snapshot::<_, BackupValidationError>(
            &snapshot.bytes,
            snapshot.schema_version,
            true,
            |source, _| {
                let copy = trusted_memory_copy(source, "main", CURRENT_SCHEMA_VERSION)?;
                assert_eq!(activity_records(&copy), expected);
                assert_eq!(
                    copy.query_row("SELECT max(tokens) FROM current_activity_days", [], |r| {
                        r.get::<_, String>(0)
                    })
                    .unwrap(),
                    "0"
                );
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(*snapshot.bytes, *original);
    }

    #[test]
    fn malformed_activity_history_and_cross_record_inconsistency_are_refused_without_changes() {
        for sql in [
            "UPDATE activity_summaries SET lifetime_tokens='01'",
            "UPDATE activity_summaries SET lifetime_tokens='9223372036854775808'",
            "UPDATE activity_summaries SET lifetime_tokens='-1'",
            "UPDATE activity_day_history SET provider_date='2023-02-29'",
            "UPDATE collection_runs SET observed_at_utc='2024-02-29T12:00:00+00:00'",
            "UPDATE collection_runs SET completed_at_utc='2024-02-29T09:00:00.000Z'",
            "UPDATE collection_runs SET provider_version=char(10)",
            "UPDATE collection_runs SET normalized_unit='invented'",
            "UPDATE collection_runs SET source_scope='inferred_app_scope'",
            "UPDATE profile_identity_events SET home_reference='../external'",
            "UPDATE profile_identity_events SET returned_email=char(10)",
            "UPDATE profile_identity_events SET previous_binding_id=id",
            "UPDATE profile_identity_events SET previous_binding_id='00000065-0000-4000-8000-000000000000' WHERE id='00000064-0000-4000-8000-000000000000'",
            "UPDATE collection_runs SET binding_id='00000001-0000-4000-8000-000000000000'",
            "DELETE FROM activity_summaries WHERE id='0000006e-0000-4000-8000-000000000000'",
            "UPDATE activity_summaries SET daily_state='empty' WHERE daily_state='returned'",
            "UPDATE current_activity_days SET tokens='1'",
            "UPDATE current_activity_days SET observed_at_utc='2024-02-29T12:00:00.000Z'",
            "UPDATE current_activity_days SET binding_id='00000064-0000-4000-8000-000000000000'",
        ] {
            let dir = crate::tests::private_directory();
            let mut storage = Storage::create(&dir.path().join("live"), &KEY).unwrap();
            storage
                .create_profile(&id(1), "Synthetic history", &id(1))
                .unwrap();
            activity_fixture(&mut storage, &id(1), &id(1), 100);
            storage
                .connection
                .execute_batch("PRAGMA foreign_keys=OFF; PRAGMA ignore_check_constraints=ON;")
                .unwrap();
            storage.connection.execute_batch(sql).unwrap();
            let snapshot = storage.backup_snapshot().unwrap();
            let original = snapshot.bytes.clone();
            assert_eq!(
                validate_backup_snapshot(&snapshot.bytes, snapshot.schema_version),
                Err(BackupValidationError::InvalidDatabase),
                "{sql}"
            );
            assert_eq!(*snapshot.bytes, *original);
        }
    }
}
