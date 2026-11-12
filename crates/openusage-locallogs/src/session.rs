//! Strict parsers for `ccusage claude session --json` and `ccusage codex session --json` from the
//! pinned version (OU-LOG-017, SC-021). The two tools give different shapes. An unknown field, a
//! wrong type or totals that do not add up reject the whole scan.

use crate::report::{MAX_ROWS, Model, ReportError, Sum, counts, micros, valid_date, valid_model};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

/// One session of one tool. Token counts are exact integers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionRow {
    pub tool: &'static str,
    pub session_key: String,
    /// Only Claude Code reports a project: the folder name that it writes (SC-021).
    pub project: Option<String>,
    /// Codex reports no start (SC-021).
    pub start_utc: Option<String>,
    pub end_utc: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_write_tokens: u64,
    pub cache_read_tokens: u64,
    /// A part of the output count. None when the tool reports none.
    pub reasoning_tokens: Option<u64>,
    pub total_tokens: u64,
    /// None when a model of the session has no price (OU-LOG-013).
    pub estimated_value_micros: Option<u64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct ClaudeReport {
    sessions: Vec<ClaudeSession>,
    totals: ClaudeTotals,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct ClaudeSession {
    cache_creation_tokens: u64,
    cache_read_tokens: u64,
    first_activity: String,
    input_tokens: u64,
    last_activity: String,
    model_breakdowns: Vec<Model>,
    models_used: Vec<String>,
    output_tokens: u64,
    project_path: String,
    session_id: String,
    total_cost: f64,
    total_tokens: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct ClaudeTotals {
    cache_creation_tokens: u64,
    cache_read_tokens: u64,
    input_tokens: u64,
    output_tokens: u64,
    total_cost: f64,
    total_tokens: u64,
    #[serde(default)]
    unpriced_models: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct CodexReport {
    sessions: Vec<CodexSession>,
    totals: CodexTotals,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct CodexSession {
    cache_creation_tokens: u64,
    cache_read_tokens: u64,
    #[serde(rename = "costUSD")]
    cost_usd: f64,
    directory: String,
    input_tokens: u64,
    last_activity: String,
    models: BTreeMap<String, CodexModel>,
    output_tokens: u64,
    reasoning_output_tokens: u64,
    session_file: String,
    session_id: String,
    total_tokens: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct CodexModel {
    pub(crate) cache_creation_tokens: u64,
    pub(crate) cache_read_tokens: u64,
    pub(crate) input_tokens: u64,
    pub(crate) is_fallback: bool,
    #[serde(default)]
    pub(crate) missing_pricing: bool,
    pub(crate) output_tokens: u64,
    pub(crate) reasoning_output_tokens: u64,
    pub(crate) total_tokens: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct CodexTotals {
    cache_creation_tokens: u64,
    cache_read_tokens: u64,
    #[serde(rename = "costUSD")]
    cost_usd: f64,
    input_tokens: u64,
    output_tokens: u64,
    reasoning_output_tokens: u64,
    total_tokens: u64,
    #[serde(default)]
    unpriced_models: Vec<String>,
}

/// The UTC form that ccusage writes and the store keeps: `2026-11-05T11:00:00.000Z`.
pub(crate) fn valid_utc(value: &str) -> bool {
    let b = value.as_bytes();
    let digits = |range: std::ops::Range<usize>| b[range].iter().all(u8::is_ascii_digit);
    b.len() == 24
        && value.is_ascii()
        && valid_date(&value[..10])
        && b[10] == b'T'
        && b[13] == b':'
        && b[16] == b':'
        && b[19] == b'.'
        && b[23] == b'Z'
        && digits(11..13)
        && digits(14..16)
        && digits(17..19)
        && digits(20..23)
        && &value[11..13] < "24"
        && &value[14..16] < "60"
        && &value[17..19] < "60"
}

/// A key, never a path to open (OU-SEC-021).
pub(crate) fn valid_key(key: &str) -> bool {
    (1..=256).contains(&key.len())
        && key
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_./:".contains(&b))
}

fn valid_project(project: &str) -> bool {
    (1..=512).contains(&project.len()) && !project.chars().any(char::is_control)
}

fn shape(what: &str, value: &str) -> ReportError {
    ReportError::Shape(format!("{what} {value:?}"))
}

fn inconsistent(key: &str) -> ReportError {
    ReportError::Inconsistent(format!("session {key}"))
}

/// Parses the session report of one tool. Only Claude Code and Codex have a session report here.
pub fn parse_sessions(tool: &str, bytes: &[u8]) -> Result<Vec<SessionRow>, ReportError> {
    match crate::tools::tool(tool).and_then(|tool| tool.session_shape) {
        Some(crate::tools::SessionShape::Claude) => parse_claude(bytes),
        Some(crate::tools::SessionShape::Codex) => parse_codex(bytes),
        _ => Err(shape("no session report for", tool)),
    }
}

fn parse_claude(bytes: &[u8]) -> Result<Vec<SessionRow>, ReportError> {
    let report: ClaudeReport =
        serde_json::from_slice(bytes).map_err(|error| ReportError::Shape(error.to_string()))?;
    if report.sessions.len() > MAX_ROWS {
        return Err(ReportError::Shape("too many sessions".into()));
    }
    let (mut rows, mut keys, mut unpriced, mut grand) =
        (Vec::new(), BTreeSet::new(), BTreeSet::new(), Sum::default());
    for session in report.sessions {
        let key = session.session_id;
        if !valid_key(&key) || !keys.insert(key.clone()) {
            return Err(shape("session", &key));
        }
        if !valid_project(&session.project_path)
            || !valid_utc(&session.first_activity)
            || !valid_utc(&session.last_activity)
            || session.first_activity > session.last_activity
        {
            return Err(shape("session", &key));
        }
        let (mut sum, mut models, mut value, mut priced) =
            (Sum::default(), BTreeSet::new(), 0u64, true);
        for model in &session.model_breakdowns {
            if !valid_model(&model.model_name) || !models.insert(model.model_name.as_str()) {
                return Err(shape("model", &model.model_name));
            }
            sum.add(counts(
                model.input_tokens,
                model.output_tokens,
                model.cache_creation_tokens,
                model.cache_read_tokens,
            )?)?;
            if model.missing_pricing {
                priced = false;
                unpriced.insert(model.model_name.clone());
            }
            value = value
                .checked_add(micros(model.cost)?)
                .ok_or_else(|| inconsistent(&key))?;
        }
        let stated = counts(
            session.input_tokens,
            session.output_tokens,
            session.cache_creation_tokens,
            session.cache_read_tokens,
        )?;
        let used: BTreeSet<&str> = session.models_used.iter().map(String::as_str).collect();
        if stated != sum
            || session.total_tokens != sum.total()?
            || used != models
            || !session.total_cost.is_finite()
        {
            return Err(inconsistent(&key));
        }
        grand.add(sum)?;
        rows.push(SessionRow {
            tool: "claude",
            session_key: key,
            project: Some(session.project_path),
            start_utc: Some(session.first_activity),
            end_utc: session.last_activity,
            input_tokens: sum.input,
            output_tokens: sum.output,
            cache_write_tokens: sum.write,
            cache_read_tokens: sum.read,
            reasoning_tokens: None,
            total_tokens: sum.total()?,
            estimated_value_micros: priced.then_some(value),
        });
    }
    let totals = report.totals;
    let stated = counts(
        totals.input_tokens,
        totals.output_tokens,
        totals.cache_creation_tokens,
        totals.cache_read_tokens,
    )?;
    let listed: BTreeSet<String> = totals.unpriced_models.into_iter().collect();
    if stated != grand
        || totals.total_tokens != grand.total()?
        || listed != unpriced
        || !totals.total_cost.is_finite()
    {
        return Err(ReportError::Inconsistent("totals".into()));
    }
    Ok(rows)
}

fn parse_codex(bytes: &[u8]) -> Result<Vec<SessionRow>, ReportError> {
    let report: CodexReport =
        serde_json::from_slice(bytes).map_err(|error| ReportError::Shape(error.to_string()))?;
    if report.sessions.len() > MAX_ROWS {
        return Err(ReportError::Shape("too many sessions".into()));
    }
    let (mut rows, mut keys, mut unpriced) = (Vec::new(), BTreeSet::new(), BTreeSet::new());
    let (mut grand, mut grand_reasoning) = (Sum::default(), 0u64);
    for session in report.sessions {
        let key = session.session_id;
        if !valid_key(&key)
            || !keys.insert(key.clone())
            || !valid_key(&session.session_file)
            || !(session.directory.is_empty() || valid_key(&session.directory))
            || !valid_utc(&session.last_activity)
        {
            return Err(shape("session", &key));
        }
        let (mut sum, mut reasoning, mut priced) = (Sum::default(), 0u64, true);
        for (name, model) in &session.models {
            if !valid_model(name) {
                return Err(shape("model", name));
            }
            let part = counts(
                model.input_tokens,
                model.output_tokens,
                model.cache_creation_tokens,
                model.cache_read_tokens,
            )?;
            // Codex counts reasoning inside output.
            if model.total_tokens != part.total()?
                || model.reasoning_output_tokens > model.output_tokens
            {
                return Err(inconsistent(&key));
            }
            let _ = model.is_fallback;
            sum.add(part)?;
            reasoning = reasoning
                .checked_add(model.reasoning_output_tokens)
                .ok_or_else(|| inconsistent(&key))?;
            if model.missing_pricing {
                priced = false;
                unpriced.insert(name.clone());
            }
        }
        let stated = counts(
            session.input_tokens,
            session.output_tokens,
            session.cache_creation_tokens,
            session.cache_read_tokens,
        )?;
        if stated != sum
            || session.total_tokens != sum.total()?
            || session.reasoning_output_tokens != reasoning
        {
            return Err(inconsistent(&key));
        }
        let value = micros(session.cost_usd)?;
        grand.add(sum)?;
        grand_reasoning = grand_reasoning
            .checked_add(reasoning)
            .ok_or_else(|| inconsistent(&key))?;
        rows.push(SessionRow {
            tool: "codex",
            session_key: key,
            project: None,
            start_utc: None,
            end_utc: session.last_activity,
            input_tokens: sum.input,
            output_tokens: sum.output,
            cache_write_tokens: sum.write,
            cache_read_tokens: sum.read,
            reasoning_tokens: Some(reasoning),
            total_tokens: sum.total()?,
            estimated_value_micros: priced.then_some(value),
        });
    }
    let totals = report.totals;
    let stated = counts(
        totals.input_tokens,
        totals.output_tokens,
        totals.cache_creation_tokens,
        totals.cache_read_tokens,
    )?;
    let listed: BTreeSet<String> = totals.unpriced_models.into_iter().collect();
    if stated != grand
        || totals.total_tokens != grand.total()?
        || totals.reasoning_output_tokens != grand_reasoning
        || listed != unpriced
        || !totals.cost_usd.is_finite()
    {
        return Err(ReportError::Inconsistent("totals".into()));
    }
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CLAUDE: &[u8] = include_bytes!("../tests/fixtures/ccusage-20.0.48-session-claude.json");
    const CODEX: &[u8] = include_bytes!("../tests/fixtures/ccusage-20.0.48-session-codex.json");

    type Edit = Box<dyn Fn(&mut serde_json::Value)>;

    fn edited(bytes: &[u8], edit: impl Fn(&mut serde_json::Value)) -> Vec<u8> {
        let mut value: serde_json::Value = serde_json::from_slice(bytes).unwrap();
        edit(&mut value);
        serde_json::to_vec(&value).unwrap()
    }

    #[test]
    fn claude_sessions_keep_project_times_and_exact_counts() {
        let rows = parse_sessions("claude", CLAUDE).unwrap();
        assert_eq!(rows.len(), 2);
        let first = &rows[0];
        assert_eq!(first.session_key, "11111111-aaaa-4aaa-8aaa-111111111111");
        assert_eq!(first.project.as_deref(), Some("-home-dev-alpha"));
        assert_eq!(first.start_utc.as_deref(), Some("2026-11-04T09:00:00.000Z"));
        assert_eq!(first.end_utc, "2026-11-05T11:00:00.000Z");
        assert_eq!(
            (
                first.input_tokens,
                first.output_tokens,
                first.cache_write_tokens,
                first.cache_read_tokens,
                first.total_tokens
            ),
            (4500, 2000, 300, 8000, 14800)
        );
        assert_eq!(
            first.reasoning_tokens, None,
            "Claude Code reports no reasoning count."
        );
        assert_eq!(first.estimated_value_micros, Some(76_875 + 31_650));
        assert_eq!(
            rows[1].estimated_value_micros, None,
            "An unpriced model has no estimate."
        );
    }

    #[test]
    fn codex_sessions_keep_reasoning_and_have_no_start_or_project() {
        let rows = parse_sessions("codex", CODEX).unwrap();
        assert_eq!(rows.len(), 1);
        let row = &rows[0];
        assert_eq!(
            row.session_key,
            "2026/11/05/rollout-2026-11-05T12-00-00-22222222-bbbb-4bbb-8bbb-222222222222"
        );
        assert_eq!(
            (row.start_utc.as_deref(), row.project.as_deref()),
            (None, None)
        );
        assert_eq!(
            (row.output_tokens, row.reasoning_tokens, row.total_tokens),
            (600, Some(200), 4600)
        );
        assert_eq!(row.estimated_value_micros, Some(9_875));
    }

    #[test]
    fn empty_reports_have_no_rows_and_other_tools_have_no_report() {
        let empty_claude = br#"{"sessions":[],"totals":{"cacheCreationTokens":0,"cacheReadTokens":0,"inputTokens":0,"outputTokens":0,"totalCost":-0.0,"totalTokens":0}}"#;
        let empty_codex = br#"{"sessions":[],"totals":{"cacheCreationTokens":0,"cacheReadTokens":0,"costUSD":0,"inputTokens":0,"outputTokens":0,"reasoningOutputTokens":0,"totalTokens":0}}"#;
        assert_eq!(parse_sessions("claude", empty_claude).unwrap(), []);
        assert_eq!(parse_sessions("codex", empty_codex).unwrap(), []);
        assert!(matches!(
            parse_sessions("opencode", empty_codex),
            Err(ReportError::Shape(_))
        ));
    }

    #[test]
    fn malformed_unicode_times_return_an_error_without_panicking() {
        for (tool, fixture) in [("claude", CLAUDE), ("codex", CODEX)] {
            let bytes = edited(fixture, |value| {
                value["sessions"][0]["lastActivity"] = "2026-11-0é03:04:05.006Z".into();
            });
            assert!(parse_sessions(tool, &bytes).is_err());
        }
    }

    #[test]
    fn impossible_calendar_days_reject_sessions() {
        for (tool, fixture) in [("claude", CLAUDE), ("codex", CODEX)] {
            let bytes = edited(fixture, |value| {
                value["sessions"][0]["lastActivity"] = "2026-02-30T03:04:05.006Z".into();
                if tool == "claude" {
                    value["sessions"][0]["firstActivity"] = "2026-02-30T03:04:05.006Z".into();
                }
            });
            assert!(parse_sessions(tool, &bytes).is_err());
        }
    }

    #[test]
    fn bad_shapes_and_sums_reject_the_scan() {
        let cases: [(&str, &[u8], Edit); 8] = [
            (
                "claude",
                CLAUDE,
                Box::new(|v| v["sessions"][0]["extra"] = 1.into()),
            ),
            (
                "claude",
                CLAUDE,
                Box::new(|v| v["sessions"][0]["inputTokens"] = 4501.into()),
            ),
            (
                "claude",
                CLAUDE,
                Box::new(|v| v["sessions"][0]["firstActivity"] = "2026-11-06T09:00:00.000Z".into()),
            ),
            (
                "claude",
                CLAUDE,
                Box::new(|v| v["sessions"][1]["sessionId"] = v["sessions"][0]["sessionId"].clone()),
            ),
            (
                "claude",
                CLAUDE,
                Box::new(|v| v["sessions"][0]["sessionId"] = "a key with spaces".into()),
            ),
            (
                "codex",
                CODEX,
                Box::new(|v| v["sessions"][0]["reasoningOutputTokens"] = 201.into()),
            ),
            (
                "codex",
                CODEX,
                Box::new(|v| {
                    v["sessions"][0]["models"]["gpt-5-codex"]["reasoningOutputTokens"] = 601.into()
                }),
            ),
            (
                "codex",
                CODEX,
                Box::new(|v| v["totals"]["totalTokens"] = 4601.into()),
            ),
        ];
        for (index, (tool, bytes, edit)) in cases.iter().enumerate() {
            assert!(
                parse_sessions(tool, &edited(bytes, edit)).is_err(),
                "case {index}"
            );
        }
    }
}
