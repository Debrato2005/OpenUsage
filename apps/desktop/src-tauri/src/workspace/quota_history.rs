//! Historical observations carry their original local association, never live freshness.
use super::*;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProfileQuotaHistory {
    profile_id: String,
    entries: Vec<QuotaHistoryEntry>,
    next_before_run_id: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct QuotaHistoryEntry {
    profile_id: String,
    binding_id: String,
    collection_run_id: String,
    observed_at_utc: String,
    source_kind: &'static str,
    provider_version: String,
    limits: Vec<QuotaHistoryLimit>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct QuotaHistoryLimit {
    limit_id: String,
    limit_name: Option<String>,
    primary: Option<provider::QuotaWindowView>,
    secondary: Option<provider::QuotaWindowView>,
}

fn historical_window(
    window: openusage_storage::QuotaWindow,
    fallback: &str,
) -> provider::QuotaWindowView {
    provider::QuotaWindowView::from_window(
        openusage_codex::QuotaWindow {
            used_percent: window.used_percent,
            derived_remaining_percent: 100 - window.used_percent,
            window_duration_mins: window.window_duration_mins,
            resets_at_unix: window.resets_at_unix,
        },
        fallback,
    )
}

impl Workspace {
    pub fn quota_history_export(
        &mut self,
        profile_id: &str,
        include_identifiers: bool,
    ) -> Result<openusage_storage::ReadingsExport, CommandError> {
        validate_profile_id(profile_id)?;
        self.storage
            .as_mut()
            .ok_or(CommandError::WorkspaceLocked)?
            .quota_history_export(profile_id, include_identifiers)
            .map_err(export_error)
    }

    pub fn load_profile_quota_history(
        &mut self,
        profile_id: &str,
        before_run_id: Option<&str>,
    ) -> Result<ProfileQuotaHistory, CommandError> {
        validate_profile_id(profile_id)?;
        if let Some(cursor) = before_run_id {
            restore::validate_preview_id(cursor)?;
        }
        let page = self
            .storage
            .as_mut()
            .ok_or(CommandError::WorkspaceLocked)?
            .quota_history(profile_id, before_run_id)
            .map_err(|error| match error {
                openusage_storage::ActivityError::InvalidInput => CommandError::InvalidArguments,
                openusage_storage::ActivityError::ProfileUnavailable => {
                    CommandError::ProfileNotFound
                }
                _ => CommandError::ProviderQuotaLoadFailed,
            })?;
        Ok(ProfileQuotaHistory {
            profile_id: page.profile_id,
            entries: page
                .entries
                .into_iter()
                .map(|entry| QuotaHistoryEntry {
                    profile_id: entry.commit.profile_id,
                    binding_id: entry.commit.binding_id,
                    collection_run_id: entry.commit.collection_run_id,
                    observed_at_utc: entry.commit.observed_at_utc,
                    source_kind: entry.source_kind.as_str(),
                    provider_version: entry.provider_version,
                    limits: entry
                        .limits
                        .into_iter()
                        .map(|limit| QuotaHistoryLimit {
                            limit_id: limit.limit_id,
                            limit_name: limit.limit_name,
                            primary: limit
                                .primary
                                .map(|window| historical_window(window, "Primary window")),
                            secondary: limit
                                .secondary
                                .map(|window| historical_window(window, "Secondary window")),
                        })
                        .collect(),
                })
                .collect(),
            next_before_run_id: page.next_before_run_id,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keyvault::DatabaseKey;
    use std::cell::RefCell;

    #[derive(Default)]
    struct TestVault(RefCell<Option<DatabaseKey>>);
    impl KeyVault for TestVault {
        fn load(&self) -> Result<Option<DatabaseKey>, VaultError> {
            Ok(self.0.borrow().as_ref().map(|key| Zeroizing::new(**key)))
        }
        fn protect(&self, key: &[u8; 32]) -> Result<(), VaultError> {
            *self.0.borrow_mut() = Some(Zeroizing::new(*key));
            Ok(())
        }
    }

    #[test]
    fn quota_history_dto_preserves_exact_windows_and_profile_scope_without_identity_or_current_state()
     {
        let root = tempfile::tempdir().unwrap();
        let mut workspace = Workspace::new(root.path().join("workspace")).unwrap();
        assert!(matches!(
            workspace.load_profile_quota_history("11111111-1111-4111-8111-111111111111", None),
            Err(CommandError::WorkspaceLocked)
        ));
        workspace.setup(Ok(&TestVault::default()), None).unwrap();
        let first = workspace
            .create_profile("Synthetic first")
            .unwrap()
            .profiles[0]
            .id
            .clone();
        let second = workspace
            .create_profile("Synthetic second")
            .unwrap()
            .profiles
            .into_iter()
            .find(|profile| profile.id != first)
            .unwrap()
            .id;
        let binding = "11111111-1111-4111-8111-111111111111";
        let run = "22222222-2222-4222-8222-222222222222";
        let time = "2026-10-21T00:00:00.001Z";
        let evidence = openusage_storage::ProfileIdentityEvidence {
            email: "synthetic-private@example.invalid".into(),
            plan: Some("synthetic-private-plan".into()),
        };
        workspace
            .storage
            .as_mut()
            .unwrap()
            .accept_quota(
                openusage_storage::QuotaReading {
                    run_id: run.into(),
                    binding: openusage_storage::ProfileBinding {
                        profile_id: first.clone(),
                        home_reference: first.clone(),
                        binding_id: binding.into(),
                    },
                    identity_evidence: evidence.clone(),
                    provider_version: "0.163.0".into(),
                    source_kind: openusage_storage::ActivitySourceKind::TestFixture,
                    started_at_utc: time.into(),
                    observed_at_utc: time.into(),
                    completed_at_utc: time.into(),
                    limits: vec![openusage_storage::QuotaLimit {
                        limit_id: "synthetic".into(),
                        limit_name: None,
                        primary: Some(openusage_storage::QuotaWindow {
                            used_percent: 0,
                            window_duration_mins: Some(300),
                            resets_at_unix: Some(253_402_300_799),
                        }),
                        secondary: Some(openusage_storage::QuotaWindow {
                            used_percent: 100,
                            window_duration_mins: None,
                            resets_at_unix: None,
                        }),
                        reached_type: None,
                        monthly_credit_limit: None,
                        spend_control_reached: None,
                    }],
                    available_reset_credits: Some("9223372036854775807".into()),
                    reset_credit_details: None,
                },
                Some(openusage_storage::ProfileBindingInput {
                    profile_id: first.clone(),
                    expected_home_reference: first.clone(),
                    expected_previous_binding_id: None,
                    new_binding_id: binding.into(),
                    evidence,
                    observed_at_utc: time.into(),
                }),
            )
            .unwrap();
        workspace.set_profile_archived(&first, true).unwrap();
        let serialized =
            serde_json::to_value(workspace.load_profile_quota_history(&first, None).unwrap())
                .unwrap();
        assert_eq!(
            serialized,
            serde_json::json!({
                "profileId": first, "nextBeforeRunId": null,
                "entries": [{"profileId": first, "bindingId": binding, "collectionRunId": run,
                    "observedAtUtc": time, "sourceKind": "test_fixture", "providerVersion": "0.163.0",
                    "limits": [{"limitId": "synthetic", "limitName": null,
                        "primary": {"usedPercent": 0, "derivedRemainingPercent": 100, "windowDurationMins": 300, "resetsAtUnix": 253_402_300_799i64, "label": "Five-hour"},
                        "secondary": {"usedPercent": 100, "derivedRemainingPercent": 0, "windowDurationMins": null, "resetsAtUnix": null, "label": "Secondary window"}}]}]
            })
        );
        let empty =
            serde_json::to_value(workspace.load_profile_quota_history(&second, None).unwrap())
                .unwrap();
        assert_eq!(
            empty,
            serde_json::json!({"profileId":second,"entries":[],"nextBeforeRunId":null})
        );
        assert!(matches!(
            workspace.load_profile_quota_history(&second, Some(run)),
            Err(CommandError::InvalidArguments)
        ));
        assert!(matches!(
            workspace.load_profile_quota_history("33333333-3333-4333-8333-333333333333", None),
            Err(CommandError::ProfileNotFound)
        ));
    }
}
