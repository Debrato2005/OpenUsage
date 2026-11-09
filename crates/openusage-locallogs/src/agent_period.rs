//! Independent period totals for the additional tools under SC-022.

use crate::agent_report::{
    ModelDetail, TokenDetails, add, bounded, inconsistent, model_name, model_names,
    parse_agent_sessions, shape,
};
use crate::report::{MAX_ROWS, Report, ReportError, Sum, counts, micros, valid_date};
use crate::tools::{PeriodPolicy, SessionShape, tool};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UsagePeriod {
    /// A scanner day in the zone supplied by the caller. No activity time is inferred.
    Day(String),
    Unplaced,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentPeriod {
    pub period: UsagePeriod,
    pub total_tokens: u64,
    pub details: TokenDetails,
    pub estimated_value_micros: Option<u64>,
    pub models: Vec<ModelDetail>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentPeriodReport {
    pub tool: &'static str,
    /// Independent scanner total. Session totals must never be added to this value.
    pub total_tokens: u64,
    pub details: TokenDetails,
    pub estimated_value_micros: Option<u64>,
    pub models: Vec<ModelDetail>,
    pub periods: Vec<AgentPeriod>,
}

/// Reads daily-by-agent JSON for dated candidates and full session JSON for unplaced tools.
/// Empty reports return None. The caller owns the pinned zone and consented scan scope.
/// This parser does not run the scanner, save data, or qualify a tool for enablement.
pub fn parse_agent_periods(
    id: &str,
    bytes: &[u8],
) -> Result<Option<AgentPeriodReport>, ReportError> {
    let tool = tool(id)
        .filter(|tool| tool.session_shape == Some(SessionShape::Agent))
        .ok_or_else(|| shape("no common period report for this tool"))?;
    let policy = tool
        .agent_report
        .as_ref()
        .ok_or_else(|| shape("missing report policy"))?;
    if policy.period == PeriodPolicy::Unplaced {
        return Ok(
            parse_agent_sessions(id, bytes)?.map(|report| AgentPeriodReport {
                tool: report.tool,
                total_tokens: report.total_tokens,
                details: report.details.clone(),
                estimated_value_micros: report.estimated_value_micros,
                models: report.models.clone(),
                periods: vec![AgentPeriod {
                    period: UsagePeriod::Unplaced,
                    total_tokens: report.total_tokens,
                    details: report.details,
                    estimated_value_micros: report.estimated_value_micros,
                    models: report.models,
                }],
            }),
        );
    }
    let report: Report =
        serde_json::from_slice(bytes).map_err(|error| shape(&error.to_string()))?;
    if report.daily.len() > MAX_ROWS {
        return Err(shape("too many days"));
    }
    let mut periods = Vec::new();
    let mut grand = Sum::default();
    let mut total = 0;
    let mut previous: Option<&str> = None;
    let mut model_count = 0usize;
    let mut unpriced = BTreeSet::new();
    let mut models: BTreeMap<String, ModelDetail> = BTreeMap::new();
    let mut all_models_attributed = true;
    for day in &report.daily {
        if day.agent != "all"
            || !valid_date(&day.period)
            || previous.is_some_and(|previous| previous >= day.period.as_str())
            || day.metadata.agents != [id]
            || day.agents.len() != 1
        {
            return Err(shape("day metadata or order"));
        }
        let agent = &day.agents[0];
        if agent.agent != id
            || agent.input_tokens != day.input_tokens
            || agent.output_tokens != day.output_tokens
            || agent.cache_creation_tokens != day.cache_creation_tokens
            || agent.cache_read_tokens != day.cache_read_tokens
            || agent.total_tokens != day.total_tokens
            || agent.total_cost != day.total_cost
            || agent.model_breakdowns != day.model_breakdowns
            || agent.models_used != day.models_used
        {
            return Err(inconsistent("day and tool details disagree"));
        }
        let stated = counts(
            agent.input_tokens,
            agent.output_tokens,
            agent.cache_creation_tokens,
            agent.cache_read_tokens,
        )?;
        bounded(agent.total_tokens, stated)?;
        micros(agent.total_cost)?;
        let used = model_names(&agent.models_used, policy)?;
        let mut seen = BTreeSet::new();
        let mut subtotal = Sum::default();
        let mut details = Vec::new();
        for model in &agent.model_breakdowns {
            model_count += 1;
            if model_count > MAX_ROWS {
                return Err(shape("too many model rows"));
            }
            let name = model_name(&model.model_name, policy)?;
            if !seen.insert(name.clone()) {
                return Err(shape("repeated model"));
            }
            let sum = counts(
                model.input_tokens,
                model.output_tokens,
                model.cache_creation_tokens,
                model.cache_read_tokens,
            )?;
            subtotal.add(sum)?;
            micros(model.cost)?;
            if model.missing_pricing {
                unpriced.insert(name.clone());
            }
            details.push(ModelDetail {
                model: name,
                total_tokens: Some(sum.total()?),
                details: TokenDetails::default(),
                estimated_value_micros: None,
            });
        }
        if seen != used
            || subtotal.input > stated.input
            || subtotal.output > stated.output
            || subtotal.write > stated.write
            || subtotal.read > stated.read
        {
            return Err(inconsistent("model details disagree with the day"));
        }
        let complete = subtotal.total()? == agent.total_tokens;
        all_models_attributed &= complete;
        for model in &mut details {
            if !complete {
                model.total_tokens = None;
            }
            if let Some(saved) = models.get_mut(&model.model) {
                saved.total_tokens = match saved.total_tokens.zip(model.total_tokens) {
                    Some((a, b)) => Some(add(a, b)?),
                    None => None,
                };
            } else {
                models.insert(model.model.clone(), model.clone());
            }
        }
        grand.add(stated)?;
        total = add(total, agent.total_tokens)?;
        previous = Some(&day.period);
        periods.push(AgentPeriod {
            period: UsagePeriod::Day(day.period.clone()),
            total_tokens: agent.total_tokens,
            details: TokenDetails::default(),
            estimated_value_micros: None,
            models: details,
        });
    }
    let stated = counts(
        report.totals.input_tokens,
        report.totals.output_tokens,
        report.totals.cache_creation_tokens,
        report.totals.cache_read_tokens,
    )?;
    bounded(report.totals.total_tokens, stated)?;
    micros(report.totals.total_cost)?;
    if grand != stated
        || total != report.totals.total_tokens
        || model_names(&report.totals.unpriced_models, policy)? != unpriced
    {
        return Err(inconsistent("report totals"));
    }
    if periods.is_empty() {
        if report.totals.total_cost != 0.0 {
            return Err(inconsistent("empty report"));
        }
        return Ok(None);
    }
    if !all_models_attributed {
        for model in models.values_mut() {
            model.total_tokens = None;
        }
    }
    Ok(Some(AgentPeriodReport {
        tool: tool.id.as_str(),
        total_tokens: total,
        details: TokenDetails::default(),
        estimated_value_micros: None,
        models: models.into_values().collect(),
        periods,
    }))
}
