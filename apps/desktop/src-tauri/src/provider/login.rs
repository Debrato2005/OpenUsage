//! Selected-profile sign-in ownership. Prompt and completion waits run outside application locks.

use super::*;
use openusage_codex::{DeviceLoginPrompt, supervisor::DeviceLoginHandle};

const WAIT_LIMIT_SECONDS: u32 = 900;
const LOGIN_BROWSER_URL: &str = "https://auth.openai.com/codex/device";

#[derive(Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LoginReceipt {
    pub profile_id: String,
    pub attempt_id: String,
    pub approval_id: String,
    pub source_kind: ExecutableSourceKind,
    pub monitoring: &'static str,
    pub application_wait_limit_seconds: u32,
}

#[derive(Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LoginPromptReceipt {
    pub profile_id: String,
    pub attempt_id: String,
    pub verification_url: String,
    pub user_code: String,
    pub source_kind: ExecutableSourceKind,
    pub application_wait_limit_seconds: u32,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LoginOutcome {
    pub attempt_id: String,
    pub diagnostic: ConnectionDiagnostic,
}

pub(crate) type LoginBinding = ProfileBinding;

#[derive(Clone, Copy, PartialEq)]
enum Phase {
    Reserved,
    AwaitingPrompt,
    Ready,
    Waiting,
}

pub(super) struct Entry {
    pub(super) binding: LoginBinding,
    phase: Phase,
    handle: Option<DeviceLoginHandle>,
}

pub(crate) struct AwaitingLoginPrompt {
    coordinator: ProviderCoordinator,
    attempt_id: String,
    binding: LoginBinding,
    handle: DeviceLoginHandle,
}

pub(crate) struct ReadyLoginPrompt {
    coordinator: ProviderCoordinator,
    attempt_id: String,
    pub binding: LoginBinding,
    handle: DeviceLoginHandle,
    prompt: DeviceLoginPrompt,
}

pub(crate) struct CompletingLogin {
    coordinator: ProviderCoordinator,
    attempt_id: String,
    binding: LoginBinding,
    handle: DeviceLoginHandle,
}

pub(crate) struct CompletedLogin {
    coordinator: ProviderCoordinator,
    attempt_id: String,
    pub binding: LoginBinding,
    probe: ProviderProbe,
}

impl ProviderCoordinator {
    pub fn login_browser_binding(
        &self,
        profile_id: &str,
        attempt_id: &str,
    ) -> Result<LoginBinding, CommandError> {
        validate_ids(profile_id, attempt_id)?;
        let state = self
            .0
            .lock()
            .map_err(|_| CommandError::ProviderUnavailable)?;
        browser_binding(&state, profile_id, attempt_id)
    }

    pub fn open_login_browser(
        &self,
        profile_id: &str,
        attempt_id: &str,
        dispatch: impl FnOnce(&'static str) -> Result<(), CommandError>,
    ) -> Result<(), CommandError> {
        validate_ids(profile_id, attempt_id)?;
        let state = self
            .0
            .lock()
            .map_err(|_| CommandError::ProviderUnavailable)?;
        browser_binding(&state, profile_id, attempt_id)?;
        // Serialize this handoff with cancellation and approval invalidation.
        dispatch(LOGIN_BROWSER_URL)
    }

    pub fn begin_login(
        &self,
        profile_id: &str,
        approval_id: &str,
        home_reference: &str,
        revision: openusage_storage::StorageRevision,
        path: &Path,
    ) -> Result<LoginReceipt, CommandError> {
        crate::restore::validate_preview_id(approval_id)?;
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
        let attempt_id = new_id()?;
        let handle = state
            .supervisor
            .start_device_login(&executable, &home)
            .map_err(login_error)?;
        let binding = LoginBinding {
            profile_id: profile_id.into(),
            home_reference: home_reference.into(),
            revision,
            held_home,
            approval_id: approval_id.into(),
            source_kind,
            generation: state.generation,
        };
        let receipt = LoginReceipt {
            profile_id: profile_id.into(),
            attempt_id: attempt_id.clone(),
            approval_id: approval_id.into(),
            source_kind,
            monitoring: "stopped",
            application_wait_limit_seconds: WAIT_LIMIT_SECONDS,
        };
        state.logins.insert(
            attempt_id,
            Entry {
                binding,
                phase: Phase::Reserved,
                handle: Some(handle),
            },
        );
        Ok(receipt)
    }

    pub fn await_login_prompt(
        &self,
        profile_id: &str,
        attempt_id: &str,
    ) -> Result<AwaitingLoginPrompt, CommandError> {
        validate_ids(profile_id, attempt_id)?;
        let mut state = self
            .0
            .lock()
            .map_err(|_| CommandError::ProviderUnavailable)?;
        require_open(&state)?;
        let entry = matching_entry(&mut state, profile_id, attempt_id)?;
        if entry.phase != Phase::Reserved {
            return Err(CommandError::ProviderLoginInProgress);
        }
        let handle = entry
            .handle
            .take()
            .ok_or(CommandError::ProviderLoginInProgress)?;
        entry.phase = Phase::AwaitingPrompt;
        Ok(AwaitingLoginPrompt {
            coordinator: self.clone(),
            attempt_id: attempt_id.into(),
            binding: entry.binding.clone(),
            handle,
        })
    }

    pub fn complete_login(
        &self,
        profile_id: &str,
        attempt_id: &str,
    ) -> Result<CompletingLogin, CommandError> {
        validate_ids(profile_id, attempt_id)?;
        let mut state = self
            .0
            .lock()
            .map_err(|_| CommandError::ProviderUnavailable)?;
        require_open(&state)?;
        let entry = matching_entry(&mut state, profile_id, attempt_id)?;
        if matches!(entry.phase, Phase::Reserved | Phase::AwaitingPrompt) {
            return Err(CommandError::ProviderLoginPromptPending);
        }
        if entry.phase != Phase::Ready {
            return Err(CommandError::ProviderLoginInProgress);
        }
        let handle = entry
            .handle
            .take()
            .ok_or(CommandError::ProviderLoginInProgress)?;
        entry.phase = Phase::Waiting;
        Ok(CompletingLogin {
            coordinator: self.clone(),
            attempt_id: attempt_id.into(),
            binding: entry.binding.clone(),
            handle,
        })
    }

    pub fn cancel_login(
        &self,
        profile_id: &str,
        attempt_id: &str,
    ) -> Result<Cancelled, CommandError> {
        validate_ids(profile_id, attempt_id)?;
        let mut state = self
            .0
            .lock()
            .map_err(|_| CommandError::ProviderUnavailable)?;
        require_open(&state)?;
        if !state
            .logins
            .get(attempt_id)
            .is_some_and(|entry| entry.binding.profile_id == profile_id)
        {
            return Ok(Cancelled { cancelled: false });
        }
        // Hold the exact attempt match through cleanup so a newer operation cannot receive this cancel.
        state
            .supervisor
            .cancel(&ProfileId::parse(profile_id).map_err(provider_error)?)
            .map_err(login_error)?;
        state.logins.remove(attempt_id);
        Ok(Cancelled { cancelled: true })
    }

    fn discard_login(&self, attempt_id: &str) {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .logins
            .remove(attempt_id);
    }
}

fn validate_ids(profile_id: &str, attempt_id: &str) -> Result<(), CommandError> {
    crate::workspace::validate_profile_id(profile_id)?;
    crate::restore::validate_preview_id(attempt_id)
}

fn matching_entry<'a>(
    state: &'a mut ProviderState,
    profile_id: &str,
    attempt_id: &str,
) -> Result<&'a mut Entry, CommandError> {
    state
        .logins
        .get_mut(attempt_id)
        .filter(|entry| entry.binding.profile_id == profile_id)
        .ok_or(CommandError::ProviderLoginNotFound)
}

fn browser_binding(
    state: &ProviderState,
    profile_id: &str,
    attempt_id: &str,
) -> Result<LoginBinding, CommandError> {
    require_open(state)?;
    let entry = state
        .logins
        .get(attempt_id)
        .filter(|entry| entry.binding.profile_id == profile_id)
        .ok_or(CommandError::ProviderLoginNotFound)?;
    if !matches!(entry.phase, Phase::Ready | Phase::Waiting) {
        return Err(CommandError::ProviderLoginPromptPending);
    }
    require_binding(state, attempt_id, &entry.binding, entry.phase)?;
    Ok(entry.binding.clone())
}

fn require_binding(
    state: &ProviderState,
    attempt_id: &str,
    binding: &LoginBinding,
    phase: Phase,
) -> Result<(), CommandError> {
    require_open(state)?;
    if state.generation != binding.generation
        || !state
            .approved
            .as_ref()
            .is_some_and(|approved| approved.metadata.approval_id == binding.approval_id)
        || !state.logins.get(attempt_id).is_some_and(|entry| {
            entry.binding.profile_id == binding.profile_id && entry.phase == phase
        })
    {
        return Err(CommandError::ProviderLoginCancelled);
    }
    binding.held_home.require_identity()
}

impl AwaitingLoginPrompt {
    pub fn wait(mut self) -> Result<ReadyLoginPrompt, CommandError> {
        match self.handle.prompt() {
            Ok(prompt) => Ok(ReadyLoginPrompt {
                coordinator: self.coordinator,
                attempt_id: self.attempt_id,
                binding: self.binding,
                handle: self.handle,
                prompt,
            }),
            Err(error) => {
                self.coordinator.discard_login(&self.attempt_id);
                Err(login_error(error))
            }
        }
    }
}

impl ReadyLoginPrompt {
    pub fn abort(self) {
        self.coordinator.discard_login(&self.attempt_id);
    }

    pub fn publish(self) -> Result<LoginPromptReceipt, CommandError> {
        let mut state = self
            .coordinator
            .0
            .lock()
            .map_err(|_| CommandError::ProviderUnavailable)?;
        if let Err(error) = require_binding(
            &state,
            &self.attempt_id,
            &self.binding,
            Phase::AwaitingPrompt,
        ) {
            state.logins.remove(&self.attempt_id);
            return Err(error);
        }
        let entry = matching_entry(&mut state, &self.binding.profile_id, &self.attempt_id)?;
        entry.phase = Phase::Ready;
        entry.handle = Some(self.handle);
        Ok(LoginPromptReceipt {
            profile_id: self.binding.profile_id,
            attempt_id: self.attempt_id,
            verification_url: self.prompt.verification_url.into(),
            user_code: self.prompt.user_code,
            source_kind: self.binding.source_kind,
            application_wait_limit_seconds: WAIT_LIMIT_SECONDS,
        })
    }
}

impl CompletingLogin {
    pub fn wait(self) -> Result<CompletedLogin, CommandError> {
        match self.handle.wait() {
            Ok(probe) => Ok(CompletedLogin {
                coordinator: self.coordinator,
                attempt_id: self.attempt_id,
                binding: self.binding,
                probe,
            }),
            Err(error) => {
                self.coordinator.discard_login(&self.attempt_id);
                Err(login_error(error))
            }
        }
    }
}

impl CompletedLogin {
    pub fn abort(self) {
        self.coordinator.discard_login(&self.attempt_id);
    }

    /// Keep the exact attempt and Provider lock through the profile-state COMMIT.
    pub fn publish(
        self,
        persist: impl FnOnce(&LoginBinding, &ConnectionDiagnostic) -> Result<(), CommandError>,
    ) -> Result<LoginOutcome, CommandError> {
        let mut state = self
            .coordinator
            .0
            .lock()
            .map_err(|_| CommandError::ProviderUnavailable)?;
        let result = (|| {
            require_binding(&state, &self.attempt_id, &self.binding, Phase::Waiting)?;
            let mut diagnostic = diagnostic(
                self.binding.profile_id.clone(),
                self.binding.approval_id.clone(),
                self.binding.source_kind,
                self.probe,
            )?;
            if diagnostic.authentication != DiagnosticAuthentication::Chatgpt
                || diagnostic.checked_at.is_none()
            {
                return Err(CommandError::ProviderLoginFailed);
            }
            persist(&self.binding, &diagnostic)?;
            diagnostic.persisted = true;
            Ok(LoginOutcome {
                attempt_id: self.attempt_id.clone(),
                diagnostic,
            })
        })();
        state.logins.remove(&self.attempt_id);
        result
    }
}

fn login_error(error: SupervisorError) -> CommandError {
    match error {
        SupervisorError::Cancelled | SupervisorError::Session(SessionError::Cancelled) => {
            CommandError::ProviderLoginCancelled
        }
        SupervisorError::Session(SessionError::TimedOut) => CommandError::ProviderLoginTimedOut,
        SupervisorError::Session(SessionError::Protocol(
            openusage_codex::ProtocolError::ProviderFailure
            | openusage_codex::ProtocolError::UnsupportedAuthentication,
        )) => CommandError::ProviderLoginFailed,
        _ => provider_error(error),
    }
}
