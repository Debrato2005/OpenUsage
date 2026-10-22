//! Non-secret storage choice for managed homes. Never read provider credential files.

use super::*;
#[cfg(target_os = "linux")]
use std::io::Write;

const MARKER: &str = ".openusage-credentials-v1";
const KEYRING: &[u8] = b"openusage-provider-keyring-direct-v1\n";

fn unavailable() -> CommandError {
    CommandError::ProviderCredentialStoreUnavailable
}

fn marked(path: &Path) -> Result<bool, CommandError> {
    let name = path.join(MARKER);
    match fs::symlink_metadata(&name) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(_) => return Err(unavailable()),
        Ok(metadata) if !metadata.is_file() || metadata.file_type().is_symlink() => {
            return Err(unavailable());
        }
        Ok(_) => {}
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let mut file = options.open(&name).map_err(|_| unavailable())?;
    let metadata = file.metadata().map_err(|_| unavailable())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let current = fs::symlink_metadata(&name).map_err(|_| unavailable())?;
        if metadata.uid() != unsafe { libc::geteuid() }
            || metadata.mode() & 0o777 != 0o600
            || metadata.nlink() != 1
            || current.dev() != metadata.dev()
            || current.ino() != metadata.ino()
        {
            return Err(unavailable());
        }
    }
    if !metadata.is_file() || metadata.len() != KEYRING.len() as u64 {
        return Err(unavailable());
    }
    let mut content = [0; KEYRING.len()];
    file.read_exact(&mut content).map_err(|_| unavailable())?;
    if content != KEYRING {
        return Err(unavailable());
    }
    Ok(true)
}

pub(super) fn apply(
    home: ManagedProfileHome,
    path: &Path,
    source: ExecutableSourceKind,
) -> Result<ManagedProfileHome, CommandError> {
    let protected = marked(path)?;
    if source != ExecutableSourceKind::OfficialArtifact {
        return if protected {
            Err(unavailable())
        } else {
            Ok(home)
        };
    }
    #[cfg(target_os = "linux")]
    {
        let address = std::env::var("DBUS_SESSION_BUS_ADDRESS").ok();
        if protected {
            let address = address.as_deref().ok_or_else(unavailable)?;
            crate::provider_credentials::available(address).map_err(|_| unavailable())?;
            return home.with_linux_keyring(address).map_err(|_| unavailable());
        }
        // An existing home can contain a provider-managed file session. Do not migrate,
        // copy, inspect or abandon it implicitly. Fresh restore homes are also empty.
        if fs::read_dir(path)
            .map_err(|_| unavailable())?
            .next()
            .is_some()
        {
            return Ok(home);
        }
        let Some(address) = address else {
            return Ok(home);
        };
        let protected_home = match home.clone().with_linux_keyring(&address) {
            Ok(home) => home,
            Err(_) => return Ok(home),
        };
        if crate::provider_credentials::available(&address).is_err() {
            return Ok(home);
        }
        use std::os::unix::fs::OpenOptionsExt;
        let mut marker = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(path.join(MARKER))
            .map_err(|_| unavailable())?;
        marker
            .write_all(KEYRING)
            .and_then(|()| marker.sync_all())
            .map_err(|_| unavailable())?;
        File::open(path)
            .and_then(|directory| directory.sync_all())
            .map_err(|_| unavailable())?;
        Ok(protected_home)
    }
    #[cfg(not(target_os = "linux"))]
    {
        if protected {
            Err(unavailable())
        } else {
            Ok(home)
        }
    }
}

/// Called only after confirmed workspace deletion stops workers and validates this tree.
/// Keep the marker and home until exact OS-entry removal succeeds, including on retry.
pub(crate) fn remove_marked(profiles: &Path) -> Result<(), CommandError> {
    for entry in fs::read_dir(profiles).map_err(|_| unavailable())? {
        let path = entry.map_err(|_| unavailable())?.path();
        if !marked(&path)? {
            continue;
        }
        #[cfg(target_os = "linux")]
        {
            let address = std::env::var("DBUS_SESSION_BUS_ADDRESS").map_err(|_| unavailable())?;
            crate::provider_credentials::remove(&path, &address).map_err(|_| unavailable())?;
        }
        #[cfg(not(target_os = "linux"))]
        return Err(unavailable());
    }
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::{PermissionsExt, symlink};

    #[test]
    fn invalid_or_linked_marker_never_silently_downgrades_to_file_storage() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path();
        assert!(!marked(path).unwrap());
        let marker = path.join(MARKER);
        fs::write(&marker, KEYRING).unwrap();
        fs::set_permissions(&marker, fs::Permissions::from_mode(0o600)).unwrap();
        assert!(marked(path).unwrap());
        fs::set_permissions(&marker, fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(marked(path), Err(unavailable()));
        fs::set_permissions(&marker, fs::Permissions::from_mode(0o600)).unwrap();
        fs::write(&marker, b"partial").unwrap();
        assert_eq!(marked(path), Err(unavailable()));
        fs::remove_file(&marker).unwrap();
        let outside = directory.path().join("outside");
        fs::write(&outside, KEYRING).unwrap();
        symlink(&outside, &marker).unwrap();
        assert_eq!(marked(path), Err(unavailable()));
        assert_eq!(fs::read(outside).unwrap(), KEYRING);
    }
}
