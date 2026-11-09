//! Saved local log totals for the This device scope (OU-LOG-016, OU-LOG-017).
//! Rust sums exact integers and sends decimal text. A missing estimate stays missing (OU-LOG-013).
use crate::workspace::{CommandError, local_logs::LocalUsageRange};
use crate::{WorkspaceService, arguments};
use jiff::{Span, civil::Date, tz::TimeZone};
use openusage_locallogs::{hour_tools, tool};
use openusage_storage::LocalUsageCoverage;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use tauri::State;

mod scopes;
use scopes::load_view;

#[derive(Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Period {
    /// UTC hours for at most `MAX_HOURLY_DAYS` days. Days still group by day (SC-021).
    Hour,
    Day,
    Week,
    Month,
}

/// The longest range that the hourly view covers: 168 hours.
const MAX_HOURLY_DAYS: i32 = 7;

/// A fixed range of `days` ending today, or a custom range from `since` to `until` (OU-LOG-017).
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LoadArgs {
    #[serde(default)]
    days: Option<u16>,
    #[serde(default)]
    since: Option<String>,
    #[serde(default)]
    until: Option<String>,
    period: Period,
    #[serde(default)]
    scope: Option<String>,
}

/// The longest custom range: one leap year of days.
const MAX_CUSTOM_DAYS: i32 = 366;

/// The first and last day of the requested range in the zone of the saved days.
fn requested_range(args: &LoadArgs, today: Date) -> Option<(Date, Date)> {
    let date = |text: &str| {
        (text.len() == 10)
            .then(|| text.parse::<Date>().ok())
            .flatten()
    };
    match (args.days, args.since.as_deref(), args.until.as_deref()) {
        (Some(days @ (7 | 30 | 90 | 365)), None, None) => Some((
            today
                .checked_sub(Span::new().days(i64::from(days) - 1))
                .ok()?,
            today,
        )),
        (None, Some(since), Some(until)) => {
            let (start, end) = (date(since)?, date(until)?);
            let length = start.until(end).ok()?.get_days() + 1;
            (start <= end && end <= today && length <= MAX_CUSTOM_DAYS).then_some((start, end))
        }
        _ => None,
    }
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
    priced_rows: usize,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ToolTotals {
    pub(crate) reasoning_tokens: Option<String>,
    pub(crate) tool: String,
    pub(crate) input_tokens: Option<String>,
    pub(crate) output_tokens: Option<String>,
    pub(crate) cache_write_tokens: Option<String>,
    pub(crate) cache_read_tokens: Option<String>,
    pub(crate) total_tokens: String,
    /// The sum of the priced rows. None when no row has a price.
    pub(crate) estimated_value_usd_micros: Option<String>,
    /// False when some rows have no price, so the estimate covers only part of the tokens.
    pub(crate) estimate_complete: bool,
    /// True when a later scan read less for a day in the range (OU-LOG-011).
    pub(crate) logs_gone: bool,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ModelTotals {
    pub(crate) tool: String,
    pub(crate) model: String,
    pub(crate) total_tokens: Option<String>,
    pub(crate) estimated_value_usd_micros: Option<String>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Bucket {
    pub(crate) start_date: String,
    pub(crate) tool: String,
    pub(crate) total_tokens: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Difference {
    pub(crate) date: Option<String>,
    pub(crate) tool: String,
    pub(crate) model: Option<String>,
    pub(crate) saved_total_tokens: String,
    pub(crate) new_total_tokens: String,
    pub(crate) scan_at_utc: String,
}

/// The newest sessions that the view sends. Projects still count every session in the range.
const MAX_SESSIONS: usize = 100;

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SessionView {
    pub(crate) key: String,
    pub(crate) tool: String,
    /// Only a reported project, without an inferred identity (SC-022).
    pub(crate) project: Option<String>,
    /// Codex reports no start (SC-021).
    pub(crate) start_utc: Option<String>,
    pub(crate) end_utc: Option<String>,
    pub(crate) total_tokens: String,
    /// A part of the output count. None when the tool reports none.
    pub(crate) reasoning_tokens: Option<String>,
    pub(crate) estimated_value_usd_micros: Option<String>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HourBucket {
    pub(crate) start_utc: String,
    pub(crate) tool: String,
    pub(crate) total_tokens: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProjectView {
    pub(crate) tool: String,
    pub(crate) project: String,
    pub(crate) sessions: usize,
    pub(crate) total_tokens: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LocalUsageView {
    pub(crate) scope: String,
    pub(crate) scopes: Vec<scopes::ScopeInfo>,
    pub(crate) available: bool,
    pub(crate) dated: bool,
    pub(crate) start_date: String,
    pub(crate) end_date: String,
    /// The zone that sets day boundaries (OU-LOG-010).
    pub(crate) time_zone: String,
    pub(crate) scanner_versions: Vec<String>,
    pub(crate) tools: Vec<ToolTotals>,
    pub(crate) models: Vec<ModelTotals>,
    pub(crate) buckets: Vec<Bucket>,
    pub(crate) differences: Vec<Difference>,
    /// Personal records for these dates: days with saved usage, the longest run of them, and the top day.
    pub(crate) active_days: Option<usize>,
    pub(crate) longest_streak_days: Option<usize>,
    pub(crate) busiest_day: Option<BusiestDay>,
    /// Sessions that ended in the range, newest first.
    pub(crate) sessions: Vec<SessionView>,
    pub(crate) projects: Vec<ProjectView>,
    /// The UTC instants of the range start and of the day after it ends.
    pub(crate) start_utc: Option<String>,
    pub(crate) end_utc: Option<String>,
    /// Saved UTC hours, only for the hourly view. A tool outside `hour_tools` has no hours.
    pub(crate) hours: Vec<HourBucket>,
    pub(crate) hour_tools: Vec<String>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BusiestDay {
    pub(crate) date: String,
    pub(crate) total_tokens: String,
}

/// Active days, the longest run of days with saved usage, and the busiest day (the earliest on a tie).
fn records(
    by_day: &BTreeMap<Date, u128>,
    start: Date,
    end: Date,
) -> (usize, usize, Option<BusiestDay>) {
    let (mut active, mut streak, mut longest) = (0, 0, 0);
    let mut busiest: Option<(Date, u128)> = None;
    let mut cursor = start;
    while cursor <= end {
        let total = by_day.get(&cursor).copied().unwrap_or(0);
        if total > 0 {
            active += 1;
            streak += 1;
            if busiest.is_none_or(|(_, best)| total > best) {
                busiest = Some((cursor, total));
            }
        } else {
            streak = 0;
        }
        longest = longest.max(streak);
        let Ok(next) = cursor.checked_add(Span::new().days(1)) else {
            break;
        };
        cursor = next;
    }
    let busiest = busiest.map(|(date, total)| BusiestDay {
        date: date.to_string(),
        total_tokens: total.to_string(),
    });
    (active, longest, busiest)
}

fn bucket_start(date: Date, period: Period) -> Date {
    match period {
        Period::Hour | Period::Day => date,
        Period::Week => date
            .checked_sub(Span::new().days(i64::from(date.weekday().to_monday_zero_offset())))
            .unwrap_or(date),
        Period::Month => date.first_of_month(),
    }
}

/// Builds the view for days `start..=end` from saved rows only.
pub(crate) fn summarize(
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
    let mut by_day: BTreeMap<Date, u128> = BTreeMap::new();
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
            Some(value) => {
                counts.priced += u128::from(value);
                counts.priced_rows += 1;
            }
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
        *by_day.entry(date).or_default() += u128::from(row.total_tokens);
    }
    let (active_days, longest_streak_days, busiest_day) = records(&by_day, start, end);
    let order = |id: &str| tool(id).map(|tool| tool.order);
    let mut tool_rows: Vec<_> = tools.into_iter().collect();
    tool_rows.sort_by_key(|(tool, _)| (order(tool), tool.to_string()));
    let mut model_rows: Vec<_> = models.into_iter().collect();
    model_rows.sort_by(|a, b| b.1.0.cmp(&a.1.0).then(a.0.cmp(&b.0)));
    let mut projects: BTreeMap<(&str, &str), (usize, u128)> = BTreeMap::new();
    for saved in &range.sessions {
        let session = &saved.session;
        if let Some(project) = session.project_name.as_deref() {
            let entry = projects
                .entry((session.tool.as_str(), project))
                .or_default();
            entry.0 += 1;
            entry.1 += u128::from(session.total_tokens);
        }
    }
    let mut project_rows: Vec<_> = projects.into_iter().collect();
    project_rows.sort_by(|a, b| b.1.1.cmp(&a.1.1).then(a.0.cmp(&b.0)));
    LocalUsageView {
        scope: "combined".into(),
        scopes: Vec::new(),
        available: true,
        dated: true,
        start_utc: range.window.as_ref().map(|w| w.0.clone()),
        end_utc: range.window.as_ref().map(|w| w.1.clone()),
        hours: if period == Period::Hour {
            range
                .hours
                .iter()
                .map(|saved| HourBucket {
                    start_utc: saved.hour.hour_start_utc.clone(),
                    tool: saved.hour.tool.clone(),
                    total_tokens: saved.hour.total_tokens.to_string(),
                })
                .collect()
        } else {
            Vec::new()
        },
        hour_tools: hour_tools().map(|tool| tool.id.clone()).collect(),
        sessions: range
            .sessions
            .iter()
            .take(MAX_SESSIONS)
            .map(|saved| {
                let s = &saved.session;
                SessionView {
                    key: s.session_key.clone(),
                    tool: s.tool.clone(),
                    project: s.project_name.clone(),
                    start_utc: s.start_utc.clone(),
                    end_utc: Some(s.end_utc.clone()),
                    total_tokens: s.total_tokens.to_string(),
                    reasoning_tokens: s.reasoning_tokens.map(|v| v.to_string()),
                    estimated_value_usd_micros: s.estimated_value_usd_micros.map(|v| v.to_string()),
                }
            })
            .collect(),
        projects: project_rows
            .into_iter()
            .map(|((tool, project), (sessions, total))| ProjectView {
                tool: tool.into(),
                project: project.into(),
                sessions,
                total_tokens: total.to_string(),
            })
            .collect(),
        active_days: (!range.days.is_empty()).then_some(active_days),
        longest_streak_days: (!range.days.is_empty()).then_some(longest_streak_days),
        busiest_day,
        start_date: start.to_string(),
        end_date: end.to_string(),
        time_zone: zone,
        scanner_versions: versions.into_iter().collect(),
        tools: tool_rows
            .into_iter()
            .map(|(tool, c)| ToolTotals {
                reasoning_tokens: None,
                tool: tool.into(),
                input_tokens: Some(c.input.to_string()),
                output_tokens: Some(c.output.to_string()),
                cache_write_tokens: Some(c.cache_write.to_string()),
                cache_read_tokens: Some(c.cache_read.to_string()),
                total_tokens: c.total.to_string(),
                // With no priced row there is no estimate at all, not an estimate of zero.
                estimated_value_usd_micros: (c.priced_rows > 0).then(|| c.priced.to_string()),
                estimate_complete: c.unpriced_rows == 0,
                logs_gone: gone.contains(tool),
            })
            .collect(),
        models: model_rows
            .into_iter()
            .map(|((tool, model), (total, value))| ModelTotals {
                tool: tool.into(),
                model: model.into(),
                total_tokens: Some(total.to_string()),
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
                    date: Some(d.local_date.clone()),
                    tool: (*source_tools.get(d.log_source_id.as_str())?).into(),
                    model: Some(d.model.clone()),
                    saved_total_tokens: d.saved_total_tokens.to_string(),
                    new_total_tokens: d.new_total_tokens.to_string(),
                    scan_at_utc: d.scan_at_utc.clone(),
                })
            })
            .collect(),
    }
}

/// The UTC instant at which `date` starts in `zone`, in the stored form.
fn day_start_utc(date: Date, zone: &TimeZone) -> Option<String> {
    let start = date.to_zoned(zone.clone()).ok()?.timestamp();
    Some(start.strftime("%Y-%m-%dT%H:%M:%S.000Z").to_string())
}

pub(crate) fn share_view(
    workspace: &mut crate::workspace::Workspace,
    scope: Option<String>,
) -> Result<LocalUsageView, CommandError> {
    load_view(
        workspace,
        &LoadArgs {
            days: Some(365),
            since: None,
            until: None,
            period: Period::Day,
            scope,
        },
        false,
    )
}

/// Saved local totals for the last `days` days in the zone of the saved days (OU-LOG-010).
#[tauri::command]
pub async fn load_local_usage(
    request: tauri::ipc::Request<'_>,
    service: State<'_, WorkspaceService>,
) -> Result<LocalUsageView, CommandError> {
    let args: LoadArgs = arguments(&request)?;
    crate::subscriptions::access(service, move |workspace| load_view(workspace, &args, true)).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use openusage_storage::{
        LocalUsageDay, LocalUsageDifference, LocalUsageHour, LocalUsageRow, LocalUsageSession,
        LogConsent, LogSource, SavedLocalHour, SavedLocalSession,
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

    fn session(
        source: &str,
        tool: &str,
        key: &str,
        project: Option<&str>,
        end: &str,
        total: u64,
        reasoning: Option<u64>,
    ) -> SavedLocalSession {
        SavedLocalSession {
            log_source_id: source.into(),
            session: LocalUsageSession {
                tool: tool.into(),
                session_key: key.into(),
                project_name: project.map(Into::into),
                start_utc: None,
                end_utc: end.into(),
                input_tokens: total - 3,
                output_tokens: 1,
                cache_write_tokens: 1,
                cache_read_tokens: 1,
                reasoning_tokens: reasoning,
                total_tokens: total,
                estimated_value_usd_micros: None,
            },
            scanner_version: "20.0.48".into(),
        }
    }

    #[test]
    fn fixed_and_custom_ranges_are_checked() {
        let today: Date = "2026-11-08".parse().unwrap();
        let args = |days: Option<u16>, since: Option<&str>, until: Option<&str>| LoadArgs {
            days,
            since: since.map(Into::into),
            until: until.map(Into::into),
            period: Period::Day,
            scope: None,
        };
        let range =
            |a: LoadArgs| requested_range(&a, today).map(|(s, e)| (s.to_string(), e.to_string()));
        assert_eq!(
            range(args(Some(7), None, None)),
            Some(("2026-11-02".into(), "2026-11-08".into()))
        );
        assert_eq!(range(args(Some(8), None, None)), None);
        assert_eq!(
            range(args(None, Some("2025-11-08"), Some("2026-11-08"))),
            Some(("2025-11-08".into(), "2026-11-08".into())),
            "366 days is the longest custom range."
        );
        for (since, until) in [
            ("2025-11-07", "2026-11-08"), // 367 days
            ("2026-11-05", "2026-11-04"), // backwards
            ("2026-11-05", "2026-11-09"), // ends after today
            ("2026-2-01", "2026-11-04"),  // not YYYY-MM-DD
            ("2026-02-30", "2026-11-04"), // no such day
        ] {
            assert_eq!(
                range(args(None, Some(since), Some(until))),
                None,
                "{since} to {until}"
            );
        }
        assert_eq!(
            range(args(Some(7), Some("2026-11-01"), Some("2026-11-02"))),
            None,
            "Both forms at once."
        );
        assert_eq!(range(args(None, Some("2026-11-01"), None)), None);
    }

    #[test]
    fn day_starts_are_utc_instants_in_the_pinned_zone() {
        let date: Date = "2026-11-08".parse().unwrap();
        let kolkata = TimeZone::get("Asia/Kolkata").unwrap();
        assert_eq!(
            day_start_utc(date, &kolkata).as_deref(),
            Some("2026-11-07T18:30:00.000Z")
        );
        assert_eq!(
            day_start_utc(date, &TimeZone::UTC).as_deref(),
            Some("2026-11-08T00:00:00.000Z")
        );
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
            hours: vec![SavedLocalHour {
                log_source_id: CLAUDE.into(),
                hour: LocalUsageHour {
                    tool: "claude".into(),
                    hour_start_utc: "2026-11-03T04:00:00.000Z".into(),
                    input_tokens: 7,
                    output_tokens: 1,
                    cache_write_tokens: 1,
                    cache_read_tokens: 1,
                    total_tokens: 10,
                },
                scanner_version: "20.0.48".into(),
            }],
            window: Some((
                "2026-10-31T18:30:00.000Z".into(),
                "2026-11-10T18:30:00.000Z".into(),
            )),
            sessions: vec![
                session(
                    CODEX,
                    "codex",
                    "2026/11/09/rollout-x",
                    None,
                    "2026-11-09T10:00:00.000Z",
                    40,
                    Some(3),
                ),
                session(
                    CLAUDE,
                    "claude",
                    "s-2",
                    Some("-home-dev-alpha"),
                    "2026-11-03T10:00:00.000Z",
                    big,
                    None,
                ),
                session(
                    CLAUDE,
                    "claude",
                    "s-1",
                    Some("-home-dev-alpha"),
                    "2026-11-02T10:00:00.000Z",
                    big,
                    None,
                ),
                session(
                    CLAUDE,
                    "claude",
                    "s-0",
                    Some("-home-dev-beta"),
                    "2026-11-02T09:00:00.000Z",
                    9,
                    None,
                ),
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
        // Records: 2, 3 and 9 November have usage, so the longest run is 2 days.
        assert_eq!(
            (view.active_days, view.longest_streak_days),
            (Some(3), Some(2))
        );
        assert_eq!(
            view.busiest_day,
            Some(BusiestDay {
                date: "2026-11-03".into(),
                total_tokens: "18446744073709551665".into()
            })
        );
        // Sessions keep their order and a missing reasoning count stays missing.
        assert_eq!(
            view.sessions
                .iter()
                .map(|s| (s.key.as_str(), s.reasoning_tokens.as_deref()))
                .collect::<Vec<_>>(),
            [
                ("2026/11/09/rollout-x", Some("3")),
                ("s-2", None),
                ("s-1", None),
                ("s-0", None)
            ]
        );
        // Projects sum exactly past u64, and a session with no project joins none.
        assert_eq!(
            view.projects
                .iter()
                .map(|p| (p.project.as_str(), p.sessions, p.total_tokens.as_str()))
                .collect::<Vec<_>>(),
            [
                ("-home-dev-alpha", 2, "36893488147419103230"),
                ("-home-dev-beta", 1, "9")
            ]
        );
        // Hours appear only in the hourly view, which groups days by day.
        assert!(view.hours.is_empty());
        let hourly = summarize(&range, start, end, Period::Hour, "Asia/Kolkata".into());
        assert_eq!(
            hourly.hours,
            [HourBucket {
                start_utc: "2026-11-03T04:00:00.000Z".into(),
                tool: "claude".into(),
                total_tokens: "10".into()
            }]
        );
        assert_eq!(hourly.hour_tools, ["claude"]);
        assert_eq!(
            hourly.start_utc.as_deref(),
            Some("2026-10-31T18:30:00.000Z")
        );
        assert_eq!(hourly.buckets.len(), 3, "Days group by day.");
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
