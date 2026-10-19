//! One explicit quota read. Publish only after Rust commits the complete validated reading.

use super::*;
use openusage_codex::{session::QuotaProbe, supervisor::QuotaHandle};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct QuotaReceipt {
    pub profile_id: String,
    pub request_id: String,
    pub approval_id: String,
    pub source_kind: ExecutableSourceKind,
    pub monitoring: &'static str,
    pub accepted: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct QuotaOutcome {
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
    pub binding_id: String,
    pub collection_run_id: String,
    pub limits: Vec<QuotaLimitView>,
    pub available_reset_credits: Option<openusage_codex::DecimalCount>,
    pub reset_credit_details: Option<Vec<openusage_codex::ResetCreditDetail>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct QuotaLimitView {
    pub limit_id: String,
    pub limit_name: Option<String>,
    pub primary: Option<QuotaWindowView>,
    pub secondary: Option<QuotaWindowView>,
    pub reached_type: Option<String>,
    pub monthly_credit_limit: Option<String>,
    pub spend_control_reached: Option<bool>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct QuotaWindowView {
    pub used_percent: u8,
    pub derived_remaining_percent: u8,
    pub window_duration_mins: Option<u64>,
    pub resets_at_unix: Option<i64>,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reset_state: Option<&'static str>,
}

impl QuotaWindowView {
    pub(crate) fn from_window(window: openusage_codex::QuotaWindow, fallback: &str) -> Self {
        let label = match window.window_duration_mins {
            Some(300) => "Five-hour".into(),
            Some(10080) => "Seven-day".into(),
            Some(minutes) => format!("{minutes} minutes"),
            None => fallback.into(),
        };
        Self {
            used_percent: window.used_percent,
            derived_remaining_percent: window.derived_remaining_percent,
            window_duration_mins: window.window_duration_mins,
            resets_at_unix: window.resets_at_unix,
            label,
            reset_state: None,
        }
    }
}

pub(super) struct Entry {
    pub(super) binding: ProfileBinding,
    handle: Option<QuotaHandle>,
    association: ReadAssociation,
}

pub(crate) struct CompletingQuota {
    coordinator: ProviderCoordinator,
    request_id: String,
    binding: ProfileBinding,
    handle: QuotaHandle,
    association: ReadAssociation,
}

pub(crate) struct CompletedQuota {
    coordinator: ProviderCoordinator,
    pub request_id: String,
    pub binding: ProfileBinding,
    probe: QuotaProbe,
    association: ReadAssociation,
}

impl ProviderCoordinator {
    pub fn begin_quota(
        &self,
        profile_id: &str,
        approval_id: &str,
        home_reference: &str,
        revision: openusage_storage::StorageRevision,
        path: &Path,
        association: ReadAssociation,
    ) -> Result<QuotaReceipt, CommandError> {
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
        if let Some(review) = &association.review {
            super::activity::require_review(&mut state, review)
                .map_err(|_| CommandError::ProviderQuotaReviewUnavailable)?;
            if review.binding.profile_id != profile_id
                || review.binding.home_reference != home_reference
                || review.binding.approval_id != approval_id
            {
                return Err(CommandError::ProviderQuotaReviewUnavailable);
            }
        }
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
        require_refresh_ready(&mut state, profile_id)?;
        let request_id = new_id()?;
        let started = Instant::now();
        let handle = state
            .supervisor
            .start_quota_diagnostic(&executable, &home)
            .map_err(quota_error)?;
        state.refresh_starts.insert(profile_id.into(), started);
        let binding = ProfileBinding {
            profile_id: profile_id.into(),
            home_reference: home_reference.into(),
            revision,
            held_home,
            approval_id: approval_id.into(),
            source_kind,
            generation: state.generation,
        };
        let receipt = QuotaReceipt {
            profile_id: profile_id.into(),
            request_id: request_id.clone(),
            approval_id: approval_id.into(),
            source_kind,
            monitoring: "stopped",
            accepted: false,
        };
        state.quotas.insert(
            request_id,
            Entry {
                binding,
                handle: Some(handle),
                association,
            },
        );
        Ok(receipt)
    }

    pub fn complete_quota(
        &self,
        profile_id: &str,
        request_id: &str,
    ) -> Result<CompletingQuota, CommandError> {
        validate_ids(profile_id, request_id)?;
        let mut state = self
            .0
            .lock()
            .map_err(|_| CommandError::ProviderUnavailable)?;
        require_open(&state)?;
        let entry = state
            .quotas
            .get_mut(request_id)
            .filter(|entry| entry.binding.profile_id == profile_id)
            .ok_or(CommandError::ProviderQuotaNotFound)?;
        let handle = entry
            .handle
            .take()
            .ok_or(CommandError::ProviderQuotaInProgress)?;
        Ok(CompletingQuota {
            coordinator: self.clone(),
            request_id: request_id.into(),
            binding: entry.binding.clone(),
            association: entry.association.clone(),
            handle,
        })
    }

    pub fn cancel_quota(
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
            .quotas
            .get(request_id)
            .is_some_and(|entry| entry.binding.profile_id == profile_id)
        {
            return Ok(Cancelled { cancelled: false });
        }
        // Keep the exact request match locked until cleanup finishes; stale tokens cannot cancel a successor.
        state
            .supervisor
            .cancel(&ProfileId::parse(profile_id).map_err(provider_error)?)
            .map_err(quota_error)?;
        state.quotas.remove(request_id);
        Ok(Cancelled { cancelled: true })
    }

    fn discard_quota(&self, request_id: &str) {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .quotas
            .remove(request_id);
    }
}

fn validate_ids(profile_id: &str, request_id: &str) -> Result<(), CommandError> {
    crate::workspace::validate_profile_id(profile_id)?;
    crate::restore::validate_preview_id(request_id)
}

impl CompletingQuota {
    /// The caller releases Workspace before waiting for the owned provider process.
    pub fn wait(self) -> Result<CompletedQuota, CommandError> {
        match self.handle.wait() {
            Ok(probe) => Ok(CompletedQuota {
                coordinator: self.coordinator,
                request_id: self.request_id,
                binding: self.binding,
                probe,
                association: self.association,
            }),
            Err(error) => {
                self.coordinator.discard_quota(&self.request_id);
                Err(quota_error(error))
            }
        }
    }
}

impl CompletedQuota {
    pub fn observed_monotonic(&self) -> std::time::Instant {
        self.probe.observed_monotonic
    }

    pub fn abort(self) {
        self.coordinator.discard_quota(&self.request_id);
    }

    /// Keep Workspace and Provider locked through the selected profile transaction.
    pub fn publish(
        self,
        persist: impl FnOnce(
            &ProfileBinding,
            &QuotaProbe,
            &str,
            &ReadAssociation,
        ) -> Result<openusage_storage::QuotaCommit, CommandError>,
    ) -> Result<QuotaOutcome, CommandError> {
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
            && state.quotas.get(&self.request_id).is_some_and(|entry| {
                entry.binding.profile_id == self.binding.profile_id && entry.handle.is_none()
            });
        state.quotas.remove(&self.request_id);
        if !valid {
            return Err(CommandError::ProviderQuotaCancelled);
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
            &self.association,
        )?;
        Ok(QuotaOutcome {
            binding_id: committed.binding_id,
            collection_run_id: committed.collection_run_id,
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
            available_reset_credits: self.probe.quota.available_reset_credits,
            reset_credit_details: self.probe.quota.reset_credit_details,
            limits: self
                .probe
                .quota
                .limits
                .into_iter()
                .map(|limit| QuotaLimitView {
                    limit_id: limit.limit_id,
                    limit_name: limit.limit_name,
                    primary: limit
                        .primary
                        .map(|window| QuotaWindowView::from_window(window, "Primary window")),
                    secondary: limit
                        .secondary
                        .map(|window| QuotaWindowView::from_window(window, "Secondary window")),
                    reached_type: limit.reached_type,
                    monthly_credit_limit: limit.monthly_credit_limit,
                    spend_control_reached: limit.spend_control_reached,
                })
                .collect(),
        })
    }
}

fn quota_error(error: SupervisorError) -> CommandError {
    use openusage_codex::ProtocolError as P;
    match error {
        SupervisorError::DuplicateProfile => CommandError::ProviderQuotaInProgress,
        SupervisorError::Cancelled | SupervisorError::Session(SessionError::Cancelled) => {
            CommandError::ProviderQuotaCancelled
        }
        SupervisorError::Session(SessionError::TimedOut) => CommandError::ProviderQuotaTimedOut,
        SupervisorError::Session(SessionError::Protocol(P::AccountChanged)) => {
            CommandError::ProviderAccountChanged
        }
        SupervisorError::Session(SessionError::Protocol(P::AccountIdentityUnavailable)) => {
            CommandError::ProviderAccountIdentityUnavailable
        }
        SupervisorError::Session(SessionError::Protocol(P::IncompatibleQuotaScope)) => {
            CommandError::ProviderIncompatibleQuotaScope
        }
        SupervisorError::Session(SessionError::Protocol(_)) => CommandError::ProviderQuotaFailed,
        other => provider_error(other),
    }
}
