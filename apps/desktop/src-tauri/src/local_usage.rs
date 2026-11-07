//! Saved local log totals for the This device scope (OU-LOG-016, OU-LOG-017).
//! Rust sums exact integers and sends decimal text. A missing estimate stays missing (OU-LOG-013).
use crate::workspace::{CommandError, local_logs::LocalUsageRange};
use crate::{WorkspaceService, arguments};
use jiff::{Span, civil::Date, tz::TimeZone};
use openusage_locallogs::FOLDER_VARIABLES;
use openusage_storage::LocalUsageCoverage;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use tauri::State;

#[derive(Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "camelCase")]
enum Period {
    Day,
    Week,
    Month,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LoadArgs {
    days: u16,
    period: Period,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct Counts {
    input: u128,
    output: u128,
    cache_write: u128,
    cache_read: u128,
    total: u128,
    priced: u128,
    unpriced_rows: usize,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ToolTotals {
    tool: String,
    input_tokens: String,
    output_tokens: String,
    cache_write_tokens: String,
    cache_read_tokens: String,
    total_tokens: String,
    /// The sum of the priced rows. None when no row has a price.
    estimated_value_usd_micros: Option<String>,
    /// False when some rows have no price, so the estimate covers only part of the tokens.
    estimate_complete: bool,
    /// True when a later scan read less for a day in the range (OU-LOG-011).
    logs_gone: bool,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ModelTotals {
    tool: String,
    model: String,
    total_tokens: String,
    estimated_value_usd_micros: Option<String>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Bucket {
    start_date: String,
    tool: String,
    total_tokens: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Difference {
    date: String,
    tool: String,
    model: String,
    saved_total_tokens: String,
    new_total_tokens: String,
    scan_at_utc: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LocalUsageView {
    start_date: String,
    end_date: String,
    /// The zone that sets day boundaries (OU-LOG-010).
    time_zone: String,
    scanner_versions: Vec<String>,
    tools: Vec<ToolTotals>,
    models: Vec<ModelTotals>,
    buckets: Vec<Bucket>,
    differences: Vec<Difference>,
}

fn bucket_start(date: Date, period: Period) -> Date {
    match period {
        Period::Day => date,
        Period::Week => date
            .checked_sub(Span::new().days(i64::from(date.weekday().to_monday_zero_offset())))
            .unwrap_or(date),
        Period::Month => date.first_of_month(),
    }
}

/// Builds the view for days `start..=end` from saved rows only.
fn summarize(
    range: &LocalUsageRange,
    start: Date,
    end: Date,
    period: Period,
    zone: String,
) -> LocalUsageView {
    let source_tools: BTreeMap<&str, &str> = range
        .sources
        .iter()
        .map(|source| (source.id.as_str(), source.tool.as_str()))
        .collect();
    let mut tools: BTreeMap<&str, Counts> = BTreeMap::new();
    let mut gone: BTreeSet<&str> = BTreeSet::new();
    let mut models: BTreeMap<(&str, &str), (u128, Option<u128>)> = BTreeMap::new();
    let mut buckets: BTreeMap<(Date, &str), u128> = BTreeMap::new();
    let mut versions = BTreeSet::new();
    for day in &range.days {
        let row = &day.counts;
        let Ok(date) = row.local_date.parse::<Date>() else {
            continue;
        };
        let counts = tools.entry(row.tool.as_str()).or_default();
        counts.input += u128::from(row.input_tokens);
        counts.output += u128::from(row.output_tokens);
        counts.cache_write += u128::from(row.cache_write_tokens);
        counts.cache_read += u128::from(row.cache_read_tokens);
        counts.total += u128::from(row.total_tokens);
        match row.estimated_value_usd_micros {
            Some(value) => counts.priced += u128::from(value),
            None => counts.unpriced_rows += 1,
        }
        if day.coverage == LocalUsageCoverage::LogsGone {
            gone.insert(row.tool.as_str());
        }
        let model = models
            .entry((row.tool.as_str(), row.model.as_str()))
            .or_insert((0, Some(0)));
        model.0 += u128::from(row.total_tokens);
        model.1 = model
            .1
            .zip(row.estimated_value_usd_micros)
            .map(|(sum, value)| sum + u128::from(value));
        *buckets
            .entry((bucket_start(date, period), row.tool.as_str()))
            .or_default() += u128::from(row.total_tokens);
        versions.insert(day.scanner_version.clone());
    }
    let order = |tool: &str| FOLDER_VARIABLES.iter().position(|(t, _)| *t == tool);
    let mut tool_rows: Vec<_> = tools.into_iter().collect();
    tool_rows.sort_by_key(|(tool, _)| (order(tool), tool.to_string()));
    let mut model_rows: Vec<_> = models.into_iter().collect();
    model_rows.sort_by(|a, b| b.1.0.cmp(&a.1.0).then(a.0.cmp(&b.0)));
    LocalUsageView {
        start_date: start.to_string(),
        end_date: end.to_string(),
        time_zone: zone,
        scanner_versions: versions.into_iter().collect(),
        tools: tool_rows
            .into_iter()
            .map(|(tool, c)| ToolTotals {
                tool: tool.into(),
                input_tokens: c.input.to_string(),
                output_tokens: c.output.to_string(),
                cache_write_tokens: c.cache_write.to_string(),
                cache_read_tokens: c.cache_read.to_string(),
                total_tokens: c.total.to_string(),
                // With no priced row there is no estimate at all, not an estimate of zero.
                estimated_value_usd_micros: (c.unpriced_rows == 0 || c.priced > 0)
                    .then(|| c.priced.to_string()),
                estimate_complete: c.unpriced_rows == 0,
                logs_gone: gone.contains(tool),
            })
            .collect(),
        models: model_rows
            .into_iter()
            .map(|((tool, model), (total, value))| ModelTotals {
                tool: tool.into(),
                model: model.into(),
                total_tokens: total.to_string(),
                estimated_value_usd_micros: value.map(|v| v.to_string()),
            })
            .collect(),
        buckets: buckets
            .into_iter()
            .map(|((date, tool), total)| Bucket {
                start_date: date.to_string(),
                tool: tool.into(),
                total_tokens: total.to_string(),
            })
            .collect(),
        differences: range
            .differences
            .iter()
            .filter_map(|d| {
                Some(Difference {
                    date: d.local_date.clone(),
                    tool: (*source_tools.get(d.log_source_id.as_str())?).into(),
                    model: d.model.clone(),
                    saved_total_tokens: d.saved_total_tokens.to_string(),
                    new_total_tokens: d.new_total_tokens.to_string(),
                    scan_at_utc: d.scan_at_utc.clone(),
                })
            })
            .collect(),
    }
}

/// Saved local totals for the last `days` days in the zone of the saved days (OU-LOG-010).
#[tauri::command]
pub async fn load_local_usage(
    request: tauri::ipc::Request<'_>,
    service: State<'_, WorkspaceService>,
) -> Result<LocalUsageView, CommandError> {
    let args: LoadArgs = arguments(&request)?;
    if !matches!(args.days, 7 | 30 | 90 | 365) {
        return Err(CommandError::InvalidArguments);
    }
    crate::subscriptions::access(service, move |workspace| {
        let zone = workspace.local_usage_zone()?;
        let today = TimeZone::get(&zone)
            .map(|tz| jiff::Timestamp::now().to_zoned(tz).date())
            .map_err(|_| CommandError::StorageUnavailable)?;
        let start = today
            .checked_sub(Span::new().days(i64::from(args.days) - 1))
            .map_err(|_| CommandError::InvalidArguments)?;
        let range = workspace.local_usage_range(&start.to_string(), &today.to_string())?;
        Ok(summarize(&range, start, today, args.period, zone))
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use openusage_storage::{
        LocalUsageDay, LocalUsageDifference, LocalUsageRow, LogConsent, LogSource,
    };

    const CLAUDE: &str = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
    const CODEX: &str = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";

    fn day(
        source: &str,
        tool: &str,
        date: &str,
        model: &str,
        total: u64,
        value: Option<u64>,
        gone: bool,
    ) -> LocalUsageDay {
        LocalUsageDay {
            log_source_id: source.into(),
            counts: LocalUsageRow {
                local_date: date.into(),
                tool: tool.into(),
                model: model.into(),
                input_tokens: total - 3,
                output_tokens: 1,
                cache_write_tokens: 1,
                cache_read_tokens: 1,
                total_tokens: total,
                estimated_value_usd_micros: value,
            },
            scanner_version: "20.0.48".into(),
            coverage: if gone {
                LocalUsageCoverage::LogsGone
            } else {
                LocalUsageCoverage::Complete
            },
        }
    }

    fn source(id: &str, tool: &str, zone: Option<&str>) -> LogSource {
        LogSource {
            id: id.into(),
            tool: tool.into(),
            folder: format!("/home/synthetic/.{tool}"),
            consent: LogConsent::Granted,
            time_zone: zone.map(Into::into),
            last_scan_at_utc: None,
        }
    }

    #[test]
    fn totals_are_exact_and_a_missing_price_stays_missing() {
        let big = u64::MAX;
        let range = LocalUsageRange {
            days: vec![
                day(
                    CLAUDE,
                    "claude",
                    "2026-11-02",
                    "claude-sonnet-4",
                    big,
                    Some(900),
                    false,
                ),
                day(
                    CLAUDE,
                    "claude",
                    "2026-11-03",
                    "claude-sonnet-4",
                    big,
                    Some(100),
                    true,
                ),
                day(
                    CLAUDE,
                    "claude",
                    "2026-11-03",
                    "unknown-model",
                    50,
                    None,
                    false,
                ),
                day(CODEX, "codex", "2026-11-09", "gpt-5-codex", 40, None, false),
            ],
            differences: vec![LocalUsageDifference {
                log_source_id: CLAUDE.into(),
                local_date: "2026-11-03".into(),
                model: "claude-sonnet-4".into(),
                saved_total_tokens: big,
                new_total_tokens: 10,
                scan_at_utc: "2026-11-08T08:00:00.000Z".into(),
            }],
            sources: vec![
                source(CODEX, "codex", None),
                source(CLAUDE, "claude", Some("Asia/Kolkata")),
            ],
        };
        let start: Date = "2026-11-01".parse().unwrap();
        let end: Date = "2026-11-10".parse().unwrap();
        let view = summarize(&range, start, end, Period::Week, "Asia/Kolkata".into());
        // Two rows of u64::MAX sum past u64, and the u128 sum stays exact.
        let claude = &view.tools[0];
        assert_eq!(claude.tool, "claude");
        assert_eq!(claude.total_tokens, "36893488147419103280");
        assert_eq!(claude.estimated_value_usd_micros.as_deref(), Some("1000"));
        assert!(!claude.estimate_complete && claude.logs_gone);
        // A tool with no priced row has no estimate at all, not zero (OU-LOG-013).
        let codex = &view.tools[1];
        assert_eq!(
            (
                codex.estimated_value_usd_micros.as_deref(),
                codex.estimate_complete
            ),
            (None, false)
        );
        assert_eq!(
            view.models
                .iter()
                .map(|m| (m.model.as_str(), m.estimated_value_usd_micros.as_deref()))
                .collect::<Vec<_>>(),
            [
                ("claude-sonnet-4", Some("1000")),
                ("unknown-model", None),
                ("gpt-5-codex", None)
            ]
        );
        // Weeks start on Monday: 2 and 3 November 2026 share the week of 2 November.
        assert_eq!(
            view.buckets
                .iter()
                .map(|b| (
                    b.start_date.as_str(),
                    b.tool.as_str(),
                    b.total_tokens.as_str()
                ))
                .collect::<Vec<_>>(),
            [
                ("2026-11-02", "claude", "36893488147419103280"),
                ("2026-11-09", "codex", "40")
            ]
        );
        assert_eq!(view.differences[0].tool, "claude");
        let months = summarize(&range, start, end, Period::Month, "UTC".into());
        assert_eq!(
            months
                .buckets
                .iter()
                .map(|b| b.start_date.as_str())
                .collect::<BTreeSet<_>>(),
            ["2026-11-01"].into()
        );
        assert_eq!(
            summarize(&range, start, end, Period::Day, "UTC".into())
                .buckets
                .len(),
            3
        );
    }
}
