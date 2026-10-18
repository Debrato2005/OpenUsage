//! One-shot profile activity. Publish only after the Rust storage transaction commits.

use super::*;
use openusage_codex::{session::ActivityProbe, supervisor::ActivityHandle};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ActivityReceipt {
    pub profile_id: String,
    pub request_id: String,
    pub approval_id: String,
    pub source_kind: ExecutableSourceKind,
    pub monitoring: &'static str,
    pub accepted: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ActivityOutcome {
    pub request_id: String,
    pub profile_id: String,
    pub approval_id: String,
    pub provider_version: &'static str,
    pub source_kind: ExecutableSourceKind,
    pub observed_at_utc: String,
    pub identity_confidence: &'static str,
    pub verified_identity_key: Option<String>,
    pub monitoring: &'static str,
    pub accepted: bool,
    pub collection_run_id: String,
    pub binding_id: String,
    pub coverage: &'static str,
    #[serde(flatten)]
    pub activity: openusage_codex::ActivitySnapshot,
}

pub(super) struct Entry {
    pub(super) binding: ProfileBinding,
    handle: Option<ActivityHandle>,
    expected_activity_binding: Option<String>,
}

pub(crate) struct CompletingActivity {
    coordinator: ProviderCoordinator,
    request_id: String,
    binding: ProfileBinding,
    handle: ActivityHandle,
    expected_activity_binding: Option<String>,
}

pub(crate) struct CompletedActivity {
    coordinator: ProviderCoordinator,
    pub request_id: String,
    pub binding: ProfileBinding,
    probe: ActivityProbe,
    expected_activity_binding: Option<String>,
}

impl ProviderCoordinator {
    pub fn begin_activity(
        &self,
        profile_id: &str,
        approval_id: &str,
        home_reference: &str,
        revision: openusage_storage::StorageRevision,
        path: &Path,
        expected_activity_binding: Option<String>,
    ) -> Result<ActivityReceipt, CommandError> {
        validate_ids(profile_id, approval_id)?;
        let held_home = Arc::new(HeldHome::open(path)?);
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
        if state.logins.len() + state.quotas.len() + state.activities.len() >= 2 {
            return Err(CommandError::ProviderCapacityReached);
        }
        let approved = state
            .approved
            .as_mut()
            .ok_or(CommandError::ProviderExecutableNotApproved)?;
        if approved.metadata.approval_id != approval_id {
            return Err(CommandError::StaleProviderApproval);
        }
        approved.held.recheck()?;
        let executable = approved.executable.clone();
        let source_kind = approved.metadata.source_kind;
        let request_id = new_id()?;
        let handle = state
            .supervisor
            .start_activity_diagnostic(&executable, &home)
            .map_err(activity_error)?;
        let binding = ProfileBinding {
            profile_id: profile_id.into(),
            home_reference: home_reference.into(),
            revision,
            held_home,
            approval_id: approval_id.into(),
            source_kind,
            generation: state.generation,
        };
        let receipt = ActivityReceipt {
            profile_id: profile_id.into(),
            request_id: request_id.clone(),
            approval_id: approval_id.into(),
            source_kind,
            monitoring: "stopped",
            accepted: false,
        };
        state.activities.insert(
            request_id,
            Entry {
                binding,
                handle: Some(handle),
                expected_activity_binding,
            },
        );
        Ok(receipt)
    }

    pub fn complete_activity(
        &self,
        profile_id: &str,
        request_id: &str,
    ) -> Result<CompletingActivity, CommandError> {
        validate_ids(profile_id, request_id)?;
        let mut state = self
            .0
            .lock()
            .map_err(|_| CommandError::ProviderUnavailable)?;
        require_open(&state)?;
        let entry = state
            .activities
            .get_mut(request_id)
            .filter(|entry| entry.binding.profile_id == profile_id)
            .ok_or(CommandError::ProviderActivityNotFound)?;
        let handle = entry
            .handle
            .take()
            .ok_or(CommandError::ProviderActivityInProgress)?;
        Ok(CompletingActivity {
            coordinator: self.clone(),
            request_id: request_id.into(),
            binding: entry.binding.clone(),
            expected_activity_binding: entry.expected_activity_binding.clone(),
            handle,
        })
    }

    pub fn cancel_activity(
        &self,
        profile_id: &str,
        request_id: &str,
    ) -> Result<Cancelled, CommandError> {
        validate_ids(profile_id, request_id)?;
        let mut state = self
            .0
            .lock()
            .map_err(|_| CommandError::ProviderUnavailable)?;
        require_open(&state)?;
        if !state
            .activities
            .get(request_id)
            .is_some_and(|entry| entry.binding.profile_id == profile_id)
        {
            return Ok(Cancelled { cancelled: false });
        }
        // Keep the exact request match locked until cleanup finishes; stale tokens cannot cancel a successor.
        state
            .supervisor
            .cancel(&ProfileId::parse(profile_id).map_err(provider_error)?)
            .map_err(activity_error)?;
        state.activities.remove(request_id);
        Ok(Cancelled { cancelled: true })
    }

    fn discard_activity(&self, request_id: &str) {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .activities
            .remove(request_id);
    }
}

fn validate_ids(profile_id: &str, request_id: &str) -> Result<(), CommandError> {
    crate::workspace::validate_profile_id(profile_id)?;
    crate::restore::validate_preview_id(request_id)
}

impl CompletingActivity {
    /// The caller releases Workspace before waiting for the owned provider process.
    pub fn wait(self) -> Result<CompletedActivity, CommandError> {
        match self.handle.wait() {
            Ok(probe) => Ok(CompletedActivity {
                coordinator: self.coordinator,
                request_id: self.request_id,
                binding: self.binding,
                probe,
                expected_activity_binding: self.expected_activity_binding,
            }),
            Err(error) => {
                self.coordinator.discard_activity(&self.request_id);
                Err(activity_error(error))
            }
        }
    }
}

impl CompletedActivity {
    pub fn abort(self) {
        self.coordinator.discard_activity(&self.request_id);
    }

    /// Workspace remains locked. Keep the Provider fence locked through the database COMMIT.
    pub fn publish(
        self,
        persist: impl FnOnce(
            &ProfileBinding,
            &ActivityProbe,
            &str,
            Option<&str>,
        ) -> Result<openusage_storage::ActivityCommit, CommandError>,
    ) -> Result<ActivityOutcome, CommandError> {
        let mut state = self
            .coordinator
            .0
            .lock()
            .map_err(|_| CommandError::ProviderUnavailable)?;
        require_open(&state)?;
        let valid = state.generation == self.binding.generation
            && state
                .approved
                .as_ref()
                .is_some_and(|approved| approved.metadata.approval_id == self.binding.approval_id)
            && state.activities.get(&self.request_id).is_some_and(|entry| {
                entry.binding.profile_id == self.binding.profile_id && entry.handle.is_none()
            });
        state.activities.remove(&self.request_id);
        if !valid {
            return Err(CommandError::ProviderActivityCancelled);
        }
        self.binding.held_home.require_identity()?;
        if self.probe.version != TESTED_PROVIDER_VERSION {
            return Err(CommandError::ProviderUnsupported);
        }
        let observed_at_utc = crate::restore::timestamp(self.probe.observed_at)
            .ok_or(CommandError::ProviderUnavailable)?;
        let committed = persist(
            &self.binding,
            &self.probe,
            &self.request_id,
            self.expected_activity_binding.as_deref(),
        )?;
        Ok(ActivityOutcome {
            request_id: self.request_id,
            profile_id: self.binding.profile_id,
            approval_id: self.binding.approval_id,
            provider_version: self.probe.version,
            source_kind: self.binding.source_kind,
            observed_at_utc,
            identity_confidence: "provisional",
            verified_identity_key: None,
            monitoring: "stopped",
            accepted: true,
            collection_run_id: committed.collection_run_id,
            binding_id: committed.binding_id,
            coverage: "unknown",
            activity: self.probe.activity,
        })
    }
}

fn activity_error(error: SupervisorError) -> CommandError {
    use openusage_codex::ProtocolError as P;
    match error {
        SupervisorError::DuplicateProfile => CommandError::ProviderActivityInProgress,
        SupervisorError::Cancelled | SupervisorError::Session(SessionError::Cancelled) => {
            CommandError::ProviderActivityCancelled
        }
        SupervisorError::Session(SessionError::TimedOut) => CommandError::ProviderActivityTimedOut,
        SupervisorError::Session(SessionError::Protocol(P::AccountChanged)) => {
            CommandError::ProviderAccountChanged
        }
        SupervisorError::Session(SessionError::Protocol(P::AccountIdentityUnavailable)) => {
            CommandError::ProviderAccountIdentityUnavailable
        }
        SupervisorError::Session(SessionError::Protocol(_)) => CommandError::ProviderActivityFailed,
        other => provider_error(other),
    }
}
