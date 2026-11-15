//! Serialized, origin-bound OS enrollment state. No network operation runs here.
use crate::{DeviceId, Enrollment, Error, Joined, transport::validate_origin};
use openusage_keys::{BoardCredential, BoardGuard, BoardGuardError, BoardVault, VaultError};
use serde::{Deserialize, Serialize};
use std::fmt;
#[cfg(feature = "native-test")]
use std::path::Path;
use zeroize::Zeroizing;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionError {
    Configuration,
    Busy,
    UnsafePath,
    StoreUnavailable,
    Unsupported,
    InvalidCredential,
    AlreadyJoined,
    NotJoined,
    SaveFailed,
}
impl fmt::Display for SessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Configuration => "The leaderboard configuration is invalid.",
            Self::Busy => "Another leaderboard command holds the credential lock.",
            Self::UnsafePath => "The leaderboard lock path is not private and safe.",
            Self::Unsupported => {
                "Leaderboard enrollment needs a private Linux user runtime directory."
            }
            Self::StoreUnavailable => "The system credential store is locked or unavailable.",
            Self::InvalidCredential => {
                "The saved leaderboard credential is invalid for this service."
            }
            Self::AlreadyJoined => {
                "This device is already joined. Use join --reconnect to authorize it again."
            }
            Self::NotJoined => "This device has no saved enrollment. Run openusage board join.",
            Self::SaveFailed => {
                "The credential save failed. Run join again with the same saved device ID."
            }
        })
    }
}
impl std::error::Error for SessionError {}
impl From<VaultError> for SessionError {
    fn from(error: VaultError) -> Self {
        match error {
            VaultError::InvalidKey => Self::InvalidCredential,
            VaultError::Unavailable => Self::StoreUnavailable,
        }
    }
}

enum State {
    Vacant,
    Pending(DeviceId),
    Joined(Enrollment),
}

/// Owns the cross-process guard for every read, authorization, save, sync, or leave.
/// The caller supplies its configured destination. Stored bytes never choose a host.
pub struct EnrollmentSession {
    _guard: BoardGuard,
    vault: BoardVault,
    origin: String,
    expected: Option<BoardCredential>,
    state: State,
}
impl fmt::Debug for EnrollmentSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("EnrollmentSession([redacted])")
    }
}
impl EnrollmentSession {
    pub fn open(origin: &str) -> Result<Self, SessionError> {
        let origin = checked_origin(origin)?;
        let guard = BoardGuard::for_current_user().map_err(guard_error)?;
        Self::with_guard(origin, guard)
    }

    /// A separate private lock path exists only in native acceptance builds.
    #[cfg(feature = "native-test")]
    pub fn open_fixture(origin: &str, directory: &Path) -> Result<Self, SessionError> {
        let origin = checked_origin(origin)?;
        let guard = BoardGuard::acquire(directory).map_err(guard_error)?;
        Self::with_guard(origin, guard)
    }

    fn with_guard(origin: String, guard: BoardGuard) -> Result<Self, SessionError> {
        let vault = BoardVault::new()?;
        let expected = vault.load()?;
        let state = expected
            .as_ref()
            .map_or(Ok(State::Vacant), |record| decode(&origin, record.expose()))?;
        Ok(Self {
            _guard: guard,
            vault,
            origin,
            expected,
            state,
        })
    }

    /// Save a stable pending ID before the first remote authorization or join.
    /// A resumed attempt reuses that ID. A reconnect keeps the old credential until replacement.
    pub fn prepare_join(&mut self, reconnect: bool) -> Result<DeviceId, SessionError> {
        match &self.state {
            State::Joined(enrollment) if reconnect => return Ok(enrollment.device_id().clone()),
            State::Joined(_) => return Err(SessionError::AlreadyJoined),
            State::Pending(id) => return Ok(id.clone()),
            State::Vacant => (),
        }
        let id = DeviceId::parse(&uuid::Uuid::new_v4().to_string())
            .map_err(|_| SessionError::Configuration)?;
        let record = pending(&self.origin, &id)?;
        self.vault.protect(&record)?;
        self.expected = Some(record);
        self.state = State::Pending(id.clone());
        Ok(id)
    }

    /// The remote join can succeed before the OS save fails. Never roll it back with global leave.
    /// The saved pending or old record retains the same ID for an explicit recovery attempt.
    pub fn finish_join(&mut self, joined: Joined) -> Result<(), SessionError> {
        let id = match &self.state {
            State::Vacant => return Err(SessionError::NotJoined),
            State::Pending(id) => id,
            State::Joined(enrollment) => enrollment.device_id(),
        };
        if &joined.device_id != id {
            return Err(SessionError::InvalidCredential);
        }
        let enrollment = Enrollment::from_joined(&self.origin, joined)
            .map_err(|_| SessionError::InvalidCredential)?;
        let replacement = BoardCredential::from_secret(
            enrollment
                .to_secret()
                .map_err(|_| SessionError::InvalidCredential)?,
        )?;
        self.vault
            .replace(
                self.expected.as_ref().ok_or(SessionError::NotJoined)?,
                &replacement,
            )
            .map_err(|_| SessionError::SaveFailed)?;
        self.expected = Some(replacement);
        self.state = State::Joined(enrollment);
        Ok(())
    }

    pub fn enrollment(&self) -> Result<&Enrollment, SessionError> {
        match &self.state {
            State::Joined(enrollment) => Ok(enrollment),
            _ => Err(SessionError::NotJoined),
        }
    }

    /// Call only after the service confirms removal. A failed remote call must retain this record.
    pub fn clear_after_leave(&mut self) -> Result<(), SessionError> {
        self.enrollment()?;
        self.vault
            .remove(self.expected.as_ref().ok_or(SessionError::NotJoined)?)?;
        self.expected = None;
        self.state = State::Vacant;
        Ok(())
    }
}

fn checked_origin(origin: &str) -> Result<String, SessionError> {
    let origin = validate_origin(origin)
        .map_err(|_| SessionError::Configuration)?
        .to_string();
    if origin.len() > 2048 {
        return Err(SessionError::Configuration);
    }
    Ok(origin)
}
fn guard_error(error: BoardGuardError) -> SessionError {
    match error {
        BoardGuardError::Busy => SessionError::Busy,
        BoardGuardError::UnsafePath => SessionError::UnsafePath,
        BoardGuardError::Unavailable => SessionError::StoreUnavailable,
        BoardGuardError::Unsupported => SessionError::Unsupported,
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Pending {
    schema_version: u8,
    phase: String,
    service_origin: String,
    device_id: DeviceIdText,
}
// The pending record has no secret fields. Its schema never accepts an active service token.
#[derive(Serialize, Deserialize)]
#[serde(transparent)]
struct DeviceIdText(String);
fn pending(origin: &str, id: &DeviceId) -> Result<BoardCredential, SessionError> {
    let bytes = serde_json::to_vec(&Pending {
        schema_version: 1,
        phase: "pending".into(),
        service_origin: origin.into(),
        device_id: DeviceIdText(id.as_str().into()),
    })
    .map_err(|_| SessionError::InvalidCredential)?;
    BoardCredential::from_secret(Zeroizing::new(bytes)).map_err(Into::into)
}
fn decode(origin: &str, bytes: &[u8]) -> Result<State, SessionError> {
    if bytes.is_empty()
        || bytes.len() > 4096
        || !bytes.is_ascii()
        || bytes.contains(&b'\\')
        || bytes.iter().find(|b| !b.is_ascii_whitespace()) != Some(&b'{')
    {
        return Err(SessionError::InvalidCredential);
    }
    #[derive(Deserialize)]
    struct Header<'a> {
        #[serde(borrow)]
        phase: Option<&'a str>,
    }
    let header: Header<'_> =
        serde_json::from_slice(bytes).map_err(|_| SessionError::InvalidCredential)?;
    match header.phase {
        Some("pending") => {
            let saved: Pending =
                serde_json::from_slice(bytes).map_err(|_| SessionError::InvalidCredential)?;
            if saved.schema_version != 1
                || saved.phase != "pending"
                || saved.service_origin != origin
            {
                return Err(SessionError::InvalidCredential);
            }
            DeviceId::parse(&saved.device_id.0)
                .map(State::Pending)
                .map_err(|_| SessionError::InvalidCredential)
        }
        Some(_) => Err(SessionError::InvalidCredential),
        None => Enrollment::from_secret(origin, Zeroizing::new(bytes.to_vec()))
            .map(State::Joined)
            .map_err(|_: Error| SessionError::InvalidCredential),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const ORIGIN: &str = "https://board.example/";
    const ID: &str = "f9168c5e-ceb2-4faa-b6bf-329bf39fa1e4";
    #[test]
    fn pending_is_strict_and_origin_bound_without_a_service_token() {
        let id = DeviceId::parse(ID).unwrap();
        let record = pending(ORIGIN, &id).unwrap();
        assert!(
            matches!(decode(ORIGIN, record.expose()), Ok(State::Pending(saved)) if saved == id)
        );
        assert!(decode("https://other.example/", record.expose()).is_err());
        let json = std::str::from_utf8(record.expose()).unwrap();
        for invalid in [
            json.replace("pending", "other"),
            json.replace("schemaVersion\":1", "schemaVersion\":2"),
            json.replace('}', ",\"serviceToken\":\"secret\"}"),
            format!("[{json}]"),
            json.replace("phase", "ph\\u0061se"),
        ] {
            assert!(decode(ORIGIN, invalid.as_bytes()).is_err());
        }
    }
}
