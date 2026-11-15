//! Versioned, lossy projections of accepted wire responses. No raw response is retained.

use crate::ProtocolError;
use serde::{
    Serialize,
    de::{DeserializeSeed, IgnoredAny, MapAccess, SeqAccess, Visitor},
};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fmt};

/// Producer metadata only. Observation, acceptance, storage, and comparison remain separate.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseEvidence {
    projection_version: u8,
    redacted_response_hash: String,
    format_fingerprint: String,
    unused_fields: Vec<UnusedFieldChange>,
}

impl ResponseEvidence {
    pub fn projection_version(&self) -> u8 {
        self.projection_version
    }

    pub fn redacted_response_hash(&self) -> &str {
        &self.redacted_response_hash
    }

    pub fn format_fingerprint(&self) -> &str {
        &self.format_fingerprint
    }

    pub fn unused_fields(&self) -> &[UnusedFieldChange] {
        &self.unused_fields
    }
}

/// Fixed schema locations contain no provider keys, identifiers, or array indexes.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
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

/// Counts occurrences at a known parent. Names, values, and descendants are discarded.
/// Equal counts cannot distinguish unknown-field renames, types, or value changes.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnusedFieldChange {
    location: EvidenceLocation,
    field_count: u32,
}

impl UnusedFieldChange {
    pub fn location(&self) -> EvidenceLocation {
        self.location
    }

    pub fn field_count(&self) -> u32 {
        self.field_count
    }
}

#[derive(Clone, Copy)]
enum ReadingKind {
    Activity,
    Quota { authoritative_map: bool },
}

#[derive(Clone, Copy)]
enum Node {
    Envelope(ReadingKind),
    Activity,
    Summary,
    DailyBuckets,
    DailyBucket,
    Quota { authoritative_map: bool },
    LimitMap,
    Limit,
    Window,
    IndividualLimit,
    ResetCredits,
    CreditRows,
    CreditDetail,
    CreditStatus,
    Number,
    Boolean,
    Date,
    Opaque,
}

impl Node {
    fn sequence_item(self, index: usize) -> Option<Self> {
        // Serde also accepts structs as positional sequences. Keep that existing behavior.
        let fields: &[&str] = match self {
            Self::DailyBuckets => return Some(Self::DailyBucket),
            Self::CreditRows => return Some(Self::CreditDetail),
            Self::Envelope(_) => &["id", "result", "error"],
            Self::Activity => &["summary", "dailyUsageBuckets", "threadUsage"],
            Self::Summary => &[
                "lifetimeTokens",
                "peakDailyTokens",
                "longestRunningTurnSec",
                "currentStreakDays",
                "longestStreakDays",
            ],
            Self::DailyBucket => &["startDate", "tokens"],
            Self::Quota { .. } => &[
                "rateLimits",
                "rateLimitsByLimitId",
                "accountId",
                "rateLimitResetCredits",
            ],
            Self::Limit => &[
                "limitId",
                "limitName",
                "primary",
                "secondary",
                "rateLimitReachedType",
                "individualLimit",
                "spendControlReached",
            ],
            Self::Window => &["usedPercent", "windowDurationMins", "resetsAt"],
            Self::IndividualLimit => &["limit"],
            Self::ResetCredits => &["availableCount", "credits"],
            Self::CreditDetail => &["title", "status", "grantedAt", "expiresAt"],
            _ => return None,
        };
        fields
            .get(index)
            .and_then(|key| self.field(key))
            .map(|(_, node)| node)
    }

    fn field(self, key: &str) -> Option<(&'static str, Self)> {
        Some(match (self, key) {
            (Self::Envelope(_), "id") => ("id", Self::Opaque),
            (Self::Envelope(ReadingKind::Activity), "result") => ("result", Self::Activity),
            (Self::Envelope(ReadingKind::Quota { authoritative_map }), "result") => {
                ("result", Self::Quota { authoritative_map })
            }
            (Self::Envelope(_), "error") => ("error", Self::Opaque),
            (Self::Activity, "summary") => ("summary", Self::Summary),
            (Self::Activity, "dailyUsageBuckets") => ("dailyUsageBuckets", Self::DailyBuckets),
            (Self::Activity, "threadUsage") => ("threadUsage", Self::Opaque),
            (Self::Summary, "lifetimeTokens") => ("lifetimeTokens", Self::Number),
            (Self::Summary, "peakDailyTokens") => ("peakDailyTokens", Self::Number),
            (Self::Summary, "longestRunningTurnSec") => ("longestRunningTurnSec", Self::Number),
            (Self::Summary, "currentStreakDays") => ("currentStreakDays", Self::Number),
            (Self::Summary, "longestStreakDays") => ("longestStreakDays", Self::Number),
            (Self::DailyBucket, "startDate") => ("startDate", Self::Date),
            (Self::DailyBucket, "tokens") => ("tokens", Self::Number),
            (Self::Quota { authoritative_map }, "rateLimits") => (
                "rateLimits",
                if authoritative_map {
                    Self::Opaque
                } else {
                    Self::Limit
                },
            ),
            (Self::Quota { .. }, "rateLimitsByLimitId") => ("rateLimitsByLimitId", Self::LimitMap),
            (Self::Quota { .. }, "accountId") => ("accountId", Self::Opaque),
            (Self::Quota { .. }, "rateLimitResetCredits") => {
                ("rateLimitResetCredits", Self::ResetCredits)
            }
            (Self::Limit, "limitId") => ("limitId", Self::Opaque),
            (Self::Limit, "limitName") => ("limitName", Self::Opaque),
            (Self::Limit, "primary") => ("primary", Self::Window),
            (Self::Limit, "secondary") => ("secondary", Self::Window),
            (Self::Limit, "rateLimitReachedType") => ("rateLimitReachedType", Self::Opaque),
            (Self::Limit, "individualLimit") => ("individualLimit", Self::IndividualLimit),
            (Self::Limit, "spendControlReached") => ("spendControlReached", Self::Boolean),
            (Self::Window, "usedPercent") => ("usedPercent", Self::Number),
            (Self::Window, "windowDurationMins") => ("windowDurationMins", Self::Number),
            (Self::Window, "resetsAt") => ("resetsAt", Self::Number),
            (Self::IndividualLimit, "limit") => ("limit", Self::Opaque),
            (Self::ResetCredits, "availableCount") => ("availableCount", Self::Number),
            (Self::ResetCredits, "credits") => ("credits", Self::CreditRows),
            (Self::CreditDetail, "title") => ("title", Self::Opaque),
            (Self::CreditDetail, "status") => ("status", Self::CreditStatus),
            (Self::CreditDetail, "grantedAt") => ("grantedAt", Self::Number),
            (Self::CreditDetail, "expiresAt") => ("expiresAt", Self::Number),
            _ => return None,
        })
    }

    fn location(self) -> Option<EvidenceLocation> {
        match self {
            Self::Activity => Some(EvidenceLocation::ActivityResult),
            Self::Summary => Some(EvidenceLocation::ActivitySummary),
            Self::DailyBucket => Some(EvidenceLocation::ActivityDailyBucket),
            Self::Quota { .. } => Some(EvidenceLocation::QuotaResult),
            Self::Limit => Some(EvidenceLocation::QuotaLimit),
            Self::Window => Some(EvidenceLocation::QuotaWindow),
            Self::IndividualLimit => Some(EvidenceLocation::QuotaIndividualLimit),
            Self::ResetCredits => Some(EvidenceLocation::QuotaResetCredits),
            Self::CreditDetail => Some(EvidenceLocation::QuotaCreditDetail),
            _ => None,
        }
    }
}

struct Projection {
    redacted: Value,
    format: Value,
}

impl Projection {
    fn scalar(redacted: Value, kind: &'static str) -> Self {
        Self {
            redacted,
            format: json!(kind),
        }
    }

    fn opaque(kind: &'static str) -> Self {
        Self::scalar(json!({"redacted":kind}), kind)
    }
}

struct Project<'a> {
    node: Node,
    unused: &'a mut BTreeMap<EvidenceLocation, u32>,
}

impl<'de> DeserializeSeed<'de> for Project<'_> {
    type Value = Projection;

    fn deserialize<D: serde::Deserializer<'de>>(
        self,
        deserializer: D,
    ) -> Result<Projection, D::Error> {
        deserializer.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for Project<'_> {
    type Value = Projection;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("an accepted response")
    }

    fn visit_unit<E: serde::de::Error>(self) -> Result<Projection, E> {
        Ok(Projection::scalar(Value::Null, "null"))
    }

    fn visit_bool<E: serde::de::Error>(self, value: bool) -> Result<Projection, E> {
        Ok(if matches!(self.node, Node::Boolean) {
            Projection::scalar(json!(value), "boolean")
        } else {
            Projection::opaque("boolean")
        })
    }

    fn visit_i64<E: serde::de::Error>(self, value: i64) -> Result<Projection, E> {
        Ok(if matches!(self.node, Node::Number) {
            Projection::scalar(json!(value), "integer")
        } else {
            Projection::opaque("integer")
        })
    }

    fn visit_u64<E: serde::de::Error>(self, value: u64) -> Result<Projection, E> {
        Ok(if matches!(self.node, Node::Number) {
            Projection::scalar(json!(value), "integer")
        } else {
            Projection::opaque("integer")
        })
    }

    fn visit_f64<E: serde::de::Error>(self, _value: f64) -> Result<Projection, E> {
        Ok(Projection::opaque("number"))
    }

    fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Projection, E> {
        if matches!(self.node, Node::Date) {
            // The public parser already checked the canonical calendar date.
            let parts = value
                .split('-')
                .map(str::parse::<u16>)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| E::custom("invalid accepted date"))?;
            Ok(Projection::scalar(json!(parts), "string"))
        } else if matches!(self.node, Node::CreditStatus) {
            Ok(Projection::scalar(json!(credit_status(value)), "string"))
        } else {
            Ok(Projection::opaque("string"))
        }
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Projection, A::Error> {
        if matches!(self.node, Node::Opaque) {
            while map.next_entry::<IgnoredAny, IgnoredAny>()?.is_some() {}
            return Ok(Projection::opaque("object"));
        }
        if matches!(self.node, Node::CreditStatus) {
            // Serde also accepts a unit enum as a one-entry object.
            let mut redacted = Map::new();
            let mut format = Map::new();
            while let Some(variant) = map.next_key::<String>()? {
                let child = map.next_value_seed(Project {
                    node: Node::Opaque,
                    unused: self.unused,
                })?;
                redacted.insert(credit_status(&variant).to_owned(), child.redacted);
                format.insert("variant".to_owned(), child.format);
            }
            return Ok(Projection {
                redacted: Value::Object(redacted),
                format: Value::Object(format),
            });
        }
        if matches!(self.node, Node::LimitMap) {
            let mut redacted = vec![];
            let mut format = vec![];
            while map.next_key::<IgnoredAny>()?.is_some() {
                let child = map.next_value_seed(Project {
                    node: Node::Limit,
                    unused: self.unused,
                })?;
                redacted.push(child.redacted);
                format.push(child.format);
            }
            // Sorting anonymous content prevents private keys from affecting either hash.
            redacted.sort_by_cached_key(Value::to_string);
            format.sort_by_cached_key(Value::to_string);
            return Ok(Projection {
                redacted: json!({"entries":redacted}),
                format: json!({"entries":format}),
            });
        }
        let mut redacted = Map::new();
        let mut format = Map::new();
        while let Some(key) = map.next_key::<String>()? {
            if let Some((name, node)) = self.node.field(&key) {
                let child = map.next_value_seed(Project {
                    node,
                    unused: self.unused,
                })?;
                redacted.insert(name.to_owned(), child.redacted);
                format.insert(name.to_owned(), child.format);
            } else {
                map.next_value::<IgnoredAny>()?;
                if let Some(location) = self.node.location() {
                    // The response byte limit bounds this count below u32::MAX.
                    *self.unused.entry(location).or_default() += 1;
                }
            }
        }
        redacted.sort_keys();
        format.sort_keys();
        Ok(Projection {
            redacted: Value::Object(redacted),
            format: Value::Object(format),
        })
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Projection, A::Error> {
        if matches!(self.node, Node::Opaque) {
            while sequence.next_element::<IgnoredAny>()?.is_some() {}
            return Ok(Projection::opaque("array"));
        }
        let mut redacted = vec![];
        let mut format = vec![];
        while let Some(child) = sequence.next_element_seed(Project {
            node: self
                .node
                .sequence_item(redacted.len())
                .unwrap_or(Node::Opaque),
            unused: self.unused,
        })? {
            redacted.push(child.redacted);
            format.push(child.format);
        }
        Ok(Projection {
            redacted: Value::Array(redacted),
            format: Value::Array(format),
        })
    }
}

pub(crate) fn activity(bytes: &[u8]) -> Result<ResponseEvidence, ProtocolError> {
    evidence(bytes, ReadingKind::Activity)
}

pub(crate) fn quota(
    bytes: &[u8],
    authoritative_map: bool,
) -> Result<ResponseEvidence, ProtocolError> {
    evidence(bytes, ReadingKind::Quota { authoritative_map })
}

fn evidence(bytes: &[u8], kind: ReadingKind) -> Result<ResponseEvidence, ProtocolError> {
    let mut unused = BTreeMap::new();
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    let projection = Project {
        node: Node::Envelope(kind),
        unused: &mut unused,
    }
    .deserialize(&mut deserializer)
    .map_err(|_| ProtocolError::InvalidMessage)?;
    deserializer
        .end()
        .map_err(|_| ProtocolError::InvalidMessage)?;
    let unused_fields = unused
        .into_iter()
        .map(|(location, field_count)| UnusedFieldChange {
            location,
            field_count,
        })
        .collect::<Vec<_>>();
    let response_kind = match kind {
        ReadingKind::Activity => "account_usage",
        ReadingKind::Quota { .. } => "quota",
    };
    Ok(ResponseEvidence {
        projection_version: 1,
        redacted_response_hash: hash(json!({
            "projectionVersion":1,"responseKind":response_kind,"response":projection.redacted
        }))?,
        format_fingerprint: hash(json!({
            "projectionVersion":1,"responseKind":response_kind,"format":projection.format,
            "unusedFields":unused_fields
        }))?,
        unused_fields,
    })
}

fn credit_status(value: &str) -> &'static str {
    match value {
        "available" => "available",
        "redeeming" => "redeeming",
        "redeemed" => "redeemed",
        _ => "unknown",
    }
}

fn hash(mut value: Value) -> Result<String, ProtocolError> {
    // Keep the v1 encoding stable if another crate enables serde_json/preserve_order.
    value.sort_all_objects();
    let bytes = serde_json::to_vec(&value).map_err(|_| ProtocolError::InvalidMessage)?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}
