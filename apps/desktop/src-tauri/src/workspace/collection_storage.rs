//! Combined readings share one provisional association and one SQLCipher transaction.
use super::activity_storage::{CachedAssociationReview, ReadKind};
use super::*;
use openusage_codex::session::CollectionProbe;
use openusage_storage::{
    ActivityError, ActivitySourceKind, ProfileBindingInput, ProfileIdentityEvidence,
};
use std::time::Instant;

impl Workspace {
    pub fn finish_profile_collection_read(
        &mut self,
        completed: provider::CompletedCollection,
    ) -> Result<provider::CollectionOutcome, CommandError> {
        let id = completed.binding.profile_id.clone();
        let request = completed.request_id.clone();
        if self.quota_requests.get(&id) != Some(&request)
            || self.activity_requests.get(&id) != Some(&request)
        {
            completed.abort();
            return Err(CommandError::ProviderCollectionCancelled);
        }
        if let Err(error) = self.require_provider_binding(
            &id,
            &completed.binding.home_reference,
            &completed.binding.revision,
            CommandError::ProviderCollectionCancelled,
        ) {
            completed.abort();
            self.note_collection_failure(&id, &request);
            return Err(error);
        }
        let (quota_anchor, activity_anchor) = completed.observation_anchors();
        let mut result =
            completed.publish(|binding, probe, quota_run, activity_run, association| {
                self.persist_collection(binding, probe, quota_run, activity_run, association)
            });
        if let Err(error) = &result {
            self.note_monitoring_read_error(&id, &request, *error);
        }
        if let Ok(value) = &mut result {
            value.monitoring = self.monitoring_state().as_str();
        }
        match &result {
            Ok(value) => {
                if let Some(quota) = &value.quota {
                    self.quota_sessions
                        .insert(id.clone(), (quota.collection_run_id.clone(), quota_anchor));
                } else if self
                    .storage
                    .as_ref()
                    .is_some_and(|storage| matches!(storage.current_quota(&id), Ok(None)))
                {
                    self.quota_sessions.remove(&id);
                }
                if let Some(activity) = &value.activity {
                    self.activity_sessions.insert(
                        id.clone(),
                        (activity.collection_run_id.clone(), activity_anchor),
                    );
                } else if self
                    .storage
                    .as_ref()
                    .is_some_and(|storage| matches!(storage.current_activity(&id), Ok(None)))
                {
                    self.activity_sessions.remove(&id);
                }
                if value.accepted {
                    self.restore_preview.clear();
                    self.activity_reviews.remove(&id);
                }
            }
            Err(_) => self.note_collection_failure(&id, &request),
        }
        result
    }
    pub fn note_collection_failure(&mut self, id: &str, request: &str) {
        self.note_quota_failure(id, request);
        self.note_activity_failure(id, request);
    }

    /// Record a current signed-out response without changing saved measurements or eligibility.
    pub fn note_collection_read_error(
        &mut self,
        binding: &provider::ProfileBinding,
        request: &str,
        error: CommandError,
    ) -> Result<(), CommandError> {
        let id = &binding.profile_id;
        if error != CommandError::ProviderAuthenticationRequired
            || self.quota_requests.get(id).map(String::as_str) != Some(request)
            || self.activity_requests.get(id).map(String::as_str) != Some(request)
            || !self.provider.binding_is_current(binding)
        {
            return Ok(());
        }
        match self.require_provider_binding(
            id,
            &binding.home_reference,
            &binding.revision,
            CommandError::ProviderCollectionCancelled,
        ) {
            Err(CommandError::ProviderCollectionCancelled) => return Ok(()),
            result => result?,
        }
        self.storage
            .as_mut()
            .ok_or(CommandError::WorkspaceLocked)?
            .record_profile_connection(
                id,
                &binding.home_reference,
                ProfileConnectionState::AuthenticationRequired,
            )
            .map_err(profile_error)?;
        self.restore_preview.clear();
        Ok(())
    }

    fn persist_collection(
        &mut self,
        binding: &provider::ProfileBinding,
        probe: &CollectionProbe,
        quota_run: &str,
        activity_run: &str,
        association: &provider::ReadAssociation,
    ) -> Result<Option<openusage_storage::CollectionCommit>, CommandError> {
        let Some(first_observation) = probe
            .quota
            .as_ref()
            .map(|r| r.observed_at)
            .or_else(|| probe.activity.as_ref().map(|r| r.observed_at))
        else {
            // Neither capability exists: no new binding, transaction, or accepted-success reset.
            return Ok(None);
        };
        let source_kind = match binding.source_kind {
            provider::ExecutableSourceKind::OfficialArtifact => {
                ActivitySourceKind::OfficialArtifact
            }
            #[cfg(any(test, feature = "native-test"))]
            provider::ExecutableSourceKind::TestFixture => ActivitySourceKind::TestFixture,
            provider::ExecutableSourceKind::Unverified => {
                return Err(CommandError::ProviderSourceUnverified);
            }
        };
        let evidence = ProfileIdentityEvidence {
            email: probe
                .account_evidence
                .email
                .clone()
                .ok_or(CommandError::ProviderAccountIdentityUnavailable)?,
            plan: probe.account_evidence.plan.clone(),
        };
        let timestamp =
            |value| restore::timestamp(value).ok_or(CommandError::ProviderCollectionSaveFailed);
        let observed = timestamp(first_observation)?;
        let started = timestamp(probe.started_at)?;
        let completed = timestamp(probe.completed_at)?;
        let storage = self.storage.as_mut().ok_or(CommandError::WorkspaceLocked)?;
        let current = storage
            .profile_binding(&binding.profile_id)
            .map_err(save_error)?;
        if current.as_ref().map(|b| b.binding_id.as_str())
            != association.expected_binding.as_deref()
        {
            return Err(CommandError::ProviderCollectionCancelled);
        }
        if let Some(review) = &association.review {
            if review.current != evidence {
                return Err(CommandError::ProviderAccountChanged);
            }
        } else if let Some(existing) = &current {
            let previous = storage
                .profile_binding_evidence(existing)
                .map_err(save_error)?;
            if existing.home_reference != binding.home_reference || previous != evidence {
                self.activity_reviews.insert(
                    binding.profile_id.clone(),
                    CachedAssociationReview {
                        kind: ReadKind::Collection,
                        review_id: provider::new_id()?,
                        approval_id: binding.approval_id.clone(),
                        previous_binding: existing.clone(),
                        previous,
                        observed_at_utc: observed,
                        fence: provider::ReadReviewFence {
                            binding: binding.clone(),
                            current: evidence,
                            created: Instant::now(),
                        },
                    },
                );
                return Err(CommandError::ProviderCollectionAssociationChanged);
            }
        }
        let create_binding = current.is_none() || association.review.is_some();
        let binding_id = if create_binding {
            provider::new_id()?
        } else {
            current
                .as_ref()
                .ok_or(CommandError::ProviderCollectionCancelled)?
                .binding_id
                .clone()
        };
        let input = create_binding.then(|| ProfileBindingInput {
            profile_id: binding.profile_id.clone(),
            expected_home_reference: binding.home_reference.clone(),
            expected_previous_binding_id: association.expected_binding.clone(),
            new_binding_id: binding_id.clone(),
            evidence: evidence.clone(),
            observed_at_utc: observed,
        });
        let local_binding = openusage_storage::ProfileBinding {
            profile_id: binding.profile_id.clone(),
            home_reference: binding.home_reference.clone(),
            binding_id,
        };
        let quota = probe
            .quota
            .as_ref()
            .map(|read| -> Result<_, CommandError> {
                let quota = &read.value;
                Ok(openusage_storage::QuotaReading {
                    run_id: quota_run.into(),
                    binding: local_binding.clone(),
                    identity_evidence: evidence.clone(),
                    provider_version: probe.version.into(),
                    source_kind,
                    started_at_utc: started.clone(),
                    observed_at_utc: timestamp(read.observed_at)?,
                    completed_at_utc: completed.clone(),
                    limits: quota
                        .limits
                        .iter()
                        .map(|l| openusage_storage::QuotaLimit {
                            limit_id: l.limit_id.clone(),
                            limit_name: l.limit_name.clone(),
                            primary: l.primary.as_ref().map(quota_storage::stored_window),
                            secondary: l.secondary.as_ref().map(quota_storage::stored_window),
                            reached_type: l.reached_type.clone(),
                            monthly_credit_limit: l.monthly_credit_limit.clone(),
                            spend_control_reached: l.spend_control_reached,
                        })
                        .collect(),
                    available_reset_credits: quota
                        .available_reset_credits
                        .as_ref()
                        .map(|v| v.as_str().into()),
                    reset_credit_details: quota.reset_credit_details.as_ref().map(|rows| {
                        rows.iter()
                            .map(|d| openusage_storage::QuotaCreditDetail {
                                title: d.title.clone(),
                                status: match d.status {
                                    openusage_codex::ResetCreditStatus::Available => {
                                        openusage_storage::QuotaCreditStatus::Available
                                    }
                                    openusage_codex::ResetCreditStatus::Redeeming => {
                                        openusage_storage::QuotaCreditStatus::Redeeming
                                    }
                                    openusage_codex::ResetCreditStatus::Redeemed => {
                                        openusage_storage::QuotaCreditStatus::Redeemed
                                    }
                                    openusage_codex::ResetCreditStatus::Unknown => {
                                        openusage_storage::QuotaCreditStatus::Unknown
                                    }
                                },
                                granted_at_unix: d.granted_at_unix,
                                expires_at_unix: d.expires_at_unix,
                            })
                            .collect()
                    }),
                })
            })
            .transpose()?;
        let activity = probe
            .activity
            .as_ref()
            .map(|read| -> Result<_, CommandError> {
                let values = &read.value.summary;
                let exact = |v: &Option<openusage_codex::DecimalCount>| {
                    v.as_ref().map(|v| v.as_str().into())
                };
                Ok(openusage_storage::ActivityReading {
                    run_id: activity_run.into(),
                    binding: local_binding.clone(),
                    identity_evidence: evidence.clone(),
                    provider_version: probe.version.into(),
                    source_kind,
                    started_at_utc: started.clone(),
                    observed_at_utc: timestamp(read.observed_at)?,
                    completed_at_utc: completed.clone(),
                    summary: openusage_storage::ActivitySummaryValues {
                        lifetime_tokens: exact(&values.lifetime_tokens),
                        peak_daily_tokens: exact(&values.peak_daily_tokens),
                        longest_running_turn_sec: exact(&values.longest_running_turn_sec),
                        current_streak_days: exact(&values.current_streak_days),
                        longest_streak_days: exact(&values.longest_streak_days),
                    },
                    daily_usage: read.value.daily_usage_buckets.as_ref().map(|rows| {
                        rows.iter()
                            .map(|r| openusage_storage::ActivityDayValue {
                                provider_date: r.start_date.clone(),
                                tokens: r.tokens.as_str().into(),
                            })
                            .collect()
                    }),
                })
            })
            .transpose()?;
        storage
            .accept_collection(quota, activity, input)
            .map(Some)
            .map_err(save_error)
    }
}
fn save_error(error: ActivityError) -> CommandError {
    match error {
        ActivityError::IdentityChanged => CommandError::ProviderCollectionAssociationChanged,
        ActivityError::BindingChanged
        | ActivityError::AlreadyAccepted
        | ActivityError::ProfileUnavailable
        | ActivityError::StaleObservation => CommandError::ProviderCollectionCancelled,
        _ => CommandError::ProviderCollectionSaveFailed,
    }
}
