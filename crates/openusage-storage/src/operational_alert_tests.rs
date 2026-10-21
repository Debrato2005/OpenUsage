//! Synthetic storage evidence proves incident behavior, not real provider failures or OS visibility.
use super::*;
use crate::{
    ActivityReading, ActivitySummaryValues, AlertDeliveryState, NotificationOutcome,
    OperationalAlertKind, OperationalAlertPreferences, OperationalFailure, OperationalFailureKind,
    PendingNotification, ResetAlertPreferences, ThresholdAlertPreferences,
};
fn time(n: u32) -> String {
    format!("2026-10-18T12:00:{n:02}.000Z")
}
fn prefs(enabled: bool) -> OperationalAlertPreferences {
    OperationalAlertPreferences {
        enabled,
        profile_overrides: vec![],
    }
}
fn baseline(kind: ActivitySourceKind, enabled: bool) -> (tempfile::TempDir, Storage, QuotaReading) {
    let (dir, mut storage, mut original) = fixture();
    original.source_kind = kind;
    storage
        .save_notification_preferences(
            &ResetAlertPreferences {
                quiet_enabled: false,
                ..Default::default()
            },
            &ThresholdAlertPreferences::default(),
            &prefs(enabled),
            &prefs(enabled),
        )
        .unwrap();
    storage
        .accept_live_quota(original.clone(), Some(association(&original, None)), TIME)
        .unwrap();
    storage
        .record_profile_connection(&id(1), &id(2), crate::ProfileConnectionState::Connected)
        .unwrap();
    (dir, storage, original)
}
fn failure(original: &QuotaReading, n: u32, kind: OperationalFailureKind) -> OperationalFailure {
    OperationalFailure {
        request_id: id(500 + n),
        profile_id: original.binding.profile_id.clone(),
        expected_home_reference: original.binding.home_reference.clone(),
        expected_binding_id: Some(original.binding.binding_id.clone()),
        provider_version: original.provider_version.clone(),
        source_kind: original.source_kind,
        observed_at_utc: time(n),
        kind,
    }
}
fn fail(storage: &mut Storage, original: &QuotaReading, n: u32, kind: OperationalFailureKind) {
    storage
        .record_operational_failure(&failure(original, n, kind), &time(n))
        .unwrap();
}
fn fresh_reading(original: &QuotaReading, n: u32) -> QuotaReading {
    let mut q = original.clone();
    q.run_id = id(800 + n);
    q.observed_at_utc = time(n);
    q.completed_at_utc = time(n);
    q
}
fn claim(storage: &mut Storage, n: u32) -> crate::OperationalNotification {
    match storage.claim_notification(&time(n)).unwrap().unwrap() {
        PendingNotification::Operational(value) => value,
        PendingNotification::Quota(_) => panic!("unexpected quota event"),
    }
}
fn validate(storage: &mut Storage) {
    let snapshot = storage.backup_snapshot().unwrap();
    crate::validate_backup_snapshot(&snapshot.bytes, snapshot.schema_version).unwrap();
}
#[test]
fn official_signed_out_requires_previous_association_read_and_never_claims_invalid_credentials() {
    let (_dir, mut storage, original) = baseline(ActivitySourceKind::OfficialArtifact, true);
    fail(
        &mut storage,
        &original,
        1,
        OperationalFailureKind::AuthenticationRequired,
    );
    let history = storage.operational_alert_history().unwrap();
    assert_eq!(history.events.len(), 1);
    assert_eq!(
        history.events[0].kind,
        OperationalAlertKind::AuthenticationRequired
    );
    assert_eq!(
        storage.list_profiles().unwrap()[0].connection_state,
        crate::ProfileConnectionState::AuthenticationRequired
    );
    let packet = claim(&mut storage, 2);
    assert_eq!(packet.kind, OperationalAlertKind::AuthenticationRequired);
    storage
        .complete_notification(
            &packet.event_id,
            &packet.attempt_id,
            NotificationOutcome::Submitted,
        )
        .unwrap();
    fail(
        &mut storage,
        &original,
        3,
        OperationalFailureKind::AuthenticationRequired,
    );
    assert_eq!(storage.operational_alert_history().unwrap().events.len(), 1);
    assert!(storage.claim_notification(&time(4)).unwrap().is_none());
    validate(&mut storage);
    let (_dir, mut empty, original) = fixture();
    empty
        .record_profile_connection(&id(1), &id(2), crate::ProfileConnectionState::Connected)
        .unwrap();
    let mut evidence = failure(&original, 1, OperationalFailureKind::AuthenticationRequired);
    evidence.expected_binding_id = None;
    empty
        .record_operational_failure(&evidence, &time(1))
        .unwrap();
    assert_eq!(
        empty.list_profiles().unwrap()[0].connection_state,
        crate::ProfileConnectionState::AuthenticationRequired
    );
    assert!(empty.operational_alert_history().unwrap().events.is_empty());
    let count: i64 = empty
        .connection
        .query_row("SELECT count(*) FROM operational_alert_outcomes", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(count, 0);
    validate(&mut empty);
}
#[test]
fn exact_third_transient_failure_emits_once_and_duplicate_request_is_idempotent() {
    let (dir, mut storage, original) = baseline(ActivitySourceKind::OfficialArtifact, true);
    for n in 1..=2 {
        fail(
            &mut storage,
            &original,
            n,
            OperationalFailureKind::TransientCollectionFailure,
        );
        assert!(
            storage
                .operational_alert_history()
                .unwrap()
                .events
                .is_empty()
        );
    }
    let duplicate = failure(
        &original,
        2,
        OperationalFailureKind::TransientCollectionFailure,
    );
    storage
        .record_operational_failure(&duplicate, &time(3))
        .unwrap();
    assert!(
        storage
            .operational_alert_history()
            .unwrap()
            .events
            .is_empty()
    );
    fail(
        &mut storage,
        &original,
        3,
        OperationalFailureKind::TransientCollectionFailure,
    );
    let packet = claim(&mut storage, 4);
    assert_eq!(packet.kind, OperationalAlertKind::CollectionFailure);
    storage
        .complete_notification(
            &packet.event_id,
            &packet.attempt_id,
            NotificationOutcome::Submitted,
        )
        .unwrap();
    fail(
        &mut storage,
        &original,
        5,
        OperationalFailureKind::TransientCollectionFailure,
    );
    assert_eq!(storage.operational_alert_history().unwrap().events.len(), 1);
    validate(&mut storage);
    drop(storage);
    let mut storage = Storage::open(&dir.path().join("quota.db"), &KEY).unwrap();
    assert!(storage.claim_notification(&time(6)).unwrap().is_none());
    fail(
        &mut storage,
        &original,
        7,
        OperationalFailureKind::TransientCollectionFailure,
    );
    assert_eq!(storage.operational_alert_history().unwrap().events.len(), 1);
    validate(&mut storage);
}
#[test]
fn live_quota_success_closes_both_incidents_and_normal_polling_adds_no_operational_rows() {
    let (_dir, mut storage, original) = baseline(ActivitySourceKind::OfficialArtifact, true);
    fail(
        &mut storage,
        &original,
        1,
        OperationalFailureKind::AuthenticationRequired,
    );
    for n in 2..=4 {
        fail(
            &mut storage,
            &original,
            n,
            OperationalFailureKind::TransientCollectionFailure,
        );
    }
    storage
        .accept_live_quota(fresh_reading(&original, 5), None, &time(5))
        .unwrap();
    assert!(storage.claim_notification(&time(6)).unwrap().is_none());
    assert!(
        storage
            .operational_alert_history()
            .unwrap()
            .events
            .iter()
            .all(|e| e.delivery_state == AlertDeliveryState::Suppressed)
    );
    let before: i64 = storage
        .connection
        .query_row("SELECT count(*) FROM operational_alert_outcomes", [], |r| {
            r.get(0)
        })
        .unwrap();
    for n in 6..=8 {
        storage
            .accept_live_quota(fresh_reading(&original, n), None, &time(n))
            .unwrap();
    }
    let after: i64 = storage
        .connection
        .query_row("SELECT count(*) FROM operational_alert_outcomes", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(before, after);
    fail(
        &mut storage,
        &original,
        9,
        OperationalFailureKind::AuthenticationRequired,
    );
    for n in 10..=12 {
        fail(
            &mut storage,
            &original,
            n,
            OperationalFailureKind::TransientCollectionFailure,
        );
    }
    assert_eq!(storage.operational_alert_history().unwrap().events.len(), 4);
    validate(&mut storage);
}
#[test]
fn historical_success_and_stale_failure_cannot_change_active_incident() {
    let (_dir, mut storage, original) = baseline(ActivitySourceKind::OfficialArtifact, true);
    fail(
        &mut storage,
        &original,
        1,
        OperationalFailureKind::AuthenticationRequired,
    );
    storage
        .accept_quota(fresh_reading(&original, 2), None)
        .unwrap();
    assert_eq!(
        storage.operational_alert_history().unwrap().events[0].delivery_state,
        AlertDeliveryState::Queued
    );
    let before = all(&storage);
    assert_eq!(
        storage.record_operational_failure(
            &failure(
                &original,
                3,
                OperationalFailureKind::TransientCollectionFailure
            ),
            "2026-10-18T12:10:03.000Z"
        ),
        Err(crate::StorageError::InvalidAlertEvidence)
    );
    let mut stale = failure(
        &original,
        1,
        OperationalFailureKind::TransientCollectionFailure,
    );
    stale.request_id = id(999);
    assert_eq!(
        storage.record_operational_failure(&stale, &time(3)),
        Err(crate::StorageError::InvalidAlertEvidence)
    );
    assert_eq!(all(&storage), before);
    validate(&mut storage);
}
#[test]
fn activity_only_and_paired_live_success_close_once_with_real_accepted_run_anchor() {
    for paired in [false, true] {
        let (_dir, mut storage, original) = baseline(ActivitySourceKind::OfficialArtifact, true);
        fail(
            &mut storage,
            &original,
            1,
            OperationalFailureKind::AuthenticationRequired,
        );
        let q = fresh_reading(&original, 2);
        let activity = ActivityReading {
            run_id: id(900),
            binding: q.binding.clone(),
            identity_evidence: q.identity_evidence.clone(),
            provider_version: q.provider_version.clone(),
            source_kind: q.source_kind,
            started_at_utc: q.started_at_utc.clone(),
            observed_at_utc: q.observed_at_utc.clone(),
            completed_at_utc: q.completed_at_utc.clone(),
            summary: ActivitySummaryValues::default(),
            daily_usage: None,
        };
        if paired {
            storage
                .accept_live_collection(Some(q.clone()), Some(activity.clone()), None, &time(2))
                .unwrap();
        } else {
            storage
                .accept_live_activity_with_binding(activity.clone(), None, &time(2))
                .unwrap();
        }
        assert!(storage.claim_notification(&time(3)).unwrap().is_none());
        let anchor:String=storage.connection.query_row("SELECT accepted_run_id FROM operational_alert_outcomes WHERE outcome='accepted_success'",[],|r|r.get(0)).unwrap();
        assert_eq!(anchor, if paired { q.run_id } else { activity.run_id });
        validate(&mut storage);
    }
}
#[test]
fn disabled_type_consumes_its_incident_and_enabling_does_not_replay_it() {
    let (_dir, mut storage, original) = baseline(ActivitySourceKind::OfficialArtifact, false);
    fail(
        &mut storage,
        &original,
        1,
        OperationalFailureKind::AuthenticationRequired,
    );
    for n in 2..=4 {
        fail(
            &mut storage,
            &original,
            n,
            OperationalFailureKind::TransientCollectionFailure,
        );
    }
    storage
        .save_notification_preferences(
            &ResetAlertPreferences {
                quiet_enabled: false,
                ..Default::default()
            },
            &ThresholdAlertPreferences::default(),
            &prefs(true),
            &prefs(true),
        )
        .unwrap();
    fail(
        &mut storage,
        &original,
        5,
        OperationalFailureKind::AuthenticationRequired,
    );
    fail(
        &mut storage,
        &original,
        6,
        OperationalFailureKind::TransientCollectionFailure,
    );
    assert!(storage.claim_notification(&time(7)).unwrap().is_none());
    assert_eq!(storage.operational_alert_history().unwrap().events.len(), 2);
    validate(&mut storage);
}
#[test]
fn shared_quiet_hours_release_only_still_active_incidents_without_requiring_quota_refresh() {
    for resolve in [false, true] {
        let (_dir, mut storage, original) = baseline(ActivitySourceKind::OfficialArtifact, true);
        let quiet = ResetAlertPreferences {
            quiet_enabled: true,
            quiet_hours: crate::AlertQuietHours {
                time_zone: "Etc/UTC".into(),
                start_minute: 720,
                end_minute: 780,
            },
            ..Default::default()
        };
        storage
            .save_notification_preferences(
                &quiet,
                &ThresholdAlertPreferences::default(),
                &prefs(true),
                &prefs(true),
            )
            .unwrap();
        fail(
            &mut storage,
            &original,
            1,
            OperationalFailureKind::AuthenticationRequired,
        );
        assert!(storage.claim_notification(&time(2)).unwrap().is_none());
        if resolve {
            storage
                .accept_live_quota(fresh_reading(&original, 3), None, &time(3))
                .unwrap();
        }
        let notification = storage
            .claim_notification("2026-10-18T13:00:00.000Z")
            .unwrap();
        assert_eq!(notification.is_none(), resolve);
        validate(&mut storage);
    }
}
#[test]
fn current_incident_and_submission_journal_survive_restore_and_undo_without_replay() {
    let (_dir, mut storage, original) = baseline(ActivitySourceKind::OfficialArtifact, true);
    let baseline = storage.backup_snapshot().unwrap();
    fail(
        &mut storage,
        &original,
        1,
        OperationalFailureKind::AuthenticationRequired,
    );
    let queued = storage.backup_snapshot().unwrap();
    let packet = claim(&mut storage, 2);
    storage
        .complete_notification(
            &packet.event_id,
            &packet.attempt_id,
            NotificationOutcome::Submitted,
        )
        .unwrap();
    for snapshot in [&queued, &baseline, &queued] {
        let metadata =
            crate::validate_checkpoint_restore_input(&snapshot.bytes, snapshot.schema_version)
                .unwrap();
        let revision = storage.restore_state().unwrap().revision;
        storage
            .replace_records_from_checkpoint(
                &snapshot.bytes,
                snapshot.schema_version,
                &revision,
                &metadata.homes,
            )
            .unwrap();
        assert!(storage.claim_notification(&time(3)).unwrap().is_none());
        validate(&mut storage);
    }
    fail(
        &mut storage,
        &original,
        4,
        OperationalFailureKind::AuthenticationRequired,
    );
    assert_eq!(storage.operational_alert_history().unwrap().events.len(), 1);
    storage
        .accept_live_quota(fresh_reading(&original, 5), None, &time(5))
        .unwrap();
    fail(
        &mut storage,
        &original,
        6,
        OperationalFailureKind::AuthenticationRequired,
    );
    assert_eq!(
        claim(&mut storage, 7).kind,
        OperationalAlertKind::AuthenticationRequired
    );
    validate(&mut storage);
}
#[test]
fn durable_partial_failure_streak_survives_older_restore_before_the_third_failure() {
    let (_dir, mut storage, original) = baseline(ActivitySourceKind::OfficialArtifact, true);
    let baseline = storage.backup_snapshot().unwrap();
    for n in 1..=2 {
        fail(
            &mut storage,
            &original,
            n,
            OperationalFailureKind::TransientCollectionFailure,
        );
    }
    let metadata =
        crate::validate_checkpoint_restore_input(&baseline.bytes, baseline.schema_version).unwrap();
    let revision = storage.restore_state().unwrap().revision;
    storage
        .replace_records_from_checkpoint(
            &baseline.bytes,
            baseline.schema_version,
            &revision,
            &metadata.homes,
        )
        .unwrap();
    fail(
        &mut storage,
        &original,
        3,
        OperationalFailureKind::TransientCollectionFailure,
    );
    assert_eq!(
        claim(&mut storage, 4).kind,
        OperationalAlertKind::CollectionFailure
    );
    validate(&mut storage);
}
#[test]
fn fixture_source_is_gated_both_before_enqueue_and_when_claiming_imported_queue() {
    let (_dir, mut storage, original) = baseline(ActivitySourceKind::TestFixture, true);
    fail(
        &mut storage,
        &original,
        1,
        OperationalFailureKind::AuthenticationRequired,
    );
    if cfg!(feature = "native-test") {
        assert_eq!(
            claim(&mut storage, 2).kind,
            OperationalAlertKind::AuthenticationRequired
        );
    } else {
        assert!(storage.claim_notification(&time(2)).unwrap().is_none());
        storage.connection.execute_batch("INSERT INTO reset_notification_outbox SELECT id,'queued',NULL,NULL,NULL FROM operational_alert_events;").unwrap();
        assert!(storage.claim_notification(&time(3)).unwrap().is_none());
        assert_eq!(
            storage.operational_alert_history().unwrap().events[0].delivery_state,
            AlertDeliveryState::Suppressed
        );
    }
    validate(&mut storage);
}
#[test]
fn notification_preferences_and_failures_rollback_as_whole_transactions() {
    let (_dir, mut storage, original) = baseline(ActivitySourceKind::OfficialArtifact, true);
    let before = all(&storage);
    let invalid = OperationalAlertPreferences {
        enabled: true,
        profile_overrides: vec![crate::ResetAlertOverride {
            profile_id: id(999),
            enabled: true,
        }],
    };
    assert!(
        storage
            .save_notification_preferences(
                &ResetAlertPreferences {
                    enabled: true,
                    quiet_enabled: false,
                    ..Default::default()
                },
                &ThresholdAlertPreferences {
                    enabled: true,
                    profile_overrides: vec![]
                },
                &prefs(false),
                &invalid
            )
            .is_err()
    );
    assert_eq!(all(&storage), before);
    storage.connection.execute_batch("CREATE TRIGGER operational_fault BEFORE INSERT ON operational_alert_events BEGIN SELECT RAISE(ABORT,'synthetic event write failure');END;").unwrap();
    assert_eq!(
        storage.record_operational_failure(
            &failure(&original, 1, OperationalFailureKind::AuthenticationRequired),
            &time(1)
        ),
        Err(crate::StorageError::WriteFailed)
    );
    assert_eq!(all(&storage), before);
    assert_eq!(
        storage.list_profiles().unwrap()[0].connection_state,
        crate::ProfileConnectionState::Connected
    );
    storage
        .connection
        .execute_batch("DROP TRIGGER operational_fault;")
        .unwrap();
    fail(
        &mut storage,
        &original,
        1,
        OperationalFailureKind::AuthenticationRequired,
    );
    let before = all(&storage);
    storage.connection.execute_batch("CREATE TRIGGER success_fault BEFORE INSERT ON operational_alert_outcomes WHEN NEW.outcome='accepted_success' BEGIN SELECT RAISE(ABORT,'synthetic transition failure');END;").unwrap();
    assert_eq!(
        storage.accept_live_quota(fresh_reading(&original, 2), None, &time(2)),
        Err(ActivityError::WriteFailed)
    );
    assert_eq!(all(&storage), before);
    storage
        .connection
        .execute_batch("DROP TRIGGER success_fault;")
        .unwrap();
    validate(&mut storage);
}
#[test]
fn wrong_profile_target_and_duplicate_request_reclassification_preserve_other_accounts() {
    let (_dir, mut storage, original) = baseline(ActivitySourceKind::OfficialArtifact, true);
    storage
        .create_profile(&id(10), "Independent account", &id(11))
        .unwrap();
    let mut second = original.clone();
    second.run_id = id(12);
    second.binding.profile_id = id(10);
    second.binding.home_reference = id(11);
    second.binding.binding_id = id(13);
    storage
        .accept_live_quota(second.clone(), Some(association(&second, None)), TIME)
        .unwrap();
    fail(
        &mut storage,
        &original,
        1,
        OperationalFailureKind::AuthenticationRequired,
    );
    let before = all(&storage);
    let mut changed = failure(
        &original,
        1,
        OperationalFailureKind::TransientCollectionFailure,
    );
    assert_eq!(
        storage.record_operational_failure(&changed, &time(2)),
        Err(crate::StorageError::InvalidAlertEvidence)
    );
    changed.request_id = id(999);
    changed.expected_binding_id = Some(second.binding.binding_id.clone());
    assert_eq!(
        storage.record_operational_failure(&changed, &time(2)),
        Err(crate::StorageError::InvalidAlertEvidence)
    );
    assert_eq!(all(&storage), before);
    for n in 2..=4 {
        fail(
            &mut storage,
            &second,
            n,
            OperationalFailureKind::TransientCollectionFailure,
        );
    }
    let history = storage.operational_alert_history().unwrap();
    assert_eq!(history.events.len(), 2);
    assert!(
        history
            .events
            .iter()
            .any(|e| e.profile_id == id(10) && e.kind == OperationalAlertKind::CollectionFailure)
    );
    validate(&mut storage);
}
#[test]
fn schema_seven_upgrade_preserves_quota_foreign_keys_history_and_delivery_journal() {
    let (_dir, mut storage, original) = baseline(ActivitySourceKind::OfficialArtifact, true);
    storage
        .save_reset_alert_preferences(&ResetAlertPreferences {
            enabled: true,
            quiet_enabled: false,
            ..Default::default()
        })
        .unwrap();
    let mut before = fresh_reading(&original, 1);
    before.limits[0].primary.as_mut().unwrap().used_percent = 100;
    before.limits[0].primary.as_mut().unwrap().resets_at_unix = Some(0);
    storage.accept_live_quota(before, None, &time(1)).unwrap();
    let mut after = fresh_reading(&original, 2);
    after.limits[0].primary.as_mut().unwrap().used_percent = 95;
    after.limits[0].primary.as_mut().unwrap().resets_at_unix = Some(1);
    storage.accept_live_quota(after, None, &time(2)).unwrap();
    let notification = storage.claim_reset_notification(&time(3)).unwrap().unwrap();
    storage
        .complete_reset_notification(
            &notification.event_id,
            &notification.attempt_id,
            NotificationOutcome::Submitted,
        )
        .unwrap();
    let quota_events = storage.alert_history().unwrap();
    assert!(!quota_events.events.is_empty());
    // Build an exact old schema from trusted migration SQL and known-column copy.
    storage
        .connection
        .pragma_update(None, "user_version", 7)
        .unwrap();
    let mut legacy =
        crate::backup_validation::trusted_memory_copy(&storage.connection, "main", 7).unwrap();
    storage
        .connection
        .pragma_update(None, "user_version", crate::CURRENT_SCHEMA_VERSION)
        .unwrap();
    crate::migrate_data_connection(&mut legacy).unwrap();
    crate::backup_validation::validate_connection(&legacy, crate::CURRENT_SCHEMA_VERSION).unwrap();
    let tables: Vec<String> = legacy
        .prepare(
            "SELECT DISTINCT \"table\" FROM pragma_foreign_key_list('alert_events') ORDER BY 1",
        )
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    assert_eq!(
        tables,
        vec![
            "notification_event_ids",
            "quota_limits",
            "quota_reset_assessments"
        ]
    );
    let event_count: i64 = legacy
        .query_row("SELECT count(*) FROM alert_events", [], |r| r.get(0))
        .unwrap();
    assert_eq!(event_count, quota_events.events.len() as i64);
    let submitted: i64 = legacy
        .query_row(
            "SELECT count(*) FROM reset_notification_receipts WHERE state='submitted'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(submitted, 1);
    let outcomes: i64 = legacy
        .query_row("SELECT count(*) FROM operational_alert_outcomes", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(outcomes, 0);
}
#[test]
fn backup_validation_rejects_forged_operational_events_and_untyped_registry_ids() {
    let (_dir, mut storage, original) = baseline(ActivitySourceKind::OfficialArtifact, true);
    for n in 1..=3 {
        fail(
            &mut storage,
            &original,
            n,
            OperationalFailureKind::TransientCollectionFailure,
        );
    }
    validate(&mut storage);
    for sql in [
        "UPDATE operational_alert_outcomes SET transient_failures=2 WHERE transient_failures=3",
        "UPDATE operational_alert_events SET evidence_at_utc='2026-10-18T12:00:01.000Z'",
        "INSERT INTO notification_event_ids VALUES('00000999-0000-4000-8000-000000000000')",
        "UPDATE operational_alert_events SET event_key='operational_alert:1:invalid'",
    ] {
        storage
            .connection
            .execute_batch("SAVEPOINT corrupt_operational")
            .unwrap();
        storage.connection.execute_batch(sql).unwrap();
        assert!(
            crate::backup_validation::validate_connection(
                &storage.connection,
                crate::CURRENT_SCHEMA_VERSION
            )
            .is_err(),
            "{sql}"
        );
        storage
            .connection
            .execute_batch("ROLLBACK TO corrupt_operational;RELEASE corrupt_operational;")
            .unwrap();
    }
}
