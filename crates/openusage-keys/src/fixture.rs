//! Native tests must prove that their D-Bus endpoint stays inside a private fixture.
use crate::VaultError;
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::path::{Path, PathBuf};

const DIRECTORY_VARIABLES: [&str; 6] = [
    "HOME",
    "XDG_CONFIG_HOME",
    "XDG_DATA_HOME",
    "XDG_CACHE_HOME",
    "XDG_STATE_HOME",
    "XDG_RUNTIME_DIR",
];

/// Check private fixture paths and the exact owned socket before any keyring call.
/// This helper reads filesystem metadata only. It never connects to D-Bus.
pub fn private_keyring_fixture() -> Result<PathBuf, VaultError> {
    let root = environment_path("OPENUSAGE_PRIVATE_KEYRING_FIXTURE")?;
    let directories = DIRECTORY_VARIABLES
        .iter()
        .map(|name| environment_path(name))
        .collect::<Result<Vec<_>, _>>()?;
    let address = std::env::var("DBUS_SESSION_BUS_ADDRESS").map_err(|_| VaultError::InvalidKey)?;
    validate(&root, &directories, &address)?;
    Ok(root)
}

fn environment_path(name: &str) -> Result<PathBuf, VaultError> {
    std::env::var_os(name)
        .map(PathBuf::from)
        .ok_or(VaultError::InvalidKey)
}

fn validate(root: &Path, directories: &[PathBuf], address: &str) -> Result<(), VaultError> {
    let uid = rustix::process::geteuid().as_raw();
    if rustix::process::getuid().as_raw() != uid || directories.len() != DIRECTORY_VARIABLES.len() {
        return Err(VaultError::InvalidKey);
    }
    private_directory(root, uid)?;
    for directory in directories {
        if directory == root || !directory.starts_with(root) {
            return Err(VaultError::InvalidKey);
        }
        private_directory(directory, uid)?;
    }
    let socket = directories
        .last()
        .ok_or(VaultError::InvalidKey)?
        .join("bus");
    let socket_text = socket.to_str().ok_or(VaultError::InvalidKey)?;
    // The harness uses a literal path. Reject characters that need D-Bus escaping
    // so an exact string cannot introduce another address or a parameter.
    if !socket_text
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || b"/_-.".contains(&byte))
    {
        return Err(VaultError::InvalidKey);
    }
    let expected = format!("unix:path={socket_text}");
    if address != expected {
        let guid = address
            .strip_prefix(&expected)
            .and_then(|suffix| suffix.strip_prefix(",guid="))
            .ok_or(VaultError::InvalidKey)?;
        if guid.len() != 32
            || !guid
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(VaultError::InvalidKey);
        }
    }
    let metadata = std::fs::symlink_metadata(&socket).map_err(|_| VaultError::InvalidKey)?;
    if !metadata.file_type().is_socket() || metadata.uid() != uid || metadata.nlink() != 1 {
        return Err(VaultError::InvalidKey);
    }
    canonical_path(&socket)
}

fn private_directory(path: &Path, uid: u32) -> Result<(), VaultError> {
    let metadata = std::fs::symlink_metadata(path).map_err(|_| VaultError::InvalidKey)?;
    if !metadata.is_dir() || metadata.uid() != uid || metadata.mode() & 0o7777 != 0o700 {
        return Err(VaultError::InvalidKey);
    }
    canonical_path(path)
}

fn canonical_path(path: &Path) -> Result<(), VaultError> {
    if !path.is_absolute()
        || path
            .canonicalize()
            .map_err(|_| VaultError::InvalidKey)?
            .as_os_str()
            != path.as_os_str()
    {
        return Err(VaultError::InvalidKey);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, DirBuilder};
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt, symlink};
    use std::os::unix::net::UnixListener;

    struct Fixture {
        root: PathBuf,
        directories: Vec<PathBuf>,
        _socket: UnixListener,
    }

    impl Fixture {
        fn new() -> Self {
            let nonce = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let root =
                std::env::temp_dir().join(format!("ou-keyring-{}-{nonce}", std::process::id()));
            DirBuilder::new().mode(0o700).create(&root).unwrap();
            let directories: Vec<_> = ["home", "config", "data", "cache", "state", "runtime"]
                .map(|name| root.join(name))
                .into();
            for directory in &directories {
                DirBuilder::new().mode(0o700).create(directory).unwrap();
            }
            let socket = UnixListener::bind(directories.last().unwrap().join("bus")).unwrap();
            Self {
                root,
                directories,
                _socket: socket,
            }
        }

        fn socket_path(&self) -> PathBuf {
            self.directories.last().unwrap().join("bus")
        }

        fn address(&self) -> String {
            format!("unix:path={}", self.socket_path().display())
        }

        fn check(&self, address: &str) -> Result<(), VaultError> {
            validate(&self.root, &self.directories, address)
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn fixture_accepts_the_exact_private_socket_and_one_optional_guid() {
        let fixture = Fixture::new();
        assert_eq!(fixture.check(&fixture.address()), Ok(()));
        assert_eq!(
            fixture.check(&format!(
                "{},guid=0123456789abcdef0123456789abcdef",
                fixture.address()
            )),
            Ok(())
        );
    }

    #[test]
    fn fixture_refuses_path_escape_fallbacks_and_extra_address_components() {
        let fixture = Fixture::new();
        let address = fixture.address();
        for invalid in [
            format!(
                "unix:path={}/../synthetic-other/bus",
                fixture.root.display()
            ),
            format!("{address};unix:path=/synthetic-other/bus"),
            format!("unix:path=/synthetic-other/bus;{address}"),
            format!("{address},guid=0123456789ABCDEF0123456789ABCDEF"),
            format!("{address},guid=0123456789abcdef0123456789abcdef,extra=value"),
            format!("{address},guid=0123456789abcdef0123456789abcdeg"),
            format!("{address},guid="),
            format!("{address},guid={}", "a".repeat(33)),
            format!("{address},path=/synthetic-other/bus"),
            format!("{address},"),
            address.replace("/bus", "/./bus"),
            address.replace("/bus", "/%62us"),
            "unix:abstract=synthetic-other".into(),
        ] {
            assert_eq!(fixture.check(&invalid), Err(VaultError::InvalidKey));
        }
    }

    #[test]
    fn fixture_refuses_nonprivate_linked_or_external_directories() {
        let mut fixture = Fixture::new();
        let address = fixture.address();
        for directory in std::iter::once(&fixture.root).chain(fixture.directories.iter()) {
            fs::set_permissions(directory, fs::Permissions::from_mode(0o755)).unwrap();
            assert_eq!(fixture.check(&address), Err(VaultError::InvalidKey));
            fs::set_permissions(directory, fs::Permissions::from_mode(0o700)).unwrap();
        }
        let home = fixture.directories[0].clone();
        for path in [
            fixture.root.clone(),
            PathBuf::from("/synthetic-other/home"),
            home.join("."),
        ] {
            fixture.directories[0] = path;
            assert_eq!(fixture.check(&address), Err(VaultError::InvalidKey));
        }
        let alias = fixture.root.join("linked-home");
        symlink(&home, &alias).unwrap();
        fixture.directories[0] = alias;
        assert_eq!(fixture.check(&address), Err(VaultError::InvalidKey));
        fixture.directories[0] = home;
        assert_eq!(fixture.check(&address), Ok(()));
    }

    #[test]
    fn fixture_refuses_regular_linked_and_hardlinked_socket_paths() {
        let fixture = Fixture::new();
        let address = fixture.address();
        let socket = fixture.socket_path();
        let alias = fixture.root.join("socket-alias");
        fs::hard_link(&socket, &alias).unwrap();
        assert_eq!(fixture.check(&address), Err(VaultError::InvalidKey));
        fs::remove_file(&alias).unwrap();
        fs::rename(&socket, &alias).unwrap();
        symlink(&alias, &socket).unwrap();
        assert_eq!(fixture.check(&address), Err(VaultError::InvalidKey));
        fs::remove_file(&socket).unwrap();
        fs::write(&socket, b"synthetic regular file").unwrap();
        assert_eq!(fixture.check(&address), Err(VaultError::InvalidKey));
    }
}
