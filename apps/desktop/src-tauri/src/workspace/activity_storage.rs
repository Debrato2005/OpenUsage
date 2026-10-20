//! Durable profile activity. Only explicit review exposes non-secret account descriptions.

use super::*;
use openusage_codex::{DecimalCount, session::ActivityProbe};
use openusage_storage::{
    ActivityDailyState, ActivityDayValue, ActivityError, ActivityReading, ActivitySourceKind,
    ActivitySummaryValues, ProfileBindingInput, ProfileIdentityEvidence,
};
use std::time::{Duration, Instant, SystemTime};

const FRESH_FOR: Duration = Duration::from_secs(30 * 60);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReadAssociationReview {
    pub review_id: String,
    profile_id: String,
    approval_id: String,
    observed_at_utc: String,
    previous: ReadAssociationHint,
    current: ReadAssociationHint,
    home_changed: bool,
    identity_confidence: &'static str,
}
#[derive(Serialize)]
struct ReadAssociationHint {
    email: String,
    plan: Option<String>,
}
impl From<&ProfileIdentityEvidence> for ReadAssociationHint {
    fn from(evidence: &ProfileIdentityEvidence) -> Self {
        Self {
            email: evidence.email.clone(),
            plan: evidence.plan.clone(),
        }
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ReadKind {
    Activity,
    Quota,
    Collection,
}
impl ReadKind {
    pub fn unavailable(self) -> CommandError {
        match self {
            Self::Activity => CommandError::ProviderActivityReviewUnavailable,
            Self::Quota => CommandError::ProviderQuotaReviewUnavailable,
            Self::Collection => CommandError::ProviderCollectionReviewUnavailable,
        }
    }
}
#[derive(Clone)]
pub(super) struct CachedAssociationReview {
    pub review_id: String,
    pub kind: ReadKind,
    pub approval_id: String,
    pub previous_binding: openusage_storage::ProfileBinding,
    pub previous: ProfileIdentityEvidence,
    pub observed_at_utc: String,
    pub fence: provider::ReadReviewFence,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SavedProfileActivity {
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
    summary: SavedActivitySummary,
    daily_state: &'static str,
    daily_usage_buckets: Vec<SavedActivityDay>,
    daily_chart: SavedActivityChart,
    daily_truncated: bool,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SavedActivitySummary {
    lifetime_tokens: Option<String>,
    peak_daily_tokens: Option<String>,
    longest_running_turn_sec: Option<String>,
    current_streak_days: Option<String>,
    longest_streak_days: Option<String>,
}
impl From<ActivitySummaryValues> for SavedActivitySummary {
    fn from(value: ActivitySummaryValues) -> Self {
        Self {
            lifetime_tokens: value.lifetime_tokens,
            peak_daily_tokens: value.peak_daily_tokens,
            longest_running_turn_sec: value.longest_running_turn_sec,
            current_streak_days: value.current_streak_days,
            longest_streak_days: value.longest_streak_days,
        }
    }
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SavedActivityDay {
    date: String,
    tokens: String,
    observed_at_utc: String,
    freshness: ActivityFreshness,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SavedActivityChart {
    max_tokens: Option<String>,
    bars: Vec<SavedActivityBar>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SavedActivityBar {
    date: String,
    height_basis_points: u16,
}

fn daily_chart(days: &[SavedActivityDay]) -> Result<SavedActivityChart, CommandError> {
    let counts = days
        .iter()
        .map(|day| {
            day.tokens
                .parse::<i64>()
                .ok()
                .filter(|value| *value >= 0 && value.to_string() == day.tokens)
                .map(|value| value as u128)
                .ok_or(CommandError::ProviderActivityLoadFailed)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let maximum = counts.iter().copied().max();
    Ok(SavedActivityChart {
        max_tokens: maximum.map(|value| value.to_string()),
        bars: days
            .iter()
            .zip(counts)
            .map(|(day, value)| SavedActivityBar {
                date: day.date.clone(),
                height_basis_points: match maximum {
                    Some(maximum) if maximum > 0 => (value * 10_000 / maximum) as u16,
                    _ => 0,
                },
            })
            .collect(),
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum ActivityFreshness {
    Unknown,
    Fresh,
    Stale,
}

impl Workspace {
    /// Return an already observed mismatch; this command never starts a provider.
    pub fn review_profile_activity_association(
        &mut self,
        id: &str,
        approval_id: &str,
    ) -> Result<ReadAssociationReview, CommandError> {
        self.review_profile_association(id, approval_id, ReadKind::Activity)
    }

    pub(super) fn review_profile_association(
        &mut self,
        id: &str,
        approval_id: &str,
        kind: ReadKind,
    ) -> Result<ReadAssociationReview, CommandError> {
        let review = self.valid_profile_review(id, approval_id, kind)?;
        Ok(ReadAssociationReview {
            review_id: review.review_id,
            profile_id: id.into(),
            approval_id: approval_id.into(),
            observed_at_utc: review.observed_at_utc,
            previous: (&review.previous).into(),
            current: (&review.fence.current).into(),
            home_changed: review.previous_binding.home_reference
                != review.fence.binding.home_reference,
            identity_confidence: "provisional",
        })
    }

    pub(super) fn valid_activity_review(
        &mut self,
        id: &str,
        approval_id: &str,
    ) -> Result<CachedAssociationReview, CommandError> {
        self.valid_profile_review(id, approval_id, ReadKind::Activity)
    }

    pub(super) fn valid_profile_review(
        &mut self,
        id: &str,
        approval_id: &str,
        kind: ReadKind,
    ) -> Result<CachedAssociationReview, CommandError> {
        restore::validate_preview_id(id)?;
        restore::validate_preview_id(approval_id)?;
        self.storage.as_ref().ok_or(CommandError::WorkspaceLocked)?;
        let review = self
            .activity_reviews
            .get(id)
            .cloned()
            .ok_or(kind.unavailable())?;
        let (home, _, _) = self
            .selected_provider_home(id)
            .map_err(|_| kind.unavailable())?;
        let storage = self.storage.as_ref().ok_or(CommandError::WorkspaceLocked)?;
        if review.kind != kind
            || review.approval_id != approval_id
            || review.fence.created.elapsed() >= provider::ACTIVITY_REVIEW_LIFETIME
            || home != review.fence.binding.home_reference
            || storage
                .profile_binding(id)
                .map_err(|_| kind.unavailable())?
                .as_ref()
                != Some(&review.previous_binding)
            || storage
                .profile_binding_evidence(&review.previous_binding)
                .map_err(|_| kind.unavailable())?
                != review.previous
        {
            return Err(kind.unavailable());
        }
        self.provider
            .validate_activity_review(&review.fence)
            .map_err(|_| kind.unavailable())?;
        Ok(review)
    }

    /// Read saved records after unlock, including archived profiles, without executable approval.
    pub fn load_profile_activity(
        &self,
        id: &str,
    ) -> Result<Option<SavedProfileActivity>, CommandError> {
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
        let Some(current) = storage.current_activity(id).map_err(load_error)? else {
            return Ok(None);
        };
        let days = storage
            .activity_days(
                id,
                "0001-01-01",
                "9999-12-31",
                openusage_storage::MAX_ACTIVITY_ROWS_PER_READ,
            )
            .map_err(load_error)?;
        let session = self
            .activity_sessions
            .get(id)
            .filter(|(run, _)| run == &current.commit.collection_run_id);
        let now = SystemTime::now();
        let observed_at_utc = current.commit.observed_at_utc;
        let summary_freshness = freshness(session, &observed_at_utc, now);
        let daily_usage_buckets: Vec<_> = days
            .rows
            .into_iter()
            .map(|day| SavedActivityDay {
                date: day.value.provider_date,
                tokens: day.value.tokens,
                freshness: freshness(session, &day.observed_at_utc, now),
                observed_at_utc: day.observed_at_utc,
            })
            .collect();
        let daily_chart = daily_chart(&daily_usage_buckets)?;
        Ok(Some(SavedProfileActivity {
            profile_id: id.into(),
            binding_id: current.binding.binding_id,
            collection_run_id: current.commit.collection_run_id,
            provider_version: current.provider_version,
            source_kind: current.source_kind.as_str(),
            observed_at_utc,
            identity_confidence: "provisional",
            verified_identity_key: None,
            monitoring: self.monitoring_state().as_str(),
            accepted: true,
            coverage: "unknown",
            scope: openusage_storage::ACTIVITY_SOURCE_SCOPE,
            freshness: summary_freshness,
            summary: current.summary.into(),
            daily_state: match current.daily_state {
                ActivityDailyState::Unavailable => "unavailable",
                ActivityDailyState::Empty => "empty",
                ActivityDailyState::Returned => "returned",
            },
            daily_usage_buckets,
            daily_chart,
            daily_truncated: days.truncated,
        }))
    }

    pub fn finish_profile_activity_read(
        &mut self,
        completed: provider::CompletedActivity,
    ) -> Result<provider::ActivityOutcome, CommandError> {
        let id = completed.binding.profile_id.clone();
        let request_id = completed.request_id.clone();
        if self.activity_requests.get(&id) != Some(&request_id) {
            completed.abort();
            return Err(CommandError::ProviderActivityCancelled);
        }
        let binding = &completed.binding;
        if let Err(error) = self.require_provider_binding(
            &binding.profile_id,
            &binding.home_reference,
            &binding.revision,
            CommandError::ProviderActivityCancelled,
        ) {
            completed.abort();
            self.note_activity_failure(&id, &request_id);
            return Err(error);
        }
        let observed_monotonic = completed.observed_monotonic();
        let mut outcome = completed.publish(|binding, probe, request, expected| {
            self.persist_activity(binding, probe, request, expected)
        });
        if let Err(error) = &outcome {
            self.note_monitoring_read_error(&id, &request_id, *error);
        }
        if let Ok(value) = &mut outcome {
            value.monitoring = self.monitoring_state().as_str();
        }
        match &outcome {
            Ok(value) => {
                self.activity_reviews.remove(&id);
                self.activity_sessions.insert(
                    id.clone(),
                    (value.collection_run_id.clone(), Some(observed_monotonic)),
                );
                if self
                    .storage
                    .as_ref()
                    .and_then(|s| s.current_quota(&id).ok())
                    .flatten()
                    .is_none()
                {
                    self.quota_sessions.remove(&id);
                }
                // A preview prepared before this durable write must be prepared again.
                self.restore_preview.clear();
            }
            Err(_) => self.note_activity_failure(&id, &request_id),
        }
        outcome
    }

    /// A failed collection changes status only; the last accepted metrics remain intact.
    pub fn note_activity_failure(&mut self, id: &str, request_id: &str) {
        if self.activity_requests.get(id).map(String::as_str) != Some(request_id) {
            return;
        }
        if let Some(current) = self
            .storage
            .as_ref()
            .and_then(|s| s.current_activity(id).ok())
            .flatten()
        {
            self.activity_sessions
                .insert(id.into(), (current.commit.collection_run_id, None));
        }
    }

    fn persist_activity(
        &mut self,
        binding: &provider::ProfileBinding,
        probe: &ActivityProbe,
        request_id: &str,
        association: &provider::ReadAssociation,
    ) -> Result<openusage_storage::ActivityCommit, CommandError> {
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
        let observed_at_utc = restore::timestamp(probe.observed_at)
            .ok_or(CommandError::ProviderActivitySaveFailed)?;
        let storage = self.storage.as_mut().ok_or(CommandError::WorkspaceLocked)?;
        let current = storage
            .profile_binding(&binding.profile_id)
            .map_err(save_error)?;
        if current.as_ref().map(|b| b.binding_id.as_str())
            != association.expected_binding.as_deref()
        {
            return Err(CommandError::ProviderActivityCancelled);
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
                        review_id: provider::new_id()?,
                        kind: ReadKind::Activity,
                        approval_id: binding.approval_id.clone(),
                        previous_binding: existing.clone(),
                        previous,
                        observed_at_utc: observed_at_utc.clone(),
                        fence: provider::ReadReviewFence {
                            binding: binding.clone(),
                            current: evidence,
                            created: Instant::now(),
                        },
                    },
                );
                return Err(CommandError::ProviderActivityAssociationChanged);
            }
        }
        // Replacing an existing association requires the separate, explicit native consent command.
        let create_binding = current.is_none() || association.review.is_some();
        let next_binding_id = if create_binding {
            provider::new_id()?
        } else {
            let existing = current
                .as_ref()
                .ok_or(CommandError::ProviderActivityCancelled)?;
            if existing.home_reference != binding.home_reference {
                return Err(CommandError::ProviderActivityAssociationChanged);
            }
            existing.binding_id.clone()
        };
        let new_association = create_binding.then(|| ProfileBindingInput {
            profile_id: binding.profile_id.clone(),
            expected_home_reference: binding.home_reference.clone(),
            expected_previous_binding_id: association.expected_binding.clone(),
            new_binding_id: next_binding_id.clone(),
            evidence: evidence.clone(),
            observed_at_utc: observed_at_utc.clone(),
        });
        let local_binding = openusage_storage::ProfileBinding {
            profile_id: binding.profile_id.clone(),
            home_reference: binding.home_reference.clone(),
            binding_id: next_binding_id,
        };
        let summary = &probe.activity.summary;
        storage
            .accept_activity_with_binding(
                ActivityReading {
                    run_id: request_id.into(),
                    binding: local_binding,
                    identity_evidence: evidence,
                    provider_version: probe.version.into(),
                    source_kind,
                    started_at_utc: restore::timestamp(probe.started_at)
                        .ok_or(CommandError::ProviderActivitySaveFailed)?,
                    observed_at_utc,
                    completed_at_utc: restore::timestamp(probe.completed_at)
                        .ok_or(CommandError::ProviderActivitySaveFailed)?,
                    summary: ActivitySummaryValues {
                        lifetime_tokens: exact(&summary.lifetime_tokens),
                        peak_daily_tokens: exact(&summary.peak_daily_tokens),
                        longest_running_turn_sec: exact(&summary.longest_running_turn_sec),
                        current_streak_days: exact(&summary.current_streak_days),
                        longest_streak_days: exact(&summary.longest_streak_days),
                    },
                    daily_usage: probe.activity.daily_usage_buckets.as_ref().map(|rows| {
                        rows.iter()
                            .map(|row| ActivityDayValue {
                                provider_date: row.start_date.clone(),
                                tokens: row.tokens.as_str().into(),
                            })
                            .collect()
                    }),
                },
                new_association,
            )
            .map_err(save_error)
    }
}

fn exact(value: &Option<DecimalCount>) -> Option<String> {
    value.as_ref().map(|count| count.as_str().into())
}
fn load_error(_: ActivityError) -> CommandError {
    CommandError::ProviderActivityLoadFailed
}
fn save_error(error: ActivityError) -> CommandError {
    match error {
        ActivityError::IdentityChanged => CommandError::ProviderActivityAssociationChanged,
        ActivityError::BindingChanged
        | ActivityError::ProfileUnavailable
        | ActivityError::AlreadyAccepted
        | ActivityError::StaleObservation => CommandError::ProviderActivityCancelled,
        _ => CommandError::ProviderActivitySaveFailed,
    }
}
fn freshness(
    session: Option<&(String, Option<Instant>)>,
    observed: &str,
    now: SystemTime,
) -> ActivityFreshness {
    freshness_for(session, observed, now, FRESH_FOR)
}

pub(super) fn freshness_for(
    session: Option<&(String, Option<Instant>)>,
    observed: &str,
    now: SystemTime,
    fresh_for: Duration,
) -> ActivityFreshness {
    let Some((_, observed_in_session)) = session else {
        return ActivityFreshness::Unknown;
    };
    let Some(observed_in_session) = observed_in_session else {
        return ActivityFreshness::Stale;
    };
    let Some(now_utc) = restore::timestamp(now) else {
        return ActivityFreshness::Unknown;
    };
    if observed > now_utc.as_str() {
        return ActivityFreshness::Unknown;
    }
    let Some(threshold) = now.checked_sub(fresh_for).and_then(restore::timestamp) else {
        return ActivityFreshness::Unknown;
    };
    if observed_in_session.elapsed() >= fresh_for || observed <= threshold.as_str() {
        ActivityFreshness::Stale
    } else {
        ActivityFreshness::Fresh
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn saved_freshness_requires_session_evidence_and_independent_row_time() {
        let now = SystemTime::now();
        let recent = restore::timestamp(now.checked_sub(Duration::from_secs(60)).unwrap()).unwrap();
        let old = restore::timestamp(now.checked_sub(FRESH_FOR + Duration::from_secs(1)).unwrap())
            .unwrap();
        let future = restore::timestamp(now.checked_add(Duration::from_secs(1)).unwrap()).unwrap();
        let session = ("run".into(), Some(Instant::now()));
        assert_eq!(freshness(None, &recent, now), ActivityFreshness::Unknown);
        assert_eq!(
            freshness(Some(&("run".into(), None)), &recent, now),
            ActivityFreshness::Stale
        );
        assert_eq!(
            freshness(Some(&session), &recent, now),
            ActivityFreshness::Fresh
        );
        assert_eq!(
            freshness(Some(&session), &old, now),
            ActivityFreshness::Stale
        );
        assert_eq!(
            freshness(Some(&session), &future, now),
            ActivityFreshness::Unknown
        );
        let expired = (
            "run".into(),
            Some(Instant::now().checked_sub(FRESH_FOR).unwrap()),
        );
        assert_eq!(
            freshness(Some(&expired), &recent, now),
            ActivityFreshness::Stale
        );
        let observed_now = restore::timestamp(now).unwrap();
        for fresh_for in [Duration::from_secs(600), FRESH_FOR] {
            let expired = (
                "run".into(),
                Some(Instant::now().checked_sub(fresh_for).unwrap()),
            );
            assert_eq!(
                freshness_for(Some(&expired), &observed_now, now, fresh_for),
                ActivityFreshness::Stale
            );
        }
    }

    fn saved_day(date: &str, tokens: &str) -> SavedActivityDay {
        SavedActivityDay {
            date: date.into(),
            tokens: tokens.into(),
            observed_at_utc: "2026-10-21T00:00:00Z".into(),
            freshness: ActivityFreshness::Unknown,
        }
    }

    #[test]
    fn daily_chart_scales_exact_large_counts_without_changing_saved_rows_or_filling_gaps() {
        let days = vec![
            saved_day("2026-10-01", "9223372036854775807"),
            saved_day("2026-10-04", "4611686018427387903"),
            saved_day("2026-10-10", "9007199254740993"),
            saved_day("2026-10-21", "0"),
        ];
        let original = serde_json::to_value(&days).unwrap();
        let chart = daily_chart(&days).unwrap();
        assert_eq!(chart.max_tokens.as_deref(), Some("9223372036854775807"));
        assert_eq!(
            chart
                .bars
                .iter()
                .map(|bar| (bar.date.as_str(), bar.height_basis_points))
                .collect::<Vec<_>>(),
            vec![
                ("2026-10-01", 10_000),
                ("2026-10-04", 4_999),
                ("2026-10-10", 9),
                ("2026-10-21", 0)
            ]
        );
        assert_eq!(serde_json::to_value(&days).unwrap(), original);
    }

    #[test]
    fn daily_chart_keeps_empty_and_observed_zero_distinct() {
        let empty = daily_chart(&[]).unwrap();
        assert!(empty.max_tokens.is_none());
        assert!(empty.bars.is_empty());
        let zero = daily_chart(&[saved_day("2026-10-21", "0")]).unwrap();
        assert_eq!(zero.max_tokens.as_deref(), Some("0"));
        assert_eq!(zero.bars.len(), 1);
        assert_eq!(zero.bars[0].date, "2026-10-21");
        assert_eq!(zero.bars[0].height_basis_points, 0);
    }

    #[test]
    fn daily_chart_refuses_invalid_saved_counts_instead_of_drawing_them_as_zero() {
        for tokens in ["", "-1", "01", "+1", "1.5", "9223372036854775808"] {
            assert!(matches!(
                daily_chart(&[saved_day("2026-10-21", tokens)]),
                Err(CommandError::ProviderActivityLoadFailed)
            ));
        }
    }
}
