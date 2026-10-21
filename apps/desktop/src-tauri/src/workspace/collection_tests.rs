//! Workspace integration with the pinned synthetic provider and fresh private homes.
use super::*;
use crate::keyvault::DatabaseKey;
use sha2::{Digest, Sha256};
use std::{cell::RefCell, collections::BTreeMap, os::unix::fs::PermissionsExt};

#[derive(Default)]
pub(super) struct MemoryVault(RefCell<Option<DatabaseKey>>);
impl KeyVault for MemoryVault {
    fn load(&self) -> Result<Option<DatabaseKey>, VaultError> {
        Ok(self.0.borrow().as_ref().map(|key| Zeroizing::new(**key)))
    }
    fn protect(&self, key: &[u8; 32]) -> Result<(), VaultError> {
        *self.0.borrow_mut() = Some(Zeroizing::new(*key));
        Ok(())
    }
}
pub(super) struct Fixture {
    pub(super) temporary: tempfile::TempDir,
    pub(super) workspace: Workspace,
    pub(super) executable: PathBuf,
    pub(super) profiles: Vec<String>,
    pub(super) vault: MemoryVault,
}
impl Fixture {
    pub(super) fn new(mode: &str) -> Self {
        let temporary = tempfile::tempdir().unwrap();
        let executable = temporary.path().join("synthetic-collection-provider.py");
        let bytes = include_bytes!(
            "../../../../../crates/openusage-codex/tests/fixtures/synthetic-collection-provider.py"
        );
        assert_eq!(
            format!("{:x}", Sha256::digest(bytes)),
            "afaeb19874b3b08b6439a9d5d82a6d2f873517697598bf695a2f92254a7f6bba"
        );
        fs::write(&executable, bytes).unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
        let mut workspace = Workspace::new(temporary.path().join("workspace")).unwrap();
        let vault = MemoryVault::default();
        workspace.setup(Ok(&vault), None).unwrap();
        for n in 1..=2 {
            workspace
                .create_profile(&format!("Synthetic collection {n}"))
                .unwrap();
        }
        let profiles = workspace
            .list_profiles()
            .unwrap()
            .profiles
            .into_iter()
            .map(|p| p.id)
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
    pub(super) fn home(&self, id: &str) -> PathBuf {
        self.workspace.directory.join("profiles").join(id)
    }
    pub(super) fn mode(&self, mode: &str) {
        for id in &self.profiles {
            provider::tests::elapse_refresh(&self.workspace.provider, id);
            fs::write(
                self.home(id).join("synthetic-collection-config.json"),
                serde_json::to_vec(&serde_json::json!({"mode":mode})).unwrap(),
            )
            .unwrap();
        }
    }
    pub(super) fn approve(&self) -> String {
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
    pub(super) fn completed(&mut self, id: &str, approval: &str) -> provider::CompletedCollection {
        provider::tests::elapse_refresh(&self.workspace.provider, id);
        let receipt = self
            .workspace
            .begin_profile_collection_read(id, approval)
            .unwrap();
        self.workspace
            .complete_profile_collection_read(id, &receipt.request_id)
            .unwrap()
            .wait()
            .unwrap()
    }
    pub(super) fn save(&mut self, id: &str, approval: &str) -> provider::CollectionOutcome {
        let completed = self.completed(id, approval);
        self.workspace
            .finish_profile_collection_read(completed)
            .unwrap()
    }
    pub(super) fn preview(&mut self) -> String {
        let source = self.temporary.path().join("Downloads/OpenUsage");
        let archive = crate::backup::create(
            &source,
            self.workspace.backup_snapshot().unwrap(),
            Zeroizing::new("synthetic restore password".into()),
        )
        .unwrap();
        self.workspace
            .prepare_restore_preview(
                &source,
                &archive.file_name,
                Zeroizing::new("synthetic restore password".into()),
            )
            .unwrap()
            .preview_id
    }
    pub(super) fn connection(&self) -> rusqlite::Connection {
        let connection =
            rusqlite::Connection::open(self.workspace.directory.join(DATABASE_FILE)).unwrap();
        let key = self.vault.load().unwrap().unwrap();
        let encoded = Zeroizing::new(key.iter().map(|b| format!("{b:02x}")).collect::<String>());
        connection
            .pragma_update(None, "key", format!("x'{}'", encoded.as_str()))
            .unwrap();
        connection
    }
}
type Records = BTreeMap<String, Vec<Vec<rusqlite::types::Value>>>;
fn records(workspace: &mut Workspace) -> Records {
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
    let mut result = BTreeMap::new();
    for table in [
        "profile_identity_events",
        "collection_runs",
        "activity_summaries",
        "current_activity",
        "current_activity_days",
        "activity_day_history",
        "profile_bindings",
        "quota_snapshots",
        "quota_limits",
        "credit_snapshots",
        "current_quota_readings",
        "current_quota",
        "quota_reset_assessments",
    ] {
        let mut statement = memory
            .prepare(&format!("SELECT * FROM {table} ORDER BY 1,2,3"))
            .unwrap();
        let columns = statement.column_count();
        let rows = statement
            .query_map([], |r| {
                (0..columns)
                    .map(|i| r.get(i))
                    .collect::<rusqlite::Result<Vec<_>>>()
            })
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
        result.insert(table.into(), rows);
    }
    result
}
fn cached(
    workspace: &Workspace,
    id: &str,
) -> (
    Option<openusage_storage::StoredQuota>,
    Option<openusage_storage::StoredActivity>,
) {
    let storage = workspace.storage.as_ref().unwrap();
    (
        storage.current_quota(id).unwrap(),
        storage.current_activity(id).unwrap(),
    )
}
fn history_retained(before: &Records, after: &Records) {
    for table in [
        "profile_identity_events",
        "collection_runs",
        "activity_summaries",
        "activity_day_history",
        "quota_snapshots",
        "quota_limits",
        "credit_snapshots",
        "quota_reset_assessments",
    ] {
        for row in &before[table] {
            assert!(
                after[table].contains(row),
                "Accepted history changed in {table}"
            );
        }
    }
}

fn failed_auth_read(
    f: &mut Fixture,
    id: &str,
    approval: &str,
) -> (String, provider::ProfileBinding, CommandError) {
    f.mode("signed_out");
    let receipt = f
        .workspace
        .begin_profile_collection_read(id, approval)
        .unwrap();
    let pending = f
        .workspace
        .complete_profile_collection_read(id, &receipt.request_id)
        .unwrap();
    let binding = pending.binding().clone();
    let error = pending.wait().err().unwrap();
    assert_eq!(error, CommandError::ProviderAuthenticationRequired);
    (receipt.request_id, binding, error)
}

#[test]
fn collection_auth_failure_after_reopen_updates_only_selected_connection_and_keeps_readings() {
    let mut f = Fixture::new("success");
    let id = f.profiles[0].clone();
    let other = f.profiles[1].clone();
    let approval = f.approve();
    f.save(&id, &approval);
    f.save(&other, &approval);
    for profile in [&id, &other] {
        f.workspace
            .storage
            .as_mut()
            .unwrap()
            .record_profile_connection(profile, profile, ProfileConnectionState::Connected)
            .unwrap();
    }
    let saved = cached(&f.workspace, &id);
    let other_saved = cached(&f.workspace, &other);
    let before = records(&mut f.workspace);
    let directory = f.workspace.directory.clone();
    drop(f.workspace);
    f.workspace = Workspace::new(directory).unwrap();
    assert_eq!(
        f.workspace.bootstrap(Ok(&f.vault)).storage,
        StorageState::Ready
    );
    assert!(
        serde_json::to_value(f.workspace.load_monitoring().unwrap()).unwrap()["profiles"]
            .as_array()
            .unwrap()
            .iter()
            .all(|profile| profile["eligible"] == false)
    );
    let approval = f.approve();
    let (request, binding, error) = failed_auth_read(&mut f, &id, &approval);
    f.workspace
        .note_collection_read_error(&binding, &request, error)
        .unwrap();
    let profiles = f.workspace.list_profiles().unwrap().profiles;
    assert_eq!(
        profiles
            .iter()
            .find(|p| p.id == id)
            .unwrap()
            .connection_state,
        "authentication_required"
    );
    assert_eq!(
        profiles
            .iter()
            .find(|p| p.id == other)
            .unwrap()
            .connection_state,
        "connected"
    );
    assert_eq!(cached(&f.workspace, &id), saved);
    assert_eq!(cached(&f.workspace, &other), other_saved);
    assert_eq!(records(&mut f.workspace), before);
    assert!(
        serde_json::to_value(f.workspace.load_monitoring().unwrap()).unwrap()["profiles"]
            .as_array()
            .unwrap()
            .iter()
            .all(|profile| profile["eligible"] == false)
    );
}

#[test]
fn collection_auth_failure_waiting_for_workspace_cannot_overwrite_a_new_login() {
    let mut f = Fixture::new("success");
    let id = f.profiles[0].clone();
    let approval = f.approve();
    f.save(&id, &approval);
    let saved = cached(&f.workspace, &id);
    let before = records(&mut f.workspace);
    let (request, binding, error) = failed_auth_read(&mut f, &id, &approval);
    f.mode("success");
    fs::write(
        f.home(&id).join("synthetic-collection-config.json"),
        br#"{"mode":"success","requireLogin":true}"#,
    )
    .unwrap();
    let login = f
        .workspace
        .begin_profile_login(&id, &approval, true)
        .unwrap();
    assert!(!f.workspace.quota_requests.contains_key(&id));
    assert!(!f.workspace.activity_requests.contains_key(&id));
    let prompt = f
        .workspace
        .await_profile_login_prompt(&id, &login.attempt_id)
        .unwrap()
        .wait()
        .unwrap();
    f.workspace.finish_profile_login_prompt(prompt).unwrap();
    let completed = f
        .workspace
        .complete_profile_login(&id, &login.attempt_id)
        .unwrap()
        .wait()
        .unwrap();
    f.workspace.finish_profile_login(completed).unwrap();
    let monitoring = serde_json::to_value(f.workspace.load_monitoring().unwrap()).unwrap();
    f.workspace
        .note_collection_read_error(&binding, &request, error)
        .unwrap();
    assert_eq!(
        f.workspace
            .list_profiles()
            .unwrap()
            .profiles
            .iter()
            .find(|p| p.id == id)
            .unwrap()
            .connection_state,
        "connected"
    );
    assert_eq!(
        serde_json::to_value(f.workspace.load_monitoring().unwrap()).unwrap(),
        monitoring
    );
    assert_eq!(cached(&f.workspace, &id), saved);
    assert_eq!(records(&mut f.workspace), before);
}

#[test]
fn collection_workspace_saves_exact_both_capabilities_retains_observation_anchors_and_reopens() {
    let mut f = Fixture::new("success");
    let id = f.profiles[0].clone();
    let other = f.profiles[1].clone();
    let approval = f.approve();
    f.save(&other, &approval);
    let original_other = cached(&f.workspace, &other);
    let completed = f.completed(&id, &approval);
    let anchors = completed.observation_anchors();
    assert!(anchors.0.is_some() && anchors.1.is_some());
    let preview = f.preview();
    let accepted = f
        .workspace
        .finish_profile_collection_read(completed)
        .unwrap();
    assert!(accepted.accepted);
    let q = accepted.quota.unwrap();
    let a = accepted.activity.unwrap();
    assert_eq!(q.binding_id, a.binding_id);
    assert_ne!(q.collection_run_id, a.collection_run_id);
    assert_eq!(
        f.workspace.quota_sessions[&id],
        (q.collection_run_id.clone(), anchors.0)
    );
    assert_eq!(
        f.workspace.activity_sessions[&id],
        (a.collection_run_id.clone(), anchors.1)
    );
    assert!(
        !f.workspace
            .cancel_restore_preview(&preview)
            .unwrap()
            .cancelled
    );
    let saved = cached(&f.workspace, &id);
    let quota = saved.0.as_ref().unwrap();
    let activity = saved.1.as_ref().unwrap();
    assert_eq!(quota.limits[0].primary.as_ref().unwrap().used_percent, 25);
    assert_eq!(
        activity.summary.lifetime_tokens.as_deref(),
        Some("9007199254740993")
    );
    assert_eq!(
        activity.summary.peak_daily_tokens.as_deref(),
        Some("9223372036854775807")
    );
    assert_eq!(activity.summary.current_streak_days.as_deref(), Some("0"));
    assert_eq!(quota.commit.observed_at_utc, q.observed_at_utc);
    assert_eq!(activity.commit.observed_at_utc, a.observed_at_utc);
    assert_eq!(cached(&f.workspace, &other), original_other);
    assert_eq!(
        f.workspace
            .begin_profile_collection_read(&id, &approval)
            .err(),
        Some(CommandError::ProviderRefreshCooldown)
    );
    assert_eq!(
        f.workspace.begin_profile_quota_read(&id, &approval).err(),
        Some(CommandError::ProviderRefreshCooldown)
    );
    assert_eq!(
        f.workspace
            .begin_profile_activity_read(&id, &approval)
            .err(),
        Some(CommandError::ProviderRefreshCooldown)
    );
    let all = records(&mut f.workspace);
    assert_eq!(all["collection_runs"].len(), 4);
    let directory = f.workspace.directory.clone();
    drop(f.workspace);
    let mut reopened = Workspace::new(directory).unwrap();
    assert_eq!(
        reopened.bootstrap(Ok(&f.vault)).storage,
        StorageState::Ready
    );
    assert_eq!(cached(&reopened, &id), saved);
    assert_eq!(cached(&reopened, &other), original_other);
    assert_eq!(records(&mut reopened), all);
    assert_eq!(
        serde_json::to_value(reopened.load_profile_quota(&id).unwrap()).unwrap()["freshness"],
        "unknown"
    );
    assert_eq!(
        serde_json::to_value(reopened.load_profile_activity(&id).unwrap()).unwrap()["freshness"],
        "unknown"
    );
}

#[test]
fn collection_workspace_explicit_unsupported_sides_preserve_metrics_and_only_commits_clear_preview()
{
    let views = |workspace: &Workspace, id: &str| {
        [
            serde_json::to_value(workspace.load_profile_quota(id).unwrap()).unwrap(),
            serde_json::to_value(workspace.load_profile_activity(id).unwrap()).unwrap(),
        ]
    };
    let mut blank = Fixture::new("both_unsupported");
    let id = blank.profiles[0].clone();
    let approval = blank.approve();
    let before = records(&mut blank.workspace);
    let completed = blank.completed(&id, &approval);
    let preview = blank.preview();
    let outcome = blank
        .workspace
        .finish_profile_collection_read(completed)
        .unwrap();
    assert!(!outcome.accepted);
    assert!(outcome.quota.is_none() && outcome.activity.is_none());
    assert!(
        blank
            .workspace
            .storage
            .as_ref()
            .unwrap()
            .profile_binding(&id)
            .unwrap()
            .is_none()
    );
    assert_eq!(records(&mut blank.workspace), before);
    assert!(
        blank
            .workspace
            .cancel_restore_preview(&preview)
            .unwrap()
            .cancelled
    );
    for mode in [
        "quota_unsupported",
        "activity_unsupported",
        "both_unsupported",
    ] {
        let mut f = Fixture::new("success");
        let id = f.profiles[0].clone();
        let approval = f.approve();
        f.save(&id, &approval);
        let previous = cached(&f.workspace, &id);
        let previous_views = views(&f.workspace, &id);
        for view in &previous_views {
            assert_eq!(view["freshness"], "fresh");
        }
        let before = records(&mut f.workspace);
        f.mode(mode);
        let completed = f.completed(&id, &approval);
        let preview = f.preview();
        let outcome = f
            .workspace
            .finish_profile_collection_read(completed)
            .unwrap();
        assert_eq!(outcome.accepted, mode != "both_unsupported");
        assert_eq!(outcome.quota.is_some(), mode == "activity_unsupported");
        assert_eq!(outcome.activity.is_some(), mode == "quota_unsupported");
        let after = cached(&f.workspace, &id);
        let after_views = views(&f.workspace, &id);
        if mode != "activity_unsupported" {
            assert_eq!(after.0, previous.0);
            assert_eq!(
                after_views[0]["observedAtUtc"],
                previous_views[0]["observedAtUtc"]
            );
            assert_eq!(
                after_views[0]["freshness"], previous_views[0]["freshness"],
                "Unsupported quota changed saved freshness in {mode}"
            );
        }
        if mode != "quota_unsupported" {
            assert_eq!(after.1, previous.1);
            assert_eq!(
                after_views[1]["observedAtUtc"],
                previous_views[1]["observedAtUtc"]
            );
            assert_eq!(
                after_views[1]["freshness"], previous_views[1]["freshness"],
                "Unsupported activity changed saved freshness in {mode}"
            );
        }
        assert_eq!(
            f.workspace
                .cancel_restore_preview(&preview)
                .unwrap()
                .cancelled,
            mode == "both_unsupported"
        );
        history_retained(&before, &records(&mut f.workspace));
    }
}

#[test]
fn collection_workspace_changed_identity_requires_exact_review_before_new_attribution() {
    let mut f = Fixture::new("success");
    let id = f.profiles[0].clone();
    let approval = f.approve();
    let first = f.save(&id, &approval);
    let original = cached(&f.workspace, &id);
    let before = records(&mut f.workspace);
    f.mode("stable_changed_identity");
    let completed = f.completed(&id, &approval);
    let preview = f.preview();
    assert_eq!(
        f.workspace.finish_profile_collection_read(completed).err(),
        Some(CommandError::ProviderCollectionAssociationChanged)
    );
    assert_eq!(cached(&f.workspace, &id), original);
    assert_eq!(records(&mut f.workspace), before);
    assert!(
        f.workspace
            .cancel_restore_preview(&preview)
            .unwrap()
            .cancelled
    );
    let review = f
        .workspace
        .review_profile_collection_association(&id, &approval)
        .unwrap();
    let details = serde_json::to_value(&review).unwrap();
    assert_ne!(details["previous"], details["current"]);
    assert_eq!(details["identityConfidence"], "provisional");
    assert_eq!(
        f.workspace
            .begin_profile_collection_reassociation(&id, &approval, &review.review_id, false)
            .err(),
        Some(CommandError::InvalidArguments)
    );
    provider::tests::elapse_refresh(&f.workspace.provider, &id);
    let receipt = f
        .workspace
        .begin_profile_collection_reassociation(&id, &approval, &review.review_id, true)
        .unwrap();
    let completed = f
        .workspace
        .complete_profile_collection_read(&id, &receipt.request_id)
        .unwrap()
        .wait()
        .unwrap();
    let accepted = f
        .workspace
        .finish_profile_collection_read(completed)
        .unwrap();
    assert!(accepted.accepted);
    assert_ne!(
        accepted.quota.as_ref().unwrap().binding_id,
        first.quota.unwrap().binding_id
    );
    assert_eq!(
        accepted.quota.as_ref().unwrap().binding_id,
        accepted.activity.as_ref().unwrap().binding_id
    );
    history_retained(&before, &records(&mut f.workspace));
    assert_eq!(
        f.workspace
            .review_profile_collection_association(&id, &approval)
            .err(),
        Some(CommandError::ProviderCollectionReviewUnavailable)
    );
}

#[test]
fn collection_workspace_post_wait_cancel_reapproval_archive_and_replaced_home_cannot_publish() {
    for fault in ["cancel", "reapproval", "archive", "home"] {
        let mut f = Fixture::new("success");
        let id = f.profiles[0].clone();
        let approval = f.approve();
        f.save(&id, &approval);
        let completed = f.completed(&id, &approval);
        let request = completed.request_id.clone();
        let original = cached(&f.workspace, &id);
        let mut moved = None;
        match fault {
            "cancel" => {
                assert!(
                    f.workspace
                        .provider
                        .cancel_collection(&id, &request)
                        .unwrap()
                        .cancelled
                );
            }
            "reapproval" => {
                f.approve();
            }
            "archive" => {
                f.workspace.set_profile_archived(&id, true).unwrap();
            }
            "home" => {
                let home = f.home(&id);
                let old = f.temporary.path().join("original-managed-home");
                fs::rename(&home, &old).unwrap();
                fs::create_dir(&home).unwrap();
                fs::set_permissions(&home, fs::Permissions::from_mode(0o700)).unwrap();
                moved = Some((home, old));
            }
            _ => unreachable!(),
        }
        let before = records(&mut f.workspace);
        assert_eq!(
            f.workspace.finish_profile_collection_read(completed).err(),
            Some(if fault == "home" {
                CommandError::ProviderHomeUnavailable
            } else {
                CommandError::ProviderCollectionCancelled
            }),
            "{fault}"
        );
        assert_eq!(cached(&f.workspace, &id), original);
        assert_eq!(records(&mut f.workspace), before);
        assert_eq!(
            f.workspace
                .complete_profile_collection_read(&id, &request)
                .err(),
            Some(CommandError::ProviderCollectionNotFound)
        );
        if let Some((home, old)) = moved {
            fs::remove_dir(&home).unwrap();
            fs::rename(old, home).unwrap();
        }
        assert_eq!(
            fs::read(f.home(&id).join("preserved-marker")).unwrap(),
            b"preserve"
        );
    }
}

#[test]
fn collection_workspace_second_half_failure_rolls_back_both_histories_binding_and_publication() {
    for reassociate in [false, true] {
        let mut f = Fixture::new("success");
        let id = f.profiles[0].clone();
        let approval = f.approve();
        let completed = if reassociate {
            f.save(&id, &approval);
            f.mode("stable_changed_identity");
            let changed = f.completed(&id, &approval);
            assert_eq!(
                f.workspace.finish_profile_collection_read(changed).err(),
                Some(CommandError::ProviderCollectionAssociationChanged)
            );
            let review = f
                .workspace
                .review_profile_collection_association(&id, &approval)
                .unwrap();
            provider::tests::elapse_refresh(&f.workspace.provider, &id);
            let receipt = f
                .workspace
                .begin_profile_collection_reassociation(&id, &approval, &review.review_id, true)
                .unwrap();
            f.workspace
                .complete_profile_collection_read(&id, &receipt.request_id)
                .unwrap()
                .wait()
                .unwrap()
        } else {
            f.completed(&id, &approval)
        };
        let request = completed.request_id.clone();
        let preview = f.preview();
        let original = cached(&f.workspace, &id);
        let before = records(&mut f.workspace);
        let connection = f.connection();
        connection.execute_batch("CREATE TRIGGER refuse_collection_activity BEFORE INSERT ON activity_summaries BEGIN SELECT RAISE(ABORT,'synthetic activity write failure'); END;").unwrap();
        assert_eq!(
            f.workspace.finish_profile_collection_read(completed).err(),
            Some(CommandError::ProviderCollectionSaveFailed)
        );
        assert_eq!(cached(&f.workspace, &id), original);
        assert_eq!(records(&mut f.workspace), before);
        assert!(
            f.workspace
                .cancel_restore_preview(&preview)
                .unwrap()
                .cancelled
        );
        assert_eq!(
            f.workspace
                .complete_profile_collection_read(&id, &request)
                .err(),
            Some(CommandError::ProviderCollectionNotFound)
        );
        connection
            .execute_batch("DROP TRIGGER refuse_collection_activity;")
            .unwrap();
        if reassociate {
            assert_eq!(
                f.workspace
                    .storage
                    .as_ref()
                    .unwrap()
                    .profile_binding(&id)
                    .unwrap()
                    .unwrap()
                    .binding_id,
                original.0.unwrap().binding.binding_id
            );
        } else {
            assert!(
                f.workspace
                    .storage
                    .as_ref()
                    .unwrap()
                    .profile_binding(&id)
                    .unwrap()
                    .is_none()
            );
        }
    }
}
