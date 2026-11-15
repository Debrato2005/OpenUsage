//! Synthetic parser evidence only. Storage, identity, and native acceptance remain separate.
use openusage_codex::{
    EvidenceLocation, MAX_MESSAGE_BYTES, ProtocolError, parse_account_usage, parse_quota,
};
use serde_json::{Value, json};

fn message(result: Value) -> Vec<u8> {
    serde_json::to_vec(&json!({"id":7,"result":result})).unwrap()
}

#[test]
fn quota_evidence_redacts_free_text_and_identifiers_without_changing_display_fields() {
    let input = json!({"accountId":"synthetic-private-account", "rateLimitsByLimitId":{
        "synthetic-private-limit":{"limitId":"synthetic-private-limit",
            "limitName":"synthetic-private-label","rateLimitReachedType":"synthetic-private-kind",
            "primary":{"usedPercent":25,"windowDurationMins":300,"resetsAt":0},
            "individualLimit":{"limit":"synthetic-private-monthly"},"spendControlReached":false}},
        "rateLimitResetCredits":{"availableCount":i64::MAX,"credits":[{
            "title":"synthetic-private-credit","status":"available","grantedAt":0,"expiresAt":null}]}});
    let parsed = parse_quota(&message(input.clone()), 7).unwrap();
    let evidence = parsed.response_evidence();
    let encoded = serde_json::to_value(evidence).unwrap();
    assert!(!format!("{encoded} {evidence:?}").contains("synthetic-private"));
    assert!(evidence.unused_fields().is_empty());
    assert_eq!(
        serde_json::to_value(&parsed).unwrap(),
        json!({
            "limits":[{"limitId":"synthetic-private-limit","limitName":"synthetic-private-label",
                "primary":{"usedPercent":25,"derivedRemainingPercent":75,"windowDurationMins":300,"resetsAtUnix":0},
                "secondary":null,"reachedType":"synthetic-private-kind",
                "monthlyCreditLimit":"synthetic-private-monthly","spendControlReached":false}],
            "availableResetCredits":i64::MAX.to_string(),
            "resetCreditDetails":[{"title":"synthetic-private-credit","status":"available","grantedAtUnix":0,"expiresAtUnix":null}]
        })
    );
    let changed = String::from_utf8(message(input))
        .unwrap()
        .replace("synthetic-private", "other-private");
    let changed = parse_quota(changed.as_bytes(), 7).unwrap();
    assert_eq!(
        evidence.redacted_response_hash(),
        changed.response_evidence().redacted_response_hash()
    );
    assert_eq!(
        evidence.format_fingerprint(),
        changed.response_evidence().format_fingerprint()
    );
}

#[test]
fn accepted_positional_structs_keep_values_and_wire_container_evidence() {
    let object = parse_account_usage(&message(json!({"summary":{"lifetimeTokens":3}})), 7).unwrap();
    let array = parse_account_usage(br#"[7,[[3,null,null,null,null],null,null],null]"#, 7).unwrap();
    assert_eq!(
        serde_json::to_value(&object).unwrap(),
        serde_json::to_value(&array).unwrap()
    );
    let changed =
        parse_account_usage(br#"[7,[[4,null,null,null,null],null,null],null]"#, 7).unwrap();
    assert_ne!(
        array.response_evidence().redacted_response_hash(),
        changed.response_evidence().redacted_response_hash()
    );
    assert_ne!(
        array.response_evidence().format_fingerprint(),
        object.response_evidence().format_fingerprint()
    );

    let array = parse_quota(br#"[7,[null,{"synthetic":[null,null,[3,300,0],null,null,["private"],false]},null,[7,[[null,"available",0,null]]]],null]"#, 7).unwrap();
    let changed = parse_quota(br#"[7,[null,{"synthetic":[null,null,[4,300,0],null,null,["private"],false]},null,[7,[[null,"available",0,null]]]],null]"#, 7).unwrap();
    assert_ne!(
        array.response_evidence().redacted_response_hash(),
        changed.response_evidence().redacted_response_hash()
    );
    assert_eq!(
        array.response_evidence().format_fingerprint(),
        changed.response_evidence().format_fingerprint()
    );
}

#[test]
fn activity_evidence_preserves_wire_presence_and_exact_values_outside_display_serialization() {
    let input = json!({"summary":{"lifetimeTokens":9007199254740993_i64,
        "peakDailyTokens":i64::MAX,"currentStreakDays":0},
        "dailyUsageBuckets":[{"startDate":"2024-02-29","tokens":9007199254740993_i64}]});
    let parsed = parse_account_usage(&message(input.clone()), 7).unwrap();
    let evidence = parsed.response_evidence();
    assert_eq!(evidence.projection_version(), 1);
    for digest in [
        evidence.redacted_response_hash(),
        evidence.format_fingerprint(),
    ] {
        assert_eq!(digest.len(), 64);
        assert!(digest.bytes().all(|byte| byte.is_ascii_hexdigit()));
    }
    assert!(evidence.unused_fields().is_empty());
    assert_eq!(
        serde_json::to_value(&parsed).unwrap(),
        json!({
            "summary":{"lifetimeTokens":"9007199254740993","peakDailyTokens":i64::MAX.to_string(),
                "longestRunningTurnSec":null,"currentStreakDays":"0","longestStreakDays":null},
            "dailyUsageBuckets":[{"startDate":"2024-02-29","tokens":"9007199254740993"}],
            "scope":"account_activity_with_unknown_application_scope"
        })
    );

    let mut explicit_null = input.clone();
    explicit_null["summary"]["longestRunningTurnSec"] = Value::Null;
    let null = parse_account_usage(&message(explicit_null), 7).unwrap();
    assert_eq!(
        serde_json::to_value(&parsed).unwrap(),
        serde_json::to_value(&null).unwrap()
    );
    assert_ne!(
        evidence.redacted_response_hash(),
        null.response_evidence().redacted_response_hash()
    );
    assert_ne!(
        evidence.format_fingerprint(),
        null.response_evidence().format_fingerprint()
    );

    let mut next_count = input;
    next_count["summary"]["lifetimeTokens"] = json!(9007199254740994_i64);
    let next = parse_account_usage(&message(next_count), 7).unwrap();
    assert_ne!(
        evidence.redacted_response_hash(),
        next.response_evidence().redacted_response_hash()
    );
    assert_eq!(
        evidence.format_fingerprint(),
        next.response_evidence().format_fingerprint()
    );
}

#[test]
fn version_one_hashes_match_independent_sha256_examples() {
    // Python hashlib SHA-256 over literal UTF-8 canonical projections, without a newline:
    // {"projectionVersion":1,"response":{"id":{"redacted":"integer"},"result":{"summary":{}}},"responseKind":"account_usage"}
    // {"format":{"id":"integer","result":{"summary":{}}},"projectionVersion":1,"responseKind":"account_usage","unusedFields":[]}
    let activity = parse_account_usage(br#"{"id":7,"result":{"summary":{}}}"#, 7).unwrap();
    assert_eq!(
        activity.response_evidence().redacted_response_hash(),
        "83fc8277837d9e7043488d0f64b18e3555c4d4c5e0024af6311e98b4583e69cc"
    );
    assert_eq!(
        activity.response_evidence().format_fingerprint(),
        "2cb9f6f20a95692fcd063ba8bc48e2fa34fd7886fcff15dc999b4c1b7a719ac7"
    );
    // {"projectionVersion":1,"response":{"id":{"redacted":"integer"},"result":{"rateLimitsByLimitId":{"entries":[]}}},"responseKind":"quota"}
    // {"format":{"id":"integer","result":{"rateLimitsByLimitId":{"entries":[]}}},"projectionVersion":1,"responseKind":"quota","unusedFields":[]}
    let quota = parse_quota(br#"{"id":7,"result":{"rateLimitsByLimitId":{}}}"#, 7).unwrap();
    assert_eq!(
        quota.response_evidence().redacted_response_hash(),
        "74fadc1fe91667a7211924e9e653631484ddd21cc4c0044379e0ef4be2c76e43"
    );
    assert_eq!(
        quota.response_evidence().format_fingerprint(),
        "09707f9ad8c13c2952d3ac9f3d813771cb7c76adf912c4c001e0127e42ee5442"
    );
}

#[test]
fn activity_unused_additions_are_counted_at_fixed_locations_without_retaining_private_content() {
    let clean = json!({"summary":{"lifetimeTokens":3},
        "dailyUsageBuckets":[{"startDate":"2024-02-29","tokens":3}]});
    let baseline = parse_account_usage(&message(clean.clone()), 7).unwrap();
    let mut input = clean;
    input["synthetic-private-email@example.test"] =
        json!({"secretToken":"synthetic-private-token"});
    input["summary"]["/synthetic/private/full/path"] = json!("synthetic-private-prompt");
    input["dailyUsageBuckets"][0]["synthetic-private-cookie"] = json!("synthetic-private-code");
    let parsed = parse_account_usage(&message(input.clone()), 7).unwrap();
    let evidence = parsed.response_evidence();
    assert_eq!(
        serde_json::to_value(&baseline).unwrap(),
        serde_json::to_value(&parsed).unwrap()
    );
    assert_eq!(
        baseline.response_evidence().redacted_response_hash(),
        evidence.redacted_response_hash()
    );
    assert_ne!(
        baseline.response_evidence().format_fingerprint(),
        evidence.format_fingerprint()
    );
    assert_eq!(
        evidence
            .unused_fields()
            .iter()
            .map(|change| (change.location(), change.field_count()))
            .collect::<Vec<_>>(),
        vec![
            (EvidenceLocation::ActivityResult, 1),
            (EvidenceLocation::ActivitySummary, 1),
            (EvidenceLocation::ActivityDailyBucket, 1)
        ]
    );
    let retained = format!("{} {evidence:?}", serde_json::to_string(evidence).unwrap());
    for private in ["synthetic-private", "/synthetic/private", "secretToken"] {
        assert!(!retained.contains(private));
    }

    // Same-count renames and payload changes deliberately give no new private fingerprint.
    let changed = String::from_utf8(message(input))
        .unwrap()
        .replace("synthetic-private", "different-private");
    let changed = parse_account_usage(changed.as_bytes(), 7).unwrap();
    assert_eq!(
        serde_json::to_value(evidence).unwrap(),
        serde_json::to_value(changed.response_evidence()).unwrap()
    );
}

#[test]
fn quota_unused_additions_record_every_known_parent_with_fixed_size_output() {
    let clean = json!({"rateLimitsByLimitId":{"synthetic-private-id":{
        "primary":{"usedPercent":1},"individualLimit":{"limit":"private-string"}}},
        "rateLimitResetCredits":{"availableCount":0,"credits":[{"status":"available","grantedAt":0,"expiresAt":null}]}});
    let baseline = parse_quota(&message(clean.clone()), 7).unwrap();
    let mut input = clean;
    for pointer in [
        "",
        "/rateLimitsByLimitId/synthetic-private-id",
        "/rateLimitsByLimitId/synthetic-private-id/primary",
        "/rateLimitsByLimitId/synthetic-private-id/individualLimit",
        "/rateLimitResetCredits",
        "/rateLimitResetCredits/credits/0",
    ] {
        input.pointer_mut(pointer).unwrap()["/synthetic/private/token@example.test"] =
            json!({"private-code":"private-prompt"});
    }
    let parsed = parse_quota(&message(input), 7).unwrap();
    let evidence = parsed.response_evidence();
    assert_eq!(
        serde_json::to_value(&baseline).unwrap(),
        serde_json::to_value(&parsed).unwrap()
    );
    assert_eq!(
        baseline.response_evidence().redacted_response_hash(),
        evidence.redacted_response_hash()
    );
    assert_ne!(
        baseline.response_evidence().format_fingerprint(),
        evidence.format_fingerprint()
    );
    assert_eq!(
        evidence
            .unused_fields()
            .iter()
            .map(|change| (change.location(), change.field_count()))
            .collect::<Vec<_>>(),
        vec![
            (EvidenceLocation::QuotaResult, 1),
            (EvidenceLocation::QuotaLimit, 1),
            (EvidenceLocation::QuotaWindow, 1),
            (EvidenceLocation::QuotaIndividualLimit, 1),
            (EvidenceLocation::QuotaResetCredits, 1),
            (EvidenceLocation::QuotaCreditDetail, 1)
        ]
    );
    let encoded = serde_json::to_string(evidence).unwrap();
    assert!(encoded.len() < 1024);
    assert!(!format!("{encoded} {evidence:?}").contains("private"));
}

#[test]
fn key_order_request_ids_and_dynamic_limit_names_do_not_affect_evidence() {
    let first = parse_account_usage(br#"{"id":7,"result":{"summary":{"lifetimeTokens":3,"peakDailyTokens":1},"dailyUsageBuckets":null}}"#, 7).unwrap();
    let second = parse_account_usage(br#"{"result":{"dailyUsageBuckets":null,"summary":{"peakDailyTokens":1,"lifetimeTokens":3}},"id":9}"#, 9).unwrap();
    assert_eq!(
        serde_json::to_value(first.response_evidence()).unwrap(),
        serde_json::to_value(second.response_evidence()).unwrap()
    );
    let first = parse_quota(br#"{"id":7,"result":{"rateLimitsByLimitId":{"aaa":{"primary":{"usedPercent":10}},"zzz":{"secondary":{"usedPercent":20}}}}}"#, 7).unwrap();
    // Renaming keys also reverses their order. Sorting by private IDs would change this hash.
    let second = parse_quota(br#"{"result":{"rateLimitsByLimitId":{"bbb":{"secondary":{"usedPercent":20}},"yyy":{"primary":{"usedPercent":10}}}},"id":9}"#, 9).unwrap();
    assert_eq!(
        serde_json::to_value(first.response_evidence()).unwrap(),
        serde_json::to_value(second.response_evidence()).unwrap()
    );
}

#[test]
fn ignored_huge_numbers_and_deep_values_keep_acceptance_and_only_count_field_occurrences() {
    let nested = format!("{}1e9999{}", "[".repeat(512), "]".repeat(512));
    for (input, activity) in [
        (
            format!(
                r#"{{"id":7,"result":{{"summary":{{"ignored":1e9999}},"ignored":{nested},"ignored":true}}}}"#
            ),
            true,
        ),
        (
            format!(
                r#"{{"id":7,"result":{{"rateLimitsByLimitId":{{"synthetic":{{"ignored":1e9999}}}},"ignored":{nested},"ignored":true}}}}"#
            ),
            false,
        ),
    ] {
        let evidence = if activity {
            let parsed = parse_account_usage(input.as_bytes(), 7).unwrap();
            serde_json::to_value(parsed.response_evidence()).unwrap()
        } else {
            let parsed = parse_quota(input.as_bytes(), 7).unwrap();
            serde_json::to_value(parsed.response_evidence()).unwrap()
        };
        assert_eq!(evidence["unusedFields"][0]["fieldCount"], 2);
        assert_eq!(evidence["unusedFields"][1]["fieldCount"], 1);
        assert!(!evidence.to_string().contains("ignored"));
    }
}

#[test]
fn evidence_remains_bounded_when_many_unused_fields_are_added() {
    let mut result = json!({"summary":{}});
    for index in 0..10_000 {
        result[format!("synthetic-private-{index}")] = json!("synthetic-private-payload");
    }
    let input = message(result);
    assert!(input.len() < MAX_MESSAGE_BYTES);
    let parsed = parse_account_usage(&input, 7).unwrap();
    let evidence = parsed.response_evidence();
    assert_eq!(evidence.unused_fields().len(), 1);
    assert_eq!(evidence.unused_fields()[0].field_count(), 10_000);
    assert!(serde_json::to_vec(evidence).unwrap().len() < 512);
}

#[test]
fn unused_value_types_and_unselected_legacy_values_do_not_create_private_fingerprints() {
    let mut previous = None;
    for value in [
        Value::Null,
        json!(true),
        json!(42),
        json!("private"),
        json!(["private"]),
        json!({"private":"private"}),
    ] {
        let activity =
            parse_account_usage(&message(json!({"summary":{},"private-key":value})), 7).unwrap();
        let evidence = serde_json::to_value(activity.response_evidence()).unwrap();
        if let Some(previous) = &previous {
            assert_eq!(previous, &evidence);
        }
        previous = Some(evidence);
    }
    let first = parse_quota(
        &message(
            json!({"rateLimitsByLimitId":{},"rateLimits":{"primary":{"usedPercent":"private"}}}),
        ),
        7,
    )
    .unwrap();
    let second = parse_quota(
        &message(json!({"rateLimitsByLimitId":{},"rateLimits":{"/private/path":{"private":123}}})),
        7,
    )
    .unwrap();
    assert_eq!(
        serde_json::to_value(first.response_evidence()).unwrap(),
        serde_json::to_value(second.response_evidence()).unwrap()
    );
}

#[test]
fn quota_exact_counts_nulls_enum_values_and_activity_dates_keep_distinct_response_hashes() {
    let input = json!({"rateLimitsByLimitId":{"synthetic":{"primary":{"usedPercent":0}}},
        "rateLimitResetCredits":{"availableCount":i64::MAX,"credits":[{"status":"available","grantedAt":0,"expiresAt":null}]}});
    let baseline = parse_quota(&message(input.clone()), 7).unwrap();
    let evidence = baseline.response_evidence();
    for (pointer, value) in [
        ("/rateLimitResetCredits/availableCount", json!(i64::MAX - 1)),
        ("/rateLimitResetCredits/credits/0/status", json!("redeemed")),
        (
            "/rateLimitsByLimitId/synthetic/primary/usedPercent",
            json!(1),
        ),
    ] {
        let mut changed = input.clone();
        *changed.pointer_mut(pointer).unwrap() = value;
        let changed = parse_quota(&message(changed), 7).unwrap();
        assert_ne!(
            evidence.redacted_response_hash(),
            changed.response_evidence().redacted_response_hash()
        );
        assert_eq!(
            evidence.format_fingerprint(),
            changed.response_evidence().format_fingerprint()
        );
    }
    let mut changed = input.clone();
    changed["rateLimitsByLimitId"]["synthetic"]["primary"]["resetsAt"] = Value::Null;
    let changed = parse_quota(&message(changed), 7).unwrap();
    assert_eq!(
        serde_json::to_value(&baseline).unwrap(),
        serde_json::to_value(&changed).unwrap()
    );
    assert_ne!(
        evidence.redacted_response_hash(),
        changed.response_evidence().redacted_response_hash()
    );
    assert_ne!(
        evidence.format_fingerprint(),
        changed.response_evidence().format_fingerprint()
    );

    let mut first = input.clone();
    let mut second = input;
    first["rateLimitResetCredits"]["credits"][0]["status"] = json!("synthetic-private-first");
    second["rateLimitResetCredits"]["credits"][0]["status"] = json!("synthetic-private-second");
    let first = parse_quota(&message(first), 7).unwrap();
    let second = parse_quota(&message(second), 7).unwrap();
    assert_eq!(
        serde_json::to_value(first.response_evidence()).unwrap(),
        serde_json::to_value(second.response_evidence()).unwrap()
    );

    let first = parse_account_usage(
        &message(json!({"summary":{},"dailyUsageBuckets":[{"startDate":"2024-02-28","tokens":0}]})),
        7,
    )
    .unwrap();
    let second = parse_account_usage(
        &message(json!({"summary":{},"dailyUsageBuckets":[{"startDate":"2024-02-29","tokens":0}]})),
        7,
    )
    .unwrap();
    assert_ne!(
        first.response_evidence().redacted_response_hash(),
        second.response_evidence().redacted_response_hash()
    );
    assert_eq!(
        first.response_evidence().format_fingerprint(),
        second.response_evidence().format_fingerprint()
    );
}

#[test]
fn accepted_unit_enum_objects_preserve_status_and_redact_unknown_variant_names() {
    let parse = |status: Value| {
        parse_quota(&message(json!({"rateLimitsByLimitId":{},
        "rateLimitResetCredits":{"availableCount":0,"credits":[{"status":status,"grantedAt":0,"expiresAt":null}]}})), 7).unwrap()
    };
    let available = parse(json!({"available":null}));
    let redeemed = parse(json!({"redeemed":null}));
    assert_eq!(
        serde_json::to_value(&available).unwrap()["resetCreditDetails"][0]["status"],
        "available"
    );
    assert_eq!(
        serde_json::to_value(&redeemed).unwrap()["resetCreditDetails"][0]["status"],
        "redeemed"
    );
    assert_ne!(
        available.response_evidence().redacted_response_hash(),
        redeemed.response_evidence().redacted_response_hash()
    );
    assert_eq!(
        available.response_evidence().format_fingerprint(),
        redeemed.response_evidence().format_fingerprint()
    );
    let text = parse(json!("available"));
    assert_ne!(
        available.response_evidence().format_fingerprint(),
        text.response_evidence().format_fingerprint()
    );
    let unknown = parse(json!({"synthetic-private-first":null}));
    let renamed = parse(json!({"synthetic-private-second":null}));
    assert_eq!(
        serde_json::to_value(unknown.response_evidence()).unwrap(),
        serde_json::to_value(renamed.response_evidence()).unwrap()
    );
    assert!(
        !format!(
            "{} {:?}",
            serde_json::to_string(unknown.response_evidence()).unwrap(),
            unknown.response_evidence()
        )
        .contains("synthetic-private")
    );
}

#[test]
fn used_field_drift_and_strict_envelopes_still_fail_without_evidence() {
    for input in [
        json!({"summary":{"lifetimeTokens":"3"}}),
        json!({"summary":{"lifetimeTokens":-1}}),
        json!({"summary":{},"dailyUsageBuckets":[{"startDate":"private@example.test","tokens":3}]}),
        json!({"summary":{},"threadUsage":{"tokens":3}}),
    ] {
        assert!(parse_account_usage(&message(input), 7).is_err());
    }
    for input in [
        json!({"rateLimitsByLimitId":{"synthetic":{"primary":{"usedPercent":"3"}}}}),
        json!({"rateLimitsByLimitId":{"synthetic":{"primary":{"usedPercent":101}}}}),
        json!({"rateLimitsByLimitId":{"synthetic":{"individualLimit":{"limit":3}}}}),
        json!({"rateLimitsByLimitId":{"synthetic":{"spendControlReached":"true"}}}),
        json!({"rateLimitsByLimitId":{},"rateLimitResetCredits":{"availableCount":0,"credits":[{"status":"available","grantedAt":0}]}}),
    ] {
        assert!(parse_quota(&message(input), 7).is_err());
    }
    assert!(matches!(
        parse_account_usage(br#"{"id":7,"result":{"summary":{}},"private":"token"}"#, 7),
        Err(ProtocolError::InvalidMessage)
    ));
    assert!(matches!(
        parse_quota(
            br#"{"id":7,"result":{"rateLimitsByLimitId":{}},"private":"token"}"#,
            7
        ),
        Err(ProtocolError::InvalidMessage)
    ));
    assert!(matches!(
        parse_account_usage(&message(json!({"summary":{}})), 8),
        Err(ProtocolError::MismatchedRequest)
    ));
    assert!(matches!(
        parse_quota(
            br#"{"id":7,"result":{"rateLimitsByLimitId":{"same":{},"same":{}}}}"#,
            7
        ),
        Err(ProtocolError::InvalidMessage)
    ));
}
