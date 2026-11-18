//! Bind parser evidence to the method, approved origin, association, and actual observation.

use super::{CommandError, Workspace, profile_error, validate_profile_id};
use openusage_storage::{
    ActivityError, ActivityReading, EvidenceLocation, EvidenceMethod, EvidenceUnusedField,
    QuotaReading, ReadingEvidenceAvailability, ReadingEvidenceContext, ReadingEvidenceInput,
    ReadingEvidenceRecord, ReadingEvidenceTarget, ResponseEvidenceInput,
};
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProfileReadingEvidence {
    profile_id: String,
    #[serde(flatten)]
    evidence: ReadingEvidenceState,
}

#[derive(Serialize)]
#[serde(tag = "availability", rename_all = "snake_case")]
enum ReadingEvidenceState {
    Complete {
        record: Box<ReadingEvidenceRecord>,
    },
    LegacyGap {
        #[serde(rename = "runId")]
        run_id: String,
        method: EvidenceMethod,
    },
}

impl Workspace {
    pub fn load_profile_reading_evidence(
        &self,
        profile_id: &str,
        method: EvidenceMethod,
        run_id: &str,
    ) -> Result<Option<ProfileReadingEvidence>, CommandError> {
        validate_profile_id(profile_id)?;
        validate_profile_id(run_id).map_err(|_| CommandError::InvalidArguments)?;
        let storage = self.storage.as_ref().ok_or(CommandError::WorkspaceLocked)?;
        if !storage
            .list_profiles()
            .map_err(profile_error)?
            .iter()
            .any(|profile| profile.id == profile_id)
        {
            return Err(CommandError::ProfileNotFound);
        }
        let Some(evidence) = storage
            .reading_evidence(ReadingEvidenceTarget {
                profile_id,
                method,
                run_id,
            })
            .map_err(|error| match error {
                ActivityError::InvalidInput => CommandError::InvalidArguments,
                _ => CommandError::ProviderEvidenceLoadFailed,
            })?
        else {
            return Ok(None);
        };
        Ok(Some(ProfileReadingEvidence {
            profile_id: profile_id.into(),
            evidence: match evidence {
                ReadingEvidenceAvailability::Complete(record) => {
                    ReadingEvidenceState::Complete { record }
                }
                ReadingEvidenceAvailability::LegacyGap => ReadingEvidenceState::LegacyGap {
                    run_id: run_id.into(),
                    method,
                },
            },
        }))
    }
}

fn bind(
    context: ReadingEvidenceContext,
    response: &openusage_codex::ResponseEvidence,
) -> ReadingEvidenceInput {
    ReadingEvidenceInput {
        context,
        adapter_revision: openusage_codex::ADAPTER_REVISION,
        response: ResponseEvidenceInput {
            projection_version: response.projection_version(),
            redacted_response_hash: response.redacted_response_hash().into(),
            format_fingerprint: response.format_fingerprint().into(),
            unused_fields: response
                .unused_fields()
                .iter()
                .map(|field| EvidenceUnusedField {
                    location: match field.location() {
                        openusage_codex::EvidenceLocation::ActivityResult => {
                            EvidenceLocation::ActivityResult
                        }
                        openusage_codex::EvidenceLocation::ActivitySummary => {
                            EvidenceLocation::ActivitySummary
                        }
                        openusage_codex::EvidenceLocation::ActivityDailyBucket => {
                            EvidenceLocation::ActivityDailyBucket
                        }
                        openusage_codex::EvidenceLocation::QuotaResult => {
                            EvidenceLocation::QuotaResult
                        }
                        openusage_codex::EvidenceLocation::QuotaLimit => {
                            EvidenceLocation::QuotaLimit
                        }
                        openusage_codex::EvidenceLocation::QuotaWindow => {
                            EvidenceLocation::QuotaWindow
                        }
                        openusage_codex::EvidenceLocation::QuotaIndividualLimit => {
                            EvidenceLocation::QuotaIndividualLimit
                        }
                        openusage_codex::EvidenceLocation::QuotaResetCredits => {
                            EvidenceLocation::QuotaResetCredits
                        }
                        openusage_codex::EvidenceLocation::QuotaCreditDetail => {
                            EvidenceLocation::QuotaCreditDetail
                        }
                    },
                    field_count: field.field_count(),
                })
                .collect(),
        },
    }
}

pub(super) fn activity(
    reading: &ActivityReading,
    response: &openusage_codex::ActivitySnapshot,
) -> ReadingEvidenceInput {
    bind(
        ReadingEvidenceContext {
            method: EvidenceMethod::Activity,
            run_id: reading.run_id.clone(),
            profile_id: reading.binding.profile_id.clone(),
            binding_id: reading.binding.binding_id.clone(),
            provider_version: reading.provider_version.clone(),
            source_kind: reading.source_kind,
            observed_at_utc: reading.observed_at_utc.clone(),
        },
        response.response_evidence(),
    )
}

pub(super) fn quota(
    reading: &QuotaReading,
    response: &openusage_codex::QuotaSnapshot,
) -> ReadingEvidenceInput {
    bind(
        ReadingEvidenceContext {
            method: EvidenceMethod::Quota,
            run_id: reading.run_id.clone(),
            profile_id: reading.binding.profile_id.clone(),
            binding_id: reading.binding.binding_id.clone(),
            provider_version: reading.provider_version.clone(),
            source_kind: reading.source_kind,
            observed_at_utc: reading.observed_at_utc.clone(),
        },
        response.response_evidence(),
    )
}

#[cfg(test)]
pub(crate) mod fixtures {
    use openusage_storage as storage;
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../tests/support/reading_evidence.rs"
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keyvault::{DatabaseKey, KeyVault, VaultError};
    use crate::workspace::{CommandError, Workspace};
    use openusage_storage::{
        ActivitySummaryValues, ProfileBinding, ProfileBindingInput, ProfileIdentityEvidence,
    };
    use zeroize::Zeroizing;

    struct UnusedVault;
    impl KeyVault for UnusedVault {
        fn load(&self) -> Result<Option<DatabaseKey>, VaultError> {
            Err(VaultError::Unavailable)
        }
        fn protect(&self, _: &[u8; 32]) -> Result<(), VaultError> {
            Err(VaultError::Unavailable)
        }
        fn remove(&self, _: &[u8; 32]) -> Result<(), VaultError> {
            Err(VaultError::Unavailable)
        }
    }

    #[test]
    fn saved_evidence_view_is_scoped_private_and_readable_after_archive() {
        let mut builder = tempfile::Builder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            builder.permissions(std::fs::Permissions::from_mode(0o700));
        }
        let directory = builder.tempdir().unwrap();
        let mut workspace = Workspace::new(directory.path().join("workspace")).unwrap();
        let run = "33333333-3333-4333-8333-333333333333";
        assert_eq!(
            workspace
                .load_profile_reading_evidence(run, EvidenceMethod::Activity, run)
                .err(),
            Some(CommandError::WorkspaceLocked)
        );
        workspace
            .setup(
                Err::<&UnusedVault, _>(VaultError::Unavailable),
                Some(Zeroizing::new(
                    "OpenUsage synthetic evidence password 2026".into(),
                )),
            )
            .unwrap();
        let profile = workspace
            .create_profile("Synthetic evidence owner")
            .unwrap()
            .profiles[0]
            .id
            .clone();
        let other = workspace
            .create_profile("Other synthetic owner")
            .unwrap()
            .profiles
            .into_iter()
            .find(|item| item.id != profile)
            .unwrap()
            .id;
        let time = "2026-11-16T00:00:00.000Z";
        let binding = "22222222-2222-4222-8222-222222222222";
        let identity = ProfileIdentityEvidence {
            email: "synthetic-private-email@example.invalid".into(),
            plan: Some("private-plan".into()),
        };
        workspace
            .storage
            .as_mut()
            .unwrap()
            .accept_activity_with_binding(
                fixtures::activity(ActivityReading {
                    run_id: run.into(),
                    binding: ProfileBinding {
                        profile_id: profile.clone(),
                        home_reference: profile.clone(),
                        binding_id: binding.into(),
                    },
                    identity_evidence: identity.clone(),
                    provider_version: "0.163.0".into(),
                    source_kind: openusage_storage::ActivitySourceKind::TestFixture,
                    started_at_utc: time.into(),
                    observed_at_utc: time.into(),
                    completed_at_utc: time.into(),
                    summary: ActivitySummaryValues {
                        lifetime_tokens: Some("9007199254740993".into()),
                        ..Default::default()
                    },
                    daily_usage: Some(vec![openusage_storage::ActivityDayValue {
                        provider_date: "2026-11-15".into(),
                        tokens: "9007199254740993".into(),
                    }]),
                    evidence: None,
                }),
                Some(ProfileBindingInput {
                    profile_id: profile.clone(),
                    expected_home_reference: profile.clone(),
                    expected_previous_binding_id: None,
                    new_binding_id: binding.into(),
                    evidence: identity,
                    observed_at_utc: time.into(),
                }),
            )
            .unwrap();
        let before =
            serde_json::to_value(workspace.load_profile_activity(&profile).unwrap()).unwrap();
        assert_eq!(
            before["audit"],
            serde_json::json!({
                "availability": "complete", "readingStatus": "official", "sourceLevel": "official"
            })
        );
        assert_eq!(before["dailyUsageBuckets"][0]["collectionRunId"], run);
        assert_eq!(before["dailyUsageBuckets"][0]["bindingId"], binding);
        assert_eq!(before["dailyUsageBuckets"][0]["providerVersion"], "0.163.0");
        assert_eq!(before["dailyUsageBuckets"][0]["sourceKind"], "test_fixture");
        assert_eq!(before["dailyUsageBuckets"][0]["audit"], before["audit"]);
        let view = workspace
            .load_profile_reading_evidence(&profile, EvidenceMethod::Activity, run)
            .unwrap()
            .unwrap();
        let value = serde_json::to_value(view).unwrap();
        assert_eq!(value["profileId"], profile);
        assert_eq!(value["availability"], "complete");
        assert_eq!(value["record"]["runId"], run);
        assert_eq!(value["record"]["method"], "activity");
        assert_eq!(value["record"]["redactedResponseHash"], "a".repeat(64));
        for private in [
            "synthetic-private-email@example.invalid",
            "private-plan",
            directory.path().to_str().unwrap(),
        ] {
            assert!(!value.to_string().contains(private));
        }
        assert!(
            workspace
                .load_profile_reading_evidence(&other, EvidenceMethod::Activity, run)
                .unwrap()
                .is_none()
        );
        assert!(
            workspace
                .load_profile_reading_evidence(&profile, EvidenceMethod::Quota, run)
                .unwrap()
                .is_none()
        );
        assert_eq!(
            workspace
                .load_profile_reading_evidence(run, EvidenceMethod::Activity, run)
                .err(),
            Some(CommandError::ProfileNotFound)
        );
        assert_eq!(
            workspace
                .load_profile_reading_evidence(&profile, EvidenceMethod::Activity, "../private")
                .err(),
            Some(CommandError::InvalidArguments)
        );
        assert_eq!(
            serde_json::to_value(workspace.load_profile_activity(&profile).unwrap()).unwrap(),
            before
        );
        workspace.set_profile_archived(&profile, true).unwrap();
        assert_eq!(
            serde_json::to_value(
                workspace
                    .load_profile_reading_evidence(&profile, EvidenceMethod::Activity, run)
                    .unwrap()
                    .unwrap()
            )
            .unwrap(),
            value
        );
    }
}
