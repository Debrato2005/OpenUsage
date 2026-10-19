//! Semantic checks for authenticated, exact-catalog schema 3 snapshots.
use crate::activity::activity_utc_timestamp;
use crate::backup_validation::{
    BackupValidationError, canonical_uuid, optional_text, require, sql_error, text,
};
use rusqlite::Connection;
type Result<T> = std::result::Result<T, BackupValidationError>;
pub(crate) fn validate_quota(connection: &Connection, schema: &str) -> Result<()> {
    let mut statement=connection.prepare(&format!("SELECT profile_id,binding_id,created_at_utc,updated_at_utc FROM {schema}.profile_bindings")).map_err(sql_error)?;
    let mut rows = statement.query([]).map_err(sql_error)?;
    while let Some(row) = rows.next().map_err(sql_error)? {
        require(
            canonical_uuid(text(row, 0)?)
                && canonical_uuid(text(row, 1)?)
                && activity_utc_timestamp(text(row, 2)?)
                && activity_utc_timestamp(text(row, 3)?),
        )?;
    }
    let mut statement = connection
        .prepare(&format!("SELECT id FROM {schema}.quota_snapshots"))
        .map_err(sql_error)?;
    let mut rows = statement.query([]).map_err(sql_error)?;
    while let Some(row) = rows.next().map_err(sql_error)? {
        crate::quota::load_snapshot(connection, schema, text(row, 0)?)
            .map_err(|_| BackupValidationError::InvalidDatabase)?;
    }
    for table in ["current_quota_readings", "current_quota"] {
        let mut statement = connection
            .prepare(&format!(
                "SELECT created_at_utc,updated_at_utc FROM {schema}.{table}"
            ))
            .map_err(sql_error)?;
        let mut rows = statement.query([]).map_err(sql_error)?;
        while let Some(row) = rows.next().map_err(sql_error)? {
            require(
                activity_utc_timestamp(text(row, 0)?) && activity_utc_timestamp(text(row, 1)?),
            )?;
        }
    }
    // Method matching and cache completeness are separate from structural foreign keys.
    let invalid:bool=connection.query_row(&format!("SELECT
 EXISTS(SELECT 1 FROM {schema}.quota_snapshots q JOIN {schema}.collection_runs r ON r.id=q.id WHERE r.source_method!='account/rateLimits/read')
 OR EXISTS(SELECT 1 FROM {schema}.collection_runs r LEFT JOIN {schema}.quota_snapshots q ON q.id=r.id WHERE r.source_method='account/rateLimits/read' AND q.id IS NULL)
 OR EXISTS(SELECT 1 FROM {schema}.current_activity c LEFT JOIN {schema}.profile_bindings b ON b.profile_id=c.profile_id WHERE b.binding_id IS NULL OR b.binding_id!=c.binding_id)
 OR EXISTS(SELECT 1 FROM {schema}.current_quota_readings c LEFT JOIN {schema}.profile_bindings b ON b.profile_id=c.profile_id JOIN {schema}.collection_runs r ON r.id=c.collection_run_id WHERE b.binding_id IS NULL OR b.binding_id!=c.binding_id OR c.updated_at_utc!=r.completed_at_utc)
 OR EXISTS(SELECT 1 FROM {schema}.current_quota c JOIN {schema}.quota_limits l ON l.collection_run_id=c.collection_run_id AND l.limit_id=c.limit_id JOIN {schema}.collection_runs r ON r.id=c.collection_run_id WHERE c.updated_at_utc!=r.completed_at_utc OR (c.window_role='primary' AND (c.used_percent IS NOT l.primary_used_percent OR c.window_duration_mins IS NOT l.primary_duration_mins OR c.resets_at_utc IS NOT l.primary_resets_at_utc)) OR (c.window_role='secondary' AND (c.used_percent IS NOT l.secondary_used_percent OR c.window_duration_mins IS NOT l.secondary_duration_mins OR c.resets_at_utc IS NOT l.secondary_resets_at_utc)))
 OR EXISTS(SELECT 1 FROM {schema}.current_quota_readings c JOIN {schema}.quota_limits l ON l.collection_run_id=c.collection_run_id WHERE (l.primary_used_percent IS NOT NULL AND NOT EXISTS(SELECT 1 FROM {schema}.current_quota w WHERE w.profile_id=c.profile_id AND w.limit_id=l.limit_id AND w.window_role='primary')) OR (l.secondary_used_percent IS NOT NULL AND NOT EXISTS(SELECT 1 FROM {schema}.current_quota w WHERE w.profile_id=c.profile_id AND w.limit_id=l.limit_id AND w.window_role='secondary')))
 OR EXISTS(SELECT 1 FROM {schema}.quota_limits WHERE spend_control_reached NOT IN (0,1))
 OR EXISTS(SELECT 1 FROM {schema}.quota_limits WHERE (primary_used_percent IS NULL AND (primary_duration_mins IS NOT NULL OR primary_resets_at_utc IS NOT NULL)) OR (secondary_used_percent IS NULL AND (secondary_duration_mins IS NOT NULL OR secondary_resets_at_utc IS NOT NULL)))
 "),[],|r|r.get(0)).map_err(sql_error)?;
    require(!invalid)?;
    // Retain strict nullable/count checks even if a malicious image ignored CHECK constraints.
    let mut statement = connection
        .prepare(&format!(
            "SELECT available_reset_credits,credit_details_state FROM {schema}.quota_snapshots"
        ))
        .map_err(sql_error)?;
    let mut rows = statement.query([]).map_err(sql_error)?;
    while let Some(row) = rows.next().map_err(sql_error)? {
        require(
            optional_text(row, 0)?.is_none_or(crate::activity::activity_decimal)
                && matches!(text(row, 1)?, "unavailable" | "empty" | "returned"),
        )?;
    }
    Ok(())
}
