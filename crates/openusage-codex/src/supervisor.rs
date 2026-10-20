//! Bounded provider checks with explicit Rust approval and owned cancellation.
//! No inference or background collection. The caller owns native application shutdown hooks.
//!
//! Process ownership follows the standard library's process-group and child-wait contracts:
//! <https://doc.rust-lang.org/std/os/unix/process/trait.CommandExt.html#tymethod.process_group>
//! <https://doc.rust-lang.org/std/process/struct.Child.html#method.wait>

use crate::session::{ActivityProbe, CollectionProbe, ProviderProbe, QuotaProbe, SessionError};
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

/// Check the executable pathname's ancestors without reading or executing its contents.
/// This policy does not approve a source or establish publisher authenticity.
pub fn validate_executable_source_path(path: &Path) -> Result<(), SupervisorError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if !absolute_normal_path(path) || path.canonicalize().ok().as_deref() != Some(path) {
            return Err(SessionError::InvalidExecutable.into());
        }
        // An unmapped UID must remain untrusted. Never treat the namespace overflow UID as root.
        let effective_uid = unsafe { libc::geteuid() };
        let trusted_owner = |uid| uid == 0 || uid == effective_uid;
        let mut child = path;
        for ancestor in path.ancestors().skip(1) {
            let parent =
                std::fs::symlink_metadata(ancestor).map_err(|_| SessionError::InvalidExecutable)?;
            if !parent.is_dir() || parent.file_type().is_symlink() || !trusted_owner(parent.uid()) {
                return Err(SessionError::InvalidExecutable.into());
            }
            if parent.mode() & 0o022 != 0 {
                let child_metadata = std::fs::symlink_metadata(child)
                    .map_err(|_| SessionError::InvalidExecutable)?;
                if parent.mode() & 0o1000 == 0 || !trusted_owner(child_metadata.uid()) {
                    return Err(SessionError::InvalidExecutable.into());
                }
            }
            child = ancestor;
        }
        Ok(())
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Err(SessionError::UnsupportedPlatform.into())
    }
}

#[cfg(unix)]
fn executable_identity(path: &Path) -> Result<ExecutableIdentity, SessionError> {
    validate_executable_source_path(path).map_err(|_| SessionError::InvalidExecutable)?;
    let metadata = std::fs::symlink_metadata(path).map_err(|_| SessionError::InvalidExecutable)?;
    identity_from_metadata(&metadata)
}

#[cfg(unix)]
fn identity_from_metadata(
    metadata: &std::fs::Metadata,
) -> Result<ExecutableIdentity, SessionError> {
    use std::os::unix::fs::MetadataExt;
    if !metadata.is_file()
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
    #[cfg(unix)]
    pinned: Option<PinnedExecutable>,
}

#[cfg(unix)]
#[derive(Clone)]
struct PinnedExecutable {
    file: Arc<std::fs::File>,
    sha256: [u8; 32],
}

#[cfg(unix)]
const MAX_APPROVED_EXECUTABLE_BYTES: u64 = 1024 * 1024 * 1024;

#[cfg(unix)]
fn hash_executable(
    path: &Path,
    file: &std::fs::File,
) -> Result<([u8; 32], ExecutableIdentity), SessionError> {
    use sha2::{Digest, Sha256};
    use std::os::unix::fs::FileExt;
    let before = executable_identity(path)?;
    if !(1..=MAX_APPROVED_EXECUTABLE_BYTES).contains(&before.length)
        || identity_from_metadata(
            &file
                .metadata()
                .map_err(|_| SessionError::InvalidExecutable)?,
        )? != before
    {
        return Err(SessionError::InvalidExecutable);
    }
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    let mut offset = 0_u64;
    while offset < before.length {
        let available = usize::try_from((before.length - offset).min(buffer.len() as u64))
            .map_err(|_| SessionError::InvalidExecutable)?;
        let read = loop {
            match file.read_at(&mut buffer[..available], offset) {
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                result => break result.map_err(|_| SessionError::InvalidExecutable)?,
            }
        };
        if read == 0 {
            return Err(SessionError::InvalidExecutable);
        }
        digest.update(&buffer[..read]);
        offset = offset
            .checked_add(read as u64)
            .ok_or(SessionError::InvalidExecutable)?;
    }
    if executable_identity(path)? != before
        || identity_from_metadata(
            &file
                .metadata()
                .map_err(|_| SessionError::InvalidExecutable)?,
        )? != before
    {
        return Err(SessionError::InvalidExecutable);
    }
    Ok((digest.finalize().into(), before))
}

fn validate_review_reference(review_reference: &str) -> Result<(), SupervisorError> {
    if review_reference.len() > 1024
        || review_reference.trim().is_empty()
        || review_reference.chars().any(char::is_control)
    {
        return Err(SupervisorError::InvalidApproval);
    }
    Ok(())
}

impl ApprovedExecutable {
    /// The Rust caller must verify and approve the executable source.
    /// A review reference records that decision. It does not prove authenticity.
    pub fn from_reviewed_source(
        path: &Path,
        review_reference: &str,
    ) -> Result<Self, SupervisorError> {
        validate_review_reference(review_reference)?;
        #[cfg(unix)]
        {
            Ok(Self {
                path: path.to_owned(),
                review_reference: review_reference.into(),
                identity: executable_identity(path)?,
                pinned: None,
            })
        }
        #[cfg(not(unix))]
        {
            let _ = path;
            Err(SessionError::UnsupportedPlatform.into())
        }
    }

    /// Bind a caller's explicit review to these executable bytes. A digest does not prove publisher authenticity.
    /// The held descriptor and digest are checked again before each provider process starts.
    pub fn from_reviewed_source_with_sha256(
        path: &Path,
        review_reference: &str,
        expected: [u8; 32],
    ) -> Result<Self, SupervisorError> {
        validate_review_reference(review_reference)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            // Validate the path before opening so FIFOs and unsafe paths never reach a blocking open.
            executable_identity(path)?;
            let file = std::fs::OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
                .open(path)
                .map_err(|_| SessionError::InvalidExecutable)?;
            let (sha256, identity) = hash_executable(path, &file)?;
            if sha256 != expected {
                return Err(SessionError::InvalidExecutable.into());
            }
            Ok(Self {
                path: path.into(),
                review_reference: review_reference.into(),
                identity,
                pinned: Some(PinnedExecutable {
                    file: Arc::new(file),
                    sha256: expected,
                }),
            })
        }
        #[cfg(not(unix))]
        {
            let _ = (path, expected);
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
        if let Some(pinned) = &self.pinned {
            let (digest, identity) = hash_executable(&self.path, &pinned.file)?;
            if identity != self.identity || digest != pinned.sha256 {
                return Err(SessionError::InvalidExecutable);
            }
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
    managed_login: bool,
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
        // A login worker sends the documented cancellation before hard cleanup. Its reader polls
        // cancellation every 100 ms and the cancellation exchange has a fixed one-second limit.
        let stopped = if self.managed_login {
            Ok(())
        } else {
            self.control.stop_child().map_err(SupervisorError::from)
        };
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

enum Operation {
    Check,
    QuotaDiagnostic,
    ActivityDiagnostic,
    CollectionDiagnostic,
    #[cfg(unix)]
    DeviceLogin(mpsc::SyncSender<Result<crate::DeviceLoginPrompt, SupervisorError>>),
}

enum OperationResult {
    Check(ProviderProbe),
    Quota(QuotaProbe),
    Activity(ActivityProbe),
    Collection(CollectionProbe),
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
        self.start_operation(executable, home, Operation::Check)
    }

    /// Start only after explicit consent for this selected profile's managed sign-in.
    /// Call `prompt` and `wait` outside application or storage locks.
    pub fn start_device_login(
        &self,
        executable: &ApprovedExecutable,
        home: &ManagedProfileHome,
    ) -> Result<DeviceLoginHandle, SupervisorError> {
        #[cfg(not(unix))]
        {
            let _ = (executable, home);
            Err(SessionError::UnsupportedPlatform.into())
        }
        #[cfg(unix)]
        {
            if executable.pinned.is_none() {
                return Err(SupervisorError::InvalidApproval);
            }
            let (sender, receiver) = mpsc::sync_channel(1);
            let check = self.start_operation(executable, home, Operation::DeviceLogin(sender))?;
            Ok(DeviceLoginHandle {
                check,
                prompt: Some(receiver),
            })
        }
    }

    /// Read provisional quota diagnostics only. It does not accept history or start monitoring.
    pub fn start_quota_diagnostic(
        &self,
        executable: &ApprovedExecutable,
        home: &ManagedProfileHome,
    ) -> Result<QuotaHandle, SupervisorError> {
        #[cfg(unix)]
        if executable.pinned.is_none() {
            return Err(SupervisorError::InvalidApproval);
        }
        self.start_operation(executable, home, Operation::QuotaDiagnostic)
            .map(|check| QuotaHandle { check })
    }

    /// Read provisional activity diagnostics only. It does not accept history or start monitoring.
    pub fn start_activity_diagnostic(
        &self,
        executable: &ApprovedExecutable,
        home: &ManagedProfileHome,
    ) -> Result<ActivityHandle, SupervisorError> {
        #[cfg(unix)]
        if executable.pinned.is_none() {
            return Err(SupervisorError::InvalidApproval);
        }
        self.start_operation(executable, home, Operation::ActivityDiagnostic)
            .map(|check| ActivityHandle { check })
    }

    /// Read both supported capabilities in one account-bound, read-only operation.
    pub fn start_collection_diagnostic(
        &self,
        executable: &ApprovedExecutable,
        home: &ManagedProfileHome,
    ) -> Result<CollectionHandle, SupervisorError> {
        #[cfg(unix)]
        if executable.pinned.is_none() {
            return Err(SupervisorError::InvalidApproval);
        }
        self.start_operation(executable, home, Operation::CollectionDiagnostic)
            .map(|check| CollectionHandle { check })
    }

    fn start_operation(
        &self,
        executable: &ApprovedExecutable,
        home: &ManagedProfileHome,
        operation: Operation,
    ) -> Result<CheckHandle, SupervisorError> {
        #[cfg(not(unix))]
        {
            let _ = (executable, home, operation);
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
                managed_login: matches!(&operation, Operation::DeviceLogin(_)),
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
                    let mut prompt_sent = false;
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        executable.recheck()?;
                        home.recheck()?;
                        let before_start = || {
                            executable.recheck()?;
                            home.recheck()
                        };
                        match &operation {
                            Operation::DeviceLogin(sender) => crate::session::unix::login_managed(
                                &executable.path,
                                &home.directory,
                                Arc::clone(&worker_job.control),
                                timeouts,
                                before_start,
                                |metadata| {
                                    prompt_sent = true;
                                    sender
                                        .send(Ok(metadata))
                                        .map_err(|_| SessionError::Cancelled)
                                },
                            )
                            .map(OperationResult::Check),
                            Operation::Check => crate::session::unix::check_managed(
                                &executable.path,
                                &home.directory,
                                Arc::clone(&worker_job.control),
                                timeouts,
                                before_start,
                            )
                            .map(OperationResult::Check),
                            Operation::QuotaDiagnostic => crate::session::unix::quota_managed(
                                &executable.path,
                                &home.directory,
                                Arc::clone(&worker_job.control),
                                timeouts,
                                before_start,
                            )
                            .map(OperationResult::Quota),
                            Operation::CollectionDiagnostic => {
                                crate::session::unix::collection_managed(
                                    &executable.path,
                                    &home.directory,
                                    Arc::clone(&worker_job.control),
                                    timeouts,
                                    before_start,
                                )
                                .map(OperationResult::Collection)
                            }
                            Operation::ActivityDiagnostic => {
                                crate::session::unix::activity_managed(
                                    &executable.path,
                                    &home.directory,
                                    Arc::clone(&worker_job.control),
                                    timeouts,
                                    before_start,
                                )
                                .map(OperationResult::Activity)
                            }
                        }
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
                    if !prompt_sent && let Operation::DeviceLogin(sender) = operation {
                        let error = result
                            .as_ref()
                            .err()
                            .copied()
                            .unwrap_or(SupervisorError::WorkerFailed);
                        let _ = sender.send(Err(error));
                    }
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

/// Owns a single selected-profile login. Dropping it cancels and joins its owned worker.
/// This type exposes no provider login ID, process handle, or credential-file contents.
pub struct DeviceLoginHandle {
    #[cfg(unix)]
    check: CheckHandle,
    #[cfg(unix)]
    prompt: Option<Receiver<Result<crate::DeviceLoginPrompt, SupervisorError>>>,
}

impl DeviceLoginHandle {
    pub fn prompt(&mut self) -> Result<crate::DeviceLoginPrompt, SupervisorError> {
        #[cfg(unix)]
        {
            self.check.job.control.check_cancelled()?;
            let prompt = self
                .prompt
                .take()
                .ok_or(SupervisorError::WorkerFailed)?
                .recv()
                .map_err(|_| SupervisorError::WorkerFailed)?;
            self.check.job.control.check_cancelled()?;
            prompt
        }
        #[cfg(not(unix))]
        {
            Err(SessionError::UnsupportedPlatform.into())
        }
    }

    /// Completion requires a matching successful login event and a fresh ChatGPT account response.
    pub fn wait(self) -> Result<ProviderProbe, SupervisorError> {
        #[cfg(unix)]
        {
            self.check.wait()
        }
        #[cfg(not(unix))]
        {
            Err(SessionError::UnsupportedPlatform.into())
        }
    }

    pub fn cancel(&self) -> Result<(), SupervisorError> {
        #[cfg(unix)]
        {
            self.check.cancel()
        }
        #[cfg(not(unix))]
        {
            Err(SessionError::UnsupportedPlatform.into())
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
    receiver: Option<Receiver<Result<OperationResult, SupervisorError>>>,
    #[cfg(unix)]
    registry: Weak<Mutex<Registry>>,
    #[cfg(unix)]
    joined: bool,
}

impl CheckHandle {
    pub fn wait(self) -> Result<ProviderProbe, SupervisorError> {
        match self.wait_result()? {
            OperationResult::Check(probe) => Ok(probe),
            OperationResult::Quota(_)
            | OperationResult::Activity(_)
            | OperationResult::Collection(_) => Err(SupervisorError::WorkerFailed),
        }
    }

    fn wait_result(self) -> Result<OperationResult, SupervisorError> {
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

/// Owns one read-only quota operation. Drop cancels and joins its process.
pub struct QuotaHandle {
    check: CheckHandle,
}

impl QuotaHandle {
    pub fn wait(self) -> Result<QuotaProbe, SupervisorError> {
        match self.check.wait_result()? {
            OperationResult::Quota(probe) => Ok(probe),
            OperationResult::Check(_)
            | OperationResult::Activity(_)
            | OperationResult::Collection(_) => Err(SupervisorError::WorkerFailed),
        }
    }

    pub fn cancel(&self) -> Result<(), SupervisorError> {
        self.check.cancel()
    }
}

/// Owns one read-only activity operation. Drop cancels and joins its process.
pub struct ActivityHandle {
    check: CheckHandle,
}

impl ActivityHandle {
    pub fn wait(self) -> Result<ActivityProbe, SupervisorError> {
        match self.check.wait_result()? {
            OperationResult::Activity(probe) => Ok(probe),
            OperationResult::Check(_)
            | OperationResult::Quota(_)
            | OperationResult::Collection(_) => Err(SupervisorError::WorkerFailed),
        }
    }

    pub fn cancel(&self) -> Result<(), SupervisorError> {
        self.check.cancel()
    }
}

/// Owns one combined collection. Drop cancels and joins its process.
pub struct CollectionHandle {
    check: CheckHandle,
}

impl CollectionHandle {
    pub fn wait(self) -> Result<CollectionProbe, SupervisorError> {
        match self.check.wait_result()? {
            OperationResult::Collection(probe) => Ok(probe),
            OperationResult::Check(_)
            | OperationResult::Quota(_)
            | OperationResult::Activity(_) => Err(SupervisorError::WorkerFailed),
        }
    }

    pub fn cancel(&self) -> Result<(), SupervisorError> {
        self.check.cancel()
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

    fn script_fixture(mode: &str, source: &str, review: &str) -> Fixture {
        use sha2::{Digest, Sha256};
        let mut fixture = Fixture::new(Mode::Hang);
        let script = source.replace("__MODE__", mode);
        fs::write(&fixture.executable.path, script.as_bytes()).unwrap();
        fixture.executable = ApprovedExecutable::from_reviewed_source_with_sha256(
            &fixture.executable.path,
            review,
            Sha256::digest(script.as_bytes()).into(),
        )
        .unwrap();
        fixture
    }

    fn login_fixture(mode: &str) -> Fixture {
        script_fixture(
            mode,
            include_str!("../tests/fixtures/synthetic-device-login.py"),
            "Owned synthetic device-code login fixture. No real authentication.",
        )
    }

    fn collection_fixture(mode: &str) -> Fixture {
        let fixture = script_fixture(
            "success",
            include_str!("../tests/fixtures/synthetic-collection-provider.py"),
            "Owned synthetic combined collection fixture. No real provider data.",
        );
        for home in &fixture.homes {
            fs::write(
                home.directory.join("synthetic-collection-config.json"),
                serde_json::to_vec(&serde_json::json!({"mode": mode})).unwrap(),
            )
            .unwrap();
        }
        fixture
    }

    fn collection_rows(fixture: &Fixture, index: usize) -> Vec<serde_json::Value> {
        fs::read_to_string(
            fixture.homes[index]
                .directory
                .join("synthetic-collection-observations.jsonl"),
        )
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
    }

    #[test]
    fn synthetic_collection_reads_both_once_between_account_checks() {
        let fixture = collection_fixture("success");
        let supervisor = Supervisor::new();
        let probe = supervisor
            .start_collection_diagnostic(&fixture.executable, &fixture.homes[0])
            .unwrap()
            .wait()
            .unwrap();
        assert_eq!(
            probe.quota.as_ref().unwrap().value.limits[0]
                .primary
                .as_ref()
                .unwrap()
                .used_percent,
            25
        );
        assert_eq!(
            probe
                .activity
                .as_ref()
                .unwrap()
                .value
                .summary
                .lifetime_tokens
                .as_ref()
                .unwrap()
                .as_str(),
            "9007199254740993"
        );
        assert_eq!(
            probe.account_evidence.identity,
            IdentityConfidence::Provisional
        );
        assert!(!format!("{probe:?}").contains("synthetic-collection@example.invalid"));
        gone(probe.process_id as libc::pid_t);
        fixture.runtime_removed(0);
        let rows = collection_rows(&fixture, 0);
        let methods: Vec<_> = rows
            .iter()
            .filter(|row| row["kind"] == "request")
            .map(|row| row["request"]["method"].as_str().unwrap())
            .collect();
        assert_eq!(
            methods,
            [
                "initialize",
                "initialized",
                "account/read",
                "account/rateLimits/read",
                "account/usage/read",
                "account/read"
            ]
        );
        assert_eq!(
            rows.iter()
                .filter(|row| row["kind"] == "start" && row["arguments"][0] == "app-server")
                .count(),
            1
        );
    }

    fn collection_stage(fixture: &Fixture, index: usize, stage: &str) {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if collection_rows(fixture, index)
                .iter()
                .any(|row| row["kind"] == stage)
            {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "Synthetic collection did not reach {stage}."
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn collection_mode(fixture: &Fixture, index: usize, mode: &str) {
        let target = fixture.homes[index]
            .directory
            .join("synthetic-collection-config.json");
        let pending = target.with_extension("pending");
        fs::write(
            &pending,
            serde_json::to_vec(&serde_json::json!({"mode": mode})).unwrap(),
        )
        .unwrap();
        fs::rename(pending, target).unwrap();
    }

    #[test]
    fn synthetic_collection_unsupported_capabilities_are_isolated_and_still_account_checked() {
        for (mode, quota, activity) in [
            ("quota_unsupported", false, true),
            ("activity_unsupported", true, false),
            ("both_unsupported", false, false),
        ] {
            let fixture = collection_fixture(mode);
            let supervisor = Supervisor::new();
            let probe = supervisor
                .start_collection_diagnostic(&fixture.executable, &fixture.homes[0])
                .unwrap()
                .wait()
                .unwrap();
            assert_eq!(probe.quota.is_some(), quota, "{mode}");
            assert_eq!(probe.activity.is_some(), activity, "{mode}");
            assert!(
                collection_rows(&fixture, 0)
                    .iter()
                    .any(|row| row["request"]["id"] == 5)
            );
            gone(probe.process_id as libc::pid_t);
            fixture.runtime_removed(0);
        }
    }

    #[test]
    fn synthetic_collection_rejects_malformed_rpc_identity_and_auth_failures_as_whole_result() {
        for mode in [
            "quota_invalid",
            "activity_invalid",
            "quota_rpc_error",
            "activity_rpc_error",
            "unknown",
            "signed_out",
            "unsupported_auth",
            "changed_email",
            "changed_plan",
            "changed_auth",
            "account_notice",
        ] {
            let fixture = collection_fixture(mode);
            let supervisor = Supervisor::new();
            let error = supervisor
                .start_collection_diagnostic(&fixture.executable, &fixture.homes[0])
                .unwrap()
                .wait()
                .err()
                .unwrap();
            let expected = match mode {
                "quota_rpc_error" | "activity_rpc_error" => ProtocolError::ProviderFailure,
                "activity_invalid" => ProtocolError::InvalidMessage,
                "unknown" => ProtocolError::AccountIdentityUnavailable,
                "signed_out" | "unsupported_auth" => ProtocolError::UnsupportedAuthentication,
                "changed_email" | "changed_plan" | "changed_auth" | "account_notice" => {
                    ProtocolError::AccountChanged
                }
                _ => ProtocolError::InvalidField,
            };
            assert_eq!(
                error,
                SupervisorError::Session(SessionError::Protocol(expected)),
                "{mode}"
            );
            gone(fixture.child(0));
            fixture.runtime_removed(0);
            let rows = collection_rows(&fixture, 0);
            if matches!(mode, "unknown" | "signed_out" | "unsupported_auth") {
                assert!(!rows.iter().any(|row| row["request"]["id"] == 3));
            }
            if mode.starts_with("quota_") {
                assert!(!rows.iter().any(|row| row["request"]["id"] == 4));
            }
        }
    }

    #[test]
    fn synthetic_collection_preserves_separate_observation_anchors_before_final_check_and_cleanup()
    {
        let fixture = collection_fixture("activity_hang");
        let supervisor = Supervisor::new();
        let started = std::time::SystemTime::now();
        let handle = supervisor
            .start_collection_diagnostic(&fixture.executable, &fixture.homes[0])
            .unwrap();
        fixture.child(0);
        collection_stage(&fixture, 0, "activity-pending");
        let activity_pending = Instant::now();
        collection_mode(&fixture, 0, "post_hold");
        collection_stage(&fixture, 0, "post-read-pending");
        let post_pending = Instant::now();
        collection_mode(&fixture, 0, "success");
        let probe = handle.wait().unwrap();
        let quota = probe.quota.as_ref().unwrap();
        let activity = probe.activity.as_ref().unwrap();
        assert!(started <= probe.started_at && probe.started_at <= quota.observed_at);
        assert!(
            quota.observed_at <= activity.observed_at && activity.observed_at <= probe.completed_at
        );
        assert!(quota.observed_monotonic <= activity_pending);
        assert!(
            activity_pending <= activity.observed_monotonic
                && activity.observed_monotonic <= post_pending
        );
        assert!(probe.completed_at <= std::time::SystemTime::now());
        gone(probe.process_id as libc::pid_t);
        fixture.runtime_removed(0);
    }

    #[test]
    fn synthetic_collection_owns_one_shared_slot_and_cancellation_drop_and_shutdown() {
        let fixture = collection_fixture("activity_hang");
        let supervisor = Supervisor::new();
        let first = supervisor
            .start_collection_diagnostic(&fixture.executable, &fixture.homes[0])
            .unwrap();
        let first_pid = fixture.child(0);
        collection_stage(&fixture, 0, "activity-pending");
        collection_mode(&fixture, 1, "quota_hang");
        let second = supervisor
            .start_quota_diagnostic(&fixture.executable, &fixture.homes[1])
            .unwrap();
        let second_pid = fixture.child(1);
        collection_stage(&fixture, 1, "quota-pending");
        assert!(matches!(
            supervisor.start_collection_diagnostic(&fixture.executable, &fixture.homes[0]),
            Err(SupervisorError::DuplicateProfile)
        ));
        assert!(matches!(
            supervisor.start_collection_diagnostic(&fixture.executable, &fixture.homes[2]),
            Err(SupervisorError::CapacityReached)
        ));
        first.cancel().unwrap();
        assert_eq!(first.wait().err(), Some(SupervisorError::Cancelled));
        gone(first_pid);
        fixture.runtime_removed(0);
        drop(second);
        gone(second_pid);
        fixture.runtime_removed(1);
        let third = supervisor
            .start_collection_diagnostic(&fixture.executable, &fixture.homes[2])
            .unwrap();
        let third_pid = fixture.child(2);
        collection_stage(&fixture, 2, "activity-pending");
        supervisor.shutdown().unwrap();
        assert_eq!(third.wait().err(), Some(SupervisorError::Cancelled));
        gone(third_pid);
        fixture.runtime_removed(2);
    }

    #[test]
    fn synthetic_collection_timeouts_at_either_capability_and_cleanup_failure_return_no_probe() {
        for mode in ["quota_hang", "activity_hang"] {
            let fixture = collection_fixture(mode);
            let supervisor = Supervisor {
                registry: Arc::default(),
                timeouts: Timeouts {
                    ready: Duration::from_secs(2),
                    read: Duration::from_millis(150),
                    login: Duration::from_secs(2),
                },
            };
            assert_eq!(
                supervisor
                    .start_collection_diagnostic(&fixture.executable, &fixture.homes[0])
                    .unwrap()
                    .wait()
                    .err(),
                Some(SupervisorError::Session(SessionError::TimedOut)),
                "{mode}"
            );
            gone(fixture.child(0));
            fixture.runtime_removed(0);
        }
        let fixture = collection_fixture("cleanup_fault");
        let supervisor = Supervisor::new();
        assert_eq!(
            supervisor
                .start_collection_diagnostic(&fixture.executable, &fixture.homes[0])
                .unwrap()
                .wait()
                .err(),
            Some(SupervisorError::Session(SessionError::CleanupFailed))
        );
        gone(fixture.child(0));
        assert_eq!(
            supervisor.shutdown(),
            Err(SupervisorError::Session(SessionError::CleanupFailed))
        );
    }

    #[test]
    fn synthetic_collection_requires_pinned_source_and_unchanged_home_and_executable() {
        let fixture = collection_fixture("success");
        let supervisor = Supervisor::new();
        let metadata = ApprovedExecutable::from_reviewed_source(
            &fixture.executable.path,
            "Synthetic metadata only.",
        )
        .unwrap();
        assert!(matches!(
            supervisor.start_collection_diagnostic(&metadata, &fixture.homes[0]),
            Err(SupervisorError::InvalidApproval)
        ));
        let home = &fixture.homes[0].directory;
        fs::rename(home, home.with_extension("retained")).unwrap();
        fs::DirBuilder::new().mode(0o700).create(home).unwrap();
        assert_eq!(
            supervisor
                .start_collection_diagnostic(&fixture.executable, &fixture.homes[0])
                .unwrap()
                .wait()
                .err(),
            Some(SupervisorError::Session(
                SessionError::InvalidManagedDirectory
            ))
        );
        fs::write(&fixture.executable.path, b"#!/bin/sh\nexit 42\n").unwrap();
        assert_eq!(
            supervisor
                .start_collection_diagnostic(&fixture.executable, &fixture.homes[1])
                .unwrap()
                .wait()
                .err(),
            Some(SupervisorError::Session(SessionError::InvalidExecutable))
        );
        assert!(
            !fixture.homes[1]
                .directory
                .join("synthetic-child-pid")
                .exists()
        );
    }

    fn quota_fixture(mode: &str) -> Fixture {
        script_fixture(
            mode,
            include_str!("../tests/fixtures/synthetic-quota-provider.py"),
            "Owned synthetic quota transport fixture. No real provider data.",
        )
    }

    #[test]
    fn synthetic_quota_reads_full_snapshot_between_account_checks_without_publishing_identity() {
        for mode in ["success", "sparse"] {
            let fixture = quota_fixture(mode);
            let supervisor = Supervisor::new();
            let metadata_only = ApprovedExecutable::from_reviewed_source(
                &fixture.executable.path,
                "Metadata-only review cannot authorize this quota diagnostic.",
            )
            .unwrap();
            assert!(matches!(
                supervisor.start_quota_diagnostic(&metadata_only, &fixture.homes[0]),
                Err(SupervisorError::InvalidApproval)
            ));
            assert!(
                !fixture.homes[0]
                    .directory
                    .join("synthetic-child-pid")
                    .exists()
            );
            let probe = supervisor
                .start_quota_diagnostic(&fixture.executable, &fixture.homes[0])
                .unwrap()
                .wait()
                .unwrap();
            assert_eq!(probe.quota.limits.len(), 1);
            assert_eq!(
                probe.quota.limits[0].primary.as_ref().unwrap().used_percent,
                25
            );
            assert_eq!(
                probe.quota.limits[0]
                    .primary
                    .as_ref()
                    .unwrap()
                    .derived_remaining_percent,
                75
            );
            assert!(probe.quota.limits[0].secondary.is_none());
            assert_eq!(
                probe.account_evidence.authentication,
                crate::AccountAuth::Chatgpt
            );
            assert!(probe.account_evidence.requires_openai_auth);
            assert_eq!(
                probe.account_evidence.identity,
                crate::IdentityConfidence::Provisional
            );
            assert_eq!(
                probe.account_evidence.email.as_deref(),
                Some("synthetic@example.invalid")
            );
            assert_eq!(probe.account_evidence.plan.as_deref(), Some("plus"));
            for private in [
                "synthetic-private-account-id",
                "synthetic@example.invalid",
                "email",
                "plus",
            ] {
                assert!(!format!("{probe:?}").contains(private));
                assert!(
                    !serde_json::to_string(&probe.quota)
                        .unwrap()
                        .contains(private)
                );
            }
            gone(probe.process_id as libc::pid_t);
            fixture.runtime_removed(0);
            let requests: Vec<serde_json::Value> = fs::read_to_string(
                fixture.homes[0]
                    .directory
                    .join("synthetic-quota-requests.jsonl"),
            )
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
            assert_eq!(
                requests
                    .iter()
                    .filter_map(|request| request["method"].as_str())
                    .collect::<Vec<_>>(),
                [
                    "initialize",
                    "initialized",
                    "account/read",
                    "account/rateLimits/read",
                    "account/read"
                ]
            );
            assert_eq!(requests[3]["params"], serde_json::json!({}));
            assert!(
                !fixture.homes[1]
                    .directory
                    .join("synthetic-child-pid")
                    .exists()
            );
        }
    }

    #[test]
    fn synthetic_quota_observation_precedes_a_held_post_read_and_later_completion() {
        let fixture = quota_fixture("post_hold");
        let supervisor = Supervisor::new();
        let started_at = std::time::SystemTime::now();
        let started_monotonic = Instant::now();
        let quota = supervisor
            .start_quota_diagnostic(&fixture.executable, &fixture.homes[0])
            .unwrap();
        let home = &fixture.homes[0].directory;
        let deadline = Instant::now() + Duration::from_secs(5);
        while !home.join("synthetic-quota-post-read-pending").exists() {
            assert!(
                Instant::now() < deadline,
                "The synthetic post-read barrier was not reached."
            );
            std::thread::yield_now();
        }
        // The provider received the post-read only after the full quota response was parsed.
        let held_at = std::time::SystemTime::now();
        let held_monotonic = Instant::now();
        fs::write(home.join("synthetic-quota-release"), b"release").unwrap();
        let probe = quota.wait().unwrap();
        let returned_at = std::time::SystemTime::now();
        let returned_monotonic = Instant::now();
        assert!(probe.started_at >= started_at && probe.started_at <= probe.observed_at);
        assert!(probe.observed_at <= held_at);
        assert!(probe.completed_at >= held_at && probe.completed_at <= returned_at);
        assert!(probe.observed_monotonic >= started_monotonic);
        assert!(probe.observed_monotonic <= held_monotonic);
        assert!(held_monotonic < returned_monotonic);
        assert_eq!(
            probe.quota.limits[0].primary.as_ref().unwrap().used_percent,
            25
        );
        gone(probe.process_id as libc::pid_t);
        fixture.runtime_removed(0);
    }

    #[test]
    fn synthetic_quota_refuses_unknown_unsupported_changed_auth_and_invalid_bucket_evidence() {
        for (mode, error) in [
            ("unknown", ProtocolError::AccountIdentityUnavailable),
            ("unsupported", ProtocolError::UnsupportedAuthentication),
            ("signed_out", ProtocolError::UnsupportedAuthentication),
            ("changed_email", ProtocolError::AccountChanged),
            ("changed_plan", ProtocolError::AccountChanged),
            ("changed_auth", ProtocolError::AccountChanged),
            ("post_unknown", ProtocolError::AccountChanged),
            ("early_auth", ProtocolError::AccountChanged),
            ("auth_notice", ProtocolError::AccountChanged),
            ("late_auth", ProtocolError::AccountChanged),
            ("invalid", ProtocolError::InvalidField),
            ("mismatch", ProtocolError::IncompatibleQuotaScope),
        ] {
            let fixture = quota_fixture(mode);
            let supervisor = Supervisor::new();
            let result = supervisor
                .start_quota_diagnostic(&fixture.executable, &fixture.homes[0])
                .unwrap()
                .wait();
            assert_eq!(
                result.err(),
                Some(SupervisorError::Session(SessionError::Protocol(error))),
                "{mode}"
            );
            gone(fixture.child(0));
            fixture.runtime_removed(0);
            let requests = fs::read_to_string(
                fixture.homes[0]
                    .directory
                    .join("synthetic-quota-requests.jsonl"),
            )
            .unwrap();
            if ["unknown", "unsupported", "signed_out", "early_auth"].contains(&mode) {
                assert!(!requests.contains("account/rateLimits/read"));
            }
        }
    }

    #[test]
    fn synthetic_quota_shares_slots_with_login_and_check_and_owns_cancel_drop_timeout_shutdown() {
        let fixture = quota_fixture("hang");
        let login_fixture = login_fixture("pending");
        let supervisor = Supervisor::new();
        let quota = supervisor
            .start_quota_diagnostic(&fixture.executable, &fixture.homes[0])
            .unwrap();
        let quota_pid = fixture.child(0);
        let mut login = supervisor
            .start_device_login(&login_fixture.executable, &fixture.homes[1])
            .unwrap();
        login.prompt().unwrap();
        let login_pid = fixture.child(1);
        let deadline = Instant::now() + Duration::from_secs(5);
        while !fixture.homes[0]
            .directory
            .join("synthetic-quota-pending")
            .exists()
        {
            assert!(
                Instant::now() < deadline,
                "The synthetic quota read did not start."
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(matches!(
            supervisor.start_check(&fixture.executable, &fixture.homes[0]),
            Err(SupervisorError::DuplicateProfile)
        ));
        assert!(matches!(
            supervisor.start_check(&fixture.executable, &fixture.homes[2]),
            Err(SupervisorError::CapacityReached)
        ));
        quota.cancel().unwrap();
        assert_eq!(quota.wait().err(), Some(SupervisorError::Cancelled));
        gone(quota_pid);
        fixture.runtime_removed(0);
        assert_eq!(unsafe { libc::kill(login_pid, 0) }, 0);
        login.cancel().unwrap();
        assert_eq!(login.wait().err(), Some(SupervisorError::Cancelled));
        gone(login_pid);
        let dropped = supervisor
            .start_quota_diagnostic(&fixture.executable, &fixture.homes[2])
            .unwrap();
        let dropped_pid = fixture.child(2);
        drop(dropped);
        gone(dropped_pid);
        fixture.runtime_removed(2);
        // Remove only a synthetic marker so the next owned child cannot be mistaken for the prior PID.
        fs::remove_file(fixture.homes[2].directory.join("synthetic-child-pid")).unwrap();
        let shutdown = supervisor
            .start_quota_diagnostic(&fixture.executable, &fixture.homes[2])
            .unwrap();
        let shutdown_pid = fixture.child(2);
        supervisor.shutdown().unwrap();
        assert_eq!(shutdown.wait().err(), Some(SupervisorError::Cancelled));
        gone(shutdown_pid);
        fixture.runtime_removed(2);
        let bounded = Supervisor {
            registry: Arc::default(),
            timeouts: Timeouts {
                ready: Duration::from_secs(2),
                read: Duration::from_millis(150),
                login: Duration::from_secs(2),
            },
        };
        fs::remove_file(fixture.homes[0].directory.join("synthetic-child-pid")).unwrap();
        fs::remove_file(
            fixture.homes[0]
                .directory
                .join("synthetic-quota-requests.jsonl"),
        )
        .unwrap();
        assert_eq!(
            bounded
                .start_quota_diagnostic(&fixture.executable, &fixture.homes[0])
                .unwrap()
                .wait()
                .err(),
            Some(SupervisorError::Session(SessionError::TimedOut))
        );
        gone(fixture.child(0));
        fixture.runtime_removed(0);
        let timed_out_requests = fs::read_to_string(
            fixture.homes[0]
                .directory
                .join("synthetic-quota-requests.jsonl"),
        )
        .unwrap();
        assert!(timed_out_requests.contains("account/rateLimits/read"));
        assert!(
            !timed_out_requests
                .lines()
                .any(|line| serde_json::from_str::<serde_json::Value>(line).unwrap()["id"] == 4)
        );
    }

    fn activity_fixture(mode: &str) -> Fixture {
        let fixture = script_fixture(
            mode,
            include_str!("../../../tests/native/fixtures/synthetic-activity-provider.py"),
            "Owned static synthetic activity fixture. No real provider data.",
        );
        activity_mode(&fixture, mode);
        fixture
    }

    fn activity_mode(fixture: &Fixture, mode: &str) {
        fs::write(
            fixture
                .temporary
                .path()
                .join("synthetic-activity-provider-config.json"),
            serde_json::to_vec(&serde_json::json!({"defaultMode":mode})).unwrap(),
        )
        .unwrap();
    }

    fn activity_rows(fixture: &Fixture) -> Vec<serde_json::Value> {
        match fs::read_to_string(
            fixture
                .temporary
                .path()
                .join("synthetic-activity-observations.jsonl"),
        ) {
            Ok(value) => value
                .lines()
                .map(|line| serde_json::from_str(line).unwrap())
                .collect(),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(error) => panic!("Synthetic activity observations failed: {error}"),
        }
    }

    fn activity_stage(fixture: &Fixture, index: usize, kind: &str, offset: usize) -> libc::pid_t {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let rows = activity_rows(fixture);
            for row in rows.iter().skip(offset).filter(|row| row["kind"] == kind) {
                if rows.iter().any(|start| {
                    start["kind"] == "start"
                        && start["pid"] == row["pid"]
                        && start["environment"]["CODEX_HOME"]
                            == fixture.homes[index].directory.to_str().unwrap()
                }) {
                    return row["pid"].as_i64().unwrap() as libc::pid_t;
                }
            }
            assert!(
                Instant::now() < deadline,
                "Synthetic activity stage was not reached: {kind}"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn activity_cleaned(fixture: &Fixture, pid: libc::pid_t) {
        gone(pid);
        let rows = activity_rows(fixture);
        let start = rows
            .iter()
            .find(|row| row["kind"] == "start" && row["pid"] == pid)
            .unwrap();
        let runtime = Path::new(start["environment"]["HOME"].as_str().unwrap())
            .parent()
            .unwrap();
        assert!(runtime.starts_with(fixture.temporary.path()));
        assert!(!runtime.exists());
    }

    #[test]
    fn synthetic_activity_exact_full_reads_preserve_scope_nulls_dates_and_redact_diagnostics() {
        for mode in [
            "success",
            "sparse",
            "null_summary",
            "null_buckets",
            "empty_buckets",
            "bound",
        ] {
            let fixture = activity_fixture(mode);
            let supervisor = Supervisor::new();
            let metadata_only = ApprovedExecutable::from_reviewed_source(
                &fixture.executable.path,
                "Metadata review cannot authorize account activity.",
            )
            .unwrap();
            assert!(matches!(
                supervisor.start_activity_diagnostic(&metadata_only, &fixture.homes[0]),
                Err(SupervisorError::InvalidApproval)
            ));
            assert!(activity_rows(&fixture).is_empty());
            let marker = fixture.homes[0].directory.join("provider-owned-marker");
            fs::write(&marker, b"Preserve this owned synthetic data.").unwrap();
            let probe = supervisor
                .start_activity_diagnostic(&fixture.executable, &fixture.homes[0])
                .unwrap()
                .wait()
                .unwrap();
            let value = serde_json::to_value(&probe.activity).unwrap();
            assert_eq!(
                value["scope"],
                "account_activity_with_unknown_application_scope"
            );
            if mode == "null_summary" {
                assert!(
                    value["summary"]
                        .as_object()
                        .unwrap()
                        .values()
                        .all(serde_json::Value::is_null)
                );
            } else {
                assert_eq!(value["summary"]["lifetimeTokens"], "9007199254740993");
                assert_eq!(value["summary"]["peakDailyTokens"], "9223372036854775807");
                assert_eq!(value["summary"]["currentStreakDays"], "0");
                assert_eq!(
                    value["summary"]["longestRunningTurnSec"],
                    serde_json::Value::Null
                );
            }
            match mode {
                "null_buckets" => assert!(value["dailyUsageBuckets"].is_null()),
                "empty_buckets" => assert_eq!(value["dailyUsageBuckets"], serde_json::json!([])),
                "bound" => assert_eq!(
                    value["dailyUsageBuckets"].as_array().unwrap().len(),
                    crate::MAX_ACTIVITY_DAILY_BUCKETS
                ),
                _ => {
                    assert_eq!(value["dailyUsageBuckets"][0]["startDate"], "2024-02-29");
                    assert_eq!(value["dailyUsageBuckets"][0]["tokens"], "9007199254740993");
                    assert_eq!(value["dailyUsageBuckets"][1]["tokens"], "0");
                }
            }
            let mut debug = format!(
                "{probe:?} {:?} {:?}",
                probe.activity, probe.activity.summary
            );
            if let Some(rows) = &probe.activity.daily_usage_buckets {
                debug.push_str(&format!("{rows:?}"));
            }
            if let Some(count) = &probe.activity.summary.lifetime_tokens {
                debug.push_str(&format!("{count:?}"));
            }
            for private in [
                "9007199254740993",
                "9223372036854775807",
                "2024-02-29",
                "synthetic-activity@example.invalid",
                "synthetic-private-activity-id",
                "synthetic private activity provider",
            ] {
                assert!(
                    !debug.contains(private),
                    "Activity diagnostics must redact provider content."
                );
            }
            let encoded = serde_json::to_string(&probe.activity).unwrap();
            for private in [
                "accountId",
                "synthetic-private-activity-id",
                "workTokens",
                "modelCounts",
                "synthetic-private-model",
                "threadUsage",
                "email",
            ] {
                assert!(!encoded.contains(private));
            }
            let rows = activity_rows(&fixture);
            let requests: Vec<_> = rows
                .iter()
                .filter(|row| row["kind"] == "request")
                .map(|row| row["request"].clone())
                .collect();
            assert_eq!(
                requests
                    .iter()
                    .map(|row| (row["method"].as_str().unwrap(), row["id"].as_u64()))
                    .collect::<Vec<_>>(),
                [
                    ("initialize", Some(1)),
                    ("initialized", None),
                    ("account/read", Some(2)),
                    ("account/usage/read", Some(3)),
                    ("account/read", Some(4))
                ]
            );
            assert_eq!(
                requests[0]["params"]["capabilities"]["experimentalApi"],
                false
            );
            assert_eq!(
                requests[2]["params"],
                serde_json::json!({"refreshToken":false})
            );
            assert_eq!(requests[3]["params"], serde_json::json!({}));
            assert_eq!(
                requests[4]["params"],
                serde_json::json!({"refreshToken":false})
            );
            for start in rows.iter().filter(|row| row["kind"] == "start") {
                assert_eq!(start["unexpectedEnvironmentKeys"], serde_json::json!([]));
                assert_eq!(start["environment"]["PATH"], "/usr/bin:/bin");
                assert_eq!(
                    start["environment"]["CODEX_HOME"],
                    fixture.homes[0].directory.to_str().unwrap()
                );
            }
            activity_cleaned(&fixture, probe.process_id as libc::pid_t);
            assert_eq!(
                fs::read(&marker).unwrap(),
                b"Preserve this owned synthetic data."
            );
            assert_eq!(
                fs::read_dir(&fixture.homes[0].directory).unwrap().count(),
                1
            );
            assert!(
                fs::read_dir(&fixture.homes[1].directory)
                    .unwrap()
                    .next()
                    .is_none()
            );
        }
    }

    #[test]
    fn synthetic_activity_observation_precedes_the_held_final_account_response() {
        let fixture = activity_fixture("post_hold");
        let supervisor = Supervisor::new();
        let started_at = std::time::SystemTime::now();
        let started_monotonic = Instant::now();
        let activity = supervisor
            .start_activity_diagnostic(&fixture.executable, &fixture.homes[0])
            .unwrap();
        let pid = activity_stage(&fixture, 0, "post-read-pending", 0);
        let held_at = std::time::SystemTime::now();
        let held_monotonic = Instant::now();
        activity_mode(&fixture, "success");
        let probe = activity.wait().unwrap();
        let returned_monotonic = Instant::now();
        assert!(probe.observed_at >= started_at && probe.observed_at <= held_at);
        assert!(probe.started_at >= started_at && probe.started_at <= probe.observed_at);
        assert!(probe.completed_at >= held_at);
        assert!(probe.observed_monotonic >= started_monotonic);
        assert!(probe.observed_monotonic <= held_monotonic);
        assert!(held_monotonic < returned_monotonic);
        assert_eq!(
            probe.account_evidence.email.as_deref(),
            Some("synthetic-activity@example.invalid")
        );
        assert_eq!(
            probe.account_evidence.identity,
            crate::IdentityConfidence::Provisional
        );

        assert_eq!(
            serde_json::to_value(&probe.activity).unwrap()["summary"]["lifetimeTokens"],
            "9007199254740993"
        );
        activity_cleaned(&fixture, pid);
    }

    #[test]
    fn synthetic_activity_rejects_auth_changes_invalid_fields_thread_scope_and_excess_rows() {
        for (mode, error) in [
            ("unknown", ProtocolError::AccountIdentityUnavailable),
            ("unsupported", ProtocolError::UnsupportedAuthentication),
            ("signed_out", ProtocolError::UnsupportedAuthentication),
            ("changed_email", ProtocolError::AccountChanged),
            ("changed_plan", ProtocolError::AccountChanged),
            ("changed_auth", ProtocolError::AccountChanged),
            ("post_unknown", ProtocolError::AccountChanged),
            ("early_auth", ProtocolError::AccountChanged),
            ("auth_notice", ProtocolError::AccountChanged),
            ("late_auth", ProtocolError::AccountChanged),
            ("negative", ProtocolError::InvalidField),
            ("fractional", ProtocolError::InvalidMessage),
            ("string_count", ProtocolError::InvalidMessage),
            ("overflow", ProtocolError::InvalidMessage),
            ("missing_summary", ProtocolError::InvalidMessage),
            ("invalid_date", ProtocolError::InvalidField),
            ("duplicate_date", ProtocolError::InvalidField),
            ("too_many", ProtocolError::InvalidField),
            ("thread_scope", ProtocolError::IncompatibleUsageScope),
            ("wrong_id", ProtocolError::MismatchedRequest),
            ("rpc_error", ProtocolError::ProviderFailure),
            ("unsupported_method", ProtocolError::UnsupportedMethod),
            ("malformed", ProtocolError::InvalidMessage),
            ("oversized", ProtocolError::OversizedMessage),
        ] {
            let fixture = activity_fixture(mode);
            let supervisor = Supervisor::new();
            let result = supervisor
                .start_activity_diagnostic(&fixture.executable, &fixture.homes[0])
                .unwrap()
                .wait();
            assert_eq!(
                result.err(),
                Some(SupervisorError::Session(SessionError::Protocol(error))),
                "{mode}"
            );
            let rows = activity_rows(&fixture);
            let start = rows
                .iter()
                .find(|row| row["kind"] == "start" && row["arguments"][0] == "app-server")
                .unwrap();
            activity_cleaned(&fixture, start["pid"].as_i64().unwrap() as libc::pid_t);
            if ["unknown", "unsupported", "signed_out", "early_auth"].contains(&mode) {
                assert!(
                    !rows
                        .iter()
                        .any(|row| row["request"]["method"] == "account/usage/read")
                );
            }
        }
    }

    #[test]
    fn synthetic_activity_shared_slots_and_stale_handle_cancellation_preserve_the_successor() {
        let fixture = activity_fixture("pending");
        let quota_fixture = quota_fixture("hang");
        let supervisor = Supervisor::new();
        let first = supervisor
            .start_activity_diagnostic(&fixture.executable, &fixture.homes[0])
            .unwrap();
        let first_pid = activity_stage(&fixture, 0, "activity-read-pending", 0);
        let quota = supervisor
            .start_quota_diagnostic(&quota_fixture.executable, &fixture.homes[1])
            .unwrap();
        let quota_pid = fixture.child(1);
        assert!(matches!(
            supervisor.start_check(&fixture.executable, &fixture.homes[0]),
            Err(SupervisorError::DuplicateProfile)
        ));
        assert!(matches!(
            supervisor.start_activity_diagnostic(&fixture.executable, &fixture.homes[2]),
            Err(SupervisorError::CapacityReached)
        ));
        first.cancel().unwrap();
        activity_cleaned(&fixture, first_pid);
        let offset = activity_rows(&fixture).len();
        let successor = supervisor
            .start_activity_diagnostic(&fixture.executable, &fixture.homes[0])
            .unwrap();
        let successor_pid = activity_stage(&fixture, 0, "activity-read-pending", offset);
        first.cancel().unwrap();
        assert_eq!(first.wait().err(), Some(SupervisorError::Cancelled));
        assert_eq!(unsafe { libc::kill(successor_pid, 0) }, 0);
        assert_eq!(unsafe { libc::kill(quota_pid, 0) }, 0);
        assert_eq!(supervisor.cancel(&fixture.homes[0].profile_id), Ok(true));
        assert_eq!(successor.wait().err(), Some(SupervisorError::Cancelled));
        activity_cleaned(&fixture, successor_pid);
        quota.cancel().unwrap();
        assert_eq!(quota.wait().err(), Some(SupervisorError::Cancelled));
        gone(quota_pid);
        fixture.runtime_removed(1);
    }

    #[test]
    fn synthetic_activity_drop_shutdown_and_profile_binding_own_only_their_processes() {
        let fixture = activity_fixture("pending");
        let login_fixture = login_fixture("pending");
        let supervisor = Supervisor::new();
        let dropped = supervisor
            .start_activity_diagnostic(&fixture.executable, &fixture.homes[0])
            .unwrap();
        let pid = activity_stage(&fixture, 0, "activity-read-pending", 0);
        drop(dropped);
        activity_cleaned(&fixture, pid);
        let alias = ManagedProfileHome::new(
            fixture.homes[1].profile_id.clone(),
            fixture.homes[0].profile_id.as_str(),
            &fixture.homes[0].directory,
        )
        .unwrap();
        assert!(matches!(
            supervisor.start_activity_diagnostic(&fixture.executable, &alias),
            Err(SupervisorError::HomeInUse)
        ));
        let original = fixture.temporary.path().join("preserved-old-home");
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
            supervisor.start_activity_diagnostic(&fixture.executable, &replacement),
            Err(SupervisorError::HomeChanged)
        ));
        let activity = supervisor
            .start_activity_diagnostic(&fixture.executable, &fixture.homes[1])
            .unwrap();
        let activity_pid = activity_stage(&fixture, 1, "activity-read-pending", 0);
        let mut login = supervisor
            .start_device_login(&login_fixture.executable, &fixture.homes[2])
            .unwrap();
        login.prompt().unwrap();
        let login_pid = fixture.child(2);
        supervisor.shutdown().unwrap();
        assert_eq!(activity.wait().err(), Some(SupervisorError::Cancelled));
        assert_eq!(login.wait().err(), Some(SupervisorError::Cancelled));
        activity_cleaned(&fixture, activity_pid);
        gone(login_pid);
        fixture.runtime_removed(2);
        assert!(matches!(
            supervisor.start_activity_diagnostic(&fixture.executable, &fixture.homes[1]),
            Err(SupervisorError::Closed)
        ));
    }

    #[test]
    fn synthetic_activity_deadlines_and_cleanup_faults_reap_the_owned_child() {
        for mode in ["pending", "partial"] {
            let fixture = activity_fixture(mode);
            let supervisor = Supervisor {
                registry: Arc::default(),
                timeouts: Timeouts {
                    ready: Duration::from_secs(2),
                    read: Duration::from_millis(150),
                    login: Duration::from_secs(2),
                },
            };
            assert_eq!(
                supervisor
                    .start_activity_diagnostic(&fixture.executable, &fixture.homes[0])
                    .unwrap()
                    .wait()
                    .err(),
                Some(SupervisorError::Session(SessionError::TimedOut))
            );
            let pid = activity_stage(&fixture, 0, "activity-read-pending", 0);
            activity_cleaned(&fixture, pid);
            assert!(
                !activity_rows(&fixture)
                    .iter()
                    .any(|row| row["request"]["id"] == 4)
            );
        }
        let fixture = activity_fixture("cleanup_fault");
        let supervisor = Supervisor::new();
        assert_eq!(
            supervisor
                .start_activity_diagnostic(&fixture.executable, &fixture.homes[0])
                .unwrap()
                .wait()
                .err(),
            Some(SupervisorError::Session(SessionError::CleanupFailed))
        );
        let pid = activity_stage(&fixture, 0, "activity-read-pending", 0);
        gone(pid);
        assert_eq!(
            supervisor.shutdown(),
            Err(SupervisorError::Session(SessionError::CleanupFailed))
        );
    }

    fn login_requests(fixture: &Fixture, index: usize) -> Vec<serde_json::Value> {
        fs::read_to_string(
            fixture.homes[index]
                .directory
                .join("synthetic-login-requests.jsonl"),
        )
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
    }

    #[test]
    fn synthetic_device_login_retains_early_completion_and_reads_selected_account_afterward() {
        let fixture = login_fixture("early");
        let supervisor = Supervisor::new();
        let metadata_only = ApprovedExecutable::from_reviewed_source(
            &fixture.executable.path,
            "Legacy metadata-only synthetic review.",
        )
        .unwrap();
        assert!(matches!(
            supervisor.start_device_login(&metadata_only, &fixture.homes[0]),
            Err(SupervisorError::InvalidApproval)
        ));
        assert!(
            !fixture.homes[0]
                .directory
                .join("synthetic-child-pid")
                .exists()
        );
        fs::write(
            fixture.homes[1].directory.join("preserved-marker"),
            b"other profile",
        )
        .unwrap();
        let mut login = supervisor
            .start_device_login(&fixture.executable, &fixture.homes[0])
            .unwrap();
        let prompt = login.prompt().unwrap();
        assert_eq!(
            prompt.verification_url.as_str(),
            "https://auth.openai.com/codex/device"
        );
        assert_eq!(prompt.user_code, "SYNTH-1234");
        assert!(matches!(login.prompt(), Err(SupervisorError::WorkerFailed)));
        let probe = login.wait().unwrap();
        assert_eq!(probe.account.authentication, AccountAuth::Chatgpt);
        assert_eq!(probe.account.identity, IdentityConfidence::Provisional);
        gone(probe.process_id as libc::pid_t);
        let requests = login_requests(&fixture, 0);
        assert_eq!(
            requests
                .iter()
                .filter_map(|request| request["method"].as_str())
                .collect::<Vec<_>>(),
            [
                "initialize",
                "initialized",
                "account/login/start",
                "account/read"
            ]
        );
        assert_eq!(requests[2]["params"]["type"], "chatgptDeviceCode");
        assert_eq!(requests[3]["params"]["refreshToken"], false);
        assert_eq!(
            fs::read(fixture.homes[1].directory.join("preserved-marker")).unwrap(),
            b"other profile"
        );
        assert_eq!(
            fs::read_dir(&fixture.homes[1].directory).unwrap().count(),
            1
        );
        fixture.runtime_removed(0);
    }

    #[test]
    fn synthetic_device_login_refuses_unmatched_failed_and_missing_account_completions() {
        for mode in [
            "wrong",
            "null",
            "failed",
            "account_missing",
            "bad_url",
            "bad_code",
            "flood",
        ] {
            let fixture = login_fixture(mode);
            let supervisor = Supervisor::new();
            let mut login = supervisor
                .start_device_login(&fixture.executable, &fixture.homes[0])
                .unwrap();
            let prompt = login.prompt();
            let error = login.wait().err().unwrap();
            assert!(
                matches!(
                    error,
                    SupervisorError::Session(
                        SessionError::Protocol(_) | SessionError::NotificationLimit
                    )
                ),
                "{mode}: {error:?}"
            );
            if ["bad_url", "bad_code", "flood"].contains(&mode) {
                assert!(prompt.is_err());
            } else {
                assert!(prompt.is_ok());
            }
            let requests = login_requests(&fixture, 0);
            assert_eq!(
                requests
                    .iter()
                    .any(|request| request["method"] == "account/read"),
                mode == "account_missing"
            );
            gone(fixture.child(0));
            fixture.runtime_removed(0);
        }
    }

    #[test]
    fn synthetic_device_login_shares_slots_and_cancel_and_shutdown_reap_owned_processes() {
        let fixture = login_fixture("pending");
        let supervisor = Supervisor::new();
        let mut login = supervisor
            .start_device_login(&fixture.executable, &fixture.homes[0])
            .unwrap();
        login.prompt().unwrap();
        let check = supervisor
            .start_check(&fixture.executable, &fixture.homes[1])
            .unwrap();
        let first_pid = fixture.child(0);
        let second_pid = fixture.child(1);
        assert!(matches!(
            supervisor.start_check(&fixture.executable, &fixture.homes[0]),
            Err(SupervisorError::DuplicateProfile)
        ));
        assert!(matches!(
            supervisor.start_device_login(&fixture.executable, &fixture.homes[2]),
            Err(SupervisorError::CapacityReached)
        ));
        assert_eq!(supervisor.cancel(&fixture.homes[0].profile_id), Ok(true));
        assert!(matches!(login.wait(), Err(SupervisorError::Cancelled)));
        let requests = login_requests(&fixture, 0);
        let cancel = requests
            .iter()
            .find(|request| request["method"] == "account/login/cancel")
            .unwrap();
        assert_eq!(cancel["params"]["loginId"], "synthetic-device-attempt");
        assert!(
            !requests
                .iter()
                .any(|request| request["method"] == "account/read"
                    || request["method"] == "account/logout")
        );
        gone(first_pid);
        let mut third = supervisor
            .start_device_login(&fixture.executable, &fixture.homes[2])
            .unwrap();
        third.prompt().unwrap();
        let third_pid = fixture.child(2);
        supervisor.shutdown().unwrap();
        assert!(matches!(third.wait(), Err(SupervisorError::Cancelled)));
        assert!(matches!(check.wait(), Err(SupervisorError::Cancelled)));
        gone(second_pid);
        gone(third_pid);
        for index in 0..3 {
            fixture.runtime_removed(index);
        }
    }

    #[test]
    fn synthetic_device_login_timeout_and_dropped_handle_cancel_even_without_acknowledgment() {
        for mode in ["updated_only", "cancel_unresponsive"] {
            let fixture = login_fixture(mode);
            let mut supervisor = Supervisor::new();
            supervisor.timeouts.login = Duration::from_millis(100);
            let mut login = supervisor
                .start_device_login(&fixture.executable, &fixture.homes[0])
                .unwrap();
            login.prompt().unwrap();
            let pid = fixture.child(0);
            let started = Instant::now();
            assert!(matches!(
                login.wait(),
                Err(SupervisorError::Session(SessionError::TimedOut))
            ));
            assert!(started.elapsed() < Duration::from_secs(3));
            assert!(
                login_requests(&fixture, 0)
                    .iter()
                    .any(|request| request["method"] == "account/login/cancel")
            );
            gone(pid);
            fixture.runtime_removed(0);
        }
        let fixture = login_fixture("pending");
        let supervisor = Supervisor::new();
        let mut login = supervisor
            .start_device_login(&fixture.executable, &fixture.homes[0])
            .unwrap();
        login.prompt().unwrap();
        let pid = fixture.child(0);
        drop(login);
        gone(pid);
        fixture.runtime_removed(0);

        let fixture = login_fixture("start_hang");
        let supervisor = Supervisor::new();
        let mut login = supervisor
            .start_device_login(&fixture.executable, &fixture.homes[0])
            .unwrap();
        let pid = fixture.child(0);
        let deadline = Instant::now() + Duration::from_secs(5);
        while !fixture.homes[0]
            .directory
            .join("synthetic-login-start-pending")
            .exists()
        {
            assert!(
                Instant::now() < deadline,
                "The synthetic login request did not arrive."
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        let started = Instant::now();
        supervisor.cancel(&fixture.homes[0].profile_id).unwrap();
        assert!(matches!(
            login.prompt(),
            Err(SupervisorError::Session(SessionError::Cancelled))
                | Err(SupervisorError::Cancelled)
        ));
        assert!(matches!(login.wait(), Err(SupervisorError::Cancelled)));
        assert!(started.elapsed() < Duration::from_secs(2));
        gone(pid);
        fixture.runtime_removed(0);
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
                ..Timeouts::default()
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
    fn source_path_policy_refuses_an_unmapped_root_owner() {
        use std::os::unix::fs::MetadataExt;
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("nonexecuted-synthetic-source");
        fs::write(&path, b"synthetic metadata only").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        let root_uid = fs::metadata("/").unwrap().uid();
        let effective_uid = unsafe { libc::geteuid() };
        let result = validate_executable_source_path(&path);
        if root_uid != 0 && root_uid != effective_uid {
            assert!(matches!(
                result,
                Err(SupervisorError::Session(SessionError::InvalidExecutable))
            ));
            println!(
                "Ancestor policy refused unmapped filesystem-root owner {root_uid} for effective UID {effective_uid}."
            );
        } else {
            result.unwrap();
        }
    }

    #[test]
    fn digest_approval_rejects_wrong_bytes_and_retains_the_approved_inode() {
        use sha2::{Digest, Sha256};
        use std::io::{Seek, SeekFrom};
        let fixture = Fixture::new(Mode::Success);
        let path = &fixture.executable.path;
        let original = fs::read(path).unwrap();
        let expected: [u8; 32] = Sha256::digest(&original).into();
        let mut wrong = expected;
        wrong[0] ^= 1;
        assert!(matches!(
            ApprovedExecutable::from_reviewed_source_with_sha256(
                path,
                "Synthetic digest review.",
                wrong
            ),
            Err(SupervisorError::Session(SessionError::InvalidExecutable))
        ));
        let approved = ApprovedExecutable::from_reviewed_source_with_sha256(
            path,
            "Synthetic digest review.",
            expected,
        )
        .unwrap();
        // File clones share a cursor. Hashing must use independent offsets, including concurrent jobs.
        approved
            .pinned
            .as_ref()
            .unwrap()
            .file
            .try_clone()
            .unwrap()
            .seek(SeekFrom::End(0))
            .unwrap();
        std::thread::scope(|scope| {
            for _ in 0..2 {
                let approved = approved.clone();
                scope.spawn(move || {
                    for _ in 0..8 {
                        approved.recheck().unwrap();
                    }
                });
            }
        });
        let mut changed = original.clone();
        *changed.last_mut().unwrap() ^= 1;
        fs::write(path, changed).unwrap();
        let mut refreshed_metadata = approved.clone();
        // Exercise the digest guard independently of stat changes. Public callers cannot change this fence.
        refreshed_metadata.identity = executable_identity(path).unwrap();
        assert_eq!(
            refreshed_metadata.recheck(),
            Err(SessionError::InvalidExecutable)
        );
        fs::write(path, &original).unwrap();
        let reviewed_again = ApprovedExecutable::from_reviewed_source_with_sha256(
            path,
            "Synthetic second review.",
            expected,
        )
        .unwrap();
        let old = fixture.temporary.path().join("retained-original");
        fs::rename(path, &old).unwrap();
        fs::copy(&old, path).unwrap();
        assert_eq!(
            reviewed_again.recheck(),
            Err(SessionError::InvalidExecutable)
        );
        assert_eq!(fs::read(&old).unwrap(), original);
        assert!(
            fs::read_dir(&fixture.homes[0].directory)
                .unwrap()
                .next()
                .is_none()
        );
        let unsafe_parent = fixture.temporary.path().join("replaceable-ancestor");
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&unsafe_parent)
            .unwrap();
        let nested = unsafe_parent.join("reviewed-provider");
        fs::copy(&old, &nested).unwrap();
        let nested_approval = ApprovedExecutable::from_reviewed_source_with_sha256(
            &nested,
            "Synthetic ancestor review.",
            expected,
        )
        .unwrap();
        fs::set_permissions(&unsafe_parent, fs::Permissions::from_mode(0o770)).unwrap();
        assert!(
            ApprovedExecutable::from_reviewed_source_with_sha256(
                &nested,
                "Synthetic permissive ancestor review.",
                expected
            )
            .is_err()
        );
        let supervisor = Supervisor::new();
        assert!(matches!(
            supervisor
                .start_check(&nested_approval, &fixture.homes[0])
                .unwrap()
                .wait(),
            Err(SupervisorError::Session(SessionError::InvalidExecutable))
        ));
        assert!(
            fs::read_dir(&fixture.homes[0].directory)
                .unwrap()
                .next()
                .is_none(),
            "A writable ancestor must fail before either provider process starts."
        );
        supervisor.shutdown().unwrap();
        // A sticky directory protects its immediately owned child, including standard /tmp fixtures.
        fs::set_permissions(&unsafe_parent, fs::Permissions::from_mode(0o1777)).unwrap();
        validate_executable_source_path(&nested).unwrap();
        fs::set_permissions(&unsafe_parent, fs::Permissions::from_mode(0o700)).unwrap();
        let oversized = fixture
            .temporary
            .path()
            .join("oversized-synthetic-provider");
        let file = fs::File::create(&oversized).unwrap();
        file.set_len(MAX_APPROVED_EXECUTABLE_BYTES + 1).unwrap();
        fs::set_permissions(&oversized, fs::Permissions::from_mode(0o700)).unwrap();
        assert!(
            ApprovedExecutable::from_reviewed_source_with_sha256(
                &oversized,
                "Synthetic oversized review.",
                expected
            )
            .is_err()
        );
        let linked = fixture.temporary.path().join("linked-approved-provider");
        symlink(&old, &linked).unwrap();
        assert!(
            ApprovedExecutable::from_reviewed_source_with_sha256(
                &linked,
                "Synthetic linked review.",
                expected
            )
            .is_err()
        );
    }

    #[test]
    fn digest_approval_checks_both_spawn_boundaries_and_supports_two_owned_checks() {
        use sha2::{Digest, Sha256};
        let fixture = Fixture::new(Mode::Success);
        let original = fs::read_to_string(&fixture.executable.path).unwrap();
        let script = original.replace("  printf 'codex-cli 0.163.0\\n'", "  printf '# changed after the version process started\\n' >> \"$0\"\n  printf 'codex-cli 0.163.0\\n'");
        assert_ne!(script, original);
        fs::write(&fixture.executable.path, &script).unwrap();
        let approved = ApprovedExecutable::from_reviewed_source_with_sha256(
            &fixture.executable.path,
            "Synthetic self-changing fixture review.",
            Sha256::digest(script.as_bytes()).into(),
        )
        .unwrap();
        let supervisor = Supervisor::new();
        assert!(matches!(
            supervisor
                .start_check(&approved, &fixture.homes[0])
                .unwrap()
                .wait(),
            Err(SupervisorError::Session(SessionError::InvalidExecutable))
        ));
        assert!(
            fs::read_to_string(&fixture.executable.path)
                .unwrap()
                .ends_with("# changed after the version process started\n")
        );
        assert!(
            fs::read_dir(&fixture.homes[0].directory)
                .unwrap()
                .next()
                .is_none(),
            "No app-server process or runtime home may survive the failed second fence."
        );
        supervisor.shutdown().unwrap();

        let fixture = Fixture::new(Mode::Success);
        let mut script = fs::read(&fixture.executable.path).unwrap();
        script.extend_from_slice(format!("\n#{}\n", "x".repeat(3 * 64 * 1024)).as_bytes());
        fs::write(&fixture.executable.path, &script).unwrap();
        let approved = ApprovedExecutable::from_reviewed_source_with_sha256(
            &fixture.executable.path,
            "Synthetic multi-chunk concurrent review.",
            Sha256::digest(&script).into(),
        )
        .unwrap();
        let supervisor = Supervisor::new();
        let first = supervisor
            .start_check(&approved, &fixture.homes[0])
            .unwrap();
        let second = supervisor
            .start_check(&approved, &fixture.homes[1])
            .unwrap();
        for result in [first.wait().unwrap(), second.wait().unwrap()] {
            assert!(result.account.requires_openai_auth);
            gone(result.process_id as libc::pid_t);
        }
        supervisor.shutdown().unwrap();
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
