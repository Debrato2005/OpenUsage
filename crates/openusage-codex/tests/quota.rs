//! Synthetic contract checks, not live provider or native application acceptance.
use openusage_codex::{ProtocolError, parse_quota};
use serde_json::{Value, json};

const QUOTA: &[u8] = include_bytes!("fixtures/rust-v0.163.0/synthetic-quota.json");
fn message(value: Value) -> Vec<u8> {
    serde_json::to_vec(&json!({"id":3,"result":value})).unwrap()
}
fn window(value: Value) -> Vec<u8> {
    message(json!({"rateLimits":{"limitId":"codex","primary":value}}))
}

#[test]
fn full_quota_preserves_wire_values_and_nulls_without_legacy_double_counting_or_identity_leaks() {
    let snapshot = parse_quota(QUOTA, 3).unwrap();
    assert_eq!(snapshot.limits.len(), 2);
    assert_eq!(snapshot.limits[0].limit_id, "codex");
    let primary = snapshot.limits[0].primary.as_ref().unwrap();
    assert_eq!(
        (primary.used_percent, primary.derived_remaining_percent),
        (25, 75)
    );
    assert_eq!(primary.window_duration_mins, Some(300));
    assert_eq!(primary.resets_at_unix, Some(1893456000));
    let secondary = snapshot.limits[0].secondary.as_ref().unwrap();
    assert_eq!(
        (secondary.used_percent, secondary.derived_remaining_percent),
        (100, 0)
    );
    assert_eq!(secondary.window_duration_mins, Some(10080));
    assert_eq!(secondary.resets_at_unix, None);
    let other = snapshot.limits[1].primary.as_ref().unwrap();
    assert_eq!(
        (other.used_percent, other.derived_remaining_percent),
        (0, 100)
    );
    assert_eq!(other.window_duration_mins, None);
    assert!(snapshot.limits[1].secondary.is_none());
    let encoded = serde_json::to_string(&snapshot).unwrap();
    for private in [
        "synthetic-private-account-id",
        "accountId",
        "email",
        "legacy-must-not-be-counted",
        "rateLimitResetCredits",
    ] {
        assert!(!encoded.contains(private));
        assert!(!format!("{snapshot:?}").contains(private));
    }
}

#[test]
fn authoritative_empty_map_does_not_resurrect_legacy_and_ids_cannot_change_scope() {
    let empty = message(json!({"rateLimitsByLimitId":{},"rateLimits":{"invalid":"legacy"}}));
    assert!(parse_quota(&empty, 3).unwrap().limits.is_empty());
    let fallback = message(
        json!({"rateLimitsByLimitId":null,"rateLimits":{"limitId":"returned-legacy","primary":null,"secondary":null}}),
    );
    let parsed = parse_quota(&fallback, 3).unwrap();
    assert_eq!(parsed.limits[0].limit_id, "returned-legacy");
    assert!(parsed.limits[0].primary.is_none());
    for value in [
        json!({"rateLimits":{"primary":{"usedPercent":25}}}),
        json!({"rateLimitsByLimitId":{"codex":{"limitId":"different"}}}),
    ] {
        assert_eq!(
            parse_quota(&message(value), 3).err(),
            Some(ProtocolError::IncompatibleQuotaScope)
        );
    }
    assert!(parse_quota(&message(json!({})), 3).is_err());
    assert!(parse_quota(QUOTA, 8).is_err());
    let sparse = br#"{"method":"account/rateLimits/updated","params":{"rateLimits":{"limitId":"codex","primary":{"usedPercent":31}}}}"#;
    assert!(parse_quota(sparse, 3).is_err());
}

#[test]
fn quota_rejects_invalid_values_without_rounding_and_preserves_safe_integer_boundaries() {
    for used in [json!(-1), json!(101), json!(25.5), json!("25"), json!(null)] {
        assert!(parse_quota(&window(json!({"usedPercent":used})), 3).is_err());
    }
    for duration in [0, -1, 9_007_199_254_740_992i64] {
        assert!(
            parse_quota(
                &window(json!({"usedPercent":25,"windowDurationMins":duration})),
                3
            )
            .is_err()
        );
    }
    for reset in [-62_135_596_801i64, 253_402_300_800] {
        assert!(parse_quota(&window(json!({"usedPercent":25,"resetsAt":reset})), 3).is_err());
    }
    for reset in [-62_135_596_800i64, 0, 253_402_300_799] {
        let parsed = parse_quota(&window(json!({"usedPercent":25,"windowDurationMins":9_007_199_254_740_991i64,"resetsAt":reset})),3).unwrap();
        let value = parsed.limits[0].primary.as_ref().unwrap();
        assert_eq!(value.resets_at_unix, Some(reset));
        assert_eq!(value.window_duration_mins, Some(9_007_199_254_740_991));
    }
}

#[test]
fn quota_bounds_map_keys_and_text_and_rejects_duplicate_identifiers() {
    let mut limits = serde_json::Map::new();
    for index in 0..65 {
        limits.insert(format!("limit-{index}"), json!({}));
    }
    assert!(parse_quota(&message(json!({"rateLimitsByLimitId":limits})), 3).is_err());
    let duplicates = br#"{"id":3,"result":{"rateLimitsByLimitId":{"codex":{},"codex":{}}}}"#;
    assert!(parse_quota(duplicates, 3).is_err());
    for value in [
        json!({"rateLimits":{"limitId":"x".repeat(129)}}),
        json!({"rateLimits":{"limitId":"codex","limitName":"x".repeat(257)}}),
        json!({"rateLimits":{"limitId":"codex","rateLimitReachedType":"synthetic\nprivate"}}),
        json!({"rateLimitsByLimitId":{"":{}}}),
        json!({"accountId":"x".repeat(257),"rateLimitsByLimitId":{}}),
    ] {
        let error = parse_quota(&message(value), 3).err().unwrap();
        assert!(!format!("{error:?} {error}").contains("synthetic"));
    }
}

#[test]
fn reset_credit_count_is_exact_nullable_and_independent_of_detail_rows() {
    for (summary, expected) in [
        (None, Value::Null),
        (Some(Value::Null), Value::Null),
        (Some(json!({"availableCount":0,"credits":null})), json!("0")),
        (Some(json!({"availableCount":7,"credits":[]})), json!("7")),
        (
            Some(json!({"availableCount":i64::MAX,"credits":null})),
            json!(i64::MAX.to_string()),
        ),
        (
            Some(json!({"availableCount":3,"credits":[{"id":"synthetic-private-credit"}]})),
            json!("3"),
        ),
    ] {
        let mut response = json!({"rateLimitsByLimitId":{}});
        if let Some(summary) = summary {
            response["rateLimitResetCredits"] = summary;
        }
        let snapshot = parse_quota(&message(response), 3).unwrap();
        let encoded = serde_json::to_value(snapshot).unwrap();
        assert_eq!(encoded.get("availableResetCredits"), Some(&expected));
        assert!(!encoded.to_string().contains("synthetic-private-credit"));
    }
}

#[test]
fn reset_credit_summary_requires_a_nonnegative_integer_count_without_detail_fallback() {
    for summary in [
        json!({}),
        json!({"credits":[]}),
        json!({"availableCount":null}),
        json!({"availableCount":-1}),
        json!({"availableCount":1.5}),
        json!({"availableCount":"3"}),
        json!({"availableCount":true}),
        json!({"availableCount":9223372036854775808u64}),
        json!([]),
        json!("3"),
    ] {
        assert!(
            parse_quota(
                &message(json!({"rateLimitsByLimitId":{},"rateLimitResetCredits":summary})),
                3
            )
            .is_err()
        );
    }
}
