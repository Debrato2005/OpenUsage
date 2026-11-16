//! Bind parser evidence to the method, approved origin, association, and actual observation.

use openusage_storage::{
    ActivityReading, EvidenceLocation, EvidenceMethod, EvidenceUnusedField, QuotaReading,
    ReadingEvidenceContext, ReadingEvidenceInput, ResponseEvidenceInput,
};

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
