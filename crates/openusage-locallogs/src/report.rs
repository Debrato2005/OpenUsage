//! Strict parser for `ccusage daily --json --by-agent` from the pinned version (OU-LOG-007).
//! An unknown field, a wrong type or totals that do not add up reject the whole scan.

use serde::Deserialize;
use std::collections::BTreeSet;

/// Tools that the pinned ccusage reports. Another value means an unknown shape.
pub const TOOLS: &[&str] = &[
    "claude",
    "codex",
    "opencode",
    "amp",
    "droid",
    "codebuff",
    "hermes",
    "pi",
    "goose",
    "kilo",
    "copilot",
    "gemini",
    "antigravity",
    "kimi",
    "qwen",
    "openclaw",
    "grok",
    "zcode",
];

/// A bound well above any real day, so a corrupt count cannot overflow later sums.
const MAX_COUNT: u64 = 1_000_000_000_000_000;
const MAX_ROWS: usize = 200_000;

/// One day of one tool and one model. Token counts are exact integers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageRow {
    pub date: String,
    pub tool: &'static str,
    pub model: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_write_tokens: u64,
    pub cache_read_tokens: u64,
    pub total_tokens: u64,
    /// Estimated API-equivalent value in millionths of a US dollar. None when ccusage has no price for the model (OU-LOG-013).
    pub estimated_value_micros: Option<u64>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ReportError {
    /// The output is not the shape the pinned version produces.
    Shape(String),
    /// The output parsed, but its numbers do not agree with each other.
    Inconsistent(String),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Report {
    daily: Vec<Day>,
    totals: Counts,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Counts {
    cache_creation_tokens: u64,
    cache_read_tokens: u64,
    input_tokens: u64,
    output_tokens: u64,
    total_cost: f64,
    total_tokens: u64,
    /// Present only when some model has no price.
    #[serde(default)]
    unpriced_models: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Day {
    agent: String,
    agents: Vec<Agent>,
    cache_creation_tokens: u64,
    cache_read_tokens: u64,
    input_tokens: u64,
    metadata: Metadata,
    model_breakdowns: Vec<Model>,
    models_used: Vec<String>,
    output_tokens: u64,
    period: String,
    total_cost: f64,
    total_tokens: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Metadata {
    agents: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Agent {
    agent: String,
    cache_creation_tokens: u64,
    cache_read_tokens: u64,
    input_tokens: u64,
    model_breakdowns: Vec<Model>,
    models_used: Vec<String>,
    output_tokens: u64,
    total_cost: f64,
    total_tokens: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Model {
    cache_creation_tokens: u64,
    cache_read_tokens: u64,
    cost: f64,
    input_tokens: u64,
    #[serde(default)]
    missing_pricing: bool,
    model_name: String,
    output_tokens: u64,
}

#[derive(Default, PartialEq, Eq, Debug, Clone, Copy)]
struct Sum {
    input: u64,
    output: u64,
    write: u64,
    read: u64,
}

impl Sum {
    fn add(&mut self, other: Sum) -> Result<(), ReportError> {
        let add = |a: u64, b: u64| {
            a.checked_add(b)
                .filter(|v| *v <= MAX_COUNT)
                .ok_or_else(|| ReportError::Inconsistent("count out of range".into()))
        };
        self.input = add(self.input, other.input)?;
        self.output = add(self.output, other.output)?;
        self.write = add(self.write, other.write)?;
        self.read = add(self.read, other.read)?;
        Ok(())
    }
    fn total(self) -> Result<u64, ReportError> {
        let sum = [self.output, self.write, self.read]
            .into_iter()
            .try_fold(self.input, |a, b| a.checked_add(b));
        sum.filter(|v| *v <= MAX_COUNT)
            .ok_or_else(|| ReportError::Inconsistent("total out of range".into()))
    }
}

fn counts(input: u64, output: u64, write: u64, read: u64) -> Result<Sum, ReportError> {
    if [input, output, write, read].iter().any(|v| *v > MAX_COUNT) {
        return Err(ReportError::Inconsistent("count out of range".into()));
    }
    Ok(Sum {
        input,
        output,
        write,
        read,
    })
}

fn valid_date(date: &str) -> bool {
    let bytes = date.as_bytes();
    if bytes.len() != 10
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || !date
            .chars()
            .enumerate()
            .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit())
    {
        return false;
    }
    let (month, day) = (
        date[5..7].parse::<u8>().unwrap_or(0),
        date[8..10].parse::<u8>().unwrap_or(0),
    );
    (1..=12).contains(&month) && (1..=31).contains(&day)
}

fn valid_model(model: &str) -> bool {
    !model.is_empty()
        && model.len() <= 128
        && model
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "._:/@+-".contains(c))
}

/// Money estimate in millionths of a dollar. ccusage prices from its offline table.
fn micros(cost: f64) -> Result<u64, ReportError> {
    if !cost.is_finite() || cost < 0.0 || cost > 1.0e9 {
        return Err(ReportError::Shape("cost out of range".into()));
    }
    Ok((cost * 1_000_000.0).round() as u64)
}

pub fn parse_daily_by_agent(bytes: &[u8]) -> Result<Vec<UsageRow>, ReportError> {
    let report: Report =
        serde_json::from_slice(bytes).map_err(|error| ReportError::Shape(error.to_string()))?;
    let mut rows = Vec::new();
    let mut previous: Option<String> = None;
    let mut grand = Sum::default();
    let mut unpriced = BTreeSet::new();
    for day in report.daily {
        if day.agent != "all"
            || !valid_date(&day.period)
            || previous.as_ref().is_some_and(|p| *p >= day.period)
        {
            return Err(ReportError::Shape(format!("day {:?}", day.period)));
        }
        let mut seen = BTreeSet::new();
        let mut day_sum = Sum::default();
        for agent in &day.agents {
            let tool = TOOLS
                .iter()
                .copied()
                .find(|t| *t == agent.agent)
                .ok_or_else(|| ReportError::Shape(format!("tool {:?}", agent.agent)))?;
            if !seen.insert(tool) {
                return Err(ReportError::Shape(format!("repeated tool {tool}")));
            }
            let mut models = BTreeSet::new();
            let mut agent_sum = Sum::default();
            for model in &agent.model_breakdowns {
                if !valid_model(&model.model_name) || !models.insert(model.model_name.as_str()) {
                    return Err(ReportError::Shape(format!("model {:?}", model.model_name)));
                }
                let sum = counts(
                    model.input_tokens,
                    model.output_tokens,
                    model.cache_creation_tokens,
                    model.cache_read_tokens,
                )?;
                agent_sum.add(sum)?;
                let value = micros(model.cost)?;
                if model.missing_pricing {
                    unpriced.insert(model.model_name.clone());
                }
                rows.push(UsageRow {
                    date: day.period.clone(),
                    tool,
                    model: model.model_name.clone(),
                    input_tokens: sum.input,
                    output_tokens: sum.output,
                    cache_write_tokens: sum.write,
                    cache_read_tokens: sum.read,
                    total_tokens: sum.total()?,
                    estimated_value_micros: (!model.missing_pricing).then_some(value),
                });
                if rows.len() > MAX_ROWS {
                    return Err(ReportError::Shape("too many rows".into()));
                }
            }
            let stated = counts(
                agent.input_tokens,
                agent.output_tokens,
                agent.cache_creation_tokens,
                agent.cache_read_tokens,
            )?;
            let used: BTreeSet<&str> = agent.models_used.iter().map(String::as_str).collect();
            if stated != agent_sum
                || agent.total_tokens != agent_sum.total()?
                || used != models
                || !agent.total_cost.is_finite()
            {
                return Err(ReportError::Inconsistent(format!(
                    "{} on {}",
                    tool, day.period
                )));
            }
            day_sum.add(agent_sum)?;
        }
        let listed: BTreeSet<&str> = day.metadata.agents.iter().map(String::as_str).collect();
        let stated = counts(
            day.input_tokens,
            day.output_tokens,
            day.cache_creation_tokens,
            day.cache_read_tokens,
        )?;
        if listed != seen
            || stated != day_sum
            || day.total_tokens != day_sum.total()?
            || day.model_breakdowns.is_empty() != day.agents.is_empty()
            || day.models_used.is_empty() != day.agents.is_empty()
            || !day.total_cost.is_finite()
        {
            return Err(ReportError::Inconsistent(format!("day {}", day.period)));
        }
        grand.add(day_sum)?;
        previous = Some(day.period);
    }
    let totals = counts(
        report.totals.input_tokens,
        report.totals.output_tokens,
        report.totals.cache_creation_tokens,
        report.totals.cache_read_tokens,
    )?;
    let listed: BTreeSet<String> = report.totals.unpriced_models.iter().cloned().collect();
    if totals != grand
        || report.totals.total_tokens != grand.total()?
        || !report.totals.total_cost.is_finite()
        || listed != unpriced
    {
        return Err(ReportError::Inconsistent("report totals".into()));
    }
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &[u8] = include_bytes!("../tests/fixtures/ccusage-20.0.48-daily-by-agent.json");

    #[test]
    fn pinned_output_becomes_exact_rows() {
        let rows = parse_daily_by_agent(FIXTURE).unwrap();
        assert_eq!(rows.len(), 4);
        let sonnet = &rows[0];
        assert_eq!(
            (sonnet.date.as_str(), sonnet.tool, sonnet.model.as_str()),
            ("2026-11-04", "claude", "claude-sonnet-4-5-20250929")
        );
        assert_eq!(
            (
                sonnet.input_tokens,
                sonnet.output_tokens,
                sonnet.cache_write_tokens,
                sonnet.cache_read_tokens,
                sonnet.total_tokens
            ),
            (3000, 1300, 200, 8000, 12500)
        );
        assert_eq!(sonnet.estimated_value_micros, Some(31650));
        let codex = rows.iter().find(|row| row.tool == "codex").unwrap();
        assert_eq!(
            (
                codex.input_tokens,
                codex.output_tokens,
                codex.cache_read_tokens,
                codex.total_tokens
            ),
            (3000, 600, 1000, 4600)
        );
    }

    #[test]
    fn an_unpriced_model_keeps_its_tokens_and_has_no_estimate() {
        let rows = parse_daily_by_agent(FIXTURE).unwrap();
        let unknown = rows
            .iter()
            .find(|row| row.model == "totally-unknown-model-x")
            .unwrap();
        assert_eq!(unknown.total_tokens, 150);
        assert_eq!(unknown.estimated_value_micros, None);
    }

    #[test]
    fn an_empty_report_has_no_rows() {
        let empty = br#"{"daily":[],"totals":{"cacheCreationTokens":0,"cacheReadTokens":0,"inputTokens":0,"outputTokens":0,"totalCost":-0.0,"totalTokens":0}}"#;
        assert_eq!(parse_daily_by_agent(empty), Ok(vec![]));
    }

    fn edited(edit: impl Fn(&mut serde_json::Value)) -> Vec<u8> {
        let mut value: serde_json::Value = serde_json::from_slice(FIXTURE).unwrap();
        edit(&mut value);
        serde_json::to_vec(&value).unwrap()
    }

    #[test]
    fn an_unknown_field_rejects_the_scan() {
        let bytes = edited(|v| {
            v["daily"][0]["agents"][0]["surprise"] = 1.into();
        });
        assert!(matches!(
            parse_daily_by_agent(&bytes),
            Err(ReportError::Shape(_))
        ));
    }

    #[test]
    fn totals_that_do_not_add_up_reject_the_scan() {
        let bytes = edited(|v| {
            v["daily"][0]["agents"][0]["inputTokens"] = 1.into();
        });
        assert!(matches!(
            parse_daily_by_agent(&bytes),
            Err(ReportError::Inconsistent(_))
        ));
        let bytes = edited(|v| {
            v["totals"]["totalTokens"] = 1.into();
        });
        assert!(matches!(
            parse_daily_by_agent(&bytes),
            Err(ReportError::Inconsistent(_))
        ));
    }

    #[test]
    fn unknown_tools_bad_dates_models_and_negative_counts_are_refused() {
        for bytes in [
            edited(|v| {
                v["daily"][0]["agents"][0]["agent"] = "mystery".into();
            }),
            edited(|v| {
                v["daily"][0]["period"] = "2026-13-01".into();
            }),
            edited(|v| {
                v["daily"][0]["agents"][0]["modelBreakdowns"][0]["modelName"] = "<script>".into();
            }),
            edited(|v| {
                v["daily"][0]["agents"][0]["modelBreakdowns"][0]["inputTokens"] = (-5).into();
            }),
            edited(|v| {
                v["daily"][1]["period"] = "2026-11-04".into();
            }),
        ] {
            assert!(parse_daily_by_agent(&bytes).is_err());
        }
    }
}
