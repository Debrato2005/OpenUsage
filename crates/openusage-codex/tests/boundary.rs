use openusage_codex::{
    AccountAuth, IdentityConfidence, LoginFlow, LoginState, MAX_MESSAGE_BYTES, ProtocolError,
    Request, initialized_notification, parse_account, parse_account_usage, parse_initialize,
    parse_login_start,
};
use serde_json::{Value, json};
use std::path::Path;

const ACCOUNT: &[u8] = include_bytes!("fixtures/rust-v0.163.0/synthetic-account.json");
const LOGIN: &[u8] = include_bytes!("fixtures/rust-v0.163.0/synthetic-login-browser.json");
const COMPLETED: &[u8] = include_bytes!("fixtures/rust-v0.163.0/synthetic-login-completed.json");
const USAGE: &[u8] = include_bytes!("fixtures/rust-v0.163.0/synthetic-usage.json");

#[test]
fn requests_allow_only_documented_account_operations_without_token_or_thread_parameters() {
    let challenge = parse_login_start(LOGIN, 3, LoginFlow::Browser).unwrap();
    let requests = [
        Request::initialize(1),
        Request::account_read(2),
        Request::login_start(3, LoginFlow::Browser),
        Request::login_start(4, LoginFlow::DeviceCode),
        challenge.attempt.cancel_request(5).unwrap(),
        Request::rate_limits_read(6),
        Request::account_usage_read(7),
        Request::logout(8, true).unwrap(),
    ];
    let messages: Vec<Value> = requests
        .iter()
        .map(|request| {
            let line = request.to_line().unwrap();
            assert_eq!(line.last(), Some(&b'\n'));
            serde_json::from_slice(&line).unwrap()
        })
        .collect();
    assert_eq!(
        messages
            .iter()
            .map(|value| value["method"].as_str().unwrap())
            .collect::<Vec<_>>(),
        [
            "initialize",
            "account/read",
            "account/login/start",
            "account/login/start",
            "account/login/cancel",
            "account/rateLimits/read",
            "account/usage/read",
            "account/logout",
        ]
    );
    assert_eq!(
        messages[0]["params"]["capabilities"]["experimentalApi"],
        false
    );
    assert_eq!(messages[1]["params"], json!({"refreshToken":false}));
    assert_eq!(messages[2]["params"], json!({"type":"chatgpt"}));
    assert_eq!(messages[3]["params"], json!({"type":"chatgptDeviceCode"}));
    assert_eq!(
        messages[4]["params"]["loginId"],
        "135fe507-c848-4537-9355-0db2942d7f9d"
    );
    assert_eq!(messages[6]["params"], json!({}));
    assert!(matches!(
        Request::logout(8, false),
        Err(ProtocolError::ConfirmationRequired)
    ));
    let initialized: Value = serde_json::from_slice(initialized_notification()).unwrap();
    assert_eq!(initialized, json!({"method":"initialized","params":{}}));
}

#[test]
fn descriptive_account_fields_never_establish_verified_identity() {
    let account = parse_account(ACCOUNT, 2).unwrap();
    assert_eq!(account.authentication, AccountAuth::Chatgpt);
    assert_eq!(account.identity, IdentityConfidence::Provisional);
    assert_eq!(account.email.as_deref(), Some("synthetic@example.invalid"));
    assert_eq!(account.plan.as_deref(), Some("pro"));

    let no_email = br#"{"id":2,"result":{"account":{"type":"chatgpt","email":null,"planType":"plus"},"requiresOpenaiAuth":true}}"#;
    assert_eq!(
        parse_account(no_email, 2).unwrap().identity,
        IdentityConfidence::Unknown
    );
    let api_key = br#"{"id":2,"result":{"account":{"type":"apiKey"},"requiresOpenaiAuth":true}}"#;
    let unsupported = parse_account(api_key, 2).unwrap();
    assert_eq!(unsupported.authentication, AccountAuth::Unsupported);
    assert_eq!(unsupported.identity, IdentityConfidence::Unknown);
    assert!(unsupported.email.is_none());
    let missing = br#"{"id":2,"result":{"account":null,"requiresOpenaiAuth":false}}"#;
    assert_eq!(
        parse_account(missing, 2).unwrap().authentication,
        AccountAuth::AuthenticationRequired
    );
}

#[test]
fn malformed_or_mismatched_provider_responses_fail_with_redacted_errors() {
    assert!(matches!(
        parse_account(ACCOUNT, 9),
        Err(ProtocolError::MismatchedRequest)
    ));
    assert!(matches!(
        parse_account(&vec![b' '; MAX_MESSAGE_BYTES + 1], 2),
        Err(ProtocolError::OversizedMessage)
    ));
    for malformed in [
        br#"{"id":2,"id":2,"result":{"account":null,"requiresOpenaiAuth":true}}"#.as_slice(),
        br#"{"id":2,"result":{"account":null,"requiresOpenaiAuth":"true"}}"#,
        br#"{"id":2,"result":{"account":null,"requiresOpenaiAuth":true},"error":{"code":1,"message":"secret"}}"#,
        br#"{"id":2,"result":null}"#,
    ] {
        assert!(matches!(parse_account(malformed, 2), Err(ProtocolError::InvalidMessage)));
    }
    let unsupported = br#"{"id":2,"error":{"code":-32601,"message":"synthetic-secret","data":{"token":"synthetic-token"}}}"#;
    assert!(matches!(
        parse_account(unsupported, 2),
        Err(ProtocolError::UnsupportedMethod)
    ));
    let failure = br#"{"id":2,"error":{"code":401,"message":"synthetic-secret"}}"#;
    let error = parse_account(failure, 2).err().unwrap();
    assert_eq!(error, ProtocolError::ProviderFailure);
    assert!(!error.to_string().contains("synthetic-secret"));
}

#[test]
fn login_completion_matches_the_selected_attempt_and_still_requires_account_read() {
    let mut challenge = parse_login_start(LOGIN, 3, LoginFlow::Browser).unwrap();
    let other = br#"{"method":"account/login/completed","params":{"loginId":"other-synthetic-login","success":true,"error":null}}"#;
    assert_eq!(
        challenge.attempt.apply_completion(other),
        Err(ProtocolError::MismatchedLogin)
    );
    assert_eq!(challenge.attempt.state(), LoginState::Pending);
    assert_eq!(
        challenge.attempt.apply_completion(COMPLETED),
        Ok(LoginState::NeedsAccountRead)
    );
    assert!(matches!(
        challenge.attempt.cancel_request(5),
        Err(ProtocolError::LoginNotPending)
    ));
    assert_eq!(
        challenge.attempt.apply_completion(COMPLETED),
        Err(ProtocolError::LoginNotPending)
    );

    let mut failed = parse_login_start(LOGIN, 3, LoginFlow::Browser).unwrap();
    let failure = br#"{"method":"account/login/completed","params":{"loginId":"135fe507-c848-4537-9355-0db2942d7f9d","success":false,"error":"synthetic-secret"}}"#;
    assert_eq!(
        failed.attempt.apply_completion(failure),
        Ok(LoginState::Failed)
    );
}

#[test]
fn login_prompts_reject_unapproved_destinations_and_auth_modes() {
    for destination in [
        "javascript:alert(1)",
        "http://auth.openai.com/",
        "https://auth.openai.com.evil.invalid/",
        "https://user:password@auth.openai.com/",
        "https://auth.openai.com:8443/",
        "https://127.0.0.1/",
    ] {
        let response = serde_json::to_vec(&json!({"id":3,"result":{"type":"chatgpt","loginId":"synthetic-login","authUrl":destination}})).unwrap();
        assert!(matches!(
            parse_login_start(&response, 3, LoginFlow::Browser),
            Err(ProtocolError::UnapprovedLoginUrl)
        ));
    }
    assert!(matches!(
        parse_login_start(LOGIN, 3, LoginFlow::DeviceCode),
        Err(ProtocolError::UnsupportedAuthentication)
    ));
    let external = br#"{"id":3,"result":{"type":"chatgptAuthTokens"}}"#;
    assert!(matches!(
        parse_login_start(external, 3, LoginFlow::Browser),
        Err(ProtocolError::UnsupportedAuthentication)
    ));
    let device = br#"{"id":4,"result":{"type":"chatgptDeviceCode","loginId":"synthetic-device","verificationUrl":"https://auth.openai.com/codex/device","userCode":"ABCD-1234"}}"#;
    assert!(parse_login_start(device, 4, LoginFlow::DeviceCode).is_ok());
}

#[test]
fn exact_activity_preserves_nulls_dates_and_counts_above_javascript_integer_precision() {
    let usage = parse_account_usage(USAGE, 7).unwrap();
    let json = serde_json::to_value(&usage).unwrap();
    assert_eq!(json["summary"]["lifetimeTokens"], "9007199254740993");
    assert_eq!(json["summary"]["peakDailyTokens"], "9223372036854775807");
    assert_eq!(json["summary"]["longestRunningTurnSec"], Value::Null);
    assert_eq!(json["summary"]["currentStreakDays"], "0");
    assert_eq!(json["dailyUsageBuckets"][0]["startDate"], "2024-02-29");
    assert_eq!(json["dailyUsageBuckets"][0]["tokens"], "9007199254740993");
    let mut value: Value = serde_json::from_slice(USAGE).unwrap();
    value["result"]["dailyUsageBuckets"] = Value::Null;
    let summary_only = parse_account_usage(&serde_json::to_vec(&value).unwrap(), 7).unwrap();
    assert!(summary_only.daily_usage_buckets.is_none());
    assert!(summary_only.summary.lifetime_tokens.is_some());
    for invalid in [
        json!(-1),
        json!(1.5),
        json!("9007199254740993"),
        json!(9223372036854775808_u64),
    ] {
        value["result"]["summary"]["lifetimeTokens"] = invalid;
        assert!(parse_account_usage(&serde_json::to_vec(&value).unwrap(), 7).is_err());
    }
    let mut value: Value = serde_json::from_slice(USAGE).unwrap();
    for date in [
        "2023-02-29",
        "2024-02-30",
        "0000-01-01",
        "2024-01-01T00:00:00Z",
    ] {
        value["result"]["dailyUsageBuckets"][0]["startDate"] = json!(date);
        assert!(matches!(
            parse_account_usage(&serde_json::to_vec(&value).unwrap(), 7),
            Err(ProtocolError::InvalidField)
        ));
    }
    let mut value: Value = serde_json::from_slice(USAGE).unwrap();
    value["result"]["threadUsage"] = json!({"estimated":true});
    assert!(matches!(
        parse_account_usage(&serde_json::to_vec(&value).unwrap(), 7),
        Err(ProtocolError::IncompatibleUsageScope)
    ));
}

#[test]
fn initialization_requires_the_selected_absolute_home_without_reading_credential_files() {
    let response = br#"{"id":1,"result":{"userAgent":"synthetic-provider","codexHome":"/synthetic/selected-home","platformFamily":"unix","platformOs":"linux"}}"#;
    assert!(parse_initialize(response, 1, Path::new("/synthetic/selected-home")).is_ok());
    assert!(matches!(
        parse_initialize(response, 1, Path::new("/synthetic/other-home")),
        Err(ProtocolError::UnexpectedHome)
    ));
    assert!(matches!(
        parse_initialize(response, 1, Path::new("relative")),
        Err(ProtocolError::UnexpectedHome)
    ));
}
