//! Native approval and lifecycle for bounded account diagnostics. No account claims are persisted.
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
};

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
            | "581062e7eae9e315e4d62cc1dd770a263f96c700739087d905c956f1b66c6f18"
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
    approval_id: String,
    source_kind: ExecutableSourceKind,
    generation: u64,
}

struct ProviderState {
    closed: bool,
    generation: u64,
    candidate: Option<Candidate>,
    approved: Option<Approved>,
    supervisor: Arc<Supervisor>,
    logins: std::collections::HashMap<String, login::Entry>,
    quotas: std::collections::HashMap<String, quota::Entry>,
}
impl Default for ProviderState {
    fn default() -> Self {
        Self {
            closed: false,
            generation: 0,
            candidate: None,
            approved: None,
            supervisor: Arc::new(Supervisor::new()),
            logins: std::collections::HashMap::new(),
            quotas: std::collections::HashMap::new(),
        }
    }
}

#[derive(Clone, Default)]
pub(crate) struct ProviderCoordinator(Arc<Mutex<ProviderState>>);
impl ProviderCoordinator {
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
        if state
            .logins
            .values()
            .any(|login| login.binding.profile_id == profile_id)
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
        if state
            .logins
            .values()
            .any(|login| login.binding.profile_id == profile_id)
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
        Ok(())
    }
}

fn reset(state: &mut ProviderState) -> Result<(), CommandError> {
    state.supervisor.shutdown().map_err(provider_error)?;
    state.logins.clear();
    state.quotas.clear();
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
fn new_id() -> Result<String, CommandError> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|_| CommandError::ProviderUnavailable)?;
    Ok(uuid::Builder::from_random_bytes(bytes)
        .into_uuid()
        .to_string())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use openusage_storage::Storage;
    use std::{
        os::unix::fs::{PermissionsExt, symlink},
        time::{Duration, Instant},
    };

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
