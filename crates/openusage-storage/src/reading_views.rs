//! Current values and audit context from one bounded read snapshot.

use crate::{
    ActivityDayValue, ActivityError, ActivitySourceKind, EvidenceMethod,
    MAX_ACTIVITY_ROWS_PER_READ, ReadingEvidenceAvailability, ReadingEvidenceContext,
    ReadingEvidenceTarget, Storage, StoredActivity, StoredQuota,
    activity::{read_error, require_uuid},
    reading_evidence::read_observation,
};
use rusqlite::{Connection, Transaction, TransactionBehavior};
use serde::Serialize;
use std::collections::{BTreeMap, btree_map::Entry};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct ReadingAudit {
    #[serde(flatten)]
    state: AuditState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "availability", rename_all = "snake_case")]
enum AuditState {
    Complete {
        #[serde(rename = "readingStatus")]
        reading_status: &'static str,
        #[serde(rename = "sourceLevel")]
        source_level: &'static str,
    },
    LegacyGap,
}

#[derive(Debug)]
pub struct AuditedActivityDay {
    pub day: ActivityDayValue,
    pub context: ReadingEvidenceContext,
    pub audit: ReadingAudit,
}

#[derive(Debug)]
pub struct CurrentActivityView {
    pub reading: StoredActivity,
    pub audit: ReadingAudit,
    pub days: Vec<AuditedActivityDay>,
    pub truncated: bool,
}

#[derive(Debug)]
pub struct CurrentQuotaView {
    pub reading: StoredQuota,
    pub audit: ReadingAudit,
}

fn observation(
    connection: &Connection,
    target: ReadingEvidenceTarget<'_>,
) -> Result<(ReadingEvidenceContext, ReadingAudit), ActivityError> {
    let saved = read_observation(connection, target)?.ok_or(ActivityError::ReadFailed)?;
    let state = match saved.availability {
        ReadingEvidenceAvailability::Complete(record) => AuditState::Complete {
            reading_status: record.reading_status,
            source_level: record.source_level,
        },
        ReadingEvidenceAvailability::LegacyGap => AuditState::LegacyGap,
    };
    Ok((saved.context, ReadingAudit { state }))
}

fn matches_context(
    context: &ReadingEvidenceContext,
    binding_id: &str,
    version: &str,
    source: ActivitySourceKind,
    observed: &str,
) -> Result<(), ActivityError> {
    if context.binding_id != binding_id
        || context.provider_version != version
        || context.source_kind != source
        || context.observed_at_utc != observed
    {
        return Err(ActivityError::ReadFailed);
    }
    Ok(())
}

impl Storage {
    pub fn current_quota_view(
        &self,
        profile_id: &str,
    ) -> Result<Option<CurrentQuotaView>, ActivityError> {
        require_uuid(profile_id)?;
        let tx = Transaction::new_unchecked(&self.connection, TransactionBehavior::Deferred)
            .map_err(read_error)?;
        let Some(reading) = self.current_quota(profile_id)? else {
            tx.commit().map_err(read_error)?;
            return Ok(None);
        };
        let (context, audit) = observation(
            &tx,
            ReadingEvidenceTarget {
                profile_id,
                method: EvidenceMethod::Quota,
                run_id: &reading.commit.collection_run_id,
            },
        )?;
        matches_context(
            &context,
            &reading.binding.binding_id,
            &reading.provider_version,
            reading.source_kind,
            &reading.commit.observed_at_utc,
        )?;
        tx.commit().map_err(read_error)?;
        Ok(Some(CurrentQuotaView { reading, audit }))
    }

    pub fn current_activity_view(
        &self,
        profile_id: &str,
    ) -> Result<Option<CurrentActivityView>, ActivityError> {
        require_uuid(profile_id)?;
        let tx = Transaction::new_unchecked(&self.connection, TransactionBehavior::Deferred)
            .map_err(read_error)?;
        let Some(reading) = self.current_activity(profile_id)? else {
            tx.commit().map_err(read_error)?;
            return Ok(None);
        };
        let (context, audit) = observation(
            &tx,
            ReadingEvidenceTarget {
                profile_id,
                method: EvidenceMethod::Activity,
                run_id: &reading.commit.collection_run_id,
            },
        )?;
        matches_context(
            &context,
            &reading.binding.binding_id,
            &reading.provider_version,
            reading.source_kind,
            &reading.commit.observed_at_utc,
        )?;
        let mut observations = BTreeMap::from([(context.run_id.clone(), (context, audit))]);
        let page = self.activity_days(
            profile_id,
            "0001-01-01",
            "9999-12-31",
            MAX_ACTIVITY_ROWS_PER_READ,
        )?;
        let mut days = Vec::with_capacity(page.rows.len());
        for day in page.rows {
            let (context, audit) = match observations.entry(day.collection_run_id) {
                Entry::Occupied(entry) => entry.into_mut(),
                Entry::Vacant(entry) => {
                    let saved = observation(
                        &tx,
                        ReadingEvidenceTarget {
                            profile_id,
                            method: EvidenceMethod::Activity,
                            run_id: entry.key(),
                        },
                    )?;
                    entry.insert(saved)
                }
            };
            if context.binding_id != day.binding_id
                || context.observed_at_utc != day.observed_at_utc
            {
                return Err(ActivityError::ReadFailed);
            }
            days.push(AuditedActivityDay {
                day: day.value,
                context: context.clone(),
                audit: *audit,
            });
        }
        tx.commit().map_err(read_error)?;
        Ok(Some(CurrentActivityView {
            reading,
            audit,
            days,
            truncated: page.truncated,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ActivityReading, ActivitySummaryValues, ProfileBinding, ProfileBindingInput,
        ProfileIdentityEvidence,
    };
    use rusqlite::hooks::{AuthAction, AuthContext, Authorization};
    use std::{sync::mpsc, time::Duration};

    const PROFILE: &str = "11111111-1111-4111-8111-111111111111";
    const HOME: &str = "22222222-2222-4222-8222-222222222222";
    const BINDING: &str = "33333333-3333-4333-8333-333333333333";
    const FIRST: &str = "44444444-4444-4444-8444-444444444444";
    const NEXT: &str = "55555555-5555-4555-8555-555555555555";
    const KEY: [u8; 32] = [73; 32];

    fn reading(run: &str, observed: &str, count: &str, date: &str) -> ActivityReading {
        crate::reading_evidence::fixture_activity(ActivityReading {
            run_id: run.into(),
            binding: ProfileBinding {
                profile_id: PROFILE.into(),
                home_reference: HOME.into(),
                binding_id: BINDING.into(),
            },
            identity_evidence: ProfileIdentityEvidence {
                email: "snapshot@example.invalid".into(),
                plan: None,
            },
            provider_version: "0.163.0".into(),
            source_kind: ActivitySourceKind::TestFixture,
            started_at_utc: observed.into(),
            observed_at_utc: observed.into(),
            completed_at_utc: observed.into(),
            summary: ActivitySummaryValues {
                lifetime_tokens: Some(count.into()),
                ..Default::default()
            },
            daily_usage: Some(vec![ActivityDayValue {
                provider_date: date.into(),
                tokens: count.into(),
            }]),
            evidence: None,
        })
    }

    #[test]
    fn current_activity_view_pins_summary_days_and_evidence_during_a_concurrent_commit() {
        let directory = crate::tests::private_directory();
        let path = directory.path().join("current-snapshot.db");
        let mut storage = Storage::create(&path, &KEY).unwrap();
        storage
            .create_profile(PROFILE, "Synthetic snapshot", HOME)
            .unwrap();
        let first = reading(FIRST, "2026-11-16T00:00:00.000Z", "10", "2026-11-15");
        storage
            .accept_activity_with_binding(
                first.clone(),
                Some(ProfileBindingInput {
                    profile_id: PROFILE.into(),
                    expected_home_reference: HOME.into(),
                    expected_previous_binding_id: None,
                    new_binding_id: BINDING.into(),
                    evidence: first.identity_evidence.clone(),
                    observed_at_utc: first.observed_at_utc.clone(),
                }),
            )
            .unwrap();
        let mut writer = Storage::open(&path, &KEY).unwrap();
        let (begin_tx, begin_rx) = mpsc::channel();
        let (done_tx, done_rx) = mpsc::channel();
        let writer = std::thread::spawn(move || {
            begin_rx.recv_timeout(Duration::from_secs(5)).unwrap();
            writer
                .accept_activity(reading(
                    NEXT,
                    "2026-11-16T00:00:01.000Z",
                    "20",
                    "2026-11-16",
                ))
                .unwrap();
            done_tx.send(()).unwrap();
        });
        let mut triggered = false;
        storage
            .connection
            .authorizer(Some(move |context: AuthContext<'_>| {
                if !triggered
                    && matches!(
                        context.action,
                        AuthAction::Read {
                            table_name: "reading_evidence",
                            ..
                        }
                    )
                {
                    triggered = true;
                    begin_tx.send(()).unwrap();
                    done_rx.recv_timeout(Duration::from_secs(5)).unwrap();
                }
                Authorization::Allow
            }))
            .unwrap();
        let view = storage.current_activity_view(PROFILE).unwrap().unwrap();
        storage
            .connection
            .authorizer(None::<fn(AuthContext<'_>) -> Authorization>)
            .unwrap();
        writer.join().unwrap();
        assert_eq!(view.reading.commit.collection_run_id, FIRST);
        assert_eq!(view.reading.summary.lifetime_tokens.as_deref(), Some("10"));
        assert_eq!(view.days.len(), 1);
        assert_eq!(view.days[0].day.tokens, "10");
        assert_eq!(view.days[0].context.run_id, FIRST);
        let next = storage.current_activity_view(PROFILE).unwrap().unwrap();
        assert_eq!(next.reading.commit.collection_run_id, NEXT);
        assert_eq!(next.reading.summary.lifetime_tokens.as_deref(), Some("20"));
        assert_eq!(next.days.len(), 2);
        let readonly = Storage::open_read_only(&path, &KEY).unwrap();
        let next = readonly.current_activity_view(PROFILE).unwrap().unwrap();
        assert_eq!(next.reading.commit.collection_run_id, NEXT);
        assert_eq!(next.days.len(), 2);
    }
}
