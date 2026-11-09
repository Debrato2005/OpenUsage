use super::*;
use crate::{GroupedLocalScan, LocalUsageRow, LocalUsageScan, LogConsent};
use openusage_locallogs::{AgentSession, ModelDetail};

const NOW: &str = "2026-11-09T12:00:00.000Z";
const KEY: [u8; 32] = [79; 32];

fn period(key: UsagePeriod, tokens: u64, model: Option<(&str, Option<u64>)>) -> AgentPeriod {
    AgentPeriod {
        period: key,
        total_tokens: tokens,
        details: TokenDetails::default(),
        estimated_value_micros: None,
        models: model
            .map(|(name, total)| ModelDetail {
                model: name.into(),
                total_tokens: total,
                details: TokenDetails::default(),
                estimated_value_micros: None,
            })
            .into_iter()
            .collect(),
    }
}
fn session(key: &str, last: Option<&str>) -> AgentSession {
    AgentSession {
        session_key: key.into(),
        project: Some("synthetic-project".into()),
        first_activity_utc: None,
        last_activity_utc: last.map(str::to_string),
        total_tokens: 999,
        details: TokenDetails::default(),
        estimated_value_micros: None,
        models: vec![],
    }
}
fn source(storage: &mut Storage, tool: &str, suffix: &str) -> String {
    storage
        .set_log_source(
            tool,
            &format!("/srv/synthetic/{suffix}"),
            LogConsent::Granted,
            NOW,
        )
        .unwrap()
        .id
}
fn save(
    storage: &mut Storage,
    tool: &str,
    sources: &[String],
    zone: Option<&str>,
    periods: &[AgentPeriod],
    sessions: &[AgentSession],
) {
    storage
        .save_grouped_local_scan(&GroupedLocalScan {
            tool,
            source_ids: sources,
            time_zone: zone,
            scanner_version: "20.0.48",
            observed_at_utc: NOW,
            periods,
            sessions: Some(sessions),
        })
        .unwrap();
}

#[test]
fn period_sums_keep_missing_metrics_and_incomplete_models_unavailable() {
    assert_eq!(total(&[]), None);
    let mut a = period(
        UsagePeriod::Day("2026-11-08".into()),
        100,
        Some(("model-a", Some(100))),
    );
    a.details.input_tokens = Some(100);
    a.estimated_value_micros = Some(0);
    assert_eq!(total(&[&a]).unwrap().estimated_value_micros, Some(0));
    let mut b = period(
        UsagePeriod::Day("2026-11-09".into()),
        73,
        Some(("model-b", None)),
    );
    b.details.output_tokens = Some(73);
    let sum = total(&[&a, &b]).unwrap();
    assert_eq!(sum.total_tokens, 173);
    assert_eq!(sum.details, LocalTokenDetails::default());
    assert_eq!(sum.estimated_value_micros, None);
    assert!(!sum.model_totals_complete);
    assert!(
        sum.models
            .iter()
            .all(|m| m.total_tokens.is_none() && m.details == LocalTokenDetails::default())
    );
    b.models[0].total_tokens = Some(73);
    let sum = total(&[&a, &b]).unwrap();
    assert!(sum.model_totals_complete);
    assert_eq!(
        sum.models
            .iter()
            .map(|m| m.total_tokens)
            .collect::<Vec<_>>(),
        [Some(100), Some(73)]
    );
    let zero = period(UsagePeriod::Unplaced, 0, None);
    assert_eq!(total(&[&zero]).unwrap().total_tokens, 0);
    assert_eq!(total(&[&zero]).unwrap().details.reasoning_tokens, None);
    let large: Vec<_> = (1..=20)
        .map(|day| {
            period(
                UsagePeriod::Day(format!("2026-11-{day:02}")),
                1_000_000_000_000_000 - u64::from(day == 20),
                None,
            )
        })
        .collect();
    assert_eq!(
        total(&large.iter().collect::<Vec<_>>())
            .unwrap()
            .total_tokens
            .to_string(),
        "19999999999999999"
    );
}

#[test]
fn range_reads_separate_unplaced_sessions_and_each_pinned_zone() {
    let dir = crate::tests::private_directory();
    let mut storage = Storage::create(&dir.path().join("usage.db"), &KEY).unwrap();
    let pi = [source(&mut storage, "pi", "pi")];
    let days = [
        period(
            UsagePeriod::Day("2026-11-08".into()),
            173,
            Some(("model", None)),
        ),
        period(UsagePeriod::Day("2026-11-09".into()), 10, None),
    ];
    let sessions = [
        session("inside", Some("2026-11-07T18:30:00.000Z")),
        session("before", Some("2026-11-07T18:29:59.999Z")),
        session("after", Some("2026-11-08T18:30:00.000Z")),
        session("untimed", None),
    ];
    save(
        &mut storage,
        "pi",
        &pi,
        Some("Asia/Kolkata"),
        &days,
        &sessions,
    );
    let amp = [source(&mut storage, "amp", "amp")];
    save(&mut storage, "amp", &amp, Some("UTC"), &days[..1], &[]);
    let other = [source(&mut storage, "opencode", "opencode")];
    save(
        &mut storage,
        "opencode",
        &other,
        None,
        &[period(UsagePeriod::Unplaced, 800, None)],
        &[session("unplaced", None)],
    );
    let view = storage
        .local_usage_read("2026-11-08", "2026-11-08")
        .unwrap();
    assert!(view.legacy.is_empty());
    let pi = view.grouped.iter().find(|s| s.scope.tool == "pi").unwrap();
    assert_eq!(pi.dated_total.as_ref().unwrap().total_tokens, 173);
    assert!(pi.unplaced_total.is_none());
    assert_eq!(
        pi.window.as_ref().unwrap().start_utc,
        "2026-11-07T18:30:00.000Z"
    );
    assert_eq!(
        pi.dated_sessions
            .iter()
            .map(|s| s.session.session_key.as_str())
            .collect::<Vec<_>>(),
        ["inside"]
    );
    assert_eq!(pi.unplaced_sessions[0].session.session_key, "untimed");
    let amp = view.grouped.iter().find(|s| s.scope.tool == "amp").unwrap();
    assert_eq!(
        amp.window.as_ref().unwrap().start_utc,
        "2026-11-08T00:00:00.000Z"
    );
    let unplaced = view
        .grouped
        .iter()
        .find(|s| s.scope.tool == "opencode")
        .unwrap();
    assert!(
        unplaced.dated.is_empty() && unplaced.dated_total.is_none() && unplaced.window.is_none()
    );
    assert_eq!(unplaced.unplaced_total.as_ref().unwrap().total_tokens, 800);
    assert!(unplaced.dated_sessions.is_empty());
    assert_eq!(unplaced.unplaced_sessions.len(), 1);
    let other_range = storage
        .local_usage_read("2020-01-01", "2020-01-01")
        .unwrap();
    assert!(other_range.grouped.iter().all(|s| s.dated_total.is_none()));
    assert_eq!(
        other_range
            .grouped
            .iter()
            .find(|s| s.scope.tool == "opencode")
            .unwrap()
            .unplaced,
        unplaced.unplaced
    );
}

#[test]
fn legacy_roots_stay_separate_and_read_only_reopen_does_not_write() {
    let dir = crate::tests::private_directory();
    let path = dir.path().join("usage.db");
    let mut storage = Storage::create(&path, &KEY).unwrap();
    let ids = [
        source(&mut storage, "amp", "one"),
        source(&mut storage, "amp", "two"),
    ];
    for (id, zone) in ids.iter().zip(["UTC", "Asia/Kolkata"]) {
        storage
            .save_local_usage_scan(&LocalUsageScan {
                log_source_ids: std::slice::from_ref(id),
                scanner_version: "20.0.48",
                time_zone: zone,
                since: None,
                scanned_at_utc: NOW,
                rows: &[LocalUsageRow {
                    tool: "amp".into(),
                    local_date: "2026-11-08".into(),
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
    save(
        &mut storage,
        "amp",
        &ids,
        Some("UTC"),
        &[period(
            UsagePeriod::Day("2026-11-08".into()),
            173,
            Some(("new-model", None)),
        )],
        &[],
    );
    save(&mut storage, "amp", &ids[..1], Some("UTC"), &[], &[]);
    let view = storage
        .local_usage_read("2026-11-08", "2026-11-08")
        .unwrap();
    assert_eq!(
        view.grouped[0].dated_total.as_ref().unwrap().total_tokens,
        173
    );
    assert!(view.grouped[0].dated[0].retained);
    assert_eq!(view.grouped[0].dated_differences[0].new_total_tokens, 0);
    assert_eq!(view.legacy.len(), 2);
    assert_ne!(
        view.legacy[0].source.time_zone,
        view.legacy[1].source.time_zone
    );
    assert!(
        view.legacy
            .iter()
            .all(|s| s.days[0].counts.total_tokens == 100)
    );
    drop(storage);
    let before = std::fs::read(&path).unwrap();
    let reader = Storage::open_read_only(&path, &KEY).unwrap();
    assert_eq!(
        reader.local_usage_read("2026-11-08", "2026-11-08").unwrap(),
        view
    );
    drop(reader);
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

#[test]
fn date_ranges_reject_invalid_and_excessive_windows() {
    let dir = crate::tests::private_directory();
    let storage = Storage::create(&dir.path().join("usage.db"), &KEY).unwrap();
    for (from, to) in [
        ("2026-02-30", "2026-03-01"),
        ("2026-11-09", "2026-11-08"),
        ("2024-01-01", "2025-01-01"),
    ] {
        assert_eq!(
            storage.local_usage_read(from, to),
            Err(LocalLogError::InvalidInput)
        );
    }
    assert!(
        storage
            .local_usage_read("2024-01-01", "2024-12-31")
            .unwrap()
            .grouped
            .is_empty()
    );
}

#[test]
fn shared_results_survive_backup_checkpoint_restore_and_lossless_export() {
    let dir = crate::tests::private_directory();
    let mut storage = Storage::create(&dir.path().join("source.db"), &KEY).unwrap();
    let roots = [
        source(&mut storage, "amp", "one"),
        source(&mut storage, "amp", "two"),
    ];
    for (id, zone) in roots.iter().zip(["UTC", "Asia/Kolkata"]) {
        storage
            .save_local_usage_scan(&LocalUsageScan {
                log_source_ids: std::slice::from_ref(id),
                scanner_version: "20.0.48",
                time_zone: zone,
                since: None,
                scanned_at_utc: NOW,
                rows: &[LocalUsageRow {
                    local_date: "2026-11-08".into(),
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
    save(
        &mut storage,
        "amp",
        &roots,
        Some("UTC"),
        &[period(
            UsagePeriod::Day("2026-11-08".into()),
            173,
            Some(("unknown-count", None)),
        )],
        &[session("untimed", None)],
    );
    save(&mut storage, "amp", &roots[..1], Some("UTC"), &[], &[]);
    let other = [source(&mut storage, "opencode", "undated")];
    save(
        &mut storage,
        "opencode",
        &other,
        None,
        &[period(UsagePeriod::Unplaced, 0, None)],
        &[session("unplaced", None)],
    );
    let read = |store: &Storage| store.local_usage_read("2026-11-08", "2026-11-09").unwrap();
    let expected = read(&storage);
    let snapshot = storage.backup_snapshot().unwrap();
    crate::validate_backup_snapshot(&snapshot.bytes, snapshot.schema_version).unwrap();
    let checkpoint = dir.path().join("checkpoint.db");
    storage.create_encrypted_checkpoint(&checkpoint).unwrap();
    assert_eq!(
        read(&Storage::open_read_only(&checkpoint, &KEY).unwrap()),
        expected
    );
    for (name, checkpoint) in [("backup.db", false), ("rollback.db", true)] {
        let mut target = Storage::create(&dir.path().join(name), &KEY).unwrap();
        let revision = target.restore_state().unwrap().revision;
        if checkpoint {
            target
                .replace_records_from_checkpoint(
                    &snapshot.bytes,
                    snapshot.schema_version,
                    &revision,
                    &[],
                )
                .unwrap();
        } else {
            target
                .replace_records_from_backup(
                    &snapshot.bytes,
                    snapshot.schema_version,
                    &revision,
                    &[],
                )
                .unwrap();
        }
        assert_eq!(read(&target), expected);
    }
    let mut bytes = Vec::new();
    storage.write_application_export(&mut bytes, NOW).unwrap();
    let export: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let mut target = Storage::create(&dir.path().join("export.db"), &KEY).unwrap();
    // Test-only reconstruction from our own export. This does not add an import command.
    target
        .connection
        .execute_batch("PRAGMA foreign_keys=OFF")
        .unwrap();
    let tx = target.connection.transaction().unwrap();
    for table in export["tables"].as_array().unwrap() {
        let name = table["name"].as_str().unwrap();
        assert!(name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'));
        tx.execute(&format!("DELETE FROM \"{name}\""), []).unwrap();
        let placeholders = vec!["?"; table["columns"].as_array().unwrap().len()].join(",");
        for row in table["rows"].as_array().unwrap() {
            let values: Vec<Option<&str>> =
                row.as_array().unwrap().iter().map(|v| v.as_str()).collect();
            tx.execute(
                &format!("INSERT INTO \"{name}\" VALUES({placeholders})"),
                rusqlite::params_from_iter(values),
            )
            .unwrap();
        }
    }
    tx.commit().unwrap();
    target
        .connection
        .execute_batch("PRAGMA foreign_keys=ON")
        .unwrap();
    crate::backup_validation::validate_connection(
        &target.connection,
        crate::CURRENT_SCHEMA_VERSION,
    )
    .unwrap();
    assert_eq!(read(&target), expected);
}
