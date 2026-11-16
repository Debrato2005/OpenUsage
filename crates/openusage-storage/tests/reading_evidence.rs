//! Private synthetic storage fixtures. They do not establish provider or identity qualification.

use openusage_storage::{
    ActivityError, ActivityReading, ActivitySourceKind, ActivitySummaryValues, EvidenceLocation,
    EvidenceMethod, EvidenceUnusedField, ProfileBinding, ProfileBindingInput, ProfileHistoryChoice,
    ProfileIdentityEvidence, ProfileRemovalRequest, QuotaLimit, QuotaReading, QuotaWindow,
    ReadingEvidenceAvailability, ReadingEvidenceContext, ReadingEvidenceInput,
    ResponseEvidenceInput, RestoreHomeMapping, Storage, validate_backup_snapshot,
    validate_checkpoint_restore_input, validate_restore_input,
};

const KEY: [u8; 32] = [83; 32];
const PROFILE: &str = "11111111-1111-4111-8111-111111111111";
const BINDING: &str = "22222222-2222-4222-8222-222222222222";
const RUN: &str = "33333333-3333-4333-8333-333333333333";
const TIME: &str = "2026-11-16T00:00:00.000Z";
const HOME: &str = "44444444-4444-4444-8444-444444444444";
const EMAIL: &str = "synthetic-private-email@example.invalid";

fn controlled_connection(path: &std::path::Path) -> rusqlite::Connection {
    let connection = rusqlite::Connection::open(path).unwrap();
    let encoded = format!(
        "x'{}'",
        KEY.iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    );
    connection
        .pragma_update(None, "key", encoded.as_str())
        .unwrap();
    connection
        .pragma_update(None, "cipher_memory_security", true)
        .unwrap();
    connection
        .pragma_update(None, "foreign_keys", true)
        .unwrap();
    connection
}

fn private_directory() -> tempfile::TempDir {
    let mut builder = tempfile::Builder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        builder.permissions(std::fs::Permissions::from_mode(0o700));
    }
    builder.tempdir().unwrap()
}

fn activity() -> ActivityReading {
    ActivityReading {
        run_id: RUN.into(),
        binding: ProfileBinding {
            profile_id: PROFILE.into(),
            home_reference: HOME.into(),
            binding_id: BINDING.into(),
        },
        identity_evidence: ProfileIdentityEvidence {
            email: EMAIL.into(),
            plan: Some("synthetic-private-plan".into()),
        },
        provider_version: "0.163.0".into(),
        source_kind: ActivitySourceKind::TestFixture,
        started_at_utc: TIME.into(),
        observed_at_utc: TIME.into(),
        completed_at_utc: TIME.into(),
        summary: ActivitySummaryValues {
            lifetime_tokens: Some("9007199254740993".into()),
            ..ActivitySummaryValues::default()
        },
        daily_usage: None,
        evidence: None,
    }
}

fn association(reading: &ActivityReading) -> ProfileBindingInput {
    ProfileBindingInput {
        profile_id: PROFILE.into(),
        expected_home_reference: HOME.into(),
        expected_previous_binding_id: None,
        new_binding_id: BINDING.into(),
        evidence: reading.identity_evidence.clone(),
        observed_at_utc: TIME.into(),
    }
}

fn evidence(reading: &ActivityReading) -> ReadingEvidenceInput {
    // These digests are synthetic storage input. Parser digest behavior has separate public tests.
    ReadingEvidenceInput {
        context: ReadingEvidenceContext {
            method: EvidenceMethod::Activity,
            run_id: reading.run_id.clone(),
            profile_id: reading.binding.profile_id.clone(),
            binding_id: reading.binding.binding_id.clone(),
            provider_version: reading.provider_version.clone(),
            source_kind: reading.source_kind,
            observed_at_utc: reading.observed_at_utc.clone(),
        },
        adapter_revision: 1,
        response: ResponseEvidenceInput {
            projection_version: 1,
            redacted_response_hash: "a".repeat(64),
            format_fingerprint: "b".repeat(64),
            unused_fields: vec![],
        },
    }
}

fn quota() -> QuotaReading {
    let activity = activity();
    let mut input = evidence(&activity);
    input.context.method = EvidenceMethod::Quota;
    input.context.run_id = "66666666-6666-4666-8666-666666666666".into();
    QuotaReading {
        run_id: input.context.run_id.clone(),
        binding: activity.binding,
        identity_evidence: activity.identity_evidence,
        provider_version: activity.provider_version,
        source_kind: activity.source_kind,
        started_at_utc: TIME.into(),
        observed_at_utc: TIME.into(),
        completed_at_utc: TIME.into(),
        limits: vec![QuotaLimit {
            limit_id: "synthetic-window".into(),
            limit_name: None,
            primary: Some(QuotaWindow {
                used_percent: 95,
                window_duration_mins: Some(300),
                resets_at_unix: None,
            }),
            secondary: None,
            reached_type: None,
            monthly_credit_limit: None,
            spend_control_reached: None,
        }],
        available_reset_credits: None,
        reset_credit_details: None,
        evidence: Some(input),
    }
}

fn application_export(storage: &mut Storage) -> Vec<u8> {
    let mut bytes = Vec::new();
    storage.write_application_export(&mut bytes, TIME).unwrap();
    bytes
}

fn complete_evidence(storage: &Storage, run_id: &str) -> serde_json::Value {
    let Some(ReadingEvidenceAvailability::Complete(record)) =
        storage.reading_evidence(run_id).unwrap()
    else {
        panic!("An accepted method must retain its complete evidence.");
    };
    serde_json::to_value(record).unwrap()
}

#[test]
fn non_live_acceptance_requires_evidence_and_preserves_it_with_exact_readings() {
    let directory = private_directory();
    let path = directory.path().join("evidence.db");
    let mut storage = Storage::create(&path, &KEY).unwrap();
    assert_eq!(storage.metadata().unwrap().schema_version, 14);
    storage.create_profile(PROFILE, "Synthetic", HOME).unwrap();
    assert!(storage.reading_evidence(RUN).unwrap().is_none());

    let mut reading = activity();
    assert_eq!(
        storage.accept_activity_with_binding(reading.clone(), Some(association(&reading))),
        Err(ActivityError::InvalidInput)
    );
    assert_eq!(storage.profile_binding(PROFILE).unwrap(), None);
    assert_eq!(storage.current_activity(PROFILE).unwrap(), None);
    assert!(storage.reading_evidence(RUN).unwrap().is_none());

    reading.evidence = Some(evidence(&reading));
    storage
        .accept_activity_with_binding(reading.clone(), Some(association(&reading)))
        .unwrap();
    let Some(ReadingEvidenceAvailability::Complete(record)) =
        storage.reading_evidence(RUN).unwrap()
    else {
        panic!("An accepted method must retain its complete evidence.");
    };
    assert_eq!(record.run_id, RUN);
    assert_eq!(record.adapter_id, "codex_app_server");
    assert_eq!(record.adapter_revision, 1);
    assert_eq!(record.source_level, "official");
    assert_eq!(record.reading_status, "official");
    assert_eq!(record.source_kind, ActivitySourceKind::TestFixture);
    assert_eq!(record.observed_at_utc, TIME);
    assert_eq!(record.provider_version, "0.163.0");
    assert_eq!(record.comparison_result, "not_performed");
    assert_eq!(record.redacted_response_hash, "a".repeat(64));
    assert_eq!(record.format_fingerprint, "b".repeat(64));
    let serialized = serde_json::to_string(&record).unwrap();
    for private in [EMAIL, HOME, "synthetic-private-plan"] {
        assert!(!serialized.contains(private));
        assert!(!format!("{record:?}").contains(private));
    }
    assert_eq!(
        storage
            .current_activity(PROFILE)
            .unwrap()
            .unwrap()
            .summary
            .lifetime_tokens
            .as_deref(),
        Some("9007199254740993")
    );
    assert_eq!(
        storage.accept_activity_with_binding(reading, None),
        Err(ActivityError::AlreadyAccepted)
    );
    drop(storage);

    let readonly = Storage::open_read_only(&path, &KEY).unwrap();
    assert!(matches!(
        readonly.reading_evidence(RUN).unwrap(),
        Some(ReadingEvidenceAvailability::Complete(_))
    ));
}

#[test]
fn complete_evidence_is_valid_in_a_backup_and_the_lossless_application_export() {
    let directory = private_directory();
    let mut storage = Storage::create(&directory.path().join("backup.db"), &KEY).unwrap();
    storage.create_profile(PROFILE, "Synthetic", HOME).unwrap();
    let mut reading = activity();
    reading.evidence = Some(evidence(&reading));
    storage
        .accept_activity_with_binding(reading.clone(), Some(association(&reading)))
        .unwrap();

    let snapshot = storage.backup_snapshot().unwrap();
    assert_eq!(snapshot.schema_version, 14);
    validate_backup_snapshot(&snapshot.bytes, snapshot.schema_version).unwrap();

    let mut bytes = Vec::new();
    let summary = storage.write_application_export(&mut bytes, TIME).unwrap();
    assert_eq!(summary.schema_version, 14);
    assert_eq!(summary.table_count, 44);
    let document: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let tables = document["tables"].as_array().unwrap();
    let evidence = tables
        .iter()
        .find(|table| table["name"] == "reading_evidence")
        .unwrap();
    assert_eq!(evidence["rows"].as_array().unwrap().len(), 1);
    let exported = serde_json::to_string(evidence).unwrap();
    assert!(exported.contains(RUN));
    assert!(exported.contains(&"a".repeat(64)));
    assert!(exported.contains("official"));
    for private in [EMAIL, HOME, "synthetic-private-plan"] {
        assert!(!exported.contains(private));
    }
}

#[test]
fn each_method_keeps_exact_evidence_through_archive_and_encrypted_checkpoint_restore() {
    let directory = private_directory();
    let path = directory.path().join("round-trip.db");
    let checkpoint = directory.path().join("checkpoint");
    let mut storage = Storage::create(&path, &KEY).unwrap();
    storage.create_profile(PROFILE, "Synthetic", HOME).unwrap();
    let mut activity = activity();
    let mut input = evidence(&activity);
    input.response.unused_fields = vec![EvidenceUnusedField {
        location: EvidenceLocation::ActivitySummary,
        field_count: 2,
    }];
    activity.evidence = Some(input);
    let mut quota = quota();
    let quota_run = quota.run_id.clone();
    let input = quota.evidence.as_mut().unwrap();
    input.adapter_revision = 7;
    input.response.redacted_response_hash = "c".repeat(64);
    input.response.format_fingerprint = "d".repeat(64);
    input.response.unused_fields = vec![EvidenceUnusedField {
        location: EvidenceLocation::QuotaWindow,
        field_count: 3,
    }];
    storage
        .accept_collection(
            Some(quota),
            Some(activity.clone()),
            Some(association(&activity)),
        )
        .unwrap();

    let expected: Vec<_> = [
        (RUN, "activity", 1, "a", "b", "activity_summary", 2),
        (quota_run.as_str(), "quota", 7, "c", "d", "quota_window", 3),
    ]
    .into_iter()
    .map(
        |(run, method, revision, hash, fingerprint, location, count)| {
            (
                run,
                serde_json::json!({
                    "runId": run,
                    "method": method,
                    "adapterId": "codex_app_server",
                    "adapterRevision": revision,
                    "sourceLevel": "official",
                    "readingStatus": "official",
                    "sourceKind": "test_fixture",
                    "providerVersion": "0.163.0",
                    "observedAtUtc": TIME,
                    "projectionVersion": 1,
                    "redactedResponseHash": hash.repeat(64),
                    "formatFingerprint": fingerprint.repeat(64),
                    "checkResults": [
                        {"id": "rpc_envelope", "result": "passed"},
                        {"id": "request_id", "result": "passed"},
                        {"id": "response_fields", "result": "passed"}
                    ],
                    "comparisonResult": "not_performed",
                    "unusedFields": [{"location": location, "fieldCount": count}]
                }),
            )
        },
    )
    .collect();
    for (run, record) in &expected {
        assert_eq!(complete_evidence(&storage, run), *record);
    }
    let saved_activity = storage.current_activity(PROFILE).unwrap().unwrap();
    let saved_quota = storage.current_quota(PROFILE).unwrap().unwrap();
    assert_eq!(
        saved_activity.summary.lifetime_tokens.as_deref(),
        Some("9007199254740993")
    );
    assert_eq!(saved_activity.binding.home_reference, HOME);
    assert_eq!(saved_activity.source_kind, ActivitySourceKind::TestFixture);
    assert_eq!(saved_quota.source_kind, ActivitySourceKind::TestFixture);

    let archive = storage.backup_snapshot().unwrap();
    let metadata = validate_restore_input(&archive.bytes, archive.schema_version).unwrap();
    assert_eq!(metadata.profile_ids, vec![PROFILE]);
    storage.create_encrypted_checkpoint(&checkpoint).unwrap();
    let encrypted_bytes = std::fs::read(&checkpoint).unwrap();
    assert!(!encrypted_bytes.starts_with(b"SQLite format 3\0"));
    assert_eq!(
        storage
            .verify_encrypted_checkpoint(&checkpoint)
            .unwrap()
            .schema_version,
        14
    );
    let restored_home = "77777777-7777-4777-8777-777777777777";
    for restore_checkpoint in [false, true] {
        if restore_checkpoint {
            // Delete both methods before recovery, so a no-op restore cannot pass.
            let revision = storage.restore_state().unwrap().revision;
            storage
                .remove_profile(
                    ProfileRemovalRequest {
                        profile_id: PROFILE.into(),
                        expected_home_reference: restored_home.into(),
                        history: ProfileHistoryChoice::Delete,
                    },
                    &revision,
                )
                .unwrap();
            for (run, _) in &expected {
                assert!(storage.reading_evidence(run).unwrap().is_none());
            }
            drop(storage);
            storage = Storage::open(&path, &KEY).unwrap();
            let snapshot = storage.read_encrypted_checkpoint(&checkpoint).unwrap();
            let metadata =
                validate_checkpoint_restore_input(&snapshot.bytes, snapshot.schema_version)
                    .unwrap();
            assert_eq!(
                metadata.homes,
                vec![RestoreHomeMapping {
                    profile_id: PROFILE.into(),
                    managed_home_reference: HOME.into(),
                }]
            );
            let revision = storage.restore_state().unwrap().revision;
            storage
                .replace_records_from_checkpoint(
                    &snapshot.bytes,
                    snapshot.schema_version,
                    &revision,
                    &metadata.homes,
                )
                .unwrap();
        } else {
            let revision = storage.restore_state().unwrap().revision;
            storage
                .replace_records_from_backup(
                    &archive.bytes,
                    archive.schema_version,
                    &revision,
                    &[RestoreHomeMapping {
                        profile_id: PROFILE.into(),
                        managed_home_reference: restored_home.into(),
                    }],
                )
                .unwrap();
        }
        drop(storage);
        storage = Storage::open(&path, &KEY).unwrap();
        for (run, record) in &expected {
            assert_eq!(complete_evidence(&storage, run), *record);
        }
        assert_eq!(
            storage.current_activity(PROFILE).unwrap(),
            Some(saved_activity.clone())
        );
        assert_eq!(
            storage.current_quota(PROFILE).unwrap(),
            Some(saved_quota.clone())
        );
        let snapshot = storage.backup_snapshot().unwrap();
        validate_backup_snapshot(&snapshot.bytes, snapshot.schema_version).unwrap();
    }
    assert_eq!(std::fs::read(&checkpoint).unwrap(), encrypted_bytes);
    assert!(!directory.path().join("checkpoint-wal").exists());
    assert!(!directory.path().join("checkpoint-shm").exists());
}

#[test]
fn mismatched_or_malformed_evidence_never_changes_any_application_record() {
    let directory = private_directory();
    let mut storage = Storage::create(&directory.path().join("rejected.db"), &KEY).unwrap();
    storage.create_profile(PROFILE, "Synthetic", HOME).unwrap();
    let mut baseline = Vec::new();
    storage
        .write_application_export(&mut baseline, TIME)
        .unwrap();

    for case in 0..12 {
        let mut reading = activity();
        let mut input = evidence(&reading);
        match case {
            0 => input.context.method = EvidenceMethod::Quota,
            1 => input.context.run_id = "55555555-5555-4555-8555-555555555555".into(),
            2 => input.context.profile_id = "55555555-5555-4555-8555-555555555555".into(),
            3 => input.context.binding_id = "55555555-5555-4555-8555-555555555555".into(),
            4 => input.context.source_kind = ActivitySourceKind::OfficialArtifact,
            5 => input.context.observed_at_utc = "2026-11-16T00:00:01.000Z".into(),
            6 => input.context.provider_version = "0.164.0".into(),
            7 => input.response.redacted_response_hash = "synthetic-private-token".into(),
            8 => input.response.projection_version = 2,
            9 => {
                input.response.unused_fields = vec![EvidenceUnusedField {
                    location: EvidenceLocation::QuotaResult,
                    field_count: 1,
                }]
            }
            10 => {
                input.response.unused_fields = vec![EvidenceUnusedField {
                    location: EvidenceLocation::ActivityResult,
                    field_count: 0,
                }]
            }
            11 => {
                reading.provider_version = EMAIL.into();
                input.context.provider_version = EMAIL.into();
            }
            _ => unreachable!(),
        }
        reading.evidence = Some(input);
        assert_eq!(
            storage.accept_activity_with_binding(reading.clone(), Some(association(&reading))),
            Err(ActivityError::InvalidInput),
            "synthetic case {case}",
        );
        assert!(storage.reading_evidence(RUN).unwrap().is_none());
        let mut after = Vec::new();
        storage.write_application_export(&mut after, TIME).unwrap();
        assert_eq!(after, baseline, "synthetic case {case}");
    }
}

#[test]
fn schema_thirteen_history_migrates_without_fabricating_response_evidence() {
    let directory = private_directory();
    let path = directory.path().join("legacy.db");
    let mut storage = Storage::create(&path, &KEY).unwrap();
    storage.create_profile(PROFILE, "Synthetic", HOME).unwrap();
    let mut reading = activity();
    reading.evidence = Some(evidence(&reading));
    storage
        .accept_activity_with_binding(reading.clone(), Some(association(&reading)))
        .unwrap();
    let saved = storage.current_activity(PROFILE).unwrap().unwrap();
    drop(storage);

    // Version 14 only adds these two tables and their triggers. The remaining catalog is version 13.
    let connection = controlled_connection(&path);
    connection.execute_batch("DROP TABLE reading_evidence; DROP TABLE reading_evidence_legacy; PRAGMA user_version=13;").unwrap();
    connection.execute_batch("ATTACH DATABASE ':memory:' AS legacy_image KEY ''; SELECT sqlcipher_export('legacy_image','main'); PRAGMA legacy_image.user_version=13;").unwrap();
    let bytes = connection.serialize("legacy_image").unwrap().to_vec();
    validate_backup_snapshot(&bytes, 13).unwrap();
    drop(connection);

    let mut migrated = Storage::open(&path, &KEY).unwrap();
    assert_eq!(migrated.metadata().unwrap().schema_version, 14);
    assert_eq!(migrated.current_activity(PROFILE).unwrap().unwrap(), saved);
    assert!(matches!(
        migrated.reading_evidence(RUN).unwrap(),
        Some(ReadingEvidenceAvailability::LegacyGap)
    ));
    let baseline = application_export(&mut migrated);
    let control = controlled_connection(&path);
    for statement in [
        "UPDATE reading_evidence_legacy SET collection_run_id=?1 WHERE collection_run_id=?1",
        "INSERT INTO reading_evidence_legacy(collection_run_id) VALUES(?1)",
        "INSERT OR REPLACE INTO reading_evidence_legacy(collection_run_id) VALUES(?1)",
        "INSERT OR IGNORE INTO reading_evidence_legacy(collection_run_id) VALUES(?1)",
    ] {
        assert!(control.execute(statement, [RUN]).is_err(), "{statement}");
        assert_eq!(application_export(&mut migrated), baseline, "{statement}");
        assert!(matches!(
            migrated.reading_evidence(RUN).unwrap(),
            Some(ReadingEvidenceAvailability::LegacyGap)
        ));
    }
    drop(control);
    let mut exported = Vec::new();
    migrated
        .write_application_export(&mut exported, TIME)
        .unwrap();
    let document: serde_json::Value = serde_json::from_slice(&exported).unwrap();
    let tables = document["tables"].as_array().unwrap();
    let complete = tables
        .iter()
        .find(|table| table["name"] == "reading_evidence")
        .unwrap();
    assert_eq!(complete["rows"].as_array().unwrap().len(), 0);
    let gaps = tables
        .iter()
        .find(|table| table["name"] == "reading_evidence_legacy")
        .unwrap();
    assert_eq!(gaps["rows"], serde_json::json!([[RUN]]));
    let snapshot = migrated.backup_snapshot().unwrap();
    validate_backup_snapshot(&snapshot.bytes, 14).unwrap();

    // Restore the actual version-13 image, not a version-14 backup of its migrated records.
    let metadata = validate_restore_input(&bytes, 13).unwrap();
    assert_eq!(metadata.profile_ids, vec![PROFILE]);
    let mut restored = Storage::create(&directory.path().join("restored.db"), &KEY).unwrap();
    let revision = restored.restore_state().unwrap().revision;
    let restored_home = "77777777-7777-4777-8777-777777777777";
    restored
        .replace_records_from_backup(
            &bytes,
            13,
            &revision,
            &[RestoreHomeMapping {
                profile_id: PROFILE.into(),
                managed_home_reference: restored_home.into(),
            }],
        )
        .unwrap();
    assert_eq!(restored.metadata().unwrap().schema_version, 14);
    assert_eq!(restored.current_activity(PROFILE).unwrap().unwrap(), saved);
    assert!(matches!(
        restored.reading_evidence(RUN).unwrap(),
        Some(ReadingEvidenceAvailability::LegacyGap)
    ));
    let document: serde_json::Value =
        serde_json::from_slice(&application_export(&mut restored)).unwrap();
    let tables = document["tables"].as_array().unwrap();
    for (name, rows) in [
        ("reading_evidence", serde_json::json!([])),
        ("reading_evidence_legacy", serde_json::json!([[RUN]])),
    ] {
        assert_eq!(
            tables.iter().find(|table| table["name"] == name).unwrap()["rows"],
            rows
        );
    }

    // The duplicate guard must still permit the existing explicit profile deletion and cascade.
    let revision = restored.restore_state().unwrap().revision;
    restored
        .remove_profile(
            ProfileRemovalRequest {
                profile_id: PROFILE.into(),
                expected_home_reference: restored_home.into(),
                history: ProfileHistoryChoice::Delete,
            },
            &revision,
        )
        .unwrap();
    assert!(restored.reading_evidence(RUN).unwrap().is_none());
    let snapshot = restored.backup_snapshot().unwrap();
    validate_backup_snapshot(&snapshot.bytes, 14).unwrap();
}

#[test]
fn evidence_write_failure_rolls_back_the_entire_first_association() {
    let directory = private_directory();
    let path = directory.path().join("rollback.db");
    let mut storage = Storage::create(&path, &KEY).unwrap();
    storage.create_profile(PROFILE, "Synthetic", HOME).unwrap();
    let mut baseline = Vec::new();
    storage
        .write_application_export(&mut baseline, TIME)
        .unwrap();
    let control = controlled_connection(&path);
    control.execute_batch("CREATE TRIGGER synthetic_evidence_failure BEFORE INSERT ON reading_evidence BEGIN SELECT RAISE(ABORT,'synthetic evidence failure'); END;").unwrap();
    let mut reading = activity();
    reading.evidence = Some(evidence(&reading));
    assert_eq!(
        storage.accept_activity_with_binding(reading.clone(), Some(association(&reading))),
        Err(ActivityError::WriteFailed)
    );
    control
        .execute_batch("DROP TRIGGER synthetic_evidence_failure;")
        .unwrap();
    assert!(storage.reading_evidence(RUN).unwrap().is_none());
    let mut after = Vec::new();
    storage.write_application_export(&mut after, TIME).unwrap();
    assert_eq!(after, baseline);
}

#[test]
fn committed_evidence_is_immutable_and_unexplained_absence_is_not_legacy() {
    let directory = private_directory();
    let path = directory.path().join("immutable.db");
    let mut storage = Storage::create(&path, &KEY).unwrap();
    storage.create_profile(PROFILE, "Synthetic", HOME).unwrap();
    let mut reading = activity();
    reading.evidence = Some(evidence(&reading));
    storage
        .accept_activity_with_binding(reading.clone(), Some(association(&reading)))
        .unwrap();
    let control = controlled_connection(&path);
    let baseline = application_export(&mut storage);
    let original = complete_evidence(&storage, RUN);
    for statement in [
        "UPDATE reading_evidence SET redacted_response_hash=printf('%064d',0) WHERE collection_run_id=?1",
        "INSERT INTO reading_evidence SELECT * FROM reading_evidence WHERE collection_run_id=?1",
        "INSERT OR REPLACE INTO reading_evidence SELECT collection_run_id,adapter_id,adapter_revision,projection_version,source_level,reading_status,check_results,comparison_result,printf('%064d',0),format_fingerprint,unused_fields FROM reading_evidence WHERE collection_run_id=?1",
        "INSERT OR IGNORE INTO reading_evidence SELECT * FROM reading_evidence WHERE collection_run_id=?1",
    ] {
        assert!(control.execute(statement, [RUN]).is_err(), "{statement}");
        assert_eq!(application_export(&mut storage), baseline, "{statement}");
        assert_eq!(complete_evidence(&storage, RUN), original, "{statement}");
    }
    assert!(
        control
            .execute(
                "INSERT INTO reading_evidence_legacy(collection_run_id) VALUES(?1)",
                [RUN]
            )
            .is_err()
    );
    assert_eq!(application_export(&mut storage), baseline);
    assert!(matches!(
        storage.reading_evidence(RUN).unwrap(),
        Some(ReadingEvidenceAvailability::Complete(_))
    ));

    // Simulate corruption in a private database, not an application deletion operation.
    control
        .execute(
            "DELETE FROM reading_evidence WHERE collection_run_id=?1",
            [RUN],
        )
        .unwrap();
    assert!(matches!(
        storage.reading_evidence(RUN),
        Err(ActivityError::ReadFailed)
    ));
    let snapshot = storage.backup_snapshot().unwrap();
    assert_eq!(
        validate_backup_snapshot(&snapshot.bytes, 14),
        Err(openusage_storage::BackupValidationError::InvalidDatabase)
    );
}

#[test]
fn second_method_evidence_failure_rolls_back_the_whole_live_collection() {
    let directory = private_directory();
    let path = directory.path().join("combined.db");
    let mut storage = Storage::create(&path, &KEY).unwrap();
    storage.create_profile(PROFILE, "Synthetic", HOME).unwrap();
    let mut baseline = Vec::new();
    storage
        .write_application_export(&mut baseline, TIME)
        .unwrap();
    let control = controlled_connection(&path);
    control.execute_batch(&format!("CREATE TRIGGER synthetic_second_evidence_failure BEFORE INSERT ON reading_evidence WHEN NEW.collection_run_id='{RUN}' BEGIN SELECT RAISE(ABORT,'synthetic second method failure'); END;")).unwrap();
    let mut activity = activity();
    activity.evidence = Some(evidence(&activity));
    let quota = quota();
    assert_eq!(
        storage.accept_live_collection(
            Some(quota.clone()),
            Some(activity.clone()),
            Some(association(&activity)),
            TIME
        ),
        Err(ActivityError::WriteFailed)
    );
    control
        .execute_batch("DROP TRIGGER synthetic_second_evidence_failure")
        .unwrap();
    let mut after = Vec::new();
    storage.write_application_export(&mut after, TIME).unwrap();
    assert_eq!(after, baseline);
    assert!(storage.reading_evidence(&quota.run_id).unwrap().is_none());
    assert!(storage.reading_evidence(RUN).unwrap().is_none());
    storage
        .accept_live_collection(
            Some(quota.clone()),
            Some(activity.clone()),
            Some(association(&activity)),
            TIME,
        )
        .unwrap();
    for (id, method) in [
        (&quota.run_id, EvidenceMethod::Quota),
        (&activity.run_id, EvidenceMethod::Activity),
    ] {
        let Some(ReadingEvidenceAvailability::Complete(record)) =
            storage.reading_evidence(id).unwrap()
        else {
            panic!("Complete method evidence required.")
        };
        assert_eq!(record.method, method);
    }
}

#[test]
fn failed_reassociation_evidence_preserves_every_old_record_and_binding() {
    let directory = private_directory();
    let path = directory.path().join("reassociation.db");
    let mut storage = Storage::create(&path, &KEY).unwrap();
    storage.create_profile(PROFILE, "Synthetic", HOME).unwrap();
    let mut first = activity();
    first.evidence = Some(evidence(&first));
    storage
        .accept_activity_with_binding(first.clone(), Some(association(&first)))
        .unwrap();
    let mut baseline = Vec::new();
    storage
        .write_application_export(&mut baseline, TIME)
        .unwrap();
    let mut next = first;
    next.run_id = "77777777-7777-4777-8777-777777777777".into();
    next.binding.binding_id = "88888888-8888-4888-8888-888888888888".into();
    next.identity_evidence.email = "different-synthetic@example.invalid".into();
    next.observed_at_utc = "2026-11-16T00:00:01.000Z".into();
    next.completed_at_utc = next.observed_at_utc.clone();
    next.evidence = Some(evidence(&next));
    let change = ProfileBindingInput {
        profile_id: PROFILE.into(),
        expected_home_reference: HOME.into(),
        expected_previous_binding_id: Some(BINDING.into()),
        new_binding_id: next.binding.binding_id.clone(),
        evidence: next.identity_evidence.clone(),
        observed_at_utc: next.observed_at_utc.clone(),
    };
    let control = controlled_connection(&path);
    control.execute_batch("CREATE TRIGGER synthetic_reassociation_failure BEFORE INSERT ON reading_evidence BEGIN SELECT RAISE(ABORT,'synthetic reassociation evidence failure'); END;").unwrap();
    assert_eq!(
        storage.accept_activity_with_binding(next, Some(change)),
        Err(ActivityError::WriteFailed)
    );
    control
        .execute_batch("DROP TRIGGER synthetic_reassociation_failure")
        .unwrap();
    let mut after = Vec::new();
    storage.write_application_export(&mut after, TIME).unwrap();
    assert_eq!(after, baseline);
    assert_eq!(
        storage
            .profile_binding(PROFILE)
            .unwrap()
            .unwrap()
            .binding_id,
        BINDING
    );
}
