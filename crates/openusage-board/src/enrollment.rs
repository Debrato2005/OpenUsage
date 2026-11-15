//! A local credential envelope. This module performs no network or storage operation.
use crate::{DeviceId, Error, Joined, ServiceToken, protocol::login, transport::validate_origin};
use serde::{Deserialize, Serialize};
use std::{fmt, io::Cursor};
use zeroize::Zeroizing;

const SCHEMA_VERSION: u8 = 1;
const MAX_BYTES: usize = 4096;
const MAX_ORIGIN_BYTES: usize = 2048;

/// One service-issued device credential, bound to its configured HTTPS origin.
/// This type has no serializer, display implementation, or implicit secret access.
pub struct Enrollment {
    service_origin: String,
    device_id: DeviceId,
    login: String,
    token: ServiceToken,
}

impl Enrollment {
    pub fn from_joined(service_origin: &str, joined: Joined) -> Result<Self, Error> {
        let service_origin = origin(service_origin)?;
        if !login(&joined.login) {
            return Err(Error::InvalidRequest);
        }
        Ok(Self {
            service_origin,
            device_id: joined.device_id,
            login: joined.login,
            token: joined.token,
        })
    }

    /// Restore only for the caller's configured origin, never a stored destination.
    pub fn from_secret(expected_origin: &str, bytes: Zeroizing<Vec<u8>>) -> Result<Self, Error> {
        let expected_origin = origin(expected_origin)?;
        if bytes.is_empty()
            || bytes.len() > MAX_BYTES
            || !bytes.is_ascii()
            || bytes.contains(&b'\\')
            || bytes.iter().find(|byte| !byte.is_ascii_whitespace()) != Some(&b'{')
        {
            return Err(Error::InvalidRequest);
        }
        let saved: Saved = serde_json::from_slice(&bytes).map_err(|_| Error::InvalidRequest)?;
        if saved.schema_version != SCHEMA_VERSION
            || saved.service_origin != expected_origin
            || !login(&saved.login)
        {
            return Err(Error::InvalidRequest);
        }
        Ok(Self {
            service_origin: expected_origin,
            device_id: saved.device_id,
            login: saved.login,
            token: saved.service_token,
        })
    }

    /// Deliberate encoding for the OS credential store. Never send these bytes over IPC.
    pub fn to_secret(&self) -> Result<Zeroizing<Vec<u8>>, Error> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct SavedRef<'a> {
            schema_version: u8,
            service_origin: &'a str,
            device_id: &'a str,
            login: &'a str,
            service_token: &'a str,
        }
        let mut buffer = Zeroizing::new([0_u8; MAX_BYTES]);
        let length = {
            let mut writer = Cursor::new(&mut buffer[..]);
            serde_json::to_writer(
                &mut writer,
                &SavedRef {
                    schema_version: SCHEMA_VERSION,
                    service_origin: &self.service_origin,
                    device_id: self.device_id.as_str(),
                    login: &self.login,
                    service_token: self.token.expose(),
                },
            )
            .map_err(|_| Error::InvalidRequest)?;
            writer.position() as usize
        };
        Ok(Zeroizing::new(buffer[..length].to_vec()))
    }

    pub fn service_origin(&self) -> &str {
        &self.service_origin
    }

    pub fn device_id(&self) -> &DeviceId {
        &self.device_id
    }

    pub fn login(&self) -> &str {
        &self.login
    }

    pub fn token(&self) -> &ServiceToken {
        &self.token
    }
}

impl fmt::Debug for Enrollment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Enrollment([redacted])")
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Saved {
    schema_version: u8,
    service_origin: String,
    device_id: DeviceId,
    login: String,
    service_token: ServiceToken,
}

fn origin(value: &str) -> Result<String, Error> {
    if value.len() > MAX_ORIGIN_BYTES {
        return Err(Error::InvalidConfiguration);
    }
    validate_origin(value).map(|value| value.to_string())
}
