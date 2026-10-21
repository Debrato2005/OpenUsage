//! Thresholds use accepted immutable observations within a confirmed reset generation.
//! Historical imports never replay crossings; covered keys share the restore-safe journal.
use crate::activity::{read_error, require, write_error};
use crate::{
    ActivityError, QuotaResetState, QuotaWindowRole, ResetAlertOverride, Storage, StorageError,
    StoredQuota,
};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use std::collections::BTreeSet;

pub(crate) const DETAIL_COLUMNS: &str =
    "event_id,threshold_percent,evidence_run_id,accepted_at_utc";
pub(crate) const PREFERENCE_COLUMNS: &str = "id,enabled";
pub(crate) const OVERRIDE_COLUMNS: &str = "profile_id,enabled";
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ThresholdAlertPreferences {
    pub enabled: bool,
    pub profile_overrides: Vec<ResetAlertOverride>,
}
pub(crate) fn validate_now(value: &str) -> Result<(), ActivityError> {
    require(
        crate::activity::activity_utc_timestamp(value) && value.parse::<jiff::Timestamp>().is_ok(),
    )
}
pub(crate) fn fresh(observed: &str, completed: &str, now: &str) -> bool {
    let parse = |value: &str| value.parse::<jiff::Timestamp>().ok();
    let (Some(observed), Some(completed), Some(now)) =
        (parse(observed), parse(completed), parse(now))
    else {
        return false;
    };
    observed <= completed
        && completed <= now
        && now.as_millisecond() - observed.as_millisecond() < 600_000
}
fn valid_preferences(value: &ThresholdAlertPreferences) -> bool {
    let mut ids = BTreeSet::new();
    value.profile_overrides.len() <= 100
        && value.profile_overrides.iter().all(|item| {
            crate::backup_validation::canonical_uuid(&item.profile_id)
                && ids.insert(&item.profile_id)
        })
}
pub(crate) fn load(connection: &Connection) -> Result<ThresholdAlertPreferences, StorageError> {
    let enabled = connection
        .query_row(
            "SELECT enabled FROM threshold_alert_preferences WHERE id=1",
            [],
            |r| r.get(0),
        )
        .optional()
        .map_err(|_| StorageError::ReadFailed)?
        .unwrap_or(false);
    let mut query = connection
        .prepare("SELECT profile_id,enabled FROM threshold_alert_overrides ORDER BY profile_id")
        .map_err(|_| StorageError::ReadFailed)?;
    let profile_overrides = query
        .query_map([], |r| {
            Ok(ResetAlertOverride {
                profile_id: r.get(0)?,
                enabled: r.get(1)?,
            })
        })
        .map_err(|_| StorageError::ReadFailed)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|_| StorageError::ReadFailed)?;
    let value = ThresholdAlertPreferences {
        enabled,
        profile_overrides,
    };
    if !valid_preferences(&value) {
        return Err(StorageError::ReadFailed);
    }
    Ok(value)
}
pub(crate) fn enabled(value: &ThresholdAlertPreferences, profile: &str) -> bool {
    value
        .profile_overrides
        .iter()
        .find(|item| item.profile_id == profile)
        .map_or(value.enabled, |item| item.enabled)
}
pub(crate) fn save(
    connection: &Connection,
    value: &ThresholdAlertPreferences,
) -> Result<(), StorageError> {
    if !valid_preferences(value) {
        return Err(StorageError::InvalidAlertPreferences);
    }
    for item in &value.profile_overrides {
        let exists: bool = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM profiles WHERE id=?1)",
                [&item.profile_id],
                |r| r.get(0),
            )
            .map_err(|_| StorageError::ReadFailed)?;
        if !exists {
            return Err(StorageError::InvalidAlertPreferences);
        }
    }
    connection.execute("INSERT INTO threshold_alert_preferences VALUES(1,?1) ON CONFLICT(id) DO UPDATE SET enabled=excluded.enabled", [value.enabled]).map_err(|_|StorageError::WriteFailed)?;
    connection
        .execute("DELETE FROM threshold_alert_overrides", [])
        .map_err(|_| StorageError::WriteFailed)?;
    for item in &value.profile_overrides {
        connection
            .execute(
                "INSERT INTO threshold_alert_overrides VALUES(?1,?2)",
                params![item.profile_id, item.enabled],
            )
            .map_err(|_| StorageError::WriteFailed)?;
    }
    let mut query = connection.prepare("SELECT o.event_id,e.profile_id FROM reset_notification_outbox o JOIN alert_events e ON e.id=o.event_id JOIN threshold_alert_events t ON t.event_id=e.id WHERE o.state='queued'").map_err(|_|StorageError::ReadFailed)?;
    let queued = query
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
        .map_err(|_| StorageError::ReadFailed)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|_| StorageError::ReadFailed)?;
    drop(query);
    for (id, profile) in queued {
        if !enabled(value, &profile) {
            connection
                .execute(
                    "UPDATE reset_notification_outbox SET state='suppressed' WHERE event_id=?1",
                    [id],
                )
                .map_err(|_| StorageError::WriteFailed)?;
        }
    }
    Ok(())
}
impl Storage {
    pub fn load_threshold_alert_preferences(
        &self,
    ) -> Result<ThresholdAlertPreferences, StorageError> {
        load(&self.connection)
    }
    pub fn save_threshold_alert_preferences(
        &mut self,
        value: &ThresholdAlertPreferences,
    ) -> Result<(), StorageError> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| StorageError::WriteFailed)?;
        save(&tx, value)?;
        tx.commit().map_err(|_| StorageError::WriteFailed)
    }
}

pub(crate) fn event_key(
    packet: &StoredQuota,
    identity: Option<&str>,
    limit: &str,
    role: QuotaWindowRole,
    duration: u64,
    generation: &str,
    threshold: u8,
) -> String {
    let key = crate::alerts::event_key(
        &packet.binding.profile_id,
        identity,
        &packet.binding.binding_id,
        limit,
        role.as_str(),
        duration,
        generation,
    );
    format!(
        "usage_threshold:1:{}:{threshold}",
        key.strip_prefix("reset_completed:1:")
            .expect("reset key prefix")
    )
}
pub(crate) fn record(
    connection: &Connection,
    packet: &StoredQuota,
    completed: &str,
    now: &str,
) -> Result<(), ActivityError> {
    if !fresh(&packet.commit.observed_at_utc, completed, now) {
        return Ok(());
    }
    let prefs = load(connection).map_err(|_| ActivityError::ReadFailed)?;
    let identity: Option<String> = connection
        .query_row(
            "SELECT verified_identity_key FROM collection_runs WHERE id=?1",
            [&packet.commit.collection_run_id],
            |r| r.get(0),
        )
        .map_err(read_error)?;
    for assessment in &packet.reset_assessments {
        if assessment.state != QuotaResetState::Confirmed {
            continue;
        }
        let Some(generation) = &assessment.generation_run_id else {
            continue;
        };
        let limit = packet
            .limits
            .iter()
            .find(|l| l.limit_id == assessment.limit_id)
            .ok_or(ActivityError::InvalidInput)?;
        let window = match assessment.window_role {
            QuotaWindowRole::Primary => limit.primary.as_ref(),
            QuotaWindowRole::Secondary => limit.secondary.as_ref(),
        }
        .ok_or(ActivityError::InvalidInput)?;
        let Some(duration) = window.window_duration_mins else {
            continue;
        };
        // A historical/stale/disabled reading still establishes that a level was
        // reached. It cannot become a new crossing after a later decline.
        let previous_max:Option<u8> = connection.query_row("SELECT max(CASE ?1 WHEN 'primary' THEN l.primary_used_percent ELSE l.secondary_used_percent END) FROM quota_limits l JOIN quota_reset_assessments a ON a.collection_run_id=l.collection_run_id AND a.limit_id=l.limit_id AND a.window_role=?1 WHERE l.profile_id=?2 AND l.binding_id=?3 AND l.limit_id=?4 AND a.generation_run_id=?5 AND l.collection_run_id!=?6",params![assessment.window_role.as_str(),packet.binding.profile_id,packet.binding.binding_id,assessment.limit_id,generation,packet.commit.collection_run_id],|r|r.get(0)).map_err(read_error)?;
        let highest = [80u8, 90, 100]
            .into_iter()
            .filter(|level| {
                window.used_percent >= *level && previous_max.is_none_or(|old| old < *level)
            })
            .max();
        let Some(highest) = highest else {
            continue;
        };
        let first = generation == &packet.commit.collection_run_id;
        for threshold in [80u8, 90, 100].into_iter().filter(|level| {
            *level <= highest && (first || previous_max.is_none_or(|old| old < *level))
        }) {
            let key = event_key(
                packet,
                identity.as_deref(),
                &assessment.limit_id,
                assessment.window_role,
                duration,
                generation,
                threshold,
            );
            let window_key = crate::reset_notifications::confirmed_window_key(
                connection,
                "main",
                &key,
                generation,
                &assessment.limit_id,
                assessment.window_role.as_str(),
            )
            .map_err(read_error)?
            .ok_or(ActivityError::ReadFailed)?;
            let consumed:bool=connection.query_row("SELECT EXISTS(SELECT 1 FROM reset_notification_receipts WHERE event_key=?1 OR window_key=?2)",params![key,window_key],|r|r.get(0)).map_err(read_error)?;
            if consumed {
                continue;
            }
            let id = crate::activity::new_uuid()?;
            crate::operational_alerts::register_id(connection, &id).map_err(write_error)?;
            let inserted = connection.execute("INSERT INTO alert_events VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,'recorded') ON CONFLICT(event_key) DO NOTHING",params![id,key,packet.binding.profile_id,packet.binding.binding_id,identity,assessment.limit_id,assessment.window_role.as_str(),duration as i64,generation,packet.commit.observed_at_utc,packet.source_kind.as_str()]).map_err(write_error)?;
            if inserted == 0 {
                connection
                    .execute("DELETE FROM notification_event_ids WHERE id=?1", [&id])
                    .map_err(write_error)?;
            }
            if inserted == 1 {
                connection
                    .execute(
                        "INSERT INTO threshold_alert_events VALUES(?1,?2,?3,?4)",
                        params![id, threshold, packet.commit.collection_run_id, now],
                    )
                    .map_err(write_error)?;
                crate::reset_notifications::enqueue_threshold(
                    connection,
                    &id,
                    &packet.binding.profile_id,
                    packet.source_kind.as_str(),
                    enabled(&prefs, &packet.binding.profile_id) && (!first || threshold == highest),
                )?;
            }
            if first && threshold < highest {
                // Consumption is durable before a higher event can leave Rust.
                // The UUID identifies a consumption record, never an OS attempt.
                connection.execute("INSERT INTO reset_notification_receipts VALUES(?1,?2,'covered',?3,?3,?4) ON CONFLICT DO NOTHING",params![key,crate::activity::new_uuid()?,now,window_key]).map_err(write_error)?;
                connection.execute("UPDATE reset_notification_outbox SET state='suppressed' WHERE event_id=(SELECT id FROM alert_events WHERE event_key=?1) AND state='queued'",[key]).map_err(write_error)?;
            }
        }
    }
    Ok(())
}

pub(crate) fn validate(
    connection: &Connection,
    schema: &str,
) -> Result<(), crate::BackupValidationError> {
    use crate::backup_validation::{canonical_uuid, optional_text, require, sql_error, text};
    require(matches!(schema, "main" | "openusage_checkpoint"))?;
    let mut query = connection
        .prepare(&format!(
            "SELECT id,enabled FROM {schema}.threshold_alert_preferences"
        ))
        .map_err(sql_error)?;
    let mut rows = query.query([]).map_err(sql_error)?;
    if let Some(row) = rows.next().map_err(sql_error)? {
        require(
            row.get::<_, i64>(0).map_err(sql_error)? == 1
                && matches!(row.get::<_, i64>(1).map_err(sql_error)?, 0 | 1),
        )?;
    }
    require(rows.next().map_err(sql_error)?.is_none())?;
    let mut query = connection
        .prepare(&format!(
            "SELECT profile_id,enabled FROM {schema}.threshold_alert_overrides"
        ))
        .map_err(sql_error)?;
    let mut rows = query.query([]).map_err(sql_error)?;
    let mut count = 0;
    while let Some(row) = rows.next().map_err(sql_error)? {
        count += 1;
        require(
            count <= 100
                && canonical_uuid(text(row, 0)?)
                && matches!(row.get::<_, i64>(1).map_err(sql_error)?, 0 | 1),
        )?;
    }
    let mut query=connection.prepare(&format!("SELECT e.id,e.event_key,e.profile_id,e.binding_id,e.verified_identity_key,e.limit_id,e.window_role,e.window_duration_mins,e.generation_run_id,e.evidence_at_utc,e.source_kind,e.delivery_state,t.threshold_percent,t.evidence_run_id,t.accepted_at_utc,r.observed_at_utc,r.completed_at_utc,r.source_kind,r.verified_identity_key,a.state,a.generation_run_id,CASE e.window_role WHEN 'primary' THEN l.primary_used_percent ELSE l.secondary_used_percent END,CASE e.window_role WHEN 'primary' THEN l.primary_duration_mins ELSE l.secondary_duration_mins END,g.state,g.generation_run_id FROM {schema}.threshold_alert_events t JOIN {schema}.alert_events e ON e.id=t.event_id JOIN {schema}.collection_runs r ON r.id=t.evidence_run_id AND r.profile_id=e.profile_id AND r.binding_id=e.binding_id JOIN {schema}.quota_limits l ON l.collection_run_id=t.evidence_run_id AND l.profile_id=e.profile_id AND l.binding_id=e.binding_id AND l.limit_id=e.limit_id JOIN {schema}.quota_reset_assessments a ON a.collection_run_id=t.evidence_run_id AND a.limit_id=e.limit_id AND a.window_role=e.window_role JOIN {schema}.quota_reset_assessments g ON g.collection_run_id=e.generation_run_id AND g.limit_id=e.limit_id AND g.window_role=e.window_role")).map_err(sql_error)?;
    let mut rows = query.query([]).map_err(sql_error)?;
    let mut checked = 0i64;
    while let Some(row) = rows.next().map_err(sql_error)? {
        checked += 1;
        let run = text(row, 13)?;
        let packet = crate::quota::load_snapshot(connection, schema, run)
            .map_err(|_| crate::BackupValidationError::InvalidDatabase)?;
        let threshold: u8 = row.get(12).map_err(sql_error)?;
        let role = match text(row, 6)? {
            "primary" => QuotaWindowRole::Primary,
            "secondary" => QuotaWindowRole::Secondary,
            _ => return Err(crate::BackupValidationError::InvalidDatabase),
        };
        let raw_duration: i64 = row.get(7).map_err(sql_error)?;
        let duration = u64::try_from(raw_duration)
            .map_err(|_| crate::BackupValidationError::InvalidDatabase)?;
        let generation = text(row, 8)?;
        require(
            canonical_uuid(text(row, 0)?)
                && canonical_uuid(generation)
                && canonical_uuid(run)
                && text(row, 2)? == packet.binding.profile_id
                && text(row, 3)? == packet.binding.binding_id
                && matches!(threshold, 80 | 90 | 100)
                && text(row, 1)?
                    == event_key(
                        &packet,
                        optional_text(row, 4)?,
                        text(row, 5)?,
                        role,
                        duration,
                        generation,
                        threshold,
                    )
                && text(row, 9)? == text(row, 15)?
                && text(row, 10)? == text(row, 17)?
                && matches!(text(row, 10)?, "official_artifact" | "test_fixture")
                && optional_text(row, 4)? == optional_text(row, 18)?
                && text(row, 11)? == "recorded"
                && crate::activity::activity_utc_timestamp(text(row, 14)?)
                && fresh(text(row, 15)?, text(row, 16)?, text(row, 14)?)
                && text(row, 19)? == "confirmed"
                && text(row, 20)? == generation
                && row.get::<_, u8>(21).map_err(sql_error)? >= threshold
                && row.get::<_, i64>(22).map_err(sql_error)? == raw_duration
                && text(row, 23)? == "confirmed"
                && text(row, 24)? == generation,
        )?;
        let old_max:Option<u8> = connection.query_row(&format!("SELECT max(CASE ?1 WHEN 'primary' THEN l.primary_used_percent ELSE l.secondary_used_percent END) FROM {schema}.quota_limits l JOIN {schema}.quota_reset_assessments a ON a.collection_run_id=l.collection_run_id AND a.limit_id=l.limit_id AND a.window_role=?1 JOIN {schema}.collection_runs r ON r.id=l.collection_run_id WHERE l.profile_id=?2 AND l.binding_id=?3 AND l.limit_id=?4 AND a.generation_run_id=?5 AND r.observed_at_utc<?6"),params![role.as_str(),packet.binding.profile_id,packet.binding.binding_id,text(row,5)?,generation,packet.commit.observed_at_utc],|r|r.get(0)).map_err(sql_error)?;
        require(old_max.is_none_or(|old| old < threshold))?;
    }
    // Inner joins must not hide forged evidence links that remain FK-valid.
    let total: i64 = connection
        .query_row(
            &format!("SELECT count(*) FROM {schema}.threshold_alert_events"),
            [],
            |r| r.get(0),
        )
        .map_err(sql_error)?;
    require(total == checked)?;
    Ok(())
}
