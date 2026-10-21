//! Durable history of accepted reset proofs. Recording does not deliver a notification.
use crate::activity::{read_error, require, write_error};
use crate::{
    ActivityError, ActivitySourceKind, QuotaResetAssessment, QuotaResetState, QuotaWindowRole,
    Storage, StorageError, StoredQuota,
};
use rusqlite::{Connection, params};
use std::{fmt, fmt::Write};

pub(crate) const ALERT_COLUMNS: &str = "id,event_key,profile_id,binding_id,verified_identity_key,limit_id,window_role,window_duration_mins,generation_run_id,evidence_at_utc,source_kind,delivery_state";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlertDeliveryState {
    Recorded,
    Queued,
    Uncertain,
    Submitted,
    Failed,
    Suppressed,
}
impl AlertDeliveryState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Recorded => "recorded",
            Self::Queued => "queued",
            Self::Uncertain => "uncertain",
            Self::Submitted => "submitted",
            Self::Failed => "failed",
            Self::Suppressed => "suppressed",
        }
    }
}
#[derive(Clone, PartialEq, Eq)]
pub struct AlertEvent {
    pub id: String,
    pub profile_id: String,
    pub profile_label: String,
    pub limit_id: String,
    pub window_role: QuotaWindowRole,
    pub window_duration_mins: u64,
    pub generation_run_id: String,
    pub evidence_at_utc: String,
    pub source_kind: ActivitySourceKind,
    pub delivery_state: AlertDeliveryState,
}
impl fmt::Debug for AlertEvent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("AlertEvent { .. }")
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AlertHistory {
    pub events: Vec<AlertEvent>,
    pub truncated: bool,
}

impl Storage {
    /// Return the newest 100 events, including archived profiles, without provider execution.
    pub fn alert_history(&self) -> Result<AlertHistory, StorageError> {
        let mut statement = self.connection.prepare("SELECT e.id,e.profile_id,p.name,e.limit_id,e.window_role,e.window_duration_mins,e.generation_run_id,e.evidence_at_utc,e.source_kind,COALESCE(o.state,e.delivery_state) FROM alert_events e JOIN profiles p ON p.id=e.profile_id LEFT JOIN reset_notification_outbox o ON o.event_id=e.id ORDER BY e.evidence_at_utc DESC,e.id DESC LIMIT 101").map_err(|_|StorageError::ReadFailed)?;
        let mut rows = statement.query([]).map_err(|_| StorageError::ReadFailed)?;
        let mut events = Vec::new();
        while let Some(row) = rows.next().map_err(|_| StorageError::ReadFailed)? {
            let event = (|| -> rusqlite::Result<AlertEvent> {
                Ok(AlertEvent {
                    id: row.get(0)?,
                    profile_id: row.get(1)?,
                    profile_label: row.get(2)?,
                    limit_id: row.get(3)?,
                    window_role: match row.get::<_, String>(4)?.as_str() {
                        "primary" => QuotaWindowRole::Primary,
                        "secondary" => QuotaWindowRole::Secondary,
                        _ => return Err(rusqlite::Error::InvalidQuery),
                    },
                    window_duration_mins: u64::try_from(row.get::<_, i64>(5)?)
                        .map_err(|_| rusqlite::Error::InvalidQuery)?,
                    generation_run_id: row.get(6)?,
                    evidence_at_utc: row.get(7)?,
                    source_kind: match row.get::<_, String>(8)?.as_str() {
                        "official_artifact" => ActivitySourceKind::OfficialArtifact,
                        "test_fixture" => ActivitySourceKind::TestFixture,
                        _ => return Err(rusqlite::Error::InvalidQuery),
                    },
                    delivery_state: crate::reset_notifications::delivery_state(
                        &row.get::<_, String>(9)?,
                    )
                    .ok_or(rusqlite::Error::InvalidQuery)?,
                })
            })()
            .map_err(|_| StorageError::ReadFailed)?;
            if !crate::backup_validation::canonical_uuid(&event.id)
                || !crate::backup_validation::canonical_uuid(&event.profile_id)
                || !crate::backup_validation::canonical_uuid(&event.generation_run_id)
                || !crate::activity::activity_utc_timestamp(&event.evidence_at_utc)
                || !crate::activity::safe_text(&event.limit_id, 128)
                || !(1..=9_007_199_254_740_991).contains(&event.window_duration_mins)
            {
                return Err(StorageError::ReadFailed);
            }
            events.push(event);
        }
        let truncated = events.len() > 100;
        events.truncate(100);
        Ok(AlertHistory { events, truncated })
    }
}

fn event_key(
    profile: &str,
    identity: Option<&str>,
    binding: &str,
    limit: &str,
    role: &str,
    duration: u64,
    generation: &str,
) -> String {
    let duration = duration.to_string();
    let mut key = String::from("reset_completed:1");
    // Length prefixes keep provider IDs containing separators distinct.
    for value in [
        profile,
        if identity.is_some() {
            "verified"
        } else {
            "unverified"
        },
        identity.unwrap_or(""),
        binding,
        limit,
        role,
        duration.as_str(),
        generation,
    ] {
        write!(key, ":{}:{value}", value.len()).expect("writing to String cannot fail");
    }
    key
}

pub(crate) fn record(
    connection: &Connection,
    packet: &StoredQuota,
    assessments: &[QuotaResetAssessment],
) -> Result<(), ActivityError> {
    for assessment in assessments {
        if assessment.state != QuotaResetState::Confirmed
            || assessment.generation_run_id.as_deref()
                != Some(packet.commit.collection_run_id.as_str())
        {
            continue;
        }
        let eligible: bool = connection.query_row("SELECT EXISTS(SELECT 1 FROM profiles p JOIN profile_bindings b ON b.profile_id=p.id JOIN current_quota_readings c ON c.profile_id=p.id WHERE p.id=?1 AND p.archived=0 AND p.approved_home_reference=?2 AND b.binding_id=?3 AND c.binding_id=?3 AND c.collection_run_id=?4)",params![packet.binding.profile_id,packet.binding.home_reference,packet.binding.binding_id,packet.commit.collection_run_id],|r|r.get(0)).map_err(read_error)?;
        require(eligible)?;
        let identity: Option<String> = connection
            .query_row(
                "SELECT verified_identity_key FROM collection_runs WHERE id=?1",
                [&packet.commit.collection_run_id],
                |row| row.get(0),
            )
            .map_err(read_error)?;
        let limit = packet
            .limits
            .iter()
            .find(|l| l.limit_id == assessment.limit_id)
            .ok_or(ActivityError::InvalidInput)?;
        let window = match assessment.window_role {
            QuotaWindowRole::Primary => &limit.primary,
            QuotaWindowRole::Secondary => &limit.secondary,
        };
        let duration = window
            .as_ref()
            .and_then(|w| w.window_duration_mins)
            .ok_or(ActivityError::InvalidInput)?;
        let id = crate::activity::new_uuid()?;
        let key = event_key(
            &packet.binding.profile_id,
            identity.as_deref(),
            &packet.binding.binding_id,
            &assessment.limit_id,
            assessment.window_role.as_str(),
            duration,
            &packet.commit.collection_run_id,
        );
        let inserted=connection.execute("INSERT INTO alert_events VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,'recorded') ON CONFLICT(event_key) DO NOTHING",params![id,key,packet.binding.profile_id,packet.binding.binding_id,identity,assessment.limit_id,assessment.window_role.as_str(),duration as i64,packet.commit.collection_run_id,packet.commit.observed_at_utc,packet.source_kind.as_str()]).map_err(write_error)?;
        if inserted == 1 {
            crate::reset_notifications::enqueue(
                connection,
                &id,
                &packet.binding.profile_id,
                packet.source_kind.as_str(),
            )?;
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
    let mut statement=connection.prepare(&format!("SELECT e.id,e.event_key,e.profile_id,e.binding_id,e.verified_identity_key,e.limit_id,e.window_role,e.window_duration_mins,e.generation_run_id,e.evidence_at_utc,e.source_kind,e.delivery_state,a.state,a.generation_run_id,r.observed_at_utc,r.source_kind,r.verified_identity_key,CASE e.window_role WHEN 'primary' THEN l.primary_duration_mins ELSE l.secondary_duration_mins END FROM {schema}.alert_events e JOIN {schema}.quota_reset_assessments a ON a.collection_run_id=e.generation_run_id AND a.limit_id=e.limit_id AND a.window_role=e.window_role JOIN {schema}.quota_limits l ON l.collection_run_id=e.generation_run_id AND l.profile_id=e.profile_id AND l.binding_id=e.binding_id AND l.limit_id=e.limit_id JOIN {schema}.collection_runs r ON r.id=e.generation_run_id")).map_err(sql_error)?;
    let mut rows = statement.query([]).map_err(sql_error)?;
    while let Some(row) = rows.next().map_err(sql_error)? {
        let profile = text(row, 2)?;
        let binding = text(row, 3)?;
        let identity = optional_text(row, 4)?;
        let limit = text(row, 5)?;
        let role = text(row, 6)?;
        let duration: i64 = row.get(7).map_err(sql_error)?;
        let generation = text(row, 8)?;
        require(
            canonical_uuid(text(row, 0)?)
                && canonical_uuid(profile)
                && canonical_uuid(binding)
                && canonical_uuid(generation)
                && matches!(role, "primary" | "secondary")
                && (1..=9_007_199_254_740_991).contains(&duration)
                && text(row, 1)?
                    == event_key(
                        profile,
                        identity,
                        binding,
                        limit,
                        role,
                        duration as u64,
                        generation,
                    )
                && text(row, 9)? == text(row, 14)?
                && text(row, 10)? == text(row, 15)?
                && matches!(text(row, 10)?, "official_artifact" | "test_fixture")
                && identity == optional_text(row, 16)?
                && text(row, 11)? == "recorded"
                && text(row, 12)? == "confirmed"
                && text(row, 13)? == generation
                && duration == row.get::<_, i64>(17).map_err(sql_error)?,
        )?;
    }
    Ok(())
}
