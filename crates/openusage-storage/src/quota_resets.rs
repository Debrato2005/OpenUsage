//! Reset proof belongs to an accepted full packet, never to a clock update.
use crate::activity::{read_error, require, require_uuid, write_error};
use crate::quota::{load_snapshot_packet, unix_to_utc, utc_to_unix};
use crate::{ActivityError, QuotaWindow, StoredQuota};
use rusqlite::{Connection, params};
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuotaWindowRole {
    Primary,
    Secondary,
}
impl QuotaWindowRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Primary => "primary",
            Self::Secondary => "secondary",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuotaResetState {
    NotConfirmed,
    Confirmed,
}
impl QuotaResetState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotConfirmed => "not_confirmed",
            Self::Confirmed => "confirmed",
        }
    }
}
#[derive(Clone, PartialEq, Eq)]
pub struct QuotaResetAssessment {
    pub limit_id: String,
    pub window_role: QuotaWindowRole,
    pub state: QuotaResetState,
    pub pending_reset_at_unix: Option<i64>,
    pub generation_run_id: Option<String>,
}
impl fmt::Debug for QuotaResetAssessment {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("QuotaResetAssessment { .. }")
    }
}
type Result<T> = std::result::Result<T, ActivityError>;

fn window<'a>(packet: &'a StoredQuota, id: &str, role: QuotaWindowRole) -> Option<&'a QuotaWindow> {
    let limit = packet.limits.iter().find(|limit| limit.limit_id == id)?;
    match role {
        QuotaWindowRole::Primary => limit.primary.as_ref(),
        QuotaWindowRole::Secondary => limit.secondary.as_ref(),
    }
}

fn derive(
    current: &StoredQuota,
    previous: Option<&StoredQuota>,
) -> Result<Vec<QuotaResetAssessment>> {
    let previous = previous.filter(|previous| previous.binding == current.binding);
    let mut assessments = Vec::new();
    for limit in &current.limits {
        for (role, next) in [
            (QuotaWindowRole::Primary, &limit.primary),
            (QuotaWindowRole::Secondary, &limit.secondary),
        ] {
            let Some(next) = next else {
                continue;
            };
            let comparable = previous.and_then(|packet| {
                let prior = window(packet, &limit.limit_id, role)?;
                (next.window_duration_mins.is_some()
                    && next.window_duration_mins == prior.window_duration_mins)
                    .then_some((packet, prior))
            });
            let mut assessment = QuotaResetAssessment {
                limit_id: limit.limit_id.clone(),
                window_role: role,
                state: QuotaResetState::NotConfirmed,
                pending_reset_at_unix: next.resets_at_unix,
                generation_run_id: None,
            };
            if let Some((packet, prior)) = comparable {
                let old = packet
                    .reset_assessments
                    .iter()
                    .find(|old| old.limit_id == limit.limit_id && old.window_role == role)
                    .ok_or(ActivityError::ReadFailed)?;
                let same_provenance = packet.provider_version == current.provider_version
                    && packet.source_kind == current.source_kind;
                let confirmed = same_provenance
                    && match (prior.resets_at_unix, next.resets_at_unix) {
                        (Some(before), Some(after)) => {
                            after > before
                                && current.commit.observed_at_utc >= unix_to_utc(before)
                                && next.used_percent < prior.used_percent
                        }
                        _ => false,
                    };
                if confirmed {
                    assessment.state = QuotaResetState::Confirmed;
                    assessment.generation_run_id = Some(current.commit.collection_run_id.clone());
                } else {
                    assessment.pending_reset_at_unix = old
                        .pending_reset_at_unix
                        .into_iter()
                        .chain(next.resets_at_unix)
                        .min();
                    assessment.generation_run_id = old.generation_run_id.clone();
                    if same_provenance
                        && next.resets_at_unix.is_some()
                        && next.resets_at_unix == prior.resets_at_unix
                        && old.state == QuotaResetState::Confirmed
                    {
                        assessment.state = QuotaResetState::Confirmed;
                    }
                }
            }
            assessments.push(assessment);
        }
    }
    assessments.sort_by(|a, b| {
        (&a.limit_id, a.window_role.as_str()).cmp(&(&b.limit_id, b.window_role.as_str()))
    });
    Ok(assessments)
}

pub(crate) fn save(
    connection: &Connection,
    packet: &StoredQuota,
    previous: Option<&StoredQuota>,
) -> Result<()> {
    for assessment in derive(packet, previous)? {
        connection
            .execute(
                "INSERT INTO quota_reset_assessments VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
                params![
                    packet.commit.collection_run_id,
                    packet.binding.profile_id,
                    packet.binding.binding_id,
                    assessment.limit_id,
                    assessment.window_role.as_str(),
                    assessment.state.as_str(),
                    assessment.pending_reset_at_unix.map(unix_to_utc),
                    assessment.generation_run_id,
                ],
            )
            .map_err(write_error)?;
    }
    Ok(())
}

pub(crate) fn load(
    connection: &Connection,
    schema: &str,
    run: &str,
) -> Result<Vec<QuotaResetAssessment>> {
    require(matches!(schema, "main" | "openusage_checkpoint"))?;
    let mut statement = connection.prepare(&format!("SELECT limit_id,window_role,state,pending_reset_at_utc,generation_run_id FROM {schema}.quota_reset_assessments WHERE collection_run_id=?1 ORDER BY limit_id,window_role LIMIT 129")).map_err(read_error)?;
    let mut rows = statement.query([run]).map_err(read_error)?;
    let mut assessments = Vec::new();
    while let Some(row) = rows.next().map_err(read_error)? {
        let window_role = match row.get::<_, String>(1).map_err(read_error)?.as_str() {
            "primary" => QuotaWindowRole::Primary,
            "secondary" => QuotaWindowRole::Secondary,
            _ => return Err(ActivityError::ReadFailed),
        };
        let state = match row.get::<_, String>(2).map_err(read_error)?.as_str() {
            "not_confirmed" => QuotaResetState::NotConfirmed,
            "confirmed" => QuotaResetState::Confirmed,
            _ => return Err(ActivityError::ReadFailed),
        };
        let pending: Option<String> = row.get(3).map_err(read_error)?;
        let generation_run_id: Option<String> = row.get(4).map_err(read_error)?;
        if let Some(id) = &generation_run_id {
            require_uuid(id)?;
        }
        assessments.push(QuotaResetAssessment {
            limit_id: row.get(0).map_err(read_error)?,
            window_role,
            state,
            pending_reset_at_unix: pending.as_deref().map(utc_to_unix).transpose()?,
            generation_run_id,
        });
    }
    require(assessments.len() <= 128)?;
    Ok(assessments)
}

pub(crate) fn validate(connection: &Connection, schema: &str) -> Result<()> {
    require(matches!(schema, "main" | "openusage_checkpoint"))?;
    // One successor per predecessor comes from the unique index. Require roots and ordered links.
    let invalid: bool = connection.query_row(&format!("WITH RECURSIVE rooted(id) AS (
        SELECT id FROM {schema}.quota_snapshots WHERE previous_quota_run_id IS NULL
        UNION SELECT q.id FROM {schema}.quota_snapshots q JOIN rooted p ON q.previous_quota_run_id=p.id
    ) SELECT EXISTS(SELECT 1 FROM {schema}.quota_snapshots q WHERE NOT EXISTS(SELECT 1 FROM rooted r WHERE r.id=q.id))
    OR EXISTS(SELECT 1 FROM {schema}.quota_snapshots q JOIN {schema}.quota_snapshots p ON p.id=q.previous_quota_run_id JOIN {schema}.collection_runs n ON n.id=q.id JOIN {schema}.collection_runs o ON o.id=p.id WHERE q.profile_id!=p.profile_id OR q.binding_id!=p.binding_id OR n.observed_at_utc<o.observed_at_utc
        OR EXISTS(SELECT 1 FROM {schema}.quota_snapshots skipped JOIN {schema}.collection_runs s ON s.id=skipped.id WHERE skipped.profile_id=q.profile_id AND skipped.binding_id=q.binding_id AND s.observed_at_utc>o.observed_at_utc AND s.observed_at_utc<n.observed_at_utc))
    OR EXISTS(SELECT 1 FROM {schema}.current_quota_readings c JOIN {schema}.quota_snapshots n ON n.previous_quota_run_id=c.collection_run_id)"), [], |row| row.get(0)).map_err(read_error)?;
    require(!invalid)?;
    let mut statement = connection
        .prepare(&format!(
            "SELECT id,previous_quota_run_id FROM {schema}.quota_snapshots"
        ))
        .map_err(read_error)?;
    let mut rows = statement.query([]).map_err(read_error)?;
    while let Some(row) = rows.next().map_err(read_error)? {
        let run: String = row.get(0).map_err(read_error)?;
        let prior: Option<String> = row.get(1).map_err(read_error)?;
        let current = load_snapshot_packet(connection, schema, &run)?;
        let previous = prior
            .map(|run| -> Result<StoredQuota> {
                let mut packet = load_snapshot_packet(connection, schema, &run)?;
                packet.reset_assessments = load(connection, schema, &run)?;
                Ok(packet)
            })
            .transpose()?;
        require(derive(&current, previous.as_ref())? == load(connection, schema, &run)?)?;
    }
    Ok(())
}
