//! Saved summary observations are snapshots, not token events or usage deltas.
use crate::activity::{
    activity_decimal, activity_utc_timestamp, read_error, require_uuid, safe_text,
};
use crate::{
    ACTIVITY_SOURCE_METHOD, ACTIVITY_SOURCE_SCOPE, ActivityCommit, ActivityDailyState,
    ActivityError, ActivitySourceKind, ActivitySummaryValues, ProfileBinding, Storage,
    StoredActivity,
};
use rusqlite::{OptionalExtension, TransactionBehavior, params};
const PAGE_SIZE: usize = 20;
#[derive(Debug)]
pub struct ActivityHistoryPage {
    pub profile_id: String,
    pub entries: Vec<StoredActivity>,
    pub next_before_run_id: Option<String>,
}
impl Storage {
    /// Profile, cursor, and all summary rows share one deferred WAL snapshot.
    /// Original associations remain available after reassociation or archiving.
    pub fn activity_history(
        &mut self,
        profile_id: &str,
        before_run_id: Option<&str>,
    ) -> Result<ActivityHistoryPage, ActivityError> {
        require_uuid(profile_id)?;
        if let Some(cursor) = before_run_id {
            require_uuid(cursor)?;
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(read_error)?;
        let exists: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM profiles WHERE id=?1)",
                [profile_id],
                |r| r.get(0),
            )
            .map_err(read_error)?;
        if !exists {
            return Err(ActivityError::ProfileUnavailable);
        }
        let cursor=before_run_id.map(|run| {
            tx.query_row("SELECT r.observed_at_utc FROM collection_runs r JOIN activity_summaries s ON s.collection_run_id=r.id AND s.id=r.id AND s.profile_id=r.profile_id AND s.binding_id=r.binding_id WHERE r.id=?1 AND r.profile_id=?2 AND r.source_method=?3 AND r.outcome='accepted'",params![run,profile_id,ACTIVITY_SOURCE_METHOD],|r|r.get::<_,String>(0)).optional().map_err(read_error)?.ok_or(ActivityError::InvalidInput)
        }).transpose()?;
        if cursor
            .as_deref()
            .is_some_and(|time| !activity_utc_timestamp(time))
        {
            return Err(ActivityError::ReadFailed);
        }
        let select = "SELECT r.id,r.profile_id,r.binding_id,e.home_reference,r.started_at_utc,r.observed_at_utc,r.completed_at_utc,r.provider_version,r.source_kind,s.id,s.lifetime_tokens,s.peak_daily_tokens,s.longest_running_turn_sec,s.current_streak_days,s.longest_streak_days,s.daily_state,r.source_scope,r.raw_scope,r.normalized_unit,r.coverage,r.identity_confidence,r.verified_identity_key FROM collection_runs r JOIN activity_summaries s ON s.collection_run_id=r.id AND s.id=r.id AND s.profile_id=r.profile_id AND s.binding_id=r.binding_id JOIN profile_identity_events e ON e.id=r.binding_id AND e.profile_id=r.profile_id WHERE r.profile_id=?1 AND r.source_method=?2 AND r.outcome='accepted'";
        let filter = if cursor.is_some() {
            " AND (r.observed_at_utc,r.id)<(?3,?4)"
        } else {
            ""
        };
        let mut query = tx
            .prepare(&format!(
                "{select}{filter} ORDER BY r.observed_at_utc DESC,r.id DESC LIMIT 21"
            ))
            .map_err(read_error)?;
        let mut rows = if let Some(time) = &cursor {
            query.query(params![
                profile_id,
                ACTIVITY_SOURCE_METHOD,
                time,
                before_run_id
            ])
        } else {
            query.query(params![profile_id, ACTIVITY_SOURCE_METHOD])
        }
        .map_err(read_error)?;
        let mut entries = Vec::with_capacity(PAGE_SIZE + 1);
        while let Some(row) = rows.next().map_err(read_error)? {
            entries.push(read_snapshot(row)?);
        }
        drop(rows);
        drop(query);
        let next_before_run_id = (entries.len() > PAGE_SIZE)
            .then(|| entries[PAGE_SIZE - 1].commit.collection_run_id.clone());
        entries.truncate(PAGE_SIZE);
        tx.commit().map_err(read_error)?;
        Ok(ActivityHistoryPage {
            profile_id: profile_id.into(),
            entries,
            next_before_run_id,
        })
    }
}
fn read_snapshot(row: &rusqlite::Row<'_>) -> Result<StoredActivity, ActivityError> {
    let run: String = row.get(0).map_err(read_error)?;
    let profile: String = row.get(1).map_err(read_error)?;
    let binding: String = row.get(2).map_err(read_error)?;
    let home: String = row.get(3).map_err(read_error)?;
    let start: String = row.get(4).map_err(read_error)?;
    let observed: String = row.get(5).map_err(read_error)?;
    let completed: String = row.get(6).map_err(read_error)?;
    let provider_version: String = row.get(7).map_err(read_error)?;
    let source_kind = match row.get::<_, String>(8).map_err(read_error)?.as_str() {
        "official_artifact" => ActivitySourceKind::OfficialArtifact,
        "test_fixture" => ActivitySourceKind::TestFixture,
        _ => return Err(ActivityError::ReadFailed),
    };
    let snapshot: String = row.get(9).map_err(read_error)?;
    let summary = ActivitySummaryValues {
        lifetime_tokens: row.get(10).map_err(read_error)?,
        peak_daily_tokens: row.get(11).map_err(read_error)?,
        longest_running_turn_sec: row.get(12).map_err(read_error)?,
        current_streak_days: row.get(13).map_err(read_error)?,
        longest_streak_days: row.get(14).map_err(read_error)?,
    };
    let daily_state = match row.get::<_, String>(15).map_err(read_error)?.as_str() {
        "unavailable" => ActivityDailyState::Unavailable,
        "empty" => ActivityDailyState::Empty,
        "returned" => ActivityDailyState::Returned,
        _ => return Err(ActivityError::ReadFailed),
    };
    let scope: String = row.get(16).map_err(read_error)?;
    let raw_scope: Option<String> = row.get(17).map_err(read_error)?;
    let unit: String = row.get(18).map_err(read_error)?;
    let coverage: String = row.get(19).map_err(read_error)?;
    let confidence: String = row.get(20).map_err(read_error)?;
    let verified_identity: Option<String> = row.get(21).map_err(read_error)?;
    if [&run, &profile, &binding, &home, &snapshot]
        .iter()
        .any(|value| require_uuid(value).is_err())
        || snapshot != run
        || !activity_utc_timestamp(&start)
        || !activity_utc_timestamp(&observed)
        || !activity_utc_timestamp(&completed)
        || start > observed
        || observed > completed
        || !safe_text(&provider_version, 64)
        || scope != ACTIVITY_SOURCE_SCOPE
        || raw_scope.is_some()
        || unit != "tokens"
        || coverage != "unknown"
        || confidence != "provisional"
        || verified_identity.is_some()
        || [
            &summary.lifetime_tokens,
            &summary.peak_daily_tokens,
            &summary.longest_running_turn_sec,
            &summary.current_streak_days,
            &summary.longest_streak_days,
        ]
        .into_iter()
        .flatten()
        .any(|count| !activity_decimal(count))
    {
        return Err(ActivityError::ReadFailed);
    }
    Ok(StoredActivity {
        binding: ProfileBinding {
            profile_id: profile.clone(),
            home_reference: home,
            binding_id: binding.clone(),
        },
        commit: ActivityCommit {
            profile_id: profile,
            binding_id: binding,
            collection_run_id: run,
            snapshot_id: snapshot,
            observed_at_utc: observed,
        },
        source_kind,
        provider_version,
        summary,
        daily_state,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ActivityDayValue, ActivityReading, ProfileBindingInput, ProfileIdentityEvidence,
        QuotaReading,
    };
    const KEY: [u8; 32] = [0x49; 32];
    const TIME: &str = "2026-10-21T12:00:00.001Z";
    const LATER: &str = "2026-10-21T12:00:00.010Z";
    fn id(n: u32) -> String {
        format!("{n:08x}-0000-4000-8000-000000000000")
    }
    fn setup() -> (tempfile::TempDir, Storage) {
        let dir = crate::tests::private_directory();
        let mut storage = Storage::create(&dir.path().join("activity-history.db"), &KEY).unwrap();
        storage
            .create_profile(&id(1), "Synthetic activity history", &id(2))
            .unwrap();
        (dir, storage)
    }
    fn reading(run: u32, profile: u32, home: u32, binding: u32, time: &str) -> ActivityReading {
        ActivityReading {
            run_id: id(run),
            binding: ProfileBinding {
                profile_id: id(profile),
                home_reference: id(home),
                binding_id: id(binding),
            },
            identity_evidence: ProfileIdentityEvidence {
                email: "history-private@example.invalid".into(),
                plan: Some("synthetic".into()),
            },
            provider_version: "0.163.0".into(),
            source_kind: ActivitySourceKind::TestFixture,
            started_at_utc: time.into(),
            observed_at_utc: time.into(),
            completed_at_utc: time.into(),
            summary: ActivitySummaryValues::default(),
            daily_usage: None,
        }
    }
    fn accept(storage: &mut Storage, reading: ActivityReading) {
        let previous = storage
            .profile_binding(&reading.binding.profile_id)
            .unwrap();
        let association = previous
            .as_ref()
            .is_none_or(|binding| binding.binding_id != reading.binding.binding_id)
            .then(|| ProfileBindingInput {
                profile_id: reading.binding.profile_id.clone(),
                expected_home_reference: reading.binding.home_reference.clone(),
                expected_previous_binding_id: previous.map(|binding| binding.binding_id),
                new_binding_id: reading.binding.binding_id.clone(),
                evidence: reading.identity_evidence.clone(),
                observed_at_utc: reading.observed_at_utc.clone(),
            });
        storage
            .accept_activity_with_binding(reading, association)
            .unwrap();
    }
    fn runs(page: &ActivityHistoryPage) -> Vec<String> {
        page.entries
            .iter()
            .map(|entry| entry.commit.collection_run_id.clone())
            .collect()
    }
    #[test]
    fn activity_history_keyset_handles_timestamp_ties_and_new_insertions_without_skips() {
        let (_dir, mut storage) = setup();
        for run in 100..145 {
            accept(&mut storage, reading(run, 1, 2, 3, TIME));
        }
        let before = crate::backup_validation::activity_records(&storage.connection);
        let first = storage.activity_history(&id(1), None).unwrap();
        assert_eq!(first.profile_id, id(1));
        assert_eq!(first.entries.len(), 20);
        assert_eq!(runs(&first), (125..145).rev().map(id).collect::<Vec<_>>());
        assert_eq!(first.next_before_run_id, Some(id(125)));
        assert_eq!(
            crate::backup_validation::activity_records(&storage.connection),
            before
        );
        accept(&mut storage, reading(200, 1, 2, 3, TIME));
        accept(&mut storage, reading(201, 1, 2, 3, LATER));
        let second = storage
            .activity_history(&id(1), first.next_before_run_id.as_deref())
            .unwrap();
        assert_eq!(runs(&second), (105..125).rev().map(id).collect::<Vec<_>>());
        assert_eq!(second.next_before_run_id, Some(id(105)));
        let third = storage
            .activity_history(&id(1), second.next_before_run_id.as_deref())
            .unwrap();
        assert_eq!(runs(&third), (100..105).rev().map(id).collect::<Vec<_>>());
        assert_eq!(third.next_before_run_id, None);
        let refreshed = storage.activity_history(&id(1), None).unwrap();
        assert_eq!(&runs(&refreshed)[..2], &[id(201), id(200)]);
        assert!(
            storage
                .activity_history(&id(1), Some(&id(100)))
                .unwrap()
                .entries
                .is_empty()
        );
    }
    #[test]
    fn activity_history_refuses_foreign_quota_unknown_and_malformed_cursors() {
        let (_dir, mut storage) = setup();
        storage
            .create_profile(&id(10), "Independent history", &id(11))
            .unwrap();
        assert!(
            storage
                .activity_history(&id(10), None)
                .unwrap()
                .entries
                .is_empty()
        );
        assert!(matches!(
            storage.activity_history(&id(99), None),
            Err(ActivityError::ProfileUnavailable)
        ));
        assert!(matches!(
            storage.activity_history("invalid", None),
            Err(ActivityError::InvalidInput)
        ));
        accept(&mut storage, reading(100, 1, 2, 3, TIME));
        accept(&mut storage, reading(101, 10, 11, 12, TIME));
        let activity = reading(102, 1, 2, 3, TIME);
        storage
            .accept_quota(
                QuotaReading {
                    run_id: activity.run_id,
                    binding: activity.binding,
                    identity_evidence: activity.identity_evidence,
                    provider_version: activity.provider_version,
                    source_kind: activity.source_kind,
                    started_at_utc: TIME.into(),
                    observed_at_utc: TIME.into(),
                    completed_at_utc: TIME.into(),
                    limits: vec![],
                    available_reset_credits: None,
                    reset_credit_details: None,
                },
                None,
            )
            .unwrap();
        let before = crate::backup_validation::activity_records(&storage.connection);
        for cursor in [id(101), id(102), id(999), "invalid".into()] {
            assert!(matches!(
                storage.activity_history(&id(1), Some(&cursor)),
                Err(ActivityError::InvalidInput)
            ));
        }
        let page = storage.activity_history(&id(1), None).unwrap();
        assert_eq!(runs(&page), vec![id(100)]);
        assert_eq!(
            crate::backup_validation::activity_records(&storage.connection),
            before
        );
    }
    #[test]
    fn activity_history_preserves_missing_zero_exact_counts_and_original_associations_after_restart()
     {
        let (dir, mut storage) = setup();
        accept(&mut storage, reading(100, 1, 2, 3, TIME));
        let mut zero = reading(101, 1, 2, 3, TIME);
        zero.summary = ActivitySummaryValues {
            lifetime_tokens: Some("0".into()),
            peak_daily_tokens: Some("0".into()),
            longest_running_turn_sec: Some("0".into()),
            current_streak_days: Some("0".into()),
            longest_streak_days: Some("0".into()),
        };
        zero.daily_usage = Some(vec![]);
        accept(&mut storage, zero);
        let mut latest = reading(102, 1, 2, 4, LATER);
        latest.source_kind = ActivitySourceKind::OfficialArtifact;
        latest.provider_version = "0.163.1".into();
        latest.summary = ActivitySummaryValues {
            lifetime_tokens: Some("9223372036854775807".into()),
            peak_daily_tokens: Some("9007199254740993".into()),
            longest_running_turn_sec: Some("123".into()),
            current_streak_days: None,
            longest_streak_days: Some("2".into()),
        };
        latest.daily_usage = Some(vec![ActivityDayValue {
            provider_date: "2026-10-21".into(),
            tokens: "0".into(),
        }]);
        accept(&mut storage, latest);
        storage.set_profile_archived(&id(1), true).unwrap();
        drop(storage);
        let mut storage = Storage::open(&dir.path().join("activity-history.db"), &KEY).unwrap();
        let before = crate::backup_validation::activity_records(&storage.connection);
        let page = storage.activity_history(&id(1), None).unwrap();
        assert_eq!(runs(&page), vec![id(102), id(101), id(100)]);
        let latest = &page.entries[0];
        assert_eq!(latest.binding.binding_id, id(4));
        assert_eq!(latest.binding.home_reference, id(2));
        assert_eq!(latest.commit.snapshot_id, id(102));
        assert_eq!(latest.commit.observed_at_utc, LATER);
        assert_eq!(latest.source_kind, ActivitySourceKind::OfficialArtifact);
        assert_eq!(latest.provider_version, "0.163.1");
        assert_eq!(
            latest.summary.lifetime_tokens.as_deref(),
            Some("9223372036854775807")
        );
        assert_eq!(
            latest.summary.peak_daily_tokens.as_deref(),
            Some("9007199254740993")
        );
        assert_eq!(latest.summary.current_streak_days, None);
        assert_eq!(latest.daily_state, ActivityDailyState::Returned);
        let zero = &page.entries[1];
        assert_eq!(zero.binding.binding_id, id(3));
        assert_eq!(zero.summary.lifetime_tokens.as_deref(), Some("0"));
        assert_eq!(zero.summary.current_streak_days.as_deref(), Some("0"));
        assert_eq!(zero.daily_state, ActivityDailyState::Empty);
        assert_eq!(page.entries[2].summary, ActivitySummaryValues::default());
        assert_eq!(page.entries[2].daily_state, ActivityDailyState::Unavailable);
        assert!(!format!("{page:?}").contains("history-private@example.invalid"));
        assert_eq!(
            crate::backup_validation::activity_records(&storage.connection),
            before
        );
    }
    #[test]
    fn activity_history_uses_one_wal_snapshot_while_another_connection_commits() {
        let (dir, mut storage) = setup();
        for run in 100..120 {
            accept(&mut storage, reading(run, 1, 2, 3, TIME));
        }
        let mut writer = Storage::open(&dir.path().join("activity-history.db"), &KEY).unwrap();
        let (begin_tx, begin_rx) = std::sync::mpsc::channel();
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let writer = std::thread::spawn(move || {
            begin_rx
                .recv_timeout(std::time::Duration::from_secs(5))
                .unwrap();
            accept(&mut writer, reading(200, 1, 2, 3, LATER));
            done_tx.send(()).unwrap();
        });
        use rusqlite::hooks::{AuthAction, AuthContext, Authorization};
        let mut triggered = false;
        storage
            .connection
            .authorizer(Some(move |context: AuthContext<'_>| {
                if !triggered
                    && matches!(
                        context.action,
                        AuthAction::Read {
                            table_name: "activity_summaries",
                            ..
                        }
                    )
                {
                    triggered = true;
                    begin_tx.send(()).unwrap();
                    done_rx
                        .recv_timeout(std::time::Duration::from_secs(5))
                        .unwrap();
                }
                Authorization::Allow
            }))
            .unwrap();
        let page = storage.activity_history(&id(1), None).unwrap();
        storage
            .connection
            .authorizer(None::<fn(AuthContext<'_>) -> Authorization>)
            .unwrap();
        writer.join().unwrap();
        assert_eq!(runs(&page), (100..120).rev().map(id).collect::<Vec<_>>());
        assert_eq!(page.next_before_run_id, None);
        let refreshed = storage.activity_history(&id(1), None).unwrap();
        assert_eq!(refreshed.entries[0].commit.collection_run_id, id(200));
        assert!(refreshed.next_before_run_id.is_some());
    }
    #[test]
    fn activity_history_runs_on_query_only_connection_and_preserves_database_and_wal_bytes() {
        let (dir, mut storage) = setup();
        for run in 100..122 {
            accept(&mut storage, reading(run, 1, 2, 3, TIME));
        }
        let path = dir.path().join("activity-history.db");
        let wal = dir.path().join("activity-history.db-wal");
        storage
            .connection
            .pragma_update(None, "query_only", true)
            .unwrap();
        let before = (std::fs::read(&path).unwrap(), std::fs::read(&wal).unwrap());
        let first = storage.activity_history(&id(1), None).unwrap();
        let second = storage
            .activity_history(&id(1), first.next_before_run_id.as_deref())
            .unwrap();
        assert_eq!(first.entries.len(), 20);
        assert_eq!(second.entries.len(), 2);
        assert!(matches!(
            storage.activity_history(&id(1), Some(&id(999))),
            Err(ActivityError::InvalidInput)
        ));
        assert_eq!(
            (std::fs::read(&path).unwrap(), std::fs::read(&wal).unwrap()),
            before
        );
        assert!(
            storage
                .connection
                .pragma_query_value(None, "query_only", |r| r.get::<_, bool>(0))
                .unwrap()
        );
    }
    #[test]
    fn activity_history_refuses_malformed_stored_counts_source_state_and_provenance() {
        let (_dir, mut storage) = setup();
        let mut initial = reading(100, 1, 2, 3, TIME);
        initial.summary.lifetime_tokens = Some("0".into());
        accept(&mut storage, initial);
        storage
            .connection
            .execute_batch("PRAGMA ignore_check_constraints=ON;")
            .unwrap();
        for (corrupt, restore) in [
            (
                "UPDATE activity_summaries SET lifetime_tokens='00'",
                "UPDATE activity_summaries SET lifetime_tokens='0'",
            ),
            (
                "UPDATE activity_summaries SET lifetime_tokens='9223372036854775808'",
                "UPDATE activity_summaries SET lifetime_tokens='0'",
            ),
            (
                "UPDATE activity_summaries SET daily_state='invented'",
                "UPDATE activity_summaries SET daily_state='unavailable'",
            ),
            (
                "UPDATE collection_runs SET source_kind='invented'",
                "UPDATE collection_runs SET source_kind='test_fixture'",
            ),
            (
                "UPDATE collection_runs SET provider_version=''",
                "UPDATE collection_runs SET provider_version='0.163.0'",
            ),
            (
                "UPDATE collection_runs SET source_scope='invented'",
                "UPDATE collection_runs SET source_scope='account_activity_with_unknown_application_scope'",
            ),
            (
                "UPDATE collection_runs SET completed_at_utc='2026-10-21T12:00:00.000Z'",
                "UPDATE collection_runs SET completed_at_utc='2026-10-21T12:00:00.001Z'",
            ),
        ] {
            storage.connection.execute_batch(corrupt).unwrap();
            assert!(
                matches!(
                    storage.activity_history(&id(1), None),
                    Err(ActivityError::ReadFailed)
                ),
                "{corrupt}"
            );
            storage.connection.execute_batch(restore).unwrap();
            assert_eq!(
                storage
                    .activity_history(&id(1), None)
                    .unwrap()
                    .entries
                    .len(),
                1
            );
        }
        storage
            .connection
            .execute_batch("PRAGMA ignore_check_constraints=OFF;")
            .unwrap();
    }
}
