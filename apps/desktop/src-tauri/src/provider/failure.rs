//! Owned read failures retain the same publication fences as accepted readings.
use super::*;
use std::{fmt, time::SystemTime};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ReadOperation {
    Quota,
    Activity,
    Collection,
}
impl ReadOperation {
    fn cancelled(self) -> CommandError {
        match self {
            Self::Quota => CommandError::ProviderQuotaCancelled,
            Self::Activity => CommandError::ProviderActivityCancelled,
            Self::Collection => CommandError::ProviderCollectionCancelled,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ReadFailureKind {
    AuthenticationRequired,
    TransientFailure,
}
impl ReadFailureKind {
    pub(super) fn from_error(error: SupervisorError) -> Option<Self> {
        match error {
            SupervisorError::Session(SessionError::Protocol(
                openusage_codex::ProtocolError::AuthenticationRequired,
            )) => Some(Self::AuthenticationRequired),
            SupervisorError::Session(
                SessionError::TimedOut
                | SessionError::TransportFailed
                | SessionError::UnexpectedExit,
            ) => Some(Self::TransientFailure),
            _ => None,
        }
    }
}

pub(crate) struct ReadFailureEvidence {
    pub kind: ReadFailureKind,
    pub request_id: String,
    pub expected_binding_id: Option<String>,
    pub observed_at: SystemTime,
    pub observed_monotonic: Instant,
    pub source_kind: ExecutableSourceKind,
    /// Approved interface target, not a claim that a failed version probe replied.
    pub tested_provider_version: &'static str,
}

#[derive(Debug)]
pub(crate) struct JoinedReadError {
    pub error: CommandError,
    pub failure: Option<Box<CompletedReadFailure>>,
}

pub(crate) struct CompletedReadFailure {
    coordinator: ProviderCoordinator,
    binding: ProfileBinding,
    operation: ReadOperation,
    evidence: ReadFailureEvidence,
    consumed: bool,
}
impl fmt::Debug for CompletedReadFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("CompletedReadFailure { .. }")
    }
}
impl CompletedReadFailure {
    pub(super) fn capture(
        coordinator: ProviderCoordinator,
        binding: ProfileBinding,
        request_id: String,
        expected_binding_id: Option<String>,
        operation: ReadOperation,
        kind: ReadFailureKind,
    ) -> Self {
        let evidence = ReadFailureEvidence {
            kind,
            request_id,
            expected_binding_id,
            observed_at: SystemTime::now(),
            observed_monotonic: Instant::now(),
            source_kind: binding.source_kind,
            tested_provider_version: TESTED_PROVIDER_VERSION,
        };
        Self {
            coordinator,
            binding,
            operation,
            evidence,
            consumed: false,
        }
    }

    pub fn binding(&self) -> &ProfileBinding {
        &self.binding
    }
    pub fn request_id(&self) -> &str {
        &self.evidence.request_id
    }
    pub fn operation(&self) -> ReadOperation {
        self.operation
    }

    fn owns_request(&self, state: &ProviderState) -> bool {
        let binding = match self.operation {
            ReadOperation::Quota => state
                .quotas
                .get(self.request_id())
                .filter(|entry| entry.failure_ready && entry.handle.is_none())
                .map(|entry| &entry.binding),
            ReadOperation::Activity => state
                .activities
                .get(self.request_id())
                .filter(|entry| entry.failure_ready && entry.handle.is_none())
                .map(|entry| &entry.binding),
            ReadOperation::Collection => state
                .collections
                .get(self.request_id())
                .filter(|entry| entry.failure_ready && entry.handle.is_none())
                .map(|entry| &entry.binding),
        };
        binding.is_some_and(|binding| {
            binding.profile_id == self.binding.profile_id
                && binding.home_reference == self.binding.home_reference
                && binding.generation == self.binding.generation
                && binding.approval_id == self.binding.approval_id
                && binding.source_kind == self.binding.source_kind
        })
    }
    fn remove_owned(&self, state: &mut ProviderState) {
        if !self.owns_request(state) {
            return;
        }
        match self.operation {
            ReadOperation::Quota => {
                state.quotas.remove(self.request_id());
            }
            ReadOperation::Activity => {
                state.activities.remove(self.request_id());
            }
            ReadOperation::Collection => {
                state.collections.remove(self.request_id());
            }
        }
    }

    /// Workspace -> Provider remain locked through the durable failure transaction.
    pub fn publish(
        mut self,
        persist: impl FnOnce(&ProfileBinding, &ReadFailureEvidence) -> Result<(), CommandError>,
    ) -> Result<(), CommandError> {
        let coordinator = self.coordinator.clone();
        let mut state = coordinator
            .0
            .lock()
            .map_err(|_| CommandError::ProviderUnavailable)?;
        // Consume on every outcome. Drop must not re-lock Provider while its
        // publication guard is held, including an early error or failed COMMIT.
        self.consumed = true;
        let current = self.owns_request(&state) && binding_current(&state, &self.binding);
        self.remove_owned(&mut state);
        if !current {
            return Err(self.operation.cancelled());
        }
        let approved = state
            .approved
            .as_mut()
            .ok_or(CommandError::StaleProviderApproval)?;
        if approved.metadata.source_kind != self.evidence.source_kind
            || approved.metadata.tested_version != self.evidence.tested_provider_version
        {
            return Err(CommandError::StaleProviderApproval);
        }
        approved.held.recheck()?;
        self.binding.held_home.require_identity()?;
        persist(&self.binding, &self.evidence)
    }

    pub fn abort(self) {
        drop(self);
    }
}
impl Drop for CompletedReadFailure {
    fn drop(&mut self) {
        if !self.consumed {
            let mut state = self
                .coordinator
                .0
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            self.remove_owned(&mut state);
        }
    }
}
