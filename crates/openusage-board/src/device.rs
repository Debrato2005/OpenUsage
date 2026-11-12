use crate::{BoardClient, Error, GithubToken, protocol::SecretText, transport::form};
use serde::{Deserialize, Deserializer};
use std::{fmt, time::Duration};
use zeroize::Zeroize;

const VERIFICATION_URI: &str = "https://github.com/login/device";
const MAX_EXPIRY: u32 = 900;
const MAX_INTERVAL: u32 = 900;

pub struct DeviceFlow {
    client_id: String,
    device_code: SecretText,
    user_code: String,
    expires_at: Duration,
    interval: Duration,
    retry_at: Duration,
    last_time: Duration,
    finished: bool,
}
impl fmt::Debug for DeviceFlow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DeviceFlow")
            .field("expires_at", &self.expires_at)
            .field("retry_at", &self.retry_at)
            .field("finished", &self.finished)
            .finish()
    }
}
impl DeviceFlow {
    pub fn user_code(&self) -> &str {
        &self.user_code
    }
    pub fn verification_uri(&self) -> &'static str {
        VERIFICATION_URI
    }
    pub fn expires_at(&self) -> Duration {
        self.expires_at
    }
    pub fn retry_at(&self) -> Duration {
        self.retry_at
    }
    pub fn interval(&self) -> Duration {
        self.interval
    }
    fn finish(&mut self) {
        self.finished = true;
        self.device_code.0.zeroize();
        self.user_code.zeroize();
    }
}
impl Drop for DeviceFlow {
    fn drop(&mut self) {
        self.finish();
    }
}
#[derive(Debug)]
pub enum PollOutcome {
    Authorized(GithubToken),
    Pending { retry_at: Duration },
    SlowDown { retry_at: Duration },
    Denied,
    Expired,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StartReply {
    device_code: SecretText,
    user_code: String,
    verification_uri: String,
    expires_in: u32,
    interval: u32,
}
#[derive(Default)]
enum Field<T> {
    #[default]
    Missing,
    Present(T),
}
impl<T> Field<T> {
    fn is_missing(&self) -> bool {
        matches!(self, Self::Missing)
    }
}
impl<'de, T: Deserialize<'de>> Deserialize<'de> for Field<T> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        T::deserialize(d).map(Self::Present)
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PollReply {
    #[serde(default)]
    access_token: Field<SecretText>,
    #[serde(default)]
    token_type: Field<SecretText>,
    #[serde(default)]
    scope: Field<SecretText>,
    #[serde(default)]
    error: Field<SecretText>,
    #[serde(default)]
    error_description: Field<SecretText>,
    #[serde(default)]
    error_uri: Field<SecretText>,
    #[serde(default)]
    interval: Field<u32>,
}

impl BoardClient {
    /// `now` is an elapsed monotonic clock value, shared with every later poll.
    /// The caller must start this flow explicitly. No browser opens automatically.
    pub fn device_start(&self, now: Duration) -> Result<DeviceFlow, Error> {
        let transport = self.transport()?;
        let (reply, ok): (StartReply, bool) = transport.oauth_json(
            transport.github_device.clone(),
            form(&[("client_id", &transport.client_id)]),
            transport.request_timeout,
        )?;
        if !ok
            || reply.device_code.0.len() != 40
            || !reply
                .device_code
                .0
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
            || reply.user_code.len() != 9
            || !reply.user_code.bytes().enumerate().all(|(i, b)| {
                if i == 4 {
                    b == b'-'
                } else {
                    b.is_ascii_uppercase() || b.is_ascii_digit()
                }
            })
            || reply.verification_uri != VERIFICATION_URI
            || !(1..=MAX_EXPIRY).contains(&reply.expires_in)
            || !(1..=MAX_INTERVAL).contains(&reply.interval)
            || reply.interval > reply.expires_in
        {
            return Err(Error::UnexpectedResponse);
        }
        let interval = Duration::from_secs(reply.interval.into());
        Ok(DeviceFlow {
            client_id: transport.client_id.clone(),
            device_code: reply.device_code,
            user_code: reply.user_code,
            expires_at: now
                .checked_add(Duration::from_secs(reply.expires_in.into()))
                .ok_or(Error::InvalidRequest)?,
            retry_at: now.checked_add(interval).ok_or(Error::InvalidRequest)?,
            interval,
            last_time: now,
            finished: false,
        })
    }
    /// Each call makes at most one HTTP request. Early, expired and finished flows make none.
    pub fn device_poll(&self, flow: &mut DeviceFlow, now: Duration) -> Result<PollOutcome, Error> {
        let transport = self.transport()?;
        if flow.client_id != transport.client_id {
            return Err(Error::InvalidRequest);
        }
        if now < flow.last_time {
            return Err(Error::ClockWentBackwards);
        }
        flow.last_time = now;
        if flow.finished {
            return Err(Error::FlowFinished);
        }
        if now >= flow.expires_at {
            flow.finish();
            return Ok(PollOutcome::Expired);
        }
        if now < flow.retry_at {
            return Err(Error::PollTooEarly {
                retry_at: flow.retry_at,
            });
        }
        // Reserve the next interval before I/O, including failed and timed-out requests.
        flow.retry_at = now
            .checked_add(flow.interval)
            .ok_or(Error::InvalidRequest)?;
        let response: Result<(PollReply, bool), Error> = transport.oauth_json(
            transport.github_poll.clone(),
            form(&[
                ("client_id", &transport.client_id),
                ("device_code", &flow.device_code.0),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ]),
            flow.expires_at - now,
        );
        let (reply, ok) = match response {
            Ok(reply) => reply,
            Err(error) => {
                if !matches!(error, Error::Transport | Error::Timeout) {
                    flow.finish();
                }
                return Err(error);
            }
        };
        if let Field::Present(error) = reply.error {
            if !reply.access_token.is_missing()
                || !reply.token_type.is_missing()
                || !reply.scope.is_missing()
                || error.0.len() > 64
                || matches!(&reply.error_description, Field::Present(s) if s.0.len() > 1024)
                || matches!(&reply.error_uri, Field::Present(s) if s.0.len() > 256)
            {
                flow.finish();
                return Err(Error::UnexpectedResponse);
            }
            match error.0.as_str() {
                "authorization_pending" if reply.interval.is_missing() => {
                    Ok(PollOutcome::Pending {
                        retry_at: flow.retry_at,
                    })
                }
                "slow_down" => {
                    let minimum = flow
                        .interval
                        .checked_add(Duration::from_secs(5))
                        .ok_or(Error::UnexpectedResponse)?;
                    let supplied = match reply.interval {
                        Field::Missing => Duration::ZERO,
                        Field::Present(v) => Duration::from_secs(v.into()),
                    };
                    let interval = minimum.max(supplied);
                    if interval > Duration::from_secs(MAX_INTERVAL.into()) {
                        flow.finish();
                        return Err(Error::UnexpectedResponse);
                    }
                    flow.interval = interval;
                    flow.retry_at = now.checked_add(interval).ok_or(Error::UnexpectedResponse)?;
                    Ok(PollOutcome::SlowDown {
                        retry_at: flow.retry_at,
                    })
                }
                "access_denied" if reply.interval.is_missing() => {
                    flow.finish();
                    Ok(PollOutcome::Denied)
                }
                "expired_token" | "token_expired" if reply.interval.is_missing() => {
                    flow.finish();
                    Ok(PollOutcome::Expired)
                }
                "unsupported_grant_type"
                | "incorrect_client_credentials"
                | "incorrect_device_code"
                | "device_flow_disabled" => {
                    flow.finish();
                    Err(Error::GithubRefused)
                }
                _ => {
                    flow.finish();
                    Err(Error::UnexpectedResponse)
                }
            }
        } else {
            flow.finish();
            match (reply.access_token, reply.token_type, reply.scope) {
                (Field::Present(token), Field::Present(kind), Field::Present(scope))
                    if ok
                        && kind.0.as_str() == "bearer"
                        && scope.0.is_empty()
                        && reply.interval.is_missing()
                        && reply.error_description.is_missing()
                        && reply.error_uri.is_missing() =>
                {
                    GithubToken::from_secret(token.0)
                        .map(PollOutcome::Authorized)
                        .map_err(|_| Error::UnexpectedResponse)
                }
                _ => Err(Error::UnexpectedResponse),
            }
        }
    }
}
