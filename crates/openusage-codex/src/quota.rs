//! Pinned full-read quota diagnostics. No identity binding, credit mutation, or history acceptance.
//! https://github.com/openai/codex/blob/83086188ae0614bd882818ff1aa23f3194962012/codex-rs/app-server-protocol/src/protocol/v2/account.rs#L309-L354
//! https://github.com/openai/codex/blob/83086188ae0614bd882818ff1aa23f3194962012/codex-rs/app-server-protocol/src/protocol/v2/account.rs#L601-L740

use crate::{DecimalCount, ProtocolError, count, decode, validate_text};
use serde::{
    Deserialize, Serialize,
    de::{MapAccess, SeqAccess, Visitor},
};
use std::{collections::HashSet, fmt};

const MAX_LIMITS: usize = 64;
// A preview resource bound, not a provider maximum or a completeness claim.
const MAX_CREDIT_DETAILS: usize = 32;
const MAX_SAFE_INTEGER: i64 = 9_007_199_254_740_991;
const FIRST_UTC_SECOND: i64 = -62_135_596_800;
const LAST_UTC_SECOND: i64 = 253_402_300_799;

/// Only reviewed display fields leave this parser. No identifiers, email, or raw frames.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaSnapshot {
    pub limits: Vec<QuotaLimit>,
    pub available_reset_credits: Option<DecimalCount>,
    pub reset_credit_details: Option<Vec<ResetCreditDetail>>,
}

impl fmt::Debug for QuotaSnapshot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("QuotaSnapshot")
            .field("limit_count", &self.limits.len())
            .finish()
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaLimit {
    pub limit_id: String,
    pub limit_name: Option<String>,
    pub primary: Option<QuotaWindow>,
    pub secondary: Option<QuotaWindow>,
    pub reached_type: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaWindow {
    pub used_percent: u8,
    pub derived_remaining_percent: u8,
    pub window_duration_mins: Option<u64>,
    pub resets_at_unix: Option<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FullRead {
    // Legacy values are not interpreted when the authoritative multi-limit map exists.
    rate_limits: Option<serde_json::Value>,
    rate_limits_by_limit_id: Option<LimitMap>,
    account_id: Option<String>,
    rate_limit_reset_credits: Option<ResetCreditCountWire>,
}

// Detail rows may be absent or capped; only the summary count is authoritative.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResetCreditCountWire {
    available_count: i64,
    credits: Option<CreditRows>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResetCreditDetail {
    pub title: Option<String>,
    pub status: ResetCreditStatus,
    pub granted_at_unix: i64,
    pub expires_at_unix: Option<i64>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ResetCreditStatus {
    Available,
    Redeeming,
    Redeemed,
    #[serde(other)]
    Unknown,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreditWire {
    title: Option<String>,
    status: ResetCreditStatus,
    granted_at: i64,
    // Unlike a missing field, an explicit null means that this credit has no expiry.
    #[serde(deserialize_with = "required_expiry")]
    expires_at: Option<i64>,
}

fn required_expiry<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<i64>, D::Error> {
    Option::<i64>::deserialize(deserializer)
}

struct CreditRows(Vec<CreditWire>);
impl<'de> Deserialize<'de> for CreditRows {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Rows;
        impl<'de> Visitor<'de> for Rows {
            type Value = CreditRows;
            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a bounded list of reset-credit details")
            }
            fn visit_seq<A: SeqAccess<'de>>(
                self,
                mut sequence: A,
            ) -> Result<Self::Value, A::Error> {
                let mut rows = vec![];
                while let Some(row) = sequence.next_element()? {
                    if rows.len() == MAX_CREDIT_DETAILS {
                        return Err(serde::de::Error::custom("too many reset-credit details"));
                    }
                    rows.push(row);
                }
                Ok(CreditRows(rows))
            }
        }
        deserializer.deserialize_seq(Rows)
    }
}

impl CreditWire {
    fn validate(self) -> Result<ResetCreditDetail, ProtocolError> {
        if let Some(title) = &self.title {
            validate_text(title, 256)?;
        }
        if !(FIRST_UTC_SECOND..=LAST_UTC_SECOND).contains(&self.granted_at)
            || self
                .expires_at
                .is_some_and(|expiry| !(FIRST_UTC_SECOND..=LAST_UTC_SECOND).contains(&expiry))
        {
            return Err(ProtocolError::InvalidField);
        }
        Ok(ResetCreditDetail {
            title: self.title,
            status: self.status,
            granted_at_unix: self.granted_at,
            expires_at_unix: self.expires_at,
        })
    }
}

struct LimitMap(Vec<(String, LimitWire)>);
impl<'de> Deserialize<'de> for LimitMap {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Limits;
        impl<'de> Visitor<'de> for Limits {
            type Value = LimitMap;
            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a bounded map of distinct quota identifiers")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut entries = vec![];
                let mut seen = HashSet::new();
                while let Some(key) = map.next_key::<String>()? {
                    if entries.len() == MAX_LIMITS
                        || validate_text(&key, 128).is_err()
                        || !seen.insert(key.clone())
                    {
                        return Err(serde::de::Error::custom("invalid quota map"));
                    }
                    entries.push((key, map.next_value()?));
                }
                Ok(LimitMap(entries))
            }
        }
        deserializer.deserialize_map(Limits)
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LimitWire {
    limit_id: Option<String>,
    limit_name: Option<String>,
    primary: Option<WindowWire>,
    secondary: Option<WindowWire>,
    rate_limit_reached_type: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WindowWire {
    used_percent: i32,
    window_duration_mins: Option<i64>,
    resets_at: Option<i64>,
}

/// Parse a complete, matching read response. Never use this function to merge sparse events.
pub fn parse_quota(bytes: &[u8], id: u32) -> Result<QuotaSnapshot, ProtocolError> {
    let response: FullRead = decode(bytes, id)?;
    if let Some(account_id) = response.account_id {
        validate_text(&account_id, 256)?;
        // It cannot be matched to a stable baseline account/read key. Do not publish or bind it.
    }
    let limits = if let Some(LimitMap(entries)) = response.rate_limits_by_limit_id {
        entries
            .into_iter()
            .map(|(key, value)| value.validate(Some(key)))
            .collect::<Result<Vec<_>, _>>()?
    } else {
        let value = response.rate_limits.ok_or(ProtocolError::InvalidField)?;
        let value: LimitWire =
            serde_json::from_value(value).map_err(|_| ProtocolError::InvalidField)?;
        vec![value.validate(None)?]
    };
    let (available_reset_credits, reset_credit_details) = match response.rate_limit_reset_credits {
        Some(summary) => (
            Some(count(summary.available_count)?),
            summary
                .credits
                .map(|CreditRows(rows)| {
                    rows.into_iter()
                        .map(CreditWire::validate)
                        .collect::<Result<Vec<_>, _>>()
                })
                .transpose()?,
        ),
        None => (None, None),
    };
    Ok(QuotaSnapshot {
        limits,
        available_reset_credits,
        reset_credit_details,
    })
}

impl LimitWire {
    fn validate(self, map_key: Option<String>) -> Result<QuotaLimit, ProtocolError> {
        if let Some(id) = &self.limit_id {
            validate_text(id, 128)?;
        }
        let limit_id = match (map_key, self.limit_id) {
            (Some(key), Some(id)) if key != id => {
                return Err(ProtocolError::IncompatibleQuotaScope);
            }
            (Some(key), _) => key,
            (None, Some(id)) => id,
            (None, None) => return Err(ProtocolError::IncompatibleQuotaScope),
        };
        if let Some(name) = &self.limit_name {
            validate_text(name, 256)?;
        }
        if let Some(reached) = &self.rate_limit_reached_type {
            validate_text(reached, 128)?;
        }
        Ok(QuotaLimit {
            limit_id,
            limit_name: self.limit_name,
            primary: self.primary.map(WindowWire::validate).transpose()?,
            secondary: self.secondary.map(WindowWire::validate).transpose()?,
            reached_type: self.rate_limit_reached_type,
        })
    }
}

impl WindowWire {
    fn validate(self) -> Result<QuotaWindow, ProtocolError> {
        if !(0..=100).contains(&self.used_percent)
            || self
                .window_duration_mins
                .is_some_and(|duration| !(1..=MAX_SAFE_INTEGER).contains(&duration))
            || self
                .resets_at
                .is_some_and(|reset| !(FIRST_UTC_SECOND..=LAST_UTC_SECOND).contains(&reset))
        {
            return Err(ProtocolError::InvalidField);
        }
        let used_percent = self.used_percent as u8;
        Ok(QuotaWindow {
            used_percent,
            derived_remaining_percent: 100 - used_percent,
            window_duration_mins: self.window_duration_mins.map(|value| value as u64),
            resets_at_unix: self.resets_at,
        })
    }
}
