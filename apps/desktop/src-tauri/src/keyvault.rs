#[cfg(not(target_os = "linux"))]
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
        let encoded = match self.secret()? {
            Some(secret) => Zeroizing::new(secret),
            None => return Ok(None),
        };
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
        Ok(Some(key))
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

#[cfg(not(target_os = "linux"))]
fn map_error(error: keyring_core::Error) -> VaultError {
    match error {
        keyring_core::Error::NoStorageAccess(_) | keyring_core::Error::PlatformFailure(_) => {
            VaultError::Unavailable
        }
        _ => VaultError::InvalidKey,
    }
}
