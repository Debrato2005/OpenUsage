use super::*;
use crate::keyvault::DatabaseKey;
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
        let executable = temporary.path().join("synthetic-login-provider.py");
        fs::write(
            &executable,
            include_bytes!("../../../../../tests/native/fixtures/synthetic-login-provider.py"),
        )
        .unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
        let mut workspace = Workspace::new(temporary.path().join("workspace")).unwrap();
        workspace.setup(Ok(&MemoryVault::default()), None).unwrap();
        workspace.create_profile("First synthetic profile").unwrap();
        let profiles = workspace
            .create_profile("Second synthetic profile")
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
    fn mode(&self, mode: &str) {
        fs::write(
            self.temporary
                .path()
                .join("synthetic-login-provider-config.json"),
            serde_json::to_vec(&serde_json::json!({"defaultMode":mode})).unwrap(),
        )
        .unwrap();
    }
    fn home(&self, id: &str) -> PathBuf {
        self.workspace.directory.join("profiles").join(id)
    }
    fn approve(&self) -> String {
        let candidate = self
            .workspace
            .inspect_provider_executable(self.executable.to_str().unwrap())
            .unwrap();
        self.workspace
            .approve_provider_executable(&candidate.candidate_id, true)
            .unwrap()
            .approval_id
    }
    fn observed_pid(&self, kind: &str, id: &str) -> libc::pid_t {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Ok(records) = fs::read_to_string(
                self.temporary
                    .path()
                    .join("synthetic-login-observations.jsonl"),
            ) {
                let mut pid = None;
                let mut observed = None;
                for record in records
                    .lines()
                    .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
                {
                    if record["kind"] == "start"
                        && record["environment"]["CODEX_HOME"] == self.home(id).to_str().unwrap()
                        && record["arguments"][0] == "app-server"
                    {
                        pid = record["pid"].as_i64();
                        observed = None;
                    }
                    if record["kind"] == kind && record["pid"].as_i64() == pid {
                        observed = pid;
                    }
                }
                if let Some(pid) = observed {
                    return pid.try_into().unwrap();
                }
            }
            assert!(
                Instant::now() < deadline,
                "The owned synthetic login did not reach its barrier."
            );
            std::thread::sleep(Duration::from_millis(10));
        }
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
fn selected_login_requires_consent_and_current_source_and_publishes_only_redacted_result() {
    let mut fixture = Fixture::new("early");
    let id = fixture.profiles[0].clone();
    let approval = fixture.approve();
    let revision = fixture
        .workspace
        .storage
        .as_mut()
        .unwrap()
        .restore_state()
        .unwrap()
        .revision;
    assert_eq!(
        fixture
            .workspace
            .begin_profile_login(&id, &approval, false)
            .err(),
        Some(CommandError::InvalidArguments)
    );
    assert_eq!(
        fixture
            .workspace
            .begin_profile_login(&id, "00000000-0000-4000-8000-000000000000", true)
            .err(),
        Some(CommandError::StaleProviderApproval)
    );
    assert!(
        !fixture
            .temporary
            .path()
            .join("synthetic-login-observations.jsonl")
            .exists()
    );
    let receipt = fixture
        .workspace
        .begin_profile_login(&id, &approval, true)
        .unwrap();
    assert_eq!(receipt.application_wait_limit_seconds, 900);
    assert_eq!(
        fixture
            .workspace
            .complete_profile_login(&id, &receipt.attempt_id)
            .err(),
        Some(CommandError::ProviderLoginPromptPending)
    );
    assert_eq!(
        fixture
            .workspace
            .await_profile_login_prompt(&fixture.profiles[1], &receipt.attempt_id)
            .err(),
        Some(CommandError::ProviderLoginNotFound)
    );
    let ready = fixture
        .workspace
        .await_profile_login_prompt(&id, &receipt.attempt_id)
        .unwrap()
        .wait()
        .unwrap();
    let prompt = fixture
        .workspace
        .finish_profile_login_prompt(ready)
        .unwrap();
    assert_eq!(
        prompt.verification_url,
        "https://auth.openai.com/codex/device"
    );
    assert_eq!(prompt.user_code, "SYNTH-1234");
    assert_eq!(prompt.attempt_id, receipt.attempt_id);
    let completed = fixture
        .workspace
        .complete_profile_login(&id, &receipt.attempt_id)
        .unwrap()
        .wait()
        .unwrap();
    let outcome = fixture.workspace.finish_profile_login(completed).unwrap();
    assert_eq!(
        outcome.diagnostic.authentication,
        provider::DiagnosticAuthentication::Chatgpt
    );
    assert_eq!(
        outcome.diagnostic.identity_confidence,
        provider::DiagnosticIdentity::Provisional
    );
    assert_eq!(outcome.diagnostic.monitoring, "stopped");
    assert!(!outcome.diagnostic.persisted);
    let json = serde_json::to_string(&outcome).unwrap();
    for forbidden in [
        "SYNTH-1234",
        "synthetic-device-attempt",
        "synthetic-login@example.invalid",
        "planType",
        "CODEX_HOME",
        "processGroup",
    ] {
        assert!(!json.contains(forbidden));
    }
    assert_eq!(
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
    assert_eq!(
        fixture
            .workspace
            .complete_profile_login(&id, &receipt.attempt_id)
            .err(),
        Some(CommandError::ProviderLoginNotFound)
    );
    assert!(
        fixture
            .home(&id)
            .join("synthetic-selected-login.json")
            .exists()
    );
    assert_eq!(
        fixture
            .workspace
            .open_profile_login_page(&id, &receipt.attempt_id, |_| panic!(
                "Completed attempt must not launch."
            ))
            .err(),
        Some(CommandError::ProviderLoginNotFound)
    );
    assert_eq!(
        fs::read_dir(fixture.home(&fixture.profiles[1]))
            .unwrap()
            .count(),
        1
    );
}

#[test]
fn exact_attempt_cancel_works_before_prompt_and_cannot_stop_newer_login_then_exit_reaps() {
    let mut fixture = Fixture::new("start_hang");
    let id = fixture.profiles[0].clone();
    let approval = fixture.approve();
    let first = fixture
        .workspace
        .begin_profile_login(&id, &approval, true)
        .unwrap();
    let pending = fixture
        .workspace
        .await_profile_login_prompt(&id, &first.attempt_id)
        .unwrap();
    let first_pid = fixture.observed_pid("start-pending", &id);
    assert!(
        fixture
            .workspace
            .provider
            .cancel_login(&id, &first.attempt_id)
            .unwrap()
            .cancelled
    );
    assert_eq!(
        pending.wait().err(),
        Some(CommandError::ProviderLoginCancelled)
    );
    gone(first_pid);
    fixture.mode("pending");
    let second = fixture
        .workspace
        .begin_profile_login(&id, &approval, true)
        .unwrap();
    let ready = fixture
        .workspace
        .await_profile_login_prompt(&id, &second.attempt_id)
        .unwrap()
        .wait()
        .unwrap();
    fixture
        .workspace
        .finish_profile_login_prompt(ready)
        .unwrap();
    let second_pid = fixture.observed_pid("challenge-emitted", &id);
    assert!(
        !fixture
            .workspace
            .provider
            .cancel_login(&id, &first.attempt_id)
            .unwrap()
            .cancelled
    );
    assert!(
        !fixture
            .workspace
            .provider
            .cancel_login(&fixture.profiles[1], &second.attempt_id)
            .unwrap()
            .cancelled
    );
    assert_eq!(
        fixture.workspace.provider.cancel(&id).err(),
        Some(CommandError::ProviderLoginInProgress)
    );
    assert_eq!(unsafe { libc::kill(second_pid, 0) }, 0);
    let completing = fixture
        .workspace
        .complete_profile_login(&id, &second.attempt_id)
        .unwrap();
    fixture
        .workspace
        .rename_profile(&id, "Renamed synthetic profile")
        .unwrap();
    assert_eq!(
        completing.wait().err(),
        Some(CommandError::ProviderLoginCancelled)
    );
    gone(second_pid);
    let third = fixture
        .workspace
        .begin_profile_login(&id, &approval, true)
        .unwrap();
    let ready = fixture
        .workspace
        .await_profile_login_prompt(&id, &third.attempt_id)
        .unwrap()
        .wait()
        .unwrap();
    fixture
        .workspace
        .finish_profile_login_prompt(ready)
        .unwrap();
    let third_pid = fixture.observed_pid("challenge-emitted", &id);
    drop(fixture.workspace);
    gone(third_pid);
}

#[test]
fn login_publications_refuse_reapproval_home_replacement_and_profile_mutation() {
    let mut fixture = Fixture::new("early");
    let id = fixture.profiles[0].clone();
    let approval = fixture.approve();
    let first = fixture
        .workspace
        .begin_profile_login(&id, &approval, true)
        .unwrap();
    let ready = fixture
        .workspace
        .await_profile_login_prompt(&id, &first.attempt_id)
        .unwrap()
        .wait()
        .unwrap();
    let next_approval = fixture.approve();
    assert_eq!(
        fixture.workspace.finish_profile_login_prompt(ready).err(),
        Some(CommandError::ProviderLoginCancelled)
    );
    let second = fixture
        .workspace
        .begin_profile_login(&id, &next_approval, true)
        .unwrap();
    let ready = fixture
        .workspace
        .await_profile_login_prompt(&id, &second.attempt_id)
        .unwrap()
        .wait()
        .unwrap();
    fixture
        .workspace
        .finish_profile_login_prompt(ready)
        .unwrap();
    let completed = fixture
        .workspace
        .complete_profile_login(&id, &second.attempt_id)
        .unwrap()
        .wait()
        .unwrap();
    fixture
        .workspace
        .rename_profile(&id, "Changed before publication")
        .unwrap();
    assert_eq!(
        fixture.workspace.finish_profile_login(completed).err(),
        Some(CommandError::ProviderLoginCancelled)
    );
    let third = fixture
        .workspace
        .begin_profile_login(&id, &next_approval, true)
        .unwrap();
    let ready = fixture
        .workspace
        .await_profile_login_prompt(&id, &third.attempt_id)
        .unwrap()
        .wait()
        .unwrap();
    let home = fixture.home(&id);
    let retained = home.with_extension("retained");
    fs::rename(&home, &retained).unwrap();
    prepare_directory(&home).unwrap();
    assert_eq!(
        fixture.workspace.finish_profile_login_prompt(ready).err(),
        Some(CommandError::ProviderHomeUnavailable)
    );
    assert_eq!(
        fs::read(retained.join("preserved-marker")).unwrap(),
        b"preserve"
    );
}

#[test]
fn browser_handoff_requires_selected_published_attempt_and_survives_launcher_failure() {
    let mut fixture = Fixture::new("pending");
    let id = fixture.profiles[0].clone();
    let other = fixture.profiles[1].clone();
    let mut locked = Workspace::new(fixture.temporary.path().join("uninitialized")).unwrap();
    assert_eq!(
        locked
            .open_profile_login_page(&id, "00000000-0000-4000-8000-000000000000", |_| panic!(
                "Locked workspace must not launch."
            ))
            .err(),
        Some(CommandError::WorkspaceLocked)
    );
    let approval = fixture.approve();
    let attempt = fixture
        .workspace
        .begin_profile_login(&id, &approval, true)
        .unwrap();
    assert_eq!(
        fixture
            .workspace
            .open_profile_login_page(&id, &attempt.attempt_id, |_| panic!(
                "Unpublished prompt must not launch."
            ))
            .err(),
        Some(CommandError::ProviderLoginPromptPending)
    );
    assert_eq!(
        fixture
            .workspace
            .open_profile_login_page(&other, &attempt.attempt_id, |_| panic!(
                "Other profile must not launch."
            ))
            .err(),
        Some(CommandError::ProviderLoginNotFound)
    );
    let ready = fixture
        .workspace
        .await_profile_login_prompt(&id, &attempt.attempt_id)
        .unwrap()
        .wait()
        .unwrap();
    assert_eq!(
        fixture
            .workspace
            .open_profile_login_page(&id, &attempt.attempt_id, |_| panic!(
                "Unpublished response must not launch."
            ))
            .err(),
        Some(CommandError::ProviderLoginPromptPending)
    );
    fixture
        .workspace
        .finish_profile_login_prompt(ready)
        .unwrap();
    let pid = fixture.observed_pid("challenge-emitted", &id);
    let mut dispatched = Vec::new();
    fixture
        .workspace
        .open_profile_login_page(&id, &attempt.attempt_id, |url| {
            dispatched.push(url);
            Ok(())
        })
        .unwrap();
    let completing = fixture
        .workspace
        .complete_profile_login(&id, &attempt.attempt_id)
        .unwrap();
    assert_eq!(
        fixture
            .workspace
            .open_profile_login_page(&id, &attempt.attempt_id, |url| {
                dispatched.push(url);
                Err(CommandError::ProviderBrowserUnavailable)
            })
            .err(),
        Some(CommandError::ProviderBrowserUnavailable)
    );
    fixture
        .workspace
        .open_profile_login_page(&id, &attempt.attempt_id, |url| {
            dispatched.push(url);
            Ok(())
        })
        .unwrap();
    assert_eq!(dispatched, ["https://auth.openai.com/codex/device"; 3]);
    assert_eq!(unsafe { libc::kill(pid, 0) }, 0);
    assert!(
        !fixture
            .home(&other)
            .join("synthetic-selected-login.json")
            .exists()
    );
    fixture
        .workspace
        .provider
        .cancel_login(&id, &attempt.attempt_id)
        .unwrap();
    assert_eq!(
        completing.wait().err(),
        Some(CommandError::ProviderLoginCancelled)
    );
    gone(pid);
    assert_eq!(
        fixture
            .workspace
            .open_profile_login_page(&id, &attempt.attempt_id, |_| panic!(
                "Ended attempt must not launch."
            ))
            .err(),
        Some(CommandError::ProviderLoginNotFound)
    );
}

#[test]
fn browser_handoff_refuses_changed_profile_home_and_source_bindings() {
    for fault in ["archived", "home", "approval", "quiesce"] {
        let mut fixture = Fixture::new("pending");
        let id = fixture.profiles[0].clone();
        let approval = fixture.approve();
        let attempt = fixture
            .workspace
            .begin_profile_login(&id, &approval, true)
            .unwrap();
        let ready = fixture
            .workspace
            .await_profile_login_prompt(&id, &attempt.attempt_id)
            .unwrap()
            .wait()
            .unwrap();
        fixture
            .workspace
            .finish_profile_login_prompt(ready)
            .unwrap();
        let completing = fixture
            .workspace
            .complete_profile_login(&id, &attempt.attempt_id)
            .unwrap();
        let pid = fixture.observed_pid("challenge-emitted", &id);
        match fault {
            "archived" => {
                fixture.workspace.set_profile_archived(&id, true).unwrap();
            }
            "home" => {
                fs::rename(
                    fixture.home(&id),
                    fixture.home(&id).with_extension("original"),
                )
                .unwrap();
                fs::create_dir(fixture.home(&id)).unwrap();
                fs::set_permissions(fixture.home(&id), fs::Permissions::from_mode(0o700)).unwrap();
            }
            "approval" => {
                fixture.approve();
            }
            "quiesce" => {
                fixture.workspace.provider.quiesce().unwrap();
            }
            _ => unreachable!(),
        }
        assert!(
            fixture
                .workspace
                .open_profile_login_page(&id, &attempt.attempt_id, |_| panic!(
                    "Changed binding must not launch."
                ))
                .is_err(),
            "{fault}"
        );
        let cancelled = fixture
            .workspace
            .provider
            .cancel_login(&id, &attempt.attempt_id);
        let completed = completing.wait();
        if fault == "home" {
            // Renaming the runtime parent makes TempDir::close fail at its original path.
            assert_eq!(cancelled.err(), Some(CommandError::ProviderCleanupFailed));
            assert_eq!(completed.err(), Some(CommandError::ProviderCleanupFailed));
            assert_eq!(
                fs::read(
                    fixture
                        .home(&id)
                        .with_extension("original")
                        .join("preserved-marker")
                )
                .unwrap(),
                b"preserve"
            );
            assert!(fs::read_dir(fixture.home(&id)).unwrap().next().is_none());
        } else {
            cancelled.unwrap_or_else(|error| panic!("{fault}: {error:?}"));
            assert!(completed.is_err(), "{fault}");
        }
        gone(pid);
    }
}
