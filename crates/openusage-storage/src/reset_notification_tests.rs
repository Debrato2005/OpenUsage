//! Controlled in-process packets exercise the official-source policy without a
//! provider or OS service. Fixture-kind packets must never enter the OS queue.
use super::*;
use crate::{
    AlertDeliveryState, AlertQuietHours, NotificationOutcome, ResetAlertOverride,
    ResetAlertPreferences,
};
fn preferences(enabled: bool) -> ResetAlertPreferences {
    ResetAlertPreferences {
        enabled,
        quiet_enabled: false,
        quiet_hours: AlertQuietHours {
            time_zone: "Etc/UTC".into(),
            start_minute: 1320,
            end_minute: 480,
        },
        profile_overrides: vec![],
    }
}
fn baseline_official(enabled: bool) -> (tempfile::TempDir, Storage, QuotaReading) {
    let (dir, mut storage, mut original) = fixture();
    original.source_kind = ActivitySourceKind::OfficialArtifact;
    original.limits[0].primary.as_mut().unwrap().used_percent = 100;
    original.limits[0].primary.as_mut().unwrap().resets_at_unix = Some(0);
    storage
        .save_reset_alert_preferences(&preferences(enabled))
        .unwrap();
    storage
        .accept_quota(original.clone(), Some(association(&original, None)))
        .unwrap();
    (dir, storage, original)
}
fn confirmed(storage: &mut Storage, original: &QuotaReading, n: u32) -> QuotaReading {
    let reading = super::alert_tests::next(original, n);
    storage.accept_quota(reading.clone(), None).unwrap();
    reading
}
#[test]
fn opted_in_reset_is_consumed_before_submission_and_never_retries_after_reopen() {
    let (dir, mut storage, original) = baseline_official(true);
    let reading = confirmed(&mut storage, &original, 1);
    assert_eq!(
        storage.alert_history().unwrap().events[0].delivery_state,
        AlertDeliveryState::Queued
    );
    let packet = storage
        .claim_reset_notification("2026-10-18T12:00:02.000Z")
        .unwrap()
        .unwrap();
    assert_eq!(packet.profile_id, id(1));
    assert_eq!(packet.evidence_at_utc, reading.observed_at_utc);
    assert_eq!(
        storage.alert_history().unwrap().events[0].delivery_state,
        AlertDeliveryState::Uncertain
    );
    assert!(
        storage
            .claim_reset_notification("2026-10-18T12:00:03.000Z")
            .unwrap()
            .is_none()
    );
    assert!(
        storage
            .complete_reset_notification(&packet.event_id, &id(999), NotificationOutcome::Submitted)
            .is_err()
    );
    let before = all(&storage);
    drop(storage);
    let mut storage = Storage::open(&dir.path().join("quota.db"), &KEY).unwrap();
    assert_eq!(all(&storage), before);
    assert!(
        storage
            .claim_reset_notification("2026-10-18T12:00:04.000Z")
            .unwrap()
            .is_none()
    );
    storage
        .complete_reset_notification(
            &packet.event_id,
            &packet.attempt_id,
            NotificationOutcome::Submitted,
        )
        .unwrap();
    assert_eq!(
        storage.alert_history().unwrap().events[0].delivery_state,
        AlertDeliveryState::Submitted
    );
    assert!(
        storage
            .claim_reset_notification("2026-10-18T12:00:05.000Z")
            .unwrap()
            .is_none()
    );
    let snapshot = storage.backup_snapshot().unwrap();
    crate::validate_backup_snapshot(&snapshot.bytes, snapshot.schema_version).unwrap();
}
#[test]
fn failed_attempt_is_terminal_and_clock_rollback_cannot_claim_a_future_event() {
    let (_dir, mut storage, original) = baseline_official(true);
    confirmed(&mut storage, &original, 1);
    assert!(
        storage
            .claim_reset_notification("2026-10-18T11:59:59.000Z")
            .unwrap()
            .is_none()
    );
    assert_eq!(
        storage.alert_history().unwrap().events[0].delivery_state,
        AlertDeliveryState::Queued
    );
    let packet = storage
        .claim_reset_notification("2026-10-18T12:00:02.000Z")
        .unwrap()
        .unwrap();
    storage
        .complete_reset_notification(
            &packet.event_id,
            &packet.attempt_id,
            NotificationOutcome::Failed,
        )
        .unwrap();
    assert_eq!(
        storage.alert_history().unwrap().events[0].delivery_state,
        AlertDeliveryState::Failed
    );
    assert!(
        storage
            .claim_reset_notification("2026-10-18T12:00:03.000Z")
            .unwrap()
            .is_none()
    );
}
#[cfg(not(feature = "native-test"))]
#[test]
fn fixture_source_never_enters_os_queue_and_enabling_does_not_replay_disabled_events() {
    let (_dir, mut storage, original) = super::alert_tests::baseline();
    storage
        .save_reset_alert_preferences(&preferences(true))
        .unwrap();
    confirmed(&mut storage, &original, 1);
    assert!(
        storage
            .claim_reset_notification("2026-10-18T12:00:02.000Z")
            .unwrap()
            .is_none()
    );
    assert!(
        storage
            .alert_history()
            .unwrap()
            .events
            .iter()
            .all(|e| e.delivery_state == AlertDeliveryState::Recorded)
    );
    let (_dir, mut storage, original) = baseline_official(false);
    confirmed(&mut storage, &original, 1);
    storage
        .save_reset_alert_preferences(&preferences(true))
        .unwrap();
    assert!(
        storage
            .claim_reset_notification("2026-10-18T12:00:02.000Z")
            .unwrap()
            .is_none()
    );
    assert_eq!(
        storage.alert_history().unwrap().events[0].delivery_state,
        AlertDeliveryState::Suppressed
    );
}
#[test]
fn per_profile_opt_in_is_atomic_and_invalid_settings_preserve_queue_and_preferences() {
    let (_dir, mut storage, original) = baseline_official(false);
    let mut prefs = preferences(false);
    prefs.profile_overrides.push(ResetAlertOverride {
        profile_id: id(1),
        enabled: true,
    });
    storage.save_reset_alert_preferences(&prefs).unwrap();
    confirmed(&mut storage, &original, 1);
    assert_eq!(
        storage.alert_history().unwrap().events[0].delivery_state,
        AlertDeliveryState::Queued
    );
    let before = all(&storage);
    for invalid in [
        ResetAlertPreferences {
            quiet_hours: AlertQuietHours {
                time_zone: "Invalid/Timezone".into(),
                ..prefs.quiet_hours.clone()
            },
            ..prefs.clone()
        },
        ResetAlertPreferences {
            quiet_hours: AlertQuietHours {
                end_minute: 1320,
                ..prefs.quiet_hours.clone()
            },
            ..prefs.clone()
        },
        ResetAlertPreferences {
            profile_overrides: vec![
                prefs.profile_overrides[0].clone(),
                prefs.profile_overrides[0].clone(),
            ],
            ..prefs.clone()
        },
        ResetAlertPreferences {
            profile_overrides: vec![ResetAlertOverride {
                profile_id: id(99),
                enabled: true,
            }],
            ..prefs.clone()
        },
    ] {
        assert_eq!(
            storage.save_reset_alert_preferences(&invalid),
            Err(crate::StorageError::InvalidAlertPreferences)
        );
        assert_eq!(all(&storage), before);
    }
    assert_eq!(storage.load_reset_alert_preferences().unwrap(), prefs);
    storage
        .save_reset_alert_preferences(&preferences(false))
        .unwrap();
    assert_eq!(
        storage.alert_history().unwrap().events[0].delivery_state,
        AlertDeliveryState::Suppressed
    );
    storage.save_reset_alert_preferences(&prefs).unwrap();
    assert!(
        storage
            .claim_reset_notification("2026-10-18T12:00:02.000Z")
            .unwrap()
            .is_none()
    );
}
#[test]
fn quiet_events_wait_until_eight_and_require_a_fresh_same_generation_reading() {
    let (_dir, mut storage, original) = baseline_official(true);
    let mut prefs = preferences(true);
    prefs.quiet_enabled = true;
    storage.save_reset_alert_preferences(&prefs).unwrap();
    let mut reading = super::alert_tests::next(&original, 1);
    reading.observed_at_utc = "2026-10-18T22:00:01.000Z".into();
    reading.completed_at_utc = reading.observed_at_utc.clone();
    storage.accept_quota(reading.clone(), None).unwrap();
    assert!(
        storage
            .claim_reset_notification("2026-10-18T22:00:02.000Z")
            .unwrap()
            .is_none()
    );
    assert!(
        storage
            .claim_reset_notification("2026-10-19T08:00:00.000Z")
            .unwrap()
            .is_none()
    );
    let mut fresh = super::alert_tests::next(&original, 2);
    fresh.observed_at_utc = "2026-10-19T07:59:59.000Z".into();
    fresh.completed_at_utc = fresh.observed_at_utc.clone();
    fresh.limits[0].primary.as_mut().unwrap().resets_at_unix = Some(1);
    storage.accept_quota(fresh, None).unwrap();
    assert!(
        storage
            .claim_reset_notification("2026-10-19T07:59:59.000Z")
            .unwrap()
            .is_none()
    );
    let packet = storage
        .claim_reset_notification("2026-10-19T08:00:00.000Z")
        .unwrap()
        .unwrap();
    assert_eq!(packet.evidence_at_utc, reading.observed_at_utc);
}
#[test]
fn expired_or_archived_or_replaced_generations_are_suppressed() {
    let (_dir, mut storage, original) = baseline_official(true);
    confirmed(&mut storage, &original, 1);
    assert!(
        storage
            .claim_reset_notification("2026-10-19T12:00:02.000Z")
            .unwrap()
            .is_none()
    );
    assert_eq!(
        storage.alert_history().unwrap().events[0].delivery_state,
        AlertDeliveryState::Suppressed
    );
    let (_dir, mut storage, original) = baseline_official(true);
    confirmed(&mut storage, &original, 1);
    storage.set_profile_archived(&id(1), true).unwrap();
    assert!(
        storage
            .claim_reset_notification("2026-10-18T12:00:02.000Z")
            .unwrap()
            .is_none()
    );
    assert_eq!(
        storage.alert_history().unwrap().events[0].delivery_state,
        AlertDeliveryState::Suppressed
    );
    let (_dir, mut storage, original) = baseline_official(true);
    let first = confirmed(&mut storage, &original, 1);
    let second = confirmed(&mut storage, &original, 2);
    let packet = storage
        .claim_reset_notification("2026-10-18T12:00:03.000Z")
        .unwrap()
        .unwrap();
    assert_eq!(packet.evidence_at_utc, second.observed_at_utc);
    assert_eq!(
        storage
            .alert_history()
            .unwrap()
            .events
            .iter()
            .find(|e| e.generation_run_id == first.run_id)
            .unwrap()
            .delivery_state,
        AlertDeliveryState::Suppressed
    );
}
#[test]
fn restore_and_undo_retain_local_attempt_receipts_and_suppress_imported_pending_work() {
    let (dir, mut storage, original) = baseline_official(true);
    confirmed(&mut storage, &original, 1);
    let queued = storage.backup_snapshot().unwrap();
    let packet = storage
        .claim_reset_notification("2026-10-18T12:00:02.000Z")
        .unwrap()
        .unwrap();
    storage
        .complete_reset_notification(
            &packet.event_id,
            &packet.attempt_id,
            NotificationOutcome::Submitted,
        )
        .unwrap();
    let metadata =
        crate::validate_checkpoint_restore_input(&queued.bytes, queued.schema_version).unwrap();
    let revision = storage.restore_state().unwrap().revision;
    storage
        .replace_records_from_checkpoint(
            &queued.bytes,
            queued.schema_version,
            &revision,
            &metadata.homes,
        )
        .unwrap();
    assert_eq!(
        storage.alert_history().unwrap().events[0].delivery_state,
        AlertDeliveryState::Submitted
    );
    assert!(
        storage
            .claim_reset_notification("2026-10-18T12:00:03.000Z")
            .unwrap()
            .is_none()
    );
    let mut imported = Storage::create(&dir.path().join("imported-notifications"), &KEY).unwrap();
    let revision = imported.restore_state().unwrap().revision;
    imported
        .replace_records_from_backup(
            &queued.bytes,
            queued.schema_version,
            &revision,
            &[crate::RestoreHomeMapping {
                profile_id: id(1),
                managed_home_reference: id(50),
            }],
        )
        .unwrap();
    assert_eq!(
        imported.alert_history().unwrap().events[0].delivery_state,
        AlertDeliveryState::Suppressed
    );
    assert!(
        imported
            .claim_reset_notification("2026-10-18T12:00:03.000Z")
            .unwrap()
            .is_none()
    );
    let checkpoint = dir.path().join("notification-checkpoint");
    storage.create_encrypted_checkpoint(&checkpoint).unwrap();
    let recovered = storage.read_encrypted_checkpoint(&checkpoint).unwrap();
    crate::validate_backup_snapshot(&recovered.bytes, recovered.schema_version).unwrap();
}

#[cfg(feature = "native-test")]
#[test]
fn explicit_native_test_build_can_queue_fixture_without_changing_its_provenance() {
    let (_dir, mut storage, original) = super::alert_tests::baseline();
    storage
        .save_reset_alert_preferences(&preferences(true))
        .unwrap();
    confirmed(&mut storage, &original, 1);
    let packet = storage
        .claim_reset_notification("2026-10-18T12:00:02.000Z")
        .unwrap()
        .unwrap();
    let event = storage
        .alert_history()
        .unwrap()
        .events
        .into_iter()
        .find(|event| event.id == packet.event_id)
        .unwrap();
    assert_eq!(event.source_kind, ActivitySourceKind::TestFixture);
    assert_eq!(event.delivery_state, AlertDeliveryState::Uncertain);
    let snapshot = storage.backup_snapshot().unwrap();
    crate::validate_backup_snapshot(&snapshot.bytes, snapshot.schema_version).unwrap();
}

#[cfg(not(feature = "native-test"))]
#[test]
fn production_suppresses_an_existing_fixture_queue_without_a_delivery_attempt() {
    let (_dir, mut storage, original) = baseline_official(true);
    confirmed(&mut storage, &original, 1);
    // Model an authenticated workspace produced by the explicit native-test build.
    storage.connection.execute_batch("UPDATE collection_runs SET source_kind='test_fixture'; UPDATE alert_events SET source_kind='test_fixture';").unwrap();
    let snapshot = storage.backup_snapshot().unwrap();
    crate::validate_backup_snapshot(&snapshot.bytes, snapshot.schema_version).unwrap();
    assert!(
        storage
            .claim_reset_notification("2026-10-18T12:00:02.000Z")
            .unwrap()
            .is_none()
    );
    assert_eq!(
        storage.alert_history().unwrap().events[0].delivery_state,
        AlertDeliveryState::Suppressed
    );
    let attempts: i64 = storage
        .connection
        .query_row(
            "SELECT count(*) FROM reset_notification_receipts",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(attempts, 0);
}
