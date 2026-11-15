use crate::{
    ApprovedSyncBody, AuthenticatedDeparture, DeviceId, Error, GithubToken, Joined, Metric,
    OwnRank, Period, ServiceToken,
    protocol::{MAX_BODY, SecretText, login},
};
use reqwest::{
    Method, Url,
    blocking::{Body, Client, ClientBuilder},
    header,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{
    io::{Cursor, Read},
    time::Duration,
};
use zeroize::Zeroizing;

pub(crate) const GITHUB_DEVICE: &str = "https://github.com/login/device/code";
pub(crate) const GITHUB_POLL: &str = "https://github.com/login/oauth/access_token";
// A 255-byte token, a 36-byte device ID, and the JSON field syntax.
const MAX_JOIN_BODY: usize = 323;
const MAX_DEPARTURE_BODY: usize = 273;
pub(crate) struct Transport {
    pub origin: Url,
    pub client_id: String,
    pub github_device: Url,
    pub github_poll: Url,
    pub client: Client,
    pub request_timeout: Duration,
}
pub(crate) fn client_builder() -> ClientBuilder {
    Client::builder()
        .tls_backend_native()
        .min_tls_version(reqwest::tls::Version::TLS_1_2)
        .connection_verbose(false)
        .https_only(true)
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .no_gzip()
        .no_brotli()
        .no_zstd()
        .no_deflate()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(15))
        .http1_only()
        .pool_max_idle_per_host(0)
}
impl Transport {
    pub fn production(origin: &str, client_id: &str) -> Result<Self, Error> {
        let origin = validate_origin(origin)?;
        if !(1..=64).contains(&client_id.len())
            || !client_id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        {
            return Err(Error::InvalidConfiguration);
        }
        Ok(Self {
            origin,
            client_id: client_id.into(),
            github_device: Url::parse(GITHUB_DEVICE).map_err(|_| Error::InvalidConfiguration)?,
            github_poll: Url::parse(GITHUB_POLL).map_err(|_| Error::InvalidConfiguration)?,
            request_timeout: Duration::from_secs(15),
            client: client_builder()
                .build()
                .map_err(|_| Error::InvalidConfiguration)?,
        })
    }
    fn endpoint(&self, path: &str) -> Result<Url, Error> {
        self.origin
            .join(path)
            .map_err(|_| Error::InvalidConfiguration)
    }
    pub fn join(&self, github: GithubToken, device_id: &DeviceId) -> Result<Joined, Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Reply {
            token: ServiceToken,
            login: String,
        }
        let body = join_body(&github, device_id)?;
        let reply: Reply = self.json(
            Method::POST,
            self.endpoint("v1/join")?,
            None,
            Some(body),
            "application/json",
        )?;
        if !login(&reply.login) {
            return Err(Error::UnexpectedResponse);
        }
        Ok(Joined {
            login: reply.login,
            token: reply.token,
            device_id: device_id.clone(),
        })
    }
    pub fn sync(&self, token: &ServiceToken, body: &ApprovedSyncBody) -> Result<u16, Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Reply {
            accepted: u16,
        }
        let reply: Reply = self.json(
            Method::POST,
            self.endpoint("v1/sync")?,
            Some(token),
            Some(Zeroizing::new(body.as_bytes().to_vec())),
            "application/json",
        )?;
        if reply.accepted != body.row_count() {
            return Err(Error::UnexpectedResponse);
        }
        Ok(reply.accepted)
    }
    pub fn leave(&self, token: &ServiceToken) -> Result<(), Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Reply {
            left: bool,
        }
        let reply: Reply = self.json(
            Method::POST,
            self.endpoint("v1/leave")?,
            Some(token),
            None,
            "application/json",
        )?;
        if reply.left {
            Ok(())
        } else {
            Err(Error::UnexpectedResponse)
        }
    }
    pub fn leave_with_github(
        &self,
        github: GithubToken,
        local: Option<&ServiceToken>,
    ) -> Result<AuthenticatedDeparture, Error> {
        struct Witness(Option<bool>);
        impl<'de> Deserialize<'de> for Witness {
            fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                struct WitnessVisitor;
                impl<'de> serde::de::Visitor<'de> for WitnessVisitor {
                    type Value = Witness;

                    fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                        f.write_str("a boolean or null")
                    }

                    fn visit_bool<E: serde::de::Error>(self, value: bool) -> Result<Witness, E> {
                        Ok(Witness(Some(value)))
                    }

                    fn visit_unit<E: serde::de::Error>(self) -> Result<Witness, E> {
                        Ok(Witness(None))
                    }
                }
                d.deserialize_any(WitnessVisitor)
            }
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields, rename_all = "camelCase")]
        struct Reply {
            left: bool,
            login: String,
            local_token_revoked: Witness,
        }
        let reply: Reply = self.json(
            Method::POST,
            self.endpoint("v1/leave/github")?,
            local,
            Some(departure_body(&github)?),
            "application/json",
        )?;
        if !reply.left
            || !login(&reply.login)
            || reply.local_token_revoked.0.is_some() != local.is_some()
        {
            return Err(Error::UnexpectedResponse);
        }
        Ok(AuthenticatedDeparture {
            login: reply.login,
            local_token_revoked: reply.local_token_revoked.0,
        })
    }
    pub fn own_rank(
        &self,
        token: &ServiceToken,
        period: Period,
        metric: Metric,
    ) -> Result<OwnRank, Error> {
        let mut url = self.endpoint("v1/me/rank")?;
        url.set_query(Some(&format!(
            "period={}&metric={}",
            period.query(),
            metric.query()
        )));
        let reply: OwnRank = self.json(Method::GET, url, Some(token), None, "application/json")?;
        reply.validate(period, metric)?;
        Ok(reply)
    }
    pub fn json<T: DeserializeOwned>(
        &self,
        method: Method,
        url: Url,
        token: Option<&ServiceToken>,
        body: Option<Zeroizing<Vec<u8>>>,
        content_type: &str,
    ) -> Result<T, Error> {
        self.read_json(method, url, token, body, content_type, None)
            .map(|(value, _)| value)
    }
    pub fn oauth_json<T: DeserializeOwned>(
        &self,
        url: Url,
        body: Zeroizing<Vec<u8>>,
        remaining: Duration,
    ) -> Result<(T, bool), Error> {
        self.read_json(
            Method::POST,
            url,
            None,
            Some(body),
            "application/x-www-form-urlencoded",
            Some(remaining),
        )
    }
    fn read_json<T: DeserializeOwned>(
        &self,
        method: Method,
        url: Url,
        token: Option<&ServiceToken>,
        body: Option<Zeroizing<Vec<u8>>>,
        content_type: &str,
        oauth: Option<Duration>,
    ) -> Result<(T, bool), Error> {
        if body.as_ref().is_some_and(|b| b.len() > MAX_BODY) {
            return Err(Error::RequestTooLarge);
        }
        let mut request = self
            .client
            .request(method, url)
            .timeout(
                self.request_timeout
                    .min(oauth.unwrap_or(self.request_timeout)),
            )
            .header(header::ACCEPT, "application/json")
            .header(header::ACCEPT_ENCODING, "identity")
            .header(header::USER_AGENT, "OpenUsage-Leaderboard-Client")
            .header(header::CONTENT_TYPE, content_type);
        if let Some(token) = token {
            let authorization = Zeroizing::new(format!("Bearer {}", token.expose()));
            let mut value =
                header::HeaderValue::from_str(&authorization).map_err(|_| Error::InvalidRequest)?;
            value.set_sensitive(true);
            request = request.header(header::AUTHORIZATION, value);
        }
        if let Some(body) = body {
            let length = body.len() as u64;
            request = request.body(Body::sized(Cursor::new(body), length));
        }
        let mut response = request.send().map_err(|e| {
            if e.is_timeout() {
                Error::Timeout
            } else {
                Error::Transport
            }
        })?;
        let status = response.status();
        if status.is_redirection() {
            return Err(Error::RedirectRefused);
        }
        if response
            .content_length()
            .is_some_and(|n| n > MAX_BODY as u64)
        {
            return Err(Error::ResponseTooLarge);
        }
        if response
            .headers()
            .get(header::CONTENT_ENCODING)
            .is_some_and(|v| v != "identity")
        {
            return Err(Error::UnexpectedResponse);
        }
        let is_json = response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|h| h.to_str().ok())
            .is_some_and(|h| {
                h.split(';')
                    .next()
                    .is_some_and(|v| v.trim().eq_ignore_ascii_case("application/json"))
            });
        if !is_json {
            return Err(Error::UnexpectedResponse);
        }
        let mut bytes = Zeroizing::new(Vec::with_capacity(MAX_BODY + 1));
        response
            .by_ref()
            .take((MAX_BODY + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::TimedOut
                    || e.get_ref()
                        .and_then(|e| e.downcast_ref::<reqwest::Error>())
                        .is_some_and(reqwest::Error::is_timeout)
                {
                    Error::Timeout
                } else {
                    Error::Transport
                }
            })?;
        if bytes.len() > MAX_BODY {
            return Err(Error::ResponseTooLarge);
        }
        if bytes.iter().find(|byte| !byte.is_ascii_whitespace()) != Some(&b'{') {
            return Err(Error::UnexpectedResponse);
        }
        if status != reqwest::StatusCode::OK
            && !(oauth.is_some() && status == reqwest::StatusCode::BAD_REQUEST)
        {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Failure {
                error: SecretText,
            }
            let failure: Failure =
                serde_json::from_slice(&bytes).map_err(|_| Error::UnexpectedResponse)?;
            return Err(match (status.as_u16(), failure.error.0.as_str()) {
                (401, "not_joined") => Error::NotJoined,
                (401, "github_refused") => Error::GithubRefused,
                (429, "rate_limited") => Error::RateLimited,
                (409, "too_many_devices") => Error::TooManyDevices,
                (500, "unavailable") | (503, "limiter_unavailable") => Error::ServiceUnavailable,
                (400, "invalid_request" | "invalid_json")
                | (404, "not_found")
                | (413, "too_large") => Error::Refused,
                _ => Error::UnexpectedResponse,
            });
        }
        serde_json::from_slice(&bytes)
            .map(|value| (value, status == reqwest::StatusCode::OK))
            .map_err(|_| Error::UnexpectedResponse)
    }
}

fn join_body(github: &GithubToken, device_id: &DeviceId) -> Result<Zeroizing<Vec<u8>>, Error> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Join<'a> {
        github_token: &'a str,
        device_id: &'a str,
    }
    secret_json::<MAX_JOIN_BODY>(&Join {
        github_token: github.expose(),
        device_id: device_id.as_str(),
    })
}
fn departure_body(github: &GithubToken) -> Result<Zeroizing<Vec<u8>>, Error> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Authorization<'a> {
        github_token: &'a str,
    }
    secret_json::<MAX_DEPARTURE_BODY>(&Authorization {
        github_token: github.expose(),
    })
}
fn secret_json<const N: usize>(value: &impl Serialize) -> Result<Zeroizing<Vec<u8>>, Error> {
    let mut buffer = Zeroizing::new([0_u8; N]);
    let length = {
        let mut writer = Cursor::new(&mut buffer[..]);
        serde_json::to_writer(&mut writer, value).map_err(|_| Error::InvalidRequest)?;
        writer.position() as usize
    };
    Ok(Zeroizing::new(buffer[..length].to_vec()))
}

pub(crate) fn validate_origin(text: &str) -> Result<Url, Error> {
    let url = Url::parse(text).map_err(|_| Error::InvalidConfiguration)?;
    let canonical = url.origin().ascii_serialization();
    if !text.starts_with("https://")
        || (text != canonical && text != format!("{canonical}/"))
        || url.scheme() != "https"
        || !url.has_host()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.path() != "/"
        || text.contains('\\')
        || text.chars().any(char::is_control)
    {
        return Err(Error::InvalidConfiguration);
    }
    Ok(url)
}

/// Each input is bounded by its typed caller. Owned form buffers are zeroized on drop.
pub(crate) fn form(fields: &[(&str, &str)]) -> Zeroizing<Vec<u8>> {
    let length = fields.len().saturating_sub(1)
        + fields
            .iter()
            .map(|(key, value)| {
                key.len()
                    + 1
                    + value
                        .bytes()
                        .map(|b| {
                            if b.is_ascii_alphanumeric() || b"-._~ ".contains(&b) {
                                1
                            } else {
                                3
                            }
                        })
                        .sum::<usize>()
            })
            .sum::<usize>();
    let mut bytes = Zeroizing::new(Vec::with_capacity(length));
    for (index, (key, value)) in fields.iter().enumerate() {
        if index != 0 {
            bytes.push(b'&');
        }
        bytes.extend_from_slice(key.as_bytes());
        bytes.push(b'=');
        for b in value.bytes() {
            if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
                bytes.push(b);
            } else if b == b' ' {
                bytes.push(b'+');
            } else {
                bytes.extend_from_slice(&[
                    b'%',
                    b"0123456789ABCDEF"[(b >> 4) as usize],
                    b"0123456789ABCDEF"[(b & 15) as usize],
                ]);
            }
        }
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::{MAX_DEPARTURE_BODY, MAX_JOIN_BODY, departure_body, form, join_body};
    use crate::{DeviceId, Error, GithubToken};
    use zeroize::Zeroizing;

    const DEVICE_ID: &str = "00000000-0000-4000-8000-000000000000";

    #[test]
    fn join_encoding_preserves_bytes_across_allocation_boundaries() {
        let device_id = DeviceId::parse(DEVICE_ID).unwrap();
        for length in [20, 59, 60, 61, 187, 188, 189, 255] {
            let token = GithubToken::from_secret(Zeroizing::new("A".repeat(length))).unwrap();
            let body = join_body(&token, &device_id).unwrap();
            let expected = format!(
                "{{\"githubToken\":\"{}\",\"deviceId\":\"{DEVICE_ID}\"}}",
                "A".repeat(length)
            );
            assert_eq!(body.as_slice(), expected.as_bytes());
            assert_eq!(body.capacity(), body.len());
        }
        let maximum = GithubToken::from_secret(Zeroizing::new("A".repeat(255))).unwrap();
        assert_eq!(
            join_body(&maximum, &device_id).unwrap().len(),
            MAX_JOIN_BODY
        );
    }

    #[test]
    fn join_encoding_refuses_to_grow_past_the_fixed_buffer() {
        let device_id = DeviceId::parse(DEVICE_ID).unwrap();
        // Deliberately bypass validation to test the writer's independent bound.
        let oversized = GithubToken(Zeroizing::new("A".repeat(256)));
        assert_eq!(
            join_body(&oversized, &device_id),
            Err(Error::InvalidRequest)
        );
    }

    #[test]
    fn departure_encoding_has_its_own_fixed_secret_bound() {
        for length in [20, 109, 110, 111, 237, 238, 239, 255] {
            let token = GithubToken::from_secret(Zeroizing::new("A".repeat(length))).unwrap();
            let body = departure_body(&token).unwrap();
            assert_eq!(
                body.as_slice(),
                format!("{{\"githubToken\":\"{}\"}}", "A".repeat(length)).as_bytes()
            );
            assert_eq!(body.capacity(), body.len());
        }
        let maximum = GithubToken::from_secret(Zeroizing::new("A".repeat(255))).unwrap();
        assert_eq!(departure_body(&maximum).unwrap().len(), MAX_DEPARTURE_BODY);
        let oversized = GithubToken(Zeroizing::new("A".repeat(256)));
        assert_eq!(departure_body(&oversized), Err(Error::InvalidRequest));
    }

    #[test]
    fn poll_form_reserves_exactly_for_every_supported_client_id_length() {
        let device_code = "0".repeat(40);
        for length in 1..=64 {
            let client_id = "a".repeat(length);
            let body = form(&[
                ("client_id", &client_id),
                ("device_code", &device_code),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ]);
            let expected = format!(
                "client_id={client_id}&device_code={device_code}&grant_type=urn%3Aietf%3Aparams%3Aoauth%3Agrant-type%3Adevice_code"
            );
            assert_eq!(body.as_slice(), expected.as_bytes());
            assert_eq!(body.capacity(), body.len());
        }
    }

    #[test]
    fn form_reservation_preserves_escaping_and_empty_fields() {
        let body = form(&[
            ("value", " AZaz09-._~+&=%:/\0\u{7f}\u{80}\u{10ffff}"),
            ("empty", ""),
        ]);
        assert_eq!(
            body.as_slice(),
            b"value=+AZaz09-._~%2B%26%3D%25%3A%2F%00%7F%C2%80%F4%8F%BF%BF&empty="
        );
        assert_eq!(body.capacity(), body.len());
        let empty = form(&[]);
        assert!(empty.is_empty());
        assert_eq!(empty.capacity(), 0);
    }
}
