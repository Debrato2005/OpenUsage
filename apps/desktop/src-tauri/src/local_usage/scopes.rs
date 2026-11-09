use super::*;
use openusage_storage::{
    GroupedUsageRead, LocalToolScope, LocalUsageKey, LocalUsageRead, LogSource,
};

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ScopeInfo {
    id: String,
    kind: &'static str,
    tool: Option<String>,
    time_zone: Option<String>,
    folder: Option<String>,
    total_tokens: Option<String>,
}

fn compatible(sources: &[LogSource], groups: &[LocalToolScope]) -> bool {
    let legacy: Vec<_> = sources.iter().filter(|s| s.time_zone.is_some()).collect();
    let dated: Vec<_> = groups.iter().filter(|s| s.time_zone.is_some()).collect();
    if !legacy.is_empty() && !dated.is_empty() {
        return false;
    }
    let unique_tools: BTreeSet<_> = legacy.iter().map(|s| &s.tool).collect();
    let zones: BTreeSet<_> = legacy
        .iter()
        .filter_map(|s| s.time_zone.as_deref())
        .chain(dated.iter().filter_map(|s| s.time_zone.as_deref()))
        .collect();
    unique_tools.len() == legacy.len() && zones.len() <= 1
}

fn scope_zone(
    selection: &str,
    sources: &[LogSource],
    groups: &[LocalToolScope],
    fallback: String,
) -> Result<String, CommandError> {
    if selection == "combined" {
        return Ok(if compatible(sources, groups) {
            sources
                .iter()
                .find_map(|s| s.time_zone.clone())
                .or_else(|| groups.iter().find_map(|s| s.time_zone.clone()))
                .unwrap_or(fallback)
        } else {
            fallback
        });
    }
    if let Some(id) = selection.strip_prefix("legacy:") {
        return sources
            .iter()
            .find(|s| s.id == id)
            .and_then(|s| s.time_zone.clone())
            .ok_or(CommandError::InvalidArguments);
    }
    let (kind, id) = selection
        .split_once(':')
        .ok_or(CommandError::InvalidArguments)?;
    let group = groups
        .iter()
        .find(|s| s.tool == id)
        .ok_or(CommandError::InvalidArguments)?;
    match kind {
        "grouped" => group
            .time_zone
            .clone()
            .ok_or(CommandError::InvalidArguments),
        "unplaced" => Ok(group.time_zone.clone().unwrap_or(fallback)),
        _ => Err(CommandError::InvalidArguments),
    }
}

fn options(read: &LocalUsageRead) -> Vec<ScopeInfo> {
    let mut scopes = vec![ScopeInfo {
        id: "combined".into(),
        kind: "combined",
        tool: None,
        time_zone: None,
        folder: None,
        total_tokens: None,
    }];
    for group in &read.grouped {
        for (kind, total, present) in [
            (
                "grouped",
                &group.dated_total,
                group.scope.time_zone.is_some(),
            ),
            (
                "unplaced",
                &group.unplaced_total,
                group.unplaced.is_some() || !group.unplaced_sessions.is_empty(),
            ),
        ] {
            if present {
                scopes.push(ScopeInfo {
                    id: format!("{kind}:{}", group.scope.tool),
                    kind,
                    tool: Some(group.scope.tool.clone()),
                    time_zone: if kind == "grouped" {
                        group.scope.time_zone.clone()
                    } else {
                        None
                    },
                    folder: None,
                    total_tokens: total.as_ref().map(|t| t.total_tokens.to_string()),
                });
            }
        }
    }
    for legacy in &read.legacy {
        if legacy.source.time_zone.is_some() {
            scopes.push(ScopeInfo {
                id: format!("legacy:{}", legacy.source.id),
                kind: "legacy",
                tool: Some(legacy.source.tool.clone()),
                time_zone: legacy.source.time_zone.clone(),
                folder: Some(legacy.source.folder.clone()),
                total_tokens: (!legacy.days.is_empty()).then(|| {
                    legacy
                        .days
                        .iter()
                        .map(|d| u128::from(d.counts.total_tokens))
                        .sum::<u128>()
                        .to_string()
                }),
            });
        }
    }
    scopes
}

pub(crate) fn load_view(
    workspace: &mut crate::workspace::Workspace,
    args: &LoadArgs,
    with_sessions: bool,
) -> Result<LocalUsageView, CommandError> {
    let selection = args.scope.as_deref().unwrap_or("combined");
    let sources = workspace.log_sources()?;
    let groups = workspace.local_usage_scopes()?;
    let zone = scope_zone(selection, &sources, &groups, workspace.local_usage_zone()?)?;
    let tz = TimeZone::get(&zone).map_err(|_| CommandError::StorageUnavailable)?;
    let today = jiff::Timestamp::now().to_zoned(tz.clone()).date();
    let (start, end) = requested_range(args, today).ok_or(CommandError::InvalidArguments)?;
    if args.period == Period::Hour
        && start
            .until(end)
            .map_err(|_| CommandError::InvalidArguments)?
            .get_days()
            + 1
            > MAX_HOURLY_DAYS
    {
        return Err(CommandError::InvalidArguments);
    }
    let next = end
        .checked_add(Span::new().days(1))
        .map_err(|_| CommandError::InvalidArguments)?;
    let since = day_start_utc(start, &tz).ok_or(CommandError::StorageUnavailable)?;
    let until = day_start_utc(next, &tz).ok_or(CommandError::StorageUnavailable)?;
    let read = workspace.local_usage_read(&start.to_string(), &end.to_string())?;
    let legacy_details =
        if with_sessions && (selection == "combined" || selection.starts_with("legacy:")) {
            workspace.local_usage_range(
                (&start.to_string(), &end.to_string()),
                Some((&since, &until)),
            )?
        } else {
            LocalUsageRange {
                days: Vec::new(),
                differences: Vec::new(),
                sources: Vec::new(),
                sessions: Vec::new(),
                hours: Vec::new(),
                window: Some((since, until)),
            }
        };
    project(
        &read,
        legacy_details,
        selection,
        start,
        end,
        args.period,
        zone,
    )
}

fn project(
    read: &LocalUsageRead,
    mut legacy: LocalUsageRange,
    selection: &str,
    start: Date,
    end: Date,
    period: Period,
    zone: String,
) -> Result<LocalUsageView, CommandError> {
    let scopes = options(read);
    if !scopes.iter().any(|s| s.id == selection) {
        return Err(CommandError::InvalidArguments);
    }
    let compatible = compatible(
        &read
            .legacy
            .iter()
            .map(|s| s.source.clone())
            .collect::<Vec<_>>(),
        &read
            .grouped
            .iter()
            .map(|s| s.scope.clone())
            .collect::<Vec<_>>(),
    );
    let available = selection != "combined" || compatible;
    let dated = !selection.starts_with("unplaced:");
    let legacy_ids: BTreeSet<&str> = read
        .legacy
        .iter()
        .filter(|l| {
            available
                && l.source.time_zone.is_some()
                && (selection == "combined" || selection == format!("legacy:{}", l.source.id))
        })
        .map(|l| l.source.id.as_str())
        .collect();
    legacy.days = read
        .legacy
        .iter()
        .filter(|l| legacy_ids.contains(l.source.id.as_str()))
        .flat_map(|l| l.days.clone())
        .collect();
    legacy.differences = read
        .legacy
        .iter()
        .filter(|l| legacy_ids.contains(l.source.id.as_str()))
        .flat_map(|l| l.differences.clone())
        .collect();
    legacy.sources = read
        .legacy
        .iter()
        .filter(|l| legacy_ids.contains(l.source.id.as_str()))
        .map(|l| l.source.clone())
        .collect();
    legacy
        .sessions
        .retain(|s| legacy_ids.contains(s.log_source_id.as_str()));
    legacy
        .hours
        .retain(|s| legacy_ids.contains(s.log_source_id.as_str()));
    let mut view = summarize(&legacy, start, end, period, zone);
    view.scope = selection.into();
    view.scopes = scopes;
    view.available = available;
    view.dated = dated;
    let groups: Vec<_> = read
        .grouped
        .iter()
        .filter(|g| {
            available
                && (selection == "combined"
                    || selection == format!("grouped:{}", g.scope.tool)
                    || selection == format!("unplaced:{}", g.scope.tool))
        })
        .collect();
    if !groups.is_empty() && legacy_ids.is_empty() {
        grouped(&mut view, &groups, dated, start, end, period);
    }
    if !dated || !available {
        view.start_utc = None;
        view.end_utc = None;
        view.hours.clear();
        view.hour_tools.clear();
        view.buckets.clear();
        view.active_days = None;
        view.longest_streak_days = None;
        view.busiest_day = None;
    }
    if !dated {
        view.time_zone = "UTC".into();
    }
    Ok(view)
}

fn grouped(
    view: &mut LocalUsageView,
    groups: &[&GroupedUsageRead],
    dated: bool,
    start: Date,
    end: Date,
    period: Period,
) {
    let mut by_day: BTreeMap<Date, u128> = BTreeMap::new();
    let mut buckets: BTreeMap<(Date, String), u128> = BTreeMap::new();
    let mut versions = BTreeSet::new();
    let mut projects: BTreeMap<(String, String), (usize, u128)> = BTreeMap::new();
    view.hour_tools.clear();
    for group in groups {
        let tool = &group.scope.tool;
        let differences = if dated {
            &group.dated_differences
        } else {
            &group.unplaced_differences
        };
        let total = if dated {
            &group.dated_total
        } else {
            &group.unplaced_total
        };
        if let Some(total) = total {
            let d = &total.details;
            view.tools.push(ToolTotals {
                tool: tool.clone(),
                input_tokens: d.input_tokens.map(|v| v.to_string()),
                output_tokens: d.output_tokens.map(|v| v.to_string()),
                cache_write_tokens: d.cache_write_tokens.map(|v| v.to_string()),
                cache_read_tokens: d.cache_read_tokens.map(|v| v.to_string()),
                reasoning_tokens: d.reasoning_tokens.map(|v| v.to_string()),
                total_tokens: total.total_tokens.to_string(),
                estimated_value_usd_micros: total.estimated_value_micros.map(|v| v.to_string()),
                estimate_complete: total.estimated_value_micros.is_some(),
                logs_gone: !differences.is_empty(),
            });
            view.models.extend(total.models.iter().map(|m| ModelTotals {
                tool: tool.clone(),
                model: m.model.clone(),
                total_tokens: m.total_tokens.map(|v| v.to_string()),
                estimated_value_usd_micros: m.estimated_value_micros.map(|v| v.to_string()),
            }));
        }
        let periods: Vec<_> = if dated {
            group.dated.iter().collect()
        } else {
            group.unplaced.iter().collect()
        };
        for saved in periods {
            versions.insert(saved.scan.scanner_version.clone());
            if let openusage_locallogs::UsagePeriod::Day(date) = &saved.period.period
                && let Ok(date) = date.parse::<Date>()
            {
                *by_day.entry(date).or_default() += u128::from(saved.period.total_tokens);
                *buckets
                    .entry((bucket_start(date, period), tool.clone()))
                    .or_default() += u128::from(saved.period.total_tokens);
            }
        }
        view.differences
            .extend(differences.iter().map(|d| Difference {
                date: if let LocalUsageKey::Day(date) = &d.key {
                    Some(date.clone())
                } else {
                    None
                },
                tool: tool.clone(),
                model: None,
                saved_total_tokens: d.saved_total_tokens.to_string(),
                new_total_tokens: d.new_total_tokens.to_string(),
                scan_at_utc: d.scan.observed_at_utc.clone(),
            }));
        for saved in if dated {
            &group.dated_sessions
        } else {
            &group.unplaced_sessions
        } {
            let s = &saved.session;
            versions.insert(saved.scan.scanner_version.clone());
            if let Some(project) = &s.project {
                let entry = projects.entry((tool.clone(), project.clone())).or_default();
                entry.0 += 1;
                entry.1 += u128::from(s.total_tokens);
            }
            view.sessions.push(SessionView {
                key: s.session_key.clone(),
                tool: tool.clone(),
                project: s.project.clone(),
                start_utc: s.first_activity_utc.clone(),
                end_utc: s.last_activity_utc.clone(),
                total_tokens: s.total_tokens.to_string(),
                reasoning_tokens: s.details.reasoning_tokens.map(|v| v.to_string()),
                estimated_value_usd_micros: s.estimated_value_micros.map(|v| v.to_string()),
            });
        }
    }
    let (active, streak, busiest) = records(&by_day, start, end);
    view.active_days = (!by_day.is_empty()).then_some(active);
    view.longest_streak_days = (!by_day.is_empty()).then_some(streak);
    view.busiest_day = busiest;
    view.buckets = buckets
        .into_iter()
        .map(|((date, tool), total)| Bucket {
            start_date: date.to_string(),
            tool,
            total_tokens: total.to_string(),
        })
        .collect();
    view.scanner_versions = versions.into_iter().collect();
    view.sessions.sort_by(|a, b| {
        b.end_utc
            .cmp(&a.end_utc)
            .then(a.tool.cmp(&b.tool))
            .then(a.key.cmp(&b.key))
    });
    view.sessions.truncate(MAX_SESSIONS);
    view.projects = projects
        .into_iter()
        .map(|((tool, project), (sessions, total))| ProjectView {
            tool,
            project,
            sessions,
            total_tokens: total.to_string(),
        })
        .collect();
    view.projects.sort_by(|a, b| {
        b.total_tokens
            .len()
            .cmp(&a.total_tokens.len())
            .then(b.total_tokens.cmp(&a.total_tokens))
            .then(a.tool.cmp(&b.tool))
            .then(a.project.cmp(&b.project))
    });
    view.tools.sort_by_key(|t| tool(&t.tool).map(|t| t.order));
}

#[cfg(test)]
mod tests;
