//! Saved quota readings and their provisional profile association remain Rust-controlled.
use super::activity_storage::{
    ActivityFreshness, CachedAssociationReview, ReadKind, freshness_for,
};
use super::*;
use openusage_codex::session::QuotaProbe;
use openusage_storage::{
    ActivityError, ActivitySourceKind, ProfileBindingInput, ProfileIdentityEvidence,
    QuotaResetAssessment, QuotaResetState, QuotaWindowRole,
};
use std::time::{Duration, Instant, SystemTime};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SavedProfileQuota {
    profile_id: String,
    binding_id: String,
    collection_run_id: String,
    provider_version: String,
    source_kind: &'static str,
    observed_at_utc: String,
    identity_confidence: &'static str,
    verified_identity_key: Option<String>,
    monitoring: &'static str,
    accepted: bool,
    coverage: &'static str,
    scope: &'static str,
    freshness: ActivityFreshness,
    limits: Vec<provider::QuotaLimitView>,
    available_reset_credits: Option<String>,
    reset_credit_details: Option<Vec<SavedCreditDetail>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SavedCreditDetail {
    title: Option<String>,
    status: &'static str,
    granted_at_unix: i64,
    expires_at_unix: Option<i64>,
}

impl Workspace {
    pub fn load_profile_quota(&self, id: &str) -> Result<Option<SavedProfileQuota>, CommandError> {
        validate_profile_id(id)?;
        let storage = self.storage.as_ref().ok_or(CommandError::WorkspaceLocked)?;
        if !storage
            .list_profiles()
            .map_err(profile_error)?
            .iter()
            .any(|p| p.id == id)
        {
            return Err(CommandError::ProfileNotFound);
        }
        let Some(saved) = storage
            .current_quota(id)
            .map_err(|_| CommandError::ProviderQuotaLoadFailed)?
        else {
            return Ok(None);
        };
        let session = self
            .quota_sessions
            .get(id)
            .filter(|(run, _)| run == &saved.commit.collection_run_id);
        let now = SystemTime::now();
        let freshness = freshness_for(
            session,
            &saved.commit.observed_at_utc,
            now,
            Duration::from_secs(600),
        );
        let limits = saved
            .limits
            .into_iter()
            .map(|limit| {
                let window = |value: Option<openusage_storage::QuotaWindow>, role, fallback| {
                    value
                        .map(|w| {
                            let assessment = saved
                                .reset_assessments
                                .iter()
                                .find(|a| a.limit_id == limit.limit_id && a.window_role == role)
                                .ok_or(CommandError::ProviderQuotaLoadFailed)?;
                            Ok::<_, CommandError>(window_view(w, fallback, assessment, now))
                        })
                        .transpose()
                };
                Ok(provider::QuotaLimitView {
                    primary: window(limit.primary, QuotaWindowRole::Primary, "Primary window")?,
                    secondary: window(
                        limit.secondary,
                        QuotaWindowRole::Secondary,
                        "Secondary window",
                    )?,
                    limit_id: limit.limit_id,
                    limit_name: limit.limit_name,
                    reached_type: limit.reached_type,
                    monthly_credit_limit: limit.monthly_credit_limit,
                    spend_control_reached: limit.spend_control_reached,
                })
            })
            .collect::<Result<Vec<_>, CommandError>>()?;
        Ok(Some(SavedProfileQuota {
            profile_id: id.into(),
            binding_id: saved.binding.binding_id,
            collection_run_id: saved.commit.collection_run_id,
            provider_version: saved.provider_version,
            source_kind: saved.source_kind.as_str(),
            observed_at_utc: saved.commit.observed_at_utc,
            identity_confidence: "provisional",
            verified_identity_key: None,
            monitoring: self.monitoring_state().as_str(),
            accepted: true,
            coverage: "unknown",
            scope: openusage_storage::QUOTA_SOURCE_SCOPE,
            freshness,
            limits,
            available_reset_credits: saved.available_reset_credits,
            reset_credit_details: saved.reset_credit_details.map(|rows| {
                rows.into_iter()
                    .map(|d| SavedCreditDetail {
                        title: d.title,
                        status: d.status.as_str(),
                        granted_at_unix: d.granted_at_unix,
                        expires_at_unix: d.expires_at_unix,
                    })
                    .collect()
            }),
        }))
    }

    pub fn finish_profile_quota_read(
        &mut self,
        completed: provider::CompletedQuota,
    ) -> Result<provider::QuotaOutcome, CommandError> {
        let id = completed.binding.profile_id.clone();
        let request = completed.request_id.clone();
        if self.quota_requests.get(&id) != Some(&request) {
            completed.abort();
            return Err(CommandError::ProviderQuotaCancelled);
        }
        let binding = &completed.binding;
        if let Err(error) = self.require_provider_binding(
            &id,
            &binding.home_reference,
            &binding.revision,
            CommandError::ProviderQuotaCancelled,
        ) {
            completed.abort();
            self.note_quota_failure(&id, &request);
            return Err(error);
        }
        let observed_monotonic = completed.observed_monotonic();
        let mut result = completed.publish(|binding, probe, request, association| {
            self.persist_quota(binding, probe, request, association)
        });
        if let Err(error) = &result {
            self.note_monitoring_read_error(&id, &request, *error);
        }
        if let Ok(value) = &mut result {
            value.monitoring = self.monitoring_state().as_str();
        }
        match &result {
            Ok(value) => {
                self.quota_sessions.insert(
                    id.clone(),
                    (value.collection_run_id.clone(), Some(observed_monotonic)),
                );
                if self
                    .storage
                    .as_ref()
                    .and_then(|s| s.current_activity(&id).ok())
                    .flatten()
                    .is_none()
                {
                    self.activity_sessions.remove(&id);
                }
                self.activity_reviews.remove(&id);
                self.restore_preview.clear();
            }
            Err(_) => self.note_quota_failure(&id, &request),
        }
        result
    }

    pub fn note_quota_failure(&mut self, id: &str, request: &str) {
        if self.quota_requests.get(id).map(String::as_str) != Some(request) {
            return;
        }
        if let Some(saved) = self
            .storage
            .as_ref()
            .and_then(|s| s.current_quota(id).ok())
            .flatten()
        {
            self.quota_sessions
                .insert(id.into(), (saved.commit.collection_run_id, None));
        }
    }

    fn persist_quota(
        &mut self,
        binding: &provider::ProfileBinding,
        probe: &QuotaProbe,
        request: &str,
        association: &provider::ReadAssociation,
    ) -> Result<openusage_storage::QuotaCommit, CommandError> {
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
        let observed =
            restore::timestamp(probe.observed_at).ok_or(CommandError::ProviderQuotaSaveFailed)?;
        let storage = self.storage.as_mut().ok_or(CommandError::WorkspaceLocked)?;
        let current = storage
            .profile_binding(&binding.profile_id)
            .map_err(save_error)?;
        if current.as_ref().map(|b| b.binding_id.as_str())
            != association.expected_binding.as_deref()
        {
            return Err(CommandError::ProviderQuotaCancelled);
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
                        kind: ReadKind::Quota,
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
                return Err(CommandError::ProviderQuotaAssociationChanged);
            }
        }
        let create_binding = current.is_none() || association.review.is_some();
        let id = if create_binding {
            provider::new_id()?
        } else {
            current
                .as_ref()
                .ok_or(CommandError::ProviderQuotaCancelled)?
                .binding_id
                .clone()
        };
        let input = create_binding.then(|| ProfileBindingInput {
            profile_id: binding.profile_id.clone(),
            expected_home_reference: binding.home_reference.clone(),
            expected_previous_binding_id: association.expected_binding.clone(),
            new_binding_id: id.clone(),
            evidence: evidence.clone(),
            observed_at_utc: observed.clone(),
        });
        storage
            .accept_quota(
                openusage_storage::QuotaReading {
                    run_id: request.into(),
                    binding: openusage_storage::ProfileBinding {
                        profile_id: binding.profile_id.clone(),
                        home_reference: binding.home_reference.clone(),
                        binding_id: id,
                    },
                    identity_evidence: evidence,
                    provider_version: probe.version.into(),
                    source_kind,
                    started_at_utc: restore::timestamp(probe.started_at)
                        .ok_or(CommandError::ProviderQuotaSaveFailed)?,
                    completed_at_utc: restore::timestamp(probe.completed_at)
                        .ok_or(CommandError::ProviderQuotaSaveFailed)?,
                    observed_at_utc: observed,
                    limits: probe
                        .quota
                        .limits
                        .iter()
                        .map(|l| openusage_storage::QuotaLimit {
                            limit_id: l.limit_id.clone(),
                            limit_name: l.limit_name.clone(),
                            primary: l.primary.as_ref().map(stored_window),
                            secondary: l.secondary.as_ref().map(stored_window),
                            reached_type: l.reached_type.clone(),
                            monthly_credit_limit: l.monthly_credit_limit.clone(),
                            spend_control_reached: l.spend_control_reached,
                        })
                        .collect(),
                    available_reset_credits: probe
                        .quota
                        .available_reset_credits
                        .as_ref()
                        .map(|v| v.as_str().into()),
                    reset_credit_details: probe.quota.reset_credit_details.as_ref().map(|rows| {
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
                },
                input,
            )
            .map_err(save_error)
    }
}

pub(super) fn stored_window(w: &openusage_codex::QuotaWindow) -> openusage_storage::QuotaWindow {
    openusage_storage::QuotaWindow {
        used_percent: w.used_percent,
        window_duration_mins: w.window_duration_mins,
        resets_at_unix: w.resets_at_unix,
    }
}
pub(super) fn reset_due(reset: Option<i64>, now: SystemTime) -> bool {
    let Some(reset @ -62_135_596_800..=253_402_300_799) = reset else {
        return false;
    };
    let time = |seconds: i64| {
        let duration = Duration::from_secs(seconds.unsigned_abs());
        if seconds >= 0 {
            std::time::UNIX_EPOCH.checked_add(duration)
        } else {
            std::time::UNIX_EPOCH.checked_sub(duration)
        }
    };
    match (time(-62_135_596_800), time(253_402_300_800), time(reset)) {
        (Some(first), Some(end), Some(deadline)) => now >= first && now < end && now >= deadline,
        _ => false,
    }
}

fn window_view(
    w: openusage_storage::QuotaWindow,
    fallback: &str,
    assessment: &QuotaResetAssessment,
    now: SystemTime,
) -> provider::QuotaWindowView {
    // Keep a prior unresolved deadline visible even if the provider moves or omits it.
    let state = if reset_due(assessment.pending_reset_at_unix, now) {
        "due"
    } else if assessment.state == QuotaResetState::Confirmed {
        "confirmed"
    } else if assessment.pending_reset_at_unix == w.resets_at_unix {
        "expected"
    } else {
        "unconfirmed"
    };
    let mut view = provider::QuotaWindowView::from_window(
        openusage_codex::QuotaWindow {
            used_percent: w.used_percent,
            derived_remaining_percent: 100 - w.used_percent,
            window_duration_mins: w.window_duration_mins,
            resets_at_unix: w.resets_at_unix,
        },
        fallback,
    );
    view.reset_state = Some(state);
    view
}
fn save_error(e: ActivityError) -> CommandError {
    match e {
        ActivityError::IdentityChanged => CommandError::ProviderQuotaAssociationChanged,
        ActivityError::BindingChanged
        | ActivityError::AlreadyAccepted
        | ActivityError::ProfileUnavailable
        | ActivityError::StaleObservation => CommandError::ProviderQuotaCancelled,
        _ => CommandError::ProviderQuotaSaveFailed,
    }
}
