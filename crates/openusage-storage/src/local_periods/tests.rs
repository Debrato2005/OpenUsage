use super::*;
use crate::{
    CURRENT_SCHEMA_VERSION, LocalUsageRow, LocalUsageScan, LogConsent, MIGRATIONS,
    backup_validation::validate_connection,
};
const NOW: &str = "2026-11-09T12:00:00.000Z";
const KEY: [u8; 32] = [71; 32];
fn setup(id: &str) -> (tempfile::TempDir, Storage, Vec<String>) {
    let dir = crate::tests::private_directory();
    let mut storage = Storage::create(&dir.path().join("usage.db"), &KEY).unwrap();
    let sources = ["a", "b"]
        .map(|suffix| {
            storage
                .set_log_source(
                    id,
                    &format!("/srv/synthetic/{suffix}"),
                    LogConsent::Granted,
                    NOW,
                )
                .unwrap()
                .id
        })
        .to_vec();
    (dir, storage, sources)
}
fn period(total: u64, model: Option<(&str, u64)>) -> AgentPeriod {
    AgentPeriod {
        period: UsagePeriod::Day("2026-11-08".into()),
        total_tokens: total,
        details: TokenDetails::default(),
        estimated_value_micros: None,
        models: model
            .into_iter()
            .map(|(name, tokens)| ModelDetail {
                model: name.into(),
                total_tokens: Some(tokens),
                details: TokenDetails::default(),
                estimated_value_micros: None,
            })
            .collect(),
    }
}
fn session(total: u64) -> AgentSession {
    AgentSession {
        session_key: "session-a".into(),
        project: None,
        first_activity_utc: None,
        last_activity_utc: None,
        total_tokens: total,
        details: TokenDetails::default(),
        estimated_value_micros: None,
        models: vec![],
    }
}
fn scan<'a>(
    id: &'a str,
    sources: &'a [String],
    periods: &'a [AgentPeriod],
    sessions: Option<&'a [AgentSession]>,
) -> GroupedLocalScan<'a> {
    GroupedLocalScan {
        tool: id,
        source_ids: sources,
        time_zone: (period_kind(id).unwrap() == "day").then_some("UTC"),
        scanner_version: "20.0.48",
        observed_at_utc: NOW,
        periods,
        sessions,
    }
}
#[test]
fn independent_totals_nullable_details_and_scope_survive_reopen() {
    let (dir, mut storage, sources) = setup("amp");
    let mut day = period(173, None);
    day.models.push(ModelDetail {
        model: "unknown-model".into(),
        total_tokens: None,
        details: TokenDetails::default(),
        estimated_value_micros: None,
    });
    let sessions = [session(173)];
    storage
        .save_grouped_local_scan(&scan("amp", &sources, &[day.clone()], Some(&sessions)))
        .unwrap();
    let saved = storage.local_periods("amp").unwrap();
    assert_eq!(saved[0].period, day);
    assert_eq!(saved[0].scan.source_ids.len(), 2);
    assert_eq!(
        storage.grouped_local_sessions("amp").unwrap()[0].session,
        sessions[0]
    );
    assert!(
        storage
            .local_usage_days("2026-01-01", "2026-12-31")
            .unwrap()
            .is_empty()
    );
    // Grouped scans do not change old source zones or dates.
    assert!(
        storage
            .log_sources()
            .unwrap()
            .iter()
            .all(|s| s.time_zone.is_none())
    );
    validate_connection(&storage.connection, CURRENT_SCHEMA_VERSION).unwrap();
    drop(storage);
    let storage = Storage::open(&dir.path().join("usage.db"), &KEY).unwrap();
    assert_eq!(storage.local_periods("amp").unwrap(), saved);
}

#[test]
fn pi_zero_observations_and_absent_times_survive_reopen_and_lower_scans() {
    let reports: serde_json::Value = serde_json::from_slice(include_bytes!(
        "../../../openusage-locallogs/tests/fixtures/remaining-tools/pi-observation-candidate.json"
    ))
    .unwrap();
    let parse = |case: &str| {
        let report = &reports[case];
        let periods = openusage_locallogs::parse_agent_periods(
            "pi",
            &serde_json::to_vec(&report["daily"]).unwrap(),
        )
        .unwrap()
        .map(|r| r.periods)
        .unwrap_or_default();
        let sessions = openusage_locallogs::parse_agent_sessions(
            "pi",
            &serde_json::to_vec(&report["session"]).unwrap(),
        )
        .unwrap()
        .map(|r| r.sessions)
        .unwrap_or_default();
        (periods, sessions)
    };
    let (dir, mut storage, sources) = setup("pi");
    let (empty_days, empty_sessions) = parse("empty");
    storage
        .save_grouped_local_scan(&scan("pi", &sources, &empty_days, Some(&empty_sessions)))
        .unwrap();
    assert!(storage.local_periods("pi").unwrap().is_empty());
    let (zero_days, mut zero_sessions) = parse("message_zero_total_only");
    zero_sessions[0].first_activity_utc = None;
    zero_sessions[0].last_activity_utc = None;
    storage
        .save_grouped_local_scan(&scan("pi", &sources, &zero_days, Some(&zero_sessions)))
        .unwrap();
    drop(storage);
    let mut storage = Storage::open(&dir.path().join("usage.db"), &KEY).unwrap();
    let saved_zero = storage.local_periods("pi").unwrap();
    assert_eq!(saved_zero.len(), 1);
    assert_eq!(saved_zero[0].period, zero_days[0]);
    assert_eq!(saved_zero[0].period.details, TokenDetails::default());
    assert_eq!(saved_zero[0].period.estimated_value_micros, None);
    assert_eq!(
        storage.grouped_local_sessions("pi").unwrap()[0].session,
        zero_sessions[0]
    );
    let (days, sessions) = parse("message_partial");
    storage
        .save_grouped_local_scan(&scan("pi", &sources, &days, Some(&sessions)))
        .unwrap();
    let accepted = storage.local_periods("pi").unwrap()[0].period.clone();
    for (days, sessions) in [(&zero_days, &zero_sessions), (&empty_days, &empty_sessions)] {
        storage
            .save_grouped_local_scan(&scan("pi", &sources, days, Some(sessions)))
            .unwrap();
        assert_eq!(storage.local_periods("pi").unwrap()[0].period, accepted);
        assert_eq!(
            storage.grouped_local_sessions("pi").unwrap()[0]
                .session
                .total_tokens,
            162
        );
    }
    let mut invalid = days;
    invalid[0].details.input_tokens = Some(163);
    assert_eq!(
        storage.save_grouped_local_scan(&scan("pi", &sources, &invalid, Some(&zero_sessions))),
        Err(LocalLogError::InvalidInput)
    );
    assert_eq!(storage.local_periods("pi").unwrap()[0].period, accepted);
    assert!(
        storage
            .local_scan_differences("pi")
            .unwrap()
            .iter()
            .any(|d| d.saved_total_tokens == 162 && d.new_total_tokens == 0)
    );
    validate_connection(&storage.connection, CURRENT_SCHEMA_VERSION).unwrap();
}

#[test]
fn pi_incomplete_producer_prefixes_do_not_lower_saved_history() {
    let snapshots: serde_json::Value = serde_json::from_slice(include_bytes!(
        "../../../openusage-locallogs/tests/fixtures/remaining-tools/pi-producer-schedules.json"
    ))
    .unwrap();
    let baseline: serde_json::Value = serde_json::from_slice(include_bytes!(
        "../../../openusage-locallogs/tests/fixtures/remaining-tools/pi-producer-candidate.json"
    ))
    .unwrap();
    let parse = |report: &serde_json::Value| {
        let days = openusage_locallogs::parse_agent_periods(
            "pi",
            &serde_json::to_vec(&report["daily"]).unwrap(),
        )
        .unwrap()
        .map(|r| r.periods)
        .unwrap_or_default();
        let sessions = openusage_locallogs::parse_agent_sessions(
            "pi",
            &serde_json::to_vec(&report["session"]).unwrap(),
        )
        .unwrap()
        .map(|r| r.sessions)
        .unwrap_or_default();
        (days, sessions)
    };
    let (_dir, mut storage, sources) = setup("pi");
    let (days, sessions) = parse(&baseline["assistant"]);
    storage
        .save_grouped_local_scan(&scan("pi", &sources, &days, Some(&sessions)))
        .unwrap();
    for case in ["paused_writer_header", "paused_writer_usage"] {
        let (prefix_days, prefix_sessions) = parse(&snapshots[case]["observedPrefix"]);
        let outcome = storage
            .save_grouped_local_scan(&scan("pi", &sources, &prefix_days, Some(&prefix_sessions)))
            .unwrap();
        assert_eq!(outcome.kept_higher, 2);
        assert_eq!(storage.local_periods("pi").unwrap()[0].period, days[0]);
        assert_eq!(
            storage.grouped_local_sessions("pi").unwrap()[0].session,
            sessions[0]
        );
    }
    assert_eq!(storage.local_scan_differences("pi").unwrap().len(), 4);
    let (days, sessions) = parse(&snapshots["paused_writer_usage"]["completed"]);
    storage
        .save_grouped_local_scan(&scan("pi", &sources, &days, Some(&sessions)))
        .unwrap();
    assert_eq!(
        storage.local_periods("pi").unwrap()[0].period.total_tokens,
        137
    );
    assert_eq!(
        storage.grouped_local_sessions("pi").unwrap()[0]
            .session
            .total_tokens,
        137
    );
    validate_connection(&storage.connection, CURRENT_SCHEMA_VERSION).unwrap();
}
#[test]
fn equal_or_higher_replaces_whole_snapshot_lower_keeps_it_once() {
    let (_dir, mut storage, sources) = setup("pi");
    storage
        .save_grouped_local_scan(&scan(
            "pi",
            &sources,
            &[period(100, Some(("old", 100)))],
            Some(&[session(100)]),
        ))
        .unwrap();
    let mut next = period(100, Some(("new", 100)));
    next.details.input_tokens = Some(100);
    next.details.output_tokens = Some(0);
    next.details.cache_write_tokens = Some(0);
    next.details.cache_read_tokens = Some(0);
    storage
        .save_grouped_local_scan(&scan("pi", &sources[..1], &[next.clone()], None))
        .unwrap();
    let accepted = storage.local_periods("pi").unwrap().remove(0);
    assert_eq!(accepted.period, next);
    assert_eq!(accepted.scan.source_ids, sources[..1]);
    for _ in 0..2 {
        let result = storage
            .save_grouped_local_scan(&scan(
                "pi",
                &sources,
                &[period(80, Some(("other", 80)))],
                Some(&[session(50)]),
            ))
            .unwrap();
        assert_eq!(result.kept_higher, 2);
    }
    let lower = storage.local_periods("pi").unwrap().remove(0);
    assert!(lower.retained);
    assert_eq!(lower.period, accepted.period);
    assert_eq!(lower.scan, accepted.scan);
    assert_eq!(storage.local_scan_differences("pi").unwrap().len(), 2);
    assert_eq!(
        storage.grouped_local_sessions("pi").unwrap()[0]
            .session
            .total_tokens,
        100
    );
    storage
        .save_grouped_local_scan(&scan(
            "pi",
            &sources,
            &[period(120, Some(("higher", 120)))],
            None,
        ))
        .unwrap();
    let raised = storage.local_periods("pi").unwrap().remove(0);
    assert!(!raised.retained);
    assert_eq!(raised.period.models[0].model, "higher");
    assert_eq!(raised.period.details.input_tokens, None);
    storage
        .save_grouped_local_scan(&scan("pi", &sources, &[], None))
        .unwrap();
    let diff = storage.local_scan_differences("pi").unwrap();
    assert!(
        diff.iter()
            .any(|d| d.saved_total_tokens == 120 && d.new_total_tokens == 0)
    );
    validate_connection(&storage.connection, CURRENT_SCHEMA_VERSION).unwrap();
}
#[test]
fn unplaced_is_one_per_tool_and_never_a_dated_value() {
    let (_dir, mut storage, sources) = setup("opencode");
    let mut unplaced = period(0, None);
    unplaced.period = UsagePeriod::Unplaced;
    storage
        .save_grouped_local_scan(&scan(
            "opencode",
            &sources,
            &[unplaced.clone()],
            Some(&[session(0)]),
        ))
        .unwrap();
    assert_eq!(storage.local_periods("opencode").unwrap().len(), 1);
    assert!(
        storage
            .local_dated_periods("opencode", "2026-01-01", "2026-12-31")
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        storage
            .local_tool_scope("opencode")
            .unwrap()
            .unwrap()
            .time_zone,
        None
    );
    assert_eq!(
        storage.save_grouped_local_scan(&scan(
            "opencode",
            &sources,
            &[unplaced.clone(), unplaced],
            None
        )),
        Err(LocalLogError::InvalidInput)
    );
    assert_eq!(
        storage.save_grouped_local_scan(&scan("opencode", &sources, &[period(0, None)], None)),
        Err(LocalLogError::InvalidInput)
    );
}
#[test]
fn consent_zone_and_atomic_rejection_are_enforced() {
    let (_dir, mut storage, sources) = setup("kilo");
    storage
        .save_grouped_local_scan(&scan("kilo", &sources, &[period(10, None)], None))
        .unwrap();
    let before = storage.local_tool_scope("kilo").unwrap();
    let periods = [period(20, None)];
    let mut input = scan("kilo", &sources, &periods, None);
    input.time_zone = Some("Asia/Kolkata");
    assert_eq!(
        storage.save_grouped_local_scan(&input),
        Err(LocalLogError::TimeZoneMismatch)
    );
    let invalid = [AgentSession {
        last_activity_utc: Some("2026-02-30T00:00:00.000Z".into()),
        ..session(20)
    }];
    assert_eq!(
        storage.save_grouped_local_scan(&scan("kilo", &sources, &periods, Some(&invalid))),
        Err(LocalLogError::InvalidInput)
    );
    storage
        .set_log_source("kilo", "/srv/synthetic/b", LogConsent::Withdrawn, NOW)
        .unwrap();
    assert_eq!(
        storage.save_grouped_local_scan(&scan("kilo", &sources, &periods, None)),
        Err(LocalLogError::NotConsented)
    );
    assert_eq!(
        storage.save_grouped_local_scan(&scan("amp", &sources, &periods, None)),
        Err(LocalLogError::NotConsented)
    );
    assert_eq!(
        storage.save_grouped_local_scan(&scan(
            "kilo",
            &[sources[0].clone(), sources[0].clone()],
            &periods,
            None
        )),
        Err(LocalLogError::InvalidInput)
    );
    assert_eq!(storage.local_tool_scope("kilo").unwrap(), before);
    assert_eq!(
        storage.local_periods("kilo").unwrap()[0]
            .period
            .total_tokens,
        10
    );
    // Historical provenance remains valid after consent withdrawal.
    validate_connection(&storage.connection, CURRENT_SCHEMA_VERSION).unwrap();
}
#[test]
fn malformed_counts_details_and_metadata_are_rejected_without_overflow() {
    let (_dir, mut storage, sources) = setup("zcode");
    for index in 0..8 {
        let mut invalid = period(173, None);
        match index {
            0 => invalid.total_tokens = u64::MAX,
            1 => invalid.details.input_tokens = Some(u64::MAX),
            2 => invalid.details.reasoning_tokens = Some(174),
            3 => invalid.models = period(173, Some(("m", 162))).models,
            4 => {
                invalid.models = vec![ModelDetail {
                    model: "m".into(),
                    total_tokens: None,
                    details: TokenDetails {
                        input_tokens: Some(174),
                        ..TokenDetails::default()
                    },
                    estimated_value_micros: None,
                }]
            }
            5 => invalid.estimated_value_micros = Some(MAX_COUNT + 1),
            6 => {
                invalid.details = TokenDetails {
                    input_tokens: Some(170),
                    output_tokens: Some(0),
                    cache_write_tokens: Some(0),
                    cache_read_tokens: Some(0),
                    reasoning_tokens: None,
                }
            }
            _ => invalid.period = UsagePeriod::Day("2026-02-30".into()),
        }
        assert_eq!(
            storage.save_grouped_local_scan(&scan("zcode", &sources, &[invalid], None)),
            Err(LocalLogError::InvalidInput),
            "case {index}"
        );
    }
    assert!(storage.local_tool_scope("zcode").unwrap().is_none());
}
fn legacy(storage: &mut Storage, source: &str, zone: &str, model: &str, total: u64) {
    storage
        .save_local_usage_scan(&LocalUsageScan {
            log_source_ids: &[source.into()],
            scanner_version: "20.0.48",
            time_zone: zone,
            since: None,
            scanned_at_utc: NOW,
            rows: &[LocalUsageRow {
                local_date: "2026-11-08".into(),
                tool: "claude".into(),
                model: model.into(),
                input_tokens: total,
                output_tokens: 0,
                cache_write_tokens: 0,
                cache_read_tokens: 0,
                total_tokens: total,
                estimated_value_usd_micros: None,
            }],
        })
        .unwrap();
}
fn downgrade_to_twelve(storage: &mut Storage) {
    for (table, _) in TABLES.iter().rev() {
        storage
            .connection
            .execute_batch(&format!("DROP TABLE {table}"))
            .unwrap();
    }
    storage
        .connection
        .pragma_update(None, "user_version", 12)
        .unwrap();
}
#[test]
fn additive_migration_and_old_backup_preserve_overlapping_history_and_zones() {
    let (_dir, mut storage, sources) = setup("claude");
    legacy(&mut storage, &sources[0], "UTC", "one", MAX_COUNT);
    legacy(&mut storage, &sources[0], "UTC", "two", MAX_COUNT);
    legacy(&mut storage, &sources[1], "Asia/Kolkata", "one", MAX_COUNT);
    let old_days = storage
        .local_usage_days("2026-01-01", "2026-12-31")
        .unwrap();
    let old_sources = storage.log_sources().unwrap();
    let old_differences = storage
        .local_usage_differences("2026-01-01", "2026-12-31")
        .unwrap();
    downgrade_to_twelve(&mut storage);
    validate_connection(&storage.connection, 12).unwrap();
    let copy =
        crate::backup_validation::trusted_memory_copy(&storage.connection, "main", 12).unwrap();
    let bytes = copy.serialize("main").unwrap().to_vec();
    crate::validate_backup_snapshot(&bytes, 12).unwrap();
    crate::migrate_data_connection(&mut storage.connection).unwrap();
    assert_eq!(
        storage
            .local_usage_days("2026-01-01", "2026-12-31")
            .unwrap(),
        old_days
    );
    assert_eq!(storage.log_sources().unwrap(), old_sources);
    assert_eq!(
        storage
            .local_usage_differences("2026-01-01", "2026-12-31")
            .unwrap(),
        old_differences
    );
    assert!(storage.local_periods("claude").unwrap().is_empty());
    let (_target_dir, mut target, target_sources) = setup("claude");
    target
        .save_grouped_local_scan(&scan("claude", &target_sources, &[period(5, None)], None))
        .unwrap();
    let revision = target.restore_state().unwrap().revision;
    target
        .replace_records_from_backup(&bytes, 12, &revision, &[])
        .unwrap();
    assert_eq!(target.log_sources().unwrap(), old_sources);
    assert_eq!(
        target.local_usage_days("2026-01-01", "2026-12-31").unwrap(),
        old_days
    );
    assert!(target.local_tool_scope("claude").unwrap().is_none());
}
#[test]
fn failed_forward_migration_rolls_back_schema_version_and_legacy_data() {
    let (_dir, mut storage, sources) = setup("claude");
    legacy(&mut storage, &sources[0], "UTC", "m", 50);
    downgrade_to_twelve(&mut storage);
    let mut migrations = MIGRATIONS.to_vec();
    migrations.push("SELECT missing_column FROM settings;");
    assert!(crate::migrate(&mut storage.connection, &migrations).is_err());
    assert_eq!(storage.metadata().unwrap().schema_version, 12);
    validate_connection(&storage.connection, 12).unwrap();
    assert_eq!(
        storage
            .local_usage_days("2026-01-01", "2026-12-31")
            .unwrap()[0]
            .counts
            .total_tokens,
        50
    );
}
#[test]
fn current_backup_checkpoint_restore_and_export_preserve_nullable_snapshots() {
    let (dir, mut storage, sources) = setup("amp");
    storage
        .save_grouped_local_scan(&scan(
            "amp",
            &sources,
            &[period(173, None)],
            Some(&[session(173)]),
        ))
        .unwrap();
    storage
        .save_grouped_local_scan(&scan("amp", &sources, &[], None))
        .unwrap();
    let expected = (
        storage.local_periods("amp").unwrap(),
        storage.grouped_local_sessions("amp").unwrap(),
        storage.local_scan_differences("amp").unwrap(),
        storage.local_tool_scope("amp").unwrap(),
    );
    let snapshot = storage.backup_snapshot().unwrap();
    crate::validate_backup_snapshot(&snapshot.bytes, snapshot.schema_version).unwrap();
    let checkpoint = dir.path().join("checkpoint.db");
    storage.create_encrypted_checkpoint(&checkpoint).unwrap();
    let reopened = Storage::open(&checkpoint, &KEY).unwrap();
    assert_eq!(reopened.local_periods("amp").unwrap(), expected.0);
    let (_other_dir, mut target, _) = setup("pi");
    let revision = target.restore_state().unwrap().revision;
    target
        .replace_records_from_checkpoint(&snapshot.bytes, snapshot.schema_version, &revision, &[])
        .unwrap();
    assert_eq!(
        (
            target.local_periods("amp").unwrap(),
            target.grouped_local_sessions("amp").unwrap(),
            target.local_scan_differences("amp").unwrap(),
            target.local_tool_scope("amp").unwrap()
        ),
        expected
    );
    let mut json = Vec::new();
    target.write_application_export(&mut json, NOW).unwrap();
    let document: serde_json::Value = serde_json::from_slice(&json).unwrap();
    let table = document["tables"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "local_usage_records")
        .unwrap();
    assert_eq!(table["rows"].as_array().unwrap().len(), 2);
    assert!(String::from_utf8(json).unwrap().contains("\"173\""));
}
#[test]
fn backup_checks_reject_inconsistent_grouped_rows_and_cross_tool_roots() {
    let (_dir, mut storage, sources) = setup("amp");
    storage
        .save_grouped_local_scan(&scan(
            "amp",
            &sources,
            &[period(173, Some(("m", 173)))],
            None,
        ))
        .unwrap();
    for sql in [
        "UPDATE local_usage_records SET input_tokens='174'",
        "UPDATE local_usage_records SET input_tokens='173',reasoning_tokens='1'",
        "UPDATE local_usage_records SET total_tokens='0173'",
        "UPDATE local_usage_records SET last_activity_utc='2026-11-08T01:00:00.000Z'",
        "UPDATE local_usage_record_models SET total_tokens='172'",
        "UPDATE local_usage_scopes SET time_zone='Mars/Unknown'",
        "UPDATE local_usage_scans SET scanner_version='unsafe version'",
        "UPDATE local_usage_scans SET latest=0",
        "DELETE FROM local_usage_scan_roots",
        "UPDATE log_sources SET tool='pi'",
    ] {
        storage
            .connection
            .pragma_update(None, "foreign_keys", false)
            .unwrap();
        storage
            .connection
            .pragma_update(None, "ignore_check_constraints", true)
            .unwrap();
        storage
            .connection
            .execute_batch("SAVEPOINT malformed")
            .unwrap();
        storage.connection.execute_batch(sql).unwrap();
        assert!(
            validate_connection(&storage.connection, CURRENT_SCHEMA_VERSION).is_err(),
            "{sql}"
        );
        storage
            .connection
            .execute_batch("ROLLBACK TO malformed; RELEASE malformed")
            .unwrap();
        storage
            .connection
            .pragma_update(None, "ignore_check_constraints", false)
            .unwrap();
        storage
            .connection
            .pragma_update(None, "foreign_keys", true)
            .unwrap();
    }
}

#[test]
fn write_failure_rolls_back_scope_roots_and_whole_snapshot() {
    let (_dir, mut storage, sources) = setup("amp");
    storage
        .save_grouped_local_scan(&scan(
            "amp",
            &sources,
            &[period(100, Some(("m", 100)))],
            None,
        ))
        .unwrap();
    let before = (
        storage.local_tool_scope("amp").unwrap(),
        storage.local_periods("amp").unwrap(),
    );
    storage.connection.execute_batch("CREATE TEMP TRIGGER reject_model BEFORE INSERT ON local_usage_record_models BEGIN SELECT RAISE(ABORT, 'synthetic failure'); END;").unwrap();
    assert_eq!(
        storage.save_grouped_local_scan(&scan(
            "amp",
            &sources[..1],
            &[period(120, Some(("n", 120)))],
            None
        )),
        Err(LocalLogError::WriteFailed)
    );
    assert_eq!(
        (
            storage.local_tool_scope("amp").unwrap(),
            storage.local_periods("amp").unwrap()
        ),
        before
    );
    storage
        .connection
        .execute_batch("DROP TRIGGER reject_model")
        .unwrap();
    validate_connection(&storage.connection, CURRENT_SCHEMA_VERSION).unwrap();
}

#[test]
fn partial_details_cannot_contradict_known_totals_or_reasoning_bounds() {
    let (_dir, mut storage, sources) = setup("pi");
    let model = |name: &str, input, output, read, reasoning| ModelDetail {
        model: name.into(),
        total_tokens: None,
        details: TokenDetails {
            input_tokens: input,
            output_tokens: output,
            cache_read_tokens: read,
            reasoning_tokens: reasoning,
            ..TokenDetails::default()
        },
        estimated_value_micros: None,
    };
    let mut impossible_reasoning = period(100, None);
    impossible_reasoning.details.input_tokens = Some(100);
    impossible_reasoning.details.reasoning_tokens = Some(1);
    let mut impossible_models = period(173, None);
    impossible_models.models = vec![
        model("a", Some(100), None, None, None),
        model("b", None, Some(100), None, None),
    ];
    let mut conflicting_details = period(173, None);
    conflicting_details.details.input_tokens = Some(80);
    conflicting_details.models = vec![model("a", None, None, Some(80), Some(20))];
    for invalid in [impossible_reasoning, impossible_models, conflicting_details] {
        assert_eq!(
            storage.save_grouped_local_scan(&scan("pi", &sources, &[invalid], None)),
            Err(LocalLogError::InvalidInput)
        );
    }
    let mut partial = period(173, None);
    partial.details.input_tokens = Some(150);
    partial.models = vec![model("known", Some(100), None, None, Some(10))];
    storage
        .save_grouped_local_scan(&scan("pi", &sources, &[partial.clone()], None))
        .unwrap();
    assert_eq!(storage.local_periods("pi").unwrap()[0].period, partial);
}
