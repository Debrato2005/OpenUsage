//! Common focused session reports from the pinned scanner (SC-022).
//! Reported totals stay separate from optional model attribution and unavailable measurements.

use crate::report::{MAX_COUNT, MAX_ROWS, ReportError, Sum, counts, micros, valid_model};
use crate::session::valid_utc;
use crate::tools::{AgentReportPolicy, ProjectPolicy, SessionShape, tool};
use serde::{Deserialize, Deserializer};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TokenDetails {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub cache_write_tokens: Option<u64>,
    pub cache_read_tokens: Option<u64>,
    pub reasoning_tokens: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelDetail {
    pub model: String,
    /// Available only when every token in the enclosing report has model attribution.
    pub total_tokens: Option<u64>,
    pub details: TokenDetails,
    pub estimated_value_micros: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentSession {
    pub session_key: String,
    pub project: Option<String>,
    pub first_activity_utc: Option<String>,
    pub last_activity_utc: Option<String>,
    pub total_tokens: u64,
    pub details: TokenDetails,
    pub estimated_value_micros: Option<u64>,
    pub models: Vec<ModelDetail>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentSessionReport {
    pub tool: &'static str,
    /// The scanner's reported total. Duplicate qualification remains a separate gate.
    pub total_tokens: u64,
    pub details: TokenDetails,
    pub estimated_value_micros: Option<u64>,
    pub models: Vec<ModelDetail>,
    pub sessions: Vec<AgentSession>,
}

// Missing and explicit null have different meanings in the pinned wire format.
#[derive(Debug, Default)]
enum Field<T> {
    #[default]
    Missing,
    Present(T),
}
impl<'de, T: Deserialize<'de>> Deserialize<'de> for Field<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        T::deserialize(deserializer).map(Self::Present)
    }
}
impl<T> Field<T> {
    fn value(&self) -> Option<&T> {
        match self {
            Self::Missing => None,
            Self::Present(value) => Some(value),
        }
    }
    fn present(&self) -> bool {
        matches!(self, Self::Present(_))
    }
}
impl Field<Option<String>> {
    fn text(&self) -> Option<&str> {
        self.value().and_then(|value| value.as_deref())
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Report {
    sessions: Vec<Session>,
    #[serde(default)]
    totals: Field<Option<Totals>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Totals {
    input_tokens: u64,
    output_tokens: u64,
    cache_creation_tokens: u64,
    cache_read_tokens: u64,
    total_tokens: u64,
    total_cost: f64,
    #[serde(default)]
    credits: Field<f64>,
    #[serde(default)]
    unpriced_models: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Session {
    session_id: String,
    input_tokens: u64,
    output_tokens: u64,
    cache_creation_tokens: u64,
    cache_read_tokens: u64,
    total_tokens: u64,
    total_cost: f64,
    model_breakdowns: Vec<Model>,
    models_used: Vec<String>,
    #[serde(default)]
    credits: Field<f64>,
    #[serde(default)]
    message_count: Field<u64>,
    #[serde(default)]
    first_activity: Field<Option<String>>,
    #[serde(default)]
    last_activity: Field<Option<String>>,
    #[serde(default)]
    project_path: Field<Option<String>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Model {
    model_name: String,
    input_tokens: u64,
    output_tokens: u64,
    cache_creation_tokens: u64,
    cache_read_tokens: u64,
    cost: f64,
    #[serde(default)]
    missing_pricing: bool,
    #[serde(default)]
    stored_cost: Field<f64>,
}

fn shape(message: &str) -> ReportError {
    ReportError::Shape(message.into())
}
fn inconsistent(message: &str) -> ReportError {
    ReportError::Inconsistent(message.into())
}
fn add(a: u64, b: u64) -> Result<u64, ReportError> {
    a.checked_add(b)
        .filter(|value| *value <= MAX_COUNT)
        .ok_or_else(|| inconsistent("count out of range"))
}
fn bounded(total: u64, sum: Sum) -> Result<(), ReportError> {
    if total > MAX_COUNT || sum.total()? > total {
        return Err(inconsistent(
            "reported total is below its token subtotal or out of range",
        ));
    }
    Ok(())
}
fn credits(value: &Field<f64>) -> Result<(), ReportError> {
    if value
        .value()
        .is_some_and(|value| !value.is_finite() || !(0.0..=1.0e9).contains(value))
    {
        return Err(shape("credit count"));
    }
    Ok(())
}
fn model_name(value: &str, policy: &AgentReportPolicy) -> Result<String, ReportError> {
    let name = match policy.model_prefix.as_deref() {
        Some(prefix) => value
            .strip_prefix(prefix)
            .ok_or_else(|| shape("model prefix"))?,
        None => value,
    };
    if !valid_model(name) {
        return Err(shape("model name"));
    }
    Ok(name.into())
}
fn model_names(
    values: &[String],
    policy: &AgentReportPolicy,
) -> Result<BTreeSet<String>, ReportError> {
    if values.len() > MAX_ROWS {
        return Err(shape("too many model names"));
    }
    let mut names = BTreeSet::new();
    for value in values {
        if !names.insert(model_name(value, policy)?) {
            return Err(shape("repeated model name"));
        }
    }
    Ok(names)
}
fn metadata(session: &Session, policy: &AgentReportPolicy) -> Result<(), ReportError> {
    for present in [
        session.first_activity.present(),
        session.last_activity.present(),
        session.project_path.present(),
    ] {
        if present != policy.metadata {
            return Err(shape("session metadata fields"));
        }
    }
    let (first, last) = (session.first_activity.text(), session.last_activity.text());
    if first.is_some_and(|value| !valid_utc(value))
        || last.is_some_and(|value| !valid_utc(value))
        || first.zip(last).is_some_and(|(first, last)| first > last)
    {
        return Err(shape("session activity time"));
    }
    if session
        .project_path
        .text()
        .is_some_and(|value| value.len() > 512 || value.chars().any(char::is_control))
    {
        return Err(shape("project name"));
    }
    Ok(())
}

/// Parses a full, unbounded focused report for one of the sixteen additional tools.
/// Empty reports return None. This parser does not enable a tool or run the scanner.
pub fn parse_agent_sessions(
    id: &str,
    bytes: &[u8],
) -> Result<Option<AgentSessionReport>, ReportError> {
    let tool = tool(id)
        .filter(|tool| tool.session_shape == Some(SessionShape::Agent))
        .ok_or_else(|| shape("no common session report for this tool"))?;
    let policy = tool
        .agent_report
        .as_ref()
        .ok_or_else(|| shape("missing report policy"))?;
    let report: Report =
        serde_json::from_slice(bytes).map_err(|error| shape(&error.to_string()))?;
    if report.sessions.len() > MAX_ROWS {
        return Err(shape("too many sessions"));
    }
    let totals = match &report.totals {
        Field::Present(Some(totals)) => totals,
        Field::Present(None) if policy.null_empty_totals && report.sessions.is_empty() => {
            return Ok(None);
        }
        _ => return Err(shape("report totals")),
    };
    let mut grand = Sum::default();
    let mut grand_total = 0;
    let mut model_count = 0usize;
    let mut keys = BTreeSet::new();
    let mut unpriced = BTreeSet::new();
    let mut models: BTreeMap<String, ModelDetail> = BTreeMap::new();
    let mut sessions = Vec::new();
    let mut all_models_attributed = true;
    for session in &report.sessions {
        if !crate::session::valid_key(&session.session_id) || !keys.insert(&session.session_id) {
            return Err(shape("session key"));
        }
        metadata(session, policy)?;
        micros(session.total_cost)?;
        credits(&session.credits)?;
        if session
            .message_count
            .value()
            .is_some_and(|value| *value > MAX_COUNT)
        {
            return Err(shape("message count"));
        }
        let stated = counts(
            session.input_tokens,
            session.output_tokens,
            session.cache_creation_tokens,
            session.cache_read_tokens,
        )?;
        bounded(session.total_tokens, stated)?;
        let used = model_names(&session.models_used, policy)?;
        let mut seen = BTreeSet::new();
        let mut subtotal = Sum::default();
        let mut detail = Vec::new();
        for model in &session.model_breakdowns {
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
            if let Some(value) = model.stored_cost.value() {
                micros(*value)?;
            }
            if model.missing_pricing {
                unpriced.insert(name.clone());
            }
            detail.push(ModelDetail {
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
            return Err(inconsistent(
                "model details exceed the session or disagree with its model list",
            ));
        }
        let complete = subtotal.total()? == session.total_tokens;
        all_models_attributed &= complete;
        for model in &mut detail {
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
        grand_total = add(grand_total, session.total_tokens)?;
        let project = session
            .project_path
            .text()
            .filter(|value| !value.is_empty())
            .filter(|value| match policy.project {
                ProjectPolicy::Unavailable => false,
                ProjectPolicy::Reported => true,
                ProjectPolicy::Pi => *value != "unknown",
            })
            .map(str::to_owned);
        sessions.push(AgentSession {
            session_key: session.session_id.clone(),
            project,
            first_activity_utc: session
                .first_activity
                .text()
                .filter(|_| policy.activity_times)
                .map(str::to_owned),
            last_activity_utc: session
                .last_activity
                .text()
                .filter(|_| policy.activity_times)
                .map(str::to_owned),
            total_tokens: session.total_tokens,
            details: TokenDetails::default(),
            estimated_value_micros: None,
            models: detail,
        });
    }
    let stated = counts(
        totals.input_tokens,
        totals.output_tokens,
        totals.cache_creation_tokens,
        totals.cache_read_tokens,
    )?;
    bounded(totals.total_tokens, stated)?;
    micros(totals.total_cost)?;
    credits(&totals.credits)?;
    if grand != stated
        || grand_total != totals.total_tokens
        || model_names(&totals.unpriced_models, policy)? != unpriced
    {
        return Err(inconsistent("report totals"));
    }
    if sessions.is_empty() {
        if totals.total_cost != 0.0 || totals.credits.value().is_some_and(|value| *value != 0.0) {
            return Err(inconsistent("empty report"));
        }
        return Ok(None);
    }
    if !all_models_attributed {
        for model in models.values_mut() {
            model.total_tokens = None;
        }
    }
    Ok(Some(AgentSessionReport {
        tool: tool.id.as_str(),
        total_tokens: grand_total,
        details: TokenDetails::default(),
        estimated_value_micros: None,
        models: models.into_values().collect(),
        sessions,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn fixture(id: &str) -> Value {
        let reports: Value = serde_json::from_slice(include_bytes!(
            "../tests/fixtures/remaining-tools/expected.json"
        ))
        .unwrap();
        reports[id]["session"].clone()
    }
    fn parse(id: &str, report: &Value) -> Result<Option<AgentSessionReport>, ReportError> {
        parse_agent_sessions(id, &serde_json::to_vec(report).unwrap())
    }
    fn set_total(report: &mut Value, total: u64) {
        report["sessions"][0]["totalTokens"] = total.into();
        report["totals"]["totalTokens"] = total.into();
    }

    #[test]
    fn all_sixteen_recorded_reports_keep_totals_without_inventing_measurements() {
        let cases = [
            ("opencode", 173, false),
            ("amp", 162, true),
            ("droid", 173, false),
            ("codebuff", 162, true),
            ("hermes", 173, false),
            ("pi", 162, true),
            ("goose", 161, false),
            ("kilo", 173, false),
            ("copilot", 120, true),
            ("gemini", 125, false),
            ("antigravity", 140, false),
            ("kimi", 270, true),
            ("qwen", 135, false),
            ("openclaw", 145, false),
            ("grok", 120, true),
            ("zcode", 130, false),
        ];
        for (id, total, model_totals_available) in cases {
            let parsed = parse(id, &fixture(id)).unwrap().unwrap();
            assert_eq!((parsed.tool, parsed.total_tokens), (id, total));
            assert_eq!(parsed.details, TokenDetails::default());
            assert_eq!(parsed.estimated_value_micros, None);
            assert!(!parsed.models.is_empty());
            assert!(
                parsed
                    .models
                    .iter()
                    .all(|model| model.total_tokens.is_some() == model_totals_available)
            );
            if model_totals_available {
                assert_eq!(
                    parsed
                        .models
                        .iter()
                        .map(|model| model.total_tokens.unwrap())
                        .sum::<u64>(),
                    total
                );
            }
            for session in &parsed.sessions {
                assert_eq!(session.details, TokenDetails::default());
                assert_eq!(session.estimated_value_micros, None);
                assert_eq!(
                    session.first_activity_utc.is_some(),
                    matches!(id, "pi" | "zcode")
                );
                assert_eq!(
                    session.last_activity_utc.is_some(),
                    matches!(id, "pi" | "zcode")
                );
                assert_eq!(
                    session.project.is_some(),
                    matches!(id, "pi" | "qwen" | "grok" | "zcode")
                );
                for model in &session.models {
                    assert!(valid_model(&model.model));
                    assert_eq!(model.details, TokenDetails::default());
                    assert_eq!(model.estimated_value_micros, None);
                }
            }
        }
    }

    #[test]
    fn a_gap_is_not_assigned_to_the_only_named_model_or_to_reasoning() {
        let mut raw = fixture("pi");
        set_total(&mut raw, 173);
        let parsed = parse("pi", &raw).unwrap().unwrap();
        assert_eq!(parsed.total_tokens, 173);
        assert_eq!(parsed.models.len(), 1);
        assert_eq!(parsed.models[0].total_tokens, None);
        assert_eq!(parsed.sessions[0].models[0].total_tokens, None);
        assert_eq!(parsed.sessions[0].details.reasoning_tokens, None);
        assert_eq!(parsed.models[0].model, "claude-sonnet-4-20250514");
    }

    #[test]
    fn unmodeled_usage_keeps_the_total_without_an_unknown_model_row() {
        let mut raw = fixture("pi");
        raw["sessions"][0]["inputTokens"] = 133.into();
        raw["totals"]["inputTokens"] = 133.into();
        set_total(&mut raw, 175);
        let parsed = parse("pi", &raw).unwrap().unwrap();
        assert_eq!(parsed.total_tokens, 175);
        assert_eq!(parsed.models.len(), 1);
        assert_eq!(parsed.models[0].total_tokens, None);

        raw["sessions"][0]["modelBreakdowns"] = json!([]);
        raw["sessions"][0]["modelsUsed"] = json!([]);
        let parsed = parse("pi", &raw).unwrap().unwrap();
        assert_eq!(parsed.total_tokens, 175);
        assert!(parsed.models.is_empty());
        assert!(parsed.sessions[0].models.is_empty());
    }

    #[test]
    fn model_totals_require_complete_attribution_for_the_whole_session() {
        let mut raw = fixture("pi");
        let mut second = raw["sessions"][0]["modelBreakdowns"][0].clone();
        second["modelName"] = "[pi] second-model".into();
        for (name, first, next) in [
            ("inputTokens", 100, 20),
            ("outputTokens", 20, 10),
            ("cacheCreationTokens", 4, 1),
            ("cacheReadTokens", 6, 1),
        ] {
            raw["sessions"][0]["modelBreakdowns"][0][name] = first.into();
            second[name] = next.into();
        }
        raw["sessions"][0]["modelBreakdowns"]
            .as_array_mut()
            .unwrap()
            .push(second);
        raw["sessions"][0]["modelsUsed"]
            .as_array_mut()
            .unwrap()
            .push("[pi] second-model".into());
        let parsed = parse("pi", &raw).unwrap().unwrap();
        assert_eq!(
            parsed
                .models
                .iter()
                .map(|model| model.total_tokens)
                .collect::<Vec<_>>(),
            [Some(130), Some(32)]
        );
        set_total(&mut raw, 173);
        let parsed = parse("pi", &raw).unwrap().unwrap();
        assert_eq!(parsed.total_tokens, 173);
        assert!(
            parsed
                .models
                .iter()
                .all(|model| model.total_tokens.is_none())
        );
    }

    #[test]
    fn incomplete_sessions_do_not_turn_model_subtotals_into_complete_report_totals() {
        let mut raw = fixture("pi");
        let mut second = raw["sessions"][0].clone();
        second["sessionId"] = "second-session".into();
        second["totalTokens"] = 173.into();
        second["modelBreakdowns"][0]["modelName"] = "[pi] second-model".into();
        second["modelsUsed"] = json!(["[pi] second-model"]);
        raw["sessions"].as_array_mut().unwrap().push(second);
        for name in [
            "inputTokens",
            "outputTokens",
            "cacheCreationTokens",
            "cacheReadTokens",
        ] {
            raw["totals"][name] = (raw["totals"][name].as_u64().unwrap() * 2).into();
        }
        raw["totals"]["totalTokens"] = 335.into();
        let parsed = parse("pi", &raw).unwrap().unwrap();
        assert_eq!(parsed.total_tokens, 335);
        assert_eq!(parsed.sessions[0].models[0].total_tokens, Some(162));
        assert_eq!(parsed.sessions[1].models[0].total_tokens, None);
        assert!(
            parsed
                .models
                .iter()
                .all(|model| model.total_tokens.is_none())
        );
    }

    #[test]
    fn missing_metadata_and_a_reported_zero_are_distinct_from_an_empty_report() {
        for tool in crate::tools::tools()
            .iter()
            .filter(|tool| tool.agent_report.is_some())
        {
            let empty = json!({"sessions":[],"totals":{
                "inputTokens":0,"outputTokens":0,"cacheCreationTokens":0,"cacheReadTokens":0,
                "totalTokens":0,"totalCost":0
            }});
            assert_eq!(parse(&tool.id, &empty).unwrap(), None);
        }
        assert_eq!(
            parse("qwen", &json!({"sessions":[],"totals":null})).unwrap(),
            None
        );
        assert!(parse("pi", &json!({"sessions":[],"totals":null})).is_err());
        assert!(parse("qwen", &json!({"sessions":[]})).is_err());
        let mut raw = fixture("pi");
        for name in ["firstActivity", "lastActivity", "projectPath"] {
            raw["sessions"][0][name] = Value::Null;
        }
        for name in [
            "inputTokens",
            "outputTokens",
            "cacheCreationTokens",
            "cacheReadTokens",
            "totalTokens",
        ] {
            raw["sessions"][0][name] = 0.into();
            raw["totals"][name] = 0.into();
            if name != "totalTokens" {
                raw["sessions"][0]["modelBreakdowns"][0][name] = 0.into();
            }
        }
        let parsed = parse("pi", &raw).unwrap().unwrap();
        assert_eq!(parsed.total_tokens, 0);
        assert_eq!(parsed.models[0].total_tokens, Some(0));
        assert_eq!(parsed.sessions[0].project, None);
        assert_eq!(parsed.sessions[0].first_activity_utc, None);
        assert_eq!(parsed.sessions[0].last_activity_utc, None);
    }

    #[test]
    fn fallback_project_and_unpriced_models_keep_absent_values_absent() {
        let mut raw = fixture("pi");
        raw["sessions"][0]["projectPath"] = "unknown".into();
        raw["sessions"][0]["modelBreakdowns"][0]["modelName"] = "[pi] unknown-price-model".into();
        raw["sessions"][0]["modelBreakdowns"][0]["missingPricing"] = true.into();
        raw["sessions"][0]["modelsUsed"] = json!(["[pi] unknown-price-model"]);
        raw["totals"]["unpricedModels"] = json!(["[pi] unknown-price-model"]);
        let parsed = parse("pi", &raw).unwrap().unwrap();
        assert_eq!(parsed.total_tokens, 162);
        assert_eq!(parsed.sessions[0].project, None);
        assert_eq!(parsed.models[0].model, "unknown-price-model");
        assert_eq!(parsed.models[0].total_tokens, Some(162));
        assert_eq!(parsed.models[0].estimated_value_micros, None);
    }

    #[test]
    fn malformed_shapes_values_and_inconsistent_totals_reject_the_report() {
        let changes: &[fn(&mut Value)] = &[
            |v| v["extra"] = 1.into(),
            |v| {
                let _ = v.as_object_mut().unwrap().remove("totals");
            },
            |v| {
                let _ = v["sessions"][0]
                    .as_object_mut()
                    .unwrap()
                    .remove("lastActivity");
            },
            |v| v["sessions"][0]["extra"] = 1.into(),
            |v| v["sessions"][0]["inputTokens"] = (-1).into(),
            |v| v["sessions"][0]["inputTokens"] = json!(1.5),
            |v| v["sessions"][0]["inputTokens"] = u64::MAX.into(),
            |v| v["sessions"][0]["totalTokens"] = 161.into(),
            |v| v["totals"]["totalTokens"] = 163.into(),
            |v| v["totals"]["inputTokens"] = 121.into(),
            |v| v["sessions"][0]["modelBreakdowns"][0]["extra"] = 1.into(),
            |v| v["sessions"][0]["modelBreakdowns"][0]["inputTokens"] = 121.into(),
            |v| v["sessions"][0]["modelsUsed"] = json!(["[pi] other"]),
            |v| {
                v["sessions"][0]["modelsUsed"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!("[pi] claude-sonnet-4-20250514"))
            },
            |v| {
                let first = v["sessions"][0].clone();
                v["sessions"].as_array_mut().unwrap().push(first);
            },
            |v| v["sessions"][0]["lastActivity"] = "2026-11-0é03:04:05.006Z".into(),
            |v| v["sessions"][0]["lastActivity"] = "2026-02-30T03:04:05.006Z".into(),
            |v| v["sessions"][0]["projectPath"] = "private\nproject".into(),
            |v| v["sessions"][0]["credits"] = Value::Null,
            |v| v["sessions"][0]["credits"] = (-1).into(),
            |v| v["sessions"][0]["messageCount"] = u64::MAX.into(),
            |v| v["sessions"][0]["modelBreakdowns"][0]["storedCost"] = (-1).into(),
            |v| v["sessions"][0]["modelBreakdowns"][0]["cost"] = (-1).into(),
            |v| v["totals"]["unpricedModels"] = json!(["[pi] absent-model"]),
        ];
        for (index, change) in changes.iter().enumerate() {
            let mut raw = fixture("pi");
            change(&mut raw);
            assert!(parse("pi", &raw).is_err(), "case {index}");
        }
        assert!(parse_agent_sessions("claude", b"{}").is_err());
        assert!(parse_agent_sessions("not-a-tool", b"{}").is_err());
    }
}
