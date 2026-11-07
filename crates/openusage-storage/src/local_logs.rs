//! Saved local log totals (SRS Section 5C). A scan can raise a day total but never lower it.
//! A lower result is saved as a difference beside the kept total (OU-LOG-011).
//! The first saved scan of a source pins its zone for day boundaries (OU-LOG-010).

use crate::backup_validation::canonical_uuid;
use crate::{Storage, activity, valid_time_zone};
use rusqlite::{OptionalExtension, Row, TransactionBehavior, params};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path};

/// A bound well above any real day, so that later sums cannot overflow.
const MAX_COUNT: u64 = 1_000_000_000_000_000;
const MAX_ROWS: usize = 200_000;
const MAX_SOURCES: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogConsent {
    Granted,
    Withdrawn,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogSource {
    pub id: String,
    pub tool: String,
    pub folder: String,
    pub consent: LogConsent,
    /// None until the first saved scan pins it.
    pub time_zone: Option<String>,
    pub last_scan_at_utc: Option<String>,
}

/// One day of one tool and one model, as one scan read it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalUsageRow {
    pub local_date: String,
    pub tool: String,
    pub model: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_write_tokens: u64,
    pub cache_read_tokens: u64,
    pub total_tokens: u64,
    /// None when ccusage has no price for the model (OU-LOG-013).
    pub estimated_value_usd_micros: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalUsageCoverage {
    Complete,
    /// A later scan read less, so the saved counts stay and the difference is saved.
    LogsGone,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalUsageDay {
    pub log_source_id: String,
    pub counts: LocalUsageRow,
    pub scanner_version: String,
    pub coverage: LocalUsageCoverage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalUsageDifference {
    pub log_source_id: String,
    pub local_date: String,
    pub model: String,
    pub saved_total_tokens: u64,
    pub new_total_tokens: u64,
    pub scan_at_utc: String,
}

pub struct LocalUsageScan<'a> {
    /// The granted sources that this run read, at most one for each tool.
    pub log_source_ids: &'a [String],
    pub scanner_version: &'a str,
    pub time_zone: &'a str,
    /// The first day that the scan read. None means every day, so a saved day that the scan lacks lost its logs.
    pub since: Option<&'a str>,
    pub scanned_at_utc: &'a str,
    pub rows: &'a [LocalUsageRow],
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LocalUsageScanSummary {
    pub inserted: usize,
    pub raised: usize,
    pub unchanged: usize,
    /// Days whose saved total stays because this scan read less (OU-LOG-011).
    pub kept_higher: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalLogError {
    InvalidInput,
    /// A source in the scan is unknown or its consent is withdrawn, so nothing is stored.
    NotConsented,
    /// The scan used another zone than the one that its source pinned.
    TimeZoneMismatch,
    ReadFailed,
    WriteFailed,
}

type Result<T> = std::result::Result<T, LocalLogError>;

fn valid_tool(tool: &str) -> bool {
    (1..=32).contains(&tool.len())
        && tool
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

fn valid_model(model: &str) -> bool {
    (1..=128).contains(&model.len())
        && model
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._:/@+-".contains(&b))
}

/// Folder text from storage. A backup from another platform keeps a path that this platform cannot parse.
fn valid_folder_text(folder: &str) -> bool {
    (1..=4096).contains(&folder.len()) && !folder.chars().any(char::is_control)
}

/// An absolute path in normal form. The caller resolves links before it saves the folder (OU-DUP-008).
fn valid_folder(folder: &str) -> bool {
    let path = Path::new(folder);
    valid_folder_text(folder)
        && path.is_absolute()
        && path
            .components()
            .all(|c| !matches!(c, Component::CurDir | Component::ParentDir))
        && path
            .components()
            .collect::<std::path::PathBuf>()
            .as_os_str()
            == path.as_os_str()
}

fn valid_version(version: &str) -> bool {
    (1..=32).contains(&version.len()) && version.bytes().all(|b| b.is_ascii_digit() || b == b'.')
}

fn valid_row(row: &LocalUsageRow) -> bool {
    let sum = [
        row.output_tokens,
        row.cache_write_tokens,
        row.cache_read_tokens,
    ]
    .into_iter()
    .try_fold(row.input_tokens, u64::checked_add);
    activity::activity_date(&row.local_date)
        && valid_tool(&row.tool)
        && valid_model(&row.model)
        && sum == Some(row.total_tokens)
        && row.total_tokens <= MAX_COUNT
        && row
            .estimated_value_usd_micros
            .is_none_or(|v| v <= MAX_COUNT)
}

fn token(text: &str) -> Option<u64> {
    activity::activity_decimal(text)
        .then(|| text.parse::<u64>().ok())
        .flatten()
        .filter(|v| *v <= MAX_COUNT)
}

fn optional_utc(value: Option<&str>) -> bool {
    value.is_none_or(activity::activity_utc_timestamp)
}

fn consent_text(consent: LogConsent) -> &'static str {
    match consent {
        LogConsent::Granted => "granted",
        LogConsent::Withdrawn => "withdrawn",
    }
}

pub(crate) const SOURCE_COLUMNS: &str =
    "id,tool,folder,consent,time_zone,last_scan_at_utc,created_at_utc,updated_at_utc";
pub(crate) const DAY_COLUMNS: &str = "id,log_source_id,tool,model,local_date,input_tokens,output_tokens,cache_write_tokens,cache_read_tokens,total_tokens,estimated_value_usd_micros,scanner_version,coverage,created_at_utc,updated_at_utc";
pub(crate) const DIFFERENCE_COLUMNS: &str =
    "id,log_source_id,local_date,model,saved_total_tokens,new_total_tokens,scan_at_utc";

/// Reads one source row in `SOURCE_COLUMNS` order. None means the row breaks a write rule.
fn read_source(row: &Row<'_>) -> rusqlite::Result<Option<LogSource>> {
    let (id, tool, folder, consent): (String, String, String, String) =
        (row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?);
    let (time_zone, last_scan_at_utc): (Option<String>, Option<String>) =
        (row.get(4)?, row.get(5)?);
    let (created, updated): (String, String) = (row.get(6)?, row.get(7)?);
    let consent = match consent.as_str() {
        "granted" => LogConsent::Granted,
        "withdrawn" => LogConsent::Withdrawn,
        _ => return Ok(None),
    };
    let valid = canonical_uuid(&id)
        && valid_tool(&tool)
        && valid_folder_text(&folder)
        && time_zone.as_deref().is_none_or(valid_time_zone)
        && optional_utc(last_scan_at_utc.as_deref())
        && activity::activity_utc_timestamp(&created)
        && activity::activity_utc_timestamp(&updated);
    Ok(valid.then_some(LogSource {
        id,
        tool,
        folder,
        consent,
        time_zone,
        last_scan_at_utc,
    }))
}

/// Reads one day row in `DAY_COLUMNS` order. None means the row breaks a write rule.
fn read_day(row: &Row<'_>) -> rusqlite::Result<Option<LocalUsageDay>> {
    let text = |index: usize| row.get::<_, String>(index);
    let count = |index: usize| text(index).map(|v| token(&v));
    let (
        Some(input_tokens),
        Some(output_tokens),
        Some(cache_write_tokens),
        Some(cache_read_tokens),
        Some(total_tokens),
    ) = (count(5)?, count(6)?, count(7)?, count(8)?, count(9)?)
    else {
        return Ok(None);
    };
    let estimated_value_usd_micros = match row.get::<_, Option<String>>(10)? {
        None => None,
        Some(text) => match token(&text) {
            None => return Ok(None),
            value => value,
        },
    };
    let coverage = match text(12)?.as_str() {
        "complete" => LocalUsageCoverage::Complete,
        "logs_gone" => LocalUsageCoverage::LogsGone,
        _ => return Ok(None),
    };
    let (id, log_source_id, scanner_version) = (text(0)?, text(1)?, text(11)?);
    let counts = LocalUsageRow {
        local_date: text(4)?,
        tool: text(2)?,
        model: text(3)?,
        input_tokens,
        output_tokens,
        cache_write_tokens,
        cache_read_tokens,
        total_tokens,
        estimated_value_usd_micros,
    };
    let valid = canonical_uuid(&id)
        && canonical_uuid(&log_source_id)
        && valid_row(&counts)
        && valid_version(&scanner_version)
        && activity::activity_utc_timestamp(&text(13)?)
        && activity::activity_utc_timestamp(&text(14)?);
    Ok(valid.then_some(LocalUsageDay {
        log_source_id,
        counts,
        scanner_version,
        coverage,
    }))
}

/// Reads one difference row in `DIFFERENCE_COLUMNS` order. None means the row breaks a write rule.
fn read_difference(row: &Row<'_>) -> rusqlite::Result<Option<LocalUsageDifference>> {
    let text = |index: usize| row.get::<_, String>(index);
    let (id, log_source_id, local_date, model) = (text(0)?, text(1)?, text(2)?, text(3)?);
    let (saved, new, scan_at_utc) = (token(&text(4)?), token(&text(5)?), text(6)?);
    let (Some(saved_total_tokens), Some(new_total_tokens)) = (saved, new) else {
        return Ok(None);
    };
    let valid = canonical_uuid(&id)
        && canonical_uuid(&log_source_id)
        && activity::activity_date(&local_date)
        && valid_model(&model)
        && new_total_tokens < saved_total_tokens
        && activity::activity_utc_timestamp(&scan_at_utc);
    Ok(valid.then_some(LocalUsageDifference {
        log_source_id,
        local_date,
        model,
        saved_total_tokens,
        new_total_tokens,
        scan_at_utc,
    }))
}

/// Checks backup and checkpoint contents with the same rules as a write.
/// The caller checks foreign keys for the whole database.
pub(crate) fn validate(
    connection: &rusqlite::Connection,
    schema: &str,
) -> std::result::Result<(), crate::BackupValidationError> {
    if !matches!(schema, "main" | "openusage_checkpoint")
        || !all_valid(
            connection,
            schema,
            "log_sources",
            SOURCE_COLUMNS,
            read_source,
        )
        || !all_valid(
            connection,
            schema,
            "local_usage_days",
            DAY_COLUMNS,
            read_day,
        )
        || !all_valid(
            connection,
            schema,
            "local_usage_differences",
            DIFFERENCE_COLUMNS,
            read_difference,
        )
    {
        return Err(crate::BackupValidationError::InvalidDatabase);
    }
    Ok(())
}

fn all_valid<T>(
    connection: &rusqlite::Connection,
    schema: &str,
    table: &str,
    columns: &str,
    read: fn(&Row<'_>) -> rusqlite::Result<Option<T>>,
) -> bool {
    let Ok(mut statement) = connection.prepare(&format!("SELECT {columns} FROM {schema}.{table}"))
    else {
        return false;
    };
    let Ok(mut rows) = statement.query([]) else {
        return false;
    };
    loop {
        match rows.next() {
            Ok(None) => return true,
            Ok(Some(row)) if matches!(read(row), Ok(Some(_))) => {}
            _ => return false,
        }
    }
}

impl Storage {
    /// Grants or withdraws consent for one tool folder (OU-LOG-001, OU-LOG-002).
    /// A withdrawn source keeps its saved totals.
    pub fn set_log_source(
        &mut self,
        tool: &str,
        folder: &str,
        consent: LogConsent,
        now_utc: &str,
    ) -> Result<LogSource> {
        if !valid_tool(tool) || !valid_folder(folder) || !activity::activity_utc_timestamp(now_utc)
        {
            return Err(LocalLogError::InvalidInput);
        }
        let id = activity::new_uuid().map_err(|_| LocalLogError::WriteFailed)?;
        self.connection
            .execute(
                "INSERT INTO log_sources(id,tool,folder,consent,time_zone,last_scan_at_utc,created_at_utc,updated_at_utc)
                 VALUES(?1,?2,?3,?4,NULL,NULL,?5,?5)
                 ON CONFLICT(tool,folder) DO UPDATE SET consent=excluded.consent,updated_at_utc=excluded.updated_at_utc",
                params![id, tool, folder, consent_text(consent), now_utc],
            )
            .map_err(|_| LocalLogError::WriteFailed)?;
        self.log_sources()?
            .into_iter()
            .find(|source| source.tool == tool && source.folder == folder)
            .ok_or(LocalLogError::ReadFailed)
    }

    /// The zone of the saved days: the first pinned source zone, else the display zone (OU-LOG-010).
    pub fn local_usage_zone(&self) -> Result<String> {
        let pinned = self
            .log_sources()?
            .into_iter()
            .find_map(|source| source.time_zone);
        match pinned {
            Some(zone) => Ok(zone),
            None => self.time_zone().map_err(|_| LocalLogError::ReadFailed),
        }
    }

    /// All sources, ordered by tool and folder.
    pub fn log_sources(&self) -> Result<Vec<LogSource>> {
        let mut statement = self
            .connection
            .prepare(&format!(
                "SELECT {SOURCE_COLUMNS} FROM log_sources ORDER BY tool,folder"
            ))
            .map_err(|_| LocalLogError::ReadFailed)?;
        let rows = statement
            .query_map([], read_source)
            .map_err(|_| LocalLogError::ReadFailed)?;
        rows.map(|row| row.ok().flatten().ok_or(LocalLogError::ReadFailed))
            .collect()
    }

    /// Saves one scan in one transaction (SRS Section 18.3). Each day is new, rises, stays,
    /// or keeps its higher saved total and saves the difference (OU-LOG-011).
    pub fn save_local_usage_scan(
        &mut self,
        scan: &LocalUsageScan<'_>,
    ) -> Result<LocalUsageScanSummary> {
        let unique_sources: BTreeSet<&String> = scan.log_source_ids.iter().collect();
        if !valid_version(scan.scanner_version)
            || !valid_time_zone(scan.time_zone)
            || scan
                .since
                .is_some_and(|since| !activity::activity_date(since))
            || !activity::activity_utc_timestamp(scan.scanned_at_utc)
            || scan.rows.len() > MAX_ROWS
            || scan.log_source_ids.is_empty()
            || scan.log_source_ids.len() > MAX_SOURCES
            || unique_sources.len() != scan.log_source_ids.len()
            || scan.log_source_ids.iter().any(|id| !canonical_uuid(id))
        {
            return Err(LocalLogError::InvalidInput);
        }
        let mut keys = BTreeSet::new();
        for row in scan.rows {
            if !valid_row(row) || !keys.insert((&row.tool, &row.local_date, &row.model)) {
                return Err(LocalLogError::InvalidInput);
            }
        }
        let write = |_| LocalLogError::WriteFailed;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(write)?;
        // Tool name to source ID, for the sources that this run read.
        let mut sources = BTreeMap::new();
        for id in scan.log_source_ids {
            let source: Option<(String, String, Option<String>)> = tx
                .query_row(
                    "SELECT tool,consent,time_zone FROM log_sources WHERE id=?1",
                    [id],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                )
                .optional()
                .map_err(|_| LocalLogError::ReadFailed)?;
            let Some((tool, consent, zone)) = source else {
                return Err(LocalLogError::NotConsented);
            };
            if consent != "granted" {
                return Err(LocalLogError::NotConsented);
            }
            if zone.is_some_and(|zone| zone != scan.time_zone) {
                return Err(LocalLogError::TimeZoneMismatch);
            }
            if sources.insert(tool, id.as_str()).is_some() {
                return Err(LocalLogError::InvalidInput);
            }
        }
        let mut summary = LocalUsageScanSummary::default();
        let save_difference = |source: &str,
                               date: &str,
                               model: &str,
                               saved: u64,
                               new: u64|
         -> Result<()> {
            let id = activity::new_uuid().map_err(|_| LocalLogError::WriteFailed)?;
            tx.execute(
                    "INSERT INTO local_usage_differences(id,log_source_id,local_date,model,saved_total_tokens,new_total_tokens,scan_at_utc)
                     VALUES(?1,?2,?3,?4,?5,?6,?7)
                     ON CONFLICT(log_source_id,local_date,model,saved_total_tokens,new_total_tokens) DO NOTHING",
                    params![id, source, date, model, saved.to_string(), new.to_string(), scan.scanned_at_utc],
                )
                .map_err(write)?;
            tx.execute(
                "UPDATE local_usage_days SET coverage='logs_gone',updated_at_utc=?4
                     WHERE log_source_id=?1 AND local_date=?2 AND model=?3",
                params![source, date, model, scan.scanned_at_utc],
            )
            .map_err(write)?;
            Ok(())
        };
        for row in scan.rows {
            let source = *sources.get(&row.tool).ok_or(LocalLogError::InvalidInput)?;
            let saved: Option<String> = tx
                .query_row(
                    "SELECT total_tokens FROM local_usage_days WHERE log_source_id=?1 AND local_date=?2 AND model=?3",
                    params![source, row.local_date, row.model],
                    |r| r.get(0),
                )
                .optional()
                .map_err(|_| LocalLogError::ReadFailed)?;
            let saved = saved
                .map(|text| token(&text).ok_or(LocalLogError::ReadFailed))
                .transpose()?;
            let (input, output, cache_write, cache_read, total) = (
                row.input_tokens.to_string(),
                row.output_tokens.to_string(),
                row.cache_write_tokens.to_string(),
                row.cache_read_tokens.to_string(),
                row.total_tokens.to_string(),
            );
            let estimate = row.estimated_value_usd_micros.map(|v| v.to_string());
            match saved {
                None => {
                    let id = activity::new_uuid().map_err(|_| LocalLogError::WriteFailed)?;
                    tx.execute(
                        "INSERT INTO local_usage_days(id,log_source_id,tool,model,local_date,input_tokens,output_tokens,cache_write_tokens,cache_read_tokens,total_tokens,estimated_value_usd_micros,scanner_version,coverage,created_at_utc,updated_at_utc)
                         VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,'complete',?13,?13)",
                        params![id, source, row.tool, row.model, row.local_date, input, output, cache_write, cache_read, total, estimate, scan.scanner_version, scan.scanned_at_utc],
                    )
                    .map_err(write)?;
                    summary.inserted += 1;
                }
                Some(saved) if row.total_tokens >= saved => {
                    tx.execute(
                        "UPDATE local_usage_days SET input_tokens=?4,output_tokens=?5,cache_write_tokens=?6,cache_read_tokens=?7,total_tokens=?8,
                         estimated_value_usd_micros=?9,scanner_version=?10,coverage='complete',updated_at_utc=?11
                         WHERE log_source_id=?1 AND local_date=?2 AND model=?3",
                        params![source, row.local_date, row.model, input, output, cache_write, cache_read, total, estimate, scan.scanner_version, scan.scanned_at_utc],
                    )
                    .map_err(write)?;
                    if row.total_tokens > saved {
                        summary.raised += 1;
                    } else {
                        summary.unchanged += 1;
                    }
                }
                Some(saved) => {
                    save_difference(source, &row.local_date, &row.model, saved, row.total_tokens)?;
                    summary.kept_higher += 1;
                }
            }
        }
        // A saved day inside the scanned range that the scan did not read lost its logs.
        for (tool, source) in &sources {
            let saved: Vec<(String, String, String)> = {
                let mut statement = tx
                    .prepare(
                        "SELECT local_date,model,total_tokens FROM local_usage_days
                         WHERE log_source_id=?1 AND local_date>=?2 ORDER BY local_date,model",
                    )
                    .map_err(|_| LocalLogError::ReadFailed)?;
                statement
                    .query_map(params![source, scan.since.unwrap_or("")], |r| {
                        Ok((r.get(0)?, r.get(1)?, r.get(2)?))
                    })
                    .and_then(|rows| rows.collect())
                    .map_err(|_| LocalLogError::ReadFailed)?
            };
            for (date, model, total) in saved {
                let total = token(&total).ok_or(LocalLogError::ReadFailed)?;
                if total > 0 && !keys.contains(&(tool, &date, &model)) {
                    save_difference(source, &date, &model, total, 0)?;
                    summary.kept_higher += 1;
                }
            }
        }
        for source in sources.values() {
            tx.execute(
                "UPDATE log_sources SET time_zone=coalesce(time_zone,?2),last_scan_at_utc=?3,updated_at_utc=?3 WHERE id=?1",
                params![source, scan.time_zone, scan.scanned_at_utc],
            )
            .map_err(write)?;
        }
        tx.commit().map_err(write)?;
        Ok(summary)
    }

    /// Saved days from `since` to `until`, inclusive, ordered by date, tool, model and source.
    pub fn local_usage_days(&self, since: &str, until: &str) -> Result<Vec<LocalUsageDay>> {
        self.read_range(
            &format!(
                "SELECT {DAY_COLUMNS} FROM local_usage_days WHERE local_date BETWEEN ?1 AND ?2
                 ORDER BY local_date,tool,model,log_source_id"
            ),
            since,
            until,
            read_day,
        )
    }

    /// Saved differences from `since` to `until`, inclusive, ordered by date, model, source and scan time.
    pub fn local_usage_differences(
        &self,
        since: &str,
        until: &str,
    ) -> Result<Vec<LocalUsageDifference>> {
        self.read_range(
            &format!(
                "SELECT {DIFFERENCE_COLUMNS} FROM local_usage_differences WHERE local_date BETWEEN ?1 AND ?2
                 ORDER BY local_date,model,log_source_id,scan_at_utc,id"
            ),
            since,
            until,
            read_difference,
        )
    }

    fn read_range<T>(
        &self,
        sql: &str,
        since: &str,
        until: &str,
        read: fn(&Row<'_>) -> rusqlite::Result<Option<T>>,
    ) -> Result<Vec<T>> {
        if !activity::activity_date(since) || !activity::activity_date(until) || since > until {
            return Err(LocalLogError::InvalidInput);
        }
        let mut statement = self
            .connection
            .prepare(sql)
            .map_err(|_| LocalLogError::ReadFailed)?;
        let rows = statement
            .query_map(params![since, until], read)
            .map_err(|_| LocalLogError::ReadFailed)?;
        rows.map(|row| row.ok().flatten().ok_or(LocalLogError::ReadFailed))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CURRENT_SCHEMA_VERSION, backup_validation::validate_connection};

    const NOW: &str = "2026-11-07T08:00:00.000Z";
    const LATER: &str = "2026-11-07T09:00:00.000Z";
    const CLAUDE_HOME: &str = "/home/synthetic/.claude";
    const CODEX_HOME: &str = "/home/synthetic/.codex";
    const KEY: [u8; 32] = [51; 32];

    fn row(date: &str, tool: &str, model: &str, input: u64, value: Option<u64>) -> LocalUsageRow {
        LocalUsageRow {
            local_date: date.into(),
            tool: tool.into(),
            model: model.into(),
            input_tokens: input,
            output_tokens: 5,
            cache_write_tokens: 2,
            cache_read_tokens: 1,
            total_tokens: input + 8,
            estimated_value_usd_micros: value,
        }
    }

    fn scan<'a>(
        sources: &'a [String],
        zone: &'a str,
        since: Option<&'a str>,
        rows: &'a [LocalUsageRow],
    ) -> LocalUsageScan<'a> {
        LocalUsageScan {
            log_source_ids: sources,
            scanner_version: "20.0.48",
            time_zone: zone,
            since,
            scanned_at_utc: LATER,
            rows,
        }
    }

    fn setup() -> (tempfile::TempDir, Storage, String, String) {
        let dir = crate::tests::private_directory();
        let mut storage = Storage::create(&dir.path().join("logs.db"), &KEY).unwrap();
        let claude = storage
            .set_log_source("claude", CLAUDE_HOME, LogConsent::Granted, NOW)
            .unwrap()
            .id;
        let codex = storage
            .set_log_source("codex", CODEX_HOME, LogConsent::Granted, NOW)
            .unwrap()
            .id;
        (dir, storage, claude, codex)
    }

    fn totals(storage: &Storage) -> Vec<(String, String, u64, Option<u64>, LocalUsageCoverage)> {
        storage
            .local_usage_days("2026-01-01", "2026-12-31")
            .unwrap()
            .into_iter()
            .map(|d| {
                (
                    d.counts.local_date,
                    d.counts.tool,
                    d.counts.total_tokens,
                    d.counts.estimated_value_usd_micros,
                    d.coverage,
                )
            })
            .collect()
    }

    fn differences(storage: &Storage) -> Vec<(String, u64, u64)> {
        storage
            .local_usage_differences("2026-01-01", "2026-12-31")
            .unwrap()
            .into_iter()
            .map(|d| (d.local_date, d.saved_total_tokens, d.new_total_tokens))
            .collect()
    }

    #[test]
    fn consent_and_the_pinned_zone_gate_every_scan() {
        let (_dir, mut storage, claude, codex) = setup();
        let rows = [row("2026-11-05", "claude", "claude-sonnet-4", 10, Some(30))];
        let unknown = ["aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa".to_string()];
        assert_eq!(
            storage.save_local_usage_scan(&scan(&unknown, "Asia/Kolkata", None, &rows)),
            Err(LocalLogError::NotConsented)
        );
        let only_claude = [claude.clone()];
        // The first saved scan pins the zone. A later display zone cannot move saved days (AT-68).
        storage
            .save_local_usage_scan(&scan(&only_claude, "Asia/Kolkata", None, &rows))
            .unwrap();
        assert_eq!(
            storage.save_local_usage_scan(&scan(&only_claude, "UTC", None, &rows)),
            Err(LocalLogError::TimeZoneMismatch)
        );
        // A row needs a consented source of its own tool in the same run.
        let codex_row = [row("2026-11-05", "codex", "gpt-5-codex", 1, None)];
        assert_eq!(
            storage.save_local_usage_scan(&scan(&only_claude, "Asia/Kolkata", None, &codex_row)),
            Err(LocalLogError::InvalidInput)
        );
        // Withdrawn consent stops scans and keeps saved totals. Granting again keeps the source and its zone.
        storage
            .set_log_source("claude", CLAUDE_HOME, LogConsent::Withdrawn, LATER)
            .unwrap();
        assert_eq!(
            storage.save_local_usage_scan(&scan(&only_claude, "Asia/Kolkata", None, &rows)),
            Err(LocalLogError::NotConsented)
        );
        assert_eq!(totals(&storage).len(), 1);
        let granted = storage
            .set_log_source("claude", CLAUDE_HOME, LogConsent::Granted, LATER)
            .unwrap();
        assert_eq!(
            (granted.id.as_str(), granted.time_zone.as_deref()),
            (claude.as_str(), Some("Asia/Kolkata"))
        );
        assert_eq!(
            storage
                .log_sources()
                .unwrap()
                .iter()
                .map(|s| (s.tool.as_str(), s.time_zone.as_deref()))
                .collect::<Vec<_>>(),
            [("claude", Some("Asia/Kolkata")), ("codex", None)]
        );
        // Two sources of one tool cannot share a run.
        let second = storage
            .set_log_source("codex", "/srv/other/.codex", LogConsent::Granted, NOW)
            .unwrap()
            .id;
        assert_eq!(
            storage.save_local_usage_scan(&scan(&[codex, second], "UTC", None, &[])),
            Err(LocalLogError::InvalidInput)
        );
        for folder in [
            "",
            "relative/.claude",
            "/a/../b",
            "/a/./b",
            "/a/",
            "/a//b",
            "/a/\nb",
        ] {
            assert_eq!(
                storage.set_log_source("claude", folder, LogConsent::Granted, NOW),
                Err(LocalLogError::InvalidInput),
                "{folder:?}"
            );
        }
        assert_eq!(
            storage.set_log_source("Claude Code", CLAUDE_HOME, LogConsent::Granted, NOW),
            Err(LocalLogError::InvalidInput)
        );
    }

    #[test]
    fn a_scan_raises_a_total_but_never_lowers_it_and_saves_the_difference() {
        let (dir, mut storage, claude, codex) = setup();
        let both = [claude.clone(), codex.clone()];
        let first = [
            row("2026-11-05", "claude", "claude-sonnet-4", 142, Some(900)),
            row("2026-11-06", "claude", "claude-sonnet-4", 32, Some(200)),
            row("2026-11-05", "codex", "gpt-5-codex", 42, None),
        ];
        assert_eq!(
            storage
                .save_local_usage_scan(&scan(&both, "UTC", None, &first))
                .unwrap(),
            LocalUsageScanSummary {
                inserted: 3,
                ..Default::default()
            }
        );
        let saved = totals(&storage);
        // The same logs give the same totals (AT-66).
        assert_eq!(
            storage
                .save_local_usage_scan(&scan(&both, "UTC", None, &first))
                .unwrap(),
            LocalUsageScanSummary {
                unchanged: 3,
                ..Default::default()
            }
        );
        assert_eq!(totals(&storage), saved);
        // Part of the 5 November Claude log and all of 6 November are deleted; the Codex day grew (AT-67).
        let second = [
            row("2026-11-05", "claude", "claude-sonnet-4", 72, Some(400)),
            row("2026-11-05", "codex", "gpt-5-codex", 62, None),
        ];
        let expected = LocalUsageScanSummary {
            raised: 1,
            kept_higher: 2,
            ..Default::default()
        };
        assert_eq!(
            storage
                .save_local_usage_scan(&scan(&both, "UTC", None, &second))
                .unwrap(),
            expected
        );
        let gone = LocalUsageCoverage::LogsGone;
        let complete = LocalUsageCoverage::Complete;
        let after = vec![
            ("2026-11-05".into(), "claude".into(), 150, Some(900), gone),
            ("2026-11-05".into(), "codex".into(), 70, None, complete),
            ("2026-11-06".into(), "claude".into(), 40, Some(200), gone),
        ];
        assert_eq!(totals(&storage), after);
        let saved_differences = vec![("2026-11-05".into(), 150, 80), ("2026-11-06".into(), 40, 0)];
        assert_eq!(differences(&storage), saved_differences);
        // A repeated lower scan saves no second copy.
        let expected = LocalUsageScanSummary {
            unchanged: 1,
            kept_higher: 2,
            ..Default::default()
        };
        assert_eq!(
            storage
                .save_local_usage_scan(&scan(&both, "UTC", None, &second))
                .unwrap(),
            expected
        );
        assert_eq!(differences(&storage), saved_differences);
        // A scan from 6 November does not judge earlier days.
        assert_eq!(
            storage
                .save_local_usage_scan(&scan(
                    std::slice::from_ref(&codex),
                    "UTC",
                    Some("2026-11-06"),
                    &[]
                ))
                .unwrap(),
            LocalUsageScanSummary::default()
        );
        assert_eq!(totals(&storage), after);
        // New usage above the saved total raises it and clears the mark.
        let raised = [row(
            "2026-11-05",
            "claude",
            "claude-sonnet-4",
            192,
            Some(1200),
        )];
        storage
            .save_local_usage_scan(&scan(&[claude], "UTC", Some("2026-11-05"), &raised))
            .unwrap();
        assert_eq!(
            totals(&storage)[0],
            (
                "2026-11-05".into(),
                "claude".into(),
                200,
                Some(1200),
                complete
            )
        );
        let before = (totals(&storage), differences(&storage));
        drop(storage);
        let storage = Storage::open(&dir.path().join("logs.db"), &KEY).unwrap();
        assert_eq!((totals(&storage), differences(&storage)), before);
        validate_connection(&storage.connection, CURRENT_SCHEMA_VERSION).unwrap();
    }

    #[test]
    fn backup_checkpoint_restore_and_export_keep_local_usage() {
        let (dir, mut source, claude, codex) = setup();
        let both = [claude, codex];
        let first = [
            row("2026-11-05", "claude", "claude-sonnet-4", 142, Some(900)),
            row("2026-11-05", "codex", "unknown-model", 42, None),
        ];
        source
            .save_local_usage_scan(&scan(&both, "UTC", None, &first))
            .unwrap();
        source
            .save_local_usage_scan(&scan(&both, "UTC", None, &first[1..]))
            .unwrap();
        let expected = (
            totals(&source),
            differences(&source),
            source.log_sources().unwrap(),
        );
        // An unknown model keeps its tokens and has no estimate (AT-69).
        assert_eq!(expected.0[1].2, 50);
        assert_eq!(expected.0[1].3, None);
        let snapshot = source.backup_snapshot().unwrap();
        crate::validate_backup_snapshot(&snapshot.bytes, snapshot.schema_version).unwrap();
        let checkpoint = dir.path().join("checkpoint.db");
        source.create_encrypted_checkpoint(&checkpoint).unwrap();
        let reopened = Storage::open(&checkpoint, &KEY).unwrap();
        assert_eq!(
            (
                totals(&reopened),
                differences(&reopened),
                reopened.log_sources().unwrap()
            ),
            expected
        );
        // Restore replaces the target's own local usage with the backup's.
        let (_target_dir, mut target, target_claude, _) = setup();
        target
            .save_local_usage_scan(&scan(
                &[target_claude],
                "UTC",
                None,
                &[row("2026-10-01", "claude", "m", 1, None)],
            ))
            .unwrap();
        let revision = target.restore_state().unwrap().revision;
        target
            .replace_records_from_backup(&snapshot.bytes, snapshot.schema_version, &revision, &[])
            .unwrap();
        assert_eq!(
            (
                totals(&target),
                differences(&target),
                target.log_sources().unwrap()
            ),
            expected
        );
        let mut json = Vec::new();
        target.write_application_export(&mut json, LATER).unwrap();
        let document: serde_json::Value = serde_json::from_slice(&json).unwrap();
        let rows = |name: &str| {
            document["tables"]
                .as_array()
                .unwrap()
                .iter()
                .find(|table| table["name"] == name)
                .unwrap()["rows"]
                .as_array()
                .unwrap()
                .len()
        };
        assert_eq!(
            (
                rows("log_sources"),
                rows("local_usage_days"),
                rows("local_usage_differences")
            ),
            (2, 2, 1)
        );
    }

    #[test]
    fn invalid_rows_and_tampered_records_are_refused() {
        let (_dir, mut storage, claude, _) = setup();
        let only_claude = [claude];
        let mut wrong_total = row("2026-11-05", "claude", "m", 1, None);
        wrong_total.total_tokens = 5;
        let duplicate = row("2026-11-05", "claude", "m", 1, None);
        for rows in [
            vec![wrong_total],
            vec![row("2026-02-30", "claude", "m", 1, None)],
            // Log text never becomes markup or a path (AT-73, OU-SEC-021).
            vec![row("2026-11-05", "claude", "<b>marker</b>", 1, None)],
            vec![duplicate.clone(), duplicate],
            vec![row("2026-11-05", "claude", "m", MAX_COUNT, None)],
            vec![row("2026-11-05", "claude", "m", 1, Some(MAX_COUNT + 1))],
        ] {
            assert_eq!(
                storage.save_local_usage_scan(&scan(&only_claude, "UTC", None, &rows)),
                Err(LocalLogError::InvalidInput)
            );
        }
        let mut bad = scan(&only_claude, "UTC", Some("2026-13-01"), &[]);
        assert_eq!(
            storage.save_local_usage_scan(&bad),
            Err(LocalLogError::InvalidInput)
        );
        bad.since = None;
        bad.scanner_version = "20.0.48-beta";
        assert_eq!(
            storage.save_local_usage_scan(&bad),
            Err(LocalLogError::InvalidInput)
        );
        assert!(totals(&storage).is_empty());
        assert_eq!(storage.log_sources().unwrap()[0].time_zone, None);
        storage
            .save_local_usage_scan(&scan(
                &only_claude,
                "UTC",
                None,
                &[
                    row("2026-11-05", "claude", "m", 100, None),
                    row("2026-11-04", "claude", "n", 1, None),
                ],
            ))
            .unwrap();
        storage
            .save_local_usage_scan(&scan(
                &only_claude,
                "UTC",
                None,
                &[
                    row("2026-11-05", "claude", "m", 50, None),
                    row("2026-11-04", "claude", "n", 1, None),
                ],
            ))
            .unwrap();
        validate_connection(&storage.connection, CURRENT_SCHEMA_VERSION).unwrap();
        for bad in [
            "UPDATE local_usage_days SET total_tokens='0108'",
            "UPDATE local_usage_days SET input_tokens='1'",
            // Day n has no difference row, so only the date rule can refuse it.
            "UPDATE local_usage_days SET local_date='2026-02-30' WHERE model='n'",
            "UPDATE local_usage_days SET scanner_version='latest'",
            "UPDATE local_usage_days SET coverage='partial'",
            "UPDATE local_usage_days SET estimated_value_usd_micros='-1'",
            "UPDATE log_sources SET folder='/a'||char(10)||'b'",
            "UPDATE log_sources SET time_zone='Not/AZone '",
            "UPDATE log_sources SET consent='maybe'",
            "UPDATE local_usage_differences SET new_total_tokens=saved_total_tokens",
        ] {
            let tx = storage.connection.transaction().unwrap();
            tx.execute_batch(&format!(
                "PRAGMA ignore_check_constraints=ON; {bad}; PRAGMA ignore_check_constraints=OFF;"
            ))
            .unwrap();
            assert_eq!(
                validate_connection(&tx, CURRENT_SCHEMA_VERSION),
                Err(crate::BackupValidationError::InvalidDatabase),
                "{bad}"
            );
            drop(tx);
        }
        // A read refuses a stored value that breaks a write rule.
        storage
            .connection
            .execute_batch("PRAGMA ignore_check_constraints=ON; UPDATE local_usage_days SET total_tokens='0108'; PRAGMA ignore_check_constraints=OFF;")
            .unwrap();
        assert_eq!(
            storage.local_usage_days("2026-01-01", "2026-12-31"),
            Err(LocalLogError::ReadFailed)
        );
    }
}
