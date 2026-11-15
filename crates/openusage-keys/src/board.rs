//! One fixed OS credential slot for future public leaderboard enrollment.
use crate::{OsVault, VaultError};
use std::fmt;
use zeroize::Zeroizing;

const ACCOUNT: &str = "leaderboard-device-session-v1";
const PREFIX: &[u8] = b"openusage-board-v1\0";
const MAX_BYTES: usize = 4096;

/// An opaque bounded credential envelope. Its caller owns the payload schema.
/// This type has no serializer or implicit display of its contents.
pub struct BoardCredential(Zeroizing<Vec<u8>>);

impl BoardCredential {
    pub fn from_secret(secret: Zeroizing<Vec<u8>>) -> Result<Self, VaultError> {
        if secret.is_empty() || secret.len() > MAX_BYTES {
            return Err(VaultError::InvalidKey);
        }
        Ok(Self(secret))
    }

    /// Deliberate access for typed Rust enrollment code, never for IPC or logs.
    pub fn expose(&self) -> &[u8] {
        &self.0
    }

    fn encoded(&self) -> Zeroizing<Vec<u8>> {
        let mut encoded = Zeroizing::new(Vec::with_capacity(PREFIX.len() + self.0.len()));
        encoded.extend_from_slice(PREFIX);
        encoded.extend_from_slice(&self.0);
        encoded
    }
}

impl fmt::Debug for BoardCredential {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("BoardCredential([redacted])")
    }
}

/// The database key and leaderboard credential use distinct fixed account names.
/// No plaintext fallback, network operation, or automatic enrollment exists here.
pub struct BoardVault(OsVault);

impl BoardVault {
    pub fn new() -> Result<Self, VaultError> {
        OsVault::for_account(ACCOUNT).map(Self)
    }

    pub fn load(&self) -> Result<Option<BoardCredential>, VaultError> {
        self.0
            .secret()?
            .map(|bytes| {
                let bytes = Zeroizing::new(bytes);
                let payload = bytes.strip_prefix(PREFIX).ok_or(VaultError::InvalidKey)?;
                BoardCredential::from_secret(Zeroizing::new(payload.to_vec()))
            })
            .transpose()
    }

    /// Refuse an occupied slot. Callers must serialize enrollment operations.
    pub fn protect(&self, credential: &BoardCredential) -> Result<(), VaultError> {
        self.0.protect_secret(
            "OpenUsage leaderboard device session",
            &credential.encoded(),
        )
    }

    /// Remove only the selected slot after an exact expected-value comparison.
    /// An absent item is already removed. Mismatches leave it untouched.
    pub fn remove(&self, expected: &BoardCredential) -> Result<(), VaultError> {
        let expected = expected.encoded();
        self.0.remove_checked(|actual| {
            let actual = Zeroizing::new(actual);
            if actual.len() != expected.len() {
                return Err(VaultError::InvalidKey);
            }
            let difference = actual
                .iter()
                .zip(expected.iter())
                .fold(0_u8, |difference, (actual, expected)| {
                    difference | (actual ^ expected)
                });
            if difference == 0 {
                Ok(())
            } else {
                Err(VaultError::InvalidKey)
            }
        })
    }
}
