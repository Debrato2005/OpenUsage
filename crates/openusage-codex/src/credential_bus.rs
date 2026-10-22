//! Explicit desktop-bus access for a reviewed provider's isolated credential store.

use crate::session::SessionError;
#[cfg(target_os = "linux")]
use std::path::PathBuf;

/// A single Linux Unix-socket bus, bound to its reviewed path and process owner.
/// No environment lookup, address alternatives, activation or network transport is permitted.
#[derive(Clone)]
pub struct LinuxCredentialBus {
    address: String,
    #[cfg(target_os = "linux")]
    path: PathBuf,
    #[cfg(target_os = "linux")]
    identities: Vec<(PathBuf, u64, u64)>,
    #[cfg(target_os = "linux")]
    socket: std::sync::Arc<std::fs::File>,
}

impl LinuxCredentialBus {
    pub fn new(address: &str) -> Result<Self, SessionError> {
        #[cfg(target_os = "linux")]
        {
            let path = parse_address(address)?;
            use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
            let identities = inspect(&path)?;
            // O_PATH pins the socket inode without opening a data channel. An unlinked
            // endpoint cannot be recycled into an apparently identical replacement.
            let socket = std::fs::OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_PATH | libc::O_NOFOLLOW | libc::O_CLOEXEC)
                .open(&path)
                .map_err(|_| SessionError::CredentialStoreUnavailable)?;
            let held = socket
                .metadata()
                .map_err(|_| SessionError::CredentialStoreUnavailable)?;
            if (held.dev(), held.ino()) != (identities[0].1, identities[0].2) {
                return Err(SessionError::CredentialStoreUnavailable);
            }
            peer(&path)?;
            if inspect(&path)? != identities {
                return Err(SessionError::CredentialStoreUnavailable);
            }
            Ok(Self {
                address: address.into(),
                path,
                identities,
                socket: std::sync::Arc::new(socket),
            })
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = address;
            Err(SessionError::UnsupportedPlatform)
        }
    }

    pub fn address(&self) -> &str {
        &self.address
    }

    pub fn recheck(&self) -> Result<(), SessionError> {
        #[cfg(target_os = "linux")]
        {
            use std::os::unix::fs::MetadataExt;
            let held = self
                .socket
                .metadata()
                .map_err(|_| SessionError::CredentialStoreUnavailable)?;
            if (held.dev(), held.ino()) != (self.identities[0].1, self.identities[0].2)
                || inspect(&self.path)? != self.identities
            {
                return Err(SessionError::CredentialStoreUnavailable);
            }
            peer(&self.path)?;
            if inspect(&self.path)? != self.identities {
                return Err(SessionError::CredentialStoreUnavailable);
            }
            Ok(())
        }
        #[cfg(not(target_os = "linux"))]
        Err(SessionError::UnsupportedPlatform)
    }
}

#[cfg(target_os = "linux")]
fn parse_address(address: &str) -> Result<PathBuf, SessionError> {
    let invalid = SessionError::CredentialStoreUnavailable;
    if address.len() > 512
        || !address.is_ascii()
        || address.bytes().any(|b| b.is_ascii_control())
        || address.contains([';', '%'])
    {
        return Err(invalid);
    }
    let raw = address.strip_prefix("unix:path=").ok_or(invalid)?;
    let path = if let Some((path, guid)) = raw.split_once(",guid=") {
        if guid.len() != 32 || !guid.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(invalid);
        }
        path
    } else {
        raw
    };
    if path.contains(',') || path.len() >= 108 {
        return Err(invalid);
    }
    let path = PathBuf::from(path);
    if !crate::session::unix::absolute_normal_path(&path) {
        return Err(invalid);
    }
    Ok(path)
}

#[cfg(target_os = "linux")]
fn inspect(path: &std::path::Path) -> Result<Vec<(PathBuf, u64, u64)>, SessionError> {
    use std::os::unix::fs::{FileTypeExt, MetadataExt};
    let invalid = SessionError::CredentialStoreUnavailable;
    let uid = unsafe { libc::geteuid() };
    if path.canonicalize().ok().as_deref() != Some(path) {
        return Err(invalid);
    }
    let socket = std::fs::symlink_metadata(path).map_err(|_| invalid)?;
    if !socket.file_type().is_socket() || socket.uid() != uid {
        return Err(invalid);
    }
    let mut identities = vec![(path.to_owned(), socket.dev(), socket.ino())];
    let mut child = socket;
    for parent in path.ancestors().skip(1) {
        let metadata = std::fs::symlink_metadata(parent).map_err(|_| invalid)?;
        if !metadata.is_dir()
            || metadata.file_type().is_symlink()
            || !(metadata.uid() == 0 || metadata.uid() == uid)
            || (metadata.mode() & 0o022 != 0
                && !(metadata.mode() & 0o1000 != 0 && (child.uid() == 0 || child.uid() == uid)))
        {
            return Err(invalid);
        }
        identities.push((parent.to_owned(), metadata.dev(), metadata.ino()));
        child = metadata;
    }
    Ok(identities)
}

#[cfg(target_os = "linux")]
fn peer(path: &std::path::Path) -> Result<(), SessionError> {
    use std::os::{
        fd::{AsRawFd, FromRawFd, OwnedFd},
        unix::ffi::OsStrExt,
    };
    let invalid = SessionError::CredentialStoreUnavailable;
    // Nonblocking Unix connect cannot wait for a stalled or full listener queue.
    let raw = unsafe {
        libc::socket(
            libc::AF_UNIX,
            libc::SOCK_STREAM | libc::SOCK_CLOEXEC | libc::SOCK_NONBLOCK,
            0,
        )
    };
    if raw < 0 {
        return Err(invalid);
    }
    let socket = unsafe { OwnedFd::from_raw_fd(raw) };
    let mut address: libc::sockaddr_un = unsafe { std::mem::zeroed() };
    address.sun_family = libc::AF_UNIX as libc::sa_family_t;
    let bytes = path.as_os_str().as_bytes();
    if bytes.len() >= address.sun_path.len() {
        return Err(invalid);
    }
    for (slot, byte) in address.sun_path.iter_mut().zip(bytes) {
        *slot = *byte as libc::c_char;
    }
    let length = std::mem::offset_of!(libc::sockaddr_un, sun_path) + bytes.len() + 1;
    if unsafe {
        libc::connect(
            socket.as_raw_fd(),
            (&address as *const libc::sockaddr_un).cast(),
            length as libc::socklen_t,
        )
    } != 0
    {
        return Err(invalid);
    }
    let mut credential: libc::ucred = unsafe { std::mem::zeroed() };
    let mut length = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
    // SO_PEERCRED checks the listener's effective identity without a D-Bus exchange.
    if unsafe {
        libc::getsockopt(
            socket.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            (&mut credential as *mut libc::ucred).cast(),
            &mut length,
        )
    } != 0
        || length as usize != std::mem::size_of::<libc::ucred>()
        || credential.uid != unsafe { libc::geteuid() }
    {
        return Err(invalid);
    }
    Ok(())
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use std::os::unix::{
        fs::{PermissionsExt, symlink},
        net::UnixListener,
    };

    #[test]
    fn credential_bus_rejects_alternatives_links_unsafe_parents_and_replaced_sockets() {
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("bus");
        let listener = UnixListener::bind(&path).unwrap();
        let address = format!("unix:path={}", path.display());
        let bus = LinuxCredentialBus::new(&address).unwrap();
        bus.recheck().unwrap();
        for invalid in [
            format!("{address};{address}"),
            format!("{address},guid=bad"),
            "unix:abstract=bus".into(),
            "tcp:host=localhost,port=1234".into(),
            "unix:path=relative".into(),
            format!("{address},unknown=value"),
            format!("{address}%00"),
        ] {
            assert!(LinuxCredentialBus::new(&invalid).is_err());
        }
        let link = temporary.path().join("linked");
        symlink(&path, &link).unwrap();
        assert!(LinuxCredentialBus::new(&format!("unix:path={}", link.display())).is_err());
        std::fs::set_permissions(temporary.path(), std::fs::Permissions::from_mode(0o777)).unwrap();
        assert!(bus.recheck().is_err());
        std::fs::set_permissions(temporary.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        drop(listener);
        std::fs::remove_file(&path).unwrap();
        let _replacement = UnixListener::bind(&path).unwrap();
        assert!(bus.recheck().is_err());
    }
}
