//! Linux coordination for this application's enrollment operations.
use std::path::Path;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BoardGuardError {
    Busy,
    UnsafePath,
    Unavailable,
    Unsupported,
}

/// Hold this guard across each complete enrollment operation, including network calls.
/// Production callers use `for_current_user` for the fixed OS credential slot.
/// The empty lock file persists after release. Never remove it to clear a busy guard.
/// This coordinates participating processes, not other programs under the same OS user.
pub struct BoardGuard {
    #[cfg(target_os = "linux")]
    _lock: std::fs::File,
    #[cfg(target_os = "linux")]
    _directory: std::fs::File,
}

impl BoardGuard {
    /// Use one Linux namespace per OS user, independent of HOME and XDG settings.
    /// Require an existing private `/run/user/<uid>` directory. There is no fallback.
    pub fn for_current_user() -> Result<Self, BoardGuardError> {
        #[cfg(target_os = "linux")]
        {
            linux::for_current_user()
        }
        #[cfg(not(target_os = "linux"))]
        {
            Err(BoardGuardError::Unsupported)
        }
    }

    /// Select an explicit path for private fixtures, never the production OS slot.
    /// Create missing directories with mode 0700 and the empty lock with mode 0600.
    /// Require an absolute path with no links, `.` components, or `..` components.
    pub fn acquire(directory: &Path) -> Result<Self, BoardGuardError> {
        #[cfg(target_os = "linux")]
        {
            linux::acquire(directory)
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = directory;
            Err(BoardGuardError::Unsupported)
        }
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use super::{BoardGuard, BoardGuardError};
    use rustix::fs::{AtFlags, FileType, Mode, OFlags, Stat, fstat, mkdirat, open, openat, statat};
    use rustix::io::Errno;
    use std::ffi::OsStr;
    use std::fs::{File, TryLockError};
    use std::os::fd::OwnedFd;
    use std::os::unix::ffi::OsStrExt;
    use std::path::{Path, PathBuf};

    const LOCK_NAME: &str = "enrollment.lock";
    const USER_DIRECTORY: &str = "app.openusage.desktop-leaderboard";
    const DIRECTORY_FLAGS: OFlags = OFlags::RDONLY
        .union(OFlags::DIRECTORY)
        .union(OFlags::NOFOLLOW)
        .union(OFlags::CLOEXEC);

    pub(super) fn acquire(path: &Path) -> Result<BoardGuard, BoardGuardError> {
        acquire_path(path, current_uid()?, 0, None)
    }

    pub(super) fn for_current_user() -> Result<BoardGuard, BoardGuardError> {
        let uid = current_uid()?;
        // /run, /run/user, and /run/user/<uid> must already exist. Only the
        // app directory can be created, and the OS user directory must be private.
        acquire_path(&production_directory(uid), uid, 3, Some(2))
    }

    fn production_directory(uid: u32) -> PathBuf {
        Path::new("/run/user")
            .join(uid.to_string())
            .join(USER_DIRECTORY)
    }

    fn current_uid() -> Result<u32, BoardGuardError> {
        let uid = rustix::process::geteuid().as_raw();
        // A set-user-ID invocation must not create a guard for another account.
        if rustix::process::getuid().as_raw() != uid {
            return Err(BoardGuardError::UnsafePath);
        }
        Ok(uid)
    }

    fn acquire_path(
        path: &Path,
        uid: u32,
        create_from: usize,
        private_parent: Option<usize>,
    ) -> Result<BoardGuard, BoardGuardError> {
        let components = checked_components(path)?;
        let directory = open_directory(&components, uid, Some(create_from), private_parent)?;
        let directory_stat = fstat(&directory).map_err(map_error)?;
        let flags = OFlags::RDWR | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK;
        let lock = match statat(&directory, LOCK_NAME, AtFlags::SYMLINK_NOFOLLOW) {
            Ok(stat) => {
                check_file(&stat, uid)?;
                openat(&directory, LOCK_NAME, flags, Mode::empty()).map_err(map_error)?
            }
            Err(Errno::NOENT) => {
                match openat(
                    &directory,
                    LOCK_NAME,
                    flags | OFlags::CREATE | OFlags::EXCL,
                    Mode::RUSR | Mode::WUSR,
                ) {
                    Ok(file) => file,
                    // Another cooperating process can create the empty file first.
                    Err(Errno::EXIST) => {
                        openat(&directory, LOCK_NAME, flags, Mode::empty()).map_err(map_error)?
                    }
                    Err(error) => return Err(map_error(error)),
                }
            }
            Err(error) => return Err(map_error(error)),
        };
        check_file(&fstat(&lock).map_err(map_error)?, uid)?;
        let lock = File::from(lock);
        lock.try_lock().map_err(|error| match error {
            TryLockError::WouldBlock => BoardGuardError::Busy,
            TryLockError::Error(_) => BoardGuardError::Unavailable,
        })?;

        // Check the names again after locking. A replaced directory or file must
        // not give two successful guards for different inodes at the same path.
        let named_directory = open_directory(&components, uid, None, private_parent)?;
        let named_directory_stat = fstat(&named_directory).map_err(map_error)?;
        if !same_inode(&directory_stat, &named_directory_stat) {
            return Err(BoardGuardError::UnsafePath);
        }
        let lock_stat = fstat(&lock).map_err(map_error)?;
        check_file(&lock_stat, uid)?;
        let named_lock =
            statat(&named_directory, LOCK_NAME, AtFlags::SYMLINK_NOFOLLOW).map_err(map_error)?;
        check_file(&named_lock, uid)?;
        if !same_inode(&lock_stat, &named_lock) {
            return Err(BoardGuardError::UnsafePath);
        }
        Ok(BoardGuard {
            _lock: lock,
            _directory: File::from(directory),
        })
    }

    fn checked_components(path: &Path) -> Result<Vec<&OsStr>, BoardGuardError> {
        let bytes = path.as_os_str().as_bytes();
        if !path.is_absolute() || bytes.len() > 4096 || bytes.contains(&0) {
            return Err(BoardGuardError::UnsafePath);
        }
        let components: Vec<_> = bytes[1..].split(|byte| *byte == b'/').collect();
        if components.len() > 128
            || components
                .iter()
                .any(|part| part.is_empty() || *part == b"." || *part == b"..")
        {
            return Err(BoardGuardError::UnsafePath);
        }
        Ok(components.into_iter().map(OsStr::from_bytes).collect())
    }

    fn open_directory(
        components: &[&OsStr],
        uid: u32,
        create_from: Option<usize>,
        private_parent: Option<usize>,
    ) -> Result<OwnedFd, BoardGuardError> {
        let mut parent = open("/", DIRECTORY_FLAGS, Mode::empty()).map_err(map_error)?;
        for (index, name) in components.iter().enumerate() {
            check_parent(&fstat(&parent).map_err(map_error)?, uid)?;
            let final_component = index == components.len() - 1;
            let (next, created) = match openat(&parent, *name, DIRECTORY_FLAGS, Mode::empty()) {
                Ok(next) => (next, false),
                Err(Errno::NOENT) if create_from.is_some_and(|first| index >= first) => {
                    match mkdirat(&parent, *name, Mode::RWXU) {
                        Ok(()) | Err(Errno::EXIST) => {}
                        Err(error) => return Err(map_error(error)),
                    }
                    (
                        openat(&parent, *name, DIRECTORY_FLAGS, Mode::empty())
                            .map_err(map_error)?,
                        true,
                    )
                }
                Err(error) => {
                    let error = map_error(error);
                    return Err(
                        if private_parent.is_some_and(|parent| index <= parent)
                            && error == BoardGuardError::Unavailable
                        {
                            BoardGuardError::Unsupported
                        } else {
                            error
                        },
                    );
                }
            };
            parent = next;
            if created || final_component || private_parent == Some(index) {
                let stat = fstat(&parent).map_err(map_error)?;
                if stat.st_uid != uid || stat.st_mode & 0o7777 != 0o700 {
                    return Err(BoardGuardError::UnsafePath);
                }
            }
        }
        Ok(parent)
    }

    fn check_parent(stat: &Stat, uid: u32) -> Result<(), BoardGuardError> {
        // Root-owned sticky directories permit private fixtures beneath /tmp.
        let sticky_root = stat.st_uid == 0 && stat.st_mode & 0o1000 != 0;
        if (stat.st_uid != uid && stat.st_uid != 0) || (stat.st_mode & 0o022 != 0 && !sticky_root) {
            return Err(BoardGuardError::UnsafePath);
        }
        Ok(())
    }

    fn check_file(stat: &Stat, uid: u32) -> Result<(), BoardGuardError> {
        if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile
            || stat.st_uid != uid
            || stat.st_mode & 0o7777 != 0o600
            || stat.st_nlink != 1
            || stat.st_size != 0
        {
            return Err(BoardGuardError::UnsafePath);
        }
        Ok(())
    }

    fn same_inode(left: &Stat, right: &Stat) -> bool {
        left.st_dev == right.st_dev && left.st_ino == right.st_ino
    }

    fn map_error(error: Errno) -> BoardGuardError {
        match error {
            Errno::LOOP
            | Errno::NOTDIR
            | Errno::ISDIR
            | Errno::NAMETOOLONG
            | Errno::INVAL
            | Errno::ACCESS
            | Errno::PERM => BoardGuardError::UnsafePath,
            _ => BoardGuardError::Unavailable,
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn production_namespace_depends_only_on_the_os_user() {
            for uid in [0, 1000, u32::MAX] {
                assert_eq!(
                    production_directory(uid),
                    PathBuf::from(format!("/run/user/{uid}/app.openusage.desktop-leaderboard"))
                );
            }
        }

        #[test]
        fn fixed_runtime_policy_never_creates_a_missing_runtime_or_accepts_shared_modes() {
            use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
            let nonce = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let root = std::env::temp_dir().join(format!(
                "openusage-board-runtime-{}-{nonce}",
                std::process::id()
            ));
            std::fs::DirBuilder::new()
                .mode(0o700)
                .create(&root)
                .unwrap();
            let runtime = root.join("synthetic-runtime");
            let directory = runtime.join(USER_DIRECTORY);
            let components = checked_components(&directory).unwrap();
            let uid = current_uid().unwrap();
            let acquire = || {
                acquire_path(
                    &directory,
                    uid,
                    components.len() - 1,
                    Some(components.len() - 2),
                )
            };
            assert!(matches!(acquire(), Err(BoardGuardError::Unsupported)));
            assert!(!runtime.exists());
            std::fs::DirBuilder::new()
                .mode(0o755)
                .create(&runtime)
                .unwrap();
            std::fs::set_permissions(&runtime, std::fs::Permissions::from_mode(0o755)).unwrap();
            assert!(matches!(acquire(), Err(BoardGuardError::UnsafePath)));
            assert!(!directory.exists());
            std::fs::set_permissions(&runtime, std::fs::Permissions::from_mode(0o700)).unwrap();
            let guard = acquire().unwrap();
            assert!(matches!(acquire(), Err(BoardGuardError::Busy)));
            drop(guard);
            drop(acquire().unwrap());
            std::fs::remove_dir_all(root).unwrap();
        }

        #[test]
        fn owner_checks_refuse_synthetic_foreign_metadata() {
            let uid = rustix::process::geteuid().as_raw();
            let root = open("/", DIRECTORY_FLAGS, Mode::empty()).unwrap();
            let mut stat = fstat(&root).unwrap();
            stat.st_uid = uid.wrapping_add(1).max(1);
            stat.st_mode = 0o040700;
            assert_eq!(check_parent(&stat, uid), Err(BoardGuardError::UnsafePath));
            stat.st_mode = 0o100600;
            stat.st_nlink = 1;
            stat.st_size = 0;
            assert_eq!(check_file(&stat, uid), Err(BoardGuardError::UnsafePath));
            stat.st_uid = uid;
            assert_eq!(check_file(&stat, uid), Ok(()));
        }
    }
}
