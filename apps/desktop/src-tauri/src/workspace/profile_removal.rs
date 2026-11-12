use super::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ProfileHistoryChoice {
    Archive,
    Delete,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProfileRemovalPreview {
    pub approval_id: String,
    pub profile_id: String,
    pub label: String,
    pub history: ProfileHistoryChoice,
}

pub(super) struct PreparedRemoval {
    preview: ProfileRemovalPreview,
    home_reference: String,
    revision: openusage_storage::StorageRevision,
}

impl Workspace {
    pub(crate) fn prepare_profile_removal(
        &mut self,
        profile_id: &str,
        history: ProfileHistoryChoice,
    ) -> Result<ProfileRemovalPreview, CommandError> {
        self.profile_removal = None;
        validate_profile_id(profile_id)?;
        let storage = self.storage.as_mut().ok_or(CommandError::WorkspaceLocked)?;
        let revision = storage
            .restore_state()
            .map_err(|_| CommandError::ProfileRemovalFailed)?
            .revision;
        let profile = storage
            .list_profiles()
            .map_err(profile_error)?
            .into_iter()
            .find(|profile| profile.id == profile_id)
            .ok_or(CommandError::ProfileNotFound)?;
        if profile.environment != ProfileEnvironment::Native {
            return Err(CommandError::ProviderUnsupported);
        }
        let home = storage
            .restore_home_mappings(&revision)
            .map_err(|_| CommandError::StaleProfileRemoval)?
            .into_iter()
            .find(|home| home.profile_id == profile_id)
            .ok_or(CommandError::ProfileUnavailable)?;
        validate_profile_id(&home.managed_home_reference)
            .map_err(|_| CommandError::ProfileUnavailable)?;
        let mut random = [0_u8; 16];
        getrandom::fill(&mut random).map_err(|_| CommandError::ProfileRemovalFailed)?;
        random[6] = (random[6] & 0x0f) | 0x40;
        random[8] = (random[8] & 0x3f) | 0x80;
        let preview = ProfileRemovalPreview {
            approval_id: uuid::Uuid::from_bytes(random).hyphenated().to_string(),
            profile_id: profile.id,
            label: profile.label,
            history,
        };
        self.profile_removal = Some(PreparedRemoval {
            preview: preview.clone(),
            home_reference: home.managed_home_reference,
            revision,
        });
        Ok(preview)
    }

    pub(crate) fn apply_profile_removal(
        &mut self,
        approval_id: &str,
        profile_id: &str,
        history: ProfileHistoryChoice,
        confirmed: bool,
    ) -> Result<ProfileView, CommandError> {
        restore::validate_preview_id(approval_id)?;
        validate_profile_id(profile_id)?;
        self.storage.as_ref().ok_or(CommandError::WorkspaceLocked)?;
        if !self
            .profile_removal
            .as_ref()
            .is_some_and(|prepared| prepared.preview.approval_id == approval_id)
        {
            return Err(CommandError::StaleProfileRemoval);
        }
        let prepared = self
            .profile_removal
            .take()
            .ok_or(CommandError::StaleProfileRemoval)?;
        if !confirmed {
            return Err(CommandError::InvalidArguments);
        }
        if prepared.preview.profile_id != profile_id || prepared.preview.history != history {
            return Err(CommandError::StaleProfileRemoval);
        }
        let current = self
            .storage
            .as_mut()
            .ok_or(CommandError::WorkspaceLocked)?
            .restore_state()
            .map_err(|_| CommandError::ProfileRemovalFailed)?;
        if current.revision != prepared.revision {
            return Err(CommandError::StaleProfileRemoval);
        }
        let mut next = self.list_profiles()?;
        let selected = next
            .profiles
            .iter()
            .position(|profile| profile.id == profile_id)
            .ok_or(CommandError::StaleProfileRemoval)?;
        self.revoke_monitoring_profile(profile_id, MonitoringIssue::ProfileUnavailable);
        let cancellation = self.provider.cancel_profile_operations(profile_id);
        self.monitoring_profile_cancelled(profile_id);
        self.activity_reviews.remove(profile_id);
        self.quota_requests.remove(profile_id);
        self.activity_requests.remove(profile_id);
        self.activity_sessions.remove(profile_id);
        self.quota_sessions.remove(profile_id);
        self.restore_preview.clear();
        cancellation?;
        self.storage
            .as_mut()
            .ok_or(CommandError::WorkspaceLocked)?
            .remove_profile(
                openusage_storage::ProfileRemovalRequest {
                    profile_id: profile_id.into(),
                    expected_home_reference: prepared.home_reference,
                    history: match history {
                        ProfileHistoryChoice::Archive => {
                            openusage_storage::ProfileHistoryChoice::Archive
                        }
                        ProfileHistoryChoice::Delete => {
                            openusage_storage::ProfileHistoryChoice::Delete
                        }
                    },
                },
                &prepared.revision,
            )
            .map_err(|error| match error {
                openusage_storage::ProfileRemovalError::StaleTarget
                | openusage_storage::ProfileRemovalError::NotFound => {
                    CommandError::StaleProfileRemoval
                }
                _ => CommandError::ProfileRemovalFailed,
            })?;
        match history {
            ProfileHistoryChoice::Archive => next.profiles[selected].archived = true,
            ProfileHistoryChoice::Delete => {
                next.profiles.retain(|profile| profile.id != profile_id)
            }
        }
        next.active_profiles = next
            .profiles
            .iter()
            .filter(|profile| !profile.archived)
            .count() as u32;
        Ok(next)
    }

    pub(crate) fn cancel_profile_removal(
        &mut self,
        approval_id: &str,
    ) -> Result<Cancelled, CommandError> {
        restore::validate_preview_id(approval_id)?;
        self.storage.as_ref().ok_or(CommandError::WorkspaceLocked)?;
        let cancelled = self
            .profile_removal
            .as_ref()
            .is_some_and(|prepared| prepared.preview.approval_id == approval_id);
        if cancelled {
            self.profile_removal = None;
        }
        Ok(Cancelled { cancelled })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (tempfile::TempDir, Workspace, String, String) {
        let directory = tempfile::tempdir().unwrap();
        let mut workspace = Workspace::new(directory.path().join("workspace")).unwrap();
        workspace.storage =
            Some(Storage::create(&workspace.directory.join(DATABASE_FILE), &[0x37; 32]).unwrap());
        let first = workspace
            .create_profile("Selected")
            .unwrap()
            .profiles
            .remove(0)
            .id;
        let second = workspace
            .create_profile("Other")
            .unwrap()
            .profiles
            .into_iter()
            .find(|profile| profile.id != first)
            .unwrap()
            .id;
        (directory, workspace, first, second)
    }

    fn seed_runtime(workspace: &mut Workspace, first: &str, second: &str) {
        for id in [first, second] {
            workspace
                .activity_sessions
                .insert(id.into(), (id.into(), None));
            workspace
                .quota_sessions
                .insert(id.into(), (id.into(), None));
            workspace.activity_requests.insert(id.into(), id.into());
            workspace.quota_requests.insert(id.into(), id.into());
        }
    }

    #[test]
    fn delete_preserves_homes_and_other_profiles_runtime() {
        let (_directory, mut workspace, first, second) = fixture();
        let state = workspace.storage.as_mut().unwrap().restore_state().unwrap();
        let homes = workspace
            .storage
            .as_mut()
            .unwrap()
            .restore_home_mappings(&state.revision)
            .unwrap();
        for home in &homes {
            fs::write(
                workspace
                    .directory
                    .join("profiles")
                    .join(&home.managed_home_reference)
                    .join("synthetic-sentinel"),
                home.profile_id.as_bytes(),
            )
            .unwrap();
        }
        seed_runtime(&mut workspace, &first, &second);
        let preview = workspace
            .prepare_profile_removal(&first, ProfileHistoryChoice::Delete)
            .unwrap();
        let next = workspace
            .apply_profile_removal(
                &preview.approval_id,
                &first,
                ProfileHistoryChoice::Delete,
                true,
            )
            .unwrap();
        assert_eq!(next.profiles.len(), 1);
        assert_eq!(next.profiles[0].id, second);
        assert_eq!(next.active_profiles, 1);
        for home in &homes {
            assert_eq!(
                fs::read(
                    workspace
                        .directory
                        .join("profiles")
                        .join(&home.managed_home_reference)
                        .join("synthetic-sentinel")
                )
                .unwrap(),
                home.profile_id.as_bytes()
            );
        }
        for sessions in [&workspace.activity_sessions, &workspace.quota_sessions] {
            assert!(!sessions.contains_key(&first));
            assert!(sessions.contains_key(&second));
        }
        for requests in [&workspace.activity_requests, &workspace.quota_requests] {
            assert!(!requests.contains_key(&first));
            assert!(requests.contains_key(&second));
        }
    }

    #[test]
    fn consent_choice_profile_and_stale_revisions_refuse_before_cleanup() {
        let (_directory, mut workspace, first, second) = fixture();
        seed_runtime(&mut workspace, &first, &second);
        for case in 0..4 {
            let preview = workspace
                .prepare_profile_removal(&first, ProfileHistoryChoice::Delete)
                .unwrap();
            if case == 3 {
                workspace
                    .rename_profile(&second, "Changed after confirmation")
                    .unwrap();
            }
            let result = workspace.apply_profile_removal(
                &preview.approval_id,
                if case == 1 { &second } else { &first },
                if case == 2 {
                    ProfileHistoryChoice::Archive
                } else {
                    ProfileHistoryChoice::Delete
                },
                case != 0,
            );
            assert_eq!(
                result,
                Err(if case == 0 {
                    CommandError::InvalidArguments
                } else {
                    CommandError::StaleProfileRemoval
                })
            );
            assert!(workspace.profile_removal.is_none());
            assert_eq!(workspace.list_profiles().unwrap().active_profiles, 2);
            assert!(workspace.activity_sessions.contains_key(&first));
            assert!(workspace.quota_requests.contains_key(&first));
        }
    }

    #[test]
    fn replacement_and_cancel_keep_the_current_approval_bound() {
        let (_directory, mut workspace, first, second) = fixture();
        let old = workspace
            .prepare_profile_removal(&first, ProfileHistoryChoice::Delete)
            .unwrap();
        let current = workspace
            .prepare_profile_removal(&second, ProfileHistoryChoice::Archive)
            .unwrap();
        assert_eq!(
            workspace.apply_profile_removal(
                &old.approval_id,
                &first,
                ProfileHistoryChoice::Delete,
                true
            ),
            Err(CommandError::StaleProfileRemoval)
        );
        assert!(
            !workspace
                .cancel_profile_removal(&old.approval_id)
                .unwrap()
                .cancelled
        );
        assert!(
            workspace
                .cancel_profile_removal(&current.approval_id)
                .unwrap()
                .cancelled
        );
        assert!(
            !workspace
                .cancel_profile_removal(&current.approval_id)
                .unwrap()
                .cancelled
        );
        assert_eq!(
            workspace.apply_profile_removal(
                &current.approval_id,
                &second,
                ProfileHistoryChoice::Archive,
                true
            ),
            Err(CommandError::StaleProfileRemoval)
        );
        assert_eq!(workspace.list_profiles().unwrap().active_profiles, 2);
    }

    #[test]
    fn failed_process_cleanup_preserves_records_and_consumes_approval() {
        let (_directory, mut workspace, first, second) = fixture();
        seed_runtime(&mut workspace, &first, &second);
        let preview = workspace
            .prepare_profile_removal(&first, ProfileHistoryChoice::Delete)
            .unwrap();
        workspace.provider.close().unwrap();
        assert_eq!(
            workspace.apply_profile_removal(
                &preview.approval_id,
                &first,
                ProfileHistoryChoice::Delete,
                true
            ),
            Err(CommandError::ProviderShuttingDown)
        );
        assert!(workspace.profile_removal.is_none());
        assert_eq!(workspace.list_profiles().unwrap().active_profiles, 2);
        assert!(!workspace.activity_sessions.contains_key(&first));
        assert!(workspace.activity_sessions.contains_key(&second));
    }

    #[test]
    fn archive_and_delete_do_not_require_access_to_the_tool_home() {
        for history in [ProfileHistoryChoice::Archive, ProfileHistoryChoice::Delete] {
            let (_directory, mut workspace, first, _second) = fixture();
            let state = workspace.storage.as_mut().unwrap().restore_state().unwrap();
            let home = workspace
                .storage
                .as_mut()
                .unwrap()
                .restore_home_mappings(&state.revision)
                .unwrap()
                .into_iter()
                .find(|home| home.profile_id == first)
                .unwrap();
            fs::remove_dir(
                workspace
                    .directory
                    .join("profiles")
                    .join(home.managed_home_reference),
            )
            .unwrap();
            let preview = workspace.prepare_profile_removal(&first, history).unwrap();
            assert!(
                workspace
                    .apply_profile_removal(&preview.approval_id, &first, history, true)
                    .is_ok()
            );
        }
    }

    #[test]
    fn archive_confirmation_preserves_other_profiles_and_consumes_approval() {
        let directory = tempfile::tempdir().unwrap();
        let mut workspace = Workspace::new(directory.path().join("workspace")).unwrap();
        workspace.storage =
            Some(Storage::create(&workspace.directory.join(DATABASE_FILE), &[0x37; 32]).unwrap());
        let first = workspace
            .create_profile("Selected")
            .unwrap()
            .profiles
            .remove(0);
        let second = workspace
            .create_profile("Other")
            .unwrap()
            .profiles
            .into_iter()
            .find(|profile| profile.id != first.id)
            .unwrap();
        let preview = workspace
            .prepare_profile_removal(&first.id, ProfileHistoryChoice::Archive)
            .unwrap();
        assert_eq!(preview.profile_id, first.id);
        assert_eq!(preview.label, "Selected");
        assert_eq!(preview.history, ProfileHistoryChoice::Archive);
        let serialized = serde_json::to_value(&preview).unwrap();
        assert_eq!(serialized.as_object().unwrap().len(), 4);
        let next = workspace
            .apply_profile_removal(
                &preview.approval_id,
                &first.id,
                ProfileHistoryChoice::Archive,
                true,
            )
            .unwrap();
        assert!(
            next.profiles
                .iter()
                .find(|p| p.id == first.id)
                .unwrap()
                .archived
        );
        assert!(
            !next
                .profiles
                .iter()
                .find(|p| p.id == second.id)
                .unwrap()
                .archived
        );
        assert!(workspace.profile_removal.is_none());
        assert_eq!(
            workspace.apply_profile_removal(
                &preview.approval_id,
                &first.id,
                ProfileHistoryChoice::Archive,
                true,
            ),
            Err(CommandError::StaleProfileRemoval)
        );
    }
}
