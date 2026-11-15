//! Fresh public departure preserves local records without an exact token witness.
use crate::{BoardClient, Enrollment, Error, GithubToken, SessionError, session::checked_origin};
use openusage_keys::{BoardCredential, BoardGuard, BoardVault, VaultError};
use serde::Serialize;
use std::fmt;
use zeroize::Zeroizing;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalCredentialState {
    Removed,
    NotPresent,
    Retained,
    Unavailable,
}

pub struct DepartureReceipt {
    pub login: String,
    pub credential_state: LocalCredentialState,
}
impl fmt::Debug for DepartureReceipt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DepartureReceipt([redacted])")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DepartureError {
    Remote(Error),
    CleanupFailed,
}
impl fmt::Display for DepartureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Remote(error) => error.fmt(f),
            Self::CleanupFailed => f.write_str(
                "Public departure was confirmed, but local credential cleanup failed. Restore store access, then run board forget.",
            ),
        }
    }
}
impl std::error::Error for DepartureError {}

struct Active {
    vault: BoardVault,
    expected: BoardCredential,
    enrollment: Enrollment,
}
enum Local {
    Active(Box<Active>),
    Absent,
    Unverified,
    Unavailable,
}
impl Local {
    fn load(origin: &str) -> Self {
        let vault = match BoardVault::new() {
            Ok(vault) => vault,
            Err(_) => return Self::Unavailable,
        };
        let expected = match vault.load() {
            Ok(Some(record)) => record,
            Ok(None) => return Self::Absent,
            Err(VaultError::InvalidKey) => return Self::Unverified,
            Err(VaultError::Unavailable) => return Self::Unavailable,
        };
        match Enrollment::from_secret(origin, Zeroizing::new(expected.expose().to_vec())) {
            Ok(enrollment) => Self::Active(Box::new(Active {
                vault,
                expected,
                enrollment,
            })),
            Err(_) => Self::Unverified,
        }
    }
    fn token(&self) -> Option<&crate::ServiceToken> {
        match self {
            Self::Active(active) => Some(active.enrollment.token()),
            _ => None,
        }
    }
    fn finish(&self, matched: Option<bool>) -> Result<LocalCredentialState, DepartureError> {
        match self {
            Self::Active(active) if matched == Some(true) => {
                active
                    .vault
                    .remove(&active.expected)
                    .map_err(|_| DepartureError::CleanupFailed)?;
                Ok(LocalCredentialState::Removed)
            }
            Self::Active(_) | Self::Unverified => Ok(LocalCredentialState::Retained),
            Self::Absent => Ok(LocalCredentialState::NotPresent),
            Self::Unavailable => Ok(LocalCredentialState::Unavailable),
        }
    }
}

pub struct DepartureSession {
    _guard: BoardGuard,
    origin: String,
    local: Local,
}
impl fmt::Debug for DepartureSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DepartureSession([redacted])")
    }
}
impl DepartureSession {
    /// The caller holds the common guard before credential access and authorization.
    pub fn from_guard(origin: &str, guard: BoardGuard) -> Result<Self, SessionError> {
        let origin = checked_origin(origin)?;
        let local = Local::load(&origin);
        Ok(Self {
            _guard: guard,
            origin,
            local,
        })
    }

    /// Reserve the public effect before this call. Always finish local reconciliation afterward.
    pub fn leave(
        self,
        client: &BoardClient,
        github: GithubToken,
    ) -> Result<DepartureReceipt, DepartureError> {
        if client.service_origin() != Some(self.origin.as_str()) {
            return Err(DepartureError::Remote(Error::InvalidConfiguration));
        }
        let departure = client
            .leave_with_github(github, self.local.token())
            .map_err(DepartureError::Remote)?;
        let credential_state = self.local.finish(departure.local_token_revoked())?;
        Ok(DepartureReceipt {
            login: departure.login().into(),
            credential_state,
        })
    }
}
