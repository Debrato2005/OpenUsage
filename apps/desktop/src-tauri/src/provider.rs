//! Native approval and lifecycle for bounded profile operations. Only Rust accepts remote readings.
//! Lock order is Workspace -> Provider -> Supervisor. Owned workers never take the Workspace lock.
//! Executions use checked paths, not atomic fexecve. This does not isolate hostile same-UID path changes.

use crate::workspace::{CommandError, require_local_filesystem};
use openusage_codex::{
    AccountAuth, IdentityConfidence,
    session::{ProviderProbe, SessionError, TESTED_PROVIDER_VERSION},
    supervisor::{
        ApprovedExecutable, CheckHandle, ManagedProfileHome, ProfileId, Supervisor, SupervisorError,
    },
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

mod activity;
pub(crate) use activity::*;
mod quota;
pub(crate) use quota::*;
mod login;
pub(crate) use login::*;
mod discovery;
pub(crate) use discovery::{ExecutableDiscovery, discover};

const MAX_EXECUTABLE_BYTES: u64 = 1024 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutableSourceKind {
    OfficialArtifact,
    #[cfg(any(test, feature = "native-test"))]
    TestFixture,
    Unverified,
}
struct SourceProof {
    kind: ExecutableSourceKind,
    url: Option<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OfficialSource {
    schema_version: u32,
    source_kind: String,
    version: String,
    target: String,
    executable_sha256: String,
    source_url: String,
}

fn reviewed_source(digest: &[u8; 32]) -> SourceProof {
    let digest = hex(digest);
    if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        let official: Result<OfficialSource, _> = serde_json::from_str(include_str!(
            "../../../../crates/openusage-codex/provider-sources/codex-0.163.0-linux-x86_64.json"
        ));
        if let Ok(source) = official
            && source.schema_version == 1
            && source.source_kind == "official_artifact"
            && source.version == TESTED_PROVIDER_VERSION
            && source.target == "x86_64-unknown-linux-musl"
            && source.executable_sha256 == digest
            && source.source_url
                == "https://github.com/openai/codex/releases/download/rust-v0.163.0/codex-x86_64-unknown-linux-musl.tar.gz"
        {
            return SourceProof {
                kind: ExecutableSourceKind::OfficialArtifact,
                url: Some(source.source_url),
            };
        }
    }
    #[cfg(any(test, feature = "native-test"))]
    if matches!(
        digest.as_str(),
        "91c56f02d6d020bcb9a29c69c00bb108c306f169849971b5c42dbcd085398080"
            | "25476e3759cbdc1cc74bcd9e46f4a01d1d85ea8ccc34425ddb45a38df27b9859"
            | "ffceb4436e3ddbaf93548e36aa057232fc62bbda7138e00f90f250480d17e25c"
            | "91e780f49518a25ffa75963b2856f5951c5fe6f4d1e1c9040b41a4231d0750ae"
    ) {
        return SourceProof {
            kind: ExecutableSourceKind::TestFixture,
            url: None,
        };
    }
    SourceProof {
        kind: ExecutableSourceKind::Unverified,
        url: None,
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutableCandidate {
    pub candidate_id: String,
    pub path: String,
    pub file_bytes: String,
    pub sha256: String,
    pub tested_version: &'static str,
    pub version_checked: bool,
    pub source_verified: bool,
    pub source_kind: ExecutableSourceKind,
    pub source_url: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutableApproval {
    pub approval_id: String,
    pub path: String,
    pub file_bytes: String,
    pub sha256: String,
    pub tested_version: &'static str,
    pub version_checked: bool,
    pub source_verified: bool,
    pub source_kind: ExecutableSourceKind,
    pub source_url: Option<String>,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticAuthentication {
    Chatgpt,
    AuthenticationRequired,
    Unsupported,
}
#[derive(Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticIdentity {
    Provisional,
    Unknown,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionDiagnostic {
    pub profile_id: String,
    pub approval_id: String,
    pub provider_version: &'static str,
    pub source_kind: ExecutableSourceKind,
    pub authentication: DiagnosticAuthentication,
    pub identity_confidence: DiagnosticIdentity,
    pub requires_openai_auth: bool,
    pub checked_at: Option<String>,
    pub monitoring: &'static str,
    pub persisted: bool,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct Cancelled {
    pub cancelled: bool,
}

struct Candidate {
    metadata: ExecutableCandidate,
    held: HeldExecutable,
}
struct Approved {
    metadata: ExecutableApproval,
    held: HeldExecutable,
    executable: ApprovedExecutable,
}
/// Storage and source identity retained across an owned provider operation.
#[derive(Clone)]
pub(crate) struct ProfileBinding {
    pub profile_id: String,
    pub home_reference: String,
    pub revision: openusage_storage::StorageRevision,
    held_home: Arc<HeldHome>,
    pub approval_id: String,
    pub source_kind: ExecutableSourceKind,
    generation: u64,
}

struct ProviderState {
    closed: bool,
    refresh_starts: std::collections::HashMap<String, Instant>,
    refresh_backoff: std::collections::HashMap<String, RetryState>,
    generation: u64,
    candidate: Option<Candidate>,
    approved: Option<Approved>,
    supervisor: Arc<Supervisor>,
    logins: std::collections::HashMap<String, login::Entry>,
    quotas: std::collections::HashMap<String, quota::Entry>,
    activities: std::collections::HashMap<String, activity::Entry>,
}
impl Default for ProviderState {
    fn default() -> Self {
        Self {
            closed: false,
            refresh_starts: std::collections::HashMap::new(),
            refresh_backoff: std::collections::HashMap::new(),
            generation: 0,
            candidate: None,
            approved: None,
            supervisor: Arc::new(Supervisor::new()),
            logins: std::collections::HashMap::new(),
            quotas: std::collections::HashMap::new(),
            activities: std::collections::HashMap::new(),
        }
    }
}

#[derive(Clone, Default)]
pub(crate) struct ProviderCoordinator(Arc<Mutex<ProviderState>>);
const MANUAL_REFRESH_INTERVAL: Duration = Duration::from_secs(30);

struct RetryState {
    failures: u8,
    not_before: Instant,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProfileRefreshStatus {
    pub profile_id: String,
    pub retry_after_seconds: u16,
}

fn refresh_wait(state: &ProviderState, profile_id: &str, now: Instant) -> u16 {
    let manual = state
        .refresh_starts
        .get(profile_id)
        .map_or(Duration::ZERO, |started| {
            MANUAL_REFRESH_INTERVAL.saturating_sub(now.saturating_duration_since(*started))
        });
    let backoff = state
        .refresh_backoff
        .get(profile_id)
        .map_or(Duration::ZERO, |retry| {
            retry.not_before.saturating_duration_since(now)
        });
    let remaining = manual.max(backoff);
    (remaining.as_secs() + u64::from(remaining.subsec_nanos() > 0)).min(4320) as u16
}

fn backoff_base(failures: u8) -> u64 {
    [30, 60, 120, 240, 480, 960, 1920, 3600][usize::from(failures.saturating_sub(1).min(7))]
}

fn jittered_backoff(base: u64, sample: u64) -> Duration {
    let spread = base / 5;
    Duration::from_secs(base - spread + sample % (spread * 2 + 1))
}

fn record_refresh_failure(
    state: &mut ProviderState,
    binding: &ProfileBinding,
    error: SupervisorError,
    owned: bool,
) {
    if !owned
        || state.closed
        || state.generation != binding.generation
        || !state
            .approved
            .as_ref()
            .is_some_and(|approved| approved.metadata.approval_id == binding.approval_id)
        || !matches!(
            error,
            SupervisorError::Session(
                SessionError::TimedOut
                    | SessionError::TransportFailed
                    | SessionError::UnexpectedExit
            )
        )
    {
        return;
    }
    let failures = state
        .refresh_backoff
        .get(&binding.profile_id)
        .map_or(1, |retry| retry.failures.saturating_add(1).min(8));
    let base = backoff_base(failures);
    let mut bytes = [0; 8];
    let delay = if getrandom::fill(&mut bytes).is_ok() {
        jittered_backoff(base, u64::from_ne_bytes(bytes))
    } else {
        Duration::from_secs(base)
    };
    state.refresh_backoff.insert(
        binding.profile_id.clone(),
        RetryState {
            failures,
            not_before: Instant::now() + delay,
        },
    );
}

fn require_refresh_ready(state: &mut ProviderState, profile_id: &str) -> Result<(), CommandError> {
    let now = Instant::now();
    state
        .refresh_starts
        .retain(|_, started| now.saturating_duration_since(*started) < MANUAL_REFRESH_INTERVAL);
    if state
        .refresh_backoff
        .get(profile_id)
        .is_some_and(|retry| retry.not_before > now)
    {
        return Err(CommandError::ProviderRefreshBackoff);
    }
    if refresh_wait(state, profile_id, now) != 0 {
        return Err(CommandError::ProviderRefreshCooldown);
    }
    Ok(())
}

impl ProviderCoordinator {
    pub fn refresh_status(&self, profile_id: &str) -> Result<ProfileRefreshStatus, CommandError> {
        let state = self
            .0
            .lock()
            .map_err(|_| CommandError::ProviderUnavailable)?;
        require_open(&state)?;
        Ok(ProfileRefreshStatus {
            profile_id: profile_id.into(),
            retry_after_seconds: refresh_wait(&state, profile_id, Instant::now()),
        })
    }

    pub fn inspect(&self, path: &str) -> Result<ExecutableCandidate, CommandError> {
        let mut state = self
            .0
            .lock()
            .map_err(|_| CommandError::ProviderUnavailable)?;
        require_open(&state)?;
        state.candidate = None;
        let held = HeldExecutable::open(path)?;
        let proof = reviewed_source(&held.digest);
        let metadata = ExecutableCandidate {
            candidate_id: new_id()?,
            path: path.into(),
            file_bytes: held.metadata.len().to_string(),
            sha256: hex(&held.digest),
            tested_version: TESTED_PROVIDER_VERSION,
            version_checked: false,
            source_verified: proof.kind != ExecutableSourceKind::Unverified,
            source_kind: proof.kind,
            source_url: proof.url,
        };
        state.candidate = Some(Candidate {
            metadata: metadata.clone(),
            held,
        });
        Ok(metadata)
    }

    pub fn approve(
        &self,
        candidate_id: &str,
        confirmed: bool,
    ) -> Result<ExecutableApproval, CommandError> {
        crate::restore::validate_preview_id(candidate_id)?;
        if !confirmed {
            return Err(CommandError::InvalidArguments);
        }
        let mut state = self
            .0
            .lock()
            .map_err(|_| CommandError::ProviderUnavailable)?;
        require_open(&state)?;
        if !state
            .candidate
            .as_ref()
            .is_some_and(|candidate| candidate.metadata.candidate_id == candidate_id)
        {
            return Err(CommandError::StaleProviderApproval);
        }
        let mut candidate = state
            .candidate
            .take()
            .ok_or(CommandError::StaleProviderApproval)?;
        candidate.held.recheck()?;
        let proof = reviewed_source(&candidate.held.digest);
        if proof.kind == ExecutableSourceKind::Unverified {
            return Err(CommandError::ProviderSourceUnverified);
        }
        // Source must match a reviewed build-time pin. Confirmation separately authorizes execution.
        let executable = ApprovedExecutable::from_reviewed_source_with_sha256(
            &candidate.held.path,
            &format!("User confirmed native source review for candidate {candidate_id}"),
            candidate.held.digest,
        )
        .map_err(provider_error)?;
        reset(&mut state)?;
        state.approved = None;
        candidate.held.recheck()?;
        let metadata = ExecutableApproval {
            approval_id: new_id()?,
            path: candidate.metadata.path,
            file_bytes: candidate.metadata.file_bytes,
            sha256: candidate.metadata.sha256,
            tested_version: TESTED_PROVIDER_VERSION,
            version_checked: false,
            source_verified: true,
            source_kind: proof.kind,
            source_url: proof.url,
        };
        state.approved = Some(Approved {
            metadata: metadata.clone(),
            held: candidate.held,
            executable,
        });
        Ok(metadata)
    }

    pub fn start(
        &self,
        profile_id: &str,
        home_reference: &str,
        revision: openusage_storage::StorageRevision,
        path: &Path,
    ) -> Result<PendingCheck, CommandError> {
        let held_home = HeldHome::open(path)?;
        let home = ManagedProfileHome::new(
            ProfileId::parse(profile_id).map_err(provider_error)?,
            home_reference,
            path,
        )
        .map_err(provider_error)?;
        let mut state = self
            .0
            .lock()
            .map_err(|_| CommandError::ProviderUnavailable)?;
        require_open(&state)?;
        require_profile_available(&state, profile_id)?;
        let approved = state
            .approved
            .as_mut()
            .ok_or(CommandError::ProviderExecutableNotApproved)?;
        approved.held.recheck()?;
        let executable = approved.executable.clone();
        let approval_id = approved.metadata.approval_id.clone();
        let source_kind = approved.metadata.source_kind;
        let handle = state
            .supervisor
            .start_check(&executable, &home)
            .map_err(provider_error)?;
        Ok(PendingCheck {
            coordinator: self.clone(),
            profile_id: profile_id.into(),
            home_reference: home_reference.into(),
            revision,
            held_home,
            approval_id,
            source_kind,
            generation: state.generation,
            handle,
        })
    }

    pub fn cancel(&self, profile_id: &str) -> Result<Cancelled, CommandError> {
        let profile = ProfileId::parse(profile_id).map_err(|_| CommandError::InvalidProfileId)?;
        let state = self
            .0
            .lock()
            .map_err(|_| CommandError::ProviderUnavailable)?;
        // This command authorizes check cancellation only. Login needs its exact attempt token.
        require_profile_available(&state, profile_id)?;
        let cancelled = state.supervisor.cancel(&profile).map_err(provider_error)?;
        Ok(Cancelled { cancelled })
    }

    /// Join owned workers before a restore or a managed-home remap. Approval remains bound to its original bytes.
    pub fn quiesce(&self) -> Result<(), CommandError> {
        let mut state = self
            .0
            .lock()
            .map_err(|_| CommandError::ProviderUnavailable)?;
        require_open(&state)?;
        reset(&mut state)
    }

    pub fn close(&self) -> Result<(), CommandError> {
        let mut state = self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.closed = true;
        state.candidate = None;
        state.approved = None;
        state.supervisor.shutdown().map_err(provider_error)?;
        state.logins.clear();
        state.quotas.clear();
        state.activities.clear();
        Ok(())
    }
}

/// Do not let a profile-only check or another operation consume a preview's token.
fn require_profile_available(state: &ProviderState, profile_id: &str) -> Result<(), CommandError> {
    if state
        .logins
        .values()
        .any(|entry| entry.binding.profile_id == profile_id)
    {
        return Err(CommandError::ProviderLoginInProgress);
    }
    if state
        .quotas
        .values()
        .any(|entry| entry.binding.profile_id == profile_id)
    {
        return Err(CommandError::ProviderQuotaInProgress);
    }
    if state
        .activities
        .values()
        .any(|entry| entry.binding.profile_id == profile_id)
    {
        return Err(CommandError::ProviderActivityInProgress);
    }
    Ok(())
}

fn reset(state: &mut ProviderState) -> Result<(), CommandError> {
    state.supervisor.shutdown().map_err(provider_error)?;
    state.logins.clear();
    state.quotas.clear();
    state.activities.clear();
    state.generation = state
        .generation
        .checked_add(1)
        .ok_or(CommandError::ProviderUnavailable)?;
    state.supervisor = Arc::new(Supervisor::new());
    Ok(())
}
fn require_open(state: &ProviderState) -> Result<(), CommandError> {
    if state.closed {
        Err(CommandError::ProviderShuttingDown)
    } else {
        Ok(())
    }
}

pub(crate) struct PendingCheck {
    coordinator: ProviderCoordinator,
    profile_id: String,
    home_reference: String,
    revision: openusage_storage::StorageRevision,
    held_home: HeldHome,
    approval_id: String,
    source_kind: ExecutableSourceKind,
    generation: u64,
    handle: CheckHandle,
}

pub(crate) struct JoinedCheck {
    coordinator: ProviderCoordinator,
    pub profile_id: String,
    pub home_reference: String,
    pub revision: openusage_storage::StorageRevision,
    held_home: HeldHome,
    approval_id: String,
    source_kind: ExecutableSourceKind,
    generation: u64,
    probe: ProviderProbe,
}
impl PendingCheck {
    /// The caller must release Workspace before joining the owned Supervisor worker.
    pub fn wait(self) -> Result<JoinedCheck, CommandError> {
        let probe = self.handle.wait().map_err(provider_error)?;
        Ok(JoinedCheck {
            coordinator: self.coordinator,
            profile_id: self.profile_id,
            home_reference: self.home_reference,
            revision: self.revision,
            held_home: self.held_home,
            approval_id: self.approval_id,
            source_kind: self.source_kind,
            generation: self.generation,
            probe,
        })
    }
}
impl JoinedCheck {
    /// Workspace checks its current binding before this final Provider lock.
    pub fn finish(self) -> Result<ConnectionDiagnostic, CommandError> {
        let state = self
            .coordinator
            .0
            .lock()
            .map_err(|_| CommandError::ProviderUnavailable)?;
        require_open(&state)?;
        self.held_home.require_identity()?;
        if state.generation != self.generation
            || !state
                .approved
                .as_ref()
                .is_some_and(|approved| approved.metadata.approval_id == self.approval_id)
        {
            return Err(CommandError::ProviderCheckCancelled);
        }
        diagnostic(
            self.profile_id,
            self.approval_id,
            self.source_kind,
            self.probe,
        )
    }
}

struct HeldHome {
    path: PathBuf,
    file: File,
}
impl HeldHome {
    fn open(path: &Path) -> Result<Self, CommandError> {
        let mut options = OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NOFOLLOW | libc::O_DIRECTORY | libc::O_NONBLOCK);
        }
        let held = Self {
            path: path.into(),
            file: options
                .open(path)
                .map_err(|_| CommandError::ProviderHomeUnavailable)?,
        };
        held.require_identity()?;
        Ok(held)
    }
    fn require_identity(&self) -> Result<(), CommandError> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let actual = fs::symlink_metadata(&self.path)
                .map_err(|_| CommandError::ProviderHomeUnavailable)?;
            let held = self
                .file
                .metadata()
                .map_err(|_| CommandError::ProviderHomeUnavailable)?;
            if !actual.is_dir()
                || actual.file_type().is_symlink()
                || self.path.canonicalize().ok().as_deref() != Some(self.path.as_path())
                || actual.dev() != held.dev()
                || actual.ino() != held.ino()
                || held.uid() != unsafe { libc::geteuid() }
                || held.mode() & 0o777 != 0o700
            {
                return Err(CommandError::ProviderHomeUnavailable);
            }
            Ok(())
        }
        #[cfg(not(unix))]
        {
            Err(CommandError::ProviderUnsupported)
        }
    }
}

fn diagnostic(
    profile_id: String,
    approval_id: String,
    source_kind: ExecutableSourceKind,
    probe: ProviderProbe,
) -> Result<ConnectionDiagnostic, CommandError> {
    // Probe.version is available only after the child returned the exact tested --version response.
    if probe.version != TESTED_PROVIDER_VERSION {
        return Err(CommandError::ProviderUnsupported);
    }
    Ok(ConnectionDiagnostic {
        profile_id,
        approval_id,
        provider_version: probe.version,
        source_kind,
        authentication: match probe.account.authentication {
            AccountAuth::Chatgpt => DiagnosticAuthentication::Chatgpt,
            AccountAuth::AuthenticationRequired => DiagnosticAuthentication::AuthenticationRequired,
            AccountAuth::Unsupported => DiagnosticAuthentication::Unsupported,
        },
        identity_confidence: match probe.account.identity {
            IdentityConfidence::Provisional => DiagnosticIdentity::Provisional,
            IdentityConfidence::Unknown => DiagnosticIdentity::Unknown,
        },
        requires_openai_auth: probe.account.requires_openai_auth,
        checked_at: crate::restore::timestamp(std::time::SystemTime::now()),
        monitoring: "stopped",
        persisted: false,
    })
}

fn provider_error(error: SupervisorError) -> CommandError {
    use SessionError as S;
    match error {
        SupervisorError::DuplicateProfile => CommandError::ProviderCheckInProgress,
        SupervisorError::CapacityReached => CommandError::ProviderCapacityReached,
        SupervisorError::Cancelled | SupervisorError::Session(S::Cancelled) => {
            CommandError::ProviderCheckCancelled
        }
        SupervisorError::Session(S::TimedOut) => CommandError::ProviderCheckTimedOut,
        SupervisorError::Session(S::UnsupportedPlatform | S::UnsupportedVersion) => {
            CommandError::ProviderUnsupported
        }
        SupervisorError::Session(S::InvalidExecutable) => CommandError::StaleProviderApproval,
        SupervisorError::HomeInUse
        | SupervisorError::HomeChanged
        | SupervisorError::InvalidHomeReference
        | SupervisorError::Session(S::InvalidManagedDirectory) => {
            CommandError::ProviderHomeUnavailable
        }
        SupervisorError::Session(S::Protocol(_) | S::NotificationLimit) => {
            CommandError::ProviderProtocolFailed
        }
        SupervisorError::Session(S::CleanupFailed | S::TerminationFailed) => {
            CommandError::ProviderCleanupFailed
        }
        SupervisorError::Closed => CommandError::ProviderShuttingDown,
        _ => CommandError::ProviderUnavailable,
    }
}

struct HeldExecutable {
    path: PathBuf,
    file: File,
    metadata: fs::Metadata,
    digest: [u8; 32],
}
impl HeldExecutable {
    fn open(path: &str) -> Result<Self, CommandError> {
        if path.is_empty()
            || path.len() > 4096
            || path.chars().any(char::is_control)
            || !Path::new(path).is_absolute()
            || Path::new(path).canonicalize().ok().as_deref() != Some(Path::new(path))
        {
            return Err(CommandError::InvalidProviderExecutable);
        }
        if !cfg!(unix) {
            return Err(CommandError::ProviderUnsupported);
        }
        require_local_filesystem(Path::new(path))
            .map_err(|_| CommandError::InvalidProviderExecutable)?;
        let mut options = OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
        }
        let file = options
            .open(path)
            .map_err(|_| CommandError::InvalidProviderExecutable)?;
        let metadata = file
            .metadata()
            .map_err(|_| CommandError::InvalidProviderExecutable)?;
        let mut held = Self {
            path: path.into(),
            file,
            metadata,
            digest: [0; 32],
        };
        held.require_identity()
            .map_err(|_| CommandError::InvalidProviderExecutable)?;
        held.digest = held.hash()?;
        held.require_identity()?;
        Ok(held)
    }
    fn require_identity(&self) -> Result<(), CommandError> {
        openusage_codex::supervisor::validate_executable_source_path(&self.path)
            .map_err(provider_error)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let now = fs::symlink_metadata(&self.path)
                .map_err(|_| CommandError::StaleProviderApproval)?;
            let held = self
                .file
                .metadata()
                .map_err(|_| CommandError::StaleProviderApproval)?;
            let original = &self.metadata;
            if !now.is_file()
                || now.file_type().is_symlink()
                || self.path.canonicalize().ok().as_deref() != Some(self.path.as_path())
                || now.dev() != held.dev()
                || now.ino() != held.ino()
                || held.dev() != original.dev()
                || held.ino() != original.ino()
                || held.mode() != original.mode()
                || held.uid() != original.uid()
                || held.len() != original.len()
                || held.mtime() != original.mtime()
                || held.mtime_nsec() != original.mtime_nsec()
                || held.ctime() != original.ctime()
                || held.ctime_nsec() != original.ctime_nsec()
                || held.mode() & 0o111 == 0
                || held.mode() & 0o6022 != 0
                || held.len() == 0
                || held.len() > MAX_EXECUTABLE_BYTES
            {
                return Err(CommandError::StaleProviderApproval);
            }
            Ok(())
        }
        #[cfg(not(unix))]
        {
            Err(CommandError::ProviderUnsupported)
        }
    }
    fn hash(&mut self) -> Result<[u8; 32], CommandError> {
        self.file
            .seek(SeekFrom::Start(0))
            .map_err(|_| CommandError::StaleProviderApproval)?;
        let mut hash = Sha256::new();
        let mut bytes = [0u8; 64 * 1024];
        let mut total = 0u64;
        loop {
            let read = self
                .file
                .read(&mut bytes)
                .map_err(|_| CommandError::StaleProviderApproval)?;
            if read == 0 {
                break;
            }
            total = total
                .checked_add(read as u64)
                .filter(|size| *size <= MAX_EXECUTABLE_BYTES)
                .ok_or(CommandError::InvalidProviderExecutable)?;
            hash.update(&bytes[..read]);
        }
        if total != self.metadata.len() {
            return Err(CommandError::StaleProviderApproval);
        }
        Ok(hash.finalize().into())
    }
    fn recheck(&mut self) -> Result<(), CommandError> {
        self.require_identity()?;
        if self.hash()? != self.digest {
            return Err(CommandError::StaleProviderApproval);
        }
        self.require_identity()
    }
}
fn hex(bytes: &[u8; 32]) -> String {
    use std::fmt::Write;
    let mut result = String::with_capacity(64);
    for byte in bytes {
        let _ = write!(&mut result, "{byte:02x}");
    }
    result
}
pub(crate) fn new_id() -> Result<String, CommandError> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|_| CommandError::ProviderUnavailable)?;
    Ok(uuid::Builder::from_random_bytes(bytes)
        .into_uuid()
        .to_string())
}

#[cfg(all(test, unix))]
pub(crate) mod tests {
    use super::*;
    use openusage_storage::Storage;
    use std::{
        os::unix::fs::{PermissionsExt, symlink},
        time::{Duration, Instant},
    };

    // Only Rust tests advance this timestamp. Native journeys use the real thirty-second wait.
    pub(crate) fn elapse_refresh(provider: &ProviderCoordinator, profile_id: &str) {
        let mut state = provider.0.lock().unwrap();
        if let Some(started) = state.refresh_starts.get_mut(profile_id) {
            *started = Instant::now().checked_sub(Duration::from_secs(30)).unwrap();
        }
        if let Some(retry) = state.refresh_backoff.get_mut(profile_id) {
            retry.not_before = Instant::now();
        }
    }

    pub(crate) fn backoff_failures(provider: &ProviderCoordinator, profile_id: &str) -> u8 {
        provider
            .0
            .lock()
            .unwrap()
            .refresh_backoff
            .get(profile_id)
            .map_or(0, |retry| retry.failures)
    }

    #[test]
    fn transient_backoff_sequence_jitter_and_later_deadline_are_bounded() {
        let bases = [30, 60, 120, 240, 480, 960, 1920, 3600, 3600];
        for (index, seconds) in bases.into_iter().enumerate() {
            let base = backoff_base((index + 1) as u8);
            assert_eq!(base, seconds);
            let width = base / 5 * 2 + 1;
            assert_eq!(
                jittered_backoff(base, width),
                Duration::from_secs(base - base / 5)
            );
            assert_eq!(
                jittered_backoff(base, width * 2 - 1),
                Duration::from_secs(base + base / 5)
            );
            assert!(jittered_backoff(base, u64::MAX) <= Duration::from_secs(base + base / 5));
        }
        let now = Instant::now();
        let mut state = ProviderState::default();
        state.refresh_starts.insert("profile".into(), now);
        state.refresh_backoff.insert(
            "profile".into(),
            RetryState {
                failures: 8,
                not_before: now + Duration::from_millis(4_319_001),
            },
        );
        assert_eq!(refresh_wait(&state, "profile", now), 4320);
        assert_eq!(refresh_wait(&state, "other", now), 0);
        assert_eq!(
            require_refresh_ready(&mut state, "profile"),
            Err(CommandError::ProviderRefreshBackoff)
        );
        reset(&mut state).unwrap();
        assert_eq!(refresh_wait(&state, "profile", now), 4320);
        state.refresh_backoff.get_mut("profile").unwrap().not_before = now;
        assert_eq!(refresh_wait(&state, "profile", now), 30);
        assert_eq!(
            require_refresh_ready(&mut state, "profile"),
            Err(CommandError::ProviderRefreshCooldown)
        );
        assert_eq!(state.refresh_backoff["profile"].failures, 8);
    }

    #[test]
    fn refresh_wait_rounds_up_and_preserves_deadlines_through_quiescence() {
        let started = Instant::now();
        let mut state = ProviderState::default();
        state.refresh_starts.insert("profile".into(), started);
        assert_eq!(refresh_wait(&state, "profile", started), 30);
        assert_eq!(
            refresh_wait(&state, "profile", started + Duration::from_millis(29999)),
            1
        );
        assert_eq!(
            refresh_wait(&state, "profile", started + Duration::from_secs(30)),
            0
        );
        assert_eq!(
            refresh_wait(
                &state,
                "profile",
                started.checked_sub(Duration::from_secs(1)).unwrap()
            ),
            30
        );
        assert_eq!(refresh_wait(&state, "other", started), 0);
        reset(&mut state).unwrap();
        assert_eq!(refresh_wait(&state, "profile", started), 30);
        assert_eq!(
            require_refresh_ready(&mut state, "profile"),
            Err(CommandError::ProviderRefreshCooldown)
        );
        *state.refresh_starts.get_mut("profile").unwrap() =
            started.checked_sub(Duration::from_secs(31)).unwrap();
        assert_eq!(require_refresh_ready(&mut state, "profile"), Ok(()));
        assert!(state.refresh_starts.is_empty());
    }

    struct Fixture {
        _temporary: tempfile::TempDir,
        provider: ProviderCoordinator,
        executable: PathBuf,
        executed: PathBuf,
        homes: Vec<(String, PathBuf)>,
        storage: Storage,
    }
    impl Fixture {
        fn new(hang: bool) -> Self {
            let temporary = tempfile::tempdir().unwrap();
            fs::set_permissions(temporary.path(), fs::Permissions::from_mode(0o700)).unwrap();
            let executable = temporary.path().join("synthetic-provider");
            let executed = temporary
                .path()
                .join("synthetic-execution-observations.jsonl");
            fs::write(
                &executable,
                include_bytes!("../../../../tests/native/fixtures/synthetic-provider.py"),
            )
            .unwrap();
            fs::write(
                temporary.path().join("synthetic-provider-config.json"),
                serde_json::to_vec(
                    &serde_json::json!({"defaultMode": if hang { "hang" } else { "provisional" }}),
                )
                .unwrap(),
            )
            .unwrap();
            fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
            let mut storage =
                Storage::create(&temporary.path().join("synthetic.db"), &[0x26; 32]).unwrap();
            let homes = (1..=3)
                .map(|number| {
                    let id = format!("{number:08x}-0000-4000-8000-000000000000");
                    let path = temporary.path().join(&id);
                    crate::workspace::prepare_directory(&path).unwrap();
                    storage
                        .create_profile(&id, "Synthetic profile", &id)
                        .unwrap();
                    fs::write(path.join("synthetic-existing-marker"), b"preserve").unwrap();
                    (id, path)
                })
                .collect();
            Self {
                _temporary: temporary,
                provider: ProviderCoordinator::default(),
                executable,
                executed,
                homes,
                storage,
            }
        }
        fn approve(&self) -> ExecutableApproval {
            let candidate = self
                .provider
                .inspect(self.executable.to_str().unwrap())
                .unwrap();
            self.provider
                .approve(&candidate.candidate_id, true)
                .unwrap()
        }
        fn start(&mut self, index: usize) -> Result<PendingCheck, CommandError> {
            let revision = self.storage.restore_state().unwrap().revision;
            self.provider.start(
                &self.homes[index].0,
                &self.homes[index].0,
                revision,
                &self.homes[index].1,
            )
        }
        fn child(&self, index: usize) -> libc::pid_t {
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                if let Ok(records) = fs::read_to_string(
                    self.homes[index]
                        .1
                        .join("synthetic-provider-observations.jsonl"),
                ) {
                    for record in records
                        .lines()
                        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
                    {
                        if record["kind"] == "account-read-pending" {
                            return record["pid"].as_i64().unwrap().try_into().unwrap();
                        }
                    }
                }
                assert!(
                    Instant::now() < deadline,
                    "Synthetic native child did not start"
                );
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }

    #[test]
    fn transient_backoff_excludes_semantic_errors_and_stale_or_unowned_completions() {
        let mut fixture = Fixture::new(false);
        let approval = fixture.approve();
        let binding = ProfileBinding {
            profile_id: fixture.homes[0].0.clone(),
            home_reference: fixture.homes[0].0.clone(),
            revision: fixture.storage.restore_state().unwrap().revision,
            held_home: Arc::new(HeldHome::open(&fixture.homes[0].1).unwrap()),
            approval_id: approval.approval_id,
            source_kind: approval.source_kind,
            generation: fixture.provider.0.lock().unwrap().generation,
        };
        let mut state = fixture.provider.0.lock().unwrap();
        for error in [
            SupervisorError::Cancelled,
            SupervisorError::CapacityReached,
            SupervisorError::WorkerFailed,
            SupervisorError::Session(SessionError::Cancelled),
            SupervisorError::Session(SessionError::StartFailed),
            SupervisorError::Session(SessionError::CleanupFailed),
            SupervisorError::Session(SessionError::TerminationFailed),
            SupervisorError::Session(SessionError::NotificationLimit),
            SupervisorError::Session(SessionError::Protocol(
                openusage_codex::ProtocolError::ProviderFailure,
            )),
            SupervisorError::Session(SessionError::Protocol(
                openusage_codex::ProtocolError::UnsupportedAuthentication,
            )),
            SupervisorError::Session(SessionError::Protocol(
                openusage_codex::ProtocolError::InvalidMessage,
            )),
            SupervisorError::Session(SessionError::Protocol(
                openusage_codex::ProtocolError::AccountChanged,
            )),
        ] {
            record_refresh_failure(&mut state, &binding, error, true);
            assert!(state.refresh_backoff.is_empty(), "{error:?}");
        }
        let transient = SupervisorError::Session(SessionError::UnexpectedExit);
        record_refresh_failure(&mut state, &binding, transient, false);
        assert!(state.refresh_backoff.is_empty());
        let mut stale = binding.clone();
        stale.generation += 1;
        record_refresh_failure(&mut state, &stale, transient, true);
        stale.generation = binding.generation;
        stale.approval_id = new_id().unwrap();
        record_refresh_failure(&mut state, &stale, transient, true);
        assert!(state.refresh_backoff.is_empty());
        for (index, error) in [
            SessionError::TimedOut,
            SessionError::TransportFailed,
            SessionError::UnexpectedExit,
        ]
        .into_iter()
        .cycle()
        .take(10)
        .enumerate()
        {
            record_refresh_failure(&mut state, &binding, error.into(), true);
            let retry = &state.refresh_backoff[&binding.profile_id];
            assert_eq!(retry.failures, ((index + 1) as u8).min(8));
            let base = backoff_base(retry.failures);
            let wait = retry.not_before.saturating_duration_since(Instant::now());
            assert!(wait <= Duration::from_secs(base + base / 5));
            assert!(wait > Duration::from_secs(base - base / 5 - 1));
        }
        state.closed = true;
        let deadline = state.refresh_backoff[&binding.profile_id].not_before;
        record_refresh_failure(&mut state, &binding, transient, true);
        assert_eq!(
            state.refresh_backoff[&binding.profile_id].not_before,
            deadline
        );
    }
    fn gone(pid: libc::pid_t) {
        assert_eq!(unsafe { libc::kill(pid, 0) }, -1);
        assert_eq!(
            std::io::Error::last_os_error().raw_os_error(),
            Some(libc::ESRCH)
        );
    }

    #[test]
    fn inspection_and_explicit_approval_execute_nothing_then_return_only_redacted_diagnostics() {
        let mut fixture = Fixture::new(false);
        assert_eq!(
            fixture.start(0).err(),
            Some(CommandError::ProviderExecutableNotApproved)
        );
        let candidate = fixture
            .provider
            .inspect(fixture.executable.to_str().unwrap())
            .unwrap();
        assert!(!candidate.version_checked);
        assert!(candidate.source_verified);
        assert_eq!(candidate.source_kind, ExecutableSourceKind::TestFixture);
        assert!(candidate.source_url.is_none());
        assert_eq!(candidate.sha256.len(), 64);
        assert_eq!(
            fixture
                .provider
                .approve(&candidate.candidate_id, false)
                .err(),
            Some(CommandError::InvalidArguments)
        );
        let approval = fixture
            .provider
            .approve(&candidate.candidate_id, true)
            .unwrap();
        assert_eq!(approval.sha256, candidate.sha256);
        assert!(!fixture.executed.exists());
        let before = fixture.storage.restore_state().unwrap();
        let joined = fixture.start(0).unwrap().wait().unwrap();
        let diagnostic = joined.finish().unwrap();
        assert_eq!(diagnostic.provider_version, "0.163.0");
        assert_eq!(diagnostic.authentication, DiagnosticAuthentication::Chatgpt);
        assert_eq!(
            diagnostic.identity_confidence,
            DiagnosticIdentity::Provisional
        );
        assert!(!diagnostic.persisted);
        assert_eq!(diagnostic.monitoring, "stopped");
        assert_eq!(diagnostic.source_kind, ExecutableSourceKind::TestFixture);
        let json = serde_json::to_string(&diagnostic).unwrap();
        for forbidden in [
            "synthetic-unverified@example.invalid",
            "synthetic-plan",
            "codexHome",
            "userAgent",
            "processId",
            fixture._temporary.path().to_str().unwrap(),
        ] {
            assert!(!json.contains(forbidden));
        }
        assert_eq!(fixture.storage.restore_state().unwrap(), before);
        assert_eq!(
            fs::read_to_string(&fixture.executed)
                .unwrap()
                .lines()
                .filter(
                    |line| serde_json::from_str::<serde_json::Value>(line).unwrap()["kind"]
                        == "start"
                )
                .count(),
            2
        );
        gone(fixture.child(0));
        for index in 0..3 {
            assert_eq!(
                fs::read(fixture.homes[index].1.join("synthetic-existing-marker")).unwrap(),
                b"preserve"
            );
        }
        let observations = fs::read_to_string(&fixture.executed).unwrap();
        let runtime: serde_json::Value =
            serde_json::from_str(observations.lines().next().unwrap()).unwrap();
        assert!(!Path::new(runtime["environment"]["HOME"].as_str().unwrap()).exists());
    }

    #[test]
    fn changed_candidates_and_approved_executables_fail_before_any_execution() {
        let mut fixture = Fixture::new(false);
        let candidate = fixture
            .provider
            .inspect(fixture.executable.to_str().unwrap())
            .unwrap();
        fs::write(&fixture.executable, b"#!/bin/sh\nexit 1\n").unwrap();
        assert_eq!(
            fixture
                .provider
                .approve(&candidate.candidate_id, true)
                .err(),
            Some(CommandError::StaleProviderApproval)
        );
        assert!(!fixture.executed.exists());
        let unknown = fixture
            .provider
            .inspect(fixture.executable.to_str().unwrap())
            .unwrap();
        assert!(!unknown.source_verified);
        assert_eq!(unknown.source_kind, ExecutableSourceKind::Unverified);
        assert_eq!(
            fixture.provider.approve(&unknown.candidate_id, true).err(),
            Some(CommandError::ProviderSourceUnverified)
        );
        assert!(!fixture.executed.exists());
        fs::write(
            &fixture.executable,
            include_bytes!("../../../../tests/native/fixtures/synthetic-provider.py"),
        )
        .unwrap();
        let approval = fixture.approve();
        assert!(!approval.version_checked);
        let old = fixture._temporary.path().join("synthetic-old-executable");
        fs::rename(&fixture.executable, &old).unwrap();
        fs::copy(&old, &fixture.executable).unwrap();
        assert_eq!(
            fixture.start(0).err(),
            Some(CommandError::StaleProviderApproval)
        );
        assert!(!fixture.executed.exists());
        let link = fixture._temporary.path().join("synthetic-link");
        symlink(&old, &link).unwrap();
        assert_eq!(
            fixture.provider.inspect(link.to_str().unwrap()).err(),
            Some(CommandError::InvalidProviderExecutable)
        );
    }

    #[test]
    fn native_cancel_close_and_reapproval_join_owned_workers_and_reject_late_results() {
        let mut fixture = Fixture::new(true);
        fixture.approve();
        let first = fixture.start(0).unwrap();
        let second = fixture.start(1).unwrap();
        let first_pid = fixture.child(0);
        let second_pid = fixture.child(1);
        assert_eq!(
            fixture.start(0).err(),
            Some(CommandError::ProviderCheckInProgress)
        );
        assert_eq!(
            fixture.start(2).err(),
            Some(CommandError::ProviderCapacityReached)
        );
        assert!(
            fixture
                .provider
                .cancel(&fixture.homes[0].0)
                .unwrap()
                .cancelled
        );
        assert_eq!(
            first.wait().err(),
            Some(CommandError::ProviderCheckCancelled)
        );
        gone(first_pid);
        // Approving a reviewed executable again must stop the prior approval's active child.
        fixture.approve();
        assert_eq!(
            second.wait().err(),
            Some(CommandError::ProviderCheckCancelled)
        );
        gone(second_pid);
        let third = fixture.start(2).unwrap();
        let third_pid = fixture.child(2);
        fixture.provider.close().unwrap();
        assert_eq!(
            third.wait().err(),
            Some(CommandError::ProviderCheckCancelled)
        );
        gone(third_pid);
        assert_eq!(
            fixture.start(0).err(),
            Some(CommandError::ProviderShuttingDown)
        );
        let mut successful = Fixture::new(false);
        successful.approve();
        let joined = successful.start(0).unwrap().wait().unwrap();
        successful.provider.quiesce().unwrap();
        assert_eq!(
            joined.finish().err(),
            Some(CommandError::ProviderCheckCancelled)
        );
    }
}
