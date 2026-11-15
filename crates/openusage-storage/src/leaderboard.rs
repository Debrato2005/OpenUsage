//! Exact public aggregates from one saved SQLCipher snapshot.
use crate::Storage;
use jiff::{Span, Timestamp, civil::Date, tz::TimeZone};
use openusage_locallogs::{AgentPeriod, UsagePeriod};
use serde::Serialize;
use std::{collections::BTreeSet, fmt};

const MAX_COUNT: u64 = 1_000_000_000_000;
const MAX_ROWS: usize = 400;
const MAX_BODY: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BoardUploadError {
    InvalidSelection,
    Incomplete(&'static str),
    InvalidData(&'static str),
    TooLarge,
    ReadFailed,
}
impl fmt::Display for BoardUploadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidSelection => "A valid prospective device UUID and day count are required.",
            Self::Incomplete(message) | Self::InvalidData(message) => message,
            Self::TooLarge => {
                "The prospective upload exceeds the service limits. Select fewer days."
            }
            Self::ReadFailed => "OpenUsage could not read the saved local usage.",
        })
    }
}
impl std::error::Error for BoardUploadError {}
fn invalid(message: &'static str) -> BoardUploadError {
    BoardUploadError::InvalidData(message)
}
fn incomplete(message: &'static str) -> BoardUploadError {
    BoardUploadError::Incomplete(message)
}
fn too_large() -> BoardUploadError {
    BoardUploadError::TooLarge
}
fn read_failed() -> BoardUploadError {
    BoardUploadError::ReadFailed
}

fn uuid(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(i, b)| match i {
            8 | 13 | 18 | 23 => b == b'-',
            14 => b == b'4',
            19 => b"89ab".contains(&b),
            _ => b.is_ascii_digit() || (b'a'..=b'f').contains(&b),
        })
}

impl Storage {
    /// Returns the exact v1 upload bytes from one saved-data snapshot.
    /// This method reads only. It must run outside an existing transaction.
    /// Network, enrollment, provider readings, and collection stay outside this boundary.
    pub fn leaderboard_upload(
        &self,
        device_id: &str,
        days: i32,
        now: Timestamp,
    ) -> Result<String, BoardUploadError> {
        if !uuid(device_id) || !(1..=366).contains(&days) {
            return Err(BoardUploadError::InvalidSelection);
        }
        let transaction = self
            .connection
            .unchecked_transaction()
            .map_err(|_| read_failed())?;
        let body = serialize_sync(self, device_id, days, now)?;
        transaction.commit().map_err(|_| read_failed())?;
        Ok(body)
    }
}

/// The only upload fields. Future sync must send these serialized bytes unchanged.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SyncBody<'a> {
    device_id: &'a str,
    rows: Vec<SyncRow>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SyncRow {
    date: String,
    tool: String,
    model: String,
    input_tokens: u64,
    output_tokens: u64,
    cache_read_tokens: u64,
    cache_write_tokens: u64,
    total_tokens: u64,
    estimated_value_micros: Option<u64>,
}
impl SyncRow {
    fn validate(&self, today: Date) -> Result<(), BoardUploadError> {
        let date = self
            .date
            .parse::<Date>()
            .ok()
            .filter(|d| d.to_string() == self.date);
        let earliest = today
            .checked_sub(Span::new().days(400))
            .map_err(|_| invalid("The service date range is unavailable."))?;
        if date.is_none_or(|date| date < earliest || date > today) {
            return Err(invalid(
                "A saved local date falls outside the service UTC date range. Its pinned day zone cannot be changed.",
            ));
        }
        if !matches!(
            self.tool.as_str(),
            "claude"
                | "codex"
                | "opencode"
                | "amp"
                | "droid"
                | "codebuff"
                | "hermes"
                | "pi"
                | "goose"
                | "kilo"
                | "copilot"
                | "gemini"
                | "antigravity"
                | "kimi"
                | "qwen"
                | "openclaw"
                | "grok"
                | "zcode"
        ) || !(1..=64).contains(&self.model.len())
            || !self
                .model
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b".:-".contains(&b))
        {
            return Err(invalid(
                "A saved tool or model does not meet the service identifier rules.",
            ));
        }
        let parts = [
            self.input_tokens,
            self.output_tokens,
            self.cache_read_tokens,
            self.cache_write_tokens,
        ];
        if parts.iter().any(|n| *n > MAX_COUNT)
            || self.total_tokens > MAX_COUNT
            || self.estimated_value_micros.is_some_and(|n| n > MAX_COUNT)
            || parts.into_iter().try_fold(0u64, u64::checked_add) != Some(self.total_tokens)
        {
            return Err(invalid(
                "Saved token counts or estimates do not meet the service count rules.",
            ));
        }
        Ok(())
    }
}

fn range(now: Timestamp, zone: &str, days: i32) -> Result<(String, String), BoardUploadError> {
    let zone = TimeZone::get(zone).map_err(|_| invalid("A saved local day zone is invalid."))?;
    let end = now.to_zoned(zone).date();
    let start = end
        .checked_sub(Span::new().days(days - 1))
        .map_err(|_| invalid("The saved local date range is unavailable."))?;
    Ok((start.to_string(), end.to_string()))
}

fn grouped_rows(
    tool: &str,
    day: &str,
    period: &AgentPeriod,
) -> Result<Vec<SyncRow>, BoardUploadError> {
    if period.models.is_empty() {
        return Err(incomplete(
            "Saved local usage has no complete model attribution. No upload was produced.",
        ));
    }
    let mut rows = Vec::new();
    let mut total = 0u64;
    for model in &period.models {
        let details = &model.details;
        let (
            Some(input_tokens),
            Some(output_tokens),
            Some(cache_read_tokens),
            Some(cache_write_tokens),
            Some(total_tokens),
        ) = (
            details.input_tokens,
            details.output_tokens,
            details.cache_read_tokens,
            details.cache_write_tokens,
            model.total_tokens,
        )
        else {
            return Err(incomplete(
                "Saved model token categories are unavailable. No upload was produced.",
            ));
        };
        total = total
            .checked_add(total_tokens)
            .ok_or_else(|| invalid("Saved model token totals overflow."))?;
        rows.push(SyncRow {
            date: day.into(),
            tool: tool.into(),
            model: model.model.clone(),
            input_tokens,
            output_tokens,
            cache_read_tokens,
            cache_write_tokens,
            total_tokens,
            estimated_value_micros: model.estimated_value_micros,
        });
    }
    if total != period.total_tokens {
        return Err(incomplete(
            "Saved model totals do not cover the independent period total. No upload was produced.",
        ));
    }
    Ok(rows)
}

/// Reads only saved local aggregates. Enrollment and transport cannot change the request shape.
/// Errors contain no saved identifiers, paths, model text or account data.
fn serialize_sync(
    storage: &Storage,
    device_id: &str,
    days: i32,
    now: Timestamp,
) -> Result<String, BoardUploadError> {
    let mut rows = Vec::new();
    let mut grouped_tools = BTreeSet::new();
    for definition in openusage_locallogs::tools() {
        let Some(scope) = storage
            .local_tool_scope(&definition.id)
            .map_err(|_| read_failed())?
        else {
            continue;
        };
        grouped_tools.insert(scope.tool.clone());
        let periods = storage
            .local_periods(&scope.tool)
            .map_err(|_| read_failed())?;
        if periods
            .iter()
            .any(|p| p.period.period == UsagePeriod::Unplaced)
        {
            return Err(incomplete(
                "Saved local usage has an undated period. An observation time cannot supply its date.",
            ));
        }
        let Some(zone) = scope.time_zone.as_deref() else {
            continue;
        };
        let (from, to) = range(now, zone, days)?;
        for period in periods {
            let UsagePeriod::Day(day) = &period.period.period else {
                continue;
            };
            if day >= &from && day <= &to {
                rows.extend(grouped_rows(&scope.tool, day, &period.period)?);
                if rows.len() > MAX_ROWS {
                    return Err(too_large());
                }
            }
        }
    }
    // Legacy rows are coherent snapshots, but retained model maxima and multiple folders
    // cannot establish one non-overlapping total. Grouped and legacy generations never add.
    let sources = storage.log_sources().map_err(|_| read_failed())?;
    let utc = now.to_zoned(TimeZone::UTC).date();
    let broad_from = utc
        .checked_sub(Span::new().days(days))
        .map_err(|_| invalid("The local date range is unavailable."))?
        .to_string();
    let broad_to = utc
        .checked_add(Span::new().days(1))
        .map_err(|_| invalid("The local date range is unavailable."))?
        .to_string();
    let legacy = storage
        .local_usage_days(&broad_from, &broad_to)
        .map_err(|_| read_failed())?;
    let mut legacy_sources = std::collections::BTreeMap::new();
    for saved in legacy {
        let source = sources
            .iter()
            .find(|s| s.id == saved.log_source_id)
            .ok_or_else(read_failed)?;
        let zone = source
            .time_zone
            .as_deref()
            .ok_or_else(|| invalid("Saved legacy usage has no pinned day zone."))?;
        let (from, to) = range(now, zone, days)?;
        let row = saved.counts;
        if row.local_date < from || row.local_date > to {
            continue;
        }
        if grouped_tools.contains(&row.tool) {
            return Err(incomplete(
                "Grouped and legacy local usage overlap for one tool. No upload was produced.",
            ));
        }
        if legacy_sources
            .insert(row.tool.clone(), saved.log_source_id.clone())
            .is_some_and(|id| id != saved.log_source_id)
        {
            return Err(incomplete(
                "Multiple saved folders can overlap for one tool. No upload was produced.",
            ));
        }
        if saved.coverage != crate::LocalUsageCoverage::Complete {
            return Err(incomplete(
                "Saved legacy model maxima retain missing or lower logs. No upload was produced.",
            ));
        }
        rows.push(SyncRow {
            date: row.local_date,
            tool: row.tool,
            model: row.model,
            input_tokens: row.input_tokens,
            output_tokens: row.output_tokens,
            cache_read_tokens: row.cache_read_tokens,
            cache_write_tokens: row.cache_write_tokens,
            total_tokens: row.total_tokens,
            estimated_value_micros: row.estimated_value_usd_micros,
        });
        if rows.len() > MAX_ROWS {
            return Err(too_large());
        }
    }
    rows.sort_by(|a, b| (&a.date, &a.tool, &a.model).cmp(&(&b.date, &b.tool, &b.model)));
    let mut keys = BTreeSet::new();
    for row in &rows {
        row.validate(utc)?;
        if !keys.insert((&row.date, &row.tool, &row.model)) {
            return Err(invalid(
                "Saved local usage repeats an upload key. No upload was produced.",
            ));
        }
    }
    let body = serde_json::to_string(&SyncBody { device_id, rows })
        .map_err(|_| invalid("The prospective upload could not be serialized."))?;
    if body.len() > MAX_BODY {
        return Err(too_large());
    }
    Ok(body)
}

#[cfg(test)]
mod tests;
