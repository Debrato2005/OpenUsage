//! Codex candidate reports retain independent totals and unavailable attribution.
//! This parser does not activate a scanner or change stored history.

use crate::agent_report::{Field, add, inconsistent, shape};
use crate::agent_snapshot::Snapshot;
use crate::report::{MAX_ROWS, Sum, counts, micros, valid_date, valid_model};
use crate::session::{CodexModel, valid_key, valid_utc};
use crate::{
    AgentPeriod, AgentSession, GroupedScanResult, ModelDetail, ReportError, TokenDetails,
    UsagePeriod,
};
use serde::de::{MapAccess, Visitor};
use serde::{Deserialize, Deserializer};
use serde_json::value::RawValue;
use std::collections::{BTreeMap, BTreeSet};

macro_rules! report_row {
    ($name:ident { $($(#[$attrs:meta])* $field:ident: $ty:ty,)* }) => {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields, rename_all = "camelCase")]
        struct $name {
            input_tokens: u64,
            output_tokens: u64,
            cache_creation_tokens: u64,
            cache_read_tokens: u64,
            reasoning_output_tokens: u64,
            total_tokens: u64,
            #[serde(default, rename = "costUSD")]
            cost_usd: Field<Option<Box<RawValue>>>,
            $($(#[$attrs])* $field: $ty,)*
        }
        impl $name {
            fn reading(&self) -> Result<Reading, ReportError> {
                let sum = counts(self.input_tokens, self.output_tokens,
                    self.cache_creation_tokens, self.cache_read_tokens)?;
                if sum.total()? != self.total_tokens || self.reasoning_output_tokens > sum.output {
                    return Err(inconsistent("Codex counts disagree"));
                }
                Ok(Reading { sum, reasoning: self.reasoning_output_tokens })
            }
        }
    };
}

report_row!(Day {
    date: String,
    #[serde(deserialize_with = "unique_models")]
    models: BTreeMap<String, CodexModel>,
});
report_row!(Session {
    directory: String,
    last_activity: String,
    session_file: String,
    session_id: String,
    #[serde(deserialize_with = "unique_models")]
    models: BTreeMap<String, CodexModel>,
});
report_row!(Totals {
    #[serde(default)]
    unpriced_models: Vec<String>,
});

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DailyReport {
    daily: Vec<Day>,
    totals: Totals,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SessionReport {
    sessions: Vec<Session>,
    totals: Totals,
}

#[derive(Default, PartialEq, Eq, Clone, Copy)]
struct Reading {
    sum: Sum,
    reasoning: u64,
}
impl Reading {
    fn details(&self) -> TokenDetails {
        TokenDetails {
            input_tokens: Some(self.sum.input),
            output_tokens: Some(self.sum.output),
            cache_write_tokens: Some(self.sum.write),
            cache_read_tokens: Some(self.sum.read),
            reasoning_tokens: Some(self.reasoning),
        }
    }
}

/// Parse both Codex reports from one candidate capture.
pub fn parse_codex_snapshot(bytes: &[u8]) -> Result<GroupedScanResult, ReportError> {
    let snapshot: Snapshot<'_> =
        serde_json::from_slice(bytes).map_err(|_| shape("Invalid Codex snapshot envelope"))?;
    if snapshot.schema_version != 1 || snapshot.tool != "codex" {
        return Err(shape("Unsupported Codex snapshot envelope"));
    }
    let daily: DailyReport =
        serde_json::from_str(snapshot.daily.get()).map_err(|_| shape("Invalid Codex report"))?;
    let session: SessionReport =
        serde_json::from_str(snapshot.session.get()).map_err(|_| shape("Invalid Codex report"))?;
    if daily.daily.len() > MAX_ROWS || session.sessions.len() > MAX_ROWS {
        return Err(shape("Too many Codex rows"));
    }
    let mut periods = Vec::new();
    let mut sessions = Vec::new();
    let mut day_check = ReportCheck::default();
    let mut session_check = ReportCheck::default();
    let mut previous: Option<String> = None;
    for day in daily.daily {
        if !valid_date(&day.date) || previous.as_ref().is_some_and(|last| *last >= day.date) {
            return Err(shape("Invalid Codex day"));
        }
        let reading = day.reading()?;
        let (models, estimate) = day_check.row(reading, &day.models, &day.cost_usd)?;
        previous = Some(day.date.clone());
        periods.push(AgentPeriod {
            details: reading.details(),
            period: UsagePeriod::Day(day.date),
            total_tokens: day.total_tokens,
            estimated_value_micros: estimate,
            models,
        });
    }
    previous = None;
    for row in session.sessions {
        let (directory, file) = row
            .session_id
            .rsplit_once('/')
            .unwrap_or(("", &row.session_id));
        if !valid_key(&row.session_id)
            || !valid_key(&row.session_file)
            || row.directory != directory
            || row.session_file != file
            || !valid_utc(&row.last_activity)
            || previous
                .as_ref()
                .is_some_and(|last| *last >= row.session_id)
        {
            return Err(shape("Invalid Codex session"));
        }
        let reading = row.reading()?;
        let (models, estimate) = session_check.row(reading, &row.models, &row.cost_usd)?;
        previous = Some(row.session_id.clone());
        sessions.push(AgentSession {
            details: reading.details(),
            session_key: row.session_id,
            project: None,
            first_activity_utc: None,
            last_activity_utc: Some(row.last_activity),
            total_tokens: row.total_tokens,
            estimated_value_micros: estimate,
            models,
        });
    }
    day_check.finish(&daily.totals)?;
    session_check.finish(&session.totals)?;
    // Equivalent sums can round to adjacent microdollars. Each section must also pass its own row-cost checks.
    let costs_agree = match (
        cost(&daily.totals.cost_usd)?,
        cost(&session.totals.cost_usd)?,
    ) {
        (Some(daily), Some(session)) => daily.abs_diff(session) <= 1,
        (None, None) => true,
        _ => false,
    };
    if periods.is_empty() != sessions.is_empty()
        || day_check.reading != session_check.reading
        || day_check.models != session_check.models
        || day_check.unpriced != session_check.unpriced
        || !costs_agree
    {
        return Err(inconsistent("Codex snapshot reports disagree"));
    }
    Ok(GroupedScanResult { periods, sessions })
}

fn cost(value: &Field<Option<Box<RawValue>>>) -> Result<Option<u64>, ReportError> {
    match value {
        Field::Missing => Err(shape("Missing Codex cost")),
        Field::Present(value) => value
            .as_ref()
            .map(|raw| {
                raw.get()
                    .parse::<f64>()
                    .map_err(|_| shape("Invalid Codex report"))
                    .and_then(micros)
            })
            .transpose(),
    }
}

impl Reading {
    fn add(&mut self, other: Self) -> Result<(), ReportError> {
        self.sum.add(other.sum)?;
        self.sum.total()?;
        self.reasoning = add(self.reasoning, other.reasoning)?;
        Ok(())
    }

    fn contains(&self, other: &Self) -> bool {
        other.sum.input <= self.sum.input
            && other.sum.output <= self.sum.output
            && other.sum.write <= self.sum.write
            && other.sum.read <= self.sum.read
            && other.reasoning <= self.reasoning
            && other.sum.output - other.reasoning <= self.sum.output - self.reasoning
    }
}

struct ReportCheck {
    reading: Reading,
    models: BTreeMap<String, Reading>,
    unpriced: BTreeSet<String>,
    cost_sum: Option<u64>,
    rows: usize,
    model_rows: usize,
}
impl Default for ReportCheck {
    fn default() -> Self {
        Self {
            reading: Reading::default(),
            models: BTreeMap::new(),
            unpriced: BTreeSet::new(),
            cost_sum: Some(0),
            rows: 0,
            model_rows: 0,
        }
    }
}
impl ReportCheck {
    fn row(
        &mut self,
        reading: Reading,
        models: &BTreeMap<String, CodexModel>,
        raw_cost: &Field<Option<Box<RawValue>>>,
    ) -> Result<(Vec<ModelDetail>, Option<u64>), ReportError> {
        self.model_rows += models.len();
        if self.model_rows > MAX_ROWS {
            return Err(shape("Too many Codex models"));
        }
        let mut subtotal = Reading::default();
        let mut details = Vec::new();
        let mut priced = true;
        for (name, model) in models {
            if !valid_model(name) {
                return Err(shape("Invalid Codex model"));
            }
            let part = Reading {
                sum: counts(
                    model.input_tokens,
                    model.output_tokens,
                    model.cache_creation_tokens,
                    model.cache_read_tokens,
                )?,
                reasoning: model.reasoning_output_tokens,
            };
            if part.sum.total()? != model.total_tokens || part.reasoning > part.sum.output {
                return Err(inconsistent("Codex counts disagree"));
            }
            subtotal.add(part)?;
            self.models.entry(name.clone()).or_default().add(part)?;
            if model.missing_pricing {
                priced = false;
                self.unpriced.insert(name.clone());
            }
            details.push(ModelDetail {
                model: name.clone(),
                total_tokens: Some(model.total_tokens),
                details: part.details(),
                // The report does not give per-model costs.
                estimated_value_micros: None,
            });
        }
        if !reading.contains(&subtotal) {
            return Err(inconsistent("Codex models exceed their row"));
        }
        let complete = subtotal == reading;
        if !complete {
            for model in &mut details {
                model.total_tokens = None;
            }
        }
        let value = cost(raw_cost)?;
        if value.is_some() && (!complete || models.is_empty()) {
            return Err(inconsistent("Codex costs disagree"));
        }
        self.reading.add(reading)?;
        self.rows += 1;
        self.cost_sum = match (self.cost_sum, value) {
            (Some(a), Some(b)) => Some(add(a, b)?),
            _ => None,
        };
        Ok((details, value.filter(|_| priced && complete)))
    }

    fn finish(&self, totals: &Totals) -> Result<(), ReportError> {
        if self.reading != totals.reading()? {
            return Err(inconsistent("Codex report totals disagree"));
        }
        let mut listed = BTreeSet::new();
        for name in &totals.unpriced_models {
            if !valid_model(name) || !listed.insert(name.clone()) {
                return Err(shape("Invalid unpriced models"));
            }
        }
        if listed != self.unpriced {
            return Err(inconsistent("Codex report totals disagree"));
        }
        // Each row rounds once to microdollars. The total can differ by at most half the row count.
        match (self.cost_sum, cost(&totals.cost_usd)?) {
            (Some(sum), Some(total)) if sum.abs_diff(total) <= self.rows as u64 / 2 => Ok(()),
            (None, None) => Ok(()),
            _ => Err(inconsistent("Codex costs disagree")),
        }
    }
}

fn unique_models<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<BTreeMap<String, CodexModel>, D::Error> {
    struct Models;
    impl<'de> Visitor<'de> for Models {
        type Value = BTreeMap<String, CodexModel>;
        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("Invalid Codex model")
        }
        fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
            let mut models = BTreeMap::new();
            while let Some((name, model)) = map.next_entry::<String, CodexModel>()? {
                if models.insert(name, model).is_some() {
                    return Err(serde::de::Error::custom("Duplicate Codex model"));
                }
                if models.len() > MAX_ROWS {
                    return Err(serde::de::Error::custom("Too many Codex models"));
                }
            }
            Ok(models)
        }
    }
    deserializer.deserialize_map(Models)
}
