#[cfg(any(not(target_os = "linux"), test))]
use keyring_core::{Entry, api::CredentialStoreApi};
use zeroize::Zeroizing;

pub type DatabaseKey = Zeroizing<[u8; 32]>;

#[cfg(not(feature = "native-test"))]
const SERVICE: &str = "app.openusage.desktop";
#[cfg(feature = "native-test")]
const SERVICE: &str = "app.openusage.desktop.native-test";
const ACCOUNT: &str = "primary-database-key-v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VaultError {
    Unavailable,
    InvalidKey,
}

pub trait KeyVault {
    fn load(&self) -> Result<Option<DatabaseKey>, VaultError>;
    fn protect(&self, key: &[u8; 32]) -> Result<(), VaultError>;
    /// Remove only this application's database key after verifying its value.
    /// An absent entry is already removed; another key must remain untouched.
    fn remove(&self, expected: &[u8; 32]) -> Result<(), VaultError>;
}

pub struct OsVault {
    #[cfg(not(target_os = "linux"))]
    entry: Entry,
    #[cfg(target_os = "linux")]
    service: secret_service::SecretService<'static>,
}

impl OsVault {
    #[cfg(target_os = "linux")]
    pub fn new() -> Result<Self, VaultError> {
        let service = bounded(async {
            secret_service::SecretService::connect(secret_service::EncryptionType::Dh)
                .await
                .map_err(|_| VaultError::Unavailable)
        })?;
        Ok(Self { service })
    }

    #[cfg(not(target_os = "linux"))]
    pub fn new() -> Result<Self, VaultError> {
        #[cfg(target_os = "macos")]
        let store = apple_native_keyring_store::keychain::Store::new().map_err(map_error)?;
        #[cfg(target_os = "windows")]
        let store = windows_native_keyring_store::Store::new().map_err(map_error)?;
        #[cfg(target_os = "windows")]
        let modifiers = Some(std::collections::HashMap::from([("persistence", "Local")]));
        #[cfg(not(target_os = "windows"))]
        let modifiers = None;
        let entry = store
            .build(SERVICE, ACCOUNT, modifiers.as_ref())
            .map_err(map_error)?;
        Ok(Self { entry })
    }

    #[cfg(target_os = "linux")]
    fn secret(&self) -> Result<Option<Vec<u8>>, VaultError> {
        bounded(self.secret_async())
    }

    #[cfg(target_os = "linux")]
    async fn secret_async(&self) -> Result<Option<Vec<u8>>, VaultError> {
        let attributes =
            std::collections::HashMap::from([("service", SERVICE), ("username", ACCOUNT)]);
        let found = self
            .service
            .search_items(attributes)
            .await
            .map_err(|_| VaultError::Unavailable)?;
        if found.unlocked.len() + found.locked.len() > 1 {
            return Err(VaultError::InvalidKey);
        }
        if !found.locked.is_empty() {
            return Err(VaultError::Unavailable);
        }
        if let Some(item) = found.unlocked.first() {
            return item
                .get_secret()
                .await
                .map(Some)
                .map_err(|_| VaultError::Unavailable);
        }
        let collection = self
            .service
            .get_default_collection()
            .await
            .map_err(|_| VaultError::Unavailable)?;
        if collection
            .is_locked()
            .await
            .map_err(|_| VaultError::Unavailable)?
        {
            return Err(VaultError::Unavailable);
        }
        Ok(None)
    }

    #[cfg(not(target_os = "linux"))]
    fn secret(&self) -> Result<Option<Vec<u8>>, VaultError> {
        match self.entry.get_secret() {
            Ok(secret) => Ok(Some(secret)),
            Err(keyring_core::Error::NoEntry) => Ok(None),
            Err(error) => Err(map_error(error)),
        }
    }
}

impl KeyVault for OsVault {
    fn load(&self) -> Result<Option<DatabaseKey>, VaultError> {
        self.secret()?.map(decode_key).transpose()
    }

    fn remove(&self, expected: &[u8; 32]) -> Result<(), VaultError> {
        #[cfg(target_os = "linux")]
        {
            bounded(async {
                let attributes =
                    std::collections::HashMap::from([("service", SERVICE), ("username", ACCOUNT)]);
                let found = self
                    .service
                    .search_items(attributes)
                    .await
                    .map_err(|_| VaultError::Unavailable)?;
                if found.unlocked.len() + found.locked.len() > 1 {
                    return Err(VaultError::InvalidKey);
                }
                if !found.locked.is_empty() {
                    return Err(VaultError::Unavailable);
                }
                let Some(item) = found.unlocked.first() else {
                    return Ok(());
                };
                // Search results and deletion retain this exact item object. Do
                // not enumerate or delete a collection, or unlock any entry.
                let attributes = item
                    .get_attributes()
                    .await
                    .map_err(|_| VaultError::Unavailable)?;
                if attributes.get("service").map(String::as_str) != Some(SERVICE)
                    || attributes.get("username").map(String::as_str) != Some(ACCOUNT)
                {
                    return Err(VaultError::InvalidKey);
                }
                verify_expected(
                    item.get_secret()
                        .await
                        .map_err(|_| VaultError::Unavailable)?,
                    expected,
                )?;
                // Item::delete rechecks its locked property before deletion.
                item.delete().await.map_err(|_| VaultError::Unavailable)
            })
        }
        #[cfg(not(target_os = "linux"))]
        remove_entry(&self.entry, expected)
    }

    fn protect(&self, key: &[u8; 32]) -> Result<(), VaultError> {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut encoded = Zeroizing::new(Vec::with_capacity(67));
        encoded.extend_from_slice(b"v1:");
        for byte in key {
            encoded.push(HEX[usize::from(byte >> 4)]);
            encoded.push(HEX[usize::from(byte & 15)]);
        }
        #[cfg(target_os = "linux")]
        {
            bounded(async {
                if self.secret_async().await?.is_some() {
                    return Err(VaultError::InvalidKey);
                }
                let collection = self
                    .service
                    .get_default_collection()
                    .await
                    .map_err(|_| VaultError::Unavailable)?;
                if collection
                    .is_locked()
                    .await
                    .map_err(|_| VaultError::Unavailable)?
                {
                    return Err(VaultError::Unavailable);
                }
                let attributes =
                    std::collections::HashMap::from([("service", SERVICE), ("username", ACCOUNT)]);
                collection
                    .create_item(
                        "OpenUsage encrypted database key",
                        attributes,
                        &encoded,
                        false,
                        "text/plain",
                    )
                    .await
                    .map_err(|_| VaultError::Unavailable)?;
                Ok(())
            })
        }
        #[cfg(not(target_os = "linux"))]
        self.entry.set_secret(&encoded).map_err(map_error)
    }
}

fn decode_key(encoded: Vec<u8>) -> Result<DatabaseKey, VaultError> {
    let encoded = Zeroizing::new(encoded);
    if encoded.len() != 67 || &encoded[..3] != b"v1:" {
        return Err(VaultError::InvalidKey);
    }
    let mut key = Zeroizing::new([0_u8; 32]);
    for (target, pair) in key.iter_mut().zip(encoded[3..].as_chunks::<2>().0) {
        let digit = |byte: u8| match byte {
            b'0'..=b'9' => Ok(byte - b'0'),
            b'a'..=b'f' => Ok(byte - b'a' + 10),
            _ => Err(VaultError::InvalidKey),
        };
        *target = digit(pair[0])? * 16 + digit(pair[1])?;
    }
    Ok(key)
}
fn verify_expected(encoded: Vec<u8>, expected: &[u8; 32]) -> Result<(), VaultError> {
    let actual = decode_key(encoded)?;
    let difference = actual
        .iter()
        .zip(expected)
        .fold(0_u8, |difference, (actual, expected)| {
            difference | (actual ^ expected)
        });
    if difference != 0 {
        return Err(VaultError::InvalidKey);
    }
    Ok(())
}
#[cfg(any(not(target_os = "linux"), test))]
fn remove_entry(entry: &Entry, expected: &[u8; 32]) -> Result<(), VaultError> {
    let encoded = match entry.get_secret() {
        Ok(secret) => secret,
        Err(keyring_core::Error::NoEntry) => return Ok(()),
        Err(error) => return Err(map_error(error)),
    };
    verify_expected(encoded, expected)?;
    match entry.delete_credential() {
        Ok(()) | Err(keyring_core::Error::NoEntry) => Ok(()),
        Err(error) => Err(map_error(error)),
    }
}

#[cfg(target_os = "linux")]
fn bounded<T>(
    future: impl std::future::Future<Output = Result<T, VaultError>>,
) -> Result<T, VaultError> {
    tauri::async_runtime::block_on(async {
        tokio::time::timeout(std::time::Duration::from_secs(5), future)
            .await
            .map_err(|_| VaultError::Unavailable)?
    })
}

#[cfg(any(not(target_os = "linux"), test))]
fn map_error(error: keyring_core::Error) -> VaultError {
    match error {
        keyring_core::Error::NoStorageAccess(_) | keyring_core::Error::PlatformFailure(_) => {
            VaultError::Unavailable
        }
        _ => VaultError::InvalidKey,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removal_is_idempotent_and_deletes_only_the_exact_private_mock_entry() {
        let store = keyring_core::mock::Store::new().unwrap();
        let entry = store.build(SERVICE, ACCOUNT, None).unwrap();
        let other_account = store.build(SERVICE, "unrelated-account", None).unwrap();
        let other_service = store.build("unrelated-service", ACCOUNT, None).unwrap();
        let encoded = format!("v1:{}", "31".repeat(32));
        other_account
            .set_secret(b"unrelated account fixture")
            .unwrap();
        other_service
            .set_secret(b"unrelated service fixture")
            .unwrap();
        assert_eq!(remove_entry(&entry, &[0x31; 32]), Ok(()));
        entry.set_secret(encoded.as_bytes()).unwrap();
        assert_eq!(remove_entry(&entry, &[0x31; 32]), Ok(()));
        assert!(matches!(
            entry.get_secret(),
            Err(keyring_core::Error::NoEntry)
        ));
        assert_eq!(remove_entry(&entry, &[0x31; 32]), Ok(()));
        assert_eq!(
            other_account.get_secret().unwrap(),
            b"unrelated account fixture"
        );
        assert_eq!(
            other_service.get_secret().unwrap(),
            b"unrelated service fixture"
        );
    }

    #[test]
    fn expected_key_mismatch_preserves_the_private_mock_entry() {
        let store = keyring_core::mock::Store::new().unwrap();
        let entry = store.build(SERVICE, ACCOUNT, None).unwrap();
        let encoded = format!("v1:{}", "31".repeat(32));
        entry.set_secret(encoded.as_bytes()).unwrap();
        assert_eq!(
            remove_entry(&entry, &[0x32; 32]),
            Err(VaultError::InvalidKey)
        );
        assert_eq!(entry.get_secret().unwrap(), encoded.as_bytes());
        assert_eq!(remove_entry(&entry, &[0x31; 32]), Ok(()));
    }

    #[test]
    fn malformed_stored_keys_are_refused_without_deleting_the_private_mock_entry() {
        let store = keyring_core::mock::Store::new().unwrap();
        let entry = store.build(SERVICE, ACCOUNT, None).unwrap();
        for encoded in [
            String::new(),
            format!("v2:{}", "31".repeat(32)),
            format!("v1:{}", "31".repeat(31)),
            format!("v1:{}", "AA".repeat(32)),
            format!("v1:{}", "gg".repeat(32)),
            format!("v1:{}0", "31".repeat(32)),
        ] {
            entry.set_secret(encoded.as_bytes()).unwrap();
            assert_eq!(
                remove_entry(&entry, &[0x31; 32]),
                Err(VaultError::InvalidKey)
            );
            assert_eq!(entry.get_secret().unwrap(), encoded.as_bytes());
        }
    }
}
