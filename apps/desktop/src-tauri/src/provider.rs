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

mod collection;
mod companion;
pub(crate) use collection::*;
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
            | "53a853b737ceca311649e9f1d9ebd1341154a5f782efd8e915f7f834c819a6dc"
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
    checks: std::collections::HashMap<String, String>,
    logins: std::collections::HashMap<String, login::Entry>,
    quotas: std::collections::HashMap<String, quota::Entry>,
    activities: std::collections::HashMap<String, activity::Entry>,
    collections: std::collections::HashMap<String, collection::Entry>,
}
impl ProviderState {
    fn active_count(&self) -> usize {
        self.logins.len() + self.quotas.len() + self.activities.len() + self.collections.len()
    }
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
            checks: std::collections::HashMap::new(),
            logins: std::collections::HashMap::new(),
            quotas: std::collections::HashMap::new(),
            activities: std::collections::HashMap::new(),
            collections: std::collections::HashMap::new(),
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
    remaining_seconds(manual.max(backoff))
}

fn remaining_seconds(remaining: Duration) -> u16 {
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
    require_backoff_ready(state, profile_id, now)?;
    if refresh_wait(state, profile_id, now) != 0 {
        return Err(CommandError::ProviderRefreshCooldown);
    }
    Ok(())
}

fn require_backoff_ready(
    state: &ProviderState,
    profile_id: &str,
    now: Instant,
) -> Result<(), CommandError> {
    if state
        .refresh_backoff
        .get(profile_id)
        .is_some_and(|retry| retry.not_before > now)
    {
        Err(CommandError::ProviderRefreshBackoff)
    } else {
        Ok(())
    }
}

fn binding_current(state: &ProviderState, binding: &ProfileBinding) -> bool {
    !state.closed
        && state.generation == binding.generation
        && binding.held_home.require_identity().is_ok()
        && state.approved.as_ref().is_some_and(|approved| {
            approved.metadata.approval_id == binding.approval_id
                && approved.held.require_identity().is_ok()
        })
}

impl ProviderCoordinator {
    /// Runtime login grants remain valid only for the same approval, generation and physical home.
    pub fn binding_is_current(&self, binding: &ProfileBinding) -> bool {
        let Ok(state) = self.0.lock() else {
            return false;
        };
        binding_current(&state, binding)
    }

    /// Scheduler status excludes the independent manual-refresh interval.
    pub fn scheduled_retry_after_seconds(&self, profile_id: &str) -> Result<u16, CommandError> {
        let state = self
            .0
            .lock()
            .map_err(|_| CommandError::ProviderUnavailable)?;
        require_open(&state)?;
        let remaining = state
            .refresh_backoff
            .get(profile_id)
            .map_or(Duration::ZERO, |retry| {
                retry.not_before.saturating_duration_since(Instant::now())
            });
        Ok(remaining_seconds(remaining))
    }

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
            &format!(
                "Reviewed native source approved for account operations: candidate {candidate_id}"
            ),
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
        let request_id = new_id()?;
        let handle = state
            .supervisor
            .start_check(&executable, &home)
            .map_err(provider_error)?;
        state.checks.insert(profile_id.into(), request_id.clone());
        Ok(PendingCheck {
            ownership: CheckOwnership {
                coordinator: self.clone(),
                profile_id: profile_id.into(),
                request_id,
            },
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
        let mut state = self
            .0
            .lock()
            .map_err(|_| CommandError::ProviderUnavailable)?;
        // This command authorizes check cancellation only. Login needs its exact attempt token.
        if !state.checks.contains_key(profile_id) {
            require_profile_available(&state, profile_id)?;
        }
        let result = state.supervisor.cancel(&profile).map_err(provider_error);
        let owned = state.checks.remove(profile_id).is_some();
        result.map(|cancelled| Cancelled {
            cancelled: cancelled || owned,
        })
    }

    /// Invalidate and join only this profile, including already completed unpublished replies.
    pub fn cancel_profile_operations(&self, profile_id: &str) -> Result<Cancelled, CommandError> {
        let profile = ProfileId::parse(profile_id).map_err(|_| CommandError::InvalidProfileId)?;
        let mut state = self
            .0
            .lock()
            .map_err(|_| CommandError::ProviderUnavailable)?;
        require_open(&state)?;
        let owned = state.checks.contains_key(profile_id)
            || state
                .logins
                .values()
                .any(|e| e.binding.profile_id == profile_id)
            || state
                .quotas
                .values()
                .any(|e| e.binding.profile_id == profile_id)
            || state
                .activities
                .values()
                .any(|e| e.binding.profile_id == profile_id)
            || state
                .collections
                .values()
                .any(|e| e.binding.profile_id == profile_id);
        // Core workers do not acquire Workspace; callers keep Workspace -> Provider lock order.
        let result = state.supervisor.cancel(&profile).map_err(provider_error);
        state.checks.remove(profile_id);
        state
            .logins
            .retain(|_, e| e.binding.profile_id != profile_id);
        state
            .quotas
            .retain(|_, e| e.binding.profile_id != profile_id);
        state
            .activities
            .retain(|_, e| e.binding.profile_id != profile_id);
        state
            .collections
            .retain(|_, e| e.binding.profile_id != profile_id);
        result.map(|cancelled| Cancelled {
            cancelled: cancelled || owned,
        })
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
        state.collections.clear();
        Ok(())
    }
}

/// Do not let a profile-only check or another operation consume a preview's token.
fn require_profile_available(state: &ProviderState, profile_id: &str) -> Result<(), CommandError> {
    if state.checks.contains_key(profile_id) {
        return Err(CommandError::ProviderCheckInProgress);
    }
    if state
        .collections
        .values()
        .any(|entry| entry.binding.profile_id == profile_id)
    {
        return Err(CommandError::ProviderCollectionInProgress);
    }
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
    state.checks.clear();
    state.logins.clear();
    state.quotas.clear();
    state.activities.clear();
    state.collections.clear();
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

struct CheckOwnership {
    coordinator: ProviderCoordinator,
    profile_id: String,
    request_id: String,
}
impl Drop for CheckOwnership {
    fn drop(&mut self) {
        let mut state = self.coordinator.0.lock().unwrap_or_else(|p| p.into_inner());
        if state.checks.get(&self.profile_id) == Some(&self.request_id) {
            state.checks.remove(&self.profile_id);
        }
    }
}

pub(crate) struct PendingCheck {
    ownership: CheckOwnership,
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
    ownership: CheckOwnership,
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
            ownership: self.ownership,
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
    pub fn abort(self) {}

    /// Workspace checks its current binding before this final Provider lock.
    pub fn finish(self) -> Result<ConnectionDiagnostic, CommandError> {
        let state = self
            .coordinator
            .0
            .lock()
            .map_err(|_| CommandError::ProviderUnavailable)?;
        require_open(&state)?;
        self.held_home.require_identity()?;
        if state.checks.get(&self.profile_id) != Some(&self.ownership.request_id)
            || state.generation != self.generation
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
                || held.nlink() != original.nlink()
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
        fn collection(
            &mut self,
            index: usize,
            approval: &str,
            scheduled: bool,
        ) -> Result<CollectionReceipt, CommandError> {
            let revision = self.storage.restore_state().unwrap().revision;
            let (id, path) = &self.homes[index];
            let association = ReadAssociation {
                expected_binding: None,
                review: None,
            };
            if scheduled {
                self.provider.begin_scheduled_collection(
                    id,
                    approval,
                    id,
                    revision,
                    path,
                    association,
                )
            } else {
                self.provider
                    .begin_collection(id, approval, id, revision, path, association)
            }
        }

        fn monitored_login(
            &mut self,
            index: usize,
            approval: &str,
        ) -> Result<(ProfileBinding, LoginOutcome), CommandError> {
            let revision = self.storage.restore_state().unwrap().revision;
            let (id, path) = &self.homes[index];
            let receipt = self
                .provider
                .begin_login(id, approval, id, revision, path)?;
            let prompt = self
                .provider
                .await_login_prompt(id, &receipt.attempt_id)?
                .wait()?
                .publish()?;
            assert_eq!(prompt.user_code, "SYNTH-1234");
            assert_eq!(
                prompt.verification_url,
                "https://auth.openai.com/codex/device"
            );
            let completed = self
                .provider
                .complete_login(id, &receipt.attempt_id)?
                .wait()?;
            let binding = completed.binding.clone();
            Ok((binding, completed.publish(|_, _| Ok(()))?))
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

    fn monitored_fixture(mode: &str, require_login: bool) -> Fixture {
        let fixture = Fixture::new(false);
        fs::write(
            &fixture.executable,
            include_bytes!(
                "../../../../crates/openusage-codex/tests/fixtures/synthetic-collection-provider.py"
            ),
        )
        .unwrap();
        for (_, path) in &fixture.homes {
            fs::write(
                path.join("synthetic-collection-config.json"),
                serde_json::to_vec(
                    &serde_json::json!({"mode": mode, "requireLogin": require_login}),
                )
                .unwrap(),
            )
            .unwrap();
        }
        fixture
    }

    fn monitored_stage(fixture: &Fixture, index: usize, stage: &str) -> libc::pid_t {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Ok(records) = fs::read_to_string(
                fixture.homes[index]
                    .1
                    .join("synthetic-collection-observations.jsonl"),
            ) {
                for row in records
                    .lines()
                    .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
                {
                    if row["kind"] == stage {
                        return row["pid"].as_i64().unwrap().try_into().unwrap();
                    }
                }
            }
            assert!(
                Instant::now() < deadline,
                "Synthetic monitored fixture did not reach {stage}."
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn scheduled_collection_ignores_and_does_not_consume_or_extend_manual_cooldown() {
        let mut fixture = monitored_fixture("success", false);
        let approval = fixture.approve().approval_id;
        let id = fixture.homes[0].0.clone();
        let receipt = fixture.collection(0, &approval, true).unwrap();
        fixture
            .provider
            .complete_collection(&id, &receipt.request_id)
            .unwrap()
            .wait()
            .unwrap()
            .abort();
        assert_eq!(
            fixture
                .provider
                .refresh_status(&id)
                .unwrap()
                .retry_after_seconds,
            0
        );
        let receipt = fixture.collection(0, &approval, false).unwrap();
        fixture
            .provider
            .complete_collection(&id, &receipt.request_id)
            .unwrap()
            .wait()
            .unwrap()
            .abort();
        let started = fixture.provider.0.lock().unwrap().refresh_starts[&id];
        assert!(
            fixture
                .provider
                .refresh_status(&id)
                .unwrap()
                .retry_after_seconds
                > 0
        );
        assert_eq!(
            fixture.provider.scheduled_retry_after_seconds(&id).unwrap(),
            0
        );
        let receipt = fixture.collection(0, &approval, true).unwrap();
        fixture
            .provider
            .complete_collection(&id, &receipt.request_id)
            .unwrap()
            .wait()
            .unwrap()
            .abort();
        assert_eq!(
            fixture.provider.0.lock().unwrap().refresh_starts[&id],
            started
        );
        assert_eq!(
            fixture.collection(0, &approval, false).err(),
            Some(CommandError::ProviderRefreshCooldown)
        );
    }

    #[test]
    fn scheduled_collection_shares_process_slots_cancellation_and_transient_backoff() {
        let mut fixture = monitored_fixture("activity_hang", false);
        let approval = fixture.approve().approval_id;
        let first_id = fixture.homes[0].0.clone();
        let second_id = fixture.homes[1].0.clone();
        let first = fixture.collection(0, &approval, true).unwrap();
        let first_pid = monitored_stage(&fixture, 0, "activity-pending");
        let second = fixture.collection(1, &approval, false).unwrap();
        let second_pid = monitored_stage(&fixture, 1, "activity-pending");
        assert_eq!(
            fixture.collection(0, &approval, true).err(),
            Some(CommandError::ProviderCollectionInProgress)
        );
        assert_eq!(
            fixture.collection(2, &approval, true).err(),
            Some(CommandError::ProviderCapacityReached)
        );
        let first_wait = fixture
            .provider
            .complete_collection(&first_id, &first.request_id)
            .unwrap();
        fixture
            .provider
            .cancel_collection(&first_id, &first.request_id)
            .unwrap();
        assert_eq!(
            first_wait.wait().err(),
            Some(CommandError::ProviderCollectionCancelled)
        );
        gone(first_pid);
        assert_eq!(
            fixture
                .provider
                .scheduled_retry_after_seconds(&first_id)
                .unwrap(),
            0
        );
        let second_wait = fixture
            .provider
            .complete_collection(&second_id, &second.request_id)
            .unwrap();
        let command = fs::read(format!("/proc/{second_pid}/cmdline")).unwrap();
        assert!(
            command
                .split(|byte| *byte == 0)
                .any(|part| part == fixture.executable.as_os_str().as_encoded_bytes())
        );
        assert_eq!(unsafe { libc::getpgid(second_pid) }, second_pid);
        assert_eq!(unsafe { libc::killpg(second_pid, libc::SIGKILL) }, 0);
        assert!(second_wait.wait().is_err());
        gone(second_pid);
        assert!(
            (24..=36).contains(
                &fixture
                    .provider
                    .scheduled_retry_after_seconds(&second_id)
                    .unwrap()
            )
        );
        assert_eq!(
            fixture.collection(1, &approval, true).err(),
            Some(CommandError::ProviderRefreshBackoff)
        );
        assert_eq!(
            fixture.collection(1, &approval, false).err(),
            Some(CommandError::ProviderRefreshBackoff)
        );
    }

    #[test]
    fn scheduled_collection_retains_typed_auth_identity_and_conflict_failures_without_backoff() {
        for (mode, expected) in [
            ("signed_out", CommandError::ProviderAuthenticationRequired),
            ("unsupported_auth", CommandError::ProviderUnsupported),
            ("no_auth_required", CommandError::ProviderUnsupported),
            ("unknown", CommandError::ProviderAccountIdentityUnavailable),
            ("changed_email", CommandError::ProviderAccountChanged),
        ] {
            let mut fixture = monitored_fixture(mode, false);
            let approval = fixture.approve().approval_id;
            let id = fixture.homes[0].0.clone();
            let receipt = fixture.collection(0, &approval, true).unwrap();
            assert_eq!(
                fixture
                    .provider
                    .complete_collection(&id, &receipt.request_id)
                    .unwrap()
                    .wait()
                    .err(),
                Some(expected),
                "{mode}"
            );
            assert_eq!(
                fixture.provider.scheduled_retry_after_seconds(&id).unwrap(),
                0
            );
            assert_eq!(
                fixture
                    .provider
                    .refresh_status(&id)
                    .unwrap()
                    .retry_after_seconds,
                0
            );
        }
    }

    #[test]
    fn monitored_login_fixture_requires_matching_explicit_login_before_collection_and_redacts_logs()
    {
        let mut fixture = monitored_fixture("success", true);
        let approval = fixture.approve().approval_id;
        let id = fixture.homes[0].0.clone();
        let diagnostic = fixture.start(0).unwrap().wait().unwrap().finish().unwrap();
        assert_eq!(
            diagnostic.authentication,
            DiagnosticAuthentication::AuthenticationRequired
        );
        let receipt = fixture.collection(0, &approval, true).unwrap();
        assert_eq!(
            fixture
                .provider
                .complete_collection(&id, &receipt.request_id)
                .unwrap()
                .wait()
                .err(),
            Some(CommandError::ProviderAuthenticationRequired)
        );
        let (binding, outcome) = fixture.monitored_login(0, &approval).unwrap();
        assert!(outcome.diagnostic.persisted);
        assert_eq!(
            outcome.diagnostic.identity_confidence,
            DiagnosticIdentity::Provisional
        );
        assert!(fixture.provider.binding_is_current(&binding));
        let receipt = fixture.collection(0, &approval, true).unwrap();
        let completed = fixture
            .provider
            .complete_collection(&id, &receipt.request_id)
            .unwrap()
            .wait()
            .unwrap();
        assert!(completed.observation_anchors().0.is_some());
        assert!(completed.observation_anchors().1.is_some());
        completed.abort();
        assert_eq!(
            fs::read_to_string(
                fixture.homes[0]
                    .1
                    .join("synthetic-monitored-login-completed")
            )
            .unwrap(),
            "Synthetic matching login completion. No credentials.\n"
        );
        assert!(
            !fixture.homes[1]
                .1
                .join("synthetic-monitored-login-completed")
                .exists()
        );
        let records = fs::read_to_string(
            fixture.homes[0]
                .1
                .join("synthetic-collection-observations.jsonl"),
        )
        .unwrap();
        for forbidden in [
            "SYNTH-1234",
            "synthetic-monitored-attempt",
            "https://auth.openai.com",
        ] {
            assert!(!records.contains(forbidden));
        }
        let times: Vec<u128> = records
            .lines()
            .map(|line| {
                serde_json::from_str::<serde_json::Value>(line).unwrap()["monotonicNs"]
                    .as_str()
                    .unwrap()
                    .parse()
                    .unwrap()
            })
            .collect();
        assert!(times.windows(2).all(|pair| pair[0] <= pair[1]));
    }

    #[test]
    fn monitored_login_fixture_failed_or_cancelled_attempts_do_not_create_login_marker() {
        let mut failed = monitored_fixture("login_failed", true);
        let approval = failed.approve().approval_id;
        assert_eq!(
            failed.monitored_login(0, &approval).err(),
            Some(CommandError::ProviderLoginFailed)
        );
        assert!(
            !failed.homes[0]
                .1
                .join("synthetic-monitored-login-completed")
                .exists()
        );
        let mut fixture = monitored_fixture("login_pending", true);
        let approval = fixture.approve().approval_id;
        let revision = fixture.storage.restore_state().unwrap().revision;
        let (id, path) = &fixture.homes[0];
        let receipt = fixture
            .provider
            .begin_login(id, &approval, id, revision, path)
            .unwrap();
        fixture
            .provider
            .await_login_prompt(id, &receipt.attempt_id)
            .unwrap()
            .wait()
            .unwrap()
            .publish()
            .unwrap();
        fixture
            .provider
            .cancel_login(id, &receipt.attempt_id)
            .unwrap();
        assert!(!path.join("synthetic-monitored-login-completed").exists());
    }

    #[test]
    fn runtime_login_binding_check_refuses_reapproval_quiescence_replaced_home_source_and_close() {
        for fault in [
            "reapprove",
            "quiesce",
            "home",
            "source",
            "hardlink",
            "close",
        ] {
            let mut fixture = monitored_fixture("success", true);
            let approval = fixture.approve().approval_id;
            let (binding, _) = fixture.monitored_login(0, &approval).unwrap();
            assert!(fixture.provider.binding_is_current(&binding));
            match fault {
                "reapprove" => {
                    fixture.approve();
                }
                "quiesce" => fixture.provider.quiesce().unwrap(),
                "home" => {
                    let path = &fixture.homes[0].1;
                    fs::rename(path, path.with_extension("retained")).unwrap();
                    crate::workspace::prepare_directory(path).unwrap();
                }
                "source" => fs::write(&fixture.executable, b"#!/bin/sh\nexit 1\n").unwrap(),
                "hardlink" => fs::hard_link(
                    &fixture.executable,
                    fixture.executable.with_extension("linked"),
                )
                .unwrap(),
                "close" => fixture.provider.close().unwrap(),
                _ => unreachable!(),
            }
            assert!(!fixture.provider.binding_is_current(&binding), "{fault}");
        }
    }

    #[test]
    fn collection_publication_rechecks_source_before_persisting() {
        let mut fixture = monitored_fixture("success", false);
        let approval = fixture.approve().approval_id;
        let id = fixture.homes[0].0.clone();
        let receipt = fixture.collection(0, &approval, true).unwrap();
        let completed = fixture
            .provider
            .complete_collection(&id, &receipt.request_id)
            .unwrap()
            .wait()
            .unwrap();
        fs::write(&fixture.executable, b"#!/bin/sh\nexit 1\n").unwrap();
        assert_eq!(
            completed
                .publish(|_, _, _, _, _| panic!("Changed source must not reach storage."))
                .err(),
            Some(CommandError::StaleProviderApproval)
        );
        assert!(
            !fixture
                .provider
                .0
                .lock()
                .unwrap()
                .collections
                .contains_key(&receipt.request_id)
        );
    }

    #[test]
    fn scoped_cancellation_rejects_old_diagnostics_and_preserves_other_profile_workers() {
        let mut fixture = Fixture::new(false);
        let approval = fixture.approve().approval_id;
        let first = fixture.homes[0].0.clone();
        let second = fixture.homes[1].0.clone();
        fs::write(fixture._temporary.path().join("synthetic-provider-config.json"),
            serde_json::to_vec(&serde_json::json!({"defaultMode":"provisional","modesByHome":{(fixture.homes[1].1.to_str().unwrap()):"hang"}})).unwrap()).unwrap();
        let old = fixture.start(0).unwrap().wait().unwrap();
        let other = fixture.start(1).unwrap();
        let other_pid = fixture.child(1);
        let generation = fixture.provider.0.lock().unwrap().generation;
        assert!(
            fixture
                .provider
                .cancel_profile_operations(&first)
                .unwrap()
                .cancelled
        );
        assert_eq!(fixture.provider.0.lock().unwrap().generation, generation);
        assert_eq!(
            fixture
                .provider
                .0
                .lock()
                .unwrap()
                .approved
                .as_ref()
                .unwrap()
                .metadata
                .approval_id,
            approval
        );
        assert_eq!(unsafe { libc::kill(other_pid, 0) }, 0);
        let fresh = fixture.start(0).unwrap().wait().unwrap();
        assert_eq!(
            old.finish().err(),
            Some(CommandError::ProviderCheckCancelled)
        );
        assert!(
            fresh.finish().is_ok(),
            "An old token must not remove its replacement."
        );
        drop(fixture.start(0).unwrap());
        assert!(fixture.start(0).unwrap().wait().unwrap().finish().is_ok());
        fixture.provider.cancel_profile_operations(&second).unwrap();
        assert_eq!(
            other.wait().err(),
            Some(CommandError::ProviderCheckCancelled)
        );
        gone(other_pid);
    }

    #[test]
    fn stale_completed_auth_error_cannot_survive_scoped_cancel_and_new_login() {
        let mut fixture = monitored_fixture("signed_out", false);
        let approval = fixture.approve().approval_id;
        let id = fixture.homes[0].0.clone();
        let receipt = fixture.collection(0, &approval, true).unwrap();
        let pending = fixture
            .provider
            .complete_collection(&id, &receipt.request_id)
            .unwrap();
        let pid = monitored_stage(&fixture, 0, "request");
        let deadline = Instant::now() + Duration::from_secs(5);
        while unsafe { libc::kill(pid, 0) } == 0 {
            assert!(
                Instant::now() < deadline,
                "Old signed-out operation did not finish."
            );
            std::thread::sleep(Duration::from_millis(5));
        }
        gone(pid);
        fixture.provider.cancel_profile_operations(&id).unwrap();
        fs::write(
            fixture.homes[0].1.join("synthetic-collection-config.json"),
            br#"{"mode":"success","requireLogin":true}"#,
        )
        .unwrap();
        let (binding, _) = fixture.monitored_login(0, &approval).unwrap();
        assert_eq!(
            pending.wait().err(),
            Some(CommandError::ProviderCollectionCancelled)
        );
        assert!(fixture.provider.binding_is_current(&binding));
        assert_eq!(
            fixture.provider.scheduled_retry_after_seconds(&id).unwrap(),
            0
        );
    }

    #[test]
    fn scoped_cancellation_fences_completed_login_and_collection_publication() {
        let mut fixture = monitored_fixture("success", true);
        let approval = fixture.approve().approval_id;
        let id = fixture.homes[0].0.clone();
        let revision = fixture.storage.restore_state().unwrap().revision;
        let receipt = fixture
            .provider
            .begin_login(&id, &approval, &id, revision, &fixture.homes[0].1)
            .unwrap();
        fixture
            .provider
            .await_login_prompt(&id, &receipt.attempt_id)
            .unwrap()
            .wait()
            .unwrap()
            .publish()
            .unwrap();
        let completed = fixture
            .provider
            .complete_login(&id, &receipt.attempt_id)
            .unwrap()
            .wait()
            .unwrap();
        fixture.provider.cancel_profile_operations(&id).unwrap();
        assert_eq!(
            completed
                .publish(|_, _| panic!("Cancelled login must not reach storage."))
                .err(),
            Some(CommandError::ProviderLoginCancelled)
        );
        let receipt = fixture.collection(0, &approval, true).unwrap();
        let completed = fixture
            .provider
            .complete_collection(&id, &receipt.request_id)
            .unwrap()
            .wait()
            .unwrap();
        fixture.provider.cancel_profile_operations(&id).unwrap();
        assert_eq!(
            completed
                .publish(|_, _, _, _, _| panic!("Cancelled collection must not reach storage."))
                .err(),
            Some(CommandError::ProviderCollectionCancelled)
        );
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
