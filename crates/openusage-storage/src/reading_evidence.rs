//! Immutable evidence for new Codex method observations. Identity evidence stays separate.

use crate::activity::{activity_utc_timestamp, read_error, require, require_uuid, write_error};
use crate::{ActivityError, ActivitySourceKind, ProfileBinding, Storage};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

const ADAPTER: &str = "codex_app_server";
const CHECKS: &str = r#"[{"id":"rpc_envelope","result":"passed"},{"id":"request_id","result":"passed"},{"id":"response_fields","result":"passed"}]"#;
const MAX_UNUSED_JSON: usize = 2048;
const MAX_UNUSED_FIELDS: u64 = 1_048_576;
pub(crate) const TABLES: &[(&str, &str)] = &[
    (
        "reading_evidence",
        "collection_run_id,adapter_id,adapter_revision,projection_version,source_level,reading_status,check_results,comparison_result,redacted_response_hash,format_fingerprint,unused_fields",
    ),
    ("reading_evidence_legacy", "collection_run_id"),
];

type Result<T> = std::result::Result<T, ActivityError>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceMethod {
    Activity,
    Quota,
}

#[derive(Clone, Copy, Debug)]
pub struct ReadingEvidenceTarget<'a> {
    pub profile_id: &'a str,
    pub method: EvidenceMethod,
    pub run_id: &'a str,
}

impl EvidenceMethod {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Activity => crate::ACTIVITY_SOURCE_METHOD,
            Self::Quota => crate::QUOTA_SOURCE_METHOD,
        }
    }

    fn parse(value: &str) -> Result<Self> {
        match value {
            crate::ACTIVITY_SOURCE_METHOD => Ok(Self::Activity),
            crate::QUOTA_SOURCE_METHOD => Ok(Self::Quota),
            _ => Err(ActivityError::ReadFailed),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceLocation {
    ActivityResult,
    ActivitySummary,
    ActivityDailyBucket,
    QuotaResult,
    QuotaLimit,
    QuotaWindow,
    QuotaIndividualLimit,
    QuotaResetCredits,
    QuotaCreditDetail,
}

impl EvidenceLocation {
    fn method(self) -> EvidenceMethod {
        match self {
            Self::ActivityResult | Self::ActivitySummary | Self::ActivityDailyBucket => {
                EvidenceMethod::Activity
            }
            _ => EvidenceMethod::Quota,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct EvidenceUnusedField {
    pub location: EvidenceLocation,
    pub field_count: u32,
}

#[derive(Clone, Debug)]
pub struct ReadingEvidenceContext {
    pub method: EvidenceMethod,
    pub run_id: String,
    pub profile_id: String,
    pub binding_id: String,
    pub provider_version: String,
    pub source_kind: ActivitySourceKind,
    pub observed_at_utc: String,
}

#[derive(Clone, Debug)]
pub struct ResponseEvidenceInput {
    pub projection_version: u8,
    pub redacted_response_hash: String,
    pub format_fingerprint: String,
    pub unused_fields: Vec<EvidenceUnusedField>,
}

#[derive(Clone, Debug)]
pub struct ReadingEvidenceInput {
    pub context: ReadingEvidenceContext,
    pub adapter_revision: u32,
    pub response: ResponseEvidenceInput,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadingEvidenceCheck {
    pub id: &'static str,
    pub result: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadingEvidenceRecord {
    pub run_id: String,
    pub method: EvidenceMethod,
    pub adapter_id: &'static str,
    pub adapter_revision: u32,
    pub source_level: &'static str,
    pub reading_status: &'static str,
    pub source_kind: ActivitySourceKind,
    pub provider_version: String,
    pub observed_at_utc: String,
    pub projection_version: u8,
    pub redacted_response_hash: String,
    pub format_fingerprint: String,
    pub check_results: Vec<ReadingEvidenceCheck>,
    pub comparison_result: &'static str,
    pub unused_fields: Vec<EvidenceUnusedField>,
}

#[derive(Debug)]
pub enum ReadingEvidenceAvailability {
    Complete(Box<ReadingEvidenceRecord>),
    LegacyGap,
}

pub(crate) struct EvidenceBinding<'a> {
    pub method: EvidenceMethod,
    pub run_id: &'a str,
    pub binding: &'a ProfileBinding,
    pub provider_version: &'a str,
    pub source_kind: ActivitySourceKind,
    pub observed_at_utc: &'a str,
}

fn valid_version(value: &str) -> bool {
    value.len() <= 32
        && value.split('.').count() == 3
        && value.split('.').all(|part| {
            !part.is_empty()
                && part.bytes().all(|byte| byte.is_ascii_digit())
                && part.parse::<u32>().is_ok()
        })
}

fn valid_hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn counters_json(fields: &[EvidenceUnusedField]) -> Result<String> {
    require(fields.len() <= 9)?;
    let mut locations = BTreeSet::new();
    let mut total = 0u64;
    for field in fields {
        require(field.field_count > 0 && locations.insert(field.location))?;
        total += u64::from(field.field_count);
    }
    require(total <= MAX_UNUSED_FIELDS)?;
    let mut fields = fields.to_vec();
    fields.sort_by_key(|field| field.location);
    let json = serde_json::to_string(&fields).map_err(|_| ActivityError::InvalidInput)?;
    require(json.len() <= MAX_UNUSED_JSON)?;
    Ok(json)
}

pub(crate) fn validate(
    input: Option<&ReadingEvidenceInput>,
    expected: EvidenceBinding<'_>,
) -> Result<()> {
    let input = input.ok_or(ActivityError::InvalidInput)?;
    let context = &input.context;
    require(
        context.method == expected.method
            && context.run_id == expected.run_id
            && context.profile_id == expected.binding.profile_id
            && context.binding_id == expected.binding.binding_id
            && context.provider_version == expected.provider_version
            && context.source_kind == expected.source_kind
            && context.observed_at_utc == expected.observed_at_utc,
    )?;
    require_uuid(&context.run_id)?;
    require_uuid(&context.profile_id)?;
    require_uuid(&context.binding_id)?;
    require(
        valid_version(&context.provider_version)
            && activity_utc_timestamp(&context.observed_at_utc)
            && (1..=65535).contains(&input.adapter_revision)
            && input.response.projection_version == 1
            && valid_hash(&input.response.redacted_response_hash)
            && valid_hash(&input.response.format_fingerprint),
    )?;
    counters_json(&input.response.unused_fields)?;
    require(
        input
            .response
            .unused_fields
            .iter()
            .all(|field| field.location.method() == context.method),
    )?;
    Ok(())
}

pub(crate) fn write(connection: &Connection, input: &ReadingEvidenceInput) -> Result<()> {
    let unused = counters_json(&input.response.unused_fields)?;
    connection.execute(
        "INSERT INTO reading_evidence(collection_run_id,adapter_id,adapter_revision,projection_version,source_level,reading_status,check_results,comparison_result,redacted_response_hash,format_fingerprint,unused_fields) VALUES(?1,?2,?3,?4,'official','official',?5,'not_performed',?6,?7,?8)",
        params![input.context.run_id, ADAPTER, input.adapter_revision, input.response.projection_version, CHECKS, input.response.redacted_response_hash, input.response.format_fingerprint, unused],
    ).map_err(write_error)?;
    Ok(())
}

struct StoredEvidence {
    adapter: String,
    revision: u32,
    projection: u8,
    level: String,
    status: String,
    checks: String,
    comparison: String,
    response_hash: String,
    fingerprint: String,
    unused: String,
}

fn stored_evidence(row: &rusqlite::Row<'_>, offset: usize) -> rusqlite::Result<StoredEvidence> {
    Ok(StoredEvidence {
        adapter: row.get(offset)?,
        revision: row.get(offset + 1)?,
        projection: row.get(offset + 2)?,
        level: row.get(offset + 3)?,
        status: row.get(offset + 4)?,
        checks: row.get(offset + 5)?,
        comparison: row.get(offset + 6)?,
        response_hash: row.get(offset + 7)?,
        fingerprint: row.get(offset + 8)?,
        unused: row.get(offset + 9)?,
    })
}

fn validate_stored(
    evidence: &StoredEvidence,
    version: &str,
    observed_at: &str,
    method: EvidenceMethod,
) -> Result<Vec<EvidenceUnusedField>> {
    if evidence.adapter != ADAPTER
        || !(1..=65535).contains(&evidence.revision)
        || evidence.projection != 1
        || evidence.level != "official"
        || evidence.status != "official"
        || evidence.checks != CHECKS
        || evidence.comparison != "not_performed"
        || !valid_hash(&evidence.response_hash)
        || !valid_hash(&evidence.fingerprint)
        || !valid_version(version)
        || !activity_utc_timestamp(observed_at)
        || evidence.unused.len() > MAX_UNUSED_JSON
    {
        return Err(ActivityError::ReadFailed);
    }
    let unused: Vec<EvidenceUnusedField> =
        serde_json::from_str(&evidence.unused).map_err(|_| ActivityError::ReadFailed)?;
    if counters_json(&unused).map_err(|_| ActivityError::ReadFailed)? != evidence.unused
        || !unused.iter().all(|field| field.location.method() == method)
    {
        return Err(ActivityError::ReadFailed);
    }
    Ok(unused)
}

pub(crate) fn validate_snapshot(connection: &Connection, schema: &str) -> Result<()> {
    if !matches!(schema, "main" | "openusage_checkpoint") {
        return Err(ActivityError::ReadFailed);
    }
    let unexplained: bool = connection.query_row(&format!(
        "SELECT EXISTS(SELECT 1 FROM {schema}.collection_runs r LEFT JOIN {schema}.reading_evidence e ON e.collection_run_id=r.id LEFT JOIN {schema}.reading_evidence_legacy l ON l.collection_run_id=r.id WHERE (e.collection_run_id IS NULL)=(l.collection_run_id IS NULL))"
    ), [], |row| row.get(0)).map_err(read_error)?;
    if unexplained {
        return Err(ActivityError::ReadFailed);
    }
    let mut statement = connection.prepare(&format!(
        "SELECT r.provider_version,r.observed_at_utc,r.source_method,e.adapter_id,e.adapter_revision,e.projection_version,e.source_level,e.reading_status,e.check_results,e.comparison_result,e.redacted_response_hash,e.format_fingerprint,e.unused_fields FROM {schema}.reading_evidence e JOIN {schema}.collection_runs r ON r.id=e.collection_run_id"
    )).map_err(read_error)?;
    let mut rows = statement.query([]).map_err(read_error)?;
    while let Some(row) = rows.next().map_err(read_error)? {
        let version: String = row.get(0).map_err(read_error)?;
        let observed: String = row.get(1).map_err(read_error)?;
        let method = EvidenceMethod::parse(&row.get::<_, String>(2).map_err(read_error)?)?;
        let evidence = stored_evidence(row, 3).map_err(read_error)?;
        validate_stored(&evidence, &version, &observed, method)?;
    }
    Ok(())
}

#[cfg(test)]
fn fixture_input(expected: EvidenceBinding<'_>) -> ReadingEvidenceInput {
    ReadingEvidenceInput {
        context: ReadingEvidenceContext {
            method: expected.method,
            run_id: expected.run_id.into(),
            profile_id: expected.binding.profile_id.clone(),
            binding_id: expected.binding.binding_id.clone(),
            provider_version: expected.provider_version.into(),
            source_kind: expected.source_kind,
            observed_at_utc: expected.observed_at_utc.into(),
        },
        adapter_revision: 1,
        response: ResponseEvidenceInput {
            projection_version: 1,
            redacted_response_hash: "a".repeat(64),
            format_fingerprint: "b".repeat(64),
            unused_fields: vec![],
        },
    }
}

// Storage-only fixtures simulate an accepted parser result. They establish no provider qualification.
#[cfg(test)]
pub(crate) fn fixture_activity(mut reading: crate::ActivityReading) -> crate::ActivityReading {
    reading.evidence = Some(fixture_input(EvidenceBinding {
        method: EvidenceMethod::Activity,
        run_id: &reading.run_id,
        binding: &reading.binding,
        provider_version: &reading.provider_version,
        source_kind: reading.source_kind,
        observed_at_utc: &reading.observed_at_utc,
    }));
    reading
}

#[cfg(test)]
pub(crate) fn fixture_quota(mut reading: crate::QuotaReading) -> crate::QuotaReading {
    reading.evidence = Some(fixture_input(EvidenceBinding {
        method: EvidenceMethod::Quota,
        run_id: &reading.run_id,
        binding: &reading.binding,
        provider_version: &reading.provider_version,
        source_kind: reading.source_kind,
        observed_at_utc: &reading.observed_at_utc,
    }));
    reading
}

struct EvidenceRow {
    run_id: String,
    binding_id: String,
    provider_version: String,
    source_kind: String,
    observed_at: String,
    method: String,
    complete: Option<StoredEvidence>,
    legacy: bool,
}

impl Storage {
    /// One snapshot binds evidence availability to the selected profile, method, and run.
    pub fn reading_evidence(
        &self,
        target: ReadingEvidenceTarget<'_>,
    ) -> Result<Option<ReadingEvidenceAvailability>> {
        read(&self.connection, target)
    }
}

pub(crate) fn read(
    connection: &Connection,
    target: ReadingEvidenceTarget<'_>,
) -> Result<Option<ReadingEvidenceAvailability>> {
    Ok(read_observation(connection, target)?.map(|value| value.availability))
}

pub(crate) struct EvidenceObservation {
    pub context: ReadingEvidenceContext,
    pub availability: ReadingEvidenceAvailability,
}

pub(crate) fn read_observation(
    connection: &Connection,
    target: ReadingEvidenceTarget<'_>,
) -> Result<Option<EvidenceObservation>> {
    require_uuid(target.profile_id)?;
    require_uuid(target.run_id)?;
    let mut statement = connection.prepare_cached(
            "SELECT r.id,r.provider_version,r.source_kind,r.observed_at_utc,r.source_method,e.collection_run_id,e.adapter_id,e.adapter_revision,e.projection_version,e.source_level,e.reading_status,e.check_results,e.comparison_result,e.redacted_response_hash,e.format_fingerprint,e.unused_fields,l.collection_run_id,r.binding_id FROM collection_runs r LEFT JOIN reading_evidence e ON e.collection_run_id=r.id LEFT JOIN reading_evidence_legacy l ON l.collection_run_id=r.id WHERE r.id=?1 AND r.profile_id=?2 AND r.source_method=?3",
        ).map_err(read_error)?;
    let row = statement
        .query_row(
            params![target.run_id, target.profile_id, target.method.as_str()],
            |row| {
                Ok(EvidenceRow {
                    run_id: row.get(0)?,
                    binding_id: row.get(17)?,
                    provider_version: row.get(1)?,
                    source_kind: row.get(2)?,
                    observed_at: row.get(3)?,
                    method: row.get(4)?,
                    complete: if row.get::<_, Option<String>>(5)?.is_some() {
                        Some(StoredEvidence {
                            adapter: row.get(6)?,
                            revision: row.get(7)?,
                            projection: row.get(8)?,
                            level: row.get(9)?,
                            status: row.get(10)?,
                            checks: row.get(11)?,
                            comparison: row.get(12)?,
                            response_hash: row.get(13)?,
                            fingerprint: row.get(14)?,
                            unused: row.get(15)?,
                        })
                    } else {
                        None
                    },
                    legacy: row.get::<_, Option<String>>(16)?.is_some(),
                })
            },
        )
        .optional()
        .map_err(read_error)?;
    let Some(row) = row else { return Ok(None) };
    require_uuid(&row.binding_id).map_err(|_| ActivityError::ReadFailed)?;
    let method = EvidenceMethod::parse(&row.method)?;
    let source_kind = match row.source_kind.as_str() {
        "official_artifact" => ActivitySourceKind::OfficialArtifact,
        "test_fixture" => ActivitySourceKind::TestFixture,
        _ => return Err(ActivityError::ReadFailed),
    };
    let context = ReadingEvidenceContext {
        method,
        run_id: row.run_id.clone(),
        profile_id: target.profile_id.into(),
        binding_id: row.binding_id,
        provider_version: row.provider_version.clone(),
        source_kind,
        observed_at_utc: row.observed_at.clone(),
    };
    let availability = match (row.complete, row.legacy) {
        (None, true) => ReadingEvidenceAvailability::LegacyGap,
        (Some(_), true) | (None, false) => return Err(ActivityError::ReadFailed),
        (Some(evidence), false) => {
            let unused =
                validate_stored(&evidence, &row.provider_version, &row.observed_at, method)?;
            ReadingEvidenceAvailability::Complete(Box::new(ReadingEvidenceRecord {
                run_id: row.run_id,
                method,
                adapter_id: ADAPTER,
                adapter_revision: evidence.revision,
                source_level: "official",
                reading_status: "official",
                source_kind,
                provider_version: row.provider_version,
                observed_at_utc: row.observed_at,
                projection_version: evidence.projection,
                redacted_response_hash: evidence.response_hash,
                format_fingerprint: evidence.fingerprint,
                check_results: ["rpc_envelope", "request_id", "response_fields"]
                    .into_iter()
                    .map(|id| ReadingEvidenceCheck {
                        id,
                        result: "passed",
                    })
                    .collect(),
                comparison_result: "not_performed",
                unused_fields: unused,
            }))
        }
    };
    Ok(Some(EvidenceObservation {
        context,
        availability,
    }))
}
