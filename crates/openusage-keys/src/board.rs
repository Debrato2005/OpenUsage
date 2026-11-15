//! One fixed OS credential slot for future public leaderboard enrollment.
use crate::{BoardGuard, OsVault, VaultError};
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
                if payload.is_empty() || payload.len() > MAX_BYTES {
                    return Err(VaultError::InvalidKey);
                }
                BoardCredential::from_secret(Zeroizing::new(payload.to_vec()))
            })
            .transpose()
    }

    /// Replace only the selected item after an exact expected-value comparison.
    /// Missing items and mismatches refuse without a write. Hold `BoardGuard` throughout.
    /// This is not an atomic comparison against changes from other OS-store clients.
    pub fn replace(
        &self,
        expected: &BoardCredential,
        replacement: &BoardCredential,
    ) -> Result<(), VaultError> {
        let expected = expected.encoded();
        let replacement = replacement.encoded();
        self.0
            .replace_checked(|actual| verify_secret(actual, &expected), &replacement)
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
        self.0
            .remove_checked(|actual| verify_secret(actual, &expected))
    }

    /// Erase this local slot after explicit confirmation of complete local deletion.
    /// Hold the current user's `BoardGuard` across the complete deletion operation.
    /// Linux also removes malformed and duplicate items with this exact service and account.
    /// No payload is read, and no public membership or remote record is changed.
    /// Locked or unavailable stores refuse, including when absence cannot be confirmed.
    /// A failure can follow partial erasure. Keep the deletion intent for explicit resume.
    pub fn erase_local(&self, _guard: &BoardGuard) -> Result<(), VaultError> {
        self.0.erase_local_slot()
    }
}

fn verify_secret(actual: Vec<u8>, expected: &[u8]) -> Result<(), VaultError> {
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SERVICE, erase_entry_local, replace_entry_checked};
    use keyring_core::api::CredentialStoreApi;

    #[test]
    fn replacement_refuses_missing_mismatch_and_malformed_secrets_without_other_slot_changes() {
        let store = keyring_core::mock::Store::new().unwrap();
        let entry = store.build(SERVICE, ACCOUNT, None).unwrap();
        let other_account = store.build(SERVICE, crate::ACCOUNT, None).unwrap();
        let other_service = store
            .build("synthetic-other-service", ACCOUNT, None)
            .unwrap();
        other_account.set_secret(b"synthetic database key").unwrap();
        other_service
            .set_secret(b"synthetic other service")
            .unwrap();
        let expected = BoardCredential::from_secret(Zeroizing::new(b"synthetic first".to_vec()))
            .unwrap()
            .encoded();
        let replacement =
            BoardCredential::from_secret(Zeroizing::new(b"synthetic second".to_vec()))
                .unwrap()
                .encoded();
        let replace = || {
            replace_entry_checked(
                &entry,
                |actual| verify_secret(actual, &expected),
                &replacement,
            )
        };
        assert_eq!(replace(), Err(VaultError::InvalidKey));
        assert!(matches!(
            entry.get_secret(),
            Err(keyring_core::Error::NoEntry)
        ));
        for invalid in [b"malformed".to_vec(), vec![1; 8192], replacement.to_vec()] {
            entry.set_secret(&invalid).unwrap();
            assert_eq!(replace(), Err(VaultError::InvalidKey));
            assert_eq!(entry.get_secret().unwrap(), invalid);
        }
        entry.set_secret(&expected).unwrap();
        replace().unwrap();
        assert_eq!(entry.get_secret().unwrap(), *replacement);
        assert_eq!(replace(), Err(VaultError::InvalidKey));
        assert_eq!(
            other_account.get_secret().unwrap(),
            b"synthetic database key"
        );
        assert_eq!(
            other_service.get_secret().unwrap(),
            b"synthetic other service"
        );
    }

    #[test]
    fn local_erasure_accepts_malformed_payloads_and_preserves_other_mock_slots() {
        let store = keyring_core::mock::Store::new().unwrap();
        let entry = store.build(SERVICE, ACCOUNT, None).unwrap();
        let database = store.build(SERVICE, crate::ACCOUNT, None).unwrap();
        let other_service = store
            .build("synthetic-other-service", ACCOUNT, None)
            .unwrap();
        database.set_secret(b"synthetic database key").unwrap();
        other_service
            .set_secret(b"synthetic other service")
            .unwrap();
        assert_eq!(erase_entry_local(&entry), Ok(()));
        for payload in [Vec::new(), b"malformed".to_vec(), vec![1; 8192]] {
            entry.set_secret(&payload).unwrap();
            assert_eq!(erase_entry_local(&entry), Ok(()));
            assert!(matches!(
                entry.get_secret(),
                Err(keyring_core::Error::NoEntry)
            ));
            assert_eq!(erase_entry_local(&entry), Ok(()));
        }
        assert_eq!(database.get_secret().unwrap(), b"synthetic database key");
        assert_eq!(
            other_service.get_secret().unwrap(),
            b"synthetic other service"
        );
    }

    #[test]
    fn local_erasure_refuses_unavailable_and_ambiguous_mock_stores() {
        let store = keyring_core::mock::Store::new().unwrap();
        let entry = store.build(SERVICE, ACCOUNT, None).unwrap();
        entry.set_secret(b"synthetic retained credential").unwrap();
        let mock: &keyring_core::mock::Cred = entry.as_any().downcast_ref().unwrap();
        for error in [
            keyring_core::Error::NoStorageAccess(Box::new(std::io::Error::other("locked"))),
            keyring_core::Error::PlatformFailure(Box::new(std::io::Error::other("unavailable"))),
        ] {
            mock.set_error(error);
            assert_eq!(erase_entry_local(&entry), Err(VaultError::Unavailable));
            assert_eq!(
                entry.get_secret().unwrap(),
                b"synthetic retained credential"
            );
        }
        mock.set_error(keyring_core::Error::Ambiguous(Vec::new()));
        assert_eq!(erase_entry_local(&entry), Err(VaultError::InvalidKey));
        assert_eq!(
            entry.get_secret().unwrap(),
            b"synthetic retained credential"
        );
    }
}
