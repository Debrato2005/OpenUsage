//! Synthetic accepted packets test storage behavior, not provider or OS delivery.
use super::*;
use crate::{
    AlertDeliveryState, NotificationOutcome, ResetAlertOverride, ResetAlertPreferences,
    ThresholdAlertPreferences,
};
fn time(second: u32) -> String {
    format!("2026-10-18T12:00:{second:02}.000Z")
}
fn prefs(enabled: bool) -> ThresholdAlertPreferences {
    ThresholdAlertPreferences {
        enabled,
        profile_overrides: vec![],
    }
}
fn baseline(
    source: ActivitySourceKind,
    enabled: bool,
) -> (tempfile::TempDir, Storage, QuotaReading) {
    let (dir, mut storage, mut original) = fixture();
    original.source_kind = source;
    original.limits[0].primary.as_mut().unwrap().used_percent = 100;
    original.limits[0].primary.as_mut().unwrap().resets_at_unix = Some(0);
    storage
        .save_alert_preferences(
            &ResetAlertPreferences {
                quiet_enabled: false,
                ..Default::default()
            },
            &prefs(enabled),
        )
        .unwrap();
    storage
        .accept_live_quota(original.clone(), Some(association(&original, None)), TIME)
        .unwrap();
    (dir, storage, original)
}
fn packet(original: &QuotaReading, n: u32, used: u8) -> QuotaReading {
    let mut reading = original.clone();
    reading.run_id = id(100 + n);
    reading.observed_at_utc = time(n);
    reading.completed_at_utc = time(n);
    reading.limits[0].primary.as_mut().unwrap().used_percent = used;
    reading.limits[0].primary.as_mut().unwrap().resets_at_unix = Some(1);
    reading
}
fn live(storage: &mut Storage, original: &QuotaReading, n: u32, used: u8) {
    storage
        .accept_live_quota(packet(original, n, used), None, &time(n))
        .unwrap();
}
fn threshold_events(storage: &Storage) -> Vec<crate::AlertEvent> {
    let mut events: Vec<_> = storage
        .alert_history()
        .unwrap()
        .events
        .into_iter()
        .filter(|e| e.threshold_percent.is_some())
        .collect();
    events.sort_by_key(|e| e.threshold_percent);
    events
}
fn validate(storage: &mut Storage) {
    let snapshot = storage.backup_snapshot().unwrap();
    crate::validate_backup_snapshot(&snapshot.bytes, snapshot.schema_version).unwrap();
}
#[test]
fn first_confirmed_window_coalesces_lower_level_without_claiming_os_delivery() {
    let (dir, mut storage, original) = baseline(ActivitySourceKind::OfficialArtifact, true);
    assert!(threshold_events(&storage).is_empty());
    live(&mut storage, &original, 1, 95);
    let events = threshold_events(&storage);
    assert_eq!(
        events
            .iter()
            .map(|e| e.threshold_percent.unwrap())
            .collect::<Vec<_>>(),
        vec![80, 90]
    );
    assert_eq!(events[0].delivery_state, AlertDeliveryState::Covered);
    assert_eq!(events[1].delivery_state, AlertDeliveryState::Queued);
    assert_eq!(events[1].evidence_run_id, id(101));
    let notification = storage.claim_usage_notification(&time(2)).unwrap().unwrap();
    assert_eq!(notification.threshold_percent, Some(90));
    storage
        .complete_reset_notification(
            &notification.event_id,
            &notification.attempt_id,
            NotificationOutcome::Submitted,
        )
        .unwrap();
    live(&mut storage, &original, 3, 70);
    live(&mut storage, &original, 4, 95);
    assert!(
        storage
            .claim_usage_notification(&time(5))
            .unwrap()
            .is_none()
    );
    assert_eq!(threshold_events(&storage).len(), 2);
    validate(&mut storage);
    drop(storage);
    let mut storage = Storage::open(&dir.path().join("quota.db"), &KEY).unwrap();
    assert_eq!(
        threshold_events(&storage)[0].delivery_state,
        AlertDeliveryState::Covered
    );
    assert!(
        storage
            .claim_usage_notification(&time(6))
            .unwrap()
            .is_none()
    );
    live(&mut storage, &original, 7, 100);
    assert_eq!(
        storage
            .claim_usage_notification(&time(8))
            .unwrap()
            .unwrap()
            .threshold_percent,
        Some(100)
    );
    validate(&mut storage);
}
#[test]
fn later_multilevel_crossings_remain_independent_and_deduplicate_after_decline() {
    let (_dir, mut storage, original) = baseline(ActivitySourceKind::OfficialArtifact, true);
    live(&mut storage, &original, 1, 79);
    live(&mut storage, &original, 2, 100);
    let events = threshold_events(&storage);
    assert_eq!(events.len(), 3);
    assert!(
        events
            .iter()
            .all(|e| e.delivery_state == AlertDeliveryState::Queued
                && e.generation_run_id == id(101)
                && e.evidence_run_id == id(102))
    );
    let mut levels = vec![];
    for _ in 0..3 {
        let event = storage.claim_usage_notification(&time(3)).unwrap().unwrap();
        levels.push(event.threshold_percent.unwrap());
        storage
            .complete_reset_notification(
                &event.event_id,
                &event.attempt_id,
                NotificationOutcome::Submitted,
            )
            .unwrap();
    }
    levels.sort();
    assert_eq!(levels, vec![80, 90, 100]);
    live(&mut storage, &original, 4, 0);
    live(&mut storage, &original, 5, 100);
    assert!(
        storage
            .claim_usage_notification(&time(6))
            .unwrap()
            .is_none()
    );
    assert_eq!(threshold_events(&storage).len(), 3);
    validate(&mut storage);
}
#[test]
fn only_exact_confirmed_generation_qualifies_and_timestamp_changes_do_not_create_one() {
    let (_dir, mut storage, original) = baseline(ActivitySourceKind::OfficialArtifact, true);
    // Initial reading has no confirmed generation even if above every threshold.
    assert!(threshold_events(&storage).is_empty());
    let mut changed = packet(&original, 1, 100);
    storage
        .accept_live_quota(changed.clone(), None, &time(1))
        .unwrap();
    assert!(threshold_events(&storage).is_empty());
    changed.run_id = id(102);
    changed.observed_at_utc = time(2);
    changed.completed_at_utc = time(2);
    changed.limits[0].primary.as_mut().unwrap().used_percent = 95;
    changed.limits[0].primary.as_mut().unwrap().resets_at_unix = Some(2);
    storage.accept_live_quota(changed, None, &time(2)).unwrap();
    assert_eq!(threshold_events(&storage).len(), 2);
    let mut changed = packet(&original, 3, 100);
    changed.limits[0].primary.as_mut().unwrap().resets_at_unix = Some(3);
    storage.accept_live_quota(changed, None, &time(3)).unwrap();
    assert_eq!(
        storage
            .current_quota(&id(1))
            .unwrap()
            .unwrap()
            .reset_assessments[0]
            .state,
        crate::QuotaResetState::NotConfirmed
    );
    assert_eq!(threshold_events(&storage).len(), 2);
    validate(&mut storage);
}
#[test]
fn historical_stale_and_disabled_crossings_never_replay_after_a_dip() {
    for mode in 0..3 {
        let (_dir, mut storage, original) =
            baseline(ActivitySourceKind::OfficialArtifact, mode != 2);
        live(&mut storage, &original, 1, 79);
        let crossed = packet(&original, 2, 95);
        match mode {
            0 => {
                storage.accept_quota(crossed, None).unwrap();
            }
            1 => {
                storage
                    .accept_live_quota(crossed, None, "2026-10-18T12:10:02.000Z")
                    .unwrap();
            }
            _ => {
                storage.accept_live_quota(crossed, None, &time(2)).unwrap();
                storage
                    .save_threshold_alert_preferences(&prefs(true))
                    .unwrap();
            }
        }
        live(&mut storage, &original, 3, 70);
        live(&mut storage, &original, 4, 95);
        assert!(
            storage
                .claim_usage_notification(&time(5))
                .unwrap()
                .is_none(),
            "mode {mode}"
        );
        assert!(
            threshold_events(&storage)
                .iter()
                .all(|e| e.delivery_state != AlertDeliveryState::Queued)
        );
        validate(&mut storage);
    }
}
#[test]
fn freshness_boundary_is_strict_and_future_completion_does_not_create_a_threshold() {
    for (now, eligible) in [
        ("2026-10-18T12:10:00.999Z", true),
        ("2026-10-18T12:10:01.000Z", false),
        ("2026-10-18T12:00:00.999Z", false),
    ] {
        let (_dir, mut storage, original) = baseline(ActivitySourceKind::OfficialArtifact, true);
        storage
            .accept_live_quota(packet(&original, 1, 80), None, now)
            .unwrap();
        assert_eq!(!threshold_events(&storage).is_empty(), eligible, "{now}");
        validate(&mut storage);
    }
    let (_dir, mut storage, original) = baseline(ActivitySourceKind::OfficialArtifact, true);
    let before = all(&storage);
    assert_eq!(
        storage.accept_live_quota(packet(&original, 1, 80), None, "invalid"),
        Err(ActivityError::InvalidInput)
    );
    assert_eq!(all(&storage), before);
}
#[test]
fn shared_preferences_are_atomic_independent_and_do_not_replay_history() {
    let (_dir, mut storage, original) = baseline(ActivitySourceKind::OfficialArtifact, false);
    let reset_before = storage.load_reset_alert_preferences().unwrap();
    let usage_before = storage.load_threshold_alert_preferences().unwrap();
    let reset = ResetAlertPreferences {
        enabled: true,
        ..reset_before.clone()
    };
    assert!(
        storage
            .save_alert_preferences(
                &reset,
                &ThresholdAlertPreferences {
                    enabled: true,
                    profile_overrides: vec![ResetAlertOverride {
                        profile_id: id(999),
                        enabled: true
                    }]
                }
            )
            .is_err()
    );
    assert_eq!(
        storage.load_reset_alert_preferences().unwrap(),
        reset_before
    );
    assert_eq!(
        storage.load_threshold_alert_preferences().unwrap(),
        usage_before
    );
    live(&mut storage, &original, 1, 79);
    live(&mut storage, &original, 2, 80);
    storage
        .save_alert_preferences(&reset, &prefs(true))
        .unwrap();
    assert!(
        storage
            .claim_usage_notification(&time(3))
            .unwrap()
            .is_none()
    );
    live(&mut storage, &original, 4, 90);
    assert_eq!(
        storage
            .claim_usage_notification(&time(5))
            .unwrap()
            .unwrap()
            .threshold_percent,
        Some(90)
    );
    validate(&mut storage);
}
#[test]
fn threshold_evidence_failure_rolls_back_the_entire_live_collection() {
    let (_dir, mut storage, original) = baseline(ActivitySourceKind::OfficialArtifact, true);
    let before = all(&storage);
    storage.connection.execute_batch("CREATE TRIGGER threshold_fault BEFORE INSERT ON threshold_alert_events BEGIN SELECT RAISE(ABORT,'synthetic threshold failure'); END;").unwrap();
    assert_eq!(
        storage.accept_live_collection(Some(packet(&original, 1, 95)), None, None, &time(1)),
        Err(ActivityError::WriteFailed)
    );
    assert_eq!(all(&storage), before);
    storage
        .connection
        .execute_batch("DROP TRIGGER threshold_fault;")
        .unwrap();
    live(&mut storage, &original, 1, 95);
    assert_eq!(threshold_events(&storage).len(), 2);
    validate(&mut storage);
}
#[test]
fn fixture_kind_is_gated_at_enqueue_and_again_when_claiming_an_imported_queue() {
    let (_dir, mut storage, original) = baseline(ActivitySourceKind::TestFixture, true);
    live(&mut storage, &original, 1, 95);
    let notification = storage.claim_usage_notification(&time(2)).unwrap();
    if cfg!(feature = "native-test") {
        assert_eq!(notification.unwrap().threshold_percent, Some(90));
    } else {
        assert!(notification.is_none());
        storage.connection.execute_batch("INSERT INTO reset_notification_outbox SELECT e.id,'queued',NULL,NULL,NULL FROM alert_events e JOIN threshold_alert_events t ON t.event_id=e.id WHERE t.threshold_percent=90;").unwrap();
        assert!(
            storage
                .claim_usage_notification(&time(3))
                .unwrap()
                .is_none()
        );
        assert_eq!(
            threshold_events(&storage)[1].delivery_state,
            AlertDeliveryState::Suppressed
        );
    }
    validate(&mut storage);
}
#[test]
fn restore_and_undo_preserve_covered_keys_and_submitted_highest_level() {
    let (dir, mut storage, original) = baseline(ActivitySourceKind::OfficialArtifact, true);
    live(&mut storage, &original, 1, 95);
    let queued = storage.backup_snapshot().unwrap();
    let notification = storage.claim_usage_notification(&time(2)).unwrap().unwrap();
    storage
        .complete_reset_notification(
            &notification.event_id,
            &notification.attempt_id,
            NotificationOutcome::Submitted,
        )
        .unwrap();
    let submitted = storage.backup_snapshot().unwrap();
    for snapshot in [&queued, &submitted, &queued] {
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
        let events = threshold_events(&storage);
        assert_eq!(events[0].delivery_state, AlertDeliveryState::Covered);
        assert_eq!(events[1].delivery_state, AlertDeliveryState::Submitted);
        assert!(
            storage
                .claim_usage_notification(&time(3))
                .unwrap()
                .is_none()
        );
        validate(&mut storage);
    }
    let path = dir.path().join("threshold-checkpoint");
    storage.create_encrypted_checkpoint(&path).unwrap();
    let checkpoint = storage.read_encrypted_checkpoint(&path).unwrap();
    crate::validate_backup_snapshot(&checkpoint.bytes, checkpoint.schema_version).unwrap();
}
#[test]
fn corrupt_threshold_evidence_is_refused_by_backup_validation() {
    let (_dir, mut storage, original) = baseline(ActivitySourceKind::OfficialArtifact, true);
    live(&mut storage, &original, 1, 79);
    live(&mut storage, &original, 2, 90);
    validate(&mut storage);
    for sql in [
        "UPDATE threshold_alert_events SET evidence_run_id=(SELECT generation_run_id FROM alert_events e WHERE e.id=threshold_alert_events.event_id)",
        "UPDATE threshold_alert_events SET accepted_at_utc='2026-10-18T12:10:02.000Z'",
        "UPDATE alert_events SET evidence_at_utc='2026-10-18T12:00:01.000Z' WHERE id IN(SELECT event_id FROM threshold_alert_events)",
    ] {
        storage
            .connection
            .execute_batch("SAVEPOINT corrupt_threshold")
            .unwrap();
        storage.connection.execute_batch(sql).unwrap();
        assert!(
            crate::backup_validation::validate_connection(
                &storage.connection,
                crate::CURRENT_SCHEMA_VERSION
            )
            .is_err()
        );
        storage
            .connection
            .execute_batch("ROLLBACK TO corrupt_threshold; RELEASE corrupt_threshold")
            .unwrap();
    }
}

#[test]
fn restoring_before_generation_cannot_reannounce_a_consumed_provider_window() {
    let (_dir, mut storage, original) = baseline(ActivitySourceKind::OfficialArtifact, true);
    let before_generation = storage.backup_snapshot().unwrap();
    live(&mut storage, &original, 1, 95);
    let notification = storage.claim_usage_notification(&time(2)).unwrap().unwrap();
    storage
        .complete_reset_notification(
            &notification.event_id,
            &notification.attempt_id,
            NotificationOutcome::Submitted,
        )
        .unwrap();
    let old_generation = storage
        .current_quota(&id(1))
        .unwrap()
        .unwrap()
        .reset_assessments[0]
        .generation_run_id
        .clone();
    let metadata = crate::validate_checkpoint_restore_input(
        &before_generation.bytes,
        before_generation.schema_version,
    )
    .unwrap();
    let revision = storage.restore_state().unwrap().revision;
    storage
        .replace_records_from_checkpoint(
            &before_generation.bytes,
            before_generation.schema_version,
            &revision,
            &metadata.homes,
        )
        .unwrap();
    live(&mut storage, &original, 3, 95);
    assert_ne!(
        storage
            .current_quota(&id(1))
            .unwrap()
            .unwrap()
            .reset_assessments[0]
            .generation_run_id,
        old_generation
    );
    assert!(threshold_events(&storage).is_empty());
    assert!(
        storage
            .claim_usage_notification(&time(4))
            .unwrap()
            .is_none()
    );
    let journal: Vec<(String, Option<String>)> = storage
        .connection
        .prepare("SELECT state,window_key FROM reset_notification_receipts ORDER BY state")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    assert_eq!(journal.len(), 2);
    assert!(journal.iter().all(|(_, key)| key.is_some()));
    assert_eq!(journal[0].0, "covered");
    assert_eq!(journal[1].0, "submitted");
    validate(&mut storage);
}
#[test]
fn reset_receipt_anchor_also_survives_restoring_before_its_generation() {
    let (_dir, mut storage, original) = baseline(ActivitySourceKind::OfficialArtifact, false);
    let reset = ResetAlertPreferences {
        enabled: true,
        quiet_enabled: false,
        ..Default::default()
    };
    storage.save_reset_alert_preferences(&reset).unwrap();
    let before_generation = storage.backup_snapshot().unwrap();
    storage
        .accept_quota(packet(&original, 1, 95), None)
        .unwrap();
    let notification = storage.claim_reset_notification(&time(2)).unwrap().unwrap();
    storage
        .complete_reset_notification(
            &notification.event_id,
            &notification.attempt_id,
            NotificationOutcome::Submitted,
        )
        .unwrap();
    let metadata = crate::validate_checkpoint_restore_input(
        &before_generation.bytes,
        before_generation.schema_version,
    )
    .unwrap();
    let revision = storage.restore_state().unwrap().revision;
    storage
        .replace_records_from_checkpoint(
            &before_generation.bytes,
            before_generation.schema_version,
            &revision,
            &metadata.homes,
        )
        .unwrap();
    storage
        .accept_quota(packet(&original, 3, 95), None)
        .unwrap();
    assert!(
        storage
            .claim_reset_notification(&time(4))
            .unwrap()
            .is_none()
    );
    validate(&mut storage);
}
#[test]
fn schema_six_receipts_migrate_with_proof_backfill_but_orphans_keep_unknown_anchor() {
    let (dir, mut storage, original) = baseline(ActivitySourceKind::OfficialArtifact, false);
    storage
        .save_reset_alert_preferences(&ResetAlertPreferences {
            enabled: true,
            quiet_enabled: false,
            ..Default::default()
        })
        .unwrap();
    storage
        .accept_quota(packet(&original, 1, 95), None)
        .unwrap();
    let notification = storage.claim_reset_notification(&time(2)).unwrap().unwrap();
    storage
        .complete_reset_notification(
            &notification.event_id,
            &notification.attempt_id,
            NotificationOutcome::Submitted,
        )
        .unwrap();
    storage.connection.execute_batch("DROP TABLE threshold_alert_events; DROP TABLE threshold_alert_preferences; DROP TABLE threshold_alert_overrides; ALTER TABLE reset_notification_receipts RENAME TO current_receipts;").unwrap();
    // Use the exact old application-owned receipt definition, not a look-alike.
    let old_schema = include_str!("schema_v6.sql");
    let definition = old_schema
        .split("CREATE TABLE reset_notification_receipts")
        .nth(1)
        .unwrap();
    storage
        .connection
        .execute_batch(&format!(
            "CREATE TABLE reset_notification_receipts{definition}"
        ))
        .unwrap();
    storage.connection.execute_batch("INSERT INTO reset_notification_receipts SELECT event_key,attempt_id,state,attempted_at_utc,completed_at_utc FROM current_receipts; DROP TABLE current_receipts; PRAGMA user_version=6;").unwrap();
    let key: String = storage
        .connection
        .query_row(
            "SELECT event_key FROM reset_notification_receipts",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let orphan_key = key.replace(&id(101), &id(999));
    storage.connection.execute("INSERT INTO reset_notification_receipts SELECT ?1,?2,state,attempted_at_utc,completed_at_utc FROM reset_notification_receipts LIMIT 1",rusqlite::params![orphan_key,id(998)]).unwrap();
    crate::backup_validation::validate_connection(&storage.connection, 6).unwrap();
    drop(storage);
    let mut storage = Storage::open(&dir.path().join("quota.db"), &KEY).unwrap();
    let rows: Vec<(String, Option<String>)> = storage
        .connection
        .prepare("SELECT event_key,window_key FROM reset_notification_receipts ORDER BY event_key")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    assert_eq!(rows.len(), 2);
    assert!(
        rows.iter()
            .find(|(candidate, _)| candidate == &key)
            .unwrap()
            .1
            .is_some()
    );
    assert!(
        rows.iter()
            .find(|(candidate, _)| candidate == &orphan_key)
            .unwrap()
            .1
            .is_none()
    );
    assert_eq!(storage.metadata().unwrap().schema_version, 7);
    validate(&mut storage);
}
