use super::*;
use openusage_locallogs::{AgentPeriod, AgentSession, TokenDetails, UsagePeriod};
use openusage_storage::{
    LegacyUsageRead, LocalModelTotal, LocalPeriodTotal, LocalScanProvenance, LocalTokenDetails,
    LocalUsageCoverage, LocalUsageDay, LocalUsageRow, LogConsent, SavedGroupedSession,
    SavedLocalPeriod,
};

fn scan() -> LocalScanProvenance {
    LocalScanProvenance {
        id: "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa".into(),
        source_ids: vec!["bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb".into()],
        scanner_version: "20.0.48".into(),
        normalization_version: 1,
        observed_at_utc: "2026-11-08T12:00:00.000Z".into(),
    }
}
fn empty() -> LocalUsageRange {
    LocalUsageRange {
        days: vec![],
        differences: vec![],
        sources: vec![],
        sessions: vec![],
        hours: vec![],
        window: Some((
            "2026-11-01T00:00:00.000Z".into(),
            "2026-11-09T00:00:00.000Z".into(),
        )),
    }
}
fn read() -> LocalUsageRead {
    LocalUsageRead {
        from_date: "2026-11-01".into(),
        to_date: "2026-11-08".into(),
        grouped: vec![],
        legacy: vec![],
    }
}
fn view(read: &LocalUsageRead, selection: &str) -> LocalUsageView {
    project(
        read,
        empty(),
        selection,
        "2026-11-01".parse().unwrap(),
        "2026-11-08".parse().unwrap(),
        Period::Day,
        "UTC".into(),
    )
    .unwrap()
}
fn grouped_fixture(tool: &str, zone: Option<&str>) -> GroupedUsageRead {
    let period = AgentPeriod {
        period: if zone.is_some() {
            UsagePeriod::Day("2026-11-08".into())
        } else {
            UsagePeriod::Unplaced
        },
        total_tokens: 173,
        details: TokenDetails {
            input_tokens: Some(10),
            ..Default::default()
        },
        estimated_value_micros: None,
        models: vec![],
    };
    let saved = SavedLocalPeriod {
        period,
        scan: scan(),
        retained: true,
    };
    let total = LocalPeriodTotal {
        total_tokens: 173,
        details: LocalTokenDetails {
            input_tokens: Some(10),
            ..Default::default()
        },
        estimated_value_micros: None,
        models: vec![LocalModelTotal {
            model: "m".into(),
            total_tokens: None,
            details: LocalTokenDetails::default(),
            estimated_value_micros: None,
        }],
        model_totals_complete: false,
    };
    let untimed = SavedGroupedSession {
        session: AgentSession {
            session_key: "session-a".into(),
            project: None,
            first_activity_utc: None,
            last_activity_utc: None,
            total_tokens: 999,
            details: TokenDetails::default(),
            estimated_value_micros: None,
            models: vec![],
        },
        scan: scan(),
        retained: false,
    };
    GroupedUsageRead {
        scope: LocalToolScope {
            tool: tool.into(),
            time_zone: zone.map(Into::into),
            latest_scan: scan(),
        },
        window: None,
        dated: if zone.is_some() {
            vec![saved.clone()]
        } else {
            vec![]
        },
        unplaced: zone.is_none().then_some(saved),
        dated_total: zone.map(|_| total.clone()),
        unplaced_total: zone.is_none().then_some(total),
        dated_sessions: vec![],
        unplaced_sessions: vec![untimed],
        dated_differences: vec![],
        unplaced_differences: vec![],
    }
}
fn legacy(id: &str, tool: &str, zone: &str) -> LegacyUsageRead {
    LegacyUsageRead {
        source: LogSource {
            id: id.into(),
            tool: tool.into(),
            folder: format!("/synthetic/{id}"),
            consent: LogConsent::Granted,
            time_zone: Some(zone.into()),
            last_scan_at_utc: Some(scan().observed_at_utc),
        },
        days: vec![LocalUsageDay {
            log_source_id: id.into(),
            counts: LocalUsageRow {
                local_date: "2026-11-08".into(),
                tool: tool.into(),
                model: "m".into(),
                input_tokens: 100,
                output_tokens: 0,
                cache_write_tokens: 0,
                cache_read_tokens: 0,
                total_tokens: 100,
                estimated_value_usd_micros: Some(0),
            },
            scanner_version: "20.0.48".into(),
            coverage: LocalUsageCoverage::LogsGone,
        }],
        differences: vec![],
    }
}

#[test]
fn independent_totals_and_nulls_reach_ipc_without_session_or_model_sums() {
    let mut read = read();
    read.grouped.push(grouped_fixture("kilo", Some("UTC")));
    let dated = view(&read, "combined");
    assert_eq!(dated.tools[0].total_tokens, "173");
    assert_eq!(dated.tools[0].input_tokens.as_deref(), Some("10"));
    assert_eq!(dated.tools[0].output_tokens, None);
    assert_eq!(dated.models[0].total_tokens, None);
    assert_eq!(dated.buckets[0].total_tokens, "173");
    assert!(dated.sessions.is_empty());
    let json = serde_json::to_value(&dated).unwrap();
    assert!(json["tools"][0]["reasoningTokens"].is_null());
    assert!(json["models"][0]["totalTokens"].is_null());
    let undated = view(&read, "unplaced:kilo");
    assert!(!undated.dated);
    assert!(undated.tools.is_empty());
    assert_eq!(undated.sessions[0].end_utc, None);
    assert_eq!(undated.sessions[0].total_tokens, "999");
    assert!(undated.buckets.is_empty());
    assert_eq!(undated.active_days, None);
}

#[test]
fn unplaced_counts_never_enter_combined_dates_or_shares() {
    let mut read = read();
    read.grouped.push(grouped_fixture("opencode", None));
    let dated = view(&read, "combined");
    assert!(dated.tools.is_empty());
    assert!(dated.buckets.is_empty());
    assert!(dated.sessions.is_empty());
    let undated = view(&read, "unplaced:opencode");
    assert_eq!(undated.tools[0].total_tokens, "173");
    assert_eq!(undated.active_days, None);
    assert!(undated.buckets.is_empty());
    assert!(undated.hours.is_empty());
}

#[test]
fn copied_legacy_roots_and_mixed_generations_have_no_combined_total() {
    let mut read = read();
    let a = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
    let b = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";
    read.legacy = vec![legacy(a, "claude", "UTC"), legacy(b, "claude", "UTC")];
    let combined = view(&read, "combined");
    assert!(!combined.available);
    assert!(combined.tools.is_empty());
    assert_eq!(combined.active_days, None);
    assert_eq!(combined.scopes[1].total_tokens.as_deref(), Some("100"));
    let selected = view(&read, &format!("legacy:{a}"));
    assert!(selected.available);
    assert_eq!(selected.tools[0].total_tokens, "100");
    assert!(selected.tools[0].logs_gone);
    assert_eq!(
        selected.tools[0].estimated_value_usd_micros.as_deref(),
        Some("0")
    );
    read.legacy.truncate(1);
    read.grouped.push(grouped_fixture("kilo", Some("UTC")));
    assert!(!view(&read, "combined").available);
    assert_eq!(view(&read, "grouped:kilo").tools[0].total_tokens, "173");
    assert_eq!(
        view(&read, &format!("legacy:{a}")).tools[0].total_tokens,
        "100"
    );
}

#[test]
fn each_scope_keeps_its_zone_and_unknown_selection_is_refused() {
    let a = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
    let b = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";
    let sources = vec![
        legacy(a, "claude", "UTC").source,
        legacy(b, "codex", "Asia/Kolkata").source,
    ];
    assert!(!compatible(&sources, &[]));
    assert_eq!(
        scope_zone(
            &format!("legacy:{b}"),
            &sources,
            &[],
            "America/New_York".into()
        )
        .unwrap(),
        "Asia/Kolkata"
    );
    let groups = vec![
        grouped_fixture("pi", Some("UTC")).scope,
        grouped_fixture("kilo", Some("Asia/Kolkata")).scope,
    ];
    assert!(!compatible(&[], &groups));
    assert_eq!(
        scope_zone("grouped:kilo", &[], &groups, "UTC".into()).unwrap(),
        "Asia/Kolkata"
    );
    assert_eq!(
        scope_zone("grouped:claude", &[], &groups, "UTC".into()),
        Err(CommandError::InvalidArguments)
    );
    assert!(compatible(&sources[..1], &[]));
    let same_zone = vec![sources[0].clone(), legacy(b, "codex", "UTC").source];
    assert!(compatible(&same_zone, &[]));
}
