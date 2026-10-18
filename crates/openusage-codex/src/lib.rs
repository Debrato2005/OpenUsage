use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{collections::BTreeSet, fmt, path::Path};
use url::Url;

mod quota;
pub mod session;
pub mod supervisor;
pub use quota::{
    QuotaLimit, QuotaSnapshot, QuotaWindow, ResetCreditDetail, ResetCreditStatus, parse_quota,
};

pub const PROTOCOL_BASELINE: &str = "rust-v0.163.0";
pub const MAX_MESSAGE_BYTES: usize = 1_048_576;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProtocolError {
    OversizedMessage,
    InvalidMessage,
    MismatchedRequest,
    UnsupportedMethod,
    ProviderFailure,
    InvalidField,
    UnexpectedHome,
    UnsupportedAuthentication,
    UnapprovedLoginUrl,
    MismatchedLogin,
    LoginNotPending,
    ConfirmationRequired,
    IncompatibleUsageScope,
    IncompatibleQuotaScope,
    AccountChanged,
    AccountIdentityUnavailable,
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::OversizedMessage => "The provider response exceeds the size limit.",
            Self::InvalidMessage => "The provider response has an invalid shape.",
            Self::MismatchedRequest => "The response does not match the pending request.",
            Self::UnsupportedMethod => "The provider does not support this account operation.",
            Self::ProviderFailure => "The provider account operation failed.",
            Self::InvalidField => "The provider response contains an invalid field.",
            Self::UnexpectedHome => "The provider reports a different profile home.",
            Self::UnsupportedAuthentication => {
                "This authentication mode does not support ChatGPT account monitoring."
            }
            Self::UnapprovedLoginUrl => "The login URL requires a supported official HTTPS domain.",
            Self::MismatchedLogin => "The notification does not match the pending login.",
            Self::LoginNotPending => "This login attempt is no longer pending.",
            Self::ConfirmationRequired => "Confirm the selected account logout before continuing.",
            Self::AccountIdentityUnavailable => {
                "The provider did not return an account description for this read."
            }
            Self::AccountChanged => "The account changed during the read. Try again.",
            Self::IncompatibleQuotaScope => "The provider returned incompatible quota identifiers.",
            Self::IncompatibleUsageScope => {
                "The provider returned estimated thread usage instead of account activity."
            }
        })
    }
}

impl std::error::Error for ProtocolError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoginFlow {
    Browser,
    DeviceCode,
}

#[derive(Serialize)]
pub struct Request {
    id: u32,
    #[serde(flatten)]
    command: Command,
}

#[derive(Serialize)]
#[serde(tag = "method", content = "params")]
enum Command {
    #[serde(rename = "initialize")]
    Initialize {
        #[serde(rename = "clientInfo")]
        client_info: ClientInfo,
        capabilities: Capabilities,
    },
    #[serde(rename = "account/read")]
    AccountRead {
        #[serde(rename = "refreshToken")]
        refresh_token: bool,
    },
    #[serde(rename = "account/login/start")]
    LoginStart(LoginParams),
    #[serde(rename = "account/login/cancel")]
    LoginCancel {
        #[serde(rename = "loginId")]
        login_id: String,
    },
    #[serde(rename = "account/rateLimits/read")]
    RateLimitsRead {},
    #[serde(rename = "account/usage/read")]
    UsageRead {},
    #[serde(rename = "account/logout")]
    Logout {},
}

#[derive(Serialize)]
struct ClientInfo {
    name: &'static str,
    title: &'static str,
    version: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Capabilities {
    experimental_api: bool,
    request_attestation: bool,
}

#[derive(Serialize)]
#[serde(tag = "type")]
enum LoginParams {
    #[serde(rename = "chatgpt")]
    Browser,
    #[serde(rename = "chatgptDeviceCode")]
    DeviceCode,
}

impl Request {
    pub fn initialize(id: u32) -> Self {
        Self {
            id,
            command: Command::Initialize {
                client_info: ClientInfo {
                    name: "openusage",
                    title: "OpenUsage",
                    version: env!("CARGO_PKG_VERSION"),
                },
                capabilities: Capabilities {
                    experimental_api: false,
                    request_attestation: false,
                },
            },
        }
    }

    pub fn account_read(id: u32) -> Self {
        Self {
            id,
            command: Command::AccountRead {
                refresh_token: false,
            },
        }
    }

    pub fn login_start(id: u32, flow: LoginFlow) -> Self {
        let params = match flow {
            LoginFlow::Browser => LoginParams::Browser,
            LoginFlow::DeviceCode => LoginParams::DeviceCode,
        };
        Self {
            id,
            command: Command::LoginStart(params),
        }
    }

    pub fn rate_limits_read(id: u32) -> Self {
        Self {
            id,
            command: Command::RateLimitsRead {},
        }
    }

    pub fn account_usage_read(id: u32) -> Self {
        Self {
            id,
            command: Command::UsageRead {},
        }
    }

    pub fn logout(id: u32, user_confirmed: bool) -> Result<Self, ProtocolError> {
        if !user_confirmed {
            return Err(ProtocolError::ConfirmationRequired);
        }
        Ok(Self {
            id,
            command: Command::Logout {},
        })
    }

    pub fn to_line(&self) -> Result<Vec<u8>, ProtocolError> {
        let mut line = serde_json::to_vec(self).map_err(|_| ProtocolError::InvalidMessage)?;
        line.push(b'\n');
        Ok(line)
    }
}

pub fn initialized_notification() -> &'static [u8] {
    b"{\"method\":\"initialized\",\"params\":{}}\n"
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope<T> {
    id: u32,
    result: Option<T>,
    error: Option<RpcError>,
}

#[derive(Deserialize)]
struct RpcError {
    code: i64,
    #[serde(rename = "message")]
    _message: String,
}

fn decode<T: DeserializeOwned>(bytes: &[u8], expected_id: u32) -> Result<T, ProtocolError> {
    let response: Envelope<T> = decode_bounded(bytes)?;
    if response.id != expected_id {
        return Err(ProtocolError::MismatchedRequest);
    }
    match (response.result, response.error) {
        (Some(result), None) => Ok(result),
        (None, Some(error)) if error.code == -32601 => Err(ProtocolError::UnsupportedMethod),
        (None, Some(_)) => Err(ProtocolError::ProviderFailure),
        _ => Err(ProtocolError::InvalidMessage),
    }
}

fn decode_bounded<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, ProtocolError> {
    if bytes.len() > MAX_MESSAGE_BYTES {
        return Err(ProtocolError::OversizedMessage);
    }
    serde_json::from_slice(bytes).map_err(|_| ProtocolError::InvalidMessage)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InitializeResponse {
    user_agent: String,
    codex_home: String,
    platform_family: String,
    platform_os: String,
}

pub struct ServerInfo {
    pub user_agent: String,
    pub platform_family: String,
    pub platform_os: String,
}

pub fn parse_initialize(
    bytes: &[u8],
    id: u32,
    approved_home: &Path,
) -> Result<ServerInfo, ProtocolError> {
    let response: InitializeResponse = decode(bytes, id)?;
    let reported_home = Path::new(&response.codex_home);
    if !approved_home.is_absolute()
        || !reported_home.is_absolute()
        || approved_home != reported_home
        || reported_home
            .components()
            .any(|part| matches!(part, std::path::Component::ParentDir))
    {
        return Err(ProtocolError::UnexpectedHome);
    }
    validate_text(&response.user_agent, 1024)?;
    validate_text(&response.platform_family, 32)?;
    validate_text(&response.platform_os, 32)?;
    Ok(ServerInfo {
        user_agent: response.user_agent,
        platform_family: response.platform_family,
        platform_os: response.platform_os,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IdentityConfidence {
    Provisional,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccountAuth {
    Chatgpt,
    AuthenticationRequired,
    Unsupported,
}

pub struct AccountSnapshot {
    pub authentication: AccountAuth,
    pub requires_openai_auth: bool,
    pub identity: IdentityConfidence,
    pub email: Option<String>,
    pub plan: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AccountResponse {
    account: Option<AccountWire>,
    requires_openai_auth: bool,
}

#[derive(Deserialize)]
#[serde(tag = "type")]
enum AccountWire {
    #[serde(rename = "chatgpt")]
    Chatgpt {
        email: Option<String>,
        #[serde(rename = "planType")]
        plan_type: String,
    },
    #[serde(other)]
    Unsupported,
}

pub fn parse_account(bytes: &[u8], id: u32) -> Result<AccountSnapshot, ProtocolError> {
    let response: AccountResponse = decode(bytes, id)?;
    let mut snapshot = AccountSnapshot {
        authentication: AccountAuth::AuthenticationRequired,
        requires_openai_auth: response.requires_openai_auth,
        identity: IdentityConfidence::Unknown,
        email: None,
        plan: None,
    };
    match response.account {
        Some(AccountWire::Chatgpt { email, plan_type }) => {
            if let Some(email) = &email {
                validate_text(email, 254)?;
            }
            validate_text(&plan_type, 64)?;
            snapshot.authentication = AccountAuth::Chatgpt;
            snapshot.identity = if email.is_some() {
                IdentityConfidence::Provisional
            } else {
                IdentityConfidence::Unknown
            };
            snapshot.email = email;
            snapshot.plan = Some(plan_type);
        }
        Some(AccountWire::Unsupported) => snapshot.authentication = AccountAuth::Unsupported,
        None => {}
    }
    Ok(snapshot)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoginState {
    Pending,
    NeedsAccountRead,
    Failed,
}

pub struct LoginAttempt {
    login_id: String,
    state: LoginState,
}

pub enum LoginPrompt {
    Browser {
        auth_url: Url,
    },
    DeviceCode {
        verification_url: Url,
        user_code: String,
    },
}

pub struct LoginChallenge {
    pub attempt: LoginAttempt,
    pub prompt: LoginPrompt,
}

/// The selected profile's device prompt. The provider login identifier stays in Rust.
pub struct DeviceLoginPrompt {
    pub verification_url: Url,
    pub user_code: String,
}

impl LoginChallenge {
    pub(crate) fn into_device(self) -> Result<(LoginAttempt, DeviceLoginPrompt), ProtocolError> {
        match self.prompt {
            LoginPrompt::DeviceCode {
                verification_url,
                user_code,
            } => {
                // The pinned managed flow uses the production issuer and this exact route.
                if verification_url.as_str() != "https://auth.openai.com/codex/device" {
                    return Err(ProtocolError::UnapprovedLoginUrl);
                }
                if !user_code.bytes().all(|byte| (0x20..=0x7e).contains(&byte)) {
                    return Err(ProtocolError::InvalidField);
                }
                Ok((
                    self.attempt,
                    DeviceLoginPrompt {
                        verification_url,
                        user_code,
                    },
                ))
            }
            _ => Err(ProtocolError::UnsupportedAuthentication),
        }
    }
}

#[derive(Deserialize)]
#[serde(tag = "type")]
enum LoginResponse {
    #[serde(rename = "chatgpt")]
    Browser {
        #[serde(rename = "loginId")]
        login_id: String,
        #[serde(rename = "authUrl")]
        auth_url: String,
    },
    #[serde(rename = "chatgptDeviceCode")]
    DeviceCode {
        #[serde(rename = "loginId")]
        login_id: String,
        #[serde(rename = "verificationUrl")]
        verification_url: String,
        #[serde(rename = "userCode")]
        user_code: String,
    },
    #[serde(other)]
    Unsupported,
}

pub fn parse_login_start(
    bytes: &[u8],
    id: u32,
    flow: LoginFlow,
) -> Result<LoginChallenge, ProtocolError> {
    let response: LoginResponse = decode(bytes, id)?;
    let (login_id, prompt) = match (flow, response) {
        (LoginFlow::Browser, LoginResponse::Browser { login_id, auth_url }) => (
            login_id,
            LoginPrompt::Browser {
                auth_url: login_url(&auth_url)?,
            },
        ),
        (
            LoginFlow::DeviceCode,
            LoginResponse::DeviceCode {
                login_id,
                verification_url,
                user_code,
            },
        ) => {
            validate_text(&user_code, 64)?;
            if verification_url != "https://auth.openai.com/codex/device" {
                return Err(ProtocolError::UnapprovedLoginUrl);
            }
            (
                login_id,
                LoginPrompt::DeviceCode {
                    verification_url: login_url(&verification_url)?,
                    user_code,
                },
            )
        }
        _ => return Err(ProtocolError::UnsupportedAuthentication),
    };
    validate_text(&login_id, 128)?;
    Ok(LoginChallenge {
        attempt: LoginAttempt {
            login_id,
            state: LoginState::Pending,
        },
        prompt,
    })
}

fn login_url(raw: &str) -> Result<Url, ProtocolError> {
    validate_text(raw, 8192)?;
    let url = Url::parse(raw).map_err(|_| ProtocolError::UnapprovedLoginUrl)?;
    if url.scheme() != "https"
        || !matches!(url.host_str(), Some("auth.openai.com" | "chatgpt.com"))
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
    {
        return Err(ProtocolError::UnapprovedLoginUrl);
    }
    Ok(url)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Notification {
    method: String,
    params: LoginCompleted,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LoginCompleted {
    login_id: Option<String>,
    success: bool,
    error: Option<String>,
}

impl LoginAttempt {
    pub fn state(&self) -> LoginState {
        self.state
    }

    pub fn cancel_request(&self, id: u32) -> Result<Request, ProtocolError> {
        if self.state != LoginState::Pending {
            return Err(ProtocolError::LoginNotPending);
        }
        Ok(Request {
            id,
            command: Command::LoginCancel {
                login_id: self.login_id.clone(),
            },
        })
    }

    pub fn apply_completion(&mut self, bytes: &[u8]) -> Result<LoginState, ProtocolError> {
        let notification: Notification = decode_bounded(bytes)?;
        if notification.method != "account/login/completed" {
            return Err(ProtocolError::InvalidMessage);
        }
        if notification.params.login_id.as_deref() != Some(self.login_id.as_str()) {
            return Err(ProtocolError::MismatchedLogin);
        }
        if self.state != LoginState::Pending {
            return Err(ProtocolError::LoginNotPending);
        }
        if notification.params.success && notification.params.error.is_some() {
            return Err(ProtocolError::InvalidMessage);
        }
        self.state = if notification.params.success {
            LoginState::NeedsAccountRead
        } else {
            LoginState::Failed
        };
        Ok(self.state)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct DecimalCount(String);

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivitySummary {
    pub lifetime_tokens: Option<DecimalCount>,
    pub peak_daily_tokens: Option<DecimalCount>,
    pub longest_running_turn_sec: Option<DecimalCount>,
    pub current_streak_days: Option<DecimalCount>,
    pub longest_streak_days: Option<DecimalCount>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyUsage {
    pub start_date: String,
    pub tokens: DecimalCount,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivityScope {
    AccountActivityWithUnknownApplicationScope,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivitySnapshot {
    pub summary: ActivitySummary,
    pub daily_usage_buckets: Option<Vec<DailyUsage>>,
    pub scope: ActivityScope,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UsageResponse {
    summary: UsageSummaryWire,
    daily_usage_buckets: Option<Vec<DailyUsageWire>>,
    thread_usage: Option<serde_json::Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UsageSummaryWire {
    lifetime_tokens: Option<i64>,
    peak_daily_tokens: Option<i64>,
    longest_running_turn_sec: Option<i64>,
    current_streak_days: Option<i64>,
    longest_streak_days: Option<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DailyUsageWire {
    start_date: String,
    tokens: i64,
}

pub fn parse_account_usage(bytes: &[u8], id: u32) -> Result<ActivitySnapshot, ProtocolError> {
    let response: UsageResponse = decode(bytes, id)?;
    if response.thread_usage.is_some() {
        return Err(ProtocolError::IncompatibleUsageScope);
    }
    let wire = response.summary;
    let summary = ActivitySummary {
        lifetime_tokens: optional_count(wire.lifetime_tokens)?,
        peak_daily_tokens: optional_count(wire.peak_daily_tokens)?,
        longest_running_turn_sec: optional_count(wire.longest_running_turn_sec)?,
        current_streak_days: optional_count(wire.current_streak_days)?,
        longest_streak_days: optional_count(wire.longest_streak_days)?,
    };
    let daily_usage_buckets = response
        .daily_usage_buckets
        .map(|buckets| {
            let mut dates = BTreeSet::new();
            buckets
                .into_iter()
                .map(|bucket| {
                    if !valid_date(&bucket.start_date) || !dates.insert(bucket.start_date.clone()) {
                        return Err(ProtocolError::InvalidField);
                    }
                    Ok(DailyUsage {
                        start_date: bucket.start_date,
                        tokens: count(bucket.tokens)?,
                    })
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?;
    Ok(ActivitySnapshot {
        summary,
        daily_usage_buckets,
        scope: ActivityScope::AccountActivityWithUnknownApplicationScope,
    })
}

fn count(value: i64) -> Result<DecimalCount, ProtocolError> {
    if value < 0 {
        return Err(ProtocolError::InvalidField);
    }
    Ok(DecimalCount(value.to_string()))
}

fn optional_count(value: Option<i64>) -> Result<Option<DecimalCount>, ProtocolError> {
    value.map(count).transpose()
}

fn validate_text(value: &str, max: usize) -> Result<(), ProtocolError> {
    if value.trim().is_empty() || value.len() > max || value.chars().any(char::is_control) {
        return Err(ProtocolError::InvalidField);
    }
    Ok(())
}

fn valid_date(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 10
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || bytes
            .iter()
            .enumerate()
            .any(|(index, byte)| index != 4 && index != 7 && !byte.is_ascii_digit())
    {
        return false;
    }
    let year = value[..4].parse::<u16>().unwrap_or(0);
    let month = value[5..7].parse::<u8>().unwrap_or(0);
    let day = value[8..].parse::<u8>().unwrap_or(0);
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
        2 => 28,
        _ => 0,
    };
    year != 0 && day > 0 && day <= days
}
