//! Accept supported readings from one supervised collection as one durable outcome.
use crate::activity::{
    associate_in_transaction, require, require_profile, validate_association, write_error,
};
use crate::{
    ActivityCommit, ActivityError, ActivityReading, ProfileBindingInput, QuotaCommit, QuotaReading,
    Storage, activity, quota,
};
use rusqlite::TransactionBehavior;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CollectionCommit {
    pub quota: Option<QuotaCommit>,
    pub activity: Option<ActivityCommit>,
}

impl Storage {
    /// The coordinator retains approval, request, generation and physical-home fences through COMMIT.
    /// A missing capability preserves its accepted state unless an approved reassociation clears it.
    pub fn accept_collection(
        &mut self,
        quota: Option<QuotaReading>,
        activity: Option<ActivityReading>,
        association: Option<ProfileBindingInput>,
    ) -> Result<CollectionCommit, ActivityError> {
        require(quota.is_some() || activity.is_some())?;
        if let Some(reading) = &quota {
            quota::validate_reading(reading)?;
        }
        if let Some(reading) = &activity {
            activity::validate_reading(reading)?;
        }
        if let (Some(q), Some(a)) = (&quota, &activity) {
            require(
                q.run_id != a.run_id
                    && q.binding == a.binding
                    && q.identity_evidence == a.identity_evidence
                    && q.provider_version == a.provider_version
                    && q.source_kind == a.source_kind
                    && q.started_at_utc == a.started_at_utc
                    && q.completed_at_utc == a.completed_at_utc,
            )?;
        }
        let (binding, evidence, observed) = if let Some(q) = &quota {
            (&q.binding, &q.identity_evidence, &q.observed_at_utc)
        } else {
            let a = activity.as_ref().ok_or(ActivityError::InvalidInput)?;
            (&a.binding, &a.identity_evidence, &a.observed_at_utc)
        };
        if let Some(input) = &association {
            validate_association(input)?;
            require(
                input.profile_id == binding.profile_id
                    && input.expected_home_reference == binding.home_reference
                    && input.new_binding_id == binding.binding_id
                    && input.evidence == *evidence
                    && input.observed_at_utc == *observed,
            )?;
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(write_error)?;
        require_profile(&tx, &binding.profile_id, &binding.home_reference)?;
        // Association clears current pointers, so both freshness fences must precede it.
        if let Some(q) = &quota {
            quota::require_new(&tx, q)?;
        }
        if let Some(a) = &activity {
            activity::require_new_reading(&tx, a)?;
        }
        if let Some(input) = &association {
            associate_in_transaction(&tx, input)?;
        }
        let commit = CollectionCommit {
            quota: quota
                .map(|reading| quota::accept_in_transaction(&tx, reading))
                .transpose()?,
            activity: activity
                .map(|reading| activity::accept_in_transaction(&tx, reading))
                .transpose()?,
        };
        tx.commit().map_err(write_error)?;
        Ok(commit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ActivityDayValue, ActivitySourceKind, ActivitySummaryValues, ProfileBinding,
        ProfileIdentityEvidence, QuotaCreditDetail, QuotaCreditStatus, QuotaLimit, QuotaResetState,
        QuotaWindow,
    };

    const KEY: [u8; 32] = [0x59; 32];
    fn id(n: u32) -> String {
        format!("{n:08x}-0000-4000-8000-000000000000")
    }
    fn time(second: u32) -> String {
        format!("2026-10-20T12:00:{second:02}.000Z")
    }
    fn readings(n: u32, second: u32, binding: u32) -> (QuotaReading, ActivityReading) {
        let binding = ProfileBinding {
            profile_id: id(1),
            home_reference: id(2),
            binding_id: id(binding),
        };
        let evidence = ProfileIdentityEvidence {
            email: "collection@example.invalid".into(),
            plan: Some("synthetic".into()),
        };
        let q = QuotaReading {
            run_id: id(n),
            binding: binding.clone(),
            identity_evidence: evidence.clone(),
            provider_version: "0.163.0".into(),
            source_kind: ActivitySourceKind::TestFixture,
            started_at_utc: time(0),
            observed_at_utc: time(second),
            completed_at_utc: time(second + 2),
            limits: vec![QuotaLimit {
                limit_id: "codex".into(),
                limit_name: None,
                primary: Some(QuotaWindow {
                    used_percent: 75,
                    window_duration_mins: Some(300),
                    resets_at_unix: Some(crate::quota::utc_to_unix(&time(2)).unwrap()),
                }),
                secondary: None,
                reached_type: None,
                monthly_credit_limit: Some("0000.01".into()),
                spend_control_reached: Some(false),
            }],
            available_reset_credits: Some("9223372036854775807".into()),
            reset_credit_details: Some(vec![QuotaCreditDetail {
                title: Some("Synthetic credit".into()),
                status: QuotaCreditStatus::Unknown,
                granted_at_unix: 0,
                expires_at_unix: None,
            }]),
        };
        let a = ActivityReading {
            run_id: id(n + 1),
            binding,
            identity_evidence: evidence,
            provider_version: q.provider_version.clone(),
            source_kind: q.source_kind,
            started_at_utc: q.started_at_utc.clone(),
            observed_at_utc: time(second + 1),
            completed_at_utc: q.completed_at_utc.clone(),
            summary: ActivitySummaryValues {
                lifetime_tokens: Some("9007199254740993".into()),
                peak_daily_tokens: Some("9223372036854775807".into()),
                current_streak_days: Some("0".into()),
                ..Default::default()
            },
            daily_usage: Some(vec![ActivityDayValue {
                provider_date: "2026-10-20".into(),
                tokens: "9007199254740993".into(),
            }]),
        };
        (q, a)
    }
    fn association(q: &QuotaReading, previous: Option<u32>) -> ProfileBindingInput {
        ProfileBindingInput {
            profile_id: q.binding.profile_id.clone(),
            expected_home_reference: q.binding.home_reference.clone(),
            expected_previous_binding_id: previous.map(id),
            new_binding_id: q.binding.binding_id.clone(),
            evidence: q.identity_evidence.clone(),
            observed_at_utc: q.observed_at_utc.clone(),
        }
    }
    fn setup() -> (tempfile::TempDir, Storage) {
        let dir = crate::tests::private_directory();
        let mut s = Storage::create(&dir.path().join("collection.db"), &KEY).unwrap();
        s.create_profile(&id(1), "Synthetic collection", &id(2))
            .unwrap();
        (dir, s)
    }
    fn records(s: &Storage) -> Vec<Vec<Vec<rusqlite::types::Value>>> {
        crate::backup_validation::activity_records(&s.connection)
    }
    fn seed(s: &mut Storage) {
        let (q, a) = readings(10, 0, 3);
        let input = association(&q, None);
        s.accept_collection(Some(q), Some(a), Some(input)).unwrap();
        let (mut q, a) = readings(12, 3, 3);
        q.limits[0].primary.as_mut().unwrap().used_percent = 10;
        q.limits[0].primary.as_mut().unwrap().resets_at_unix =
            Some(crate::quota::utc_to_unix(&time(10)).unwrap());
        s.accept_collection(Some(q), Some(a), None).unwrap();
        assert_eq!(
            s.current_quota(&id(1)).unwrap().unwrap().reset_assessments[0].state,
            QuotaResetState::Confirmed
        );
    }
    fn assert_history_retained(
        before: &[Vec<Vec<rusqlite::types::Value>>],
        after: &[Vec<Vec<rusqlite::types::Value>>],
    ) {
        for index in [0, 1, 2, 5, 7, 8, 9, 12] {
            for row in &before[index] {
                assert!(
                    after[index].contains(row),
                    "History changed in table {index}"
                );
            }
        }
    }

    #[test]
    fn collection_commits_exact_both_capabilities_and_reopens_encrypted_with_history() {
        let (dir, mut s) = setup();
        seed(&mut s);
        let before = records(&s);
        let (q, mut a) = readings(20, 6, 3);
        a.summary.lifetime_tokens = Some("9223372036854775807".into());
        a.daily_usage.as_mut().unwrap()[0].tokens = "9223372036854775807".into();
        let commit = s
            .accept_collection(Some(q.clone()), Some(a.clone()), None)
            .unwrap();
        let saved_q = s.current_quota(&id(1)).unwrap().unwrap();
        let saved_a = s.current_activity(&id(1)).unwrap().unwrap();
        assert_eq!(commit.quota, Some(saved_q.commit.clone()));
        assert_eq!(commit.activity, Some(saved_a.commit.clone()));
        assert_eq!(saved_q.limits, q.limits);
        assert_eq!(saved_q.reset_credit_details, q.reset_credit_details);
        assert_eq!(saved_q.available_reset_credits, q.available_reset_credits);
        assert_eq!(saved_a.summary, a.summary);
        assert_eq!(saved_q.commit.observed_at_utc, time(6));
        assert_eq!(saved_a.commit.observed_at_utc, time(7));
        assert_eq!(
            s.activity_days(&id(1), "2026-10-20", "2026-10-20", 10)
                .unwrap()
                .rows[0]
                .value
                .tokens,
            "9223372036854775807"
        );
        let after = records(&s);
        assert_history_retained(&before, &after);
        drop(s);
        let path = dir.path().join("collection.db");
        assert!(
            !std::fs::read(&path)
                .unwrap()
                .starts_with(b"SQLite format 3\0")
        );
        assert!(Storage::open(&path, &[0x11; 32]).is_err());
        let reopened = Storage::open(&path, &KEY).unwrap();
        assert_eq!(records(&reopened), after);
        assert_eq!(reopened.current_quota(&id(1)).unwrap().unwrap(), saved_q);
        assert_eq!(reopened.current_activity(&id(1)).unwrap().unwrap(), saved_a);
    }

    #[test]
    fn collection_second_writer_and_commit_faults_roll_back_binding_history_and_resets() {
        let (dir, mut s) = setup();
        seed(&mut s);
        let before = records(&s);
        for reassociate in [false, true] {
            let (q, a) = readings(20, 6, if reassociate { 4 } else { 3 });
            s.connection.execute_batch("CREATE TRIGGER collection_fault BEFORE INSERT ON activity_summaries BEGIN SELECT RAISE(ABORT,'synthetic second writer fault'); END;").unwrap();
            let input = reassociate.then(|| association(&q, Some(3)));
            assert_eq!(
                s.accept_collection(Some(q.clone()), Some(a.clone()), input),
                Err(ActivityError::WriteFailed)
            );
            assert_eq!(records(&s), before);
            s.connection
                .execute_batch("DROP TRIGGER collection_fault;")
                .unwrap();
            s.connection.commit_hook(Some(|| true)).unwrap();
            let input = reassociate.then(|| association(&q, Some(3)));
            assert_eq!(
                s.accept_collection(Some(q), Some(a), input),
                Err(ActivityError::WriteFailed)
            );
            s.connection.commit_hook(None::<fn() -> bool>).unwrap();
            assert_eq!(records(&s), before);
        }
        drop(s);
        let mut s = Storage::open(&dir.path().join("collection.db"), &KEY).unwrap();
        assert_eq!(records(&s), before);
        let (q, a) = readings(20, 6, 4);
        let input = association(&q, Some(3));
        s.accept_collection(Some(q), Some(a), Some(input)).unwrap();
        assert_eq!(
            s.profile_binding(&id(1)).unwrap().unwrap().binding_id,
            id(4)
        );
        assert_history_retained(&before, &records(&s));
    }

    #[test]
    fn collection_unsupported_capability_preserves_saved_state_until_explicit_reassociation() {
        for quota_only in [false, true] {
            let (_dir, mut s) = setup();
            let (q, a) = readings(10, 0, 3);
            let mut input = association(&q, None);
            if !quota_only {
                input.observed_at_utc = a.observed_at_utc.clone();
            }
            let first = s
                .accept_collection(
                    quota_only.then_some(q),
                    (!quota_only).then_some(a),
                    Some(input),
                )
                .unwrap();
            assert_eq!(first.quota.is_some(), quota_only);
            assert_eq!(first.activity.is_some(), !quota_only);
            let (q, a) = readings(12, 3, 3);
            s.accept_collection(Some(q), Some(a), None).unwrap();
            let prior_q = s.current_quota(&id(1)).unwrap();
            let prior_a = s.current_activity(&id(1)).unwrap();
            let prior_days = s
                .activity_days(&id(1), "2026-10-20", "2026-10-20", 10)
                .unwrap();
            let before = records(&s);
            let (q, a) = readings(20, 6, 3);
            s.accept_collection(quota_only.then_some(q), (!quota_only).then_some(a), None)
                .unwrap();
            if quota_only {
                assert_eq!(s.current_activity(&id(1)).unwrap(), prior_a);
                assert_eq!(
                    s.activity_days(&id(1), "2026-10-20", "2026-10-20", 10)
                        .unwrap(),
                    prior_days
                );
            } else {
                assert_eq!(s.current_quota(&id(1)).unwrap(), prior_q);
            }
            let (q, a) = readings(30, 9, 4);
            let mut input = association(&q, Some(3));
            if !quota_only {
                input.observed_at_utc = a.observed_at_utc.clone();
            }
            s.accept_collection(
                quota_only.then_some(q),
                (!quota_only).then_some(a),
                Some(input),
            )
            .unwrap();
            if quota_only {
                assert_eq!(s.current_activity(&id(1)).unwrap(), None);
                assert!(
                    s.activity_days(&id(1), "2026-10-20", "2026-10-20", 10)
                        .unwrap()
                        .rows
                        .is_empty()
                );
            } else {
                assert_eq!(s.current_quota(&id(1)).unwrap(), None);
            }
            assert_history_retained(&before, &records(&s));
        }
    }

    #[test]
    fn collection_refuses_stale_duplicate_invalid_or_mixed_session_readings_without_publication() {
        let (_dir, mut s) = setup();
        seed(&mut s);
        let before = records(&s);
        assert_eq!(
            s.accept_collection(None, None, None),
            Err(ActivityError::InvalidInput)
        );
        for field in 0..10 {
            let (q, mut a) = readings(20, 6, 3);
            match field {
                0 => a.binding.profile_id = id(7),
                1 => a.binding.home_reference = id(7),
                2 => a.binding.binding_id = id(7),
                3 => a.identity_evidence.email = "other@example.invalid".into(),
                4 => a.identity_evidence.plan = None,
                5 => a.provider_version = "0.164.0".into(),
                6 => a.source_kind = ActivitySourceKind::OfficialArtifact,
                7 => a.started_at_utc = time(1),
                8 => a.completed_at_utc = time(9),
                _ => a.run_id = q.run_id.clone(),
            }
            assert_eq!(
                s.accept_collection(Some(q), Some(a), None),
                Err(ActivityError::InvalidInput)
            );
            assert_eq!(records(&s), before);
        }
        for activity_side in [false, true] {
            let (mut q, mut a) = readings(20, 6, 4);
            if activity_side {
                a.run_id = id(11);
            } else {
                q.run_id = id(10);
            }
            let input = association(&q, Some(3));
            assert_eq!(
                s.accept_collection(Some(q), Some(a), Some(input)),
                Err(ActivityError::AlreadyAccepted)
            );
            let (mut q, mut a) = readings(20, 6, 4);
            if activity_side {
                a.observed_at_utc = time(0);
            } else {
                q.observed_at_utc = time(0);
            }
            let input = association(&q, Some(3));
            assert_eq!(
                s.accept_collection(Some(q), Some(a), Some(input)),
                Err(ActivityError::StaleObservation)
            );
            assert_eq!(records(&s), before);
        }
        let (q, mut a) = readings(20, 6, 4);
        let mut input = association(&q, Some(3));
        input.observed_at_utc = a.observed_at_utc.clone();
        assert_eq!(
            s.accept_collection(Some(q), Some(a.clone()), Some(input)),
            Err(ActivityError::InvalidInput)
        );
        let (q, _) = readings(20, 6, 4);
        a.summary.lifetime_tokens = Some("1.5".into());
        let input = association(&q, Some(3));
        assert_eq!(
            s.accept_collection(Some(q), Some(a), Some(input)),
            Err(ActivityError::InvalidInput)
        );
        let (mut q, a) = readings(20, 6, 4);
        q.limits[0].primary.as_mut().unwrap().used_percent = 101;
        let input = association(&q, Some(3));
        assert_eq!(
            s.accept_collection(Some(q), Some(a), Some(input)),
            Err(ActivityError::InvalidInput)
        );
        assert_eq!(records(&s), before);
    }
}
