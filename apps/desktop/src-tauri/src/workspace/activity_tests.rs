//! Native service checks with the reviewed static activity fixture and fresh managed homes.

use super::*;
use crate::keyvault::DatabaseKey;
use sha2::{Digest, Sha256};
use std::{
    cell::RefCell,
    os::unix::fs::PermissionsExt,
    time::{Duration, Instant, SystemTime},
};

#[derive(Default)]
struct MemoryVault(RefCell<Option<DatabaseKey>>);
impl KeyVault for MemoryVault {
    fn load(&self) -> Result<Option<DatabaseKey>, VaultError> {
        Ok(self.0.borrow().as_ref().map(|key| Zeroizing::new(**key)))
    }
    fn protect(&self, key: &[u8; 32]) -> Result<(), VaultError> {
        *self.0.borrow_mut() = Some(Zeroizing::new(*key));
        Ok(())
    }
}

struct Fixture {
    temporary: tempfile::TempDir,
    workspace: Workspace,
    executable: PathBuf,
    profiles: Vec<String>,
    vault: MemoryVault,
}
impl Fixture {
    fn new(mode: &str) -> Self {
        let temporary = tempfile::tempdir().unwrap();
        let executable = temporary.path().join("synthetic-activity-provider.py");
        fs::write(
            &executable,
            include_bytes!("../../../../../tests/native/fixtures/synthetic-activity-provider.py"),
        )
        .unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
        let mut workspace = Workspace::new(temporary.path().join("workspace")).unwrap();
        let vault = MemoryVault::default();
        workspace.setup(Ok(&vault), None).unwrap();
        for number in 1..=3 {
            workspace
                .create_profile(&format!("Synthetic activity profile {number}"))
                .unwrap();
        }
        let profiles = workspace
            .list_profiles()
            .unwrap()
            .profiles
            .into_iter()
            .map(|profile| profile.id)
            .collect();
        let fixture = Self {
            temporary,
            workspace,
            executable,
            profiles,
            vault,
        };
        fixture.mode(mode);
        for id in &fixture.profiles {
            fs::write(fixture.home(id).join("preserved-marker"), b"preserve").unwrap();
        }
        fixture
    }
    fn home(&self, id: &str) -> PathBuf {
        self.workspace.directory.join("profiles").join(id)
    }
    fn mode(&self, mode: &str) {
        fs::write(
            self.temporary
                .path()
                .join("synthetic-activity-provider-config.json"),
            serde_json::to_vec(&serde_json::json!({"defaultMode":mode})).unwrap(),
        )
        .unwrap();
    }
    fn approve(&self) -> String {
        let candidate = self
            .workspace
            .inspect_provider_executable(self.executable.to_str().unwrap())
            .unwrap();
        assert_eq!(
            candidate.source_kind,
            provider::ExecutableSourceKind::TestFixture
        );
        self.workspace
            .approve_provider_executable(&candidate.candidate_id, true)
            .unwrap()
            .approval_id
    }
    fn read(
        &mut self,
        id: &str,
        approval: &str,
    ) -> Result<provider::CompletedActivity, CommandError> {
        let receipt = self.workspace.begin_profile_activity_read(id, approval)?;
        self.workspace
            .complete_profile_activity_read(id, &receipt.request_id)?
            .wait()
    }
    fn pending_pid(&self, id: &str, previous: Option<libc::pid_t>) -> libc::pid_t {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let records = fs::read_to_string(
                self.temporary
                    .path()
                    .join("synthetic-activity-observations.jsonl"),
            )
            .unwrap_or_default();
            let mut current = None;
            let mut pending = None;
            for record in records
                .lines()
                .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
            {
                if record["kind"] == "start"
                    && record["arguments"][0] == "app-server"
                    && record["environment"]["CODEX_HOME"] == self.home(id).to_str().unwrap()
                {
                    current = record["pid"].as_i64();
                    pending = None;
                }
                if record["kind"] == "activity-read-pending" && record["pid"].as_i64() == current {
                    pending = current;
                }
            }
            if let Some(pid) = pending {
                let pid = pid.try_into().unwrap();
                if Some(pid) != previous {
                    return pid;
                }
            }
            assert!(
                Instant::now() < deadline,
                "The owned activity fixture did not reach its request barrier."
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    fn database_hashes(&self) -> Vec<Option<Vec<u8>>> {
        ["openusage.db", "openusage.db-wal", "openusage.db-shm"]
            .iter()
            .map(|name| {
                fs::read(self.workspace.directory.join(name))
                    .ok()
                    .map(|bytes| Sha256::digest(bytes).to_vec())
            })
            .collect()
    }
}
fn gone(pid: libc::pid_t) {
    assert_eq!(unsafe { libc::kill(pid, 0) }, -1);
    assert_eq!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(libc::ESRCH)
    );
    assert_eq!(unsafe { libc::killpg(pid, 0) }, -1);
}

#[test]
fn activity_acceptance_keeps_exact_counts_unknown_scope_and_independent_cached_days() {
    let mut fixture = Fixture::new("success");
    let id = fixture.profiles[0].clone();
    let approval = fixture.approve();
    assert_eq!(
        fixture
            .workspace
            .begin_profile_activity_read(&id, "00000000-0000-4000-8000-000000000000")
            .err(),
        Some(CommandError::StaleProviderApproval)
    );
    assert!(
        !fixture
            .temporary
            .path()
            .join("synthetic-activity-observations.jsonl")
            .exists()
    );
    let before = fixture.database_hashes();
    let completed = fixture.read(&id, &approval).unwrap();
    let joined = crate::restore::timestamp(std::time::SystemTime::now()).unwrap();
    std::thread::sleep(Duration::from_millis(20));
    let value = serde_json::to_value(
        fixture
            .workspace
            .finish_profile_activity_read(completed)
            .unwrap(),
    )
    .unwrap();
    assert_eq!(value["profileId"], id);
    assert_eq!(value["approvalId"], approval);
    assert_eq!(value["identityConfidence"], "provisional");
    assert!(value["verifiedIdentityKey"].is_null());
    assert_eq!(value["accepted"], true);
    assert_eq!(value["collectionRunId"], value["requestId"]);
    assert!(uuid::Uuid::parse_str(value["bindingId"].as_str().unwrap()).is_ok());
    assert_eq!(value["monitoring"], "stopped");
    assert_eq!(value["coverage"], "unknown");
    assert_eq!(
        value["scope"],
        "account_activity_with_unknown_application_scope"
    );
    assert_eq!(value["summary"]["lifetimeTokens"], "9007199254740993");
    assert_eq!(value["summary"]["peakDailyTokens"], "9223372036854775807");
    assert_eq!(value["summary"]["currentStreakDays"], "0");
    assert!(value["summary"]["longestRunningTurnSec"].is_null());
    assert_eq!(value["dailyUsageBuckets"][0]["startDate"], "2024-02-29");
    assert_eq!(value["dailyUsageBuckets"][0]["tokens"], "9007199254740993");
    assert!(value["observedAtUtc"].as_str().unwrap() <= joined.as_str());
    for secret in [
        "synthetic-private",
        "synthetic-activity@example.invalid",
        "CODEX_HOME",
        "workTokens",
        "modelCounts",
    ] {
        assert!(!value.to_string().contains(secret));
    }
    for (mode, expected) in [
        ("null_buckets", serde_json::Value::Null),
        ("empty_buckets", serde_json::json!([])),
    ] {
        fixture.mode(mode);
        let completed = fixture.read(&id, &approval).unwrap();
        let value = serde_json::to_value(
            fixture
                .workspace
                .finish_profile_activity_read(completed)
                .unwrap(),
        )
        .unwrap();
        assert_eq!(value["dailyUsageBuckets"], expected);
        assert_eq!(value["summary"]["currentStreakDays"], "0");
    }
    assert_ne!(fixture.database_hashes()[..2], before[..2]);
    let saved_before = fixture.database_hashes();
    let saved = serde_json::to_value(
        fixture
            .workspace
            .load_profile_activity(&id)
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(saved["accepted"], true);
    assert_eq!(saved["dailyState"], "empty");
    assert_eq!(saved["freshness"], "fresh");
    assert_eq!(saved["dailyUsageBuckets"][0]["tokens"], "9007199254740993");
    assert_eq!(saved["dailyUsageBuckets"][0]["date"], "2024-02-29");
    assert_eq!(saved["dailyUsageBuckets"][0]["freshness"], "fresh");
    assert_eq!(
        fixture.database_hashes()[..2],
        saved_before[..2],
        "A cached read must not write the database or WAL; WAL read marks may update shared memory."
    );
    assert!(before.iter().take(2).all(Option::is_some));
    assert_eq!(
        fs::read(fixture.home(&id).join("preserved-marker")).unwrap(),
        b"preserve"
    );
}

#[test]
fn activity_exact_cancellation_blocks_other_operations_and_reaps_on_quiescence() {
    let mut fixture = Fixture::new("pending");
    let id = fixture.profiles[0].clone();
    let other = fixture.profiles[1].clone();
    let approval = fixture.approve();
    let before = fixture.database_hashes();
    let receipt = fixture
        .workspace
        .begin_profile_activity_read(&id, &approval)
        .unwrap();
    let pid = fixture.pending_pid(&id, None);
    assert_eq!(
        fixture.workspace.provider.cancel(&id).err(),
        Some(CommandError::ProviderActivityInProgress)
    );
    assert_eq!(
        fixture.workspace.start_profile_check(&id).err(),
        Some(CommandError::ProviderActivityInProgress)
    );
    assert_eq!(
        fixture
            .workspace
            .begin_profile_login(&id, &approval, true)
            .err(),
        Some(CommandError::ProviderActivityInProgress)
    );
    assert_eq!(
        fixture
            .workspace
            .begin_profile_quota_read(&id, &approval)
            .err(),
        Some(CommandError::ProviderActivityInProgress)
    );
    assert!(
        !fixture
            .workspace
            .provider
            .cancel_quota(&id, &receipt.request_id)
            .unwrap()
            .cancelled
    );
    assert!(
        !fixture
            .workspace
            .provider
            .cancel_activity(&other, &receipt.request_id)
            .unwrap()
            .cancelled
    );
    let pending = fixture
        .workspace
        .complete_profile_activity_read(&id, &receipt.request_id)
        .unwrap();
    assert_eq!(
        fixture
            .workspace
            .complete_profile_activity_read(&id, &receipt.request_id)
            .err(),
        Some(CommandError::ProviderActivityInProgress)
    );
    let second = fixture
        .workspace
        .begin_profile_activity_read(&other, &approval)
        .unwrap();
    let second_pid = fixture.pending_pid(&other, None);
    assert_eq!(
        fixture
            .workspace
            .begin_profile_activity_read(&fixture.profiles[2], &approval)
            .err(),
        Some(CommandError::ProviderCapacityReached)
    );
    assert!(
        fixture
            .workspace
            .provider
            .cancel_activity(&id, &receipt.request_id)
            .unwrap()
            .cancelled
    );
    assert_eq!(
        pending.wait().err(),
        Some(CommandError::ProviderActivityCancelled)
    );
    gone(pid);
    let successor = fixture
        .workspace
        .begin_profile_activity_read(&id, &approval)
        .unwrap();
    let successor_pid = fixture.pending_pid(&id, Some(pid));
    assert!(
        !fixture
            .workspace
            .provider
            .cancel_activity(&id, &receipt.request_id)
            .unwrap()
            .cancelled
    );
    assert_eq!(unsafe { libc::kill(successor_pid, 0) }, 0);
    fixture.workspace.provider.quiesce().unwrap();
    gone(successor_pid);
    gone(second_pid);
    assert_eq!(
        fixture
            .workspace
            .complete_profile_activity_read(&id, &successor.request_id)
            .err(),
        Some(CommandError::ProviderActivityNotFound)
    );
    assert!(
        !fixture
            .workspace
            .provider
            .cancel_activity(&other, &second.request_id)
            .unwrap()
            .cancelled
    );
    assert_eq!(fixture.database_hashes(), before);
}

#[test]
fn activity_publication_refuses_changed_approval_home_and_archived_profile() {
    let mut fixture = Fixture::new("success");
    let id = fixture.profiles[0].clone();
    let approval = fixture.approve();
    let completed = fixture.read(&id, &approval).unwrap();
    let approval = fixture.approve();
    assert_eq!(
        fixture
            .workspace
            .finish_profile_activity_read(completed)
            .err(),
        Some(CommandError::ProviderActivityCancelled)
    );
    let completed = fixture.read(&id, &approval).unwrap();
    let home = fixture.home(&id);
    let old_home = home.with_extension("preserved");
    fs::rename(&home, &old_home).unwrap();
    prepare_directory(&home).unwrap();
    assert_eq!(
        fixture
            .workspace
            .finish_profile_activity_read(completed)
            .err(),
        Some(CommandError::ProviderHomeUnavailable)
    );
    fs::remove_dir(&home).unwrap();
    fs::rename(&old_home, &home).unwrap();
    let completed = fixture.read(&id, &approval).unwrap();
    fixture
        .workspace
        .storage
        .as_mut()
        .unwrap()
        .set_profile_archived(&id, true)
        .unwrap();
    assert_eq!(
        fixture
            .workspace
            .finish_profile_activity_read(completed)
            .err(),
        Some(CommandError::ProviderActivityCancelled)
    );
    fixture
        .workspace
        .storage
        .as_mut()
        .unwrap()
        .set_profile_archived(&id, false)
        .unwrap();
    for (mode, error) in [
        ("changed_email", CommandError::ProviderAccountChanged),
        ("auth_notice", CommandError::ProviderAccountChanged),
        ("thread_scope", CommandError::ProviderActivityFailed),
        ("overflow", CommandError::ProviderActivityFailed),
    ] {
        fixture.mode(mode);
        assert_eq!(fixture.read(&id, &approval).err(), Some(error), "{mode}");
    }
    assert_eq!(
        fs::read(home.join("preserved-marker")).unwrap(),
        b"preserve"
    );
}

#[test]
fn activity_restart_archive_and_failed_refresh_keep_durable_exact_readings_without_approval() {
    let mut fixture = Fixture::new("success");
    let id = fixture.profiles[0].clone();
    let other = fixture.profiles[1].clone();
    let approval = fixture.approve();
    let completed = fixture.read(&id, &approval).unwrap();
    let first = fixture
        .workspace
        .finish_profile_activity_read(completed)
        .unwrap();
    let before = fixture.database_hashes();
    fixture.mode("overflow");
    assert_eq!(
        fixture.read(&id, &approval).err(),
        Some(CommandError::ProviderActivityFailed)
    );
    let failed_request = fixture
        .workspace
        .activity_requests
        .get(&id)
        .unwrap()
        .clone();
    fixture
        .workspace
        .note_activity_failure(&id, &failed_request);
    let cached = serde_json::to_value(
        fixture
            .workspace
            .load_profile_activity(&id)
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(cached["collectionRunId"], first.collection_run_id);
    assert_eq!(cached["freshness"], "stale");
    assert_eq!(cached["dailyUsageBuckets"][0]["freshness"], "stale");
    assert_eq!(cached["summary"]["peakDailyTokens"], "9223372036854775807");
    assert_eq!(fixture.database_hashes()[..2], before[..2]);
    assert!(
        fixture
            .workspace
            .load_profile_activity(&other)
            .unwrap()
            .is_none()
    );
    fixture.workspace.set_profile_archived(&id, true).unwrap();
    assert!(
        fixture
            .workspace
            .load_profile_activity(&id)
            .unwrap()
            .is_some()
    );
    let directory = fixture.workspace.directory.clone();
    drop(fixture.workspace);
    let mut reopened = Workspace::new(directory).unwrap();
    assert_eq!(
        reopened.load_profile_activity(&id).err(),
        Some(CommandError::WorkspaceLocked)
    );
    assert_eq!(
        reopened.bootstrap(Ok(&fixture.vault)).storage,
        StorageState::Ready
    );
    let saved =
        serde_json::to_value(reopened.load_profile_activity(&id).unwrap().unwrap()).unwrap();
    assert_eq!(saved["collectionRunId"], first.collection_run_id);
    assert_eq!(saved["summary"]["peakDailyTokens"], "9223372036854775807");
    assert_eq!(saved["freshness"], "unknown");
    assert_eq!(saved["dailyUsageBuckets"][0]["freshness"], "unknown");
    assert!(reopened.load_profile_activity(&other).unwrap().is_none());
}

#[test]
fn activity_two_completed_profiles_commit_independently_and_binding_change_refuses_before_writes() {
    let mut fixture = Fixture::new("success");
    let first = fixture.profiles[0].clone();
    let second = fixture.profiles[1].clone();
    let approval = fixture.approve();
    let pending_first = fixture.read(&first, &approval).unwrap();
    let pending_second = fixture.read(&second, &approval).unwrap();
    fixture
        .workspace
        .finish_profile_activity_read(pending_first)
        .unwrap();
    fixture
        .workspace
        .finish_profile_activity_read(pending_second)
        .unwrap();
    assert!(
        fixture
            .workspace
            .load_profile_activity(&first)
            .unwrap()
            .is_some()
    );
    assert!(
        fixture
            .workspace
            .load_profile_activity(&second)
            .unwrap()
            .is_some()
    );
    let completed = fixture.read(&first, &approval).unwrap();
    let old = fixture
        .workspace
        .storage
        .as_ref()
        .unwrap()
        .activity_binding(&first)
        .unwrap()
        .unwrap();
    fixture
        .workspace
        .storage
        .as_mut()
        .unwrap()
        .associate_activity_binding(openusage_storage::ActivityBindingInput {
            profile_id: first.clone(),
            expected_home_reference: old.home_reference,
            expected_previous_binding_id: Some(old.binding_id),
            new_binding_id: provider::new_id().unwrap(),
            evidence: openusage_storage::ActivityIdentityEvidence {
                email: "new-synthetic@example.invalid".into(),
                plan: None,
            },
            observed_at_utc: restore::timestamp(SystemTime::now()).unwrap(),
        })
        .unwrap();
    let before = fixture.database_hashes();
    assert_eq!(
        fixture
            .workspace
            .finish_profile_activity_read(completed)
            .err(),
        Some(CommandError::ProviderActivityCancelled)
    );
    assert_eq!(fixture.database_hashes()[..2], before[..2]);
    assert!(
        fixture
            .workspace
            .load_profile_activity(&first)
            .unwrap()
            .is_none()
    );
    assert!(
        fixture
            .workspace
            .load_profile_activity(&second)
            .unwrap()
            .is_some()
    );
}

#[test]
fn activity_obsolete_failures_cannot_downgrade_a_newer_committed_reading() {
    let mut fixture = Fixture::new("success");
    let id = fixture.profiles[0].clone();
    let approval = fixture.approve();
    let old = fixture.read(&id, &approval).unwrap();
    let old_request = old.request_id.clone();
    let approval = fixture.approve();
    let newer = fixture.read(&id, &approval).unwrap();
    let committed = fixture
        .workspace
        .finish_profile_activity_read(newer)
        .unwrap();
    fixture.workspace.note_activity_failure(&id, &old_request);
    assert_eq!(
        fixture.workspace.finish_profile_activity_read(old).err(),
        Some(CommandError::ProviderActivityCancelled)
    );
    let saved = serde_json::to_value(
        fixture
            .workspace
            .load_profile_activity(&id)
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(saved["collectionRunId"], committed.collection_run_id);
    assert_eq!(saved["freshness"], "fresh");
    assert_eq!(saved["dailyUsageBuckets"][0]["freshness"], "fresh");
}

fn activity_tables(
    workspace: &mut Workspace,
) -> std::collections::BTreeMap<String, Vec<Vec<rusqlite::types::Value>>> {
    let snapshot = workspace.backup_snapshot().unwrap();
    let mut memory = rusqlite::Connection::open_in_memory().unwrap();
    memory
        .deserialize_read_exact(
            "main",
            std::io::Cursor::new(snapshot.bytes.as_slice()),
            snapshot.bytes.len(),
            true,
        )
        .unwrap();
    let mut all = std::collections::BTreeMap::new();
    for table in [
        "profile_identity_events",
        "collection_runs",
        "activity_summaries",
        "current_activity",
        "current_activity_days",
        "activity_day_history",
    ] {
        let mut statement = memory
            .prepare(&format!("SELECT * FROM {table} ORDER BY 1,2"))
            .unwrap();
        let columns = statement.column_count();
        let rows = statement
            .query_map([], |row| {
                (0..columns)
                    .map(|index| row.get(index))
                    .collect::<rusqlite::Result<Vec<rusqlite::types::Value>>>()
            })
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
        all.insert(table.into(), rows);
    }
    all
}

fn restore_activity_to_fresh_homes(
    fixture: &mut Fixture,
) -> Vec<openusage_storage::RestoreHomeMapping> {
    let snapshot = fixture.workspace.backup_snapshot().unwrap();
    let homes = fixture
        .profiles
        .iter()
        .map(|profile| {
            let home = provider::new_id().unwrap();
            prepare_directory(&fixture.workspace.directory.join("profiles").join(&home)).unwrap();
            openusage_storage::RestoreHomeMapping {
                profile_id: profile.clone(),
                managed_home_reference: home,
            }
        })
        .collect::<Vec<_>>();
    fixture.workspace.provider.quiesce().unwrap();
    let storage = fixture.workspace.storage.as_mut().unwrap();
    let before = storage.restore_state().unwrap();
    storage
        .replace_records_from_backup(
            &snapshot.bytes,
            snapshot.schema_version,
            &before.revision,
            &homes,
        )
        .unwrap();
    fixture.workspace.activity_sessions.clear();
    fixture.workspace.activity_requests.clear();
    fixture.workspace.activity_reviews.clear();
    homes
}

fn fresh_activity_review(
    fixture: &mut Fixture,
    id: &str,
    approval: &str,
) -> super::activity_storage::ActivityAssociationReview {
    fixture.mode("success");
    let completed = fixture.read(id, approval).unwrap();
    assert_eq!(
        fixture
            .workspace
            .finish_profile_activity_read(completed)
            .err(),
        Some(CommandError::ProviderActivityAssociationChanged)
    );
    fixture
        .workspace
        .review_profile_activity_association(id, approval)
        .unwrap()
}

#[test]
fn restored_activity_requires_explicit_new_sign_in_consent_and_keeps_original_history() {
    let mut fixture = Fixture::new("success");
    let id = fixture.profiles[0].clone();
    let approval = fixture.approve();
    assert_eq!(
        fixture
            .workspace
            .begin_profile_activity_reassociation(
                &id,
                &approval,
                &provider::new_id().unwrap(),
                true
            )
            .err(),
        Some(CommandError::ProviderActivityReviewUnavailable)
    );
    let completed = fixture.read(&id, &approval).unwrap();
    let initial = fixture
        .workspace
        .finish_profile_activity_read(completed)
        .unwrap();
    let old_tables = activity_tables(&mut fixture.workspace);
    let old_home = fixture.home(&id);
    let homes = restore_activity_to_fresh_homes(&mut fixture);
    let completed = fixture.read(&id, &approval).unwrap();
    assert_eq!(
        fixture
            .workspace
            .finish_profile_activity_read(completed)
            .err(),
        Some(CommandError::ProviderActivityAssociationChanged)
    );
    assert_eq!(activity_tables(&mut fixture.workspace), old_tables);
    let review = fixture
        .workspace
        .review_profile_activity_association(&id, &approval)
        .unwrap();
    assert_eq!(
        fixture
            .workspace
            .begin_profile_activity_reassociation(&id, &approval, &review.review_id, false)
            .err(),
        Some(CommandError::InvalidArguments)
    );
    assert_eq!(activity_tables(&mut fixture.workspace), old_tables);
    let consented = fixture
        .workspace
        .begin_profile_activity_reassociation(&id, &approval, &review.review_id, true)
        .unwrap();
    assert!(!consented.accepted);
    let completed = fixture
        .workspace
        .complete_profile_activity_read(&id, &consented.request_id)
        .unwrap()
        .wait()
        .unwrap();
    let accepted = fixture
        .workspace
        .finish_profile_activity_read(completed)
        .unwrap();
    assert!(accepted.accepted);
    assert_ne!(accepted.binding_id, initial.binding_id);
    let updated = activity_tables(&mut fixture.workspace);
    for table in [
        "profile_identity_events",
        "collection_runs",
        "activity_summaries",
        "activity_day_history",
    ] {
        for row in &old_tables[table] {
            assert!(
                updated[table].contains(row),
                "The association switch changed a historical field in {table}."
            );
        }
    }
    assert_eq!(updated["profile_identity_events"].len(), 2);
    assert_eq!(updated["collection_runs"].len(), 2);
    let cached = fixture
        .workspace
        .storage
        .as_ref()
        .unwrap()
        .current_activity(&id)
        .unwrap()
        .unwrap();
    assert_eq!(cached.binding.binding_id, accepted.binding_id);
    assert_eq!(
        cached.binding.home_reference,
        homes
            .iter()
            .find(|home| home.profile_id == id)
            .unwrap()
            .managed_home_reference
    );
    assert_eq!(
        cached.summary.lifetime_tokens.as_deref(),
        Some("9007199254740993")
    );
    assert_eq!(
        fs::read(old_home.join("preserved-marker")).unwrap(),
        b"preserve"
    );
}

#[test]
fn confirmed_activity_switch_still_refuses_unstable_account_responses_and_cancellation() {
    let mut fixture = Fixture::new("success");
    let id = fixture.profiles[0].clone();
    let approval = fixture.approve();
    let completed = fixture.read(&id, &approval).unwrap();
    fixture
        .workspace
        .finish_profile_activity_read(completed)
        .unwrap();
    restore_activity_to_fresh_homes(&mut fixture);
    let review = fresh_activity_review(&mut fixture, &id, &approval);
    let before = activity_tables(&mut fixture.workspace);
    fixture.mode("changed_email");
    let consented = fixture
        .workspace
        .begin_profile_activity_reassociation(&id, &approval, &review.review_id, true)
        .unwrap();
    assert_eq!(
        fixture
            .workspace
            .complete_profile_activity_read(&id, &consented.request_id)
            .unwrap()
            .wait()
            .err(),
        Some(CommandError::ProviderAccountChanged)
    );
    assert_eq!(activity_tables(&mut fixture.workspace), before);
    let review = fresh_activity_review(&mut fixture, &id, &approval);
    fixture.mode("pending");
    let consented = fixture
        .workspace
        .begin_profile_activity_reassociation(&id, &approval, &review.review_id, true)
        .unwrap();
    let pid = fixture.pending_pid(&id, None);
    let pending = fixture
        .workspace
        .complete_profile_activity_read(&id, &consented.request_id)
        .unwrap();
    assert!(
        fixture
            .workspace
            .provider
            .cancel_activity(&id, &consented.request_id)
            .unwrap()
            .cancelled
    );
    assert_eq!(
        pending.wait().err(),
        Some(CommandError::ProviderActivityCancelled)
    );
    gone(pid);
    assert_eq!(activity_tables(&mut fixture.workspace), before);
    let review = fresh_activity_review(&mut fixture, &id, &approval);
    let stale = fixture
        .workspace
        .begin_profile_activity_reassociation(&id, &approval, &review.review_id, true)
        .unwrap();
    let completed = fixture
        .workspace
        .complete_profile_activity_read(&id, &stale.request_id)
        .unwrap()
        .wait()
        .unwrap();
    fixture.approve();
    assert_eq!(
        fixture
            .workspace
            .finish_profile_activity_read(completed)
            .err(),
        Some(CommandError::ProviderActivityCancelled)
    );
    assert_eq!(activity_tables(&mut fixture.workspace), before);
}

#[test]
fn activity_review_exposes_only_bounded_hints_without_execution_and_consumes_only_a_started_read() {
    let mut fixture = Fixture::new("success");
    let id = fixture.profiles[0].clone();
    let approval = fixture.approve();
    let completed = fixture.read(&id, &approval).unwrap();
    fixture
        .workspace
        .finish_profile_activity_read(completed)
        .unwrap();
    restore_activity_to_fresh_homes(&mut fixture);
    let review = fresh_activity_review(&mut fixture, &id, &approval);
    let before = activity_tables(&mut fixture.workspace);
    let observations = fixture
        .temporary
        .path()
        .join("synthetic-activity-observations.jsonl");
    let executions = fs::read(&observations).unwrap();
    let repeated = fixture
        .workspace
        .review_profile_activity_association(&id, &approval)
        .unwrap();
    assert_eq!(review.review_id, repeated.review_id);
    let view = serde_json::to_value(&repeated).unwrap();
    assert_eq!(view.as_object().unwrap().len(), 8);
    assert_eq!(view["profileId"], id);
    assert_eq!(view["approvalId"], approval);
    assert_eq!(view["identityConfidence"], "provisional");
    assert_eq!(view["homeChanged"], true);
    assert_eq!(
        view["previous"],
        serde_json::json!({"email":"synthetic-activity@example.invalid","plan":"plus"})
    );
    assert_eq!(view["current"], view["previous"]);
    assert_eq!(
        fixture
            .workspace
            .begin_profile_activity_reassociation(&id, &approval, &review.review_id, false)
            .err(),
        Some(CommandError::InvalidArguments)
    );
    assert_eq!(
        fixture
            .workspace
            .begin_profile_activity_reassociation(
                &id,
                &approval,
                &provider::new_id().unwrap(),
                true
            )
            .err(),
        Some(CommandError::ProviderActivityReviewUnavailable)
    );
    assert_eq!(
        fixture
            .workspace
            .review_profile_activity_association(&id, &provider::new_id().unwrap())
            .err(),
        Some(CommandError::ProviderActivityReviewUnavailable)
    );
    assert_eq!(
        fixture
            .workspace
            .review_profile_activity_association(&fixture.profiles[1], &approval)
            .err(),
        Some(CommandError::ProviderActivityReviewUnavailable)
    );
    assert_eq!(
        fixture
            .workspace
            .review_profile_activity_association("bad-id", &approval)
            .err(),
        Some(CommandError::InvalidArguments)
    );
    assert_eq!(
        fixture
            .workspace
            .review_profile_activity_association(&id, &approval)
            .unwrap()
            .review_id,
        review.review_id
    );
    assert_eq!(fs::read(&observations).unwrap(), executions);
    assert_eq!(activity_tables(&mut fixture.workspace), before);
    // A successful start spends this review even when the user subsequently cancels.
    fixture.mode("pending");
    let request = fixture
        .workspace
        .begin_profile_activity_reassociation(&id, &approval, &review.review_id, true)
        .unwrap();
    assert_eq!(
        fixture
            .workspace
            .review_profile_activity_association(&id, &approval)
            .err(),
        Some(CommandError::ProviderActivityReviewUnavailable)
    );
    let pending = fixture
        .workspace
        .complete_profile_activity_read(&id, &request.request_id)
        .unwrap();
    assert!(
        fixture
            .workspace
            .provider
            .cancel_activity(&id, &request.request_id)
            .unwrap()
            .cancelled
    );
    assert_eq!(
        pending.wait().err(),
        Some(CommandError::ProviderActivityCancelled)
    );
    assert_eq!(activity_tables(&mut fixture.workspace), before);
}

#[test]
fn activity_review_refuses_expired_or_invalidated_fences_and_changed_postconsent_description() {
    for fault in [
        "expiry",
        "approval",
        "home",
        "binding",
        "archived",
        "description",
    ] {
        let mut fixture = Fixture::new("success");
        let id = fixture.profiles[0].clone();
        let approval = fixture.approve();
        let completed = fixture.read(&id, &approval).unwrap();
        fixture
            .workspace
            .finish_profile_activity_read(completed)
            .unwrap();
        restore_activity_to_fresh_homes(&mut fixture);
        let review = fresh_activity_review(&mut fixture, &id, &approval);
        match fault {
            "expiry" => {
                fixture
                    .workspace
                    .activity_reviews
                    .get_mut(&id)
                    .unwrap()
                    .fence
                    .created = Instant::now() - Duration::from_secs(301)
            }
            "approval" => {
                fixture.approve();
            }
            "home" => {
                let reference = &fixture.workspace.activity_reviews[&id]
                    .fence
                    .binding
                    .home_reference;
                let path = fixture.workspace.directory.join("profiles").join(reference);
                fs::rename(&path, path.with_extension("displaced")).unwrap();
                prepare_directory(&path).unwrap();
            }
            "binding" => {
                let cached = &fixture.workspace.activity_reviews[&id];
                fixture
                    .workspace
                    .storage
                    .as_mut()
                    .unwrap()
                    .associate_activity_binding(openusage_storage::ActivityBindingInput {
                        profile_id: id.clone(),
                        expected_home_reference: cached.fence.binding.home_reference.clone(),
                        expected_previous_binding_id: Some(
                            cached.previous_binding.binding_id.clone(),
                        ),
                        new_binding_id: provider::new_id().unwrap(),
                        evidence: cached.fence.current.clone(),
                        observed_at_utc: cached.observed_at_utc.clone(),
                    })
                    .unwrap();
            }
            "archived" => {
                fixture.workspace.set_profile_archived(&id, true).unwrap();
            }
            "description" => {
                // Model a reviewed account description that differs from the next stable full probe.
                fixture
                    .workspace
                    .activity_reviews
                    .get_mut(&id)
                    .unwrap()
                    .fence
                    .current
                    .email = "reviewed-other@example.invalid".into();
            }
            _ => unreachable!(),
        }
        let before = activity_tables(&mut fixture.workspace);
        if fault == "description" {
            let request = fixture
                .workspace
                .begin_profile_activity_reassociation(&id, &approval, &review.review_id, true)
                .unwrap();
            let completed = fixture
                .workspace
                .complete_profile_activity_read(&id, &request.request_id)
                .unwrap()
                .wait()
                .unwrap();
            assert_eq!(
                fixture
                    .workspace
                    .finish_profile_activity_read(completed)
                    .err(),
                Some(CommandError::ProviderAccountChanged)
            );
        } else {
            assert_eq!(
                fixture
                    .workspace
                    .review_profile_activity_association(&id, &approval)
                    .err(),
                Some(CommandError::ProviderActivityReviewUnavailable),
                "Fault: {fault}"
            );
            assert_eq!(
                fixture
                    .workspace
                    .begin_profile_activity_reassociation(&id, &approval, &review.review_id, true)
                    .err(),
                Some(CommandError::ProviderActivityReviewUnavailable),
                "Fault: {fault}"
            );
        }
        assert_eq!(
            activity_tables(&mut fixture.workspace),
            before,
            "Fault: {fault}"
        );
    }
}
