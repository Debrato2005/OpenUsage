//! Saved local scopes. No total combines folders, accounting generations or day boundaries.
use crate::{CliError, metrics::Metrics};
use openusage_storage::{LocalTokenDetails, LocalUsageCoverage, Storage};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalScope {
    pub id: String,
    pub tool: String,
    pub name: String,
    pub kind: &'static str,
    pub date: Option<String>,
    pub time_zone: Option<String>,
    pub source_level: &'static str,
    pub last_scan_at_utc: Option<String>,
    pub retained: bool,
    pub metrics: Metrics,
}

#[derive(Debug, Serialize)]
pub struct Local {
    pub scopes: Vec<LocalScope>,
}

fn failed() -> CliError {
    CliError {
        code: "read_failed",
        exit: 10,
        message: "OpenUsage could not read the saved data.".into(),
    }
}
fn date(now: jiff::Timestamp, zone: &str) -> Result<String, CliError> {
    jiff::tz::TimeZone::get(zone)
        .map(|tz| now.to_zoned(tz).date().to_string())
        .map_err(|_| failed())
}
fn name(tool: &str) -> String {
    openusage_locallogs::tool(tool).map_or_else(|| tool.to_owned(), |t| t.name.clone())
}

pub fn read(storage: &Storage, now: jiff::Timestamp) -> Result<Local, CliError> {
    let utc = date(now, "UTC")?;
    let initial = storage.local_usage_read(&utc, &utc).map_err(|_| failed())?;
    let sources = storage.log_sources().map_err(|_| failed())?;
    let zones = sources.iter().filter_map(|s| s.time_zone.as_deref()).chain(
        initial
            .grouped
            .iter()
            .filter_map(|g| g.scope.time_zone.as_deref()),
    );
    let dates = zones
        .map(|zone| date(now, zone))
        .collect::<Result<BTreeSet<_>, _>>()?;
    let mut reads = BTreeMap::from([(utc.clone(), initial)]);
    for day in dates {
        if let std::collections::btree_map::Entry::Vacant(entry) = reads.entry(day) {
            let view = storage
                .local_usage_read(entry.key(), entry.key())
                .map_err(|_| failed())?;
            entry.insert(view);
        }
    }
    let mut scopes = Vec::new();
    for (day, view) in reads {
        for group in view.grouped {
            let zone = group.scope.time_zone.as_deref();
            let dated = zone.map(|z| date(now, z)).transpose()?.as_ref() == Some(&day);
            let selected = if dated {
                Some(("grouped", group.dated_total.as_ref(), group.dated.first()))
            } else if zone.is_none() && day == utc {
                Some((
                    "unplaced",
                    group.unplaced_total.as_ref(),
                    group.unplaced.as_ref(),
                ))
            } else {
                None
            };
            let Some((kind, total, record)) = selected else {
                continue;
            };
            let source = record.map(|r| format!("ccusage {}", r.scan.scanner_version));
            let observed = record.map(|r| r.scan.observed_at_utc.as_str());
            scopes.push(LocalScope {
                id: format!("{kind}:{}", group.scope.tool),
                name: name(&group.scope.tool),
                tool: group.scope.tool,
                kind,
                date: dated.then(|| day.clone()),
                time_zone: group.scope.time_zone,
                source_level: "local_log",
                last_scan_at_utc: Some(group.scope.latest_scan.observed_at_utc),
                retained: record.is_some_and(|r| r.retained),
                metrics: Metrics::new(
                    total.map(|t| t.total_tokens),
                    &total.map(|t| t.details.clone()).unwrap_or_default(),
                    total.and_then(|t| t.estimated_value_micros),
                    source.as_deref(),
                    observed,
                ),
            });
        }
        for legacy in view.legacy {
            let Some(zone) = legacy.source.time_zone.as_deref() else {
                return Err(failed());
            };
            if date(now, zone)? != day {
                continue;
            }
            let rows = &legacy.days;
            let counts = |f: fn(&openusage_storage::LocalUsageRow) -> u64| {
                (!rows.is_empty()).then(|| rows.iter().map(|r| u128::from(f(&r.counts))).sum())
            };
            let details = LocalTokenDetails {
                input_tokens: counts(|r| r.input_tokens),
                output_tokens: counts(|r| r.output_tokens),
                cache_write_tokens: counts(|r| r.cache_write_tokens),
                cache_read_tokens: counts(|r| r.cache_read_tokens),
                reasoning_tokens: None,
            };
            let versions: BTreeSet<_> = rows
                .iter()
                .map(|r| format!("ccusage {}", r.scanner_version))
                .collect();
            let source =
                (!versions.is_empty()).then(|| versions.into_iter().collect::<Vec<_>>().join(", "));
            let estimate = (!rows.is_empty())
                .then(|| {
                    rows.iter()
                        .map(|r| r.counts.estimated_value_usd_micros.map(u128::from))
                        .sum::<Option<u128>>()
                })
                .flatten();
            scopes.push(LocalScope {
                id: format!("legacy:{}", legacy.source.id),
                name: name(&legacy.source.tool),
                tool: legacy.source.tool,
                kind: "legacy",
                date: Some(day.clone()),
                time_zone: legacy.source.time_zone,
                source_level: "local_log",
                last_scan_at_utc: legacy.source.last_scan_at_utc,
                retained: rows
                    .iter()
                    .any(|r| r.coverage == LocalUsageCoverage::LogsGone),
                // Legacy model maxima do not have a per-record observation time.
                metrics: Metrics::new(
                    counts(|r| r.total_tokens),
                    &details,
                    estimate,
                    source.as_deref(),
                    None,
                ),
            });
        }
    }
    scopes.sort_by_key(|s| {
        (
            openusage_locallogs::tool(&s.tool).map(|t| t.order),
            s.kind,
            s.id.clone(),
        )
    });
    Ok(Local { scopes })
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use openusage_locallogs::{AgentPeriod, TokenDetails, UsagePeriod};
    use openusage_storage::{GroupedLocalScan, LocalUsageRow, LocalUsageScan, LogConsent};

    #[test]
    fn zones_generations_retained_provenance_missing_and_zero_stay_distinct() {
        let dir = tempfile::tempdir().unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        let path = dir.path().join("usage.db");
        let key = [87; 32];
        let mut store = Storage::create(&path, &key).unwrap();
        let observed = "2026-11-08T19:00:00.000Z";
        let later = "2026-11-08T20:00:00.000Z";
        let mut roots = Vec::new();
        for (tool, suffix) in [
            ("amp", "one"),
            ("amp", "two"),
            ("opencode", "undated"),
            ("pi", "empty"),
        ] {
            roots.push(
                store
                    .set_log_source(
                        tool,
                        &format!("/srv/synthetic/{suffix}"),
                        LogConsent::Granted,
                        observed,
                    )
                    .unwrap()
                    .id,
            );
        }
        for id in &roots[..2] {
            store
                .save_local_usage_scan(&LocalUsageScan {
                    log_source_ids: std::slice::from_ref(id),
                    scanner_version: "20.0.48",
                    time_zone: "Asia/Kolkata",
                    since: None,
                    scanned_at_utc: observed,
                    rows: &[LocalUsageRow {
                        local_date: "2026-11-09".into(),
                        tool: "amp".into(),
                        model: "legacy".into(),
                        input_tokens: 100,
                        output_tokens: 0,
                        cache_write_tokens: 0,
                        cache_read_tokens: 0,
                        total_tokens: 100,
                        estimated_value_usd_micros: None,
                    }],
                })
                .unwrap();
        }
        let period = AgentPeriod {
            period: UsagePeriod::Day("2026-11-08".into()),
            total_tokens: 173,
            details: TokenDetails::default(),
            estimated_value_micros: None,
            models: vec![],
        };
        let save =
            |store: &mut Storage, tool, ids: &[String], zone, at, periods: &[AgentPeriod]| {
                store
                    .save_grouped_local_scan(&GroupedLocalScan {
                        tool,
                        source_ids: ids,
                        time_zone: zone,
                        scanner_version: "20.0.48",
                        observed_at_utc: at,
                        periods,
                        sessions: Some(&[]),
                    })
                    .unwrap();
            };
        save(
            &mut store,
            "amp",
            &roots[..2],
            Some("UTC"),
            observed,
            &[period],
        );
        save(&mut store, "amp", &roots[..1], Some("UTC"), later, &[]);
        save(
            &mut store,
            "opencode",
            &roots[2..3],
            None,
            observed,
            &[AgentPeriod {
                period: UsagePeriod::Unplaced,
                total_tokens: 0,
                details: TokenDetails::default(),
                estimated_value_micros: Some(0),
                models: vec![],
            }],
        );
        save(&mut store, "pi", &roots[3..], Some("UTC"), observed, &[]);
        drop(store);
        let before = std::fs::read(&path).unwrap();
        let store = Storage::open_read_only(&path, &key).unwrap();
        let result = read(&store, later.parse().unwrap()).unwrap();
        let json = serde_json::to_value(&result).unwrap();
        let find = |kind, tool| {
            result
                .scopes
                .iter()
                .find(|s| s.kind == kind && s.tool == tool)
                .unwrap()
        };
        let grouped = find("grouped", "amp");
        assert_eq!(grouped.date.as_deref(), Some("2026-11-08"));
        assert_eq!(grouped.metrics.total_tokens.value.as_deref(), Some("173"));
        assert_eq!(
            grouped.metrics.total_tokens.observed_at_utc.as_deref(),
            Some(observed)
        );
        assert_eq!(grouped.last_scan_at_utc.as_deref(), Some(later));
        assert!(grouped.retained);
        assert_eq!(grouped.metrics.input_tokens.value, None);
        let legacy: Vec<_> = result
            .scopes
            .iter()
            .filter(|s| s.kind == "legacy")
            .collect();
        assert_eq!(legacy.len(), 2);
        for scope in legacy {
            assert_eq!(scope.date.as_deref(), Some("2026-11-09"));
            assert_eq!(scope.time_zone.as_deref(), Some("Asia/Kolkata"));
            assert_eq!(scope.metrics.total_tokens.value.as_deref(), Some("100"));
            assert_eq!(scope.metrics.total_tokens.observed_at_utc, None);
        }
        let unplaced = find("unplaced", "opencode");
        assert_eq!(unplaced.date, None);
        assert_eq!(unplaced.time_zone, None);
        assert_eq!(unplaced.metrics.total_tokens.value.as_deref(), Some("0"));
        assert_eq!(
            unplaced.metrics.estimated_value_usd_micros.value.as_deref(),
            Some("0")
        );
        assert_eq!(
            find("grouped", "pi").metrics.total_tokens.state,
            "not_available"
        );
        assert!(json.get("totalTokens").is_none());
        assert!(!json.to_string().contains("/srv/synthetic"));
        drop(store);
        assert_eq!(std::fs::read(path).unwrap(), before);
    }
}
