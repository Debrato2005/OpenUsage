use crate::Error;
use jiff::{Span, Timestamp, civil::Date, tz::TimeZone};
use serde::{Deserialize, Deserializer};
use std::{collections::BTreeSet, fmt};
use zeroize::Zeroizing;

pub(crate) const MAX_BODY: usize = 64 * 1024;
const MAX_COUNT: u64 = 1_000_000_000_000;
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

pub(crate) struct SecretText(pub Zeroizing<String>);
impl<'de> Deserialize<'de> for SecretText {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        String::deserialize(d).map(|v| Self(Zeroizing::new(v)))
    }
}
macro_rules! token {
    ($name:ident, $visibility:vis, $valid:expr) => {
        pub struct $name(pub(crate) Zeroizing<String>);
        impl $name {
            $visibility fn from_secret(value: Zeroizing<String>) -> Result<Self, Error> {
                if ($valid)(&value) {
                    Ok(Self(value))
                } else {
                    Err(Error::InvalidRequest)
                }
            }
            /// Deliberate access for the request and the caller's OS credential store.
            $visibility fn expose(&self) -> &str {
                &self.0
            }
        }
        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(concat!(stringify!($name), "([redacted])"))
            }
        }
    };
}
token!(GithubToken, pub(crate), |s: &str| (20..=255).contains(&s.len())
    && s.bytes()
        .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b)));
token!(ServiceToken, pub, |s: &str| s.len() == 64
    && s.bytes()
        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)));

impl<'de> Deserialize<'de> for ServiceToken {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let value = Zeroizing::new(String::deserialize(d)?);
        Self::from_secret(value).map_err(|_| serde::de::Error::custom("invalid secret field"))
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct DeviceId(String);
impl DeviceId {
    pub fn parse(value: &str) -> Result<Self, Error> {
        let valid = value.len() == 36
            && value.bytes().enumerate().all(|(i, b)| match i {
                8 | 13 | 18 | 23 => b == b'-',
                14 => b == b'4',
                19 => b"89ab".contains(&b),
                _ => b.is_ascii_digit() || (b'a'..=b'f').contains(&b),
            });
        if valid {
            Ok(Self(value.into()))
        } else {
            Err(Error::InvalidRequest)
        }
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl fmt::Debug for DeviceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DeviceId([redacted])")
    }
}
impl<'de> Deserialize<'de> for DeviceId {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Self::parse(&String::deserialize(d)?)
            .map_err(|_| serde::de::Error::custom("invalid device field"))
    }
}

pub struct Joined {
    pub login: String,
    pub token: ServiceToken,
    pub device_id: DeviceId,
}
impl fmt::Debug for Joined {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Joined([redacted])")
    }
}

/// Public departure acknowledgment. Login is descriptive metadata, never local identity proof.
pub struct AuthenticatedDeparture {
    pub(crate) login: String,
    pub(crate) local_token_revoked: Option<bool>,
}
impl AuthenticatedDeparture {
    pub fn login(&self) -> &str {
        &self.login
    }
    pub fn local_token_revoked(&self) -> Option<bool> {
        self.local_token_revoked
    }
}
impl fmt::Debug for AuthenticatedDeparture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AuthenticatedDeparture([redacted])")
    }
}

/// Validates the existing v1 schema and retains the original approved bytes unchanged.
/// This type does not build payloads or infer missing counts.
pub struct ApprovedSyncBody {
    bytes: Vec<u8>,
    device_id: DeviceId,
    rows: u16,
}
impl fmt::Debug for ApprovedSyncBody {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApprovedSyncBody")
            .field("bytes", &self.bytes.len())
            .field("rows", &self.rows)
            .finish()
    }
}
impl ApprovedSyncBody {
    pub fn from_json(bytes: Vec<u8>, now: Timestamp) -> Result<Self, Error> {
        if bytes.len() > MAX_BODY {
            return Err(Error::RequestTooLarge);
        }
        let body: SyncWire = serde_json::from_slice(&bytes).map_err(|_| Error::InvalidRequest)?;
        if body.rows.len() > 400 {
            return Err(Error::InvalidRequest);
        }
        let today = now.to_zoned(TimeZone::UTC).date();
        let earliest = today
            .checked_sub(Span::new().days(400))
            .map_err(|_| Error::InvalidRequest)?;
        let mut keys = BTreeSet::new();
        for row in &body.rows {
            let day = row
                .date
                .parse::<Date>()
                .map_err(|_| Error::InvalidRequest)?;
            let parts = [
                row.input_tokens,
                row.output_tokens,
                row.cache_read_tokens,
                row.cache_write_tokens,
            ];
            if day.to_string() != row.date
                || day < earliest
                || day > today
                || !matches!(
                    row.tool.as_str(),
                    "claude"
                        | "codex"
                        | "opencode"
                        | "amp"
                        | "droid"
                        | "codebuff"
                        | "hermes"
                        | "pi"
                        | "goose"
                        | "kilo"
                        | "copilot"
                        | "gemini"
                        | "antigravity"
                        | "kimi"
                        | "qwen"
                        | "openclaw"
                        | "grok"
                        | "zcode"
                )
                || !(1..=64).contains(&row.model.len())
                || !row
                    .model
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b".:-".contains(&b))
                || parts.iter().any(|n| *n > MAX_COUNT)
                || row.total_tokens > MAX_COUNT
                || parts.into_iter().try_fold(0u64, u64::checked_add) != Some(row.total_tokens)
                || row.estimated_value_micros.is_some_and(|n| n > MAX_COUNT)
                || !keys.insert((&row.date, &row.tool, &row.model))
            {
                return Err(Error::InvalidRequest);
            }
        }
        Ok(Self {
            bytes,
            device_id: body.device_id,
            rows: body.rows.len() as u16,
        })
    }
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn device_id(&self) -> &DeviceId {
        &self.device_id
    }
    pub fn row_count(&self) -> u16 {
        self.rows
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct SyncWire {
    device_id: DeviceId,
    rows: Vec<RowWire>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RowWire {
    date: String,
    tool: String,
    model: String,
    input_tokens: u64,
    output_tokens: u64,
    cache_read_tokens: u64,
    cache_write_tokens: u64,
    total_tokens: u64,
    #[serde(deserialize_with = "nullable")]
    estimated_value_micros: Option<u64>,
}
fn nullable<'de, D: Deserializer<'de>, T: Deserialize<'de>>(d: D) -> Result<Option<T>, D::Error> {
    Option::<T>::deserialize(d)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum Period {
    #[serde(rename = "7d")]
    SevenDays,
    #[serde(rename = "30d")]
    ThirtyDays,
    #[serde(rename = "all")]
    All,
}
impl Period {
    pub(crate) fn query(self) -> &'static str {
        match self {
            Self::SevenDays => "7d",
            Self::ThirtyDays => "30d",
            Self::All => "all",
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Metric {
    Tokens,
    Value,
}
impl Metric {
    pub(crate) fn query(self) -> &'static str {
        match self {
            Self::Tokens => "tokens",
            Self::Value => "value",
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RankEntry {
    pub rank: u64,
    pub login: String,
    pub total_tokens: u64,
    #[serde(deserialize_with = "nullable")]
    pub estimated_value_micros: Option<u64>,
}
impl fmt::Debug for RankEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RankEntry([redacted])")
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct OwnRank {
    pub title: String,
    pub period: Period,
    pub metric: Metric,
    pub snapshot_at: String,
    pub total_participants: u64,
    #[serde(deserialize_with = "nullable")]
    pub entry: Option<RankEntry>,
}
impl fmt::Debug for OwnRank {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("OwnRank([redacted])")
    }
}
pub(crate) fn login(value: &str) -> bool {
    (1..=39).contains(&value.len())
        && value
            .bytes()
            .next()
            .is_some_and(|b| b.is_ascii_alphanumeric())
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
}
impl OwnRank {
    pub(crate) fn validate(&self, period: Period, metric: Metric) -> Result<(), Error> {
        if self.title != "Self-reported"
            || self.period != period
            || self.metric != metric
            || self.snapshot_at.len() > 40
            || !self.snapshot_at.ends_with('Z')
            || self.snapshot_at.parse::<Timestamp>().is_err()
            || self.total_participants > MAX_SAFE_INTEGER
        {
            return Err(Error::UnexpectedResponse);
        }
        if let Some(entry) = &self.entry
            && (entry.rank == 0
                || entry.rank > self.total_participants
                || !login(&entry.login)
                || entry.total_tokens > MAX_SAFE_INTEGER
                || entry
                    .estimated_value_micros
                    .is_some_and(|n| n > MAX_SAFE_INTEGER))
        {
            return Err(Error::UnexpectedResponse);
        }
        Ok(())
    }
}
