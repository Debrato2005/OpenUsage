//! Native service checks with the reviewed static quota fixture and fresh managed homes.

use super::*;
use crate::keyvault::DatabaseKey;
use sha2::{Digest, Sha256};
use std::{
    cell::RefCell,
    os::unix::fs::PermissionsExt,
    time::{Duration, Instant},
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
        let executable = temporary.path().join("synthetic-quota-provider.py");
        fs::write(
            &executable,
            include_bytes!("../../../../../tests/native/fixtures/synthetic-quota-provider.py"),
        )
        .unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
        let mut workspace = Workspace::new(temporary.path().join("workspace")).unwrap();
        let vault = MemoryVault::default();
        workspace.setup(Ok(&vault), None).unwrap();
        for number in 1..=3 {
            workspace
                .create_profile(&format!("Synthetic quota profile {number}"))
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
        // These parser/persistence cases advance only their test fixture's monotonic timestamps.
        for id in &self.profiles {
            provider::tests::elapse_refresh(&self.workspace.provider, id);
        }
        fs::write(
            self.temporary
                .path()
                .join("synthetic-quota-provider-config.json"),
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
    fn read(&mut self, id: &str, approval: &str) -> Result<provider::CompletedQuota, CommandError> {
        provider::tests::elapse_refresh(&self.workspace.provider, id);
        let receipt = self.workspace.begin_profile_quota_read(id, approval)?;
        self.workspace
            .complete_profile_quota_read(id, &receipt.request_id)?
            .wait()
    }
    fn pending_pid(&self, id: &str, previous: Option<libc::pid_t>) -> libc::pid_t {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let records = fs::read_to_string(
                self.temporary
                    .path()
                    .join("synthetic-quota-observations.jsonl"),
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
                if record["kind"] == "quota-read-pending" && record["pid"].as_i64() == current {
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
                "The owned quota fixture did not reach its request barrier."
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
fn quota_reset_due_requires_a_known_reached_deadline_and_a_supported_clock() {
    use super::quota_storage::reset_due;
    use std::time::{SystemTime, UNIX_EPOCH};
    let time = |seconds: i64| -> SystemTime {
        let duration = Duration::from_secs(seconds.unsigned_abs());
        if seconds >= 0 {
            UNIX_EPOCH.checked_add(duration).unwrap()
        } else {
            UNIX_EPOCH.checked_sub(duration).unwrap()
        }
    };
    for reset in [-62_135_596_800, -1, 0, 253_402_300_799] {
        let at = time(reset);
        assert!(reset_due(Some(reset), at));
        assert!(reset_due(
            Some(reset),
            at.checked_add(Duration::from_nanos(1)).unwrap()
        ));
        assert!(!reset_due(
            Some(reset),
            at.checked_sub(Duration::from_nanos(1)).unwrap()
        ));
    }
    assert!(!reset_due(None, UNIX_EPOCH));
    for reset in [i64::MIN, -62_135_596_801, 253_402_300_800, i64::MAX] {
        assert!(!reset_due(Some(reset), UNIX_EPOCH));
    }
    assert!(!reset_due(Some(-1), time(-62_135_596_801)));
    assert!(!reset_due(Some(0), time(253_402_300_800)));
}

#[test]
fn saved_quota_reset_state_keeps_every_accepted_measurement_unchanged() {
    let mut fixture = Fixture::new("multi");
    let id = fixture.profiles[0].clone();
    let approval = fixture.approve();
    let completed = fixture.read(&id, &approval).unwrap();
    let outcome = serde_json::to_value(
        fixture
            .workspace
            .finish_profile_quota_read(completed)
            .unwrap(),
    )
    .unwrap();
    for limit in outcome["limits"].as_array().unwrap() {
        for role in ["primary", "secondary"] {
            if let Some(window) = limit[role].as_object() {
                assert_eq!(window.len(), 5);
                assert!(!window.contains_key("resetState"));
            }
        }
    }
    let storage = fixture.workspace.storage.as_mut().unwrap();
    let original = storage.current_quota(&id).unwrap().unwrap();
    let reading = openusage_storage::QuotaReading {
        run_id: provider::new_id().unwrap(),
        binding: original.binding.clone(),
        identity_evidence: storage.profile_binding_evidence(&original.binding).unwrap(),
        provider_version: original.provider_version.clone(),
        source_kind: original.source_kind,
        started_at_utc: original.commit.observed_at_utc.clone(),
        observed_at_utc: original.commit.observed_at_utc.clone(),
        completed_at_utc: original.commit.observed_at_utc.clone(),
        limits: vec![openusage_storage::QuotaLimit {
            limit_id: "synthetic-reset-boundaries".into(),
            limit_name: None,
            primary: Some(openusage_storage::QuotaWindow {
                used_percent: 25,
                window_duration_mins: Some(300),
                resets_at_unix: Some(-1),
            }),
            secondary: Some(openusage_storage::QuotaWindow {
                used_percent: 75,
                window_duration_mins: Some(10_080),
                resets_at_unix: Some(253_402_300_799),
            }),
            reached_type: None,
            monthly_credit_limit: None,
            spend_control_reached: None,
        }],
        available_reset_credits: original.available_reset_credits,
        reset_credit_details: original.reset_credit_details,
    };
    storage.accept_quota(reading, None).unwrap();
    let accepted = storage.current_quota(&id).unwrap();
    let revision = storage.restore_state().unwrap().revision;
    let before = fixture.database_hashes();
    let saved =
        serde_json::to_value(fixture.workspace.load_profile_quota(&id).unwrap().unwrap()).unwrap();
    for (role, state, used, remaining, reset) in [
        ("primary", "due", 25, 75, -1i64),
        ("secondary", "expected", 75, 25, 253_402_300_799),
    ] {
        let window = &saved["limits"][0][role];
        assert_eq!(window.as_object().unwrap().len(), 6);
        assert_eq!(window["resetState"], state);
        assert_eq!(window["usedPercent"], used);
        assert_eq!(window["derivedRemainingPercent"], remaining);
        assert_eq!(window["resetsAtUnix"], reset);
    }
    assert_eq!(fixture.database_hashes(), before);
    let storage = fixture.workspace.storage.as_mut().unwrap();
    assert_eq!(storage.current_quota(&id).unwrap(), accepted);
    assert_eq!(storage.restore_state().unwrap().revision, revision);
}

#[test]
fn quota_read_commits_exact_provisional_values_and_loads_without_approval() {
    let mut fixture = Fixture::new("multi");
    let id = fixture.profiles[0].clone();
    let approval = fixture.approve();
    assert_eq!(
        fixture
            .workspace
            .begin_profile_quota_read(&id, "00000000-0000-4000-8000-000000000000")
            .err(),
        Some(CommandError::StaleProviderApproval)
    );
    assert!(
        !fixture
            .temporary
            .path()
            .join("synthetic-quota-observations.jsonl")
            .exists()
    );
    let before = fixture.database_hashes();
    let revision = fixture
        .workspace
        .storage
        .as_mut()
        .unwrap()
        .restore_state()
        .unwrap()
        .revision;
    let completed = fixture.read(&id, &approval).unwrap();
    let joined_at = crate::restore::timestamp(std::time::SystemTime::now()).unwrap();
    let joined_monotonic = Instant::now();
    // Delayed IPC publication must retain the read time, even after the worker has joined.
    std::thread::sleep(Duration::from_millis(20));
    let outcome = fixture
        .workspace
        .finish_profile_quota_read(completed)
        .unwrap();
    assert!(fixture.workspace.quota_sessions[&id].1.unwrap() <= joined_monotonic);
    let json = serde_json::to_value(outcome).unwrap();
    assert_eq!(json["profileId"], id);
    assert_eq!(json["approvalId"], approval);
    assert_eq!(json["identityConfidence"], "provisional");
    assert!(json["verifiedIdentityKey"].is_null());
    assert_eq!(json["monitoring"], "stopped");
    assert_eq!(json["accepted"], true);
    assert_eq!(json["availableResetCredits"], "3");
    assert!(json["resetCreditDetails"].is_null());
    assert!(json["observedAtUtc"].as_str().unwrap().ends_with('Z'));
    assert!(json["observedAtUtc"].as_str().unwrap() <= joined_at.as_str());
    assert_eq!(json["limits"].as_array().unwrap().len(), 2);
    for limit in json["limits"].as_array().unwrap() {
        assert!(limit["monthlyCreditLimit"].is_null());
        assert!(limit["spendControlReached"].is_null());
    }
    assert_eq!(json["limits"][0]["primary"]["usedPercent"], 25);
    assert_eq!(json["limits"][0]["primary"]["derivedRemainingPercent"], 75);
    assert_eq!(json["limits"][0]["primary"]["label"], "Five-hour");
    assert_eq!(json["limits"][0]["secondary"]["label"], "Seven-day");
    assert!(json["limits"][0]["secondary"]["resetsAtUnix"].is_null());
    assert_eq!(json["limits"][1]["primary"]["label"], "Primary window");
    assert!(json["limits"][1]["secondary"].is_null());
    let text = json.to_string();
    for forbidden in [
        "synthetic-quota@example.invalid",
        "synthetic-private-account-id",
        "syntheticPrivateCredit",
        "legacy-must-not-be-counted",
        "CODEX_HOME",
        "processGroup",
    ] {
        assert!(!text.contains(forbidden));
    }
    for (mode, count) in [("empty", 0), ("null_windows", 1), ("legacy", 1)] {
        fixture.mode(mode);
        let completed = fixture.read(&id, &approval).unwrap();
        let outcome = serde_json::to_value(
            fixture
                .workspace
                .finish_profile_quota_read(completed)
                .unwrap(),
        )
        .unwrap();
        assert_eq!(outcome["limits"].as_array().unwrap().len(), count);
        for limit in outcome["limits"].as_array().unwrap() {
            assert!(limit["monthlyCreditLimit"].is_null());
            assert!(limit["spendControlReached"].is_null());
        }
        assert_eq!(outcome["availableResetCredits"], "3");
        assert!(outcome["resetCreditDetails"].is_null());
        if mode == "null_windows" {
            assert!(outcome["limits"][0]["primary"].is_null());
            assert!(outcome["limits"][0]["secondary"].is_null());
        }
        if mode == "legacy" {
            assert_eq!(outcome["limits"][0]["primary"]["label"], "1440 minutes");
        }
    }
    assert_ne!(
        fixture
            .workspace
            .storage
            .as_mut()
            .unwrap()
            .restore_state()
            .unwrap()
            .revision,
        revision
    );
    assert_ne!(fixture.database_hashes(), before);
    let saved =
        serde_json::to_value(fixture.workspace.load_profile_quota(&id).unwrap().unwrap()).unwrap();
    assert_eq!(saved["accepted"], true);
    assert_eq!(saved["freshness"], "fresh");
    assert_eq!(saved["limits"][0]["primary"]["label"], "1440 minutes");
    assert_eq!(saved["bindingId"], json["bindingId"]);
    assert_ne!(saved["collectionRunId"], json["collectionRunId"]);
    fixture.workspace.provider.quiesce().unwrap();
    assert!(fixture.workspace.load_profile_quota(&id).unwrap().is_some());
    fixture.workspace.set_profile_archived(&id, true).unwrap();
    assert!(fixture.workspace.load_profile_quota(&id).unwrap().is_some());
    assert_eq!(
        fs::read(fixture.home(&id).join("preserved-marker")).unwrap(),
        b"preserve"
    );
}

#[test]
fn quota_exact_cancel_shared_slots_and_quiescence_own_the_actual_pending_process() {
    let mut fixture = Fixture::new("pending");
    let id = fixture.profiles[0].clone();
    let second_id = fixture.profiles[1].clone();
    let approval = fixture.approve();
    let receipt = fixture
        .workspace
        .begin_profile_quota_read(&id, &approval)
        .unwrap();
    let pid = fixture.pending_pid(&id, None);
    assert_eq!(
        fixture.workspace.provider.cancel(&id).err(),
        Some(CommandError::ProviderQuotaInProgress)
    );
    assert_eq!(
        fixture.workspace.start_profile_check(&id).err(),
        Some(CommandError::ProviderQuotaInProgress)
    );
    assert_eq!(
        fixture
            .workspace
            .begin_profile_login(&id, &approval, true)
            .err(),
        Some(CommandError::ProviderQuotaInProgress)
    );
    assert_eq!(
        fixture
            .workspace
            .complete_profile_quota_read(&second_id, &receipt.request_id)
            .err(),
        Some(CommandError::ProviderQuotaNotFound)
    );
    let pending = fixture
        .workspace
        .complete_profile_quota_read(&id, &receipt.request_id)
        .unwrap();
    assert_eq!(
        fixture
            .workspace
            .complete_profile_quota_read(&id, &receipt.request_id)
            .err(),
        Some(CommandError::ProviderQuotaInProgress)
    );
    let second = fixture
        .workspace
        .begin_profile_quota_read(&second_id, &approval)
        .unwrap();
    let second_pid = fixture.pending_pid(&second_id, None);
    assert_eq!(
        fixture
            .workspace
            .begin_profile_quota_read(&fixture.profiles[2], &approval)
            .err(),
        Some(CommandError::ProviderCapacityReached)
    );
    assert!(
        !fixture
            .workspace
            .provider
            .cancel_quota(&second_id, &receipt.request_id)
            .unwrap()
            .cancelled
    );
    assert!(
        fixture
            .workspace
            .provider
            .cancel_quota(&id, &receipt.request_id)
            .unwrap()
            .cancelled
    );
    assert_eq!(
        pending.wait().err(),
        Some(CommandError::ProviderQuotaCancelled)
    );
    gone(pid);
    assert_eq!(
        fixture
            .workspace
            .begin_profile_quota_read(&id, &approval)
            .err(),
        Some(CommandError::ProviderRefreshCooldown)
    );
    provider::tests::elapse_refresh(&fixture.workspace.provider, &id);
    let successor = fixture
        .workspace
        .begin_profile_quota_read(&id, &approval)
        .unwrap();
    let successor_pid = fixture.pending_pid(&id, Some(pid));
    assert!(
        !fixture
            .workspace
            .provider
            .cancel_quota(&id, &receipt.request_id)
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
            .complete_profile_quota_read(&id, &successor.request_id)
            .err(),
        Some(CommandError::ProviderQuotaNotFound)
    );
    assert!(
        !fixture
            .workspace
            .provider
            .cancel_quota(&second_id, &second.request_id)
            .unwrap()
            .cancelled
    );
    assert_eq!(
        fixture
            .workspace
            .begin_profile_quota_read(&id, &approval)
            .err(),
        Some(CommandError::ProviderRefreshCooldown)
    );
    provider::tests::elapse_refresh(&fixture.workspace.provider, &id);
    let final_request = fixture
        .workspace
        .begin_profile_quota_read(&id, &approval)
        .unwrap();
    let final_pid = fixture.pending_pid(&id, Some(successor_pid));
    let provider = fixture.workspace.provider.clone();
    drop(fixture.workspace);
    gone(final_pid);
    assert_eq!(
        provider.cancel_quota(&id, &final_request.request_id).err(),
        Some(CommandError::ProviderShuttingDown)
    );
}

#[test]
fn quota_publication_refuses_source_home_profile_changes_and_redacts_protocol_faults() {
    let mut fixture = Fixture::new("multi");
    let id = fixture.profiles[0].clone();
    let approval = fixture.approve();
    let completed = fixture.read(&id, &approval).unwrap();
    let approval = fixture.approve();
    assert_eq!(
        fixture.workspace.finish_profile_quota_read(completed).err(),
        Some(CommandError::ProviderQuotaCancelled)
    );
    let completed = fixture.read(&id, &approval).unwrap();
    let home = fixture.home(&id);
    let preserved = home.with_extension("preserved");
    fs::rename(&home, &preserved).unwrap();
    prepare_directory(&home).unwrap();
    assert_eq!(
        fixture.workspace.finish_profile_quota_read(completed).err(),
        Some(CommandError::ProviderHomeUnavailable)
    );
    fs::remove_dir(&home).unwrap();
    fs::rename(&preserved, &home).unwrap();
    let completed = fixture.read(&id, &approval).unwrap();
    fixture
        .workspace
        .storage
        .as_mut()
        .unwrap()
        .set_profile_archived(&id, true)
        .unwrap();
    assert_eq!(
        fixture.workspace.finish_profile_quota_read(completed).err(),
        Some(CommandError::ProviderQuotaCancelled)
    );
    fixture
        .workspace
        .storage
        .as_mut()
        .unwrap()
        .set_profile_archived(&id, false)
        .unwrap();
    for (mode, expected) in [
        (
            "unknown_identity",
            CommandError::ProviderAccountIdentityUnavailable,
        ),
        ("changed_email", CommandError::ProviderAccountChanged),
        ("early_auth", CommandError::ProviderAccountChanged),
        ("late_auth", CommandError::ProviderAccountChanged),
        ("mismatch", CommandError::ProviderIncompatibleQuotaScope),
        ("invalid", CommandError::ProviderQuotaFailed),
    ] {
        fixture.mode(mode);
        assert_eq!(
            fixture.read(&id, &approval).err(),
            Some(expected),
            "synthetic mode {mode}"
        );
    }
    assert_eq!(
        fs::read(home.join("preserved-marker")).unwrap(),
        b"preserve"
    );
}

#[test]
fn saved_quota_failures_keep_exact_values_and_request_scoped_freshness_after_restart() {
    let mut fixture = Fixture::new("multi");
    let id = fixture.profiles[0].clone();
    let other = fixture.profiles[1].clone();
    let approval = fixture.approve();
    let completed = fixture.read(&id, &approval).unwrap();
    fixture
        .workspace
        .finish_profile_quota_read(completed)
        .unwrap();
    let original = fixture
        .workspace
        .storage
        .as_ref()
        .unwrap()
        .current_quota(&id)
        .unwrap()
        .unwrap();
    assert!(
        fixture
            .workspace
            .load_profile_quota(&other)
            .unwrap()
            .is_none()
    );
    fixture.mode("invalid");
    assert_eq!(
        fixture.read(&id, &approval).err(),
        Some(CommandError::ProviderQuotaFailed)
    );
    let failed = fixture.workspace.quota_requests[&id].clone();
    fixture.workspace.note_quota_failure(&id, &failed);
    assert_eq!(
        fixture
            .workspace
            .storage
            .as_ref()
            .unwrap()
            .current_quota(&id)
            .unwrap(),
        Some(original.clone())
    );
    assert_eq!(
        serde_json::to_value(fixture.workspace.load_profile_quota(&id).unwrap()).unwrap()["freshness"],
        "stale"
    );
    fixture.mode("legacy");
    let completed = fixture.read(&id, &approval).unwrap();
    fixture
        .workspace
        .finish_profile_quota_read(completed)
        .unwrap();
    fixture.workspace.note_quota_failure(&id, &failed);
    assert_eq!(
        serde_json::to_value(fixture.workspace.load_profile_quota(&id).unwrap()).unwrap()["freshness"],
        "fresh"
    );
    let current = fixture
        .workspace
        .storage
        .as_ref()
        .unwrap()
        .current_quota(&id)
        .unwrap();
    let directory = fixture.workspace.directory.clone();
    drop(fixture.workspace);
    let mut workspace = Workspace::new(directory).unwrap();
    assert_eq!(
        workspace.bootstrap(Ok(&fixture.vault)).storage,
        StorageState::Ready
    );
    assert_eq!(
        workspace
            .storage
            .as_ref()
            .unwrap()
            .current_quota(&id)
            .unwrap(),
        current
    );
    assert_eq!(
        serde_json::to_value(workspace.load_profile_quota(&id).unwrap()).unwrap()["freshness"],
        "unknown"
    );
}

#[test]
fn quota_reassociation_reviews_are_read_only_one_use_and_bound_to_quota() {
    let mut fixture = Fixture::new("multi");
    let id = fixture.profiles[0].clone();
    let approval = fixture.approve();
    let completed = fixture.read(&id, &approval).unwrap();
    fixture
        .workspace
        .finish_profile_quota_read(completed)
        .unwrap();
    let snapshot = fixture.workspace.backup_snapshot().unwrap();
    let mappings = fixture
        .profiles
        .iter()
        .map(|id| {
            let home = provider::new_id().unwrap();
            prepare_directory(&fixture.workspace.directory.join("profiles").join(&home)).unwrap();
            openusage_storage::RestoreHomeMapping {
                profile_id: id.clone(),
                managed_home_reference: home,
            }
        })
        .collect::<Vec<_>>();
    fixture.workspace.provider.quiesce().unwrap();
    let storage = fixture.workspace.storage.as_mut().unwrap();
    let revision = storage.restore_state().unwrap().revision;
    storage
        .replace_records_from_backup(
            &snapshot.bytes,
            snapshot.schema_version,
            &revision,
            &mappings,
        )
        .unwrap();
    let before = storage.current_quota(&id).unwrap().unwrap();
    let approval = fixture.approve();
    let completed = fixture.read(&id, &approval).unwrap();
    assert_eq!(
        fixture.workspace.finish_profile_quota_read(completed).err(),
        Some(CommandError::ProviderQuotaAssociationChanged)
    );
    let observations = fixture
        .temporary
        .path()
        .join("synthetic-quota-observations.jsonl");
    let executions = fs::read(&observations).unwrap();
    let hashes = fixture.database_hashes();
    let review = fixture
        .workspace
        .review_profile_quota_association(&id, &approval)
        .unwrap();
    let token = review.review_id.clone();
    let json = serde_json::to_value(review).unwrap();
    assert_eq!(json.as_object().unwrap().len(), 8);
    assert_eq!(json["homeChanged"], true);
    assert_eq!(json["previous"], json["current"]);
    assert_eq!(
        serde_json::to_value(
            fixture
                .workspace
                .review_profile_quota_association(&id, &approval)
                .unwrap()
        )
        .unwrap(),
        json
    );
    assert_eq!(
        fixture
            .workspace
            .review_profile_activity_association(&id, &approval)
            .err(),
        Some(CommandError::ProviderActivityReviewUnavailable)
    );
    assert_eq!(
        fixture
            .workspace
            .begin_profile_quota_reassociation(&id, &approval, &token, false)
            .err(),
        Some(CommandError::InvalidArguments)
    );
    assert_eq!(
        fixture
            .workspace
            .begin_profile_quota_reassociation(&id, &provider::new_id().unwrap(), &token, true)
            .err(),
        Some(CommandError::ProviderQuotaReviewUnavailable)
    );
    assert_eq!(fs::read(&observations).unwrap(), executions);
    assert_eq!(fixture.database_hashes(), hashes);
    assert_eq!(
        fixture
            .workspace
            .begin_profile_quota_reassociation(&id, &approval, &token, true)
            .err(),
        Some(CommandError::ProviderRefreshCooldown)
    );
    assert_eq!(fixture.database_hashes(), hashes);
    provider::tests::elapse_refresh(&fixture.workspace.provider, &id);
    let receipt = fixture
        .workspace
        .begin_profile_quota_reassociation(&id, &approval, &token, true)
        .unwrap();
    assert_eq!(
        fixture
            .workspace
            .review_profile_quota_association(&id, &approval)
            .err(),
        Some(CommandError::ProviderQuotaReviewUnavailable)
    );
    let completed = fixture
        .workspace
        .complete_profile_quota_read(&id, &receipt.request_id)
        .unwrap()
        .wait()
        .unwrap();
    let accepted = fixture
        .workspace
        .finish_profile_quota_read(completed)
        .unwrap();
    assert_ne!(accepted.binding_id, before.binding.binding_id);
    assert_eq!(
        fixture
            .workspace
            .storage
            .as_ref()
            .unwrap()
            .current_quota(&id)
            .unwrap()
            .unwrap()
            .binding
            .home_reference,
        mappings
            .iter()
            .find(|m| m.profile_id == id)
            .unwrap()
            .managed_home_reference
    );
    assert!(
        fixture
            .workspace
            .storage
            .as_ref()
            .unwrap()
            .current_activity(&id)
            .unwrap()
            .is_none()
    );
}

#[test]
fn manual_refresh_cooldown_refuses_repeat_and_cross_kind_reads_before_spawn() {
    let mut fixture = Fixture::new("multi");
    let id = fixture.profiles[0].clone();
    let approval = fixture.approve();
    assert_eq!(
        fixture
            .workspace
            .load_profile_refresh_status(&id)
            .unwrap()
            .retry_after_seconds,
        0
    );
    let completed = fixture.read(&id, &approval).unwrap();
    fixture
        .workspace
        .finish_profile_quota_read(completed)
        .unwrap();
    let status = fixture.workspace.load_profile_refresh_status(&id).unwrap();
    assert_eq!(status.profile_id, id);
    assert!((1..=30).contains(&status.retry_after_seconds));
    let observations = fixture
        .temporary
        .path()
        .join("synthetic-quota-observations.jsonl");
    let before = fs::read(&observations).unwrap();
    let repeat = fixture.workspace.begin_profile_quota_read(&id, &approval);
    // Cleanup also runs against the old implementation, where this start incorrectly succeeds.
    if let Ok(receipt) = &repeat {
        fixture
            .workspace
            .provider
            .cancel_quota(&id, &receipt.request_id)
            .unwrap();
    }
    assert_eq!(
        repeat
            .err()
            .map(|error| serde_json::to_value(error).unwrap()),
        Some(serde_json::json!("provider_refresh_cooldown"))
    );
    assert_eq!(fs::read(&observations).unwrap(), before);
    assert_eq!(
        fixture
            .workspace
            .begin_profile_activity_read(&id, &approval)
            .err()
            .map(|error| serde_json::to_value(error).unwrap()),
        Some(serde_json::json!("provider_refresh_cooldown"))
    );
    assert_eq!(fs::read(&observations).unwrap(), before);
    let approval = fixture.approve();
    assert_eq!(
        fixture
            .workspace
            .begin_profile_quota_read(&id, &approval)
            .err(),
        Some(CommandError::ProviderRefreshCooldown)
    );
    let other = fixture.profiles[1].clone();
    let completed = fixture.read(&other, &approval).unwrap();
    fixture
        .workspace
        .finish_profile_quota_read(completed)
        .unwrap();
    assert!(
        (1..=30).contains(
            &fixture
                .workspace
                .load_profile_refresh_status(&id)
                .unwrap()
                .retry_after_seconds
        )
    );
    let missing = provider::new_id().unwrap();
    assert_eq!(
        fixture
            .workspace
            .load_profile_refresh_status(&missing)
            .err(),
        Some(CommandError::ProfileNotFound)
    );
    let third = fixture.profiles[2].clone();
    assert_eq!(
        fixture
            .workspace
            .begin_profile_quota_read(&third, &missing)
            .err(),
        Some(CommandError::StaleProviderApproval)
    );
    assert_eq!(
        fixture
            .workspace
            .load_profile_refresh_status(&third)
            .unwrap()
            .retry_after_seconds,
        0
    );
    let previous_pid = fixture.pending_pid(&id, None);
    fixture.mode("pending");
    let request = fixture
        .workspace
        .begin_profile_quota_read(&id, &approval)
        .unwrap();
    let pid = fixture.pending_pid(&id, Some(previous_pid));
    fixture
        .workspace
        .provider
        .cancel_quota(&id, &request.request_id)
        .unwrap();
    gone(pid);
    assert_eq!(
        fixture
            .workspace
            .begin_profile_quota_read(&id, &approval)
            .err(),
        Some(CommandError::ProviderRefreshCooldown)
    );
    fixture.workspace.provider.quiesce().unwrap();
    assert!(
        (1..=30).contains(
            &fixture
                .workspace
                .load_profile_refresh_status(&id)
                .unwrap()
                .retry_after_seconds
        )
    );
}
