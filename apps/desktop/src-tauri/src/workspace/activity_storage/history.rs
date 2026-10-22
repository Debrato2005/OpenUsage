//! Saved summary observations retain source metadata without live freshness or invented events.
use super::*;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProfileActivityHistory {
    profile_id: String,
    entries: Vec<ActivityHistoryEntry>,
    next_before_run_id: Option<String>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ActivityHistoryEntry {
    profile_id: String,
    binding_id: String,
    collection_run_id: String,
    observed_at_utc: String,
    source_kind: &'static str,
    provider_version: String,
    source_scope: &'static str,
    coverage: &'static str,
    summary: SavedActivitySummary,
    daily_buckets_state: &'static str,
}
impl Workspace {
    pub fn activity_history_export(
        &mut self,
        profile_id: &str,
        include_identifiers: bool,
    ) -> Result<openusage_storage::ReadingsExport, CommandError> {
        validate_profile_id(profile_id)?;
        self.storage
            .as_mut()
            .ok_or(CommandError::WorkspaceLocked)?
            .activity_history_export(profile_id, include_identifiers)
            .map_err(export_error)
    }

    pub fn load_profile_activity_history(
        &mut self,
        profile_id: &str,
        before_run_id: Option<&str>,
    ) -> Result<ProfileActivityHistory, CommandError> {
        validate_profile_id(profile_id)?;
        if let Some(cursor) = before_run_id {
            restore::validate_preview_id(cursor)?;
        }
        let page = self
            .storage
            .as_mut()
            .ok_or(CommandError::WorkspaceLocked)?
            .activity_history(profile_id, before_run_id)
            .map_err(|error| match error {
                ActivityError::InvalidInput => CommandError::InvalidArguments,
                ActivityError::ProfileUnavailable => CommandError::ProfileNotFound,
                _ => CommandError::ProviderActivityLoadFailed,
            })?;
        Ok(ProfileActivityHistory {
            profile_id: page.profile_id,
            next_before_run_id: page.next_before_run_id,
            entries: page
                .entries
                .into_iter()
                .map(|entry| ActivityHistoryEntry {
                    profile_id: entry.commit.profile_id,
                    binding_id: entry.commit.binding_id,
                    collection_run_id: entry.commit.collection_run_id,
                    observed_at_utc: entry.commit.observed_at_utc,
                    source_kind: entry.source_kind.as_str(),
                    provider_version: entry.provider_version,
                    source_scope: "account_activity_with_unknown_application_scope",
                    coverage: "unknown",
                    summary: entry.summary.into(),
                    daily_buckets_state: match entry.daily_state {
                        ActivityDailyState::Unavailable => "unavailable",
                        ActivityDailyState::Empty => "empty",
                        ActivityDailyState::Returned => "returned",
                    },
                })
                .collect(),
        })
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::workspace::collection_tests::Fixture;

    #[test]
    fn saved_history_after_restart_needs_no_provider_and_exposes_only_snapshot_metadata() {
        let mut f = Fixture::new("success");
        let id = f.profiles[0].clone();
        let approval = f.approve();
        f.save(&id, &approval);
        let directory = f.workspace.directory.clone();
        drop(f.workspace);
        f.workspace = Workspace::new(directory).unwrap();
        assert_eq!(
            f.workspace.bootstrap(Ok(&f.vault)).storage,
            StorageState::Ready
        );
        let value = serde_json::to_value(
            f.workspace
                .load_profile_activity_history(&id, None)
                .unwrap(),
        )
        .unwrap();
        assert_eq!(value["profileId"], id);
        assert_eq!(value["entries"].as_array().unwrap().len(), 1);
        let entry = &value["entries"][0];
        assert_eq!(entry.as_object().unwrap().len(), 10);
        assert_eq!(entry["summary"]["lifetimeTokens"], "9007199254740993");
        assert_eq!(entry["coverage"], "unknown");
        assert_eq!(
            entry["sourceScope"],
            "account_activity_with_unknown_application_scope"
        );
        for field in [
            "email",
            "homeReference",
            "freshness",
            "approvalId",
            "token",
            "delta",
        ] {
            assert!(entry.get(field).is_none());
        }
        let export = f.workspace.activity_history_export(&id, false).unwrap();
        assert!(export.rows.iter().any(|row| {
            row[5].as_deref() == Some("lifetime_tokens")
                && row[6].as_deref() == Some("9007199254740993")
        }));
        assert!(export.rows.iter().all(|row| {
            row[1..4].iter().all(Option::is_none)
                && row[21]
                    .as_deref()
                    .is_some_and(|value| value.starts_with("reading-"))
        }));
        assert!(
            f.workspace
                .load_profile_activity_history(&id, Some("../unsafe"))
                .is_err()
        );
        assert!(
            f.workspace
                .load_profile_activity_history(&id, Some(&f.profiles[1]))
                .is_err()
        );
        assert!(
            f.workspace
                .load_profile_activity_history(&f.profiles[1], None)
                .unwrap()
                .entries
                .is_empty()
        );
    }
}
