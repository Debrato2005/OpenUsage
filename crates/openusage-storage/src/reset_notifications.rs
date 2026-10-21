//! Durable at-most-once submission attempts, with opt-in and IANA quiet hours.
//! Submitted means that the OS service accepted the request, not that a person saw it.
use crate::{AlertDeliveryState, QuotaWindowRole, Storage, StorageError};
use jiff::{Timestamp, tz::TimeZone};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use std::collections::BTreeSet;

pub(crate) const PREFERENCE_COLUMNS: &str =
    "id,enabled,quiet_enabled,time_zone,quiet_start_minute,quiet_end_minute";
pub(crate) const OVERRIDE_COLUMNS: &str = "profile_id,enabled";
pub(crate) const OUTBOX_COLUMNS: &str =
    "event_id,state,attempt_id,attempted_at_utc,completed_at_utc";
pub(crate) const RECEIPT_COLUMNS: &str =
    "event_key,attempt_id,state,attempted_at_utc,completed_at_utc";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AlertQuietHours {
    pub time_zone: String,
    pub start_minute: u16,
    pub end_minute: u16,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResetAlertOverride {
    pub profile_id: String,
    pub enabled: bool,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResetAlertPreferences {
    pub enabled: bool,
    pub quiet_enabled: bool,
    pub quiet_hours: AlertQuietHours,
    pub profile_overrides: Vec<ResetAlertOverride>,
}
impl Default for ResetAlertPreferences {
    fn default() -> Self {
        let zone = TimeZone::system();
        Self {
            enabled: false,
            quiet_enabled: true,
            quiet_hours: AlertQuietHours {
                time_zone: zone.iana_name().unwrap_or("Etc/UTC").to_owned(),
                start_minute: 22 * 60,
                end_minute: 8 * 60,
            },
            profile_overrides: Vec::new(),
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NotificationOutcome {
    Submitted,
    Failed,
}
impl NotificationOutcome {
    fn as_str(self) -> &'static str {
        match self {
            Self::Submitted => "submitted",
            Self::Failed => "failed",
        }
    }
}
#[derive(Clone, Eq, PartialEq)]
pub struct ResetNotification {
    pub event_id: String,
    pub attempt_id: String,
    pub profile_id: String,
    pub profile_label: String,
    pub window_role: QuotaWindowRole,
    pub window_duration_mins: u64,
    pub evidence_at_utc: String,
}
impl std::fmt::Debug for ResetNotification {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ResetNotification { .. }")
    }
}
type Result<T> = std::result::Result<T, StorageError>;
fn read(_: rusqlite::Error) -> StorageError {
    StorageError::ReadFailed
}
fn write(_: rusqlite::Error) -> StorageError {
    StorageError::WriteFailed
}
fn timestamp(value: &str) -> Result<Timestamp> {
    if !crate::activity::activity_utc_timestamp(value) {
        return Err(StorageError::InvalidAlertPreferences);
    }
    value
        .parse()
        .map_err(|_| StorageError::InvalidAlertPreferences)
}
fn valid_zone(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"/_+-".contains(&b))
        && TimeZone::get(value)
            .is_ok_and(|zone| zone.iana_name() == Some(value) || matches!(value, "UTC" | "Etc/UTC"))
}
fn validate_preferences(value: &ResetAlertPreferences) -> Result<()> {
    if !valid_zone(&value.quiet_hours.time_zone)
        || value.quiet_hours.start_minute >= 1440
        || value.quiet_hours.end_minute >= 1440
        || value.quiet_hours.start_minute == value.quiet_hours.end_minute
        || value.profile_overrides.len() > 100
    {
        return Err(StorageError::InvalidAlertPreferences);
    }
    let mut ids = BTreeSet::new();
    for item in &value.profile_overrides {
        if !crate::backup_validation::canonical_uuid(&item.profile_id)
            || !ids.insert(&item.profile_id)
        {
            return Err(StorageError::InvalidAlertPreferences);
        }
    }
    Ok(())
}
fn load(connection: &Connection) -> Result<ResetAlertPreferences> {
    let record = connection.query_row("SELECT enabled,quiet_enabled,time_zone,quiet_start_minute,quiet_end_minute FROM reset_alert_preferences WHERE id=1", [], |r| Ok(ResetAlertPreferences {
        enabled:r.get(0)?,quiet_enabled:r.get(1)?,quiet_hours:AlertQuietHours{time_zone:r.get(2)?,start_minute:r.get(3)?,end_minute:r.get(4)?},profile_overrides:Vec::new()
    })).optional().map_err(read)?;
    let mut value = record.unwrap_or_default();
    let mut statement = connection
        .prepare("SELECT profile_id,enabled FROM reset_alert_overrides ORDER BY profile_id")
        .map_err(read)?;
    value.profile_overrides = statement
        .query_map([], |r| {
            Ok(ResetAlertOverride {
                profile_id: r.get(0)?,
                enabled: r.get(1)?,
            })
        })
        .map_err(read)?
        .collect::<rusqlite::Result<_>>()
        .map_err(read)?;
    validate_preferences(&value).map_err(|_| StorageError::ReadFailed)?;
    Ok(value)
}
fn source_eligible(value: &str) -> bool {
    value == "official_artifact" || (cfg!(feature = "native-test") && value == "test_fixture")
}
fn enabled(value: &ResetAlertPreferences, profile: &str) -> bool {
    value
        .profile_overrides
        .iter()
        .find(|item| item.profile_id == profile)
        .map_or(value.enabled, |item| item.enabled)
}
fn quiet(value: &ResetAlertPreferences, now: Timestamp) -> Result<bool> {
    if !value.quiet_enabled {
        return Ok(false);
    }
    let local = now.to_zoned(
        TimeZone::get(&value.quiet_hours.time_zone).map_err(|_| StorageError::ReadFailed)?,
    );
    let minute = local.hour() as u16 * 60 + local.minute() as u16;
    let (start, end) = (value.quiet_hours.start_minute, value.quiet_hours.end_minute);
    Ok(if start < end {
        minute >= start && minute < end
    } else {
        minute >= start || minute < end
    })
}
impl Storage {
    pub fn load_reset_alert_preferences(&self) -> Result<ResetAlertPreferences> {
        load(&self.connection)
    }
    pub fn save_reset_alert_preferences(&mut self, value: &ResetAlertPreferences) -> Result<()> {
        validate_preferences(value)?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(write)?;
        for item in &value.profile_overrides {
            let exists: bool = tx
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM profiles WHERE id=?1)",
                    [&item.profile_id],
                    |r| r.get(0),
                )
                .map_err(read)?;
            if !exists {
                return Err(StorageError::InvalidAlertPreferences);
            }
        }
        tx.execute("INSERT INTO reset_alert_preferences VALUES(1,?1,?2,?3,?4,?5) ON CONFLICT(id) DO UPDATE SET enabled=excluded.enabled,quiet_enabled=excluded.quiet_enabled,time_zone=excluded.time_zone,quiet_start_minute=excluded.quiet_start_minute,quiet_end_minute=excluded.quiet_end_minute",params![value.enabled,value.quiet_enabled,value.quiet_hours.time_zone,value.quiet_hours.start_minute,value.quiet_hours.end_minute]).map_err(write)?;
        tx.execute("DELETE FROM reset_alert_overrides", [])
            .map_err(write)?;
        for item in &value.profile_overrides {
            tx.execute(
                "INSERT INTO reset_alert_overrides VALUES(?1,?2)",
                params![item.profile_id, item.enabled],
            )
            .map_err(write)?;
        }
        // Disabling an account cancels its existing queue; enabling never replays it.
        let mut query=tx.prepare("SELECT o.event_id,e.profile_id FROM reset_notification_outbox o JOIN alert_events e ON e.id=o.event_id WHERE o.state='queued'").map_err(read)?;
        let queued = query
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
            .map_err(read)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(read)?;
        drop(query);
        for (id, profile) in queued {
            if !enabled(value, &profile) {
                tx.execute("UPDATE reset_notification_outbox SET state='suppressed' WHERE event_id=?1 AND state='queued'",[id]).map_err(write)?;
            }
        }
        tx.commit().map_err(write)
    }
    /// Atomically consume one eligible event before any external OS side effect.
    /// An interrupted attempt stays uncertain and is never automatically retried.
    pub fn claim_reset_notification(&mut self, now_utc: &str) -> Result<Option<ResetNotification>> {
        let now = timestamp(now_utc)?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(write)?;
        let prefs = load(&tx)?;
        let mut query=tx.prepare("SELECT e.id,e.event_key,e.profile_id,p.name,e.window_role,e.window_duration_mins,e.evidence_at_utc,p.archived,p.approved_home_reference,e.binding_id,e.generation_run_id,e.limit_id,b.binding_id,i.home_reference,c.collection_run_id,r.observed_at_utc,a.generation_run_id,e.source_kind,r.source_kind,a.state FROM reset_notification_outbox o JOIN alert_events e ON e.id=o.event_id JOIN profiles p ON p.id=e.profile_id LEFT JOIN profile_bindings b ON b.profile_id=p.id LEFT JOIN profile_identity_events i ON i.id=b.binding_id AND i.profile_id=b.profile_id LEFT JOIN current_quota_readings c ON c.profile_id=p.id LEFT JOIN collection_runs r ON r.id=c.collection_run_id LEFT JOIN quota_reset_assessments a ON a.collection_run_id=c.collection_run_id AND a.limit_id=e.limit_id AND a.window_role=e.window_role WHERE o.state='queued' ORDER BY e.evidence_at_utc,e.id LIMIT 256").map_err(read)?;
        let candidates = query
            .query_map([], |r| {
                Ok(Candidate {
                    id: r.get(0)?,
                    key: r.get(1)?,
                    profile: r.get(2)?,
                    label: r.get(3)?,
                    role: r.get(4)?,
                    duration: u64::try_from(r.get::<_, i64>(5)?)
                        .map_err(|_| rusqlite::Error::InvalidQuery)?,
                    evidence: r.get(6)?,
                    archived: r.get(7)?,
                    home: r.get(8)?,
                    binding: r.get(9)?,
                    generation: r.get(10)?,
                    current_binding: r.get(12)?,
                    current_home: r.get(13)?,
                    current_run: r.get(14)?,
                    current_observed: r.get(15)?,
                    current_generation: r.get(16)?,
                    source: r.get(17)?,
                    latest_source: r.get(18)?,
                    current_reset_state: r.get(19)?,
                })
            })
            .map_err(read)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(read)?;
        drop(query);
        let is_quiet = quiet(&prefs, now)?;
        for candidate in candidates {
            let recorded = timestamp(&candidate.evidence)?;
            let latest = candidate
                .current_observed
                .as_deref()
                .map(timestamp)
                .transpose()?;
            let obsolete = candidate.archived
                || !source_eligible(&candidate.source)
                || candidate
                    .latest_source
                    .as_deref()
                    .is_none_or(|source| !source_eligible(source))
                || !enabled(&prefs, &candidate.profile)
                || candidate.current_binding.as_deref() != Some(&candidate.binding)
                || candidate.current_home.as_deref() != Some(&candidate.home)
                || candidate.current_generation.as_deref() != Some(&candidate.generation)
                || candidate.current_reset_state.as_deref() != Some("confirmed")
                || candidate.current_run.is_none()
                || now.as_millisecond() - recorded.as_millisecond() > 86_400_000;
            if obsolete {
                tx.execute("UPDATE reset_notification_outbox SET state='suppressed' WHERE event_id=?1 AND state='queued'",[&candidate.id]).map_err(write)?;
                continue;
            }
            // Wall-clock rollback cannot manufacture eligibility. Wait for a fresh reading.
            if recorded > now
                || latest.is_none_or(|value| {
                    value > now || now.as_millisecond() - value.as_millisecond() > 600_000
                })
                || is_quiet
            {
                continue;
            }
            let consumed: bool = tx
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM reset_notification_receipts WHERE event_key=?1)",
                    [&candidate.key],
                    |r| r.get(0),
                )
                .map_err(read)?;
            if consumed {
                tx.execute("UPDATE reset_notification_outbox SET state='suppressed' WHERE event_id=?1 AND state='queued'",[&candidate.id]).map_err(write)?;
                continue;
            }
            let attempt = crate::activity::new_uuid().map_err(|_| StorageError::WriteFailed)?;
            tx.execute(
                "INSERT INTO reset_notification_receipts VALUES(?1,?2,'uncertain',?3,NULL)",
                params![candidate.key, attempt, now_utc],
            )
            .map_err(write)?;
            tx.execute("UPDATE reset_notification_outbox SET state='uncertain',attempt_id=?2,attempted_at_utc=?3 WHERE event_id=?1 AND state='queued'",params![candidate.id,attempt,now_utc]).map_err(write)?;
            let notification = ResetNotification {
                event_id: candidate.id,
                attempt_id: attempt,
                profile_id: candidate.profile,
                profile_label: candidate.label,
                window_role: match candidate.role.as_str() {
                    "primary" => QuotaWindowRole::Primary,
                    "secondary" => QuotaWindowRole::Secondary,
                    _ => return Err(StorageError::ReadFailed),
                },
                window_duration_mins: candidate.duration,
                evidence_at_utc: candidate.evidence,
            };
            tx.commit().map_err(write)?;
            return Ok(Some(notification));
        }
        tx.commit().map_err(write)?;
        Ok(None)
    }
    pub fn complete_reset_notification(
        &mut self,
        event_id: &str,
        attempt_id: &str,
        outcome: NotificationOutcome,
    ) -> Result<()> {
        if !crate::backup_validation::canonical_uuid(event_id)
            || !crate::backup_validation::canonical_uuid(attempt_id)
        {
            return Err(StorageError::InvalidAlertPreferences);
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(write)?;
        let key:Option<String>=tx.query_row("SELECT e.event_key FROM reset_notification_outbox o JOIN alert_events e ON e.id=o.event_id JOIN reset_notification_receipts r ON r.event_key=e.event_key AND r.attempt_id=o.attempt_id WHERE o.event_id=?1 AND o.attempt_id=?2 AND o.state='uncertain' AND r.state='uncertain'",params![event_id,attempt_id],|r|r.get(0)).optional().map_err(read)?;
        let Some(key) = key else {
            return Err(StorageError::WriteFailed);
        };
        let completed: String = tx
            .query_row("SELECT strftime('%Y-%m-%dT%H:%M:%fZ','now')", [], |r| {
                r.get(0)
            })
            .map_err(read)?;
        tx.execute("UPDATE reset_notification_receipts SET state=?2,completed_at_utc=?3 WHERE event_key=?1 AND state='uncertain'",params![key,outcome.as_str(),completed]).map_err(write)?;
        tx.execute("UPDATE reset_notification_outbox SET state=?2,completed_at_utc=?3 WHERE event_id=?1 AND state='uncertain'",params![event_id,outcome.as_str(),completed]).map_err(write)?;
        tx.commit().map_err(write)
    }
}
struct Candidate {
    id: String,
    key: String,
    profile: String,
    label: String,
    role: String,
    duration: u64,
    evidence: String,
    archived: bool,
    home: String,
    binding: String,
    generation: String,
    current_binding: Option<String>,
    current_home: Option<String>,
    current_run: Option<String>,
    current_observed: Option<String>,
    current_generation: Option<String>,
    source: String,
    latest_source: Option<String>,
    current_reset_state: Option<String>,
}

pub(crate) fn enqueue(
    connection: &Connection,
    event_id: &str,
    profile: &str,
    source: &str,
) -> std::result::Result<(), crate::ActivityError> {
    // Only a compile-time native-test build can queue explicit synthetic sources.
    // The production predicate is repeated at claim to refuse an imported fixture queue.
    if !source_eligible(source) {
        return Ok(());
    }
    let prefs = load(connection).map_err(|_| crate::ActivityError::ReadFailed)?;
    connection
        .execute(
            "INSERT INTO reset_notification_outbox VALUES(?1,?2,NULL,NULL,NULL)",
            params![
                event_id,
                if enabled(&prefs, profile) {
                    "queued"
                } else {
                    "suppressed"
                }
            ],
        )
        .map_err(crate::activity::write_error)?;
    Ok(())
}

pub(crate) fn delivery_state(value: &str) -> Option<AlertDeliveryState> {
    Some(match value {
        "recorded" => AlertDeliveryState::Recorded,
        "queued" => AlertDeliveryState::Queued,
        "uncertain" => AlertDeliveryState::Uncertain,
        "submitted" => AlertDeliveryState::Submitted,
        "failed" => AlertDeliveryState::Failed,
        "suppressed" => AlertDeliveryState::Suppressed,
        _ => return None,
    })
}

pub(crate) fn validate(
    connection: &Connection,
    schema: &str,
) -> std::result::Result<(), crate::BackupValidationError> {
    use crate::backup_validation::{canonical_uuid, require, sql_error, text};
    require(matches!(schema, "main" | "openusage_checkpoint"))?;
    let mut preference=connection.prepare(&format!("SELECT id,enabled,quiet_enabled,time_zone,quiet_start_minute,quiet_end_minute FROM {schema}.reset_alert_preferences")).map_err(sql_error)?;
    let mut rows = preference.query([]).map_err(sql_error)?;
    if let Some(row) = rows.next().map_err(sql_error)? {
        require(row.get::<_, i64>(0).map_err(sql_error)? == 1 && valid_zone(text(row, 3)?))?;
        let start: i64 = row.get(4).map_err(sql_error)?;
        let end: i64 = row.get(5).map_err(sql_error)?;
        require((0..1440).contains(&start) && (0..1440).contains(&end) && start != end)?;
        for index in [1, 2] {
            require(matches!(
                row.get::<_, i64>(index).map_err(sql_error)?,
                0 | 1
            ))?;
        }
    }
    require(rows.next().map_err(sql_error)?.is_none())?;
    let mut overrides = connection
        .prepare(&format!(
            "SELECT profile_id,enabled FROM {schema}.reset_alert_overrides"
        ))
        .map_err(sql_error)?;
    let mut rows = overrides.query([]).map_err(sql_error)?;
    let mut count = 0;
    while let Some(row) = rows.next().map_err(sql_error)? {
        count += 1;
        require(
            count <= 100
                && canonical_uuid(text(row, 0)?)
                && matches!(row.get::<_, i64>(1).map_err(sql_error)?, 0 | 1),
        )?;
    }
    let mut receipts = connection
        .prepare(&format!(
            "SELECT {RECEIPT_COLUMNS} FROM {schema}.reset_notification_receipts"
        ))
        .map_err(sql_error)?;
    let mut rows = receipts.query([]).map_err(sql_error)?;
    while let Some(row) = rows.next().map_err(sql_error)? {
        let key = text(row, 0)?;
        let state = text(row, 2)?;
        let completed: Option<String> = row.get(4).map_err(sql_error)?;
        require(
            key.starts_with("reset_completed:1:")
                && crate::activity::safe_text(key, 8192)
                && canonical_uuid(text(row, 1)?)
                && matches!(state, "uncertain" | "submitted" | "failed")
                && crate::activity::activity_utc_timestamp(text(row, 3)?)
                && if state == "uncertain" {
                    completed.is_none()
                } else {
                    completed
                        .as_deref()
                        .is_some_and(crate::activity::activity_utc_timestamp)
                },
        )?;
    }
    let mut outbox=connection.prepare(&format!("SELECT o.event_id,o.state,o.attempt_id,o.attempted_at_utc,o.completed_at_utc,e.source_kind,r.attempt_id,r.state,r.attempted_at_utc,r.completed_at_utc FROM {schema}.reset_notification_outbox o JOIN {schema}.alert_events e ON e.id=o.event_id LEFT JOIN {schema}.reset_notification_receipts r ON r.event_key=e.event_key")).map_err(sql_error)?;
    let mut rows = outbox.query([]).map_err(sql_error)?;
    while let Some(row) = rows.next().map_err(sql_error)? {
        let state = text(row, 1)?;
        let attempt: Option<String> = row.get(2).map_err(sql_error)?;
        let attempted: Option<String> = row.get(3).map_err(sql_error)?;
        let completed: Option<String> = row.get(4).map_err(sql_error)?;
        require(
            canonical_uuid(text(row, 0)?)
                && matches!(text(row, 5)?, "official_artifact" | "test_fixture"),
        )?;
        if matches!(state, "queued" | "suppressed") {
            require(attempt.is_none() && attempted.is_none() && completed.is_none())?;
        } else {
            require(
                matches!(state, "uncertain" | "submitted" | "failed")
                    && attempt.as_deref().is_some_and(canonical_uuid)
                    && attempted
                        .as_deref()
                        .is_some_and(crate::activity::activity_utc_timestamp)
                    && if state == "uncertain" {
                        completed.is_none()
                    } else {
                        completed
                            .as_deref()
                            .is_some_and(crate::activity::activity_utc_timestamp)
                    },
            )?;
            require(
                attempt == row.get::<_, Option<String>>(6).map_err(sql_error)?
                    && row
                        .get::<_, Option<String>>(7)
                        .map_err(sql_error)?
                        .as_deref()
                        == Some(state)
                    && attempted == row.get::<_, Option<String>>(8).map_err(sql_error)?
                    && completed == row.get::<_, Option<String>>(9).map_err(sql_error)?,
            )?;
        }
    }
    Ok(())
}

/// Copy only application-owned columns. Restore retains current receipts and never
/// reactivates imported pending effects, including when applying an older checkpoint.
pub(crate) fn copy_records(
    source: &Connection,
    target: &Connection,
    schema: &str,
    restore: bool,
) -> std::result::Result<(), crate::BackupValidationError> {
    use crate::backup_validation::{copy_fields, require, sql_error};
    require(matches!(schema, "main" | "openusage_checkpoint"))?;
    for (table, columns) in [
        ("reset_alert_preferences", PREFERENCE_COLUMNS),
        ("reset_alert_overrides", OVERRIDE_COLUMNS),
    ] {
        copy_fields(source, target, schema, table, columns, columns)?;
    }
    let mut receipts = source
        .prepare(&format!(
            "SELECT {RECEIPT_COLUMNS} FROM {schema}.reset_notification_receipts"
        ))
        .map_err(sql_error)?;
    let rows = receipts
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, Option<String>>(4)?,
            ))
        })
        .map_err(sql_error)?;
    for row in rows {
        let (key, attempt, state, attempted, completed) = row.map_err(sql_error)?;
        target.execute("INSERT INTO reset_notification_receipts VALUES(?1,?2,?3,?4,?5) ON CONFLICT(event_key) DO NOTHING",params![key,attempt,state,attempted,completed]).map_err(sql_error)?;
    }
    copy_fields(
        source,
        target,
        schema,
        "reset_notification_outbox",
        OUTBOX_COLUMNS,
        OUTBOX_COLUMNS,
    )?;
    if restore {
        target.execute("UPDATE reset_notification_outbox SET state='suppressed',attempt_id=NULL,attempted_at_utc=NULL,completed_at_utc=NULL WHERE state='queued'",[]).map_err(sql_error)?;
        // A current receipt wins over an older archive's receipt for the same event.
        target.execute_batch("UPDATE reset_notification_outbox SET (state,attempt_id,attempted_at_utc,completed_at_utc)=(SELECT r.state,r.attempt_id,r.attempted_at_utc,r.completed_at_utc FROM reset_notification_receipts r JOIN alert_events e ON e.event_key=r.event_key WHERE e.id=reset_notification_outbox.event_id) WHERE EXISTS(SELECT 1 FROM reset_notification_receipts r JOIN alert_events e ON e.event_key=r.event_key WHERE e.id=reset_notification_outbox.event_id)").map_err(sql_error)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn quiet_hours_follow_real_iana_dst_transitions_and_exact_boundaries() {
        let prefs = ResetAlertPreferences {
            quiet_hours: AlertQuietHours {
                time_zone: "America/New_York".into(),
                start_minute: 1320,
                end_minute: 480,
            },
            ..Default::default()
        };
        for (instant, expected) in [
            ("2026-03-08T06:59:59Z", true),
            ("2026-03-08T07:00:00Z", true),
            ("2026-03-08T11:59:59Z", true),
            ("2026-03-08T12:00:00Z", false),
            ("2026-11-01T05:59:59Z", true),
            ("2026-11-01T06:00:00Z", true),
            ("2026-11-01T12:59:59Z", true),
            ("2026-11-01T13:00:00Z", false),
        ] {
            assert_eq!(
                quiet(&prefs, instant.parse().unwrap()).unwrap(),
                expected,
                "{instant}"
            );
        }
        let disabled = ResetAlertPreferences {
            quiet_enabled: false,
            ..prefs.clone()
        };
        assert!(!quiet(&disabled, "2026-03-08T07:00:00Z".parse().unwrap()).unwrap());
    }
}
