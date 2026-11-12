//! Logical profile removal. Native callers own confirmation, process fences, and home protection.
use crate::{ProfileEnvironment, ProfileError, ProfileSummary, Storage, StorageRevision};
use rusqlite::{Connection, TransactionBehavior, params};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProfileHistoryChoice {
    /// Keep the profile and all its records, and free its active slot.
    Archive,
    /// Remove the profile, its history, overrides, and local subscription records.
    /// Shared accounts, independent usage, and durable notification journals remain.
    Delete,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProfileRemovalRequest {
    pub profile_id: String,
    /// A managed home UUID, never a path. Storage does not access the home.
    pub expected_home_reference: String,
    pub history: ProfileHistoryChoice,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProfileRemovalOutcome {
    Archived(ProfileSummary),
    Deleted { profile_id: String },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProfileRemovalError {
    InvalidId,
    InvalidHomeReference,
    NotFound,
    StaleTarget,
    ReadFailed,
    WriteFailed,
}

impl std::fmt::Display for ProfileRemovalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::InvalidId => "Use a canonical lowercase profile UUID.",
            Self::InvalidHomeReference => "Use a canonical lowercase managed home UUID.",
            Self::NotFound => "The profile does not exist.",
            Self::StaleTarget => {
                "The workspace or profile target changed. Check the profile again."
            }
            Self::ReadFailed => {
                "The profile removal checks failed. Previous records were preserved."
            }
            Self::WriteFailed => "The profile removal failed. Previous records were preserved.",
        })
    }
}
impl std::error::Error for ProfileRemovalError {}

impl Storage {
    /// Apply an explicit native profile choice against the revision from `restore_state`.
    /// The caller must retain confirmation, process, and physical-home fences through commit.
    /// This operation does not log out or cancel a remote subscription.
    /// It does not change files outside the database.
    /// Delete preserves accounts and durable notification journals, including in exports and backups.
    /// Previous backups remain restorable. This operation does not promise physical erasure.
    /// Repeated deletion returns `NotFound` with a fresh revision, or `StaleTarget` with an old one.
    pub fn remove_profile(
        &mut self,
        request: ProfileRemovalRequest,
        expected: &StorageRevision,
    ) -> Result<ProfileRemovalOutcome, ProfileRemovalError> {
        crate::profiles::validate_uuid(&request.profile_id)
            .map_err(|()| ProfileRemovalError::InvalidId)?;
        crate::profiles::validate_uuid(&request.expected_home_reference)
            .map_err(|()| ProfileRemovalError::InvalidHomeReference)?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| ProfileRemovalError::WriteFailed)?;
        if !expected
            .matches(&self.revision_scope, &tx)
            .map_err(|_| ProfileRemovalError::ReadFailed)?
        {
            return Err(ProfileRemovalError::StaleTarget);
        }
        let mut profile =
            crate::profiles::profile_by_id(&tx, &request.profile_id).map_err(|error| {
                if error == ProfileError::NotFound {
                    ProfileRemovalError::NotFound
                } else {
                    ProfileRemovalError::ReadFailed
                }
            })?;
        let home: String = tx
            .query_row(
                "SELECT approved_home_reference FROM profiles WHERE id=?1",
                [&request.profile_id],
                |row| row.get(0),
            )
            .map_err(|_| ProfileRemovalError::ReadFailed)?;
        if home != request.expected_home_reference
            || profile.environment != ProfileEnvironment::Native
        {
            return Err(ProfileRemovalError::StaleTarget);
        }
        let outcome = match request.history {
            ProfileHistoryChoice::Archive => {
                if !profile.archived {
                    tx.execute("UPDATE profiles SET archived=1,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?1", [&request.profile_id])
                        .map_err(|_| ProfileRemovalError::WriteFailed)?;
                    profile.archived = true;
                }
                ProfileRemovalOutcome::Archived(profile)
            }
            ProfileHistoryChoice::Delete => {
                delete_records(&tx, &request.profile_id)
                    .map_err(|_| ProfileRemovalError::WriteFailed)?;
                ProfileRemovalOutcome::Deleted {
                    profile_id: request.profile_id,
                }
            }
        };
        tx.commit().map_err(|_| ProfileRemovalError::WriteFailed)?;
        Ok(outcome)
    }
}

fn delete_records(connection: &Connection, profile_id: &str) -> rusqlite::Result<()> {
    // SQLCipher connections use MEMORY for temporary storage. The transaction owns this set.
    // Keep event IDs until constrained children are gone, without a history-sized Rust vector.
    connection.execute_batch(
        "CREATE TEMP TABLE profile_removal_event_ids(id TEXT PRIMARY KEY NOT NULL) STRICT",
    )?;
    connection.execute("INSERT INTO temp.profile_removal_event_ids SELECT id FROM alert_events WHERE profile_id=?1 UNION SELECT id FROM operational_alert_events WHERE profile_id=?1", [profile_id])?;
    for table in ["reset_notification_outbox", "threshold_alert_events"] {
        connection.execute(&format!("DELETE FROM {table} WHERE event_id IN (SELECT id FROM temp.profile_removal_event_ids)"), [])?;
    }
    for table in ["alert_events", "operational_alert_events"] {
        delete_owned(connection, table, profile_id)?;
    }
    connection.execute("DELETE FROM notification_event_ids WHERE id IN (SELECT id FROM temp.profile_removal_event_ids)", [])?;
    for table in [
        "operational_alert_outcomes",
        "quota_reset_assessments",
        "current_quota",
        "current_quota_readings",
        "credit_snapshots",
        "quota_limits",
    ] {
        delete_owned(connection, table, profile_id)?;
    }
    connection.execute(
        "UPDATE quota_snapshots SET previous_quota_run_id=NULL WHERE profile_id=?1",
        [profile_id],
    )?;
    delete_owned(connection, "quota_snapshots", profile_id)?;
    for table in [
        "profile_bindings",
        "activity_day_history",
        "current_activity_days",
        "current_activity",
        "activity_summaries",
        "collection_runs",
    ] {
        delete_owned(connection, table, profile_id)?;
    }
    connection.execute(
        "UPDATE profile_identity_events SET previous_binding_id=NULL WHERE profile_id=?1",
        [profile_id],
    )?;
    for table in [
        "profile_identity_events",
        "subscriptions",
        "reset_alert_overrides",
        "threshold_alert_overrides",
        "operational_alert_overrides",
    ] {
        delete_owned(connection, table, profile_id)?;
    }
    connection.execute("DELETE FROM profiles WHERE id=?1", [profile_id])?;
    connection.execute_batch("DROP TABLE temp.profile_removal_event_ids")?;
    Ok(())
}

fn delete_owned(connection: &Connection, table: &str, profile_id: &str) -> rusqlite::Result<usize> {
    // Table names come only from the fixed lists above. Every owner value is bound.
    connection.execute(
        &format!("DELETE FROM {table} WHERE profile_id=?1"),
        params![profile_id],
    )
}
