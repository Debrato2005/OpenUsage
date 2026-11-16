// Synthetic storage fixture metadata only. This does not establish an actual provider observation.
// The including module names its storage crate `storage`.

fn input(context: storage::ReadingEvidenceContext) -> storage::ReadingEvidenceInput {
    storage::ReadingEvidenceInput {
        context,
        adapter_revision: 1,
        response: storage::ResponseEvidenceInput {
            projection_version: 1,
            redacted_response_hash: "a".repeat(64),
            format_fingerprint: "b".repeat(64),
            unused_fields: vec![],
        },
    }
}

#[allow(dead_code)] // A fixture binary can exercise only one method.
pub fn activity(mut reading: storage::ActivityReading) -> storage::ActivityReading {
    reading.evidence = Some(input(storage::ReadingEvidenceContext {
        method: storage::EvidenceMethod::Activity,
        run_id: reading.run_id.clone(),
        profile_id: reading.binding.profile_id.clone(),
        binding_id: reading.binding.binding_id.clone(),
        provider_version: reading.provider_version.clone(),
        source_kind: reading.source_kind,
        observed_at_utc: reading.observed_at_utc.clone(),
    }));
    reading
}

#[allow(dead_code)] // A fixture binary can exercise only one method.
pub fn quota(mut reading: storage::QuotaReading) -> storage::QuotaReading {
    reading.evidence = Some(input(storage::ReadingEvidenceContext {
        method: storage::EvidenceMethod::Quota,
        run_id: reading.run_id.clone(),
        profile_id: reading.binding.profile_id.clone(),
        binding_id: reading.binding.binding_id.clone(),
        provider_version: reading.provider_version.clone(),
        source_kind: reading.source_kind,
        observed_at_utc: reading.observed_at_utc.clone(),
    }));
    reading
}
