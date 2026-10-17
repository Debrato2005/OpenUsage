//! Bounded provider checks with explicit Rust approval and owned cancellation.
//! No login, inference, polling, or native application shutdown hook is implemented here.
//!
//! Process ownership follows the standard library's process-group and child-wait contracts:
//! <https://doc.rust-lang.org/std/os/unix/process/trait.CommandExt.html#tymethod.process_group>
//! <https://doc.rust-lang.org/std/process/struct.Child.html#method.wait>

use crate::session::{ProviderProbe, SessionError};
use std::{fmt, path::Path};

#[cfg(unix)]
use crate::session::unix::{ProcessControl, Timeouts, absolute_normal_path};
#[cfg(unix)]
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
    },
    thread::JoinHandle,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SupervisorError {
    InvalidProfileId,
    InvalidHomeReference,
    InvalidApproval,
    DuplicateProfile,
    CapacityReached,
    HomeInUse,
    HomeChanged,
    Closed,
    Cancelled,
    WorkerFailed,
    Session(SessionError),
}

impl From<SessionError> for SupervisorError {
    fn from(error: SessionError) -> Self {
        if error == SessionError::Cancelled {
            Self::Cancelled
        } else {
            Self::Session(error)
        }
    }
}

impl fmt::Display for SupervisorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidProfileId => "Use a canonical lowercase profile UUID.",
            Self::InvalidHomeReference => {
                "The managed home does not match its approved UUID reference."
            }
            Self::InvalidApproval => {
                "Supply an explicit review reference for the approved executable."
            }
            Self::DuplicateProfile => "This profile already has an active provider check.",
            Self::CapacityReached => "Two provider checks are already active.",
            Self::HomeInUse => "Another profile owns this managed home.",
            Self::HomeChanged => "The profile's approved managed home changed.",
            Self::Closed => "The provider supervisor is closed.",
            Self::Cancelled => "The provider check was cancelled.",
            Self::WorkerFailed => "The provider worker could not complete safely.",
            Self::Session(_) => "The provider session could not complete safely.",
        })
    }
}

impl std::error::Error for SupervisorError {}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ProfileId(String);

impl ProfileId {
    pub fn parse(value: &str) -> Result<Self, SupervisorError> {
        if value.len() != 36
            || !value.bytes().enumerate().all(|(index, byte)| {
                if [8, 13, 18, 23].contains(&index) {
                    byte == b'-'
                } else {
                    byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)
                }
            })
        {
            return Err(SupervisorError::InvalidProfileId);
        }
        Ok(Self(value.into()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(unix)]
#[derive(Clone, Eq, PartialEq)]
struct ExecutableIdentity {
    device: u64,
    inode: u64,
    length: u64,
    mode: u32,
    uid: u32,
    modified: (i64, i64),
    changed: (i64, i64),
}

#[cfg(unix)]
fn executable_identity(path: &Path) -> Result<ExecutableIdentity, SessionError> {
    use std::os::unix::fs::MetadataExt;
    if !absolute_normal_path(path) {
        return Err(SessionError::InvalidExecutable);
    }
    let metadata = std::fs::symlink_metadata(path).map_err(|_| SessionError::InvalidExecutable)?;
    if path.canonicalize().ok().as_deref() != Some(path)
        || !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.mode() & 0o111 == 0
        || metadata.mode() & 0o6022 != 0
    {
        return Err(SessionError::InvalidExecutable);
    }
    Ok(ExecutableIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
        length: metadata.len(),
        mode: metadata.mode(),
        uid: metadata.uid(),
        modified: (metadata.mtime(), metadata.mtime_nsec()),
        changed: (metadata.ctime(), metadata.ctime_nsec()),
    })
}

#[derive(Clone)]
pub struct ApprovedExecutable {
    #[cfg(unix)]
    path: PathBuf,
    review_reference: String,
    #[cfg(unix)]
    identity: ExecutableIdentity,
}

impl ApprovedExecutable {
    /// The Rust caller must verify and approve the executable source.
    /// A review reference records that decision. It does not prove authenticity.
    pub fn from_reviewed_source(
        path: &Path,
        review_reference: &str,
    ) -> Result<Self, SupervisorError> {
        if review_reference.len() > 1024
            || review_reference.trim().is_empty()
            || review_reference.chars().any(char::is_control)
        {
            return Err(SupervisorError::InvalidApproval);
        }
        #[cfg(unix)]
        {
            Ok(Self {
                path: path.to_owned(),
                review_reference: review_reference.into(),
                identity: executable_identity(path)?,
            })
        }
        #[cfg(not(unix))]
        {
            let _ = path;
            Err(SessionError::UnsupportedPlatform.into())
        }
    }

    pub fn review_reference(&self) -> &str {
        &self.review_reference
    }

    #[cfg(unix)]
    fn recheck(&self) -> Result<(), SessionError> {
        if executable_identity(&self.path)? != self.identity {
            return Err(SessionError::InvalidExecutable);
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct ManagedProfileHome {
    #[cfg(unix)]
    profile_id: ProfileId,
    #[cfg(unix)]
    directory: PathBuf,
    #[cfg(unix)]
    identity: (u64, u64),
}

impl ManagedProfileHome {
    pub fn new(
        profile_id: ProfileId,
        immutable_home_reference: &str,
        directory: &Path,
    ) -> Result<Self, SupervisorError> {
        ProfileId::parse(immutable_home_reference)
            .map_err(|_| SupervisorError::InvalidHomeReference)?;
        if directory.file_name() != Some(std::ffi::OsStr::new(immutable_home_reference)) {
            return Err(SupervisorError::InvalidHomeReference);
        }
        #[cfg(unix)]
        {
            Ok(Self {
                profile_id,
                directory: directory.to_owned(),
                identity: home_identity(directory)?,
            })
        }
        #[cfg(not(unix))]
        {
            let _ = profile_id;
            Err(SessionError::UnsupportedPlatform.into())
        }
    }

    #[cfg(unix)]
    fn recheck(&self) -> Result<(), SessionError> {
        if home_identity(&self.directory)? != self.identity {
            return Err(SessionError::InvalidManagedDirectory);
        }
        Ok(())
    }
}

#[cfg(unix)]
fn home_identity(path: &Path) -> Result<(u64, u64), SessionError> {
    use std::os::unix::fs::MetadataExt;
    if !absolute_normal_path(path) {
        return Err(SessionError::InvalidManagedDirectory);
    }
    let metadata =
        std::fs::symlink_metadata(path).map_err(|_| SessionError::InvalidManagedDirectory)?;
    // geteuid reads the process identity without pointer arguments.
    if path.canonicalize().ok().as_deref() != Some(path)
        || !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || metadata.mode() & 0o077 != 0
        || metadata.uid() != unsafe { libc::geteuid() }
    {
        return Err(SessionError::InvalidManagedDirectory);
    }
    Ok((metadata.dev(), metadata.ino()))
}

#[cfg(unix)]
#[derive(Default)]
struct Registry {
    closed: bool,
    cleanup_error: Option<SupervisorError>,
    active: HashMap<ProfileId, Arc<Job>>,
    jobs: Vec<Arc<Job>>,
    bindings: HashMap<ProfileId, (PathBuf, (u64, u64))>,
    homes: HashMap<PathBuf, ProfileId>,
}

#[cfg(unix)]
struct Job {
    profile_id: ProfileId,
    control: Arc<ProcessControl>,
    worker: Mutex<Option<JoinHandle<()>>>,
    finished: AtomicBool,
    cleanup_result: Mutex<Option<Result<(), SupervisorError>>>,
}

#[cfg(unix)]
impl Job {
    fn request_cancel(&self) -> bool {
        let completion = self
            .cleanup_result
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if completion.is_some() {
            return false;
        }
        self.control.request_cancel();
        true
    }

    fn join(&self) -> Result<(), SupervisorError> {
        let mut worker = self
            .worker
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let result = worker
            .take()
            .map(|worker| worker.join().map_err(|_| SupervisorError::WorkerFailed))
            .unwrap_or(Ok(()));
        drop(worker);
        result?;
        self.cleanup_result
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .unwrap_or(Err(SupervisorError::WorkerFailed))
    }

    fn cancel_wait(&self) -> Result<bool, SupervisorError> {
        let accepted = self.request_cancel();
        let stopped = self.control.stop_child().map_err(SupervisorError::from);
        let joined = self.join();
        stopped.and(joined).map(|()| accepted)
    }
}

#[cfg(unix)]
fn retire(registry: &Mutex<Registry>, job: &Arc<Job>) {
    let owns_child = job.control.has_child();
    let cleanup = *job
        .cleanup_result
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut registry = registry
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(Err(error)) = cleanup
        && registry.cleanup_error.is_none()
    {
        registry.cleanup_error = Some(error);
    }
    if owns_child {
        return;
    }
    if registry
        .active
        .get(&job.profile_id)
        .is_some_and(|current| Arc::ptr_eq(current, job))
    {
        registry.active.remove(&job.profile_id);
    }
    registry.jobs.retain(|current| !Arc::ptr_eq(current, job));
}

pub struct Supervisor {
    #[cfg(unix)]
    registry: Arc<Mutex<Registry>>,
    #[cfg(unix)]
    timeouts: Timeouts,
}

impl Default for Supervisor {
    fn default() -> Self {
        Self::new()
    }
}

impl Supervisor {
    pub fn new() -> Self {
        Self {
            #[cfg(unix)]
            registry: Arc::default(),
            #[cfg(unix)]
            timeouts: Timeouts::default(),
        }
    }

    pub fn start_check(
        &self,
        executable: &ApprovedExecutable,
        home: &ManagedProfileHome,
    ) -> Result<CheckHandle, SupervisorError> {
        #[cfg(not(unix))]
        {
            let _ = (executable, home);
            Err(SessionError::UnsupportedPlatform.into())
        }
        #[cfg(unix)]
        {
            self.reap_finished()?;
            let mut registry = self
                .registry
                .lock()
                .map_err(|_| SupervisorError::WorkerFailed)?;
            if registry.closed {
                return Err(SupervisorError::Closed);
            }
            if registry.active.contains_key(&home.profile_id) {
                return Err(SupervisorError::DuplicateProfile);
            }
            if registry.active.len() >= 2 {
                return Err(SupervisorError::CapacityReached);
            }
            if registry
                .bindings
                .get(&home.profile_id)
                .is_some_and(|(path, identity)| {
                    path != &home.directory || *identity != home.identity
                })
            {
                return Err(SupervisorError::HomeChanged);
            }
            if registry
                .homes
                .get(&home.directory)
                .is_some_and(|profile| profile != &home.profile_id)
            {
                return Err(SupervisorError::HomeInUse);
            }
            let job = Arc::new(Job {
                profile_id: home.profile_id.clone(),
                control: Arc::default(),
                worker: Mutex::new(None),
                finished: AtomicBool::new(false),
                cleanup_result: Mutex::new(None),
            });
            let (sender, receiver) = mpsc::channel();
            let worker_job = Arc::clone(&job);
            let worker_registry = Arc::downgrade(&self.registry);
            let executable = executable.clone();
            let home = home.clone();
            let timeouts = self.timeouts;
            registry.bindings.insert(
                home.profile_id.clone(),
                (home.directory.clone(), home.identity),
            );
            registry
                .homes
                .insert(home.directory.clone(), home.profile_id.clone());
            registry
                .active
                .insert(home.profile_id.clone(), Arc::clone(&job));
            registry.jobs.push(Arc::clone(&job));
            let thread = std::thread::Builder::new()
                .name("openusage-provider-check".into())
                .spawn(move || {
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        executable.recheck()?;
                        home.recheck()?;
                        crate::session::unix::check_managed(
                            &executable.path,
                            &home.directory,
                            Arc::clone(&worker_job.control),
                            timeouts,
                            || {
                                executable.recheck()?;
                                home.recheck()
                            },
                        )
                        .map_err(SupervisorError::from)
                    }))
                    .unwrap_or(Err(SupervisorError::WorkerFailed));
                    let stopped = worker_job
                        .control
                        .stop_child()
                        .map_err(SupervisorError::from);
                    if !worker_job.control.has_child()
                        && let Some(registry) = worker_registry.upgrade()
                    {
                        let mut registry = registry
                            .lock()
                            .unwrap_or_else(|poisoned| poisoned.into_inner());
                        if registry
                            .active
                            .get(&worker_job.profile_id)
                            .is_some_and(|current| Arc::ptr_eq(current, &worker_job))
                        {
                            registry.active.remove(&worker_job.profile_id);
                        }
                    }
                    let mut completion = worker_job
                        .cleanup_result
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    let mut result = stopped.and(result);
                    if worker_job.control.check_cancelled().is_err()
                        && !matches!(
                            &result,
                            Err(SupervisorError::Session(
                                SessionError::CleanupFailed | SessionError::TerminationFailed
                            ) | SupervisorError::WorkerFailed)
                        )
                    {
                        result = Err(SupervisorError::Cancelled);
                    }
                    let cleanup = match &result {
                        Err(
                            error @ (SupervisorError::Session(
                                SessionError::CleanupFailed | SessionError::TerminationFailed,
                            )
                            | SupervisorError::WorkerFailed),
                        ) => Err(*error),
                        _ => Ok(()),
                    };
                    *completion = Some(cleanup);
                    drop(completion);
                    worker_job.finished.store(true, Ordering::Release);
                    let _ = sender.send(result);
                });
            match thread {
                Ok(thread) => {
                    *job.worker
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(thread)
                }
                Err(_) => {
                    registry.active.remove(&job.profile_id);
                    registry.jobs.retain(|current| !Arc::ptr_eq(current, &job));
                    return Err(SupervisorError::WorkerFailed);
                }
            }
            Ok(CheckHandle {
                job,
                receiver: Some(receiver),
                registry: Arc::downgrade(&self.registry),
                joined: false,
            })
        }
    }

    #[cfg(unix)]
    fn reap_finished(&self) -> Result<(), SupervisorError> {
        let jobs: Vec<_> = self
            .registry
            .lock()
            .map_err(|_| SupervisorError::WorkerFailed)?
            .jobs
            .iter()
            .filter(|job| job.finished.load(Ordering::Acquire))
            .cloned()
            .collect();
        for job in jobs {
            let _ = job.join();
            retire(&self.registry, &job);
        }
        Ok(())
    }

    pub fn cancel(&self, profile_id: &ProfileId) -> Result<bool, SupervisorError> {
        #[cfg(not(unix))]
        {
            let _ = profile_id;
            Err(SessionError::UnsupportedPlatform.into())
        }
        #[cfg(unix)]
        {
            let job = self
                .registry
                .lock()
                .map_err(|_| SupervisorError::WorkerFailed)?
                .active
                .get(profile_id)
                .cloned();
            let Some(job) = job else {
                return Ok(false);
            };
            let result = job.cancel_wait();
            retire(&self.registry, &job);
            result
        }
    }

    /// Shutdown rejects new work and waits for all workers.
    /// Use explicit shutdown to report cleanup failures.
    pub fn shutdown(&self) -> Result<(), SupervisorError> {
        #[cfg(not(unix))]
        {
            Ok(())
        }
        #[cfg(unix)]
        {
            let (jobs, prior_error) = {
                let mut registry = self
                    .registry
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                registry.closed = true;
                (registry.jobs.clone(), registry.cleanup_error)
            };
            for job in &jobs {
                job.request_cancel();
            }
            let mut result = prior_error.map_or(Ok(()), Err);
            for job in jobs {
                let stopped = job.cancel_wait().map(|_| ());
                retire(&self.registry, &job);
                if result.is_ok() {
                    result = stopped;
                }
            }
            result
        }
    }
}

impl Drop for Supervisor {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

/// A handle cancels and joins its check when dropped.
pub struct CheckHandle {
    #[cfg(unix)]
    job: Arc<Job>,
    #[cfg(unix)]
    receiver: Option<Receiver<Result<ProviderProbe, SupervisorError>>>,
    #[cfg(unix)]
    registry: Weak<Mutex<Registry>>,
    #[cfg(unix)]
    joined: bool,
}

impl CheckHandle {
    pub fn wait(self) -> Result<ProviderProbe, SupervisorError> {
        #[cfg(not(unix))]
        {
            Err(SessionError::UnsupportedPlatform.into())
        }
        #[cfg(unix)]
        {
            let mut handle = self;
            let result = handle
                .receiver
                .take()
                .ok_or(SupervisorError::WorkerFailed)?
                .recv()
                .map_err(|_| SupervisorError::WorkerFailed);
            let joined = handle.job.join();
            handle.joined = true;
            if let Some(registry) = handle.registry.upgrade() {
                retire(&registry, &handle.job);
            }
            joined?;
            result?
        }
    }

    pub fn cancel(&self) -> Result<(), SupervisorError> {
        #[cfg(not(unix))]
        {
            Err(SessionError::UnsupportedPlatform.into())
        }
        #[cfg(unix)]
        {
            let result = self.job.cancel_wait();
            if let Some(registry) = self.registry.upgrade() {
                retire(&registry, &self.job);
            }
            result.map(|_| ())
        }
    }
}

impl Drop for CheckHandle {
    fn drop(&mut self) {
        #[cfg(unix)]
        if !self.joined {
            let _ = self.cancel();
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::{AccountAuth, IdentityConfidence, ProtocolError};
    use std::{
        fs,
        os::unix::{
            fs::{DirBuilderExt, PermissionsExt, symlink},
            process::CommandExt,
        },
        process::{Child, Command, Stdio},
        time::{Duration, Instant},
    };

    enum Mode {
        Success,
        Hang,
        Partial,
        Oversized,
        MoveRuntime,
    }

    struct Fixture {
        temporary: tempfile::TempDir,
        executable: ApprovedExecutable,
        homes: Vec<ManagedProfileHome>,
    }

    impl Fixture {
        fn new(mode: Mode) -> Self {
            let temporary = tempfile::tempdir().unwrap();
            let executable = temporary.path().join("synthetic-provider");
            let mut script = r##"#!/bin/sh
if [ "$1" = "--version" ]; then
  printf 'codex-cli 0.163.0\n'
  exit 0
fi
if [ -n "${OPENAI_API_KEY-}" ] || [ -n "${OPENAI_ACCESS_TOKEN-}" ] || [ -n "${DBUS_SESSION_BUS_ADDRESS-}" ]; then exit 42; fi
printf '%s' "$$" > "$CODEX_HOME/synthetic-child-pid"
printf '%s' "$HOME" > "$CODEX_HOME/synthetic-runtime-home"
IFS= read -r request
printf '{"id":1,"result":{"userAgent":"synthetic supervisor fixture","codexHome":"%s","platformFamily":"unix","platformOs":"linux"}}\n' "$CODEX_HOME"
IFS= read -r notification
IFS= read -r request
"##.to_owned();
            script.push_str(match mode {
                Mode::Success => "printf '{\"id\":2,\"result\":{\"account\":null,\"requiresOpenaiAuth\":true}}\\n'\nexec /bin/sleep 60\n",
                Mode::Hang => "exec /bin/sleep 60\n",
                Mode::Partial => "printf '{\"id\":2'\nexec /bin/sleep 60\n",
                Mode::Oversized => "exec /usr/bin/head -c 1048577 /dev/zero\n",
                Mode::MoveRuntime => "owned_root=${HOME%/home}\nmv \"$owned_root\" \"$owned_root-moved\"\nprintf '{\"id\":2,\"result\":{\"account\":null,\"requiresOpenaiAuth\":true}}\\n'\nexec /bin/sleep 60\n",
            });
            fs::write(&executable, script).unwrap();
            fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
            let executable = ApprovedExecutable::from_reviewed_source(
                &executable,
                "Owned executable synthetic transport fixture; no real provider.",
            )
            .unwrap();
            let homes = (1..=3)
                .map(|number| {
                    let id = format!("{number:08x}-0000-4000-8000-000000000000");
                    let directory = temporary.path().join(&id);
                    fs::DirBuilder::new()
                        .mode(0o700)
                        .create(&directory)
                        .unwrap();
                    ManagedProfileHome::new(ProfileId::parse(&id).unwrap(), &id, &directory)
                        .unwrap()
                })
                .collect();
            Self {
                temporary,
                executable,
                homes,
            }
        }

        fn child(&self, index: usize) -> libc::pid_t {
            let marker = self.homes[index].directory.join("synthetic-child-pid");
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                if let Ok(value) = fs::read_to_string(&marker)
                    && let Ok(pid) = value.parse()
                {
                    return pid;
                }
                assert!(
                    Instant::now() < deadline,
                    "The synthetic executable did not report readiness."
                );
                std::thread::sleep(Duration::from_millis(10));
            }
        }

        fn runtime_removed(&self, index: usize) {
            let path =
                fs::read_to_string(self.homes[index].directory.join("synthetic-runtime-home"))
                    .unwrap();
            let path = Path::new(&path);
            assert!(path.starts_with(self.temporary.path()));
            assert!(!path.exists());
            assert!(
                !fs::read_dir(&self.homes[index].directory)
                    .unwrap()
                    .any(|entry| entry
                        .unwrap()
                        .file_name()
                        .to_string_lossy()
                        .starts_with("openusage-provider-"))
            );
        }
    }

    struct Unrelated(Child);
    impl Drop for Unrelated {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    fn gone(pid: libc::pid_t) {
        // Signal zero observes only this fixture's recorded process identity.
        assert_eq!(unsafe { libc::kill(pid, 0) }, -1);
        assert_eq!(
            std::io::Error::last_os_error().raw_os_error(),
            Some(libc::ESRCH)
        );
        assert_eq!(unsafe { libc::killpg(pid, 0) }, -1);
        assert_eq!(
            std::io::Error::last_os_error().raw_os_error(),
            Some(libc::ESRCH)
        );
    }

    #[test]
    fn synthetic_two_slot_duplicate_cancel_and_shutdown_preserve_unrelated_process() {
        let fixture = Fixture::new(Mode::Hang);
        let supervisor = Supervisor::new();
        let mut unrelated = Unrelated(
            Command::new("/bin/sleep")
                .arg("60")
                .env_clear()
                .process_group(0)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap(),
        );
        let first = supervisor
            .start_check(&fixture.executable, &fixture.homes[0])
            .unwrap();
        let second = supervisor
            .start_check(&fixture.executable, &fixture.homes[1])
            .unwrap();
        let first_pid = fixture.child(0);
        let second_pid = fixture.child(1);
        assert!(matches!(
            supervisor.start_check(&fixture.executable, &fixture.homes[0]),
            Err(SupervisorError::DuplicateProfile)
        ));
        assert!(matches!(
            supervisor.start_check(&fixture.executable, &fixture.homes[2]),
            Err(SupervisorError::CapacityReached)
        ));
        assert_eq!(supervisor.cancel(&fixture.homes[0].profile_id), Ok(true));
        assert!(matches!(first.wait(), Err(SupervisorError::Cancelled)));
        gone(first_pid);
        fixture.runtime_removed(0);
        assert_eq!(unrelated.0.try_wait().unwrap(), None);
        let third = supervisor
            .start_check(&fixture.executable, &fixture.homes[2])
            .unwrap();
        let third_pid = fixture.child(2);
        supervisor.shutdown().unwrap();
        assert!(matches!(second.wait(), Err(SupervisorError::Cancelled)));
        assert!(matches!(third.wait(), Err(SupervisorError::Cancelled)));
        gone(second_pid);
        gone(third_pid);
        fixture.runtime_removed(1);
        fixture.runtime_removed(2);
        assert_eq!(unrelated.0.try_wait().unwrap(), None);
        assert!(matches!(
            supervisor.start_check(&fixture.executable, &fixture.homes[0]),
            Err(SupervisorError::Closed)
        ));
        assert!(supervisor.registry.lock().unwrap().active.is_empty());
        assert!(supervisor.registry.lock().unwrap().jobs.is_empty());
    }

    #[test]
    fn synthetic_handle_and_supervisor_drop_cancel_and_join_owned_checks() {
        let fixture = Fixture::new(Mode::Hang);
        let supervisor = Supervisor::new();
        let first = supervisor
            .start_check(&fixture.executable, &fixture.homes[0])
            .unwrap();
        let first_pid = fixture.child(0);
        drop(first);
        gone(first_pid);
        fixture.runtime_removed(0);
        let second = supervisor
            .start_check(&fixture.executable, &fixture.homes[1])
            .unwrap();
        let second_pid = fixture.child(1);
        drop(supervisor);
        gone(second_pid);
        fixture.runtime_removed(1);
        assert!(matches!(second.wait(), Err(SupervisorError::Cancelled)));
    }

    #[test]
    fn synthetic_success_preserves_home_and_rejects_aliases_or_replaced_home_identity() {
        let fixture = Fixture::new(Mode::Success);
        let supervisor = Supervisor::new();
        let marker = fixture.homes[0].directory.join("provider-owned-marker");
        fs::write(
            &marker,
            b"Synthetic managed provider data. Preserve this file.",
        )
        .unwrap();
        let result = supervisor
            .start_check(&fixture.executable, &fixture.homes[0])
            .unwrap()
            .wait()
            .unwrap();
        assert_eq!(
            result.account.authentication,
            AccountAuth::AuthenticationRequired
        );
        assert_eq!(result.account.identity, IdentityConfidence::Unknown);
        gone(result.process_id as libc::pid_t);
        fixture.runtime_removed(0);
        assert_eq!(
            fs::read(&marker).unwrap(),
            b"Synthetic managed provider data. Preserve this file."
        );
        let alias = ManagedProfileHome::new(
            fixture.homes[1].profile_id.clone(),
            fixture.homes[0].profile_id.as_str(),
            &fixture.homes[0].directory,
        )
        .unwrap();
        assert!(matches!(
            supervisor.start_check(&fixture.executable, &alias),
            Err(SupervisorError::HomeInUse)
        ));
        let different_home = ManagedProfileHome::new(
            fixture.homes[0].profile_id.clone(),
            fixture.homes[1].profile_id.as_str(),
            &fixture.homes[1].directory,
        )
        .unwrap();
        assert!(matches!(
            supervisor.start_check(&fixture.executable, &different_home),
            Err(SupervisorError::HomeChanged)
        ));
        let original = fixture.temporary.path().join("preserve-original-home");
        fs::rename(&fixture.homes[0].directory, &original).unwrap();
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&fixture.homes[0].directory)
            .unwrap();
        let replacement = ManagedProfileHome::new(
            fixture.homes[0].profile_id.clone(),
            fixture.homes[0].profile_id.as_str(),
            &fixture.homes[0].directory,
        )
        .unwrap();
        assert!(matches!(
            supervisor.start_check(&fixture.executable, &replacement),
            Err(SupervisorError::HomeChanged)
        ));
        assert_eq!(
            fs::read(original.join("provider-owned-marker")).unwrap(),
            b"Synthetic managed provider data. Preserve this file."
        );
    }

    #[test]
    fn synthetic_timeouts_stop_partial_reads_and_cleanup_faults_return_errors_after_reaping() {
        for (mode, expected) in [
            (Mode::Hang, SessionError::TimedOut),
            (Mode::Partial, SessionError::TimedOut),
            (
                Mode::Oversized,
                SessionError::Protocol(ProtocolError::OversizedMessage),
            ),
        ] {
            let fixture = Fixture::new(mode);
            let mut supervisor = Supervisor::new();
            supervisor.timeouts = Timeouts {
                ready: Duration::from_secs(2),
                read: Duration::from_millis(80),
            };
            let started = Instant::now();
            let handle = supervisor
                .start_check(&fixture.executable, &fixture.homes[0])
                .unwrap();
            let pid = fixture.child(0);
            assert!(
                matches!(handle.wait(), Err(SupervisorError::Session(error)) if error == expected)
            );
            assert!(started.elapsed() < Duration::from_secs(5));
            gone(pid);
            fixture.runtime_removed(0);
        }
        let fixture = Fixture::new(Mode::MoveRuntime);
        let supervisor = Supervisor::new();
        let handle = supervisor
            .start_check(&fixture.executable, &fixture.homes[0])
            .unwrap();
        let pid = fixture.child(0);
        let result = handle.wait();
        assert!(matches!(
            result,
            Err(SupervisorError::Session(SessionError::CleanupFailed))
        ));
        gone(pid);
        assert_eq!(
            supervisor.shutdown(),
            Err(SupervisorError::Session(SessionError::CleanupFailed))
        );
        assert_eq!(
            fs::read_dir(&fixture.homes[0].directory)
                .unwrap()
                .filter(|entry| entry
                    .as_ref()
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .starts_with("openusage-provider-"))
                .count(),
            1
        );
    }

    #[test]
    fn synthetic_approval_and_managed_target_checks_reject_substitution_links_and_unsafe_modes() {
        let fixture = Fixture::new(Mode::Success);
        for reference in [String::new(), "x".repeat(1025), "contains\ncontrol".into()] {
            assert!(matches!(
                ApprovedExecutable::from_reviewed_source(&fixture.executable.path, &reference),
                Err(SupervisorError::InvalidApproval)
            ));
        }
        let old = fixture.temporary.path().join("original-synthetic-provider");
        fs::rename(&fixture.executable.path, &old).unwrap();
        fs::copy(&old, &fixture.executable.path).unwrap();
        let supervisor = Supervisor::new();
        assert!(matches!(
            supervisor
                .start_check(&fixture.executable, &fixture.homes[0])
                .unwrap()
                .wait(),
            Err(SupervisorError::Session(SessionError::InvalidExecutable))
        ));
        assert!(
            fs::read_dir(&fixture.homes[0].directory)
                .unwrap()
                .next()
                .is_none()
        );
        for mode in [0o770, 0o4700] {
            fs::set_permissions(&fixture.executable.path, fs::Permissions::from_mode(mode))
                .unwrap();
            assert!(matches!(
                ApprovedExecutable::from_reviewed_source(
                    &fixture.executable.path,
                    "Synthetic review."
                ),
                Err(SupervisorError::Session(SessionError::InvalidExecutable))
            ));
        }
        let linked_executable = fixture.temporary.path().join("linked-executable");
        symlink(&old, &linked_executable).unwrap();
        assert!(
            ApprovedExecutable::from_reviewed_source(&linked_executable, "Synthetic review.")
                .is_err()
        );
        let alias_parent = fixture.temporary.path().join("aliases");
        fs::create_dir(&alias_parent).unwrap();
        let linked_home = alias_parent.join(fixture.homes[0].profile_id.as_str());
        symlink(&fixture.homes[0].directory, &linked_home).unwrap();
        assert!(
            ManagedProfileHome::new(
                fixture.homes[0].profile_id.clone(),
                fixture.homes[0].profile_id.as_str(),
                &linked_home
            )
            .is_err()
        );
        fs::set_permissions(
            &fixture.homes[1].directory,
            fs::Permissions::from_mode(0o755),
        )
        .unwrap();
        assert!(
            ManagedProfileHome::new(
                fixture.homes[1].profile_id.clone(),
                fixture.homes[1].profile_id.as_str(),
                &fixture.homes[1].directory
            )
            .is_err()
        );
        assert!(
            ManagedProfileHome::new(
                fixture.homes[2].profile_id.clone(),
                fixture.homes[0].profile_id.as_str(),
                &fixture.homes[2].directory
            )
            .is_err()
        );
    }
}
