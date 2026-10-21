//! Publish only joined, current provider failures. No raw provider text crosses this boundary.
use super::*;
use openusage_storage::{ActivitySourceKind, OperationalFailure, OperationalFailureKind};
use std::time::{Duration, SystemTime};

impl Workspace {
    pub(crate) fn record_profile_read_failure(
        &mut self,
        failure: Box<provider::CompletedReadFailure>,
    ) -> Result<(), CommandError> {
        let binding = failure.binding().clone();
        let id = &binding.profile_id;
        let request = failure.request_id();
        let quota_current = self.quota_requests.get(id).map(String::as_str) == Some(request);
        let activity_current = self.activity_requests.get(id).map(String::as_str) == Some(request);
        let (current, cancelled) = match failure.operation() {
            provider::ReadOperation::Quota => (quota_current, CommandError::ProviderQuotaCancelled),
            provider::ReadOperation::Activity => {
                (activity_current, CommandError::ProviderActivityCancelled)
            }
            provider::ReadOperation::Collection => (
                quota_current && activity_current,
                CommandError::ProviderCollectionCancelled,
            ),
        };
        if !current {
            failure.abort();
            return Err(cancelled);
        }
        self.require_provider_binding(id, &binding.home_reference, &binding.revision, cancelled)?;
        failure.publish(|binding, evidence| {
            if evidence.observed_monotonic.elapsed() >= Duration::from_secs(600) {
                return Err(cancelled);
            }
            let source_kind = match evidence.source_kind {
                provider::ExecutableSourceKind::OfficialArtifact => {
                    ActivitySourceKind::OfficialArtifact
                }
                #[cfg(any(test, feature = "native-test"))]
                provider::ExecutableSourceKind::TestFixture => ActivitySourceKind::TestFixture,
                provider::ExecutableSourceKind::Unverified => {
                    return Err(CommandError::ProviderSourceUnverified);
                }
            };
            let kind = match evidence.kind {
                provider::ReadFailureKind::AuthenticationRequired => {
                    OperationalFailureKind::AuthenticationRequired
                }
                provider::ReadFailureKind::TransientFailure => {
                    OperationalFailureKind::TransientCollectionFailure
                }
            };
            let now =
                restore::timestamp(SystemTime::now()).ok_or(CommandError::StorageUnavailable)?;
            let observed =
                restore::timestamp(evidence.observed_at).ok_or(CommandError::StorageUnavailable)?;
            self.storage
                .as_mut()
                .ok_or(CommandError::WorkspaceLocked)?
                .record_operational_failure(
                    &OperationalFailure {
                        request_id: evidence.request_id.clone(),
                        profile_id: binding.profile_id.clone(),
                        expected_home_reference: binding.home_reference.clone(),
                        expected_binding_id: evidence.expected_binding_id.clone(),
                        provider_version: evidence.tested_provider_version.into(),
                        source_kind,
                        observed_at_utc: observed,
                        kind,
                    },
                    &now,
                )
                .map_err(|error| match error {
                    StorageError::InvalidAlertEvidence => cancelled,
                    _ => CommandError::StorageUnavailable,
                })?;
            self.restore_preview.clear();
            Ok(())
        })
    }
}
