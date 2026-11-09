//! Read projections for consumers. Grouped totals, unplaced usage and legacy history stay separate.
use crate::{
    LocalLogError, LocalScanDifference, LocalToolScope, LocalUsageDay, LocalUsageDifference,
    LocalUsageKey, LogSource, SavedGroupedSession, SavedLocalPeriod, Storage,
};
use jiff::{Span, civil::Date, tz::TimeZone};
use openusage_locallogs::{AgentPeriod, ModelDetail, TokenDetails, UsagePeriod, tools};
use std::collections::{BTreeMap, BTreeSet};

type Result<T> = std::result::Result<T, LocalLogError>;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LocalTokenDetails {
    pub input_tokens: Option<u128>,
    pub output_tokens: Option<u128>,
    pub cache_write_tokens: Option<u128>,
    pub cache_read_tokens: Option<u128>,
    pub reasoning_tokens: Option<u128>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalModelTotal {
    pub model: String,
    pub total_tokens: Option<u128>,
    pub details: LocalTokenDetails,
    pub estimated_value_micros: Option<u128>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalPeriodTotal {
    pub total_tokens: u128,
    pub details: LocalTokenDetails,
    pub estimated_value_micros: Option<u128>,
    pub models: Vec<LocalModelTotal>,
    pub model_totals_complete: bool,
}

/// Each dated scope uses its own pinned day boundaries. There is no inferred common zone.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalUsageWindow {
    pub start_utc: String,
    pub end_utc: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GroupedUsageRead {
    pub scope: LocalToolScope,
    pub window: Option<LocalUsageWindow>,
    pub dated: Vec<SavedLocalPeriod>,
    pub unplaced: Option<SavedLocalPeriod>,
    /// None when the selected dates have no saved period. An explicit zero remains Some(0).
    pub dated_total: Option<LocalPeriodTotal>,
    pub unplaced_total: Option<LocalPeriodTotal>,
    pub dated_sessions: Vec<SavedGroupedSession>,
    /// Outside every date filter. No observation time substitutes for an absent activity time.
    pub unplaced_sessions: Vec<SavedGroupedSession>,
    pub dated_differences: Vec<LocalScanDifference>,
    pub unplaced_differences: Vec<LocalScanDifference>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LegacyUsageRead {
    pub source: LogSource,
    pub days: Vec<LocalUsageDay>,
    pub differences: Vec<LocalUsageDifference>,
}

/// No combined total crosses these collections. Legacy model maxima are not new period totals.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalUsageRead {
    pub from_date: String,
    pub to_date: String,
    pub grouped: Vec<GroupedUsageRead>,
    pub legacy: Vec<LegacyUsageRead>,
}

impl LocalTokenDetails {
    fn from_counts(value: &TokenDetails) -> Self {
        Self {
            input_tokens: value.input_tokens.map(u128::from),
            output_tokens: value.output_tokens.map(u128::from),
            cache_write_tokens: value.cache_write_tokens.map(u128::from),
            cache_read_tokens: value.cache_read_tokens.map(u128::from),
            reasoning_tokens: value.reasoning_tokens.map(u128::from),
        }
    }
    fn add(&mut self, value: &TokenDetails) {
        let add = |a: Option<u128>, b: Option<u64>| a.zip(b).map(|(a, b)| a + u128::from(b));
        self.input_tokens = add(self.input_tokens, value.input_tokens);
        self.output_tokens = add(self.output_tokens, value.output_tokens);
        self.cache_write_tokens = add(self.cache_write_tokens, value.cache_write_tokens);
        self.cache_read_tokens = add(self.cache_read_tokens, value.cache_read_tokens);
        self.reasoning_tokens = add(self.reasoning_tokens, value.reasoning_tokens);
    }
}

fn model_total(model: &ModelDetail) -> LocalModelTotal {
    LocalModelTotal {
        model: model.model.clone(),
        total_tokens: model.total_tokens.map(u128::from),
        details: LocalTokenDetails::from_counts(&model.details),
        estimated_value_micros: model.estimated_value_micros.map(u128::from),
    }
}

/// Sums independent period totals, never sessions or model subtotals. Missing details stay missing.
fn total(periods: &[&AgentPeriod]) -> Option<LocalPeriodTotal> {
    let first = periods.first()?;
    let mut details = LocalTokenDetails::from_counts(&first.details);
    let mut estimate = first.estimated_value_micros.map(u128::from);
    let mut tokens = 0;
    let mut models: BTreeMap<String, LocalModelTotal> = BTreeMap::new();
    let mut complete = true;
    for (index, period) in periods.iter().enumerate() {
        tokens += u128::from(period.total_tokens);
        if index != 0 {
            details.add(&period.details);
            estimate = estimate
                .zip(period.estimated_value_micros)
                .map(|(a, b)| a + u128::from(b));
        }
        complete &= period
            .models
            .iter()
            .map(|m| m.total_tokens.map(u128::from))
            .sum::<Option<u128>>()
            == Some(u128::from(period.total_tokens));
        for model in &period.models {
            match models.get_mut(&model.model) {
                None => {
                    models.insert(model.model.clone(), model_total(model));
                }
                Some(saved) => {
                    saved.total_tokens = saved
                        .total_tokens
                        .zip(model.total_tokens)
                        .map(|(a, b)| a + u128::from(b));
                    saved.details.add(&model.details);
                    saved.estimated_value_micros = saved
                        .estimated_value_micros
                        .zip(model.estimated_value_micros)
                        .map(|(a, b)| a + u128::from(b));
                }
            }
        }
    }
    if !complete {
        for model in models.values_mut() {
            model.total_tokens = None;
            model.details = LocalTokenDetails::default();
            model.estimated_value_micros = None;
        }
    }
    Some(LocalPeriodTotal {
        total_tokens: tokens,
        details,
        estimated_value_micros: estimate,
        models: models.into_values().collect(),
        model_totals_complete: complete,
    })
}

fn window(from: Date, to: Date, zone: &str) -> Result<LocalUsageWindow> {
    let zone = TimeZone::get(zone).map_err(|_| LocalLogError::ReadFailed)?;
    let timestamp = |day: Date| {
        day.to_zoned(zone.clone())
            .map(|v| v.timestamp().strftime("%Y-%m-%dT%H:%M:%S.000Z").to_string())
            .map_err(|_| LocalLogError::ReadFailed)
    };
    let next = to
        .checked_add(Span::new().days(1))
        .map_err(|_| LocalLogError::InvalidInput)?;
    Ok(LocalUsageWindow {
        start_utc: timestamp(from)?,
        end_utc: timestamp(next)?,
    })
}

impl Storage {
    /// One read transaction for at most 366 dated days, plus explicitly separate unplaced history.
    /// This API does not run a scanner or mutate the database, including on read-only connections.
    pub fn local_usage_read(&self, from: &str, to: &str) -> Result<LocalUsageRead> {
        if !crate::activity::activity_date(from) || !crate::activity::activity_date(to) {
            return Err(LocalLogError::InvalidInput);
        }
        let start: Date = from.parse().map_err(|_| LocalLogError::InvalidInput)?;
        let end: Date = to.parse().map_err(|_| LocalLogError::InvalidInput)?;
        if start > end
            || start
                .until(end)
                .map_err(|_| LocalLogError::InvalidInput)?
                .get_days()
                >= 366
        {
            return Err(LocalLogError::InvalidInput);
        }
        let transaction = self
            .connection
            .unchecked_transaction()
            .map_err(|_| LocalLogError::ReadFailed)?;
        let mut grouped = Vec::new();
        for definition in tools() {
            let Some(scope) = self.local_tool_scope(&definition.id)? else {
                continue;
            };
            let window = scope
                .time_zone
                .as_deref()
                .map(|zone| window(start, end, zone))
                .transpose()?;
            let mut dated = Vec::new();
            let mut unplaced = None;
            for saved in self.local_periods(&definition.id)? {
                match &saved.period.period {
                    UsagePeriod::Day(day) if day.as_str() >= from && day.as_str() <= to => {
                        dated.push(saved)
                    }
                    UsagePeriod::Unplaced => unplaced = Some(saved),
                    _ => {}
                }
            }
            let dated_total = total(&dated.iter().map(|s| &s.period).collect::<Vec<_>>());
            let unplaced_total = total(&unplaced.iter().map(|s| &s.period).collect::<Vec<_>>());
            let mut dated_sessions = Vec::new();
            let mut unplaced_sessions = Vec::new();
            for saved in self.grouped_local_sessions(&definition.id)? {
                match (&window, &saved.session.last_activity_utc) {
                    (Some(window), Some(last))
                        if last >= &window.start_utc && last < &window.end_utc =>
                    {
                        dated_sessions.push(saved)
                    }
                    (None, _) | (_, None) => unplaced_sessions.push(saved),
                    _ => {}
                }
            }
            dated_sessions.sort_by(|a, b| {
                b.session
                    .last_activity_utc
                    .cmp(&a.session.last_activity_utc)
                    .then(a.session.session_key.cmp(&b.session.session_key))
            });
            let mut dated_differences = Vec::new();
            let mut unplaced_differences = Vec::new();
            let dated_keys: BTreeSet<_> = dated_sessions
                .iter()
                .map(|s| &s.session.session_key)
                .collect();
            let unplaced_keys: BTreeSet<_> = unplaced_sessions
                .iter()
                .map(|s| &s.session.session_key)
                .collect();
            for difference in self.local_scan_differences(&definition.id)? {
                match &difference.key {
                    LocalUsageKey::Day(day) if day.as_str() >= from && day.as_str() <= to => {
                        dated_differences.push(difference)
                    }
                    LocalUsageKey::Unplaced => unplaced_differences.push(difference),
                    LocalUsageKey::Session(key) if dated_keys.contains(key) => {
                        dated_differences.push(difference)
                    }
                    LocalUsageKey::Session(key) if unplaced_keys.contains(key) => {
                        unplaced_differences.push(difference)
                    }
                    _ => {}
                }
            }
            grouped.push(GroupedUsageRead {
                scope,
                window,
                dated,
                unplaced,
                dated_total,
                unplaced_total,
                dated_sessions,
                unplaced_sessions,
                dated_differences,
                unplaced_differences,
            });
        }
        let days = self.local_usage_days(from, to)?;
        let differences = self.local_usage_differences(from, to)?;
        let legacy = self
            .log_sources()?
            .into_iter()
            .filter_map(|source| {
                let days: Vec<_> = days
                    .iter()
                    .filter(|d| d.log_source_id == source.id)
                    .cloned()
                    .collect();
                let differences: Vec<_> = differences
                    .iter()
                    .filter(|d| d.log_source_id == source.id)
                    .cloned()
                    .collect();
                (!days.is_empty() || !differences.is_empty()).then_some(LegacyUsageRead {
                    source,
                    days,
                    differences,
                })
            })
            .collect();
        transaction
            .commit()
            .map_err(|_| LocalLogError::ReadFailed)?;
        Ok(LocalUsageRead {
            from_date: from.into(),
            to_date: to.into(),
            grouped,
            legacy,
        })
    }
}

#[cfg(test)]
mod tests;
