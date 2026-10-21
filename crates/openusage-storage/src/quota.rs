//! Durable full quota readings and conservative local reset assessments. Identity stays provisional.
use crate::activity::{
    activity_decimal, activity_utc_timestamp, associate_in_transaction, read_error, require,
    require_profile, require_uuid, safe_text, validate_association, validate_evidence, write_error,
};
use crate::{
    ActivityError, ActivitySourceKind, ProfileBinding, ProfileBindingInput,
    ProfileIdentityEvidence, Storage,
};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use std::{collections::BTreeSet, fmt};
pub const QUOTA_SOURCE_METHOD: &str = "account/rateLimits/read";
pub const QUOTA_SOURCE_SCOPE: &str = "account_quota_with_unknown_application_scope";
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct QuotaWindow {
    pub used_percent: u8,
    pub window_duration_mins: Option<u64>,
    pub resets_at_unix: Option<i64>,
}
#[derive(Clone, PartialEq, Eq)]
pub struct QuotaLimit {
    pub limit_id: String,
    pub limit_name: Option<String>,
    pub primary: Option<QuotaWindow>,
    pub secondary: Option<QuotaWindow>,
    pub reached_type: Option<String>,
    pub monthly_credit_limit: Option<String>,
    pub spend_control_reached: Option<bool>,
}
impl fmt::Debug for QuotaLimit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("QuotaLimit { .. }")
    }
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum QuotaCreditStatus {
    Available,
    Redeeming,
    Redeemed,
    Unknown,
}
impl QuotaCreditStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Available => "available",
            Self::Redeeming => "redeeming",
            Self::Redeemed => "redeemed",
            Self::Unknown => "unknown",
        }
    }
}
#[derive(Clone, PartialEq, Eq)]
pub struct QuotaCreditDetail {
    pub title: Option<String>,
    pub status: QuotaCreditStatus,
    pub granted_at_unix: i64,
    pub expires_at_unix: Option<i64>,
}
impl fmt::Debug for QuotaCreditDetail {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("QuotaCreditDetail { .. }")
    }
}
#[derive(Clone)]
pub struct QuotaReading {
    pub run_id: String,
    pub binding: ProfileBinding,
    pub identity_evidence: ProfileIdentityEvidence,
    pub provider_version: String,
    pub source_kind: ActivitySourceKind,
    pub started_at_utc: String,
    pub observed_at_utc: String,
    pub completed_at_utc: String,
    pub limits: Vec<QuotaLimit>,
    pub available_reset_credits: Option<String>,
    pub reset_credit_details: Option<Vec<QuotaCreditDetail>>,
}
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct QuotaCommit {
    pub profile_id: String,
    pub binding_id: String,
    pub collection_run_id: String,
    pub observed_at_utc: String,
}
#[derive(Clone, PartialEq, Eq)]
pub struct StoredQuota {
    pub binding: ProfileBinding,
    pub commit: QuotaCommit,
    pub provider_version: String,
    pub source_kind: ActivitySourceKind,
    pub limits: Vec<QuotaLimit>,
    pub available_reset_credits: Option<String>,
    pub reset_credit_details: Option<Vec<QuotaCreditDetail>>,
    pub reset_assessments: Vec<crate::QuotaResetAssessment>,
}
impl fmt::Debug for StoredQuota {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("StoredQuota { .. }")
    }
}

type Result<T> = std::result::Result<T, ActivityError>;
impl Storage {
    /// The native coordinator must retain source, request, approval, generation and physical-home fences through COMMIT.
    pub fn accept_quota(
        &mut self,
        reading: QuotaReading,
        association: Option<ProfileBindingInput>,
    ) -> Result<QuotaCommit> {
        self.accept_quota_at(reading, association, None)
    }
    /// Accept a supervised live reading. Only a fresh observation can create a
    /// threshold event; historical ingestion through `accept_quota` cannot.
    pub fn accept_live_quota(
        &mut self,
        reading: QuotaReading,
        association: Option<ProfileBindingInput>,
        now_utc: &str,
    ) -> Result<QuotaCommit> {
        crate::threshold_alerts::validate_now(now_utc)?;
        self.accept_quota_at(reading, association, Some(now_utc))
    }
    fn accept_quota_at(
        &mut self,
        reading: QuotaReading,
        association: Option<ProfileBindingInput>,
        now_utc: Option<&str>,
    ) -> Result<QuotaCommit> {
        validate_reading(&reading)?;
        if let Some(input) = &association {
            validate_association(input)?;
            require(
                input.profile_id == reading.binding.profile_id
                    && input.expected_home_reference == reading.binding.home_reference
                    && input.new_binding_id == reading.binding.binding_id
                    && input.evidence == reading.identity_evidence
                    && input.observed_at_utc == reading.observed_at_utc,
            )?;
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(write_error)?;
        require_profile(
            &tx,
            &reading.binding.profile_id,
            &reading.binding.home_reference,
        )?;
        require_new(&tx, &reading)?;
        if let Some(input) = &association {
            associate_in_transaction(&tx, input)?;
        }
        let commit = accept_live_in_transaction(&tx, reading, now_utc)?;
        if let Some(now) = now_utc {
            crate::operational_alerts::accepted_success(&tx, &commit.collection_run_id, now)
                .map_err(|_| ActivityError::WriteFailed)?;
        }
        tx.commit().map_err(write_error)?;
        Ok(commit)
    }
    /// Cached reads require no provider approval and include archived profiles.
    pub fn current_quota(&self, profile_id: &str) -> Result<Option<StoredQuota>> {
        require_uuid(profile_id)?;
        let run: Option<String> = self
            .connection
            .query_row(
                "SELECT collection_run_id FROM current_quota_readings WHERE profile_id=?1",
                [profile_id],
                |r| r.get(0),
            )
            .optional()
            .map_err(read_error)?;
        run.map(|run| load_snapshot(&self.connection, "main", &run))
            .transpose()
    }
}
pub(crate) fn accept_live_in_transaction(
    tx: &Connection,
    reading: QuotaReading,
    now_utc: Option<&str>,
) -> Result<QuotaCommit> {
    require_profile(
        tx,
        &reading.binding.profile_id,
        &reading.binding.home_reference,
    )?;
    require_new(tx, &reading)?;
    let current:Option<(String,String,Option<String>,String)>=tx.query_row("SELECT b.binding_id,e.returned_email,e.returned_plan,e.home_reference FROM profile_bindings b JOIN profile_identity_events e ON e.id=b.binding_id AND e.profile_id=b.profile_id WHERE b.profile_id=?1",[&reading.binding.profile_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional().map_err(read_error)?;
    let Some((binding, email, plan, home)) = current else {
        return Err(ActivityError::BindingChanged);
    };
    if binding != reading.binding.binding_id || home != reading.binding.home_reference {
        return Err(ActivityError::BindingChanged);
    }
    if email != reading.identity_evidence.email || plan != reading.identity_evidence.plan {
        return Err(ActivityError::IdentityChanged);
    }
    let previous_run: Option<String> = tx
        .query_row(
            "SELECT collection_run_id FROM current_quota_readings WHERE profile_id=?1",
            [&reading.binding.profile_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(read_error)?;
    let previous = previous_run
        .as_ref()
        .map(|run| load_snapshot(tx, "main", run))
        .transpose()?;
    tx.execute("INSERT INTO collection_runs VALUES (?1,?2,?3,?4,?5,?6,'account/rateLimits/read',?7,?8,'account_quota_with_unknown_application_scope',NULL,'percent','unknown','provisional',NULL,'accepted')",params![reading.run_id,reading.binding.profile_id,reading.binding.binding_id,reading.started_at_utc,reading.completed_at_utc,reading.observed_at_utc,reading.provider_version,reading.source_kind.as_str()]).map_err(write_error)?;
    let detail_state = match &reading.reset_credit_details {
        None => "unavailable",
        Some(v) if v.is_empty() => "empty",
        Some(_) => "returned",
    };
    tx.execute(
        "INSERT INTO quota_snapshots VALUES (?1,?2,?3,?4,?5,?6)",
        params![
            reading.run_id,
            reading.binding.profile_id,
            reading.binding.binding_id,
            reading.available_reset_credits,
            detail_state,
            previous_run
        ],
    )
    .map_err(write_error)?;
    tx.execute(
        "DELETE FROM current_quota WHERE profile_id=?1",
        [&reading.binding.profile_id],
    )
    .map_err(write_error)?;
    tx.execute("INSERT INTO current_quota_readings VALUES (?1,?2,?3,?4,?4) ON CONFLICT(profile_id) DO UPDATE SET binding_id=excluded.binding_id,collection_run_id=excluded.collection_run_id,updated_at_utc=excluded.updated_at_utc",params![reading.binding.profile_id,reading.binding.binding_id,reading.run_id,reading.completed_at_utc]).map_err(write_error)?;
    for limit in &reading.limits {
        let (p, pd, pr) = window_columns(&limit.primary);
        let (s, sd, sr) = window_columns(&limit.secondary);
        tx.execute(
            "INSERT INTO quota_limits VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)",
            params![
                reading.run_id,
                reading.binding.profile_id,
                reading.binding.binding_id,
                limit.limit_id,
                limit.limit_name,
                limit.reached_type,
                limit.monthly_credit_limit,
                limit.spend_control_reached,
                p,
                pd,
                pr,
                s,
                sd,
                sr
            ],
        )
        .map_err(write_error)?;
        for (role, window) in [("primary", &limit.primary), ("secondary", &limit.secondary)] {
            if let Some(w) = window {
                tx.execute(
                    "INSERT INTO current_quota VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?9)",
                    params![
                        reading.binding.profile_id,
                        limit.limit_id,
                        role,
                        reading.binding.binding_id,
                        reading.run_id,
                        w.used_percent,
                        w.window_duration_mins.map(|v| v as i64),
                        w.resets_at_unix.map(unix_to_utc),
                        reading.completed_at_utc
                    ],
                )
                .map_err(write_error)?;
            }
        }
    }
    for (index, detail) in reading.reset_credit_details.iter().flatten().enumerate() {
        tx.execute(
            "INSERT INTO credit_snapshots VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
            params![
                reading.run_id,
                reading.binding.profile_id,
                reading.binding.binding_id,
                index as i64,
                detail.title,
                detail.status.as_str(),
                detail.granted_at_unix,
                detail.expires_at_unix
            ],
        )
        .map_err(write_error)?;
    }
    let packet = load_snapshot_packet(tx, "main", &reading.run_id)?;
    crate::quota_resets::save(tx, &packet, previous.as_ref())?;
    if let Some(now_utc) = now_utc {
        let mut packet = packet;
        packet.reset_assessments = crate::quota_resets::load(tx, "main", &reading.run_id)?;
        crate::threshold_alerts::record(tx, &packet, &reading.completed_at_utc, now_utc)?;
    }
    let commit = QuotaCommit {
        profile_id: reading.binding.profile_id,
        binding_id: reading.binding.binding_id,
        collection_run_id: reading.run_id,
        observed_at_utc: reading.observed_at_utc,
    };
    Ok(commit)
}

fn window_columns(w: &Option<QuotaWindow>) -> (Option<u8>, Option<i64>, Option<String>) {
    match w {
        Some(w) => (
            Some(w.used_percent),
            w.window_duration_mins.map(|v| v as i64),
            w.resets_at_unix.map(unix_to_utc),
        ),
        None => (None, None, None),
    }
}
pub(crate) fn require_new(tx: &Connection, r: &QuotaReading) -> Result<()> {
    let duplicate: bool = tx
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM collection_runs WHERE id=?1)",
            [&r.run_id],
            |v| v.get(0),
        )
        .map_err(read_error)?;
    if duplicate {
        return Err(ActivityError::AlreadyAccepted);
    }
    let latest:Option<String>=tx.query_row("SELECT max(r.observed_at_utc) FROM collection_runs r JOIN profile_bindings b ON b.profile_id=r.profile_id AND b.binding_id=r.binding_id WHERE b.profile_id=?1 AND r.source_method='account/rateLimits/read'",[&r.binding.profile_id],|v|v.get(0)).map_err(read_error)?;
    if latest.is_some_and(|v| v > r.observed_at_utc) {
        return Err(ActivityError::StaleObservation);
    }
    Ok(())
}
pub(crate) fn validate_packet(
    limits: &[QuotaLimit],
    count: &Option<String>,
    details: &Option<Vec<QuotaCreditDetail>>,
) -> Result<()> {
    require(limits.len() <= 64 && count.as_ref().is_none_or(|v| activity_decimal(v)))?;
    let mut seen = BTreeSet::new();
    for limit in limits {
        require(
            safe_text(&limit.limit_id, 128)
                && seen.insert(&limit.limit_id)
                && limit.limit_name.as_ref().is_none_or(|v| safe_text(v, 256))
                && limit
                    .reached_type
                    .as_ref()
                    .is_none_or(|v| safe_text(v, 128))
                && limit
                    .monthly_credit_limit
                    .as_ref()
                    .is_none_or(|v| safe_text(v, 256)),
        )?;
        for window in [&limit.primary, &limit.secondary].into_iter().flatten() {
            require(
                window.used_percent <= 100
                    && window
                        .window_duration_mins
                        .is_none_or(|v| (1..=9_007_199_254_740_991).contains(&v))
                    && window.resets_at_unix.is_none_or(valid_unix),
            )?;
        }
    }
    if let Some(details) = details {
        require(count.is_some() && details.len() <= 32)?;
        for detail in details {
            require(
                detail.title.as_ref().is_none_or(|v| safe_text(v, 256))
                    && valid_unix(detail.granted_at_unix)
                    && detail.expires_at_unix.is_none_or(valid_unix),
            )?;
        }
    }
    Ok(())
}
fn valid_unix(value: i64) -> bool {
    (-62_135_596_800..=253_402_300_799).contains(&value)
}

// Proleptic Gregorian calendar. The accepted Unix range covers UTC years 0001 through 9999.
pub(crate) fn unix_to_utc(value: i64) -> String {
    let days = value.div_euclid(86_400) + 719_468;
    let seconds = value.rem_euclid(86_400);
    let era = days.div_euclid(146_097);
    let day_of_era = days - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_part = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_part + 2) / 5 + 1;
    let month = month_part + if month_part < 10 { 3 } else { -9 };
    let year = year + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.000Z",
        seconds / 3_600,
        seconds % 3_600 / 60,
        seconds % 60
    )
}

pub(crate) fn utc_to_unix(value: &str) -> Result<i64> {
    require(activity_utc_timestamp(value) && value.ends_with(".000Z"))?;
    let number = |start, end| {
        value[start..end]
            .parse::<i64>()
            .map_err(|_| ActivityError::ReadFailed)
    };
    let year = number(0, 4)?;
    let month = number(5, 7)?;
    let day = number(8, 10)?;
    let year = year - i64::from(month <= 2);
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let month_part = month + if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * month_part + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    let days = era * 146_097 + day_of_era - 719_468;
    let value = days * 86_400 + number(11, 13)? * 3_600 + number(14, 16)? * 60 + number(17, 19)?;
    require(valid_unix(value))?;
    Ok(value)
}
pub(crate) fn validate_reading(r: &QuotaReading) -> Result<()> {
    for id in [
        &r.run_id,
        &r.binding.profile_id,
        &r.binding.binding_id,
        &r.binding.home_reference,
    ] {
        require_uuid(id)?;
    }
    validate_evidence(&r.identity_evidence)?;
    require(
        safe_text(&r.provider_version, 64)
            && activity_utc_timestamp(&r.started_at_utc)
            && activity_utc_timestamp(&r.observed_at_utc)
            && activity_utc_timestamp(&r.completed_at_utc)
            && r.started_at_utc <= r.observed_at_utc
            && r.observed_at_utc <= r.completed_at_utc,
    )?;
    validate_packet(
        &r.limits,
        &r.available_reset_credits,
        &r.reset_credit_details,
    )
}

pub(crate) fn load_snapshot(
    connection: &Connection,
    schema: &str,
    run: &str,
) -> Result<StoredQuota> {
    let mut packet = load_snapshot_packet(connection, schema, run)?;
    packet.reset_assessments = crate::quota_resets::load(connection, schema, run)?;
    Ok(packet)
}

pub(crate) fn load_snapshot_packet(
    connection: &Connection,
    schema: &str,
    run: &str,
) -> Result<StoredQuota> {
    require(matches!(schema, "main" | "openusage_checkpoint"))?;
    let (profile,binding,home,observed,version,source,count,state):(String,String,String,String,String,String,Option<String>,String)=connection.query_row(&format!("SELECT q.profile_id,q.binding_id,e.home_reference,r.observed_at_utc,r.provider_version,r.source_kind,q.available_reset_credits,q.credit_details_state FROM {schema}.quota_snapshots q JOIN {schema}.collection_runs r ON r.id=q.id AND r.profile_id=q.profile_id AND r.binding_id=q.binding_id JOIN {schema}.profile_identity_events e ON e.id=q.binding_id AND e.profile_id=q.profile_id WHERE q.id=?1"),[run],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?,r.get(7)?))).map_err(read_error)?;
    let source_kind = match source.as_str() {
        "official_artifact" => ActivitySourceKind::OfficialArtifact,
        "test_fixture" => ActivitySourceKind::TestFixture,
        _ => return Err(ActivityError::ReadFailed),
    };
    let mut statement=connection.prepare(&format!("SELECT limit_id,limit_name,reached_type,monthly_credit_limit,spend_control_reached,primary_used_percent,primary_duration_mins,primary_resets_at_utc,secondary_used_percent,secondary_duration_mins,secondary_resets_at_utc FROM {schema}.quota_limits WHERE collection_run_id=?1 ORDER BY limit_id LIMIT 65")).map_err(read_error)?;
    let limits = statement
        .query_map([run], |r| {
            let window = |offset| -> rusqlite::Result<Option<QuotaWindow>> {
                let used: Option<u8> = r.get(offset)?;
                used.map(|used_percent| -> rusqlite::Result<QuotaWindow> {
                    Ok(QuotaWindow {
                        used_percent,
                        window_duration_mins: r
                            .get::<_, Option<i64>>(offset + 1)?
                            .map(|value| {
                                u64::try_from(value).map_err(|_| {
                                    rusqlite::Error::IntegralValueOutOfRange(offset + 1, value)
                                })
                            })
                            .transpose()?,
                        resets_at_unix: r
                            .get::<_, Option<String>>(offset + 2)?
                            .map(|value| {
                                utc_to_unix(&value).map_err(|error| {
                                    rusqlite::Error::FromSqlConversionFailure(
                                        offset + 2,
                                        rusqlite::types::Type::Text,
                                        Box::new(error),
                                    )
                                })
                            })
                            .transpose()?,
                    })
                })
                .transpose()
            };
            Ok(QuotaLimit {
                limit_id: r.get(0)?,
                limit_name: r.get(1)?,
                reached_type: r.get(2)?,
                monthly_credit_limit: r.get(3)?,
                spend_control_reached: r.get(4)?,
                primary: window(5)?,
                secondary: window(8)?,
            })
        })
        .map_err(read_error)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(read_error)?;
    let mut statement=connection.prepare(&format!("SELECT ordinal,title,status,granted_at_unix,expires_at_unix FROM {schema}.credit_snapshots WHERE collection_run_id=?1 ORDER BY ordinal LIMIT 33")).map_err(read_error)?;
    let mut rows = statement.query([run]).map_err(read_error)?;
    let mut details = Vec::new();
    while let Some(row) = rows.next().map_err(read_error)? {
        require(row.get::<_, i64>(0).map_err(read_error)? == details.len() as i64)?;
        let status = match row.get::<_, String>(2).map_err(read_error)?.as_str() {
            "available" => QuotaCreditStatus::Available,
            "redeeming" => QuotaCreditStatus::Redeeming,
            "redeemed" => QuotaCreditStatus::Redeemed,
            "unknown" => QuotaCreditStatus::Unknown,
            _ => return Err(ActivityError::ReadFailed),
        };
        details.push(QuotaCreditDetail {
            title: row.get(1).map_err(read_error)?,
            status,
            granted_at_unix: row.get(3).map_err(read_error)?,
            expires_at_unix: row.get(4).map_err(read_error)?,
        });
    }
    let reset_credit_details = match state.as_str() {
        "unavailable" if details.is_empty() => None,
        "empty" if details.is_empty() => Some(details),
        "returned" if !details.is_empty() => Some(details),
        _ => return Err(ActivityError::ReadFailed),
    };
    validate_packet(&limits, &count, &reset_credit_details)?;
    Ok(StoredQuota {
        binding: ProfileBinding {
            profile_id: profile.clone(),
            binding_id: binding.clone(),
            home_reference: home,
        },
        commit: QuotaCommit {
            profile_id: profile,
            binding_id: binding,
            collection_run_id: run.into(),
            observed_at_utc: observed,
        },
        provider_version: version,
        source_kind,
        limits,
        available_reset_credits: count,
        reset_credit_details,
        reset_assessments: Vec::new(),
    })
}

#[cfg(test)]
#[path = "quota_tests.rs"]
mod tests;
