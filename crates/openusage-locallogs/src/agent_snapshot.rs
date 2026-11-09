//! Pi candidate reports from one captured source set. No tool is enabled here.
use crate::agent_report::{inconsistent, shape};
use crate::{GroupedScanResult, ReportError, parse_agent_periods, parse_agent_sessions};
use serde::Deserialize;
use serde_json::value::RawValue;

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Snapshot<'a> {
    schema_version: u32,
    tool: &'a str,
    // Keep the original JSON so the report parsers can reject duplicate fields.
    #[serde(borrow)]
    daily: &'a RawValue,
    #[serde(borrow)]
    session: &'a RawValue,
}

pub(crate) fn parse_pi_snapshot(bytes: &[u8]) -> Result<GroupedScanResult, ReportError> {
    let snapshot: Snapshot<'_> =
        serde_json::from_slice(bytes).map_err(|_| shape("invalid Pi snapshot envelope"))?;
    if snapshot.schema_version != 1 || snapshot.tool != "pi" {
        return Err(shape("unsupported Pi snapshot envelope"));
    }
    let daily = parse_agent_periods("pi", snapshot.daily.get().as_bytes())?;
    let session = parse_agent_sessions("pi", snapshot.session.get().as_bytes())?;
    match (daily, session) {
        (None, None) => Ok(GroupedScanResult {
            periods: Vec::new(),
            sessions: Vec::new(),
        }),
        (Some(daily), Some(session)) if daily.total_tokens == session.total_tokens => {
            Ok(GroupedScanResult {
                periods: daily.periods,
                sessions: session.sessions,
            })
        }
        _ => Err(inconsistent("Pi snapshot reports disagree")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn fixture() -> Value {
        let reports: Value = serde_json::from_slice(include_bytes!(
            "../tests/fixtures/remaining-tools/pi-snapshot-candidate.json"
        ))
        .unwrap();
        reports["total_only"].clone()
    }

    #[test]
    fn snapshot_preserves_independent_totals_and_missing_measurements() {
        let result = parse_pi_snapshot(&serde_json::to_vec(&fixture()).unwrap()).unwrap();
        assert_eq!(result.periods[0].total_tokens, 162);
        assert_eq!(result.sessions[0].total_tokens, 162);
        assert_eq!(result.periods[0].details, crate::TokenDetails::default());
        assert_eq!(result.sessions[0].details, crate::TokenDetails::default());
        assert_eq!(result.periods[0].estimated_value_micros, None);
        assert_eq!(result.sessions[0].estimated_value_micros, None);
    }

    #[test]
    fn snapshot_rejects_unknown_missing_changed_or_duplicate_fields() {
        let base = fixture();
        for case in [
            "version",
            "tool",
            "unknown",
            "missing",
            "nested",
            "mismatched",
        ] {
            let mut value = base.clone();
            match case {
                "version" => value["schemaVersion"] = json!(2),
                "tool" => value["tool"] = json!("amp"),
                "unknown" => value["extra"] = json!(true),
                "missing" => {
                    value.as_object_mut().unwrap().remove("session");
                }
                "nested" => value["daily"]["daily"][0]["extra"] = json!(true),
                "mismatched" => {
                    value["session"]["sessions"][0]["totalTokens"] = json!(163);
                    value["session"]["totals"]["totalTokens"] = json!(163);
                }
                _ => unreachable!(),
            }
            assert!(
                parse_pi_snapshot(&serde_json::to_vec(&value).unwrap()).is_err(),
                "{case}"
            );
        }
        let text = serde_json::to_string(&base).unwrap();
        for (from, to) in [
            (
                "\"schemaVersion\":1",
                "\"schemaVersion\":1,\"schemaVersion\":1",
            ),
            (
                "\"totalTokens\":162",
                "\"totalTokens\":162,\"totalTokens\":162",
            ),
        ] {
            let changed = text.replacen(from, to, 1);
            assert_ne!(changed, text);
            assert!(parse_pi_snapshot(changed.as_bytes()).is_err());
        }
    }
    #[test]
    fn recorded_snapshot_keeps_empty_sources_and_unattributed_usage_distinct() {
        let reports: Value = serde_json::from_slice(include_bytes!(
            "../tests/fixtures/remaining-tools/pi-snapshot-candidate.json"
        ))
        .unwrap();
        let empty = parse_pi_snapshot(&serde_json::to_vec(&reports["empty"]).unwrap()).unwrap();
        assert!(empty.periods.is_empty() && empty.sessions.is_empty());
        let result =
            parse_pi_snapshot(&serde_json::to_vec(&reports["unattributed"]).unwrap()).unwrap();
        assert_eq!(result.periods[0].total_tokens, 162);
        assert_eq!(result.sessions[0].total_tokens, 162);
        assert!(result.periods[0].models.is_empty() && result.sessions[0].models.is_empty());
        let mut incomplete = reports["baseline"].clone();
        incomplete["session"] = reports["empty"]["session"].clone();
        assert!(parse_pi_snapshot(&serde_json::to_vec(&incomplete).unwrap()).is_err());
    }
}
