//! Selected current readings, without attribution or aggregation across provisional profiles.
use crate::{Storage, activity::require_uuid, quota::load_snapshot};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};
use zeroize::Zeroizing;

const MAX_ROWS: usize = 50_000;
const COLUMNS: &[&str] = &[
    "profile",
    "profile_id",
    "profile_label",
    "email",
    "record_kind",
    "metric",
    "value",
    "unit",
    "provider_date",
    "observed_at_utc",
    "source_method",
    "source_kind",
    "provider_version",
    "source_scope",
    "raw_scope",
    "identity_confidence",
    "coverage",
    "status",
    "inclusion_rule",
    "limit_id",
    "window_role",
];
const HISTORY_COLUMNS: &[&str] = &[
    "profile",
    "profile_id",
    "profile_label",
    "email",
    "record_kind",
    "metric",
    "value",
    "unit",
    "provider_date",
    "observed_at_utc",
    "source_method",
    "source_kind",
    "provider_version",
    "source_scope",
    "raw_scope",
    "identity_confidence",
    "coverage",
    "status",
    "inclusion_rule",
    "limit_id",
    "window_role",
    "collection_run_id",
    "binding_id",
];

pub struct ReadingsExport {
    pub columns: &'static [&'static str],
    pub rows: Zeroizing<Vec<Vec<Option<String>>>>,
    pub profile_count: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExportError {
    InvalidSelection,
    ReadFailed,
    TooLarge,
}
impl fmt::Display for ExportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidSelection => "Select 1 to 10 different active profiles.",
            Self::ReadFailed => "The saved readings could not be read.",
            Self::TooLarge => "The selected readings exceed the 50,000-row export limit.",
        })
    }
}
impl std::error::Error for ExportError {}
fn read_error(_: rusqlite::Error) -> ExportError {
    ExportError::ReadFailed
}

struct Metric<'a> {
    kind: &'a str,
    name: &'a str,
    value: Option<String>,
    unit: &'a str,
    date: Option<&'a str>,
    limit: Option<&'a str>,
    window: Option<&'a str>,
}
fn metric<'a>(kind: &'a str, name: &'a str, value: Option<String>, unit: &'a str) -> Metric<'a> {
    Metric {
        kind,
        name,
        value,
        unit,
        date: None,
        limit: None,
        window: None,
    }
}
fn emit(
    rows: &mut Zeroizing<Vec<Vec<Option<String>>>>,
    base: &[Option<String>],
    metric: Metric<'_>,
) -> Result<(), ExportError> {
    if rows.len() == MAX_ROWS {
        return Err(ExportError::TooLarge);
    }
    let mut row = base.to_vec();
    row[4] = Some(metric.kind.into());
    row[5] = Some(metric.name.into());
    row[17] = Some(
        if metric.value.is_some() {
            "available"
        } else {
            "unavailable"
        }
        .into(),
    );
    row[6] = metric.value;
    row[7] = Some(metric.unit.into());
    row[8] = metric.date.map(str::to_owned);
    row[19] = metric.limit.map(str::to_owned);
    row[20] = metric.window.map(str::to_owned);
    rows.push(row);
    Ok(())
}

fn context(
    connection: &Connection,
    id: &str,
    alias: &str,
    label: Option<&str>,
    run: Option<&str>,
    identifiers: bool,
) -> Result<Zeroizing<Vec<Option<String>>>, ExportError> {
    let mut base = Zeroizing::new(vec![None; COLUMNS.len()]);
    base[0] = Some(alias.into());
    if identifiers {
        base[1] = Some(id.into());
        base[2] = label.map(str::to_owned);
    }
    base[18] = Some("separate_profile_no_aggregation".into());
    if let Some(run) = run {
        connection.query_row(
            "SELECT r.observed_at_utc,r.source_method,r.source_kind,r.provider_version,r.source_scope,r.raw_scope,r.identity_confidence,r.coverage,e.returned_email FROM collection_runs r JOIN profile_identity_events e ON e.id=r.binding_id AND e.profile_id=r.profile_id WHERE r.id=?1 AND r.profile_id=?2",
            [run,id], |row| {
                for (column,index) in [(9,0),(10,1),(11,2),(12,3),(13,4),(15,6),(16,7)] {
                    base[column] = Some(row.get::<_,String>(index)?);
                }
                base[14] = row.get(5)?;
                if identifiers { base[3] = Some(row.get::<_,String>(8)?); }
                Ok(())
            },
        ).map_err(read_error)?;
    }
    Ok(base)
}

impl Storage {
    /// Export every accepted quota run for one local profile, including prior associations
    /// and archived profiles. One deferred transaction pins labels, runs and packets.
    /// References are stable aliases unless local identifiers are explicitly requested.
    /// No history cursor or current-reading pointer limits the selection.
    pub fn quota_history_export(
        &mut self,
        profile_id: &str,
        include_identifiers: bool,
    ) -> Result<ReadingsExport, ExportError> {
        require_uuid(profile_id).map_err(|_| ExportError::InvalidSelection)?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(read_error)?;
        let label = tx
            .query_row(
                "SELECT CASE WHEN ?2 THEN name ELSE NULL END FROM profiles WHERE id=?1",
                rusqlite::params![profile_id, include_identifiers],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()
            .map_err(read_error)?
            .ok_or(ExportError::InvalidSelection)?;
        let mut rows = Zeroizing::new(Vec::new());
        {
            let mut bindings = BTreeMap::new();
            // Every snapshot emits at least three rows, so this lookahead cannot hide
            // an oversized result: emit() fails before this run bound is reached.
            let mut statement = tx.prepare("SELECT r.id,r.binding_id,r.observed_at_utc,r.source_method,r.source_kind,r.provider_version,r.source_scope,r.raw_scope,r.identity_confidence,r.coverage FROM collection_runs r WHERE r.profile_id=?1 AND r.source_method=?2 AND r.outcome='accepted' ORDER BY r.observed_at_utc,r.id LIMIT 50001").map_err(read_error)?;
            let mut runs = statement
                .query(rusqlite::params![profile_id, crate::QUOTA_SOURCE_METHOD])
                .map_err(read_error)?;
            let mut ordinal = 0;
            while let Some(run) = runs.next().map_err(read_error)? {
                let run_id: String = run.get(0).map_err(read_error)?;
                let binding_id: String = run.get(1).map_err(read_error)?;
                ordinal += 1;
                let next_binding = bindings.len() + 1;
                let binding_alias = bindings
                    .entry(binding_id.clone())
                    .or_insert_with(|| format!("association-{next_binding}"));
                let mut base = Zeroizing::new(vec![None; HISTORY_COLUMNS.len()]);
                base[0] = Some("profile-1".into());
                base[18] = Some("separate_profile_no_aggregation".into());
                for (column, index) in
                    [(9, 2), (10, 3), (11, 4), (12, 5), (13, 6), (15, 8), (16, 9)]
                {
                    base[column] = Some(run.get::<_, String>(index).map_err(read_error)?);
                }
                base[14] = run.get(7).map_err(read_error)?;
                if include_identifiers {
                    base[1] = Some(profile_id.into());
                    base[2] = label.clone();
                    base[21] = Some(run_id.clone());
                    base[22] = Some(binding_id);
                } else {
                    base[21] = Some(format!("reading-{ordinal}"));
                    base[22] = Some(binding_alias.clone());
                }
                export_quota(&tx, &run_id, &base, &mut rows)?;
            }
        }
        tx.commit().map_err(read_error)?;
        Ok(ReadingsExport {
            columns: HISTORY_COLUMNS,
            rows,
            profile_count: 1,
        })
    }

    /// The deferred transaction pins profile selection and every capability to one WAL snapshot.
    pub fn readings_export(
        &mut self,
        profile_ids: &[String],
        include_identifiers: bool,
    ) -> Result<ReadingsExport, ExportError> {
        if !(1..=10).contains(&profile_ids.len()) {
            return Err(ExportError::InvalidSelection);
        }
        let mut seen = BTreeSet::new();
        for id in profile_ids {
            if require_uuid(id).is_err() || !seen.insert(id) {
                return Err(ExportError::InvalidSelection);
            }
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(read_error)?;
        // Validate the complete selection before returning any row. Even labels are opt-in.
        let mut profiles = Zeroizing::new(Vec::new());
        for id in profile_ids {
            let label = tx.query_row("SELECT CASE WHEN ?2 THEN name ELSE NULL END FROM profiles WHERE id=?1 AND archived=0", rusqlite::params![id,include_identifiers], |row| row.get::<_,Option<String>>(0)).optional().map_err(read_error)?.ok_or(ExportError::InvalidSelection)?;
            profiles.push(label);
        }
        let mut rows = Zeroizing::new(Vec::new());
        for (index, id) in profile_ids.iter().enumerate() {
            let alias = format!("profile-{}", index + 1);
            let label = profiles[index].as_deref();
            let quota: Option<String> = tx
                .query_row(
                    "SELECT collection_run_id FROM current_quota_readings WHERE profile_id=?1",
                    [id],
                    |row| row.get(0),
                )
                .optional()
                .map_err(read_error)?;
            let base = context(
                &tx,
                id,
                &alias,
                label,
                quota.as_deref(),
                include_identifiers,
            )?;
            if let Some(run) = quota {
                export_quota(&tx, &run, &base, &mut rows)?;
            } else {
                emit(
                    &mut rows,
                    &base,
                    metric("quota_summary", "reading", None, "provider_value"),
                )?;
            }
            let activity: Option<String> = tx
                .query_row(
                    "SELECT collection_run_id FROM current_activity WHERE profile_id=?1",
                    [id],
                    |row| row.get::<_, Option<String>>(0),
                )
                .optional()
                .map_err(read_error)?
                .flatten();
            let base = context(
                &tx,
                id,
                &alias,
                label,
                activity.as_deref(),
                include_identifiers,
            )?;
            export_activity(&tx, activity.as_deref(), &base, &mut rows)?;
            // Current dates can outlive the latest summary. Retain each date's own provenance.
            let mut statement = tx.prepare("SELECT d.provider_date,d.tokens,d.collection_run_id FROM current_activity_days d JOIN profile_bindings b ON b.profile_id=d.profile_id AND b.binding_id=d.binding_id WHERE d.profile_id=?1 ORDER BY d.provider_date,d.source_scope LIMIT 50001").map_err(read_error)?;
            let mut days = statement.query([id]).map_err(read_error)?;
            while let Some(day) = days.next().map_err(read_error)? {
                let date: String = day.get(0).map_err(read_error)?;
                let value: String = day.get(1).map_err(read_error)?;
                let run: String = day.get(2).map_err(read_error)?;
                let base = context(&tx, id, &alias, label, Some(&run), include_identifiers)?;
                let mut entry = metric("activity_daily", "tokens", Some(value), "tokens");
                entry.date = Some(&date);
                emit(&mut rows, &base, entry)?;
            }
        }
        tx.commit().map_err(read_error)?;
        Ok(ReadingsExport {
            columns: COLUMNS,
            rows,
            profile_count: profile_ids.len() as u32,
        })
    }
}

fn export_activity(
    connection: &Connection,
    run: Option<&str>,
    base: &[Option<String>],
    rows: &mut Zeroizing<Vec<Vec<Option<String>>>>,
) -> Result<(), ExportError> {
    let values = if let Some(run) = run {
        connection.query_row("SELECT lifetime_tokens,peak_daily_tokens,longest_running_turn_sec,current_streak_days,longest_streak_days,daily_state FROM activity_summaries WHERE id=?1",[run],|row|{
            let mut values = Vec::with_capacity(6);
            for i in 0..6 { values.push(row.get::<_,Option<String>>(i)?); }
            Ok(Zeroizing::new(values))
        }).map_err(read_error)?
    } else {
        Zeroizing::new(vec![None; 6])
    };
    for (index, (name, unit)) in [
        ("lifetime_tokens", "tokens"),
        ("peak_daily_tokens", "tokens"),
        ("longest_running_turn_sec", "seconds"),
        ("current_streak_days", "days"),
        ("longest_streak_days", "days"),
        ("daily_buckets_state", "text"),
    ]
    .iter()
    .enumerate()
    {
        emit(
            rows,
            base,
            metric("activity_summary", name, values[index].clone(), unit),
        )?;
    }
    Ok(())
}

fn export_quota(
    connection: &Connection,
    run: &str,
    base: &[Option<String>],
    rows: &mut Zeroizing<Vec<Vec<Option<String>>>>,
) -> Result<(), ExportError> {
    let saved = load_snapshot(connection, "main", run).map_err(|_| ExportError::ReadFailed)?;
    emit(
        rows,
        base,
        metric(
            "quota_summary",
            "available_reset_credits",
            saved.available_reset_credits,
            "credits",
        ),
    )?;
    let state = match &saved.reset_credit_details {
        None => "unavailable",
        Some(values) if values.is_empty() => "empty",
        Some(_) => "returned",
    };
    emit(
        rows,
        base,
        metric(
            "quota_summary",
            "credit_details_state",
            Some(state.into()),
            "text",
        ),
    )?;
    emit(
        rows,
        base,
        metric(
            "quota_summary",
            "quota_limits_state",
            Some(
                if saved.limits.is_empty() {
                    "empty"
                } else {
                    "returned"
                }
                .into(),
            ),
            "text",
        ),
    )?;
    for limit in &saved.limits {
        for (name, value, unit) in [
            ("limit_name", limit.limit_name.clone(), "text"),
            ("reached_type", limit.reached_type.clone(), "text"),
            (
                "monthly_credit_limit",
                limit.monthly_credit_limit.clone(),
                "provider_value",
            ),
            (
                "spend_control_reached",
                limit.spend_control_reached.map(|v| u8::from(v).to_string()),
                "boolean",
            ),
        ] {
            let mut entry = metric("quota_limit", name, value, unit);
            entry.limit = Some(&limit.limit_id);
            emit(rows, base, entry)?;
        }
        for (role, window) in [("primary", &limit.primary), ("secondary", &limit.secondary)] {
            let assessment = saved
                .reset_assessments
                .iter()
                .find(|a| a.limit_id == limit.limit_id && a.window_role.as_str() == role);
            for (name, value, unit) in [
                (
                    "used_percent",
                    window.as_ref().map(|w| w.used_percent.to_string()),
                    "percent",
                ),
                (
                    "window_duration_mins",
                    window
                        .as_ref()
                        .and_then(|w| w.window_duration_mins)
                        .map(|v| v.to_string()),
                    "minutes",
                ),
                (
                    "resets_at_unix",
                    window
                        .as_ref()
                        .and_then(|w| w.resets_at_unix)
                        .map(|v| v.to_string()),
                    "unix_seconds",
                ),
                (
                    "reset_confirmation_state",
                    assessment.map(|a| a.state.as_str().into()),
                    "text",
                ),
                (
                    "pending_reset_at_unix",
                    assessment
                        .and_then(|a| a.pending_reset_at_unix)
                        .map(|v| v.to_string()),
                    "unix_seconds",
                ),
            ] {
                let mut entry = metric("quota_window", name, value, unit);
                entry.limit = Some(&limit.limit_id);
                entry.window = Some(role);
                emit(rows, base, entry)?;
            }
        }
    }
    for (ordinal, credit) in saved.reset_credit_details.iter().flatten().enumerate() {
        for (field, value, unit) in [
            ("title", credit.title.clone(), "text"),
            ("status", Some(credit.status.as_str().into()), "text"),
            (
                "granted_at_unix",
                Some(credit.granted_at_unix.to_string()),
                "unix_seconds",
            ),
            (
                "expires_at_unix",
                credit.expires_at_unix.map(|v| v.to_string()),
                "unix_seconds",
            ),
        ] {
            emit(
                rows,
                base,
                metric(
                    "quota_credit",
                    &format!("reset_credit_{}_{field}", ordinal + 1),
                    value,
                    unit,
                ),
            )?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ActivityDayValue, ActivityReading, ActivitySourceKind, ActivitySummaryValues,
        ProfileBinding, ProfileBindingInput, ProfileIdentityEvidence, QuotaCreditDetail,
        QuotaCreditStatus, QuotaLimit, QuotaReading, QuotaWindow,
    };
    const KEY: [u8; 32] = [0x73; 32];
    fn id(n: u32) -> String {
        format!("{n:08x}-0000-4000-8000-000000000000")
    }
    fn time(second: u32) -> String {
        format!("2026-10-20T12:00:{second:02}.000Z")
    }
    fn pair(n: u32, second: u32) -> (QuotaReading, ActivityReading) {
        let binding = ProfileBinding {
            profile_id: id(1),
            home_reference: id(2),
            binding_id: id(3),
        };
        let evidence = ProfileIdentityEvidence {
            email: "selected@example.invalid".into(),
            plan: Some("private-plan".into()),
        };
        let q = QuotaReading {
            run_id: id(n),
            binding: binding.clone(),
            identity_evidence: evidence.clone(),
            provider_version: "0.163.0".into(),
            source_kind: ActivitySourceKind::TestFixture,
            started_at_utc: time(0),
            observed_at_utc: time(second),
            completed_at_utc: time(second),
            limits: vec![QuotaLimit {
                limit_id: "codex".into(),
                limit_name: Some("Provider bucket".into()),
                primary: Some(QuotaWindow {
                    used_percent: 0,
                    window_duration_mins: Some(300),
                    resets_at_unix: Some(0),
                }),
                secondary: None,
                reached_type: None,
                monthly_credit_limit: Some("0000.01".into()),
                spend_control_reached: Some(false),
            }],
            available_reset_credits: Some("9223372036854775807".into()),
            reset_credit_details: Some(vec![QuotaCreditDetail {
                title: Some("Provider credit".into()),
                status: QuotaCreditStatus::Unknown,
                granted_at_unix: 0,
                expires_at_unix: None,
            }]),
        };
        let a = ActivityReading {
            run_id: id(n + 1),
            binding,
            identity_evidence: evidence,
            provider_version: q.provider_version.clone(),
            source_kind: q.source_kind,
            started_at_utc: q.started_at_utc.clone(),
            observed_at_utc: q.observed_at_utc.clone(),
            completed_at_utc: q.completed_at_utc.clone(),
            summary: ActivitySummaryValues {
                lifetime_tokens: Some("9007199254740993".into()),
                peak_daily_tokens: Some("9223372036854775807".into()),
                current_streak_days: Some("0".into()),
                ..Default::default()
            },
            daily_usage: Some(vec![
                ActivityDayValue {
                    provider_date: "2026-10-19".into(),
                    tokens: "9007199254740993".into(),
                },
                ActivityDayValue {
                    provider_date: "2026-10-20".into(),
                    tokens: "0".into(),
                },
            ]),
        };
        (q, a)
    }
    fn setup() -> (tempfile::TempDir, Storage) {
        let directory = crate::tests::private_directory();
        let mut storage = Storage::create(&directory.path().join("export.db"), &KEY).unwrap();
        storage
            .create_profile(&id(1), "Private selected label", &id(2))
            .unwrap();
        storage
            .create_profile(&id(4), "Private other label", &id(5))
            .unwrap();
        let (q, a) = pair(10, 0);
        let input = ProfileBindingInput {
            profile_id: q.binding.profile_id.clone(),
            expected_home_reference: q.binding.home_reference.clone(),
            expected_previous_binding_id: None,
            new_binding_id: q.binding.binding_id.clone(),
            evidence: q.identity_evidence.clone(),
            observed_at_utc: q.observed_at_utc.clone(),
        };
        storage
            .accept_collection(Some(q), Some(a), Some(input))
            .unwrap();
        (directory, storage)
    }
    fn find<'a>(
        export: &'a ReadingsExport,
        alias: &str,
        kind: &str,
        name: &str,
    ) -> &'a [Option<String>] {
        export
            .rows
            .iter()
            .find(|r| {
                r[0].as_deref() == Some(alias)
                    && r[4].as_deref() == Some(kind)
                    && r[5].as_deref() == Some(name)
            })
            .unwrap()
    }
    fn files(directory: &std::path::Path) -> (Vec<u8>, Vec<u8>) {
        (
            std::fs::read(directory.join("export.db")).unwrap(),
            std::fs::read(directory.join("export.db-wal")).unwrap(),
        )
    }

    #[test]
    fn quota_history_export_includes_all_archived_runs_and_prior_bindings_with_opt_in_local_references()
     {
        let (directory, mut storage) = setup();
        for index in 0..21 {
            let (mut quota, _) = pair(100 + index, index);
            let association = if index == 10 {
                quota.binding.binding_id = id(6);
                Some(ProfileBindingInput {
                    profile_id: id(1),
                    expected_home_reference: id(2),
                    expected_previous_binding_id: Some(id(3)),
                    new_binding_id: id(6),
                    evidence: quota.identity_evidence.clone(),
                    observed_at_utc: quota.observed_at_utc.clone(),
                })
            } else {
                if index > 10 {
                    quota.binding.binding_id = id(6);
                }
                None
            };
            if index == 1 {
                quota.available_reset_credits = None;
                quota.reset_credit_details = None;
            }
            storage.accept_quota(quota, association).unwrap();
        }
        storage.set_profile_archived(&id(1), true).unwrap();
        let before = files(directory.path());
        let records = crate::backup_validation::activity_records(&storage.connection);
        let redacted = storage.quota_history_export(&id(1), false).unwrap();
        assert_eq!(redacted.profile_count, 1);
        assert_eq!(&redacted.columns[..21], COLUMNS);
        assert_eq!(
            &redacted.columns[21..],
            &["collection_run_id", "binding_id"]
        );
        assert!(redacted.rows.iter().all(|row| row.len() == 23
            && row[0].as_deref() == Some("profile-1")
            && row[1..4].iter().all(Option::is_none)));
        let summaries = redacted
            .rows
            .iter()
            .filter(|row| row[5].as_deref() == Some("quota_limits_state"))
            .collect::<Vec<_>>();
        assert_eq!(
            summaries.len(),
            22,
            "History must exceed the browsing page without truncation."
        );
        for (index, row) in summaries.iter().enumerate() {
            assert_eq!(row[21], Some(format!("reading-{}", index + 1)));
            assert_eq!(
                row[22].as_deref(),
                Some(if index < 11 {
                    "association-1"
                } else {
                    "association-2"
                })
            );
            assert_eq!(row[10].as_deref(), Some(crate::QUOTA_SOURCE_METHOD));
            assert_eq!(row[11].as_deref(), Some("test_fixture"));
            assert_eq!(row[13].as_deref(), Some(crate::QUOTA_SOURCE_SCOPE));
            assert!(row[14].is_none());
            assert_eq!(row[15].as_deref(), Some("provisional"));
            assert_eq!(row[16].as_deref(), Some("unknown"));
            assert_eq!(row[18].as_deref(), Some("separate_profile_no_aggregation"));
        }
        let primary = redacted
            .rows
            .iter()
            .find(|row| {
                row[5].as_deref() == Some("used_percent") && row[20].as_deref() == Some("primary")
            })
            .unwrap();
        assert_eq!(primary[6].as_deref(), Some("0"));
        assert_eq!(primary[7].as_deref(), Some("percent"));
        assert_eq!(primary[17].as_deref(), Some("available"));
        let secondary = redacted
            .rows
            .iter()
            .find(|row| {
                row[5].as_deref() == Some("used_percent") && row[20].as_deref() == Some("secondary")
            })
            .unwrap();
        assert!(secondary[6].is_none());
        assert_eq!(secondary[17].as_deref(), Some("unavailable"));
        assert_eq!(
            find(
                &redacted,
                "profile-1",
                "quota_summary",
                "available_reset_credits"
            )[6]
            .as_deref(),
            Some("9223372036854775807")
        );
        assert_eq!(
            find(
                &redacted,
                "profile-1",
                "quota_limit",
                "monthly_credit_limit"
            )[6]
            .as_deref(),
            Some("0000.01")
        );
        assert!(
            redacted
                .rows
                .iter()
                .any(|row| row[21].as_deref() == Some("reading-3")
                    && row[5].as_deref() == Some("available_reset_credits")
                    && row[6].is_none()
                    && row[17].as_deref() == Some("unavailable"))
        );
        let joined = redacted
            .rows
            .iter()
            .flatten()
            .flatten()
            .cloned()
            .collect::<Vec<_>>()
            .join("|");
        for private in [
            id(1),
            id(2),
            id(3),
            id(4),
            id(5),
            id(6),
            id(10),
            id(100),
            "Private selected label".into(),
            "selected@example.invalid".into(),
            "private-plan".into(),
        ] {
            assert!(
                !joined.contains(&private),
                "Private field escaped the redacted history export."
            );
        }
        let identified = storage.quota_history_export(&id(1), true).unwrap();
        assert!(
            identified
                .rows
                .iter()
                .all(|row| row[1].as_deref() == Some(id(1).as_str())
                    && row[2].as_deref() == Some("Private selected label")
                    && row[3].is_none())
        );
        let runs = identified
            .rows
            .iter()
            .filter(|row| row[5].as_deref() == Some("quota_limits_state"))
            .map(|row| (row[21].clone(), row[22].clone()))
            .collect::<Vec<_>>();
        assert_eq!(runs[0], (Some(id(10)), Some(id(3))));
        assert_eq!(runs[1], (Some(id(100)), Some(id(3))));
        assert_eq!(runs[11], (Some(id(110)), Some(id(6))));
        assert!(
            storage
                .quota_history_export(&id(4), false)
                .unwrap()
                .rows
                .is_empty()
        );
        for invalid in [id(99), "../external".into()] {
            assert_eq!(
                storage.quota_history_export(&invalid, false).err(),
                Some(ExportError::InvalidSelection)
            );
        }
        assert_eq!(files(directory.path()), before);
        assert_eq!(
            crate::backup_validation::activity_records(&storage.connection),
            records
        );
        drop(storage);
        let mut reopened = Storage::open(&directory.path().join("export.db"), &KEY).unwrap();
        assert_eq!(
            reopened
                .quota_history_export(&id(1), false)
                .unwrap()
                .rows
                .as_slice(),
            redacted.rows.as_slice()
        );
    }

    #[test]
    fn quota_history_export_pins_profile_name_and_runs_to_one_wal_snapshot() {
        let (directory, mut storage) = setup();
        let mut writer = Storage::open(&directory.path().join("export.db"), &KEY).unwrap();
        let (begin_tx, begin_rx) = std::sync::mpsc::channel();
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let writer = std::thread::spawn(move || {
            begin_rx
                .recv_timeout(std::time::Duration::from_secs(5))
                .unwrap();
            writer
                .rename_profile(&id(1), "Changed during history export")
                .unwrap();
            let (mut quota, _) = pair(20, 1);
            quota.limits[0].primary.as_mut().unwrap().used_percent = 99;
            writer.accept_quota(quota, None).unwrap();
            done_tx.send(()).unwrap();
        });
        use rusqlite::hooks::{AuthAction, AuthContext, Authorization};
        let mut triggered = false;
        storage
            .connection
            .authorizer(Some(move |context: AuthContext<'_>| {
                if !triggered
                    && matches!(
                        context.action,
                        AuthAction::Read {
                            table_name: "collection_runs",
                            ..
                        }
                    )
                {
                    triggered = true;
                    begin_tx.send(()).unwrap();
                    done_rx
                        .recv_timeout(std::time::Duration::from_secs(5))
                        .unwrap();
                }
                Authorization::Allow
            }))
            .unwrap();
        let snapshot = storage.quota_history_export(&id(1), true).unwrap();
        storage
            .connection
            .authorizer(None::<fn(AuthContext<'_>) -> Authorization>)
            .unwrap();
        writer.join().unwrap();
        assert!(
            snapshot
                .rows
                .iter()
                .all(|row| row[2].as_deref() == Some("Private selected label")
                    && row[21].as_deref() == Some(id(10).as_str()))
        );
        let next = storage.quota_history_export(&id(1), true).unwrap();
        assert_eq!(
            next.rows
                .iter()
                .filter(|row| row[5].as_deref() == Some("quota_limits_state"))
                .count(),
            2
        );
        assert!(
            next.rows
                .iter()
                .all(|row| row[2].as_deref() == Some("Changed during history export"))
        );
        assert!(
            next.rows
                .iter()
                .any(|row| row[21].as_deref() == Some(id(20).as_str())
                    && row[5].as_deref() == Some("used_percent")
                    && row[6].as_deref() == Some("99"))
        );
    }

    #[test]
    fn quota_history_export_fails_atomically_on_packet_read_error_and_row_limit() {
        let (directory, mut storage) = setup();
        use rusqlite::hooks::{AuthAction, AuthContext, Authorization};
        let before = files(directory.path());
        storage
            .connection
            .authorizer(Some(|context: AuthContext<'_>| {
                if matches!(
                    context.action,
                    AuthAction::Read {
                        table_name: "quota_limits",
                        ..
                    }
                ) {
                    Authorization::Deny
                } else {
                    Authorization::Allow
                }
            }))
            .unwrap();
        assert_eq!(
            storage.quota_history_export(&id(1), false).err(),
            Some(ExportError::ReadFailed)
        );
        storage
            .connection
            .authorizer(None::<fn(AuthContext<'_>) -> Authorization>)
            .unwrap();
        assert!(storage.connection.is_autocommit());
        assert_eq!(files(directory.path()), before);
        for index in 0..49 {
            let (mut quota, _) = pair(1000 + index, 0);
            quota.limits = (0..64)
                .map(|limit| {
                    let mut value = quota.limits[0].clone();
                    value.limit_id = format!("bucket-{limit}");
                    value
                })
                .collect();
            quota.reset_credit_details = Some(vec![
                quota.reset_credit_details.as_ref().unwrap()[0]
                    .clone();
                32
            ]);
            storage.accept_quota(quota, None).unwrap();
        }
        let before = files(directory.path());
        assert_eq!(
            storage.quota_history_export(&id(1), false).err(),
            Some(ExportError::TooLarge)
        );
        assert!(storage.connection.is_autocommit());
        assert_eq!(files(directory.path()), before);
    }

    #[test]
    fn export_exact_selected_readings_redacts_identifiers_and_preserves_encrypted_wal() {
        let (directory, mut storage) = setup();
        let before = files(directory.path());
        let records = crate::backup_validation::activity_records(&storage.connection);
        let export = storage.readings_export(&[id(4), id(1)], false).unwrap();
        assert_eq!(export.profile_count, 2);
        assert!(
            export
                .rows
                .iter()
                .all(|r| r.len() == 21 && r[1..4].iter().all(Option::is_none))
        );
        let joined = export
            .rows
            .iter()
            .flatten()
            .flatten()
            .cloned()
            .collect::<Vec<_>>()
            .join("|");
        for private in [
            id(1),
            id(2),
            id(3),
            id(4),
            id(5),
            id(10),
            id(11),
            "Private selected label".into(),
            "Private other label".into(),
            "selected@example.invalid".into(),
            "private-plan".into(),
            directory.path().display().to_string(),
        ] {
            assert!(
                !joined.contains(&private),
                "Private storage field escaped the projection"
            );
        }
        let exact = find(&export, "profile-2", "activity_summary", "lifetime_tokens");
        assert_eq!(exact[6].as_deref(), Some("9007199254740993"));
        assert_eq!(exact[7].as_deref(), Some("tokens"));
        assert_eq!(exact[9].as_deref(), Some(time(0).as_str()));
        assert_eq!(exact[10].as_deref(), Some("account/usage/read"));
        assert_eq!(exact[11].as_deref(), Some("test_fixture"));
        assert_eq!(
            exact[13].as_deref(),
            Some("account_activity_with_unknown_application_scope")
        );
        assert!(exact[14].is_none());
        assert_eq!(exact[15].as_deref(), Some("provisional"));
        assert_eq!(exact[16].as_deref(), Some("unknown"));
        assert_eq!(
            exact[18].as_deref(),
            Some("separate_profile_no_aggregation")
        );
        assert_eq!(
            find(
                &export,
                "profile-2",
                "activity_summary",
                "peak_daily_tokens"
            )[6]
            .as_deref(),
            Some("9223372036854775807")
        );
        assert_eq!(
            find(
                &export,
                "profile-2",
                "activity_summary",
                "current_streak_days"
            )[6]
            .as_deref(),
            Some("0")
        );
        let missing = find(
            &export,
            "profile-2",
            "activity_summary",
            "longest_running_turn_sec",
        );
        assert!(missing[6].is_none());
        assert_eq!(missing[17].as_deref(), Some("unavailable"));
        assert!(find(&export, "profile-1", "activity_summary", "lifetime_tokens")[6].is_none());
        assert_eq!(
            find(
                &export,
                "profile-2",
                "quota_summary",
                "available_reset_credits"
            )[6]
            .as_deref(),
            Some("9223372036854775807")
        );
        assert_eq!(
            find(&export, "profile-2", "quota_window", "used_percent")[6].as_deref(),
            Some("0")
        );
        assert_eq!(
            find(&export, "profile-2", "quota_limit", "monthly_credit_limit")[6].as_deref(),
            Some("0000.01")
        );
        assert_eq!(
            find(
                &export,
                "profile-2",
                "quota_window",
                "reset_confirmation_state"
            )[6]
            .as_deref(),
            Some("not_confirmed")
        );
        assert_eq!(
            find(
                &export,
                "profile-2",
                "quota_credit",
                "reset_credit_1_status"
            )[6]
            .as_deref(),
            Some("unknown")
        );
        assert!(
            export
                .rows
                .iter()
                .filter(|r| r[4].as_deref() != Some("activity_daily"))
                .all(|r| r[8].is_none())
        );
        let identified = storage.readings_export(&[id(1)], true).unwrap();
        let row = find(
            &identified,
            "profile-1",
            "activity_summary",
            "lifetime_tokens",
        );
        assert_eq!(row[1].as_deref(), Some(id(1).as_str()));
        assert_eq!(row[2].as_deref(), Some("Private selected label"));
        assert_eq!(row[3].as_deref(), Some("selected@example.invalid"));
        assert_eq!(files(directory.path()), before);
        assert_eq!(
            crate::backup_validation::activity_records(&storage.connection),
            records
        );
        assert!(!before.0.starts_with(b"SQLite format 3\0"));
        drop(storage);
        assert!(Storage::open(&directory.path().join("export.db"), &[0x44; 32]).is_err());
        let mut reopened = Storage::open(&directory.path().join("export.db"), &KEY).unwrap();
        assert_eq!(
            reopened
                .readings_export(&[id(1)], false)
                .unwrap()
                .rows
                .iter()
                .filter(|r| r[4].as_deref() == Some("activity_daily"))
                .count(),
            2
        );
    }

    #[test]
    fn export_latest_summary_keeps_each_retained_day_source_and_explicit_missing_states() {
        let (_directory, mut storage) = setup();
        let (mut q, mut a) = pair(20, 1);
        q.limits.clear();
        q.available_reset_credits = None;
        q.reset_credit_details = None;
        a.provider_version = "0.164.0".into();
        a.summary.lifetime_tokens = Some("7".into());
        a.daily_usage = None;
        storage.accept_quota(q, None).unwrap();
        storage.accept_activity(a).unwrap();
        let export = storage.readings_export(&[id(1)], false).unwrap();
        let summary = find(&export, "profile-1", "activity_summary", "lifetime_tokens");
        assert_eq!(summary[6].as_deref(), Some("7"));
        assert_eq!(summary[12].as_deref(), Some("0.164.0"));
        assert_eq!(
            find(
                &export,
                "profile-1",
                "activity_summary",
                "daily_buckets_state"
            )[6]
            .as_deref(),
            Some("unavailable")
        );
        let days = export
            .rows
            .iter()
            .filter(|r| r[4].as_deref() == Some("activity_daily"))
            .collect::<Vec<_>>();
        assert_eq!(days.len(), 2);
        for day in days {
            assert_eq!(day[9].as_deref(), Some(time(0).as_str()));
            assert_eq!(day[12].as_deref(), Some("0.163.0"));
            assert_eq!(day[16].as_deref(), Some("unknown"));
            assert!(day[8].as_deref().is_some_and(|v| v.len() == 10));
        }
        assert_eq!(
            find(&export, "profile-1", "quota_summary", "quota_limits_state")[6].as_deref(),
            Some("empty")
        );
        assert!(
            !export
                .rows
                .iter()
                .any(|r| r[4].as_deref() == Some("quota_window"))
        );
        assert!(
            find(
                &export,
                "profile-1",
                "quota_summary",
                "available_reset_credits"
            )[6]
            .is_none()
        );
    }

    #[test]
    fn export_rejects_unsafe_selections_and_read_failure_without_any_database_change() {
        let (directory, mut storage) = setup();
        storage.set_profile_archived(&id(4), true).unwrap();
        let before = files(directory.path());
        for selection in [
            vec![],
            vec![id(1), id(1)],
            vec![id(99)],
            vec![id(4)],
            vec!["ABCDEF01-0000-4000-8000-000000000000".into()],
            vec!["../private".into()],
            vec![id(1); 11],
        ] {
            assert_eq!(
                storage.readings_export(&selection, false).err(),
                Some(ExportError::InvalidSelection)
            );
        }
        use rusqlite::hooks::{AuthAction, AuthContext, Authorization};
        storage
            .connection
            .authorizer(Some(|context: AuthContext<'_>| {
                if matches!(
                    context.action,
                    AuthAction::Read {
                        table_name: "activity_summaries",
                        ..
                    }
                ) {
                    Authorization::Deny
                } else {
                    Authorization::Allow
                }
            }))
            .unwrap();
        assert_eq!(
            storage.readings_export(&[id(1)], false).err(),
            Some(ExportError::ReadFailed)
        );
        storage
            .connection
            .authorizer(None::<fn(AuthContext<'_>) -> Authorization>)
            .unwrap();
        assert!(storage.connection.is_autocommit());
        assert_eq!(files(directory.path()), before);
        assert!(storage.readings_export(&[id(1)], false).is_ok());
    }

    #[test]
    fn export_reads_one_wal_snapshot_when_another_connection_commits_between_capabilities() {
        let (directory, mut storage) = setup();
        let mut writer = Storage::open(&directory.path().join("export.db"), &KEY).unwrap();
        let (begin_tx, begin_rx) = std::sync::mpsc::channel();
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let writer = std::thread::spawn(move || {
            begin_rx
                .recv_timeout(std::time::Duration::from_secs(5))
                .unwrap();
            writer
                .rename_profile(&id(1), "Changed during export")
                .unwrap();
            let (mut q, mut a) = pair(20, 1);
            q.limits[0].primary.as_mut().unwrap().used_percent = 99;
            a.summary.lifetime_tokens = Some("88".into());
            writer.accept_collection(Some(q), Some(a), None).unwrap();
            done_tx.send(()).unwrap();
        });
        use rusqlite::hooks::{AuthAction, AuthContext, Authorization};
        let mut triggered = false;
        storage
            .connection
            .authorizer(Some(move |context: AuthContext<'_>| {
                if !triggered
                    && matches!(
                        context.action,
                        AuthAction::Read {
                            table_name: "current_quota_readings",
                            ..
                        }
                    )
                {
                    triggered = true;
                    begin_tx.send(()).unwrap();
                    done_rx
                        .recv_timeout(std::time::Duration::from_secs(5))
                        .unwrap();
                }
                Authorization::Allow
            }))
            .unwrap();
        let snapshot = storage.readings_export(&[id(1)], true).unwrap();
        storage
            .connection
            .authorizer(None::<fn(AuthContext<'_>) -> Authorization>)
            .unwrap();
        writer.join().unwrap();
        assert_eq!(
            find(&snapshot, "profile-1", "quota_window", "used_percent")[6].as_deref(),
            Some("0")
        );
        let row = find(
            &snapshot,
            "profile-1",
            "activity_summary",
            "lifetime_tokens",
        );
        assert_eq!(row[6].as_deref(), Some("9007199254740993"));
        assert_eq!(row[2].as_deref(), Some("Private selected label"));
        let next = storage.readings_export(&[id(1)], true).unwrap();
        assert_eq!(
            find(&next, "profile-1", "quota_window", "used_percent")[6].as_deref(),
            Some("99")
        );
        let row = find(&next, "profile-1", "activity_summary", "lifetime_tokens");
        assert_eq!(row[6].as_deref(), Some("88"));
        assert_eq!(row[2].as_deref(), Some("Changed during export"));
    }

    #[test]
    fn export_refuses_accumulated_current_days_over_the_row_limit_without_truncation() {
        let (_directory, mut storage) = setup();
        for batch in 0..13 {
            let (_, mut a) = pair(100 + batch * 2, batch + 1);
            a.daily_usage = Some(
                (0..4096)
                    .map(|offset| {
                        let date = crate::quota::unix_to_utc(
                            1_577_836_800 + i64::from(batch * 4096 + offset) * 86400,
                        );
                        ActivityDayValue {
                            provider_date: date[..10].into(),
                            tokens: "0".into(),
                        }
                    })
                    .collect(),
            );
            storage.accept_activity(a).unwrap();
        }
        let count: i64 = storage
            .connection
            .query_row(
                "SELECT count(*) FROM current_activity_days WHERE profile_id=?1",
                [id(1)],
                |row| row.get(0),
            )
            .unwrap();
        assert!(count > 50_000);
        assert_eq!(
            storage.readings_export(&[id(1)], false).err(),
            Some(ExportError::TooLarge)
        );
        assert_eq!(
            storage
                .connection
                .query_row(
                    "SELECT count(*) FROM current_activity_days WHERE profile_id=?1",
                    [id(1)],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            count
        );
    }
}
