//! Durable profile activity; no credential or provider identity evidence crosses IPC.

use super::*;
use openusage_codex::{DecimalCount, session::ActivityProbe};
use openusage_storage::{
    ActivityBindingInput, ActivityDailyState, ActivityDayValue, ActivityError,
    ActivityIdentityEvidence, ActivityReading, ActivitySourceKind, ActivitySummaryValues,
};
use std::time::{Duration, Instant, SystemTime};

const FRESH_FOR: Duration = Duration::from_secs(30 * 60);

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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum ActivityFreshness {
    Unknown,
    Fresh,
    Stale,
}

impl Workspace {
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
        Ok(Some(SavedProfileActivity {
            profile_id: id.into(),
            binding_id: current.binding.binding_id,
            collection_run_id: current.commit.collection_run_id,
            provider_version: current.provider_version,
            source_kind: current.source_kind.as_str(),
            observed_at_utc,
            identity_confidence: "provisional",
            verified_identity_key: None,
            monitoring: "stopped",
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
            daily_usage_buckets: days
                .rows
                .into_iter()
                .map(|day| SavedActivityDay {
                    date: day.value.provider_date,
                    tokens: day.value.tokens,
                    freshness: freshness(session, &day.observed_at_utc, now),
                    observed_at_utc: day.observed_at_utc,
                })
                .collect(),
            daily_truncated: days.truncated,
        }))
    }

    pub fn finish_profile_activity_read(
        &mut self,
        completed: provider::CompletedActivity,
    ) -> Result<provider::ActivityOutcome, CommandError> {
        let id = completed.binding.profile_id.clone();
        let request_id = completed.request_id.clone();
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
        let outcome = completed.publish(|binding, probe, request, expected| {
            self.persist_activity(binding, probe, request, expected)
        });
        match &outcome {
            Ok(value) => {
                self.activity_sessions
                    .insert(id, (value.collection_run_id.clone(), Some(Instant::now())));
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
        expected_binding: Option<&str>,
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
        let evidence = ActivityIdentityEvidence {
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
            .activity_binding(&binding.profile_id)
            .map_err(save_error)?;
        if current.as_ref().map(|b| b.binding_id.as_str()) != expected_binding {
            return Err(CommandError::ProviderActivityCancelled);
        }
        let local_binding = match current {
            Some(current) => current,
            None => storage
                .associate_activity_binding(ActivityBindingInput {
                    profile_id: binding.profile_id.clone(),
                    expected_home_reference: binding.home_reference.clone(),
                    expected_previous_binding_id: None,
                    new_binding_id: provider::new_id()?,
                    evidence: evidence.clone(),
                    observed_at_utc: observed_at_utc.clone(),
                })
                .map_err(save_error)?,
        };
        let summary = &probe.activity.summary;
        storage
            .accept_activity(ActivityReading {
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
            })
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
        ActivityError::IdentityChanged => CommandError::ProviderAccountChanged,
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
    let Some(threshold) = now.checked_sub(FRESH_FOR).and_then(restore::timestamp) else {
        return ActivityFreshness::Unknown;
    };
    if observed_in_session.elapsed() >= FRESH_FOR || observed <= threshold.as_str() {
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
    }
}
