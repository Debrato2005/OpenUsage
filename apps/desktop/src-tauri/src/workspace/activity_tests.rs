//! Native service checks with the reviewed static activity fixture and fresh managed homes.

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
        workspace.setup(Ok(&MemoryVault::default()), None).unwrap();
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
fn activity_preview_keeps_exact_counts_unknown_scope_and_observation_time_without_writes() {
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
    assert_eq!(value["accepted"], false);
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
    assert_eq!(fixture.database_hashes(), before);
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
