//! Public API checks with synthetic credentials. No network or OS store access.
use openusage_board::{DeviceId, Enrollment, Error, Joined, ServiceToken};
use serde_json::{Value, json};
use zeroize::Zeroizing;

const ORIGIN: &str = "https://board.openusage.invalid";
const DEVICE: &str = "f9168c5e-ceb2-4faa-b6bf-329bf39fa1e4";
const TOKEN: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn joined() -> Joined {
    Joined {
        login: "synthetic-user".into(),
        token: ServiceToken::from_secret(Zeroizing::new(TOKEN.into())).unwrap(),
        device_id: DeviceId::parse(DEVICE).unwrap(),
    }
}

fn bytes(value: &Value) -> Zeroizing<Vec<u8>> {
    Zeroizing::new(serde_json::to_vec(value).unwrap())
}

fn saved() -> Value {
    json!({
        "schemaVersion": 1,
        "serviceOrigin": format!("{ORIGIN}/"),
        "deviceId": DEVICE,
        "login": "synthetic-user",
        "serviceToken": TOKEN,
    })
}

#[test]
fn saved_enrollment_round_trips_for_only_its_configured_origin() {
    let enrollment = Enrollment::from_joined(ORIGIN, joined()).unwrap();
    assert_eq!(enrollment.service_origin(), format!("{ORIGIN}/"));
    let encoded = enrollment.to_secret().unwrap();
    assert_eq!(serde_json::from_slice::<Value>(&encoded).unwrap(), saved());
    assert!(encoded.len() <= 4096);
    let restored = Enrollment::from_secret(&format!("{ORIGIN}/"), encoded).unwrap();
    assert_eq!(restored.device_id().as_str(), DEVICE);
    assert_eq!(restored.login(), "synthetic-user");
    assert_eq!(restored.token().expose(), TOKEN);
    assert_eq!(
        restored.to_secret().unwrap(),
        enrollment.to_secret().unwrap()
    );
    assert_eq!(
        Enrollment::from_secret(
            "https://other.openusage.invalid",
            restored.to_secret().unwrap()
        )
        .unwrap_err(),
        Error::InvalidRequest
    );
}

#[test]
fn saved_destination_cannot_select_a_service_or_an_alias() {
    for destination in [
        "https://other.openusage.invalid/",
        "http://board.openusage.invalid/",
        ORIGIN,
        "https://board.openusage.invalid:443/",
        "https://board.openusage.invalid@other.openusage.invalid/",
        "https://board.openusage.invalid/v1/",
    ] {
        let mut value = saved();
        value["serviceOrigin"] = json!(destination);
        assert_eq!(
            Enrollment::from_secret(ORIGIN, bytes(&value)).unwrap_err(),
            Error::InvalidRequest
        );
    }
}

#[test]
fn configuration_uses_the_transport_origin_policy_and_a_byte_limit() {
    for origin in [
        "http://board.openusage.invalid",
        "https://user:password@board.openusage.invalid",
        "https://board.openusage.invalid/path",
        "https://board.openusage.invalid?token=synthetic",
        "https://board.openusage.invalid#fragment",
        "https://board.openusage.invalid:443",
        "https://BOARD.openusage.invalid",
        "https://board.openusage.invalid\\path",
        "https://board.openusage.invalid\n",
        &format!("https://{}", "a".repeat(2048)),
    ] {
        assert_eq!(
            Enrollment::from_joined(origin, joined()).unwrap_err(),
            Error::InvalidConfiguration
        );
        assert_eq!(
            Enrollment::from_secret(origin, bytes(&saved())).unwrap_err(),
            Error::InvalidConfiguration
        );
    }
}

#[test]
fn rejects_missing_unknown_duplicate_and_wrongly_typed_fields() {
    let valid = saved();
    for key in [
        "schemaVersion",
        "serviceOrigin",
        "deviceId",
        "login",
        "serviceToken",
    ] {
        let mut missing = valid.clone();
        missing.as_object_mut().unwrap().remove(key);
        assert_eq!(
            Enrollment::from_secret(ORIGIN, bytes(&missing)).unwrap_err(),
            Error::InvalidRequest
        );
        let mut null = valid.clone();
        null[key] = Value::Null;
        assert_eq!(
            Enrollment::from_secret(ORIGIN, bytes(&null)).unwrap_err(),
            Error::InvalidRequest
        );
        let mut duplicate = serde_json::to_string(&valid).unwrap();
        duplicate.pop();
        duplicate.push_str(&format!(",\"{key}\":{}}}", valid[key]));
        assert_eq!(
            Enrollment::from_secret(ORIGIN, Zeroizing::new(duplicate.into_bytes())).unwrap_err(),
            Error::InvalidRequest
        );
    }
    for (key, value) in [
        ("schemaVersion", json!(2)),
        ("schemaVersion", json!(1.0)),
        ("deviceId", json!("../../another-device")),
        ("deviceId", json!("f9168c5e-ceb2-1faa-b6bf-329bf39fa1e4")),
        ("login", json!("private@example.invalid")),
        ("login", json!("synthetic\nuser")),
        ("login", json!("a".repeat(40))),
        ("serviceToken", json!("A".repeat(64))),
        ("serviceToken", json!("a".repeat(63))),
        ("githubToken", json!("synthetic-unexpected-grant")),
        ("refreshToken", json!("synthetic-unexpected-refresh")),
        ("providerAccountId", json!("synthetic-unexpected-id")),
    ] {
        let mut value_with_change = valid.clone();
        value_with_change[key] = value;
        assert_eq!(
            Enrollment::from_secret(ORIGIN, bytes(&value_with_change)).unwrap_err(),
            Error::InvalidRequest
        );
    }
}

#[test]
fn malformed_or_excessive_records_refuse_without_disclosing_contents() {
    for data in [
        Vec::new(),
        vec![b' '; 4097],
        vec![0xff],
        b"{} {}".to_vec(),
        b"null".to_vec(),
        b"[]".to_vec(),
    ] {
        let error = Enrollment::from_secret(ORIGIN, Zeroizing::new(data)).unwrap_err();
        assert_eq!(error, Error::InvalidRequest);
        assert!(!format!("{error:?} {error}").contains(TOKEN));
    }
    let mut within_limit = serde_json::to_vec(&saved()).unwrap();
    within_limit.resize(4096, b' ');
    assert!(Enrollment::from_secret(ORIGIN, Zeroizing::new(within_limit.clone())).is_ok());
    within_limit.push(b' ');
    assert_eq!(
        Enrollment::from_secret(ORIGIN, Zeroizing::new(within_limit)).unwrap_err(),
        Error::InvalidRequest
    );
    let enrollment = Enrollment::from_joined(ORIGIN, joined()).unwrap();
    assert_eq!(format!("{enrollment:?}"), "Enrollment([redacted])");
}

#[test]
fn canonical_records_reject_sequences_escapes_and_non_ascii_before_decoding() {
    let positional = json!([1, format!("{ORIGIN}/"), DEVICE, "synthetic-user", TOKEN]);
    assert_eq!(
        Enrollment::from_secret(ORIGIN, bytes(&positional)).unwrap_err(),
        Error::InvalidRequest
    );
    let ordinary = serde_json::to_string(&saved()).unwrap();
    for text in [
        ordinary.replace(TOKEN, &"\\u0061".repeat(64)),
        ordinary.replace("synthetic-user", "synthetic-\\u0075ser"),
        ordinary.replace("schemaVersion", "schema\\u0056ersion"),
        ordinary.replace("synthetic-user", "synthetic-é"),
    ] {
        assert_eq!(
            Enrollment::from_secret(ORIGIN, Zeroizing::new(text.into_bytes())).unwrap_err(),
            Error::InvalidRequest
        );
    }
}

#[test]
fn encoded_tokens_survive_buffer_boundaries_without_changing_the_schema() {
    let origin = format!("https://{}.invalid", "a".repeat(45));
    let enrollment = Enrollment::from_joined(&origin, joined()).unwrap();
    let secret = enrollment.to_secret().unwrap();
    assert_eq!(secret.len(), 257);
    let restored = Enrollment::from_secret(&origin, secret).unwrap();
    assert_eq!(restored.token().expose(), TOKEN);
}

#[test]
fn joined_identity_is_checked_even_when_constructed_by_another_caller() {
    let mut value = joined();
    value.login = "synthetic@example.invalid".into();
    assert_eq!(
        Enrollment::from_joined(ORIGIN, value).unwrap_err(),
        Error::InvalidRequest
    );
}
