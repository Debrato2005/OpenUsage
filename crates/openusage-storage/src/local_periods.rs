//! Independent grouped totals. Legacy source summaries are never combined with these records.
use crate::backup_validation::canonical_uuid;
use crate::local_logs::{valid_model, valid_version};
use crate::{LocalLogError, LocalUsageScanSummary, Storage, activity, valid_time_zone};
use openusage_locallogs::tools::{PeriodPolicy, tool};
use openusage_locallogs::{AgentPeriod, AgentSession, ModelDetail, TokenDetails, UsagePeriod};
use rusqlite::{Connection, OptionalExtension, Row, TransactionBehavior, params};
use std::collections::{BTreeMap, BTreeSet};

type Result<T> = std::result::Result<T, LocalLogError>;
const MAX_COUNT: u64 = 1_000_000_000_000_000;
const MAX_ROWS: usize = 200_000;

/// One complete, successful scan of all selected roots for one tool. None means sessions were not read.
pub struct GroupedLocalScan<'a> {
    pub tool: &'a str,
    pub source_ids: &'a [String],
    pub time_zone: Option<&'a str>,
    pub scanner_version: &'a str,
    pub observed_at_utc: &'a str,
    pub periods: &'a [AgentPeriod],
    pub sessions: Option<&'a [AgentSession]>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalScanProvenance {
    pub id: String,
    pub source_ids: Vec<String>,
    pub scanner_version: String,
    pub normalization_version: u32,
    pub observed_at_utc: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalToolScope {
    pub tool: String,
    pub time_zone: Option<String>,
    pub latest_scan: LocalScanProvenance,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavedLocalPeriod {
    pub period: AgentPeriod,
    pub scan: LocalScanProvenance,
    pub retained: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavedGroupedSession {
    pub session: AgentSession,
    pub scan: LocalScanProvenance,
    pub retained: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum LocalUsageKey {
    Day(String),
    Unplaced,
    Session(String),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalScanDifference {
    pub key: LocalUsageKey,
    pub saved_total_tokens: u64,
    pub new_total_tokens: u64,
    pub scan: LocalScanProvenance,
}

// Explicit trusted column lists are also used by backup and checkpoint copying.
const METRICS: &str = "total_tokens,input_tokens,output_tokens,cache_write_tokens,cache_read_tokens,reasoning_tokens,estimated_value_micros";
pub(crate) const TABLES: &[(&str, &str)] = &[
    ("local_usage_scopes", "tool,period_kind,time_zone"),
    (
        "local_usage_scans",
        "id,tool,scanner_version,normalization_version,observed_at_utc,latest",
    ),
    ("local_usage_scan_roots", "scan_id,tool,log_source_id"),
    (
        "local_usage_records",
        "id,tool,kind,period_key,scan_id,retained,project,first_activity_utc,last_activity_utc,total_tokens,input_tokens,output_tokens,cache_write_tokens,cache_read_tokens,reasoning_tokens,estimated_value_micros",
    ),
    (
        "local_usage_record_models",
        "record_id,model,total_tokens,input_tokens,output_tokens,cache_write_tokens,cache_read_tokens,reasoning_tokens,estimated_value_micros",
    ),
    (
        "local_usage_scan_differences",
        "record_id,tool,scan_id,saved_total_tokens,new_total_tokens",
    ),
];
pub(crate) const DELETE_RECORDS: &str = "DELETE FROM local_usage_scan_differences; DELETE FROM local_usage_record_models; DELETE FROM local_usage_records; DELETE FROM local_usage_scan_roots; DELETE FROM local_usage_scans; DELETE FROM local_usage_scopes;";

#[derive(Clone)]
struct Record {
    id: String,
    tool: String,
    key: LocalUsageKey,
    scan_id: String,
    retained: bool,
    project: Option<String>,
    first: Option<String>,
    last: Option<String>,
    total: u64,
    details: TokenDetails,
    estimate: Option<u64>,
    models: Vec<ModelDetail>,
}
impl LocalUsageKey {
    fn parts(&self) -> (&str, &str) {
        match self {
            Self::Day(day) => ("day", day),
            Self::Unplaced => ("unplaced", ""),
            Self::Session(key) => ("session", key),
        }
    }
}
fn key(kind: &str, value: String) -> Result<LocalUsageKey> {
    match kind {
        "day" if activity::activity_date(&value) => Ok(LocalUsageKey::Day(value)),
        "unplaced" if value.is_empty() => Ok(LocalUsageKey::Unplaced),
        "session"
            if (1..=256).contains(&value.len())
                && value
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-_./:".contains(&b)) =>
        {
            Ok(LocalUsageKey::Session(value))
        }
        _ => Err(LocalLogError::InvalidInput),
    }
}
fn period_kind(id: &str) -> Result<&'static str> {
    let definition = tool(id).ok_or(LocalLogError::InvalidInput)?;
    Ok(
        if definition
            .agent_report
            .as_ref()
            .is_some_and(|p| p.period == PeriodPolicy::Unplaced)
        {
            "unplaced"
        } else {
            "day"
        },
    )
}
fn decimal(value: &str) -> Result<u64> {
    if !activity::activity_decimal(value) {
        return Err(LocalLogError::InvalidInput);
    }
    value
        .parse()
        .ok()
        .filter(|n| *n <= MAX_COUNT)
        .ok_or(LocalLogError::InvalidInput)
}
fn metric_values(total: Option<u64>, d: &TokenDetails, estimate: Option<u64>) -> [Option<u64>; 7] {
    [
        total,
        d.input_tokens,
        d.output_tokens,
        d.cache_write_tokens,
        d.cache_read_tokens,
        d.reasoning_tokens,
        estimate,
    ]
}
fn valid_metrics(
    total: Option<u64>,
    details: &TokenDetails,
    estimate: Option<u64>,
    bound: u64,
) -> bool {
    let values = metric_values(total, details, estimate);
    let limit = total.unwrap_or(bound);
    if values.iter().flatten().any(|v| *v > MAX_COUNT) {
        return false;
    }
    let sum: u64 = values[1..5].iter().flatten().sum();
    limit <= bound
        && sum <= limit
        && (values[1..5].iter().any(Option::is_none) || total.is_none() || sum == limit)
        && details.reasoning_tokens.is_none_or(|v| {
            v <= details.output_tokens.unwrap_or(
                limit - values[1].unwrap_or(0) - values[3].unwrap_or(0) - values[4].unwrap_or(0),
            )
        })
}
fn valid_record(record: &Record) -> bool {
    let (kind, value) = record.key.parts();
    if key(kind, value.to_owned()).is_err()
        || !matches!(period_kind(&record.tool), Ok(policy) if kind == "session" || policy == kind)
        || !valid_metrics(
            Some(record.total),
            &record.details,
            record.estimate,
            MAX_COUNT,
        )
        || record
            .project
            .as_deref()
            .is_some_and(|v| v.is_empty() || v.len() > 512 || v.chars().any(char::is_control))
        || record
            .first
            .as_deref()
            .is_some_and(|v| !activity::activity_utc_timestamp(v))
        || record
            .last
            .as_deref()
            .is_some_and(|v| !activity::activity_utc_timestamp(v))
        || record
            .first
            .as_ref()
            .zip(record.last.as_ref())
            .is_some_and(|(a, b)| a > b)
        || (kind != "session"
            && (record.project.is_some() || record.first.is_some() || record.last.is_some()))
        || record.models.len() > MAX_ROWS
    {
        return false;
    }
    let mut names = BTreeSet::new();
    let mut sums = [0u64; 7];
    let mut known = [true; 7];
    for model in &record.models {
        if !valid_model(&model.model)
            || !names.insert(&model.model)
            || !valid_metrics(
                model.total_tokens,
                &model.details,
                model.estimated_value_micros,
                record.total,
            )
        {
            return false;
        }
        for (i, value) in metric_values(
            model.total_tokens,
            &model.details,
            model.estimated_value_micros,
        )
        .iter()
        .enumerate()
        {
            known[i] &= value.is_some();
            let Some(sum) = sums[i].checked_add(value.unwrap_or(0)) else {
                return false;
            };
            sums[i] = sum;
        }
    }
    if record.models.is_empty() {
        return true;
    }
    let parent = metric_values(Some(record.total), &record.details, record.estimate);
    // Model totals are either fully attributed or unavailable throughout this snapshot.
    if !known[0] && record.models.iter().any(|m| m.total_tokens.is_some()) {
        return false;
    }
    if !(0..7).all(|i| sums[i] <= if i == 6 { MAX_COUNT } else { record.total })
        || !(0..7).all(|i| {
            parent[i].is_none_or(|p| sums[i] <= p && (!known[0] || !known[i] || sums[i] == p))
        })
    {
        return false;
    }
    // Known contributions form lower bounds. Missing fields remain unavailable in the saved record.
    let lower = [1, 2, 3, 4].map(|i| parent[i].unwrap_or(0).max(sums[i]));
    let used: u64 = lower.iter().sum();
    used <= record.total
        && parent[5].unwrap_or(0).max(sums[5])
            <= parent[2].unwrap_or(record.total - lower[0] - lower[2] - lower[3])
}
fn read_metrics(row: &Row<'_>, start: usize) -> Result<(Option<u64>, TokenDetails, Option<u64>)> {
    let mut values = [None; 7];
    for (index, value) in values.iter_mut().enumerate() {
        *value = row
            .get::<_, Option<String>>(start + index)
            .map_err(|_| LocalLogError::ReadFailed)?
            .as_deref()
            .map(decimal)
            .transpose()?;
    }
    Ok((
        values[0],
        TokenDetails {
            input_tokens: values[1],
            output_tokens: values[2],
            cache_write_tokens: values[3],
            cache_read_tokens: values[4],
            reasoning_tokens: values[5],
        },
        values[6],
    ))
}
fn read_records(connection: &Connection, schema: &str, tool_id: &str) -> Result<Vec<Record>> {
    let mut statement = connection
        .prepare(&format!(
            "SELECT {} FROM {schema}.local_usage_records WHERE tool=?1 ORDER BY kind,period_key",
            TABLES[3].1
        ))
        .map_err(|_| LocalLogError::ReadFailed)?;
    let mut rows = statement
        .query([tool_id])
        .map_err(|_| LocalLogError::ReadFailed)?;
    let mut records = Vec::new();
    while let Some(row) = rows.next().map_err(|_| LocalLogError::ReadFailed)? {
        let get = |n| {
            row.get::<_, String>(n)
                .map_err(|_| LocalLogError::ReadFailed)
        };
        let optional = |n| {
            row.get::<_, Option<String>>(n)
                .map_err(|_| LocalLogError::ReadFailed)
        };
        let (total, details, estimate) = read_metrics(row, 9)?;
        let mut record = Record {
            id: get(0)?,
            tool: get(1)?,
            key: key(&get(2)?, get(3)?)?,
            scan_id: get(4)?,
            retained: row.get(5).map_err(|_| LocalLogError::ReadFailed)?,
            project: optional(6)?,
            first: optional(7)?,
            last: optional(8)?,
            total: total.ok_or(LocalLogError::ReadFailed)?,
            details,
            estimate,
            models: Vec::new(),
        };
        let mut models = connection.prepare(&format!("SELECT model,{METRICS} FROM {schema}.local_usage_record_models WHERE record_id=?1 ORDER BY model")).map_err(|_| LocalLogError::ReadFailed)?;
        let mut model_rows = models
            .query([&record.id])
            .map_err(|_| LocalLogError::ReadFailed)?;
        while let Some(row) = model_rows.next().map_err(|_| LocalLogError::ReadFailed)? {
            let (total_tokens, details, estimated_value_micros) = read_metrics(row, 1)?;
            record.models.push(ModelDetail {
                model: row.get(0).map_err(|_| LocalLogError::ReadFailed)?,
                total_tokens,
                details,
                estimated_value_micros,
            });
            if record.models.len() > MAX_ROWS {
                return Err(LocalLogError::ReadFailed);
            }
        }
        if !canonical_uuid(&record.id) || !canonical_uuid(&record.scan_id) || !valid_record(&record)
        {
            return Err(LocalLogError::ReadFailed);
        }
        records.push(record);
    }
    Ok(records)
}
fn provenance(connection: &Connection, schema: &str, id: &str) -> Result<LocalScanProvenance> {
    let (scanner_version, normalization_version, observed_at_utc): (String, u32, String) = connection.query_row(&format!("SELECT scanner_version,normalization_version,observed_at_utc FROM {schema}.local_usage_scans WHERE id=?1"), [id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).map_err(|_| LocalLogError::ReadFailed)?;
    let mut statement = connection.prepare(&format!("SELECT log_source_id FROM {schema}.local_usage_scan_roots WHERE scan_id=?1 ORDER BY log_source_id")).map_err(|_| LocalLogError::ReadFailed)?;
    let source_ids = statement
        .query_map([id], |r| r.get::<_, String>(0))
        .map_err(|_| LocalLogError::ReadFailed)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|_| LocalLogError::ReadFailed)?;
    if !canonical_uuid(id)
        || !valid_version(&scanner_version)
        || normalization_version != 1
        || !activity::activity_utc_timestamp(&observed_at_utc)
        || !(1..=64).contains(&source_ids.len())
        || source_ids.iter().any(|s| !canonical_uuid(s))
    {
        return Err(LocalLogError::ReadFailed);
    }
    Ok(LocalScanProvenance {
        id: id.into(),
        source_ids,
        scanner_version,
        normalization_version,
        observed_at_utc,
    })
}
fn write_record(connection: &Connection, record: &Record) -> Result<()> {
    let (kind, value) = record.key.parts();
    let values = metric_values(Some(record.total), &record.details, record.estimate)
        .map(|v| v.map(|v| v.to_string()));
    connection.execute("INSERT INTO local_usage_records VALUES(?1,?2,?3,?4,?5,0,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15) ON CONFLICT(tool,kind,period_key) DO UPDATE SET scan_id=excluded.scan_id,retained=0,project=excluded.project,first_activity_utc=excluded.first_activity_utc,last_activity_utc=excluded.last_activity_utc,total_tokens=excluded.total_tokens,input_tokens=excluded.input_tokens,output_tokens=excluded.output_tokens,cache_write_tokens=excluded.cache_write_tokens,cache_read_tokens=excluded.cache_read_tokens,reasoning_tokens=excluded.reasoning_tokens,estimated_value_micros=excluded.estimated_value_micros", params![record.id,record.tool,kind,value,record.scan_id,record.project,record.first,record.last,values[0],values[1],values[2],values[3],values[4],values[5],values[6]]).map_err(|_| LocalLogError::WriteFailed)?;
    connection
        .execute(
            "DELETE FROM local_usage_record_models WHERE record_id=?1",
            [&record.id],
        )
        .map_err(|_| LocalLogError::WriteFailed)?;
    for model in &record.models {
        let values = metric_values(
            model.total_tokens,
            &model.details,
            model.estimated_value_micros,
        )
        .map(|v| v.map(|v| v.to_string()));
        connection
            .execute(
                "INSERT INTO local_usage_record_models VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",
                params![
                    record.id,
                    model.model,
                    values[0],
                    values[1],
                    values[2],
                    values[3],
                    values[4],
                    values[5],
                    values[6]
                ],
            )
            .map_err(|_| LocalLogError::WriteFailed)?;
    }
    Ok(())
}
fn keep_higher(
    connection: &Connection,
    saved: &Record,
    observed: u64,
    scan_id: &str,
) -> Result<()> {
    connection.execute("INSERT INTO local_usage_scan_differences VALUES(?1,?2,?3,?4,?5) ON CONFLICT(record_id,saved_total_tokens,new_total_tokens) DO NOTHING", params![saved.id,saved.tool,scan_id,saved.total.to_string(),observed.to_string()]).map_err(|_| LocalLogError::WriteFailed)?;
    connection
        .execute(
            "UPDATE local_usage_records SET retained=1 WHERE id=?1",
            [&saved.id],
        )
        .map_err(|_| LocalLogError::WriteFailed)?;
    Ok(())
}
impl Storage {
    pub fn local_tool_scope(&self, tool_id: &str) -> Result<Option<LocalToolScope>> {
        let scope: Option<(Option<String>, String)> = self.connection.query_row("SELECT s.time_zone,r.id FROM local_usage_scopes s JOIN local_usage_scans r ON r.tool=s.tool AND r.latest=1 WHERE s.tool=?1", [tool_id], |r| Ok((r.get(0)?,r.get(1)?))).optional().map_err(|_| LocalLogError::ReadFailed)?;
        scope
            .map(|(time_zone, id)| {
                Ok(LocalToolScope {
                    tool: tool_id.into(),
                    time_zone,
                    latest_scan: provenance(&self.connection, "main", &id)?,
                })
            })
            .transpose()
    }
    /// Includes the explicit unplaced period. Use local_dated_periods for any dated view.
    pub fn local_periods(&self, tool_id: &str) -> Result<Vec<SavedLocalPeriod>> {
        read_records(&self.connection, "main", tool_id)?
            .into_iter()
            .filter(|r| !matches!(r.key, LocalUsageKey::Session(_)))
            .map(|r| {
                let period = match r.key {
                    LocalUsageKey::Day(day) => UsagePeriod::Day(day),
                    LocalUsageKey::Unplaced => UsagePeriod::Unplaced,
                    _ => unreachable!(),
                };
                Ok(SavedLocalPeriod {
                    period: AgentPeriod {
                        period,
                        total_tokens: r.total,
                        details: r.details,
                        estimated_value_micros: r.estimate,
                        models: r.models,
                    },
                    scan: provenance(&self.connection, "main", &r.scan_id)?,
                    retained: r.retained,
                })
            })
            .collect()
    }
    pub fn local_dated_periods(
        &self,
        tool_id: &str,
        from: &str,
        to: &str,
    ) -> Result<Vec<SavedLocalPeriod>> {
        if !activity::activity_date(from) || !activity::activity_date(to) || from > to {
            return Err(LocalLogError::InvalidInput);
        }
        Ok(self.local_periods(tool_id)?.into_iter().filter(|r| matches!(&r.period.period, UsagePeriod::Day(day) if day.as_str() >= from && day.as_str() <= to)).collect())
    }
    pub fn grouped_local_sessions(&self, tool_id: &str) -> Result<Vec<SavedGroupedSession>> {
        read_records(&self.connection, "main", tool_id)?
            .into_iter()
            .filter(|r| matches!(r.key, LocalUsageKey::Session(_)))
            .map(|r| {
                let LocalUsageKey::Session(session_key) = r.key else {
                    unreachable!()
                };
                Ok(SavedGroupedSession {
                    session: AgentSession {
                        session_key,
                        project: r.project,
                        first_activity_utc: r.first,
                        last_activity_utc: r.last,
                        total_tokens: r.total,
                        details: r.details,
                        estimated_value_micros: r.estimate,
                        models: r.models,
                    },
                    scan: provenance(&self.connection, "main", &r.scan_id)?,
                    retained: r.retained,
                })
            })
            .collect()
    }
    pub fn local_scan_differences(&self, tool_id: &str) -> Result<Vec<LocalScanDifference>> {
        read_differences(&self.connection, "main", tool_id)
    }
    pub fn save_grouped_local_scan(
        &mut self,
        scan: &GroupedLocalScan<'_>,
    ) -> Result<LocalUsageScanSummary> {
        let policy = period_kind(scan.tool)?;
        if !valid_version(scan.scanner_version)
            || !activity::activity_utc_timestamp(scan.observed_at_utc)
            || !(1..=64).contains(&scan.source_ids.len())
            || scan.source_ids.iter().any(|id| !canonical_uuid(id))
            || scan.source_ids.iter().collect::<BTreeSet<_>>().len() != scan.source_ids.len()
            || match policy {
                "day" => scan.time_zone.is_none_or(|z| !valid_time_zone(z)),
                _ => scan.time_zone.is_some(),
            }
        {
            return Err(LocalLogError::InvalidInput);
        }
        let count = scan
            .periods
            .iter()
            .map(|p| p.models.len())
            .chain(
                scan.sessions
                    .unwrap_or_default()
                    .iter()
                    .map(|s| s.models.len()),
            )
            .fold(
                scan.periods
                    .len()
                    .checked_add(scan.sessions.map_or(0, |s| s.len())),
                |sum, models| sum.and_then(|n| n.checked_add(models)),
            );
        if count.is_none_or(|n| n > MAX_ROWS) {
            return Err(LocalLogError::InvalidInput);
        }
        let scan_id = activity::new_uuid().map_err(|_| LocalLogError::WriteFailed)?;
        let mut records = Vec::new();
        for period in scan.periods {
            records.push(Record {
                id: activity::new_uuid().map_err(|_| LocalLogError::WriteFailed)?,
                tool: scan.tool.into(),
                key: match &period.period {
                    UsagePeriod::Day(day) => LocalUsageKey::Day(day.clone()),
                    UsagePeriod::Unplaced => LocalUsageKey::Unplaced,
                },
                scan_id: scan_id.clone(),
                retained: false,
                project: None,
                first: None,
                last: None,
                total: period.total_tokens,
                details: period.details.clone(),
                estimate: period.estimated_value_micros,
                models: period.models.clone(),
            });
        }
        for session in scan.sessions.unwrap_or_default() {
            records.push(Record {
                id: activity::new_uuid().map_err(|_| LocalLogError::WriteFailed)?,
                tool: scan.tool.into(),
                key: LocalUsageKey::Session(session.session_key.clone()),
                scan_id: scan_id.clone(),
                retained: false,
                project: session.project.clone(),
                first: session.first_activity_utc.clone(),
                last: session.last_activity_utc.clone(),
                total: session.total_tokens,
                details: session.details.clone(),
                estimate: session.estimated_value_micros,
                models: session.models.clone(),
            });
        }
        if records.iter().any(|r| !valid_record(r))
            || records
                .iter()
                .map(|r| &r.key)
                .collect::<BTreeSet<_>>()
                .len()
                != records.len()
        {
            return Err(LocalLogError::InvalidInput);
        }
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| LocalLogError::WriteFailed)?;
        for source in scan.source_ids {
            let allowed: bool = transaction.query_row("SELECT EXISTS(SELECT 1 FROM log_sources WHERE id=?1 AND tool=?2 AND consent='granted')", params![source,scan.tool], |r| r.get(0)).map_err(|_| LocalLogError::WriteFailed)?;
            if !allowed {
                return Err(LocalLogError::NotConsented);
            }
        }
        let zone: Option<Option<String>> = transaction
            .query_row(
                "SELECT time_zone FROM local_usage_scopes WHERE tool=?1",
                [scan.tool],
                |r| r.get(0),
            )
            .optional()
            .map_err(|_| LocalLogError::ReadFailed)?;
        if zone
            .as_ref()
            .is_some_and(|z| z.as_deref() != scan.time_zone)
        {
            return Err(LocalLogError::TimeZoneMismatch);
        }
        transaction
            .execute(
                "INSERT INTO local_usage_scopes VALUES(?1,?2,?3) ON CONFLICT(tool) DO NOTHING",
                params![scan.tool, policy, scan.time_zone],
            )
            .map_err(|_| LocalLogError::WriteFailed)?;
        let saved: BTreeMap<_, _> = read_records(&transaction, "main", scan.tool)?
            .into_iter()
            .map(|r| (r.key.clone(), r))
            .collect();
        transaction
            .execute(
                "UPDATE local_usage_scans SET latest=0 WHERE tool=?1",
                [scan.tool],
            )
            .map_err(|_| LocalLogError::WriteFailed)?;
        transaction
            .execute(
                "INSERT INTO local_usage_scans VALUES(?1,?2,?3,1,?4,1)",
                params![
                    scan_id,
                    scan.tool,
                    scan.scanner_version,
                    scan.observed_at_utc
                ],
            )
            .map_err(|_| LocalLogError::WriteFailed)?;
        for source in scan.source_ids {
            transaction
                .execute(
                    "INSERT INTO local_usage_scan_roots VALUES(?1,?2,?3)",
                    params![scan_id, scan.tool, source],
                )
                .map_err(|_| LocalLogError::WriteFailed)?;
        }
        let mut summary = LocalUsageScanSummary::default();
        for record in &mut records {
            if let Some(previous) = saved.get(&record.key) {
                record.id.clone_from(&previous.id);
                if previous.total > record.total {
                    keep_higher(&transaction, previous, record.total, &scan_id)?;
                    summary.kept_higher += 1;
                    continue;
                }
                if previous.total == record.total {
                    summary.unchanged += 1;
                } else {
                    summary.raised += 1;
                }
            } else {
                summary.inserted += 1;
            }
            write_record(&transaction, record)?;
        }
        let observed: BTreeSet<_> = records.iter().map(|r| &r.key).collect();
        for previous in saved.values() {
            if previous.total > 0
                && (scan.sessions.is_some() || !matches!(previous.key, LocalUsageKey::Session(_)))
                && !observed.contains(&previous.key)
            {
                // Absence in this complete scan is an observed zero; nullable metrics stay nullable.
                keep_higher(&transaction, previous, 0, &scan_id)?;
                summary.kept_higher += 1;
            }
        }
        transaction.execute("DELETE FROM local_usage_scans WHERE latest=0 AND id NOT IN (SELECT scan_id FROM local_usage_records UNION SELECT scan_id FROM local_usage_scan_differences)", []).map_err(|_| LocalLogError::WriteFailed)?;
        transaction
            .commit()
            .map_err(|_| LocalLogError::WriteFailed)?;
        Ok(summary)
    }
}
fn read_differences(
    connection: &Connection,
    schema: &str,
    tool_id: &str,
) -> Result<Vec<LocalScanDifference>> {
    let mut statement = connection.prepare(&format!("SELECT r.kind,r.period_key,d.saved_total_tokens,d.new_total_tokens,d.scan_id,r.total_tokens FROM {schema}.local_usage_scan_differences d JOIN {schema}.local_usage_records r ON r.id=d.record_id WHERE d.tool=?1 ORDER BY r.kind,r.period_key,d.saved_total_tokens,d.new_total_tokens")).map_err(|_| LocalLogError::ReadFailed)?;
    let mut rows = statement
        .query([tool_id])
        .map_err(|_| LocalLogError::ReadFailed)?;
    let mut result = Vec::new();
    while let Some(row) = rows.next().map_err(|_| LocalLogError::ReadFailed)? {
        let get = |i| {
            row.get::<_, String>(i)
                .map_err(|_| LocalLogError::ReadFailed)
        };
        let saved = decimal(&get(2)?)?;
        let new = decimal(&get(3)?)?;
        if new >= saved || saved > decimal(&get(5)?)? {
            return Err(LocalLogError::ReadFailed);
        }
        result.push(LocalScanDifference {
            key: key(&get(0)?, get(1)?)?,
            saved_total_tokens: saved,
            new_total_tokens: new,
            scan: provenance(connection, schema, &get(4)?)?,
        });
    }
    Ok(result)
}
/// Backup validation applies all write rules to trusted-schema rows, including nullable values.
pub(crate) fn validate(
    connection: &Connection,
    schema: &str,
) -> std::result::Result<(), crate::BackupValidationError> {
    let check = || -> Result<()> {
        if !matches!(schema, "main" | "openusage_checkpoint") {
            return Err(LocalLogError::ReadFailed);
        }
        let mut scopes = connection
            .prepare(&format!(
                "SELECT tool,period_kind,time_zone FROM {schema}.local_usage_scopes"
            ))
            .map_err(|_| LocalLogError::ReadFailed)?;
        let mut rows = scopes.query([]).map_err(|_| LocalLogError::ReadFailed)?;
        while let Some(row) = rows.next().map_err(|_| LocalLogError::ReadFailed)? {
            let id: String = row.get(0).map_err(|_| LocalLogError::ReadFailed)?;
            let kind: String = row.get(1).map_err(|_| LocalLogError::ReadFailed)?;
            let zone: Option<String> = row.get(2).map_err(|_| LocalLogError::ReadFailed)?;
            if period_kind(&id)? != kind
                || if kind == "day" {
                    zone.as_deref().is_none_or(|z| !valid_time_zone(z))
                } else {
                    zone.is_some()
                }
            {
                return Err(LocalLogError::ReadFailed);
            }
            let latest: i64 = connection
                .query_row(
                    &format!(
                        "SELECT COUNT(*) FROM {schema}.local_usage_scans WHERE tool=?1 AND latest=1"
                    ),
                    [&id],
                    |r| r.get(0),
                )
                .map_err(|_| LocalLogError::ReadFailed)?;
            if latest != 1 {
                return Err(LocalLogError::ReadFailed);
            }
            read_records(connection, schema, &id)?;
            read_differences(connection, schema, &id)?;
        }
        let mut scans = connection
            .prepare(&format!("SELECT id FROM {schema}.local_usage_scans"))
            .map_err(|_| LocalLogError::ReadFailed)?;
        let mut rows = scans.query([]).map_err(|_| LocalLogError::ReadFailed)?;
        while let Some(row) = rows.next().map_err(|_| LocalLogError::ReadFailed)? {
            let id: String = row.get(0).map_err(|_| LocalLogError::ReadFailed)?;
            provenance(connection, schema, &id)?;
        }
        Ok(())
    };
    check().map_err(|_| crate::BackupValidationError::InvalidDatabase)
}
#[cfg(test)]
pub(crate) fn drop_tables_for_legacy_fixture(connection: &Connection, schema: &str) {
    assert!(matches!(schema, "main" | "old_fixture"));
    for (table, _) in TABLES.iter().rev() {
        connection
            .execute_batch(&format!("DROP TABLE {schema}.{table}"))
            .unwrap();
    }
}
#[cfg(test)]
mod tests;
