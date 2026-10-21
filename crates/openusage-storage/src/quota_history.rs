//! Bounded local quota snapshots, without combining provisional associations.
use crate::activity::{read_error, require_uuid};
use crate::{ActivityError, QUOTA_SOURCE_METHOD, Storage, StoredQuota};
use rusqlite::{OptionalExtension, TransactionBehavior, params};

const PAGE_SIZE: usize = 20;

#[derive(Debug)]
pub struct QuotaHistoryPage {
    pub profile_id: String,
    pub entries: Vec<StoredQuota>,
    pub next_before_run_id: Option<String>,
}

impl Storage {
    /// One deferred WAL snapshot covers profile, cursor and every returned packet.
    /// Canonical millisecond UTC timestamps sort chronologically; UUID breaks ties.
    /// Prior associations and migrated snapshots do not need predecessor links.
    pub fn quota_history(
        &mut self,
        profile_id: &str,
        before_run_id: Option<&str>,
    ) -> Result<QuotaHistoryPage, ActivityError> {
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
                |row| row.get(0),
            )
            .map_err(read_error)?;
        if !exists {
            return Err(ActivityError::ProfileUnavailable);
        }
        let cursor = before_run_id
            .map(|run| {
                tx.query_row(
                    "SELECT r.observed_at_utc FROM collection_runs r JOIN quota_snapshots q ON q.id=r.id AND q.profile_id=r.profile_id AND q.binding_id=r.binding_id WHERE r.id=?1 AND r.profile_id=?2 AND r.source_method=?3 AND r.outcome='accepted'",
                    params![run, profile_id, QUOTA_SOURCE_METHOD],
                    |row| row.get::<_, String>(0),
                )
                .optional()
                .map_err(read_error)?
                .ok_or(ActivityError::InvalidInput)
            })
            .transpose()?;
        let select = "SELECT r.id FROM collection_runs r JOIN quota_snapshots q ON q.id=r.id AND q.profile_id=r.profile_id AND q.binding_id=r.binding_id WHERE r.profile_id=?1 AND r.source_method=?2 AND r.outcome='accepted'";
        let order = " ORDER BY r.observed_at_utc DESC,r.id DESC LIMIT 21";
        let runs = if let Some(time) = cursor {
            let mut statement = tx
                .prepare(&format!(
                    "{select} AND (r.observed_at_utc,r.id)<(?3,?4){order}"
                ))
                .map_err(read_error)?;
            statement
                .query_map(
                    params![profile_id, QUOTA_SOURCE_METHOD, time, before_run_id],
                    |row| row.get::<_, String>(0),
                )
                .map_err(read_error)?
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(read_error)?
        } else {
            let mut statement = tx
                .prepare(&format!("{select}{order}"))
                .map_err(read_error)?;
            statement
                .query_map(params![profile_id, QUOTA_SOURCE_METHOD], |row| {
                    row.get::<_, String>(0)
                })
                .map_err(read_error)?
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(read_error)?
        };
        let next_before_run_id = (runs.len() > PAGE_SIZE).then(|| runs[PAGE_SIZE - 1].clone());
        let entries = runs
            .iter()
            .take(PAGE_SIZE)
            .map(|run| crate::quota::load_snapshot_packet(&tx, "main", run))
            .collect::<Result<Vec<_>, _>>()?;
        tx.commit().map_err(read_error)?;
        Ok(QuotaHistoryPage {
            profile_id: profile_id.into(),
            entries,
            next_before_run_id,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ActivityReading, ActivitySourceKind, ActivitySummaryValues, ProfileBinding,
        ProfileBindingInput, ProfileIdentityEvidence, QuotaLimit, QuotaReading, QuotaWindow,
    };

    const KEY: [u8; 32] = [0x48; 32];
    const TIME: &str = "2026-10-21T12:00:00.001Z";
    const LATER: &str = "2026-10-21T12:00:00.010Z";
    fn id(n: u32) -> String {
        format!("{n:08x}-0000-4000-8000-000000000000")
    }
    fn fixture() -> (tempfile::TempDir, Storage) {
        let dir = crate::tests::private_directory();
        let mut storage = Storage::create(&dir.path().join("history.db"), &KEY).unwrap();
        storage
            .create_profile(&id(1), "Synthetic history", &id(2))
            .unwrap();
        (dir, storage)
    }
    fn reading(run: u32, profile: u32, home: u32, binding: u32, time: &str) -> QuotaReading {
        QuotaReading {
            run_id: id(run),
            binding: ProfileBinding {
                profile_id: id(profile),
                home_reference: id(home),
                binding_id: id(binding),
            },
            identity_evidence: ProfileIdentityEvidence {
                email: "synthetic-shared@example.invalid".into(),
                plan: None,
            },
            provider_version: "0.163.0".into(),
            source_kind: ActivitySourceKind::TestFixture,
            started_at_utc: time.into(),
            observed_at_utc: time.into(),
            completed_at_utc: time.into(),
            limits: vec![QuotaLimit {
                limit_id: "synthetic".into(),
                limit_name: None,
                primary: Some(QuotaWindow {
                    used_percent: 0,
                    window_duration_mins: None,
                    resets_at_unix: None,
                }),
                secondary: None,
                reached_type: None,
                monthly_credit_limit: None,
                spend_control_reached: None,
            }],
            available_reset_credits: None,
            reset_credit_details: None,
        }
    }
    fn accept(storage: &mut Storage, reading: QuotaReading) {
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
        storage.accept_quota(reading, association).unwrap();
    }

    #[test]
    fn quota_history_keyset_has_no_duplicates_or_skips_after_new_inserts_and_timestamp_ties() {
        let (_dir, mut storage) = fixture();
        for run in 100..125 {
            accept(&mut storage, reading(run, 1, 2, 3, TIME));
        }
        let before = crate::backup_validation::activity_records(&storage.connection);
        let first = storage.quota_history(&id(1), None).unwrap();
        assert_eq!(first.entries.len(), 20);
        assert_eq!(
            first
                .entries
                .iter()
                .map(|entry| entry.commit.collection_run_id.clone())
                .collect::<Vec<_>>(),
            (105..125).rev().map(id).collect::<Vec<_>>()
        );
        assert_eq!(first.next_before_run_id, Some(id(105)));
        assert_eq!(
            crate::backup_validation::activity_records(&storage.connection),
            before
        );
        accept(&mut storage, reading(200, 1, 2, 3, TIME));
        accept(&mut storage, reading(201, 1, 2, 3, LATER));
        let second = storage
            .quota_history(&id(1), first.next_before_run_id.as_deref())
            .unwrap();
        assert_eq!(
            second
                .entries
                .iter()
                .map(|entry| entry.commit.collection_run_id.clone())
                .collect::<Vec<_>>(),
            (100..105).rev().map(id).collect::<Vec<_>>()
        );
        assert_eq!(second.next_before_run_id, None);
        let refreshed = storage.quota_history(&id(1), None).unwrap();
        assert_eq!(refreshed.entries[0].commit.collection_run_id, id(201));
        assert_eq!(refreshed.entries[1].commit.collection_run_id, id(200));
        assert_eq!(
            storage
                .quota_history(&id(1), Some(&id(100)))
                .unwrap()
                .entries
                .len(),
            0
        );
    }

    #[test]
    fn quota_history_refuses_foreign_activity_and_unknown_cursors_and_distinguishes_empty_profile()
    {
        let (_dir, mut storage) = fixture();
        storage
            .create_profile(&id(10), "Synthetic second", &id(11))
            .unwrap();
        assert!(
            storage
                .quota_history(&id(10), None)
                .unwrap()
                .entries
                .is_empty()
        );
        assert!(matches!(
            storage.quota_history(&id(99), None),
            Err(ActivityError::ProfileUnavailable)
        ));
        accept(&mut storage, reading(100, 1, 2, 3, TIME));
        accept(&mut storage, reading(101, 10, 11, 12, TIME));
        let quota = reading(102, 1, 2, 3, TIME);
        storage
            .accept_activity(ActivityReading {
                run_id: quota.run_id,
                binding: quota.binding,
                identity_evidence: quota.identity_evidence,
                provider_version: quota.provider_version,
                source_kind: quota.source_kind,
                started_at_utc: TIME.into(),
                observed_at_utc: TIME.into(),
                completed_at_utc: TIME.into(),
                summary: ActivitySummaryValues::default(),
                daily_usage: None,
            })
            .unwrap();
        let before = crate::backup_validation::activity_records(&storage.connection);
        for cursor in [id(101), id(102), id(999), "invalid".into()] {
            assert!(matches!(
                storage.quota_history(&id(1), Some(&cursor)),
                Err(ActivityError::InvalidInput)
            ));
        }
        let page = storage.quota_history(&id(1), None).unwrap();
        assert_eq!(page.entries.len(), 1);
        assert_eq!(page.entries[0].commit.collection_run_id, id(100));
        assert_eq!(
            crate::backup_validation::activity_records(&storage.connection),
            before
        );
    }

    #[test]
    fn quota_history_keeps_prior_associations_null_windows_and_archived_readings_after_reopen() {
        let (dir, mut storage) = fixture();
        accept(&mut storage, reading(100, 1, 2, 3, TIME));
        let mut next = reading(101, 1, 2, 4, LATER);
        next.limits[0].primary = None;
        next.limits[0].secondary = Some(QuotaWindow {
            used_percent: 100,
            window_duration_mins: Some(9_007_199_254_740_991),
            resets_at_unix: Some(-62_135_596_800),
        });
        let mut missing = next.limits[0].clone();
        missing.limit_id = "no-windows".into();
        missing.secondary = None;
        next.limits.push(missing);
        accept(&mut storage, next);
        storage.set_profile_archived(&id(1), true).unwrap();
        drop(storage);
        let mut reopened = Storage::open(&dir.path().join("history.db"), &KEY).unwrap();
        let before = crate::backup_validation::activity_records(&reopened.connection);
        let page = reopened.quota_history(&id(1), None).unwrap();
        assert_eq!(page.entries.len(), 2);
        assert_eq!(page.entries[0].commit.binding_id, id(4));
        assert_eq!(page.entries[1].commit.binding_id, id(3));
        let limits = &page.entries[0].limits;
        assert!(limits.iter().any(|limit| limit.limit_id == "no-windows"
            && limit.primary.is_none()
            && limit.secondary.is_none()));
        let secondary = limits
            .iter()
            .find(|limit| limit.limit_id == "synthetic")
            .unwrap()
            .secondary
            .as_ref()
            .unwrap();
        assert_eq!(secondary.used_percent, 100);
        assert_eq!(secondary.window_duration_mins, Some(9_007_199_254_740_991));
        assert_eq!(secondary.resets_at_unix, Some(-62_135_596_800));
        let prior = page.entries[1].limits[0].primary.as_ref().unwrap();
        assert_eq!(prior.used_percent, 0);
        assert_eq!(prior.window_duration_mins, None);
        assert_eq!(prior.resets_at_unix, None);
        assert_eq!(
            crate::backup_validation::activity_records(&reopened.connection),
            before
        );
    }
}
