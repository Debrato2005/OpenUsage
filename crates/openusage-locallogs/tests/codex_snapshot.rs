use openusage_locallogs::{TokenDetails, parse_codex_snapshot};
use serde_json::Value;

fn fixture(name: &str) -> Vec<u8> {
    #[derive(serde::Deserialize)]
    struct Case {
        snapshot: Box<serde_json::value::RawValue>,
    }
    #[derive(serde::Deserialize)]
    struct Reports {
        cases: std::collections::BTreeMap<String, Case>,
    }
    let reports: Reports =
        serde_json::from_slice(include_bytes!("fixtures/codex-snapshot-candidate.json")).unwrap();
    reports.cases[name].snapshot.get().as_bytes().to_vec()
}

#[test]
fn unattributed_tokens_keep_reported_categories_without_a_model_or_estimate() {
    let result = parse_codex_snapshot(&fixture("unattributed")).unwrap();
    assert_eq!((result.periods.len(), result.sessions.len()), (1, 1));
    let period = &result.periods[0];
    let session = &result.sessions[0];
    assert_eq!((period.total_tokens, session.total_tokens), (17, 17));
    let details = TokenDetails {
        input_tokens: Some(17),
        output_tokens: Some(0),
        cache_write_tokens: Some(0),
        cache_read_tokens: Some(0),
        reasoning_tokens: Some(0),
    };
    assert_eq!(period.details, details);
    assert_eq!(session.details, details);
    assert!(period.models.is_empty() && session.models.is_empty());
    assert_eq!(period.estimated_value_micros, None);
    assert_eq!(session.estimated_value_micros, None);
    assert_eq!(session.session_key, "thread-a");
    assert_eq!(session.first_activity_utc, None);
    assert_eq!(session.project, None);
    assert_eq!(
        session.last_activity_utc.as_deref(),
        Some("2026-11-11T12:00:00.000Z")
    );
}

#[test]
fn known_models_keep_exact_counts_and_the_reported_estimate() {
    let result = parse_codex_snapshot(&fixture("known")).unwrap();
    assert_eq!(result.periods[0].estimated_value_micros, Some(21));
    assert_eq!(result.sessions[0].estimated_value_micros, Some(21));
    for model in [&result.periods[0].models[0], &result.sessions[0].models[0]] {
        assert_eq!(model.model, "gpt-5");
        assert_eq!(model.total_tokens, Some(17));
        assert_eq!(model.details.input_tokens, Some(17));
        assert_eq!(model.details.reasoning_tokens, Some(0));
        assert_eq!(model.estimated_value_micros, None);
    }
}

#[test]
fn missing_prices_and_partial_attribution_keep_unavailable_values() {
    let unknown = parse_codex_snapshot(&fixture("unknown_model")).unwrap();
    assert_eq!(unknown.periods[0].total_tokens, 17);
    assert_eq!(unknown.periods[0].estimated_value_micros, None);
    assert_eq!(unknown.sessions[0].estimated_value_micros, None);
    assert_eq!(unknown.periods[0].models[0].total_tokens, Some(17));
    assert_eq!(
        unknown.periods[0].models[0].model,
        "openusage-unknown-codex-model"
    );
    let mixed = parse_codex_snapshot(&fixture("mixed_attribution")).unwrap();
    assert_eq!(mixed.periods[0].total_tokens, 40);
    assert_eq!(mixed.periods[0].estimated_value_micros, None);
    assert_eq!(mixed.periods[0].models[0].total_tokens, None);
    assert_eq!(mixed.periods[0].models[0].details.input_tokens, Some(23));
    assert!(mixed.sessions[0].models.is_empty());
    assert_eq!(mixed.sessions[1].models[0].total_tokens, Some(23));
    assert_eq!(mixed.sessions[1].estimated_value_micros, Some(29));
}

#[test]
fn all_recorded_captures_preserve_independent_totals_and_zero_observations() {
    for (name, total, days, sessions) in [
        ("known", 17, 1, 1),
        ("unattributed", 17, 1, 1),
        ("unknown_model", 17, 1, 1),
        ("zero", 0, 1, 1),
        ("zero_unattributed", 0, 1, 1),
        ("empty", 0, 0, 0),
        ("prefix", 17, 1, 1),
        ("mixed_attribution", 40, 1, 2),
        ("two_days", 40, 2, 2),
        ("lineage", 12650, 1, 3),
        ("rounding_partition", 802, 1, 5),
    ] {
        let result = parse_codex_snapshot(&fixture(name)).unwrap();
        assert_eq!(
            (result.periods.len(), result.sessions.len()),
            (days, sessions),
            "{name}"
        );
        assert_eq!(
            result
                .periods
                .iter()
                .map(|row| row.total_tokens)
                .sum::<u64>(),
            total,
            "{name}"
        );
        assert_eq!(
            result
                .sessions
                .iter()
                .map(|row| row.total_tokens)
                .sum::<u64>(),
            total,
            "{name}"
        );
        assert!(
            result
                .sessions
                .iter()
                .all(|row| row.project.is_none() && row.first_activity_utc.is_none())
        );
    }
    let zero = parse_codex_snapshot(&fixture("zero")).unwrap();
    assert_eq!(zero.periods[0].models[0].total_tokens, Some(0));
    assert_eq!(zero.periods[0].estimated_value_micros, Some(0));
    let unknown_zero = parse_codex_snapshot(&fixture("zero_unattributed")).unwrap();
    assert!(unknown_zero.periods[0].models.is_empty());
    assert_eq!(unknown_zero.periods[0].details.input_tokens, Some(0));
    assert_eq!(unknown_zero.periods[0].estimated_value_micros, None);
    let days = parse_codex_snapshot(&fixture("two_days")).unwrap();
    assert_eq!(
        days.periods[0].period,
        openusage_locallogs::UsagePeriod::Day("2026-11-11".into())
    );
    assert_eq!(
        days.periods[1].period,
        openusage_locallogs::UsagePeriod::Day("2026-11-12".into())
    );
    assert_eq!(
        (days.periods[0].total_tokens, days.periods[1].total_tokens),
        (17, 23)
    );
    let lineage = parse_codex_snapshot(&fixture("lineage")).unwrap();
    assert_eq!(
        lineage.periods[0].details,
        TokenDetails {
            input_tokens: Some(8250),
            output_tokens: Some(1650),
            cache_write_tokens: Some(0),
            cache_read_tokens: Some(2750),
            reasoning_tokens: Some(550),
        }
    );
    assert_eq!(lineage.periods[0].estimated_value_micros, Some(27156));
    for (row, key, total, input, output, read, reasoning, micros, time) in lineage
        .sessions
        .iter()
        .zip([
            (
                "01a1ccf3-2cb4-7cb2-a02e-1563bc53ae57",
                6900,
                4500,
                900,
                1500,
                300,
                14813,
                "2026-11-11T23:30:56.337Z",
            ),
            (
                "01a1ccf3-40d6-7ff3-904c-e8991446e6e2",
                4600,
                3000,
                600,
                1000,
                200,
                9875,
                "2026-11-11T23:30:56.381Z",
            ),
            (
                "01a1ccf3-4101-7620-b501-55d8fd79c6b0",
                1150,
                750,
                150,
                250,
                50,
                2469,
                "2026-11-11T23:30:56.435Z",
            ),
        ])
        .map(
            |(row, (key, total, input, output, read, reasoning, micros, time))| {
                (
                    row, key, total, input, output, read, reasoning, micros, time,
                )
            },
        )
    {
        assert_eq!(row.session_key, key);
        assert_eq!(row.total_tokens, total);
        assert_eq!(
            row.details,
            TokenDetails {
                input_tokens: Some(input),
                output_tokens: Some(output),
                cache_write_tokens: Some(0),
                cache_read_tokens: Some(read),
                reasoning_tokens: Some(reasoning)
            }
        );
        assert_eq!(row.estimated_value_micros, Some(micros));
        assert_eq!(row.last_activity_utc.as_deref(), Some(time));
    }
}

#[test]
fn duplicate_fields_and_model_keys_reject_the_whole_capture() {
    let text = serde_json::to_string(&serde_json::from_slice::<Value>(&fixture("known")).unwrap())
        .unwrap();
    for (from, to) in [
        (
            "\"schemaVersion\":1",
            "\"schemaVersion\":1,\"schemaVersion\":1",
        ),
        (
            "\"costUSD\":0.00002125",
            "\"costUSD\":0.00002125,\"costUSD\":null",
        ),
        (
            "\"totalTokens\":17",
            "\"totalTokens\":17,\"totalTokens\":17",
        ),
        (
            "\"isFallback\":false",
            "\"isFallback\":false,\"isFallback\":false",
        ),
    ] {
        let changed = text.replacen(from, to, 1);
        assert_ne!(changed, text, "{from}");
        assert!(parse_codex_snapshot(changed.as_bytes()).is_err(), "{from}");
    }
    let raw: Value = serde_json::from_slice(&fixture("known")).unwrap();
    let model = serde_json::to_string(&raw["daily"]["daily"][0]["models"]["gpt-5"]).unwrap();
    let key = format!("\"gpt-5\":{model}");
    for occurrence in [1, 2] {
        let position = if occurrence == 1 {
            text.find(&key)
        } else {
            text.rfind(&key)
        }
        .unwrap();
        let mut changed = text.clone();
        changed.insert_str(position, &format!("{key},"));
        assert_ne!(changed, text);
        assert!(
            parse_codex_snapshot(changed.as_bytes()).is_err(),
            "duplicate model {occurrence}"
        );
    }
}

fn edited(name: &str, edit: impl FnOnce(&mut Value)) -> Vec<u8> {
    let mut value = serde_json::from_slice(&fixture(name)).unwrap();
    edit(&mut value);
    serde_json::to_vec(&value).unwrap()
}

#[test]
fn inconsistent_counts_models_metadata_and_sections_reject_the_capture() {
    let changes: &[fn(&mut Value)] = &[
        |v| v["daily"]["totals"]["inputTokens"] = 18.into(),
        |v| v["daily"]["totals"]["totalTokens"] = 18.into(),
        |v| v["daily"]["daily"][0]["models"]["gpt-5"]["totalTokens"] = 16.into(),
        |v| v["daily"]["daily"][0]["models"]["gpt-5"]["inputTokens"] = 18.into(),
        |v| v["daily"]["daily"][0]["models"]["gpt-5"]["reasoningOutputTokens"] = 1.into(),
        |v| v["daily"]["daily"][0]["date"] = "2026-02-30".into(),
        |v| v["session"]["sessions"][0]["lastActivity"] = "2026-11-0é03:04:05.006Z".into(),
        |v| v["session"]["sessions"][0]["sessionId"] = "bad key".into(),
        |v| v["session"]["sessions"][0]["sessionFile"] = "other".into(),
        |v| v["session"]["sessions"][0]["directory"] = "other".into(),
        |v| {
            let model = v["session"]["sessions"][0]["models"]
                .as_object_mut()
                .unwrap()
                .remove("gpt-5")
                .unwrap();
            v["session"]["sessions"][0]["models"]["other-model"] = model;
        },
        |v| {
            for row in ["/session/sessions/0", "/session/totals"] {
                let row = v.pointer_mut(row).unwrap();
                row["inputTokens"] = 16.into();
                row["outputTokens"] = 1.into();
                if let Some(model) = row.pointer_mut("/models/gpt-5") {
                    model["inputTokens"] = 16.into();
                    model["outputTokens"] = 1.into();
                }
            }
        },
        |v| v["daily"]["totals"]["unpricedModels"] = serde_json::json!(["missing"]),
        |v| v["daily"]["daily"][0]["costUSD"] = 100.into(),
        |v| v["daily"]["totals"]["costUSD"] = 100.into(),
        |v| {
            v["session"]["sessions"] = serde_json::json!([]);
        },
    ];
    for (index, change) in changes.iter().enumerate() {
        assert!(
            parse_codex_snapshot(&edited("known", change)).is_err(),
            "case {index}"
        );
    }
    for section in ["daily", "session"] {
        let key = if section == "daily" {
            "daily"
        } else {
            "sessions"
        };
        assert!(
            parse_codex_snapshot(&edited("two_days", |v| {
                v[section][key].as_array_mut().unwrap().reverse();
            }))
            .is_err(),
            "{section} order"
        );
        assert!(
            parse_codex_snapshot(&edited("zero", |v| {
                let row = v[section][key][0].clone();
                v[section][key].as_array_mut().unwrap().push(row);
            }))
            .is_err(),
            "{section} duplicate"
        );
    }
    let empty: Value = serde_json::from_slice(&fixture("empty")).unwrap();
    assert!(
        parse_codex_snapshot(&edited("zero", |v| v["session"] = empty["session"].clone())).is_err()
    );
}

#[test]
fn required_nullable_cost_and_all_strict_struct_fields_reject_missing_or_unknown_values() {
    let base: Value = serde_json::from_slice(&fixture("known")).unwrap();
    for path in [
        "",
        "/daily",
        "/session",
        "/daily/daily/0",
        "/session/sessions/0",
        "/daily/totals",
        "/session/totals",
        "/daily/daily/0/models/gpt-5",
        "/session/sessions/0/models/gpt-5",
    ] {
        let keys = base
            .pointer(path)
            .unwrap()
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        for key in keys {
            assert!(
                parse_codex_snapshot(&edited("known", |v| {
                    v.pointer_mut(path)
                        .unwrap()
                        .as_object_mut()
                        .unwrap()
                        .remove(&key);
                }))
                .is_err(),
                "missing {path}/{key}"
            );
        }
        assert!(
            parse_codex_snapshot(&edited("known", |v| {
                v.pointer_mut(path).unwrap()["extra"] = true.into();
            }))
            .is_err(),
            "unknown {path}"
        );
    }
    for path in [
        "/daily/daily/0/costUSD",
        "/daily/totals/costUSD",
        "/session/sessions/0/costUSD",
        "/session/totals/costUSD",
    ] {
        assert!(
            parse_codex_snapshot(&edited("unattributed", |v| {
                let (parent, key) = path.rsplit_once('/').unwrap();
                v.pointer_mut(parent)
                    .unwrap()
                    .as_object_mut()
                    .unwrap()
                    .remove(key);
            }))
            .is_err(),
            "missing nullable {path}"
        );
        for invalid in [
            serde_json::json!(-1),
            serde_json::json!(1.0e10),
            serde_json::json!("0"),
            serde_json::json!(true),
        ] {
            assert!(
                parse_codex_snapshot(&edited("known", |v| *v.pointer_mut(path).unwrap() = invalid))
                    .is_err(),
                "invalid {path}"
            );
        }
    }
    for path in [
        "/daily/daily/0",
        "/daily/totals",
        "/session/sessions/0",
        "/session/totals",
        "/daily/daily/0/models/gpt-5",
        "/session/sessions/0/models/gpt-5",
    ] {
        for key in [
            "inputTokens",
            "outputTokens",
            "cacheCreationTokens",
            "cacheReadTokens",
            "reasoningOutputTokens",
            "totalTokens",
        ] {
            for invalid in [
                serde_json::json!(-1),
                serde_json::json!(0.5),
                serde_json::json!(1_000_000_000_000_001u64),
                Value::Null,
                serde_json::json!("17"),
            ] {
                assert!(
                    parse_codex_snapshot(
                        &edited("known", |v| v.pointer_mut(path).unwrap()[key] = invalid)
                    )
                    .is_err(),
                    "{path}/{key}"
                );
            }
        }
    }
    for (path, bad) in [
        ("/tool", serde_json::json!("pi")),
        ("/schemaVersion", serde_json::json!(2)),
        ("/schemaVersion", serde_json::json!(1.5)),
        ("/daily", Value::Null),
        ("/session", Value::Null),
    ] {
        assert!(
            parse_codex_snapshot(&edited("known", |v| *v.pointer_mut(path).unwrap() = bad))
                .is_err()
        );
    }
}

#[test]
fn coherent_section_changes_still_require_common_model_and_count_totals() {
    for change in ["count", "reasoning", "attribution", "price", "unpriced"] {
        let bytes = edited("lineage", |v| {
            let section = &mut v["daily"];
            match change {
                "count" => {
                    for path in ["/daily/0", "/daily/0/models/gpt-5", "/totals"] {
                        let row = section.pointer_mut(path).unwrap();
                        row["inputTokens"] = 8251.into();
                        row["totalTokens"] = 12651.into();
                    }
                }
                "reasoning" => {
                    for path in ["/daily/0", "/daily/0/models/gpt-5", "/totals"] {
                        section.pointer_mut(path).unwrap()["reasoningOutputTokens"] = 549.into();
                    }
                }
                "attribution" => {
                    let model = &mut section["daily"][0]["models"]["gpt-5"];
                    model["inputTokens"] = 8249.into();
                    model["totalTokens"] = 12649.into();
                    section["daily"][0]["costUSD"] = Value::Null;
                    section["totals"]["costUSD"] = Value::Null;
                }
                "price" => {
                    section["daily"][0]["costUSD"] = 1.into();
                    section["totals"]["costUSD"] = 1.into();
                }
                "unpriced" => {
                    section["daily"][0]["models"]["gpt-5"]["missingPricing"] = true.into();
                    section["totals"]["unpricedModels"] = serde_json::json!(["gpt-5"]);
                }
                _ => unreachable!(),
            }
        });
        assert!(parse_codex_snapshot(&bytes).is_err(), "{change}");
    }
}

#[test]
fn every_reported_category_and_path_shaped_session_key_stay_exact() {
    let bytes = edited("known", |v| {
        for path in [
            "/daily/daily/0",
            "/daily/daily/0/models/gpt-5",
            "/daily/totals",
            "/session/sessions/0",
            "/session/sessions/0/models/gpt-5",
            "/session/totals",
        ] {
            let row = v.pointer_mut(path).unwrap();
            for (name, count) in [
                ("inputTokens", 7),
                ("outputTokens", 11),
                ("cacheCreationTokens", 3),
                ("cacheReadTokens", 5),
                ("reasoningOutputTokens", 4),
                ("totalTokens", 26),
            ] {
                row[name] = count.into();
            }
        }
        v["session"]["sessions"][0]["sessionId"] = "folder/thread-a".into();
        v["session"]["sessions"][0]["directory"] = "folder".into();
    });
    let result = parse_codex_snapshot(&bytes).unwrap();
    let details = TokenDetails {
        input_tokens: Some(7),
        output_tokens: Some(11),
        cache_write_tokens: Some(3),
        cache_read_tokens: Some(5),
        reasoning_tokens: Some(4),
    };
    assert_eq!(result.periods[0].details, details);
    assert_eq!(result.sessions[0].details, details);
    assert_eq!(result.periods[0].models[0].details, details);
    assert_eq!(result.sessions[0].models[0].details, details);
    assert_eq!(result.sessions[0].session_key, "folder/thread-a");
    assert_eq!(result.sessions[0].project, None);
    assert_eq!(result.sessions[0].first_activity_utc, None);
    assert_eq!(result.periods[0].total_tokens, 26);
}

#[test]
fn duplicate_fields_at_every_typed_depth_reject_even_equal_values() {
    for name in ["known", "unattributed"] {
        let text = serde_json::to_string(&serde_json::from_slice::<Value>(&fixture(name)).unwrap())
            .unwrap();
        let value: Value = serde_json::from_str(&text).unwrap();
        for path in [
            "",
            "/daily",
            "/session",
            "/daily/daily/0",
            "/session/sessions/0",
            "/daily/totals",
            "/session/totals",
            "/daily/daily/0/models/gpt-5",
            "/session/sessions/0/models/gpt-5",
        ] {
            let Some(object) = value.pointer(path) else {
                continue;
            };
            let original = serde_json::to_string(object).unwrap();
            for (key, field) in object.as_object().unwrap() {
                let duplicate = format!(
                    "{{{}:{},{}",
                    serde_json::to_string(key).unwrap(),
                    field,
                    &original[1..]
                );
                let offset = if path.starts_with("/session") {
                    text.rfind(&original)
                } else {
                    text.find(&original)
                }
                .unwrap();
                let mut changed = text.clone();
                changed.replace_range(offset..offset + original.len(), &duplicate);
                assert!(
                    parse_codex_snapshot(changed.as_bytes()).is_err(),
                    "{name} {path}/{key}"
                );
            }
        }
    }
}

#[test]
fn invalid_model_metadata_prices_and_reasoning_subtotals_reject() {
    for name in ["", "bad model", "<script>", &"m".repeat(129)] {
        assert!(
            parse_codex_snapshot(&edited("known", |v| {
                let model = v["daily"]["daily"][0]["models"]
                    .as_object_mut()
                    .unwrap()
                    .remove("gpt-5")
                    .unwrap();
                v["daily"]["daily"][0]["models"][name] = model;
            }))
            .is_err()
        );
    }
    for (path, bad) in [
        ("/daily/daily/0/models/gpt-5/isFallback", Value::Null),
        (
            "/daily/daily/0/models/gpt-5/isFallback",
            serde_json::json!(0),
        ),
        ("/daily/daily/0/models/gpt-5/missingPricing", Value::Null),
        (
            "/daily/totals/unpricedModels",
            serde_json::json!(["gpt-5", "gpt-5"]),
        ),
        ("/daily/totals/unpricedModels", Value::Null),
        (
            "/session/sessions/0/lastActivity",
            serde_json::json!("2026-02-30T12:00:00.000Z"),
        ),
        (
            "/session/sessions/0/sessionId",
            serde_json::json!("a".repeat(257)),
        ),
    ] {
        assert!(
            parse_codex_snapshot(&edited("known", |v| {
                let (parent, key) = path.rsplit_once('/').unwrap();
                v.pointer_mut(parent).unwrap()[key] = bad;
            }))
            .is_err(),
            "{path}"
        );
    }
    assert!(
        parse_codex_snapshot(&edited(
            "mixed_attribution",
            |v| v["daily"]["daily"][0]["costUSD"] = 0.into()
        ))
        .is_err()
    );
    assert!(
        parse_codex_snapshot(&edited("lineage", |v| {
            v["daily"]["daily"][0]["models"]["gpt-5"]["reasoningOutputTokens"] = 549.into();
        }))
        .is_err()
    );
    for path in ["/daily/totals/costUSD", "/session/totals/costUSD"] {
        assert!(
            parse_codex_snapshot(&edited("empty", |v| *v.pointer_mut(path).unwrap() = Value::Null))
                .is_err()
        );
    }
}

#[test]
fn exact_count_limit_accepts_the_boundary_and_refuses_aggregate_overflow() {
    let bytes = edited("known", |v| {
        for path in [
            "/daily/daily/0",
            "/daily/daily/0/models/gpt-5",
            "/daily/totals",
            "/session/sessions/0",
            "/session/sessions/0/models/gpt-5",
            "/session/totals",
        ] {
            let row = v.pointer_mut(path).unwrap();
            row["inputTokens"] = 1_000_000_000_000_000u64.into();
            row["totalTokens"] = 1_000_000_000_000_000u64.into();
        }
    });
    let result = parse_codex_snapshot(&bytes).unwrap();
    assert_eq!(result.periods[0].total_tokens, 1_000_000_000_000_000);
    assert_eq!(
        result.sessions[0].models[0].total_tokens,
        Some(1_000_000_000_000_000)
    );
    let mut value: Value = serde_json::from_slice(&bytes).unwrap();
    let mut next = value["daily"]["daily"][0].clone();
    next["date"] = "2026-11-12".into();
    value["daily"]["daily"].as_array_mut().unwrap().push(next);
    assert!(parse_codex_snapshot(&serde_json::to_vec(&value).unwrap()).is_err());
}

#[test]
fn excessive_rows_and_models_reject_the_whole_capture() {
    let base: Value = serde_json::from_slice(&fixture("zero_unattributed")).unwrap();
    for (section, rows) in [("daily", "daily"), ("session", "sessions")] {
        let row = serde_json::to_string(&base[section][rows][0]).unwrap();
        let original = format!("[{row}]");
        let oversized = format!("[{}{}]", format!("{row},").repeat(200_000), row);
        let text = serde_json::to_string(&base)
            .unwrap()
            .replacen(&original, &oversized, 1);
        assert!(
            matches!(parse_codex_snapshot(text.as_bytes()), Err(openusage_locallogs::ReportError::Shape(message)) if message == "Too many Codex rows")
        );
    }
    let model = r#"{"inputTokens":0,"outputTokens":0,"cacheCreationTokens":0,"cacheReadTokens":0,"reasoningOutputTokens":0,"totalTokens":0,"isFallback":false}"#;
    let mut models = String::from("{");
    for index in 0..200_001 {
        if index != 0 {
            models.push(',');
        }
        models.push_str(&format!("\"model-{index}\":{model}"));
    }
    models.push('}');
    let text = serde_json::to_string(&base).unwrap().replacen(
        "\"models\":{}",
        &format!("\"models\":{models}"),
        1,
    );
    assert!(parse_codex_snapshot(text.as_bytes()).is_err());
}

#[test]
fn actual_partition_rounding_preserves_counts_and_each_rows_own_estimate() {
    let result = parse_codex_snapshot(&fixture("rounding_partition")).unwrap();
    assert_eq!((result.periods.len(), result.sessions.len()), (1, 5));
    assert_eq!(result.periods[0].total_tokens, 802);
    assert_eq!(result.periods[0].estimated_value_micros, Some(1003));
    assert_eq!(
        result
            .sessions
            .iter()
            .map(|row| row.total_tokens)
            .collect::<Vec<_>>(),
        [182, 265, 46, 205, 104]
    );
    assert_eq!(
        result
            .sessions
            .iter()
            .map(|row| row.estimated_value_micros)
            .collect::<Vec<_>>(),
        [Some(228), Some(331), Some(58), Some(256), Some(130)]
    );
    for changed_cost in [
        serde_json::json!(0.001004),
        serde_json::json!(0.001000),
        Value::Null,
    ] {
        assert!(
            parse_codex_snapshot(&edited("rounding_partition", |v| {
                v["daily"]["daily"][0]["costUSD"] = changed_cost.clone();
                v["daily"]["totals"]["costUSD"] = changed_cost;
            }))
            .is_err()
        );
    }
}

#[test]
fn nonfinite_or_non_numeric_cost_literals_reject_before_conversion() {
    let raw: Value = serde_json::from_slice(&fixture("known")).unwrap();
    let text = serde_json::to_string(&raw).unwrap();
    for invalid in [
        "1e999", "-1e999", "NaN", "Infinity", "\"NaN\"", "[]", "{}", "true",
    ] {
        let changed = text.replace("\"costUSD\":0.00002125", &format!("\"costUSD\":{invalid}"));
        assert_ne!(changed, text);
        assert!(
            parse_codex_snapshot(changed.as_bytes()).is_err(),
            "{invalid}"
        );
    }
}
