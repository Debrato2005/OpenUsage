//! Exact cleanup for the pinned provider's direct Linux credential backend.
//! The caller owns confirmation and the held managed-root boundary. No secret is read.

#![cfg(target_os = "linux")]

use openusage_codex::supervisor::LinuxCredentialBus;
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    fs::{self, File, OpenOptions},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
};

const SERVICE: &str = "Codex Auth";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CredentialError {
    InvalidHome,
    InvalidEntry,
    Unavailable,
}

/// Check the exact desktop bus and an unlocked default collection without reading entries.
pub(crate) fn available(address: &str) -> Result<(), CredentialError> {
    let bus = LinuxCredentialBus::new(address).map_err(|_| CredentialError::Unavailable)?;
    bounded(async {
        let service = connect(&bus).await?;
        let collection = service
            .get_default_collection()
            .await
            .map_err(|_| CredentialError::Unavailable)?;
        if collection
            .is_locked()
            .await
            .map_err(|_| CredentialError::Unavailable)?
        {
            return Err(CredentialError::Unavailable);
        }
        bus.recheck().map_err(|_| CredentialError::Unavailable)
    })
}

/// Remove only the exact direct-backend entry for an explicitly selected managed home.
/// An unavailable store is not evidence that the entry is absent. Keep the home on error.
pub(crate) fn remove(home: &Path, address: &str) -> Result<(), CredentialError> {
    let home = HeldHome::open(home)?;
    let account = account_from_canonical_path(&home.path)?;
    let bus = LinuxCredentialBus::new(address).map_err(|_| CredentialError::Unavailable)?;
    bounded(async {
        let service = connect(&bus).await?;
        let found = service
            .search_items(HashMap::from([
                ("service", SERVICE),
                ("username", account.as_str()),
            ]))
            .await
            .map_err(|_| CredentialError::Unavailable)?;
        if found.unlocked.len() + found.locked.len() > 1 {
            return Err(CredentialError::InvalidEntry);
        }
        if !found.locked.is_empty() {
            return Err(CredentialError::Unavailable);
        }
        let Some(item) = found.unlocked.first() else {
            home.recheck()?;
            return bus.recheck().map_err(|_| CredentialError::Unavailable);
        };
        let attributes = item
            .get_attributes()
            .await
            .map_err(|_| CredentialError::Unavailable)?;
        if attributes.get("service").map(String::as_str) != Some(SERVICE)
            || attributes.get("username").map(String::as_str) != Some(account.as_str())
        {
            return Err(CredentialError::InvalidEntry);
        }
        if item
            .is_locked()
            .await
            .map_err(|_| CredentialError::Unavailable)?
        {
            return Err(CredentialError::Unavailable);
        }
        home.recheck()?;
        bus.recheck().map_err(|_| CredentialError::Unavailable)?;
        // Keep the search result's exact item object. Never delete a collection or unlock it.
        item.delete()
            .await
            .map_err(|_| CredentialError::Unavailable)
    })
}

/// Match pinned Codex compute_store_key, rejecting ambiguous or unsafe home paths.
#[cfg(test)]
pub(crate) fn account_for_home(home: &Path) -> Result<String, CredentialError> {
    let home = HeldHome::open(home)?;
    account_from_canonical_path(&home.path)
}

fn account_from_canonical_path(home: &Path) -> Result<String, CredentialError> {
    let path = home.to_str().ok_or(CredentialError::InvalidHome)?;
    let digest = Sha256::digest(path.as_bytes());
    Ok(format!("cli|{digest:x}")[..20].to_owned())
}

struct HeldHome {
    path: PathBuf,
    file: File,
    identity: (u64, u64, u32, u32),
}

impl HeldHome {
    fn open(path: &Path) -> Result<Self, CredentialError> {
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(path)
            .map_err(|_| CredentialError::InvalidHome)?;
        let metadata = file.metadata().map_err(|_| CredentialError::InvalidHome)?;
        let home = Self {
            path: path.to_owned(),
            file,
            identity: (
                metadata.dev(),
                metadata.ino(),
                metadata.uid(),
                metadata.mode(),
            ),
        };
        home.recheck()?;
        Ok(home)
    }

    fn recheck(&self) -> Result<(), CredentialError> {
        let current = fs::symlink_metadata(&self.path).map_err(|_| CredentialError::InvalidHome)?;
        let held = self
            .file
            .metadata()
            .map_err(|_| CredentialError::InvalidHome)?;
        if !self.path.is_absolute()
            || self.path.canonicalize().ok().as_ref().map(|path| path.as_os_str())
                != Some(self.path.as_os_str())
            || !current.is_dir()
            || current.file_type().is_symlink()
            // geteuid reads the process identity and has no pointer arguments.
            || current.uid() != unsafe { libc::geteuid() }
            || current.mode() & 0o077 != 0
            || (current.dev(), current.ino(), current.uid(), current.mode()) != self.identity
            || (held.dev(), held.ino(), held.uid(), held.mode()) != self.identity
        {
            return Err(CredentialError::InvalidHome);
        }
        Ok(())
    }
}

async fn connect(
    bus: &LinuxCredentialBus,
) -> Result<secret_service::SecretService<'static>, CredentialError> {
    bus.recheck().map_err(|_| CredentialError::Unavailable)?;
    let connection = zbus::connection::Builder::address(bus.address())
        .map_err(|_| CredentialError::Unavailable)?
        .build()
        .await
        .map_err(|_| CredentialError::Unavailable)?;
    bus.recheck().map_err(|_| CredentialError::Unavailable)?;
    secret_service::SecretService::connect_with_existing(
        secret_service::EncryptionType::Dh,
        connection,
    )
    .await
    .map_err(|_| CredentialError::Unavailable)
}

fn bounded<T>(
    future: impl std::future::Future<Output = Result<T, CredentialError>>,
) -> Result<T, CredentialError> {
    tauri::async_runtime::block_on(async {
        tokio::time::timeout(std::time::Duration::from_secs(5), future)
            .await
            .map_err(|_| CredentialError::Unavailable)?
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{PermissionsExt, symlink};

    #[test]
    fn direct_account_matches_pinned_codex_path_hash_and_distinguishes_homes() {
        let home = Path::new(
            "/home/example/OpenUsage/workspace/profiles/00000000-0000-4000-8000-000000000001",
        );
        assert_eq!(
            account_from_canonical_path(home).unwrap(),
            "cli|c742e685a7c0e636"
        );
        let temporary = tempfile::tempdir().unwrap();
        let first = temporary.path().join("first");
        let second = temporary.path().join("second");
        for path in [&first, &second] {
            fs::create_dir(path).unwrap();
            fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
        }
        let before = account_for_home(&first).unwrap();
        assert_ne!(before, account_for_home(&second).unwrap());
        assert_eq!(before, account_for_home(&first).unwrap());
        symlink(&first, temporary.path().join("alias")).unwrap();
        assert_eq!(
            account_for_home(&temporary.path().join("alias")),
            Err(CredentialError::InvalidHome)
        );
        fs::set_permissions(&first, fs::Permissions::from_mode(0o750)).unwrap();
        assert_eq!(account_for_home(&first), Err(CredentialError::InvalidHome));
    }

    #[test]
    fn replacement_home_cannot_inherit_held_cleanup_authority() {
        let temporary = tempfile::tempdir().unwrap();
        let home = temporary.path().join("selected");
        fs::create_dir(&home).unwrap();
        fs::set_permissions(&home, fs::Permissions::from_mode(0o700)).unwrap();
        let held = HeldHome::open(&home).unwrap();
        fs::rename(&home, temporary.path().join("original")).unwrap();
        fs::create_dir(&home).unwrap();
        fs::set_permissions(&home, fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(held.recheck(), Err(CredentialError::InvalidHome));
    }
}
