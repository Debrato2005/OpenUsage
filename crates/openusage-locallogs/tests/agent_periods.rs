use openusage_locallogs::{
    AgentPeriodReport, TokenDetails, UsagePeriod, parse_agent_periods, parse_agent_sessions,
};
use serde_json::{Value, json};

fn fixture(id: &str, kind: &str) -> Value {
    let reports: Value =
        serde_json::from_slice(include_bytes!("fixtures/remaining-tools/expected.json")).unwrap();
    reports[id][kind].clone()
}

fn parse(id: &str, value: &Value) -> Result<Option<AgentPeriodReport>, String> {
    parse_agent_periods(id, &serde_json::to_vec(value).unwrap()).map_err(|e| format!("{e:?}"))
}

// Keep the redundant wire views equal when a case changes the underlying measurement.
fn sync_day(value: &mut Value) {
    let agent = value["daily"][0]["agents"][0].as_object().unwrap().clone();
    for (name, val) in agent {
        if name != "agent" {
            value["daily"][0][&name] = val.clone();
            if !matches!(name.as_str(), "modelBreakdowns" | "modelsUsed") {
                value["totals"][&name] = val;
            }
        }
    }
}

#[test]
fn four_recorded_daily_reports_preserve_independent_totals() {
    for (id, total, day, attributed) in [
        ("amp", 162, "2026-11-08", true),
        ("pi", 162, "2026-11-08", true),
        ("kilo", 173, "2026-11-08", false),
        ("zcode", 130, "2026-05-28", false),
    ] {
        let raw = fixture(id, "daily");
        let report = parse(id, &raw).unwrap().unwrap();
        assert_eq!(parse(id, &raw).unwrap().unwrap(), report);
        assert_eq!((report.tool, report.total_tokens), (id, total));
        assert_eq!(report.details, TokenDetails::default());
        assert_eq!(report.estimated_value_micros, None);
        assert_eq!(report.periods.len(), 1);
        let period = &report.periods[0];
        assert_eq!(period.period, UsagePeriod::Day(day.into()));
        assert_eq!(period.total_tokens, total);
        assert_eq!(period.details, TokenDetails::default());
        assert_eq!(period.estimated_value_micros, None);
        for model in report.models.iter().chain(&period.models) {
            assert_eq!(model.total_tokens, attributed.then_some(total));
            assert_eq!(model.details, TokenDetails::default());
            assert_eq!(model.estimated_value_micros, None);
        }
        assert!(parse(id, &fixture(id, "session")).is_err());
    }
}

#[test]
fn recorded_kilo_qualification_gaps_stay_disabled_and_keep_unknown_metrics() {
    let cases: Value = serde_json::from_slice(include_bytes!(
        "fixtures/remaining-tools/kilo-qualification.json"
    ))
    .unwrap();
    assert!(
        !openusage_locallogs::tools()
            .iter()
            .find(|t| t.id == "kilo")
            .unwrap()
            .enabled
    );
    // The parser cannot recover identities or omitted rows from an aggregate report.
    // Matching these reports does not qualify their duplicate handling.
    for (name, reports) in cases.as_object().unwrap() {
        let expected = reports["daily"]["totals"]["totalTokens"].as_u64().unwrap();
        let daily = parse("kilo", &reports["daily"]).unwrap();
        let sessions =
            parse_agent_sessions("kilo", &serde_json::to_vec(&reports["session"]).unwrap())
                .unwrap();
        if matches!(name.as_str(), "empty" | "missing_time") {
            assert!(daily.is_none());
            assert!(sessions.is_none());
            continue;
        }
        let daily = daily.unwrap();
        let sessions = sessions.unwrap();
        assert_eq!(daily.total_tokens, expected, "{name}");
        assert_eq!(sessions.total_tokens, expected, "{name}");
        assert_eq!(daily.details, TokenDetails::default());
        assert_eq!(sessions.details, TokenDetails::default());
        assert_eq!(daily.estimated_value_micros, None);
        assert_eq!(sessions.estimated_value_micros, None);
        for period in &daily.periods {
            assert_eq!(period.details, TokenDetails::default());
            assert_eq!(period.estimated_value_micros, None);
        }
        for session in &sessions.sessions {
            assert_eq!(session.details, TokenDetails::default());
            assert_eq!(session.estimated_value_micros, None);
            assert_eq!(session.first_activity_utc, None);
            assert_eq!(session.last_activity_utc, None);
            assert_eq!(session.project, None);
        }
        for model in daily.models.iter().chain(&sessions.models) {
            assert_eq!(model.details, TokenDetails::default());
            assert_eq!(model.estimated_value_micros, None);
        }
    }
}

#[test]
fn recorded_pi_qualification_gaps_keep_missing_fields_unavailable() {
    let cases: Value = serde_json::from_slice(include_bytes!(
        "fixtures/remaining-tools/pi-qualification.json"
    ))
    .unwrap();
    assert!(
        !openusage_locallogs::tools()
            .iter()
            .find(|t| t.id == "pi")
            .unwrap()
            .enabled
    );
    // Aggregate reports cannot recover identities or usage omitted by the scanner.
    for (name, reports) in cases.as_object().unwrap() {
        let total = reports["daily"]["totals"]["totalTokens"].as_u64().unwrap();
        let daily = parse("pi", &reports["daily"]).unwrap();
        let sessions =
            parse_agent_sessions("pi", &serde_json::to_vec(&reports["session"]).unwrap()).unwrap();
        if matches!(name.as_str(), "empty" | "missing_time") {
            assert!(daily.is_none());
            assert!(sessions.is_none());
            continue;
        }
        let daily = daily.unwrap();
        let sessions = sessions.unwrap();
        assert_eq!(daily.total_tokens, total, "{name}");
        assert_eq!(sessions.total_tokens, total, "{name}");
        assert_eq!(daily.details, TokenDetails::default());
        assert_eq!(sessions.details, TokenDetails::default());
        assert_eq!(daily.estimated_value_micros, None);
        assert_eq!(sessions.estimated_value_micros, None);
        for period in &daily.periods {
            assert_eq!(period.period, UsagePeriod::Day("2026-05-28".into()));
            assert_eq!(period.details, TokenDetails::default());
        }
        for session in &sessions.sessions {
            assert_eq!(session.details, TokenDetails::default());
            assert_eq!(session.estimated_value_micros, None);
            for time in [&session.first_activity_utc, &session.last_activity_utc] {
                assert!(matches!(
                    time.as_deref(),
                    Some("2026-05-28T20:26:40.000Z" | "2026-05-28T22:00:00.000Z")
                ));
            }
            if name == "missing_project" {
                assert_eq!(session.project, None);
            } else {
                assert!(matches!(
                    session.project.as_deref(),
                    Some("project-a" | "project-b")
                ));
            }
        }
        if name == "missing_model" {
            assert!(daily.models.is_empty());
            assert!(sessions.models.is_empty());
        }
        for model in daily.models.iter().chain(&sessions.models) {
            assert_eq!(model.details, TokenDetails::default());
            assert_eq!(model.estimated_value_micros, None);
        }
    }
}

#[test]
fn recorded_zcode_reports_preserve_missing_projects_and_known_times() {
    let cases: Value = serde_json::from_slice(include_bytes!(
        "fixtures/remaining-tools/zcode-qualification.json"
    ))
    .unwrap();
    assert!(
        !openusage_locallogs::tools()
            .iter()
            .find(|t| t.id == "zcode")
            .unwrap()
            .enabled
    );
    for (name, reports) in cases.as_object().unwrap() {
        let expected = reports["daily"]["totals"]["totalTokens"].as_u64().unwrap();
        let daily = parse("zcode", &reports["daily"]).unwrap();
        let sessions =
            parse_agent_sessions("zcode", &serde_json::to_vec(&reports["session"]).unwrap())
                .unwrap();
        if matches!(name.as_str(), "empty" | "missing_time" | "noncompleted") {
            assert!(daily.is_none());
            assert!(sessions.is_none());
            continue;
        }
        let daily = daily.unwrap();
        let sessions = sessions.unwrap();
        assert_eq!(daily.total_tokens, expected, "{name}");
        assert_eq!(sessions.total_tokens, expected, "{name}");
        assert_eq!(daily.details, TokenDetails::default());
        assert_eq!(sessions.details, TokenDetails::default());
        assert_eq!(daily.estimated_value_micros, None);
        assert_eq!(sessions.estimated_value_micros, None);
        for session in &sessions.sessions {
            let project = (!matches!(name.as_str(), "missing_project" | "empty_project"))
                .then_some("/synthetic/project");
            assert_eq!(session.project.as_deref(), project, "{name}");
            assert_eq!(
                session.first_activity_utc.as_deref(),
                Some("2026-05-28T20:26:40.000Z")
            );
            assert_eq!(
                session.last_activity_utc.as_deref(),
                Some("2026-05-28T20:26:40.000Z")
            );
            assert_eq!(session.details, TokenDetails::default());
            assert_eq!(session.estimated_value_micros, None);
        }
    }
    let mut actual_project = cases["baseline"]["session"].clone();
    actual_project["sessions"][0]["projectPath"] = "/synthetic/ZCode".into();
    let parsed = parse_agent_sessions("zcode", &serde_json::to_vec(&actual_project).unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(
        parsed.sessions[0].project.as_deref(),
        Some("/synthetic/ZCode")
    );
}

#[test]
fn the_other_twelve_tools_use_only_an_explicit_unplaced_scope() {
    let mut unplaced = Vec::new();
    for tool in openusage_locallogs::tools().iter().filter(|t| !t.enabled) {
        if matches!(tool.id.as_str(), "amp" | "pi" | "kilo" | "zcode") {
            continue;
        }
        unplaced.push(tool.id.as_str());
        let raw = fixture(&tool.id, "session");
        let sessions = parse_agent_sessions(&tool.id, &serde_json::to_vec(&raw).unwrap())
            .unwrap()
            .unwrap();
        let report = parse(&tool.id, &raw).unwrap().unwrap();
        assert_eq!(report.total_tokens, sessions.total_tokens);
        assert_eq!(report.models, sessions.models);
        assert_eq!(report.periods.len(), 1);
        assert_eq!(report.periods[0].period, UsagePeriod::Unplaced);
        assert_eq!(report.periods[0].total_tokens, report.total_tokens);
        assert_eq!(report.periods[0].models, report.models);
        assert_eq!(report.details, TokenDetails::default());
        assert_eq!(report.estimated_value_micros, None);
        assert!(parse(&tool.id, &fixture(&tool.id, "daily")).is_err());
    }
    assert_eq!(unplaced.len(), 12);
    for id in ["claude", "codex", "not-a-tool"] {
        assert!(parse_agent_periods(id, b"{}").is_err());
    }
}

#[test]
fn gaps_and_model_less_usage_never_become_model_totals_or_reasoning() {
    let mut raw = fixture("pi", "daily");
    raw["daily"][0]["agents"][0]["totalTokens"] = 173.into();
    sync_day(&mut raw);
    let report = parse("pi", &raw).unwrap().unwrap();
    assert_eq!(report.total_tokens, 173);
    assert_eq!(report.models[0].total_tokens, None);
    assert_eq!(report.periods[0].models[0].total_tokens, None);
    assert_eq!(report.periods[0].details.reasoning_tokens, None);

    raw["daily"][0]["agents"][0]["modelBreakdowns"] = json!([]);
    raw["daily"][0]["agents"][0]["modelsUsed"] = json!([]);
    sync_day(&mut raw);
    let report = parse("pi", &raw).unwrap().unwrap();
    assert_eq!(report.total_tokens, 173);
    assert!(report.models.is_empty());
    assert!(report.periods[0].models.is_empty());
}

#[test]
fn multiple_models_and_days_keep_complete_and_incomplete_scopes_separate() {
    let mut raw = fixture("pi", "daily");
    let mut model = raw["daily"][0]["agents"][0]["modelBreakdowns"][0].clone();
    model["modelName"] = "[pi] second-model".into();
    for (key, a, b) in [
        ("inputTokens", 100, 20),
        ("outputTokens", 20, 10),
        ("cacheCreationTokens", 4, 1),
        ("cacheReadTokens", 6, 1),
    ] {
        raw["daily"][0]["agents"][0]["modelBreakdowns"][0][key] = a.into();
        model[key] = b.into();
    }
    raw["daily"][0]["agents"][0]["modelBreakdowns"]
        .as_array_mut()
        .unwrap()
        .push(model);
    raw["daily"][0]["agents"][0]["modelsUsed"]
        .as_array_mut()
        .unwrap()
        .push(json!("[pi] second-model"));
    sync_day(&mut raw);
    let report = parse("pi", &raw).unwrap().unwrap();
    assert_eq!(
        report
            .models
            .iter()
            .map(|m| m.total_tokens)
            .collect::<Vec<_>>(),
        [Some(130), Some(32)]
    );

    let mut second = raw["daily"][0].clone();
    second["period"] = "2026-11-09".into();
    raw["daily"].as_array_mut().unwrap().push(second);
    for key in [
        "inputTokens",
        "outputTokens",
        "cacheCreationTokens",
        "cacheReadTokens",
        "totalTokens",
    ] {
        raw["totals"][key] = (raw["totals"][key].as_u64().unwrap() * 2).into();
    }
    raw["totals"]["totalCost"] = (raw["totals"]["totalCost"].as_f64().unwrap() * 2.0).into();
    let report = parse("pi", &raw).unwrap().unwrap();
    assert_eq!(report.total_tokens, 324);
    assert_eq!(
        report
            .models
            .iter()
            .map(|m| m.total_tokens)
            .collect::<Vec<_>>(),
        [Some(260), Some(64)]
    );

    raw["daily"][1]["totalTokens"] = 173.into();
    raw["daily"][1]["agents"][0]["totalTokens"] = 173.into();
    raw["totals"]["totalTokens"] = 335.into();
    let report = parse("pi", &raw).unwrap().unwrap();
    assert_eq!(report.total_tokens, 335);
    assert!(report.models.iter().all(|m| m.total_tokens.is_none()));
    assert!(
        report.periods[0]
            .models
            .iter()
            .all(|m| m.total_tokens.is_some())
    );
    assert!(
        report.periods[1]
            .models
            .iter()
            .all(|m| m.total_tokens.is_none())
    );
}

#[test]
fn empty_and_reported_zero_are_distinct_and_missing_fields_are_errors() {
    let empty = json!({"daily":[],"totals":{
        "inputTokens":0,"outputTokens":0,"cacheCreationTokens":0,"cacheReadTokens":0,
        "totalTokens":0,"totalCost":0
    }});
    for id in ["amp", "pi", "kilo", "zcode"] {
        assert_eq!(parse(id, &empty).unwrap(), None);
        let mut raw = fixture(id, "daily");
        for key in [
            "inputTokens",
            "outputTokens",
            "cacheCreationTokens",
            "cacheReadTokens",
            "totalTokens",
            "totalCost",
        ] {
            raw["daily"][0]["agents"][0][key] = 0.into();
        }
        raw["daily"][0]["agents"][0]["modelBreakdowns"] = json!([]);
        raw["daily"][0]["agents"][0]["modelsUsed"] = json!([]);
        sync_day(&mut raw);
        let report = parse(id, &raw).unwrap().unwrap();
        assert_eq!(report.total_tokens, 0);
        assert_eq!(report.periods.len(), 1);
        assert_eq!(report.details, TokenDetails::default());
        assert!(parse(id, &json!({"daily":[],"totals":null})).is_err());
        assert!(parse(id, &json!({"daily":[]})).is_err());
    }
    assert_eq!(
        parse("qwen", &json!({"sessions":[],"totals":null})).unwrap(),
        None
    );
}

#[test]
fn unknown_model_keeps_tokens_without_an_estimate() {
    let mut raw = fixture("pi", "daily");
    raw["daily"][0]["agents"][0]["modelBreakdowns"][0]["modelName"] = "[pi] no-price-model".into();
    raw["daily"][0]["agents"][0]["modelBreakdowns"][0]["missingPricing"] = true.into();
    raw["daily"][0]["agents"][0]["modelsUsed"] = json!(["[pi] no-price-model"]);
    sync_day(&mut raw);
    raw["totals"]["unpricedModels"] = json!(["[pi] no-price-model"]);
    let report = parse("pi", &raw).unwrap().unwrap();
    assert_eq!(report.total_tokens, 162);
    assert_eq!(report.models[0].model, "no-price-model");
    assert_eq!(report.models[0].total_tokens, Some(162));
    assert_eq!(report.models[0].estimated_value_micros, None);
}

#[test]
fn malformed_reports_reject_the_whole_result() {
    let changes: &[fn(&mut Value)] = &[
        |v| v["extra"] = true.into(),
        |v| v["daily"][0]["extra"] = true.into(),
        |v| v["totals"]["extra"] = true.into(),
        |v| v["daily"][0]["metadata"]["extra"] = true.into(),
        |v| v["daily"][0]["agents"][0]["extra"] = true.into(),
        |v| v["daily"][0]["agents"][0]["modelBreakdowns"][0]["extra"] = true.into(),
        |v| v["daily"][0]["agent"] = "pi".into(),
        |v| v["daily"][0]["agents"][0]["agent"] = "amp".into(),
        |v| v["daily"][0]["metadata"]["agents"] = json!(["pi", "pi"]),
        |v| v["daily"][0]["agents"] = json!([]),
        |v| v["daily"][0]["period"] = "2026-02-29".into(),
        |v| v["daily"][0]["period"] = "2026-02-30".into(),
        |v| v["daily"][0]["period"] = "2026-11-0é".into(),
        |v| v["daily"][0]["period"] = "2026-11-08T00:00:00Z".into(),
        |v| {
            let day = v["daily"][0].clone();
            v["daily"].as_array_mut().unwrap().push(day);
        },
        |v| v["daily"][0]["inputTokens"] = 121.into(),
        |v| v["totals"]["totalTokens"] = 163.into(),
        |v| v["totals"]["inputTokens"] = 121.into(),
        |v| v["totals"]["totalCost"] = (-1).into(),
        |v| v["totals"]["unpricedModels"] = json!(["[pi] absent"]),
        |v| v["daily"][0]["agents"][0]["inputTokens"] = (-1).into(),
        |v| v["daily"][0]["agents"][0]["inputTokens"] = json!(1.5),
        |v| v["daily"][0]["agents"][0]["inputTokens"] = "120".into(),
        |v| v["daily"][0]["agents"][0]["inputTokens"] = Value::Null,
        |v| {
            v["daily"][0]["agents"][0]["totalTokens"] = 161.into();
            sync_day(v);
        },
        |v| {
            v["daily"][0]["agents"][0]["totalTokens"] = u64::MAX.into();
            sync_day(v);
        },
        |v| {
            v["daily"][0]["agents"][0]["totalCost"] = (-1).into();
            sync_day(v);
        },
        |v| {
            v["daily"][0]["agents"][0]["modelBreakdowns"][0]["cost"] = (-1).into();
            sync_day(v);
        },
        |v| {
            v["daily"][0]["agents"][0]["modelBreakdowns"][0]["inputTokens"] = 121.into();
            sync_day(v);
        },
        |v| {
            v["daily"][0]["agents"][0]["modelsUsed"] = json!(["[pi] other"]);
            sync_day(v);
        },
        |v| {
            let m = v["daily"][0]["agents"][0]["modelBreakdowns"][0].clone();
            v["daily"][0]["agents"][0]["modelBreakdowns"]
                .as_array_mut()
                .unwrap()
                .push(m);
            sync_day(v);
        },
        |v| {
            v["daily"][0]["agents"][0]["modelBreakdowns"][0]["modelName"] = "missing-prefix".into();
            v["daily"][0]["agents"][0]["modelsUsed"] = json!(["missing-prefix"]);
            sync_day(v);
        },
    ];
    for (i, change) in changes.iter().enumerate() {
        let mut raw = fixture("pi", "daily");
        change(&mut raw);
        assert!(parse("pi", &raw).is_err(), "case {i}");
    }
}
