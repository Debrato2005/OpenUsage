//! Runtime eligibility and actual supervised scheduled persistence; native acceptance uses real time.
use super::collection_tests::Fixture;
use super::*;
use std::time::{Duration, Instant};

fn mode(f: &Fixture, id: &str, mode: &str) {
    fs::write(
        f.home(id).join("synthetic-collection-config.json"),
        serde_json::to_vec(&serde_json::json!({"mode":mode,"requireLogin":true})).unwrap(),
    )
    .unwrap();
}
fn completed_login(f: &mut Fixture, id: &str, approval: &str) -> provider::CompletedLogin {
    mode(f, id, "success");
    let receipt = f.workspace.begin_profile_login(id, approval, true).unwrap();
    let prompt = f
        .workspace
        .await_profile_login_prompt(id, &receipt.attempt_id)
        .unwrap()
        .wait()
        .unwrap();
    f.workspace.finish_profile_login_prompt(prompt).unwrap();
    f.workspace
        .complete_profile_login(id, &receipt.attempt_id)
        .unwrap()
        .wait()
        .unwrap()
}
fn login(f: &mut Fixture, id: &str, approval: &str) {
    let completed = completed_login(f, id, approval);
    assert!(
        f.workspace
            .finish_profile_login(completed)
            .unwrap()
            .diagnostic
            .persisted
    );
}
fn failed_manual(f: &mut Fixture, id: &str, approval: &str, kind: &str) -> (String, CommandError) {
    mode(f, id, "signed_out");
    provider::tests::elapse_refresh(&f.workspace.provider, id);
    match kind {
        "quota" => {
            let read = f.workspace.begin_profile_quota_read(id, approval).unwrap();
            let error = f
                .workspace
                .complete_profile_quota_read(id, &read.request_id)
                .unwrap()
                .wait()
                .err()
                .unwrap();
            (read.request_id, error)
        }
        "activity" => {
            let read = f
                .workspace
                .begin_profile_activity_read(id, approval)
                .unwrap();
            let error = f
                .workspace
                .complete_profile_activity_read(id, &read.request_id)
                .unwrap()
                .wait()
                .err()
                .unwrap();
            (read.request_id, error)
        }
        "collection" => {
            let read = f
                .workspace
                .begin_profile_collection_read(id, approval)
                .unwrap();
            let error = f
                .workspace
                .complete_profile_collection_read(id, &read.request_id)
                .unwrap()
                .wait()
                .err()
                .unwrap();
            (read.request_id, error)
        }
        _ => unreachable!(),
    }
}
fn status(workspace: &mut Workspace, id: &str) -> serde_json::Value {
    serde_json::to_value(workspace.load_monitoring().unwrap()).unwrap()["profiles"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["profileId"] == id)
        .unwrap()
        .clone()
}
fn publish(workspace: &mut Workspace, job: monitoring::ScheduledCollection) {
    let result = job.pending.wait();
    workspace.finish_scheduled_collection(&job.profile_id, &job.request_id, job.revision, result);
}
fn child(f: &Fixture, id: &str, barrier: &str, previous: Option<libc::pid_t>) -> libc::pid_t {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let records =
            fs::read_to_string(f.home(id).join("synthetic-collection-observations.jsonl"))
                .unwrap_or_default();
        let mut pid = None;
        let mut reached = None;
        for value in records
            .lines()
            .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        {
            if value["kind"] == "start" && value["arguments"][0] == "app-server" {
                pid = value["pid"].as_i64();
                reached = None;
            }
            if value["kind"] == barrier && value["pid"].as_i64() == pid {
                reached = pid;
            }
        }
        if let Some(pid) = reached {
            let pid = pid.try_into().unwrap();
            if Some(pid) != previous {
                return pid;
            }
        }
        assert!(
            Instant::now() < deadline,
            "Synthetic scheduled read did not reach {barrier}"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn monitoring_requires_committed_runtime_login_then_saves_both_and_keeps_manual_admission_independent()
 {
    let mut f = Fixture::new("success");
    let id = f.profiles[0].clone();
    let other = f.profiles[1].clone();
    let approval = f.approve();
    f.save(&id, &approval);
    f.workspace
        .storage
        .as_mut()
        .unwrap()
        .record_profile_connection(&id, &id, ProfileConnectionState::Connected)
        .unwrap();
    assert_eq!(
        f.workspace.start_profile_monitoring(&id, &approval).err(),
        Some(CommandError::MonitoringNotEligible)
    );
    assert_eq!(status(&mut f.workspace, &id)["eligible"], false);
    provider::tests::elapse_refresh(&f.workspace.provider, &id);
    login(&mut f, &id, &approval);
    assert_eq!(status(&mut f.workspace, &id)["state"], "stopped");
    assert_eq!(status(&mut f.workspace, &id)["eligible"], true);
    let previous = f.workspace.load_monitoring().unwrap().reading_revision;
    let started = f
        .workspace
        .start_profile_monitoring(&id, &approval)
        .unwrap();
    assert_eq!(started.state, MonitoringState::Running);
    let job = f.workspace.next_scheduled_collection().unwrap();
    assert_eq!(status(&mut f.workspace, &id)["readingActive"], true);
    publish(&mut f.workspace, job);
    let after = f.workspace.load_monitoring().unwrap();
    assert_ne!(after.reading_revision, previous);
    assert!(
        status(&mut f.workspace, &id)["nextReadInSeconds"]
            .as_u64()
            .is_some_and(|s| (295..=300).contains(&s))
    );
    let storage = f.workspace.storage.as_ref().unwrap();
    let q = storage.current_quota(&id).unwrap().unwrap();
    let a = storage.current_activity(&id).unwrap().unwrap();
    assert_eq!(q.binding, a.binding);
    assert_ne!(q.commit.collection_run_id, a.commit.collection_run_id);
    assert_eq!(
        a.summary.lifetime_tokens.as_deref(),
        Some("9007199254740993")
    );
    assert!(storage.current_quota(&other).unwrap().is_none());
    assert!(storage.current_activity(&other).unwrap().is_none());
    let manual = f
        .workspace
        .begin_profile_collection_read(&id, &approval)
        .unwrap();
    assert_eq!(manual.monitoring, "running");
    let completed = f
        .workspace
        .complete_profile_collection_read(&id, &manual.request_id)
        .unwrap()
        .wait()
        .unwrap();
    f.workspace
        .finish_profile_collection_read(completed)
        .unwrap();
    assert_eq!(
        f.workspace.load_monitoring().unwrap().reading_revision,
        after.reading_revision
    );
    assert_eq!(
        f.workspace
            .begin_profile_collection_read(&id, &approval)
            .err(),
        Some(CommandError::ProviderRefreshCooldown)
    );
    assert_eq!(f.workspace.bootstrap(Ok(&f.vault)).monitoring, "running");
    let quota = f
        .workspace
        .storage
        .as_ref()
        .unwrap()
        .current_quota(&id)
        .unwrap();
    let activity = f
        .workspace
        .storage
        .as_ref()
        .unwrap()
        .current_activity(&id)
        .unwrap();
    mode(&f, &id, "both_unsupported");
    let job = f
        .workspace
        .next_scheduled_collection_at(Instant::now() + Duration::from_secs(301))
        .unwrap();
    publish(&mut f.workspace, job);
    let paused = status(&mut f.workspace, &id);
    assert_eq!(paused["state"], "paused");
    assert_eq!(paused["issue"], "unsupported");
    assert_eq!(paused["eligible"], true);
    assert!(paused["nextReadInSeconds"].is_null());
    assert_eq!(
        f.workspace.load_monitoring().unwrap().reading_revision,
        after.reading_revision
    );
    assert_eq!(
        f.workspace
            .storage
            .as_ref()
            .unwrap()
            .current_quota(&id)
            .unwrap(),
        quota
    );
    assert_eq!(
        f.workspace
            .storage
            .as_ref()
            .unwrap()
            .current_activity(&id)
            .unwrap(),
        activity
    );
}

#[test]
fn monitoring_pause_after_wait_prevents_publication_and_restart_never_recovers_the_grant() {
    let mut f = Fixture::new("success");
    let id = f.profiles[0].clone();
    let approval = f.approve();
    login(&mut f, &id, &approval);
    f.workspace
        .start_profile_monitoring(&id, &approval)
        .unwrap();
    let job = f.workspace.next_scheduled_collection().unwrap();
    let completed = job.pending.wait().unwrap();
    let revision = f.workspace.load_monitoring().unwrap().reading_revision;
    assert_eq!(
        f.workspace.pause_profile_monitoring(&id).unwrap(),
        Some(job.request_id.clone())
    );
    assert_eq!(status(&mut f.workspace, &id)["readingActive"], true);
    f.workspace
        .provider
        .cancel_collection(&id, &job.request_id)
        .unwrap();
    f.workspace.cancelled_scheduled_read(&id, &job.request_id);
    f.workspace
        .finish_scheduled_collection(&id, &job.request_id, job.revision, Ok(completed));
    assert_eq!(
        f.workspace.load_monitoring().unwrap().reading_revision,
        revision
    );
    assert!(
        f.workspace
            .storage
            .as_ref()
            .unwrap()
            .current_quota(&id)
            .unwrap()
            .is_none()
    );
    assert_eq!(status(&mut f.workspace, &id)["state"], "paused");
    assert_eq!(status(&mut f.workspace, &id)["eligible"], true);
    assert_eq!(f.workspace.pause_profile_monitoring(&id).unwrap(), None);
    assert!(fs::metadata(f.home(&id).join("synthetic-monitored-login-completed")).is_ok());
    let directory = f.workspace.directory.clone();
    let executable = f.executable.clone();
    drop(f.workspace);
    let mut reopened = Workspace::new(directory).unwrap();
    assert_eq!(reopened.bootstrap(Ok(&f.vault)).monitoring, "stopped");
    let candidate = reopened
        .inspect_provider_executable(executable.to_str().unwrap())
        .unwrap();
    let approval = reopened
        .approve_provider_executable(&candidate.candidate_id, true)
        .unwrap();
    assert_eq!(
        reopened
            .start_profile_monitoring(&id, &approval.approval_id)
            .err(),
        Some(CommandError::MonitoringNotEligible)
    );
    assert_eq!(status(&mut reopened, &id)["eligible"], false);
}

#[test]
fn monitoring_deadlines_stagger_and_skip_missed_intervals_without_replay() {
    let mut f = Fixture::new("success");
    let id = f.profiles[0].clone();
    let other = f.profiles[1].clone();
    let approval = f.approve();
    login(&mut f, &id, &approval);
    login(&mut f, &other, &approval);
    f.workspace
        .start_profile_monitoring(&id, &approval)
        .unwrap();
    f.workspace
        .start_profile_monitoring(&other, &approval)
        .unwrap();
    let at = Instant::now() + Duration::from_secs(1);
    let first = f.workspace.next_scheduled_collection_at(at).unwrap();
    assert_eq!(first.profile_id, id);
    publish(&mut f.workspace, first);
    assert!(
        f.workspace
            .next_scheduled_collection_at(at + Duration::from_secs(4))
            .is_none()
    );
    let second = f
        .workspace
        .next_scheduled_collection_at(at + Duration::from_secs(5))
        .unwrap();
    assert_eq!(second.profile_id, other);
    publish(&mut f.workspace, second);
    assert!(
        f.workspace
            .next_scheduled_collection_at(at + Duration::from_secs(299))
            .is_none()
    );
    let catchup = at + Duration::from_secs(3600);
    let job = f.workspace.next_scheduled_collection_at(catchup).unwrap();
    assert_eq!(job.profile_id, id);
    publish(&mut f.workspace, job);
    assert!(
        f.workspace
            .next_scheduled_collection_at(catchup + Duration::from_secs(4))
            .is_none()
    );
    let job = f
        .workspace
        .next_scheduled_collection_at(catchup + Duration::from_secs(5))
        .unwrap();
    assert_eq!(job.profile_id, other);
    publish(&mut f.workspace, job);
    assert!(
        f.workspace
            .next_scheduled_collection_at(catchup + Duration::from_secs(299))
            .is_none()
    );
    let job = f
        .workspace
        .next_scheduled_collection_at(catchup + Duration::from_secs(300))
        .unwrap();
    assert_eq!(job.profile_id, id);
    publish(&mut f.workspace, job);
}

#[test]
fn monitoring_auth_failure_conflict_and_source_or_home_boundaries_revoke_without_replacing_metrics()
{
    for fault in [
        "signed_out",
        "stable_changed_identity",
        "reapproval",
        "archive",
        "home",
        "restore",
        "new_login",
    ] {
        let mut f = Fixture::new("success");
        let id = f.profiles[0].clone();
        let approval = f.approve();
        login(&mut f, &id, &approval);
        f.workspace
            .start_profile_monitoring(&id, &approval)
            .unwrap();
        let job = f.workspace.next_scheduled_collection().unwrap();
        publish(&mut f.workspace, job);
        let saved = f
            .workspace
            .storage
            .as_ref()
            .unwrap()
            .current_activity(&id)
            .unwrap();
        let mut moved = None;
        match fault {
            "signed_out" | "stable_changed_identity" => {
                mode(&f, &id, fault);
                let job = f
                    .workspace
                    .next_scheduled_collection_at(Instant::now() + Duration::from_secs(301))
                    .unwrap();
                publish(&mut f.workspace, job);
            }
            "reapproval" => {
                f.approve();
            }
            "archive" => {
                f.workspace.set_profile_archived(&id, true).unwrap();
            }
            "home" => {
                let old = f.temporary.path().join("original-home");
                fs::rename(f.home(&id), &old).unwrap();
                fs::create_dir(f.home(&id)).unwrap();
                fs::set_permissions(
                    f.home(&id),
                    std::os::unix::fs::PermissionsExt::from_mode(0o700),
                )
                .unwrap();
                moved = Some(old);
            }
            "restore" => {
                let preview = f.preview();
                f.workspace.apply_restore(&preview, true).unwrap();
            }
            "new_login" => {
                mode(&f, &id, "login_failed");
                let receipt = f
                    .workspace
                    .begin_profile_login(&id, &approval, true)
                    .unwrap();
                let prompt = f
                    .workspace
                    .await_profile_login_prompt(&id, &receipt.attempt_id)
                    .unwrap()
                    .wait()
                    .unwrap();
                f.workspace.finish_profile_login_prompt(prompt).unwrap();
                assert!(
                    f.workspace
                        .complete_profile_login(&id, &receipt.attempt_id)
                        .unwrap()
                        .wait()
                        .is_err()
                );
            }
            _ => unreachable!(),
        }
        if fault != "archive" {
            assert_eq!(status(&mut f.workspace, &id)["eligible"], false, "{fault}");
            assert_eq!(
                f.workspace.start_profile_monitoring(&id, &approval).err(),
                Some(CommandError::MonitoringNotEligible),
                "{fault}"
            );
        }
        assert_eq!(
            f.workspace
                .storage
                .as_ref()
                .unwrap()
                .current_activity(&id)
                .unwrap(),
            saved,
            "{fault}"
        );
        if fault == "signed_out" {
            assert_eq!(status(&mut f.workspace, &id)["issue"], "sign_in_required");
        }
        if fault == "stable_changed_identity" {
            assert_eq!(status(&mut f.workspace, &id)["issue"], "account_changed");
        }
        if let Some(old) = moved {
            fs::remove_dir(f.home(&id)).unwrap();
            fs::rename(old, f.home(&id)).unwrap();
        }
    }
}

#[test]
fn monitoring_owned_timer_shutdown_reaps_a_pending_partial_collection_without_commit() {
    let mut f = Fixture::new("success");
    let id = f.profiles[0].clone();
    let approval = f.approve();
    login(&mut f, &id, &approval);
    mode(&f, &id, "activity_hang");
    f.workspace
        .start_profile_monitoring(&id, &approval)
        .unwrap();
    let directory = f.home(&id);
    let provider = f.workspace.provider.clone();
    let workspace = std::sync::Arc::new(std::sync::Mutex::new(Some(f.workspace)));
    let worker = crate::monitoring::MonitoringWorker::start(workspace.clone(), provider);
    let deadline = Instant::now() + Duration::from_secs(5);
    let pid = loop {
        let records = fs::read_to_string(directory.join("synthetic-collection-observations.jsonl"))
            .unwrap_or_default();
        let pid = records
            .lines()
            .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
            .rfind(|v| v["kind"] == "activity-pending")
            .and_then(|v| v["pid"].as_i64());
        if let Some(pid) = pid {
            break pid as libc::pid_t;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(10));
    };
    worker.close().unwrap();
    assert_eq!(unsafe { libc::kill(pid, 0) }, -1);
    assert_eq!(unsafe { libc::killpg(pid, 0) }, -1);
    let mut state = workspace.lock().unwrap();
    let workspace = state.as_mut().unwrap();
    assert_eq!(status(workspace, &id)["readingActive"], false);
    assert!(
        workspace
            .storage
            .as_ref()
            .unwrap()
            .current_quota(&id)
            .unwrap()
            .is_none()
    );
    assert!(
        workspace
            .storage
            .as_ref()
            .unwrap()
            .current_activity(&id)
            .unwrap()
            .is_none()
    );
}

#[test]
fn monitoring_transient_exit_keeps_eligibility_and_accepted_values_while_honoring_shared_backoff() {
    let mut f = Fixture::new("success");
    let id = f.profiles[0].clone();
    let approval = f.approve();
    login(&mut f, &id, &approval);
    f.workspace
        .start_profile_monitoring(&id, &approval)
        .unwrap();
    let job = f.workspace.next_scheduled_collection().unwrap();
    publish(&mut f.workspace, job);
    let saved = f
        .workspace
        .storage
        .as_ref()
        .unwrap()
        .current_activity(&id)
        .unwrap();
    mode(&f, &id, "activity_hang");
    let previous = fs::read_to_string(f.home(&id).join("synthetic-child-pid"))
        .unwrap()
        .parse()
        .unwrap();
    let at = Instant::now() + Duration::from_secs(301);
    let job = f.workspace.next_scheduled_collection_at(at).unwrap();
    let pid = child(&f, &id, "activity-pending", Some(previous));
    assert_eq!(unsafe { libc::kill(pid, libc::SIGKILL) }, 0);
    publish(&mut f.workspace, job);
    assert_eq!(status(&mut f.workspace, &id)["eligible"], true);
    assert_eq!(status(&mut f.workspace, &id)["state"], "running");
    assert_eq!(
        f.workspace
            .storage
            .as_ref()
            .unwrap()
            .current_activity(&id)
            .unwrap(),
        saved
    );
    assert!(
        f.workspace
            .provider
            .scheduled_retry_after_seconds(&id)
            .unwrap()
            > 0
    );
    assert!(
        f.workspace
            .next_scheduled_collection_at(at + Duration::from_secs(301))
            .is_none()
    );
    assert_eq!(
        f.workspace
            .begin_profile_collection_read(&id, &approval)
            .err(),
        Some(CommandError::ProviderRefreshBackoff)
    );
}

#[test]
fn monitoring_unrelated_profile_mutations_preserve_grants_and_an_owned_scheduled_commit() {
    let mut f = Fixture::new("success");
    let id = f.profiles[0].clone();
    let other = f.profiles[1].clone();
    let approval = f.approve();
    for profile in [&id, &other] {
        login(&mut f, profile, &approval);
        f.workspace
            .start_profile_monitoring(profile, &approval)
            .unwrap();
    }
    let job = f.workspace.next_scheduled_collection().unwrap();
    assert_eq!(job.profile_id, id);
    f.workspace
        .create_profile("New independent profile")
        .unwrap();
    f.workspace
        .rename_profile(&other, "Renamed independently")
        .unwrap();
    let connection = f.connection();
    connection.execute_batch("CREATE TRIGGER refuse_profile_insert BEFORE INSERT ON profiles BEGIN SELECT RAISE(ABORT,'synthetic profile write failure'); END; CREATE TRIGGER refuse_profile_update BEFORE UPDATE ON profiles BEGIN SELECT RAISE(ABORT,'synthetic profile write failure'); END;").unwrap();
    assert!(
        f.workspace
            .create_profile("Failed independent create")
            .is_err()
    );
    assert!(
        f.workspace
            .rename_profile(&other, "Failed independent rename")
            .is_err()
    );
    assert!(f.workspace.set_profile_archived(&other, true).is_err());
    for profile in [&id, &other] {
        let current = status(&mut f.workspace, profile);
        assert_eq!(current["state"], "running");
        assert_eq!(current["eligible"], true);
    }
    connection
        .execute_batch("DROP TRIGGER refuse_profile_insert; DROP TRIGGER refuse_profile_update;")
        .unwrap();
    f.workspace.set_profile_archived(&other, true).unwrap();
    let current = status(&mut f.workspace, &id);
    assert_eq!(current["state"], "running");
    assert_eq!(current["eligible"], true);
    assert_eq!(current["readingActive"], true);
    publish(&mut f.workspace, job);
    let storage = f.workspace.storage.as_ref().unwrap();
    assert!(storage.current_quota(&id).unwrap().is_some());
    assert!(storage.current_activity(&id).unwrap().is_some());
    f.workspace.set_profile_archived(&other, false).unwrap();
    assert_eq!(status(&mut f.workspace, &other)["eligible"], false);
    assert_eq!(status(&mut f.workspace, &id)["state"], "running");
}

#[test]
fn monitoring_old_manual_auth_failures_cannot_revoke_a_new_login_but_current_failures_do() {
    for kind in ["quota", "activity", "collection"] {
        let mut f = Fixture::new("success");
        let id = f.profiles[0].clone();
        let approval = f.approve();
        login(&mut f, &id, &approval);
        f.workspace
            .start_profile_monitoring(&id, &approval)
            .unwrap();
        let (old_request, old_error) = failed_manual(&mut f, &id, &approval, kind);
        assert_eq!(old_error, CommandError::ProviderAuthenticationRequired);
        // The provider wait returned before the Workspace completion callback acquired its lock.
        login(&mut f, &id, &approval);
        f.workspace
            .start_profile_monitoring(&id, &approval)
            .unwrap();
        f.workspace
            .note_monitoring_read_error(&id, &old_request, old_error);
        let current = status(&mut f.workspace, &id);
        assert_eq!(
            current["state"], "running",
            "Stale {kind} failure changed the new login"
        );
        assert_eq!(current["eligible"], true);
        assert!(current["issue"].is_null());
        let (request, error) = failed_manual(&mut f, &id, &approval, kind);
        f.workspace.note_monitoring_read_error(&id, &request, error);
        let current = status(&mut f.workspace, &id);
        assert_eq!(current["state"], "paused");
        assert_eq!(current["eligible"], false);
        assert_eq!(current["issue"], "sign_in_required");
    }
}

#[test]
fn monitoring_archive_unarchive_fences_late_login_and_diagnostics_without_revoking_other_profiles()
{
    let mut f = Fixture::new("success");
    let id = f.profiles[0].clone();
    let other = f.profiles[1].clone();
    let approval = f.approve();
    login(&mut f, &other, &approval);
    f.workspace
        .start_profile_monitoring(&other, &approval)
        .unwrap();
    let old_login = completed_login(&mut f, &id, &approval);
    f.workspace.set_profile_archived(&id, true).unwrap();
    f.workspace.set_profile_archived(&id, false).unwrap();
    assert_eq!(
        f.workspace.finish_profile_login(old_login).err(),
        Some(CommandError::ProviderLoginCancelled)
    );
    assert_eq!(status(&mut f.workspace, &id)["eligible"], false);
    login(&mut f, &id, &approval);
    mode(&f, &id, "signed_out");
    let diagnostic = f
        .workspace
        .start_profile_check(&id)
        .unwrap()
        .wait()
        .unwrap();
    f.workspace.set_profile_archived(&id, true).unwrap();
    f.workspace.set_profile_archived(&id, false).unwrap();
    login(&mut f, &id, &approval);
    assert_eq!(
        f.workspace.finish_profile_check(diagnostic).err(),
        Some(CommandError::ProviderCheckCancelled)
    );
    assert_eq!(status(&mut f.workspace, &id)["eligible"], true);
    let current = status(&mut f.workspace, &other);
    assert_eq!(current["eligible"], true);
    assert_eq!(current["state"], "running");
    mode(&f, &id, "signed_out");
    let diagnostic = f
        .workspace
        .start_profile_check(&id)
        .unwrap()
        .wait()
        .unwrap();
    login(&mut f, &id, &approval);
    assert_eq!(
        f.workspace.finish_profile_check(diagnostic).err(),
        Some(CommandError::ProviderCheckCancelled)
    );
    assert_eq!(status(&mut f.workspace, &id)["eligible"], true);
}

#[test]
fn monitoring_busy_due_profile_does_not_starve_another_eligible_profile() {
    let mut f = Fixture::new("success");
    let id = f.profiles[0].clone();
    let other = f.profiles[1].clone();
    let approval = f.approve();
    login(&mut f, &id, &approval);
    login(&mut f, &other, &approval);
    mode(&f, &id, "activity_hang");
    let manual = f
        .workspace
        .begin_profile_collection_read(&id, &approval)
        .unwrap();
    let pending = f
        .workspace
        .complete_profile_collection_read(&id, &manual.request_id)
        .unwrap();
    child(&f, &id, "activity-pending", None);
    f.workspace
        .start_profile_monitoring(&id, &approval)
        .unwrap();
    f.workspace
        .start_profile_monitoring(&other, &approval)
        .unwrap();
    let now = Instant::now();
    assert!(f.workspace.next_scheduled_collection_at(now).is_none());
    let healthy = f
        .workspace
        .next_scheduled_collection_at(now + Duration::from_millis(250))
        .unwrap();
    assert_eq!(healthy.profile_id, other);
    publish(&mut f.workspace, healthy);
    assert!(
        f.workspace
            .storage
            .as_ref()
            .unwrap()
            .current_activity(&other)
            .unwrap()
            .is_some()
    );
    assert!(
        f.workspace
            .storage
            .as_ref()
            .unwrap()
            .current_activity(&id)
            .unwrap()
            .is_none()
    );
    f.workspace
        .provider
        .cancel_collection(&id, &manual.request_id)
        .unwrap();
    assert!(pending.wait().is_err());
}

#[test]
fn monitoring_cancelled_restore_preview_preserves_pending_read_and_valid_restore_reaps_it() {
    let mut f = Fixture::new("success");
    let id = f.profiles[0].clone();
    let approval = f.approve();
    login(&mut f, &id, &approval);
    f.workspace
        .start_profile_monitoring(&id, &approval)
        .unwrap();
    let seed = f.workspace.next_scheduled_collection().unwrap();
    publish(&mut f.workspace, seed);
    let quota = f
        .workspace
        .storage
        .as_ref()
        .unwrap()
        .current_quota(&id)
        .unwrap();
    let activity = f
        .workspace
        .storage
        .as_ref()
        .unwrap()
        .current_activity(&id)
        .unwrap();
    let revision = f.workspace.load_monitoring().unwrap().reading_revision;
    let previous = fs::read_to_string(f.home(&id).join("synthetic-child-pid"))
        .unwrap()
        .parse()
        .unwrap();
    mode(&f, &id, "activity_hang");
    let job = f
        .workspace
        .next_scheduled_collection_at(Instant::now() + Duration::from_secs(301))
        .unwrap();
    let pid = child(&f, &id, "activity-pending", Some(previous));
    let cancelled = f.preview();
    assert!(
        f.workspace
            .cancel_restore_preview(&cancelled)
            .unwrap()
            .cancelled
    );
    assert_eq!(
        f.workspace.apply_restore(&cancelled, true).err(),
        Some(CommandError::StaleRestorePreview)
    );
    let current = status(&mut f.workspace, &id);
    assert_eq!(current["state"], "running");
    assert_eq!(current["eligible"], true);
    assert_eq!(current["readingActive"], true);
    assert_eq!(unsafe { libc::kill(pid, 0) }, 0);
    assert_eq!(
        f.workspace
            .storage
            .as_ref()
            .unwrap()
            .current_quota(&id)
            .unwrap(),
        quota
    );
    assert_eq!(
        f.workspace
            .storage
            .as_ref()
            .unwrap()
            .current_activity(&id)
            .unwrap(),
        activity
    );
    assert_eq!(
        f.workspace.load_monitoring().unwrap().reading_revision,
        revision
    );

    let preview = f.preview();
    let restored = f.workspace.apply_restore(&preview, true).unwrap();
    assert_eq!(restored.monitoring, "stopped");
    assert!(restored.authentication_required);
    assert_eq!(unsafe { libc::kill(pid, 0) }, -1);
    assert_eq!(unsafe { libc::killpg(pid, 0) }, -1);
    let result = job.pending.wait();
    assert!(result.is_err());
    f.workspace
        .finish_scheduled_collection(&id, &job.request_id, job.revision, result);
    let current = status(&mut f.workspace, &id);
    assert_eq!(current["state"], "stopped");
    assert_eq!(current["eligible"], false);
    assert_eq!(current["readingActive"], false);
    assert_eq!(
        f.workspace
            .storage
            .as_ref()
            .unwrap()
            .current_quota(&id)
            .unwrap(),
        quota
    );
    assert_eq!(
        f.workspace
            .storage
            .as_ref()
            .unwrap()
            .current_activity(&id)
            .unwrap(),
        activity
    );
    assert_eq!(
        f.workspace.load_monitoring().unwrap().reading_revision,
        revision
    );
    assert_eq!(
        f.workspace.start_profile_monitoring(&id, &approval).err(),
        Some(CommandError::MonitoringNotEligible)
    );
}
