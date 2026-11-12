use super::*;
use openusage_locallogs::{AgentPeriod, AgentSession, ModelDetail, TokenDetails, UsagePeriod};
use rusqlite::{Connection, params};

pub fn time(second: u32) -> String {
    format!("2026-10-18T12:00:{second:02}.000Z")
}

pub fn evidence() -> ProfileIdentityEvidence {
    ProfileIdentityEvidence {
        email: "synthetic@example.invalid".into(),
        plan: None,
    }
}

pub fn connection(path: &std::path::Path) -> Connection {
    let connection = Connection::open(path).unwrap();
    connection
        .pragma_update(None, "key", format!("x'{}'", "48".repeat(32)))
        .unwrap();
    connection
        .pragma_update(None, "foreign_keys", true)
        .unwrap();
    connection
}

pub fn raw_records(path: &std::path::Path) -> BTreeMap<String, Vec<Vec<rusqlite::types::Value>>> {
    let connection = connection(path);
    let mut names = connection
        .prepare("SELECT name FROM sqlite_schema WHERE type='table' ORDER BY name")
        .unwrap();
    let names = names
        .query_map([], |r| r.get::<_, String>(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    names
        .into_iter()
        .map(|name| {
            let mut statement = connection
                .prepare(&format!("SELECT * FROM \"{name}\" ORDER BY rowid"))
                .unwrap();
            let count = statement.column_count();
            let rows = statement
                .query_map([], |r| (0..count).map(|i| r.get(i)).collect())
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap();
            (name, rows)
        })
        .collect()
}

pub fn populated() -> Fixture {
    let mut fixture = Fixture::new();
    fixture.storage.set_theme(Theme::Dark).unwrap();
    let overrides = (1..=2)
        .map(|n| ResetAlertOverride {
            profile_id: id(n),
            enabled: true,
        })
        .collect::<Vec<_>>();
    let operational = OperationalAlertPreferences {
        enabled: true,
        profile_overrides: overrides.clone(),
    };
    fixture
        .storage
        .save_notification_preferences(
            &ResetAlertPreferences {
                enabled: true,
                quiet_enabled: false,
                quiet_hours: AlertQuietHours {
                    time_zone: "UTC".into(),
                    start_minute: 1320,
                    end_minute: 480,
                },
                profile_overrides: overrides.clone(),
            },
            &ThresholdAlertPreferences {
                enabled: true,
                profile_overrides: overrides,
            },
            &operational,
            &operational,
        )
        .unwrap();
    for n in 1..=2 {
        profile_history(&mut fixture.storage, n);
        fixture
            .storage
            .create_subscription(
                &id(n),
                &id(n * 1000 + 80),
                &id(n * 1000 + 81),
                SubscriptionInput {
                    plan: "Synthetic subscription".into(),
                    tier: None,
                    currency: "USD".into(),
                    price: Some("0".into()),
                    interval_unit: Some(SubscriptionInterval::Month),
                    interval_count: Some(1),
                    start_date: None,
                    renewal_date: None,
                    end_date: None,
                    cancellation_date: None,
                    auto_renew: None,
                },
                NOW,
            )
            .unwrap();
    }
    let conn = connection(&fixture.path());
    for n in 90..=92 {
        conn.execute("INSERT INTO accounts VALUES (?1,NULL,'unknown','synthetic@example.invalid',NULL,?2,?2)", params![id(n), NOW]).unwrap();
    }
    conn.execute("UPDATE profiles SET account_id=?1", [id(90)])
        .unwrap();
    drop(conn);
    local_usage(&mut fixture);
    let snapshot = fixture.storage.backup_snapshot().unwrap();
    validate_backup_snapshot(&snapshot.bytes, snapshot.schema_version).unwrap();
    fixture
}

fn profile_history(storage: &mut Storage, n: u32) {
    let base = n * 1000;
    let mut previous = None;
    for generation in 0..2 {
        let offset = generation * 10;
        let binding = ProfileBinding {
            profile_id: id(n),
            home_reference: id(n + 10),
            binding_id: id(base + offset + 1),
        };
        for revision in 0..2 {
            let now = time(offset + revision);
            let reading = ActivityReading {
                run_id: id(base + offset + 2 + revision),
                binding: binding.clone(),
                identity_evidence: evidence(),
                provider_version: "0.163.0".into(),
                source_kind: ActivitySourceKind::OfficialArtifact,
                started_at_utc: now.clone(),
                observed_at_utc: now.clone(),
                completed_at_utc: now.clone(),
                summary: ActivitySummaryValues {
                    lifetime_tokens: Some("0".into()),
                    current_streak_days: Some("0".into()),
                    ..Default::default()
                },
                daily_usage: Some(vec![ActivityDayValue {
                    provider_date: "2026-10-18".into(),
                    tokens: revision.to_string(),
                }]),
            };
            let association = (revision == 0).then(|| ProfileBindingInput {
                profile_id: id(n),
                expected_home_reference: id(n + 10),
                expected_previous_binding_id: previous.clone(),
                new_binding_id: binding.binding_id.clone(),
                evidence: evidence(),
                observed_at_utc: now,
            });
            storage
                .accept_activity_with_binding(reading, association)
                .unwrap();
        }
        for revision in 0..2 {
            let now = time(offset + revision + 2);
            let reading = QuotaReading {
                run_id: id(base + offset + 4 + revision),
                binding: binding.clone(),
                identity_evidence: evidence(),
                provider_version: "0.163.0".into(),
                source_kind: ActivitySourceKind::OfficialArtifact,
                started_at_utc: now.clone(),
                observed_at_utc: now.clone(),
                completed_at_utc: now.clone(),
                limits: vec![QuotaLimit {
                    limit_id: "codex".into(),
                    limit_name: None,
                    primary: Some(QuotaWindow {
                        used_percent: if revision == 0 { 100 } else { 95 },
                        window_duration_mins: Some(300),
                        resets_at_unix: Some(i64::from(revision)),
                    }),
                    secondary: None,
                    reached_type: None,
                    monthly_credit_limit: None,
                    spend_control_reached: None,
                }],
                available_reset_credits: Some("0".into()),
                reset_credit_details: Some(vec![QuotaCreditDetail {
                    title: None,
                    status: QuotaCreditStatus::Unknown,
                    granted_at_unix: 0,
                    expires_at_unix: None,
                }]),
            };
            storage.accept_live_quota(reading, None, &now).unwrap();
        }
        previous = Some(binding.binding_id);
    }
    storage
        .record_profile_connection(&id(n), &id(n + 10), ProfileConnectionState::Connected)
        .unwrap();
    for second in 24..=27 {
        storage
            .record_operational_failure(
                &OperationalFailure {
                    request_id: id(base + second),
                    profile_id: id(n),
                    expected_home_reference: id(n + 10),
                    expected_binding_id: previous.clone(),
                    provider_version: "0.163.0".into(),
                    source_kind: ActivitySourceKind::OfficialArtifact,
                    observed_at_utc: time(second),
                    kind: if second == 27 {
                        OperationalFailureKind::AuthenticationRequired
                    } else {
                        OperationalFailureKind::TransientCollectionFailure
                    },
                },
                &time(second),
            )
            .unwrap();
    }
}

fn local_usage(fixture: &mut Fixture) {
    for name in [
        "managed-home",
        "external-home",
        "wsl-home",
        "other-tool-logs",
    ] {
        let path = fixture.directory.path().join(name);
        fs::create_dir(&path).unwrap();
        fs::write(path.join("sentinel"), format!("synthetic {name}")).unwrap();
    }
    let folder = fixture.directory.path().join("managed-home");
    let source = fixture
        .storage
        .set_log_source("claude", folder.to_str().unwrap(), LogConsent::Granted, NOW)
        .unwrap()
        .id;
    let sources = [source.clone()];
    for total in [5, 3] {
        fixture
            .storage
            .save_local_usage_scan(&LocalUsageScan {
                log_source_ids: &sources,
                scanner_version: "20.0.48",
                time_zone: "UTC",
                since: None,
                scanned_at_utc: NOW,
                rows: &[LocalUsageRow {
                    local_date: "2026-10-18".into(),
                    tool: "claude".into(),
                    model: "synthetic".into(),
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
    fixture
        .storage
        .save_local_sessions(&LocalSessionScan {
            log_source_id: &source,
            scanner_version: "20.0.48",
            scanned_at_utc: NOW,
            sessions: &[LocalUsageSession {
                tool: "claude".into(),
                session_key: "synthetic-session".into(),
                project_name: Some("Synthetic".into()),
                start_utc: None,
                end_utc: NOW.into(),
                input_tokens: 5,
                output_tokens: 0,
                cache_write_tokens: 0,
                cache_read_tokens: 0,
                reasoning_tokens: None,
                total_tokens: 5,
                estimated_value_usd_micros: None,
            }],
        })
        .unwrap();
    fixture
        .storage
        .save_local_hours(&LocalHourScan {
            log_source_id: &source,
            scanner_version: "20.0.48",
            scanned_at_utc: NOW,
            hours: &[LocalUsageHour {
                tool: "claude".into(),
                hour_start_utc: NOW.into(),
                input_tokens: 5,
                output_tokens: 0,
                cache_write_tokens: 0,
                cache_read_tokens: 0,
                total_tokens: 5,
            }],
        })
        .unwrap();
    let source = fixture
        .storage
        .set_log_source(
            "pi",
            fixture
                .directory
                .path()
                .join("other-tool-logs")
                .to_str()
                .unwrap(),
            LogConsent::Granted,
            NOW,
        )
        .unwrap()
        .id;
    for total in [5, 3] {
        let models = vec![ModelDetail {
            model: "synthetic".into(),
            total_tokens: Some(total),
            details: TokenDetails::default(),
            estimated_value_micros: None,
        }];
        fixture
            .storage
            .save_grouped_local_scan(&GroupedLocalScan {
                tool: "pi",
                source_ids: std::slice::from_ref(&source),
                time_zone: Some("UTC"),
                scanner_version: "20.0.48",
                observed_at_utc: NOW,
                periods: &[AgentPeriod {
                    period: UsagePeriod::Day("2026-10-18".into()),
                    total_tokens: total,
                    details: TokenDetails::default(),
                    estimated_value_micros: None,
                    models: models.clone(),
                }],
                sessions: Some(&[AgentSession {
                    session_key: "synthetic-session".into(),
                    project: None,
                    first_activity_utc: None,
                    last_activity_utc: None,
                    total_tokens: total,
                    details: TokenDetails::default(),
                    estimated_value_micros: None,
                    models,
                }]),
            })
            .unwrap();
    }
}

pub fn consume_notifications(storage: &mut Storage) -> usize {
    let mut consumed = 0;
    while let Some(notification) = storage.claim_notification(&time(28)).unwrap() {
        let (event, attempt) = match notification {
            PendingNotification::Quota(value) => (value.event_id, value.attempt_id),
            PendingNotification::Operational(value) => (value.event_id, value.attempt_id),
        };
        storage
            .complete_notification(&event, &attempt, NotificationOutcome::Submitted)
            .unwrap();
        consumed += 1;
        assert!(consumed < 30);
    }
    consumed
}

pub fn assert_sentinels(fixture: &Fixture) {
    for name in [
        "managed-home",
        "external-home",
        "wsl-home",
        "other-tool-logs",
    ] {
        assert_eq!(
            fs::read_to_string(fixture.directory.path().join(name).join("sentinel")).unwrap(),
            format!("synthetic {name}")
        );
    }
}
