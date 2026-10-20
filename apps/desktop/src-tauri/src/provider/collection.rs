//! One admitted refresh collects supported readings and publishes only after their shared COMMIT.
use super::*;
use openusage_codex::{session::CollectionProbe, supervisor::CollectionHandle};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CollectionReceipt {
    pub profile_id: String,
    pub request_id: String,
    pub approval_id: String,
    pub source_kind: ExecutableSourceKind,
    pub monitoring: &'static str,
    pub accepted: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CollectionReading {
    pub binding_id: String,
    pub collection_run_id: String,
    pub observed_at_utc: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CollectionOutcome {
    request_id: String,
    profile_id: String,
    approval_id: String,
    provider_version: &'static str,
    source_kind: ExecutableSourceKind,
    identity_confidence: &'static str,
    verified_identity_key: Option<String>,
    monitoring: &'static str,
    pub accepted: bool,
    /// None means this method explicitly returned UnsupportedMethod, never an empty reading.
    pub quota: Option<CollectionReading>,
    pub activity: Option<CollectionReading>,
}

pub(super) struct Entry {
    pub(super) binding: ProfileBinding,
    handle: Option<CollectionHandle>,
    association: ReadAssociation,
    activity_run_id: String,
}

pub(crate) struct CompletingCollection {
    coordinator: ProviderCoordinator,
    request_id: String,
    activity_run_id: String,
    binding: ProfileBinding,
    association: ReadAssociation,
    handle: CollectionHandle,
}

pub(crate) struct CompletedCollection {
    coordinator: ProviderCoordinator,
    pub request_id: String,
    pub binding: ProfileBinding,
    activity_run_id: String,
    association: ReadAssociation,
    probe: CollectionProbe,
}

impl ProviderCoordinator {
    pub fn begin_collection(
        &self,
        profile_id: &str,
        approval_id: &str,
        home_reference: &str,
        revision: openusage_storage::StorageRevision,
        path: &Path,
        association: ReadAssociation,
    ) -> Result<CollectionReceipt, CommandError> {
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
                .map_err(|_| CommandError::ProviderCollectionReviewUnavailable)?;
            if review.binding.profile_id != profile_id
                || review.binding.home_reference != home_reference
                || review.binding.approval_id != approval_id
            {
                return Err(CommandError::ProviderCollectionReviewUnavailable);
            }
        }
        if state.active_count() >= 2 {
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
        let activity_run_id = new_id()?;
        let started = Instant::now();
        let handle = state
            .supervisor
            .start_collection_diagnostic(&executable, &home)
            .map_err(provider_error)?;
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
        let receipt = CollectionReceipt {
            profile_id: profile_id.into(),
            request_id: request_id.clone(),
            approval_id: approval_id.into(),
            source_kind,
            monitoring: "stopped",
            accepted: false,
        };
        state.collections.insert(
            request_id,
            Entry {
                binding,
                handle: Some(handle),
                association,
                activity_run_id,
            },
        );
        Ok(receipt)
    }

    pub fn complete_collection(
        &self,
        profile_id: &str,
        request_id: &str,
    ) -> Result<CompletingCollection, CommandError> {
        validate_ids(profile_id, request_id)?;
        let mut state = self
            .0
            .lock()
            .map_err(|_| CommandError::ProviderUnavailable)?;
        require_open(&state)?;
        let entry = state
            .collections
            .get_mut(request_id)
            .filter(|e| e.binding.profile_id == profile_id)
            .ok_or(CommandError::ProviderCollectionNotFound)?;
        let handle = entry
            .handle
            .take()
            .ok_or(CommandError::ProviderCollectionInProgress)?;
        Ok(CompletingCollection {
            coordinator: self.clone(),
            request_id: request_id.into(),
            activity_run_id: entry.activity_run_id.clone(),
            binding: entry.binding.clone(),
            association: entry.association.clone(),
            handle,
        })
    }

    pub fn cancel_collection(
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
            .collections
            .get(request_id)
            .is_some_and(|e| e.binding.profile_id == profile_id)
        {
            return Ok(Cancelled { cancelled: false });
        }
        state
            .supervisor
            .cancel(&ProfileId::parse(profile_id).map_err(provider_error)?)
            .map_err(provider_error)?;
        state.collections.remove(request_id);
        Ok(Cancelled { cancelled: true })
    }

    fn discard_collection(&self, request: &str) {
        self.0
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .collections
            .remove(request);
    }
}
fn validate_ids(profile: &str, request: &str) -> Result<(), CommandError> {
    crate::workspace::validate_profile_id(profile)?;
    crate::restore::validate_preview_id(request)
}
impl CompletingCollection {
    pub fn wait(self) -> Result<CompletedCollection, CommandError> {
        match self.handle.wait() {
            Ok(probe) => Ok(CompletedCollection {
                coordinator: self.coordinator,
                request_id: self.request_id,
                activity_run_id: self.activity_run_id,
                binding: self.binding,
                association: self.association,
                probe,
            }),
            Err(error) => {
                let mut state = self.coordinator.0.lock().unwrap_or_else(|p| p.into_inner());
                let owned = state
                    .collections
                    .get(&self.request_id)
                    .is_some_and(|entry| {
                        entry.binding.profile_id == self.binding.profile_id
                            && entry.binding.generation == self.binding.generation
                            && entry.binding.approval_id == self.binding.approval_id
                            && entry.handle.is_none()
                    });
                record_refresh_failure(&mut state, &self.binding, error, owned);
                state.collections.remove(&self.request_id);
                Err(if error == SupervisorError::Cancelled {
                    CommandError::ProviderCollectionCancelled
                } else {
                    provider_error(error)
                })
            }
        }
    }
}
impl CompletedCollection {
    pub fn observation_anchors(&self) -> (Option<Instant>, Option<Instant>) {
        (
            self.probe.quota.as_ref().map(|r| r.observed_monotonic),
            self.probe.activity.as_ref().map(|r| r.observed_monotonic),
        )
    }
    pub fn abort(self) {
        self.coordinator.discard_collection(&self.request_id);
    }
    /// Workspace -> Provider remain locked through the single storage transaction.
    pub fn publish(
        self,
        persist: impl FnOnce(
            &ProfileBinding,
            &CollectionProbe,
            &str,
            &str,
            &ReadAssociation,
        )
            -> Result<Option<openusage_storage::CollectionCommit>, CommandError>,
    ) -> Result<CollectionOutcome, CommandError> {
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
                .is_some_and(|a| a.metadata.approval_id == self.binding.approval_id)
            && state.collections.get(&self.request_id).is_some_and(|e| {
                e.binding.profile_id == self.binding.profile_id && e.handle.is_none()
            });
        state.collections.remove(&self.request_id);
        if !valid {
            return Err(CommandError::ProviderCollectionCancelled);
        }
        self.binding.held_home.require_identity()?;
        if self.probe.version != TESTED_PROVIDER_VERSION {
            return Err(CommandError::ProviderUnsupported);
        }
        let committed = persist(
            &self.binding,
            &self.probe,
            &self.request_id,
            &self.activity_run_id,
            &self.association,
        )?;
        let accepted = committed.is_some();
        if accepted {
            state.refresh_backoff.remove(&self.binding.profile_id);
        }
        let (quota, activity) = committed
            .map(|c| {
                (
                    c.quota.map(|r| CollectionReading {
                        binding_id: r.binding_id,
                        collection_run_id: r.collection_run_id,
                        observed_at_utc: r.observed_at_utc,
                    }),
                    c.activity.map(|r| CollectionReading {
                        binding_id: r.binding_id,
                        collection_run_id: r.collection_run_id,
                        observed_at_utc: r.observed_at_utc,
                    }),
                )
            })
            .unwrap_or_default();
        Ok(CollectionOutcome {
            request_id: self.request_id,
            profile_id: self.binding.profile_id,
            approval_id: self.binding.approval_id,
            provider_version: self.probe.version,
            source_kind: self.binding.source_kind,
            identity_confidence: "provisional",
            verified_identity_key: None,
            monitoring: "stopped",
            accepted,
            quota,
            activity,
        })
    }
}
