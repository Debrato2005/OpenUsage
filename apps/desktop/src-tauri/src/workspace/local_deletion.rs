use super::*;
#[cfg(unix)]
use serde::{Deserialize, Serialize};
#[cfg(unix)]
use sha2::{Digest, Sha256};

const MARKER: &str = ".local-deletion.v1";
#[cfg(unix)]
const LIMIT: u64 = 600;
#[cfg(unix)]
const MANAGED_DIRS: [&str; 4] = [
    "profiles",
    "backup-inspection",
    "restore-staging",
    "restore-checkpoints",
];
#[cfg(unix)]
const MANAGED_FILES: [&str; 4] = [
    DATABASE_FILE,
    "openusage.db-wal",
    "openusage.db-shm",
    WRAPPED_KEY_FILE,
];

pub(super) fn pending(directory: &Path) -> bool {
    match fs::symlink_metadata(directory.join(MARKER)) {
        Ok(_) => true,
        Err(error) => error.kind() != std::io::ErrorKind::NotFound,
    }
}

#[cfg(unix)]
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Intent {
    version: u8,
    mode: Mode,
    os_key_fingerprint: Option<String>,
    database_identity: [u64; 2],
    wrapped_identity: Option<[u64; 2]>,
}

#[cfg(unix)]
#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Mode {
    Passphrase,
    OsStore,
}

#[cfg(unix)]
impl Intent {
    fn valid(&self) -> bool {
        self.version == 1
            && match self.mode {
                Mode::Passphrase => {
                    self.os_key_fingerprint.is_none() && self.wrapped_identity.is_some()
                }
                Mode::OsStore => {
                    self.wrapped_identity.is_none()
                        && self.os_key_fingerprint.as_ref().is_some_and(|value| {
                            value.len() == 64
                                && value
                                    .bytes()
                                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                        })
                }
            }
    }
}

#[cfg(unix)]
fn fingerprint(key: &[u8; 32]) -> String {
    format!("{:x}", Sha256::digest(key))
}

impl Workspace {
    /// Explicitly delete this managed workspace, preserving its instance lease.
    /// A persisted intent blocks normal setup until this operation is resumed.
    pub fn delete_all_local_data(
        &mut self,
        vault: Result<&impl KeyVault, VaultError>,
        confirmed: bool,
    ) -> Result<Bootstrap, CommandError> {
        if !confirmed {
            return Err(CommandError::InvalidArguments);
        }
        // No deletion on a platform where ownership and path identity cannot be proved.
        #[cfg(not(unix))]
        {
            let _ = vault;
            Err(CommandError::LocalDeletionUnsupported)
        }
        #[cfg(unix)]
        {
            self.delete_private_workspace(vault)
        }
    }

    #[cfg(unix)]
    fn delete_private_workspace(
        &mut self,
        vault: Result<&impl KeyVault, VaultError>,
    ) -> Result<Bootstrap, CommandError> {
        let root = Held::open(&self.directory, true)?;
        let lease = Held::open(&self.directory.join(".instance.lock"), false)?;
        if lease.identity != identity(&self._lock.metadata().map_err(failed)?) {
            return Err(CommandError::LocalDeletionUnsafe);
        }
        let resuming = pending(&self.directory);
        if self.local_deletion_pending && !resuming {
            return Err(CommandError::LocalDeletionFailed);
        }
        if !resuming && self.storage.is_none() {
            return Err(CommandError::WorkspaceLocked);
        }
        inspect_root(&root)?;
        let database = optional_held(&self.directory.join(DATABASE_FILE), false)?;
        let wrapped = optional_held(&self.directory.join(WRAPPED_KEY_FILE), false)?;
        let (intent, existing_marker, key) = if resuming {
            let marker = Held::open(&self.directory.join(MARKER), false)?;
            let intent = read_intent(&marker)?;
            check_original(&intent, database.as_ref(), wrapped.as_ref())?;
            let key = resume_key(&intent, vault)?;
            (intent, Some(marker), key)
        } else {
            let database = database.as_ref().ok_or(CommandError::LocalDeletionUnsafe)?;
            let (mode, key) = match self.protection {
                KeyProtection::OsStore if wrapped.is_none() => {
                    let key = vault
                        .map_err(|_| CommandError::KeyStoreUnavailable)?
                        .load()
                        .map_err(|_| CommandError::KeyStoreUnavailable)?
                        .ok_or(CommandError::LocalDeletionUnsafe)?;
                    // Never remove an unrelated OS key merely because the workspace is open.
                    drop(
                        Storage::open(&database.path, &key)
                            .map_err(|_| CommandError::LocalDeletionUnsafe)?,
                    );
                    database.check()?;
                    (Mode::OsStore, Some(key))
                }
                KeyProtection::Passphrase if wrapped.is_some() => (Mode::Passphrase, None),
                _ => return Err(CommandError::LocalDeletionUnsafe),
            };
            let intent = Intent {
                version: 1,
                mode,
                os_key_fingerprint: key.as_ref().map(|key| fingerprint(key)),
                database_identity: database.identity,
                wrapped_identity: wrapped.as_ref().map(|file| file.identity),
            };
            (intent, None, key)
        };
        let monitoring = monitoring::RuntimeMonitoring::new()?;
        self.provider.reset_for_local_deletion()?;
        root.check()?;
        lease.check()?;
        inspect_root(&root)?;
        check_original_paths(&self.directory, &intent)?;
        let marker_result = match existing_marker {
            Some(marker) => Ok(marker),
            None => write_intent(&root, &intent),
        };
        // A partial marker is deliberately retained and must block all normal operations.
        if pending(&self.directory) {
            self.local_deletion_pending = true;
            drop(self.storage.take());
            self.restore_preview.clear();
            self.activity_sessions.clear();
            self.quota_sessions.clear();
            self.quota_requests.clear();
            self.activity_requests.clear();
            self.activity_reviews.clear();
            self.monitoring = monitoring;
        }
        let marker = marker_result?;
        root.check()?;
        lease.check()?;
        check_original_paths(&self.directory, &intent)?;
        // Closing SQLCipher may remove its WAL/SHM; inspect their current state, not a stale inventory.
        inspect_root(&root)?;
        for name in MANAGED_DIRS {
            if let Some(directory) = optional_held(&self.directory.join(name), true)? {
                inspect_tree(&directory, &root, name == "profiles", 0)?;
                directory.check()?;
                root.check()?;
                fs::remove_dir_all(&directory.path).map_err(failed)?;
            }
        }
        root.check()?;
        lease.check()?;
        check_original_paths(&self.directory, &intent)?;
        if let Some(key) = key {
            vault
                .map_err(|_| CommandError::KeyStoreUnavailable)?
                .remove(&key)
                .map_err(|_| CommandError::KeyStoreUnavailable)?;
        }
        for name in MANAGED_FILES {
            if let Some(file) = optional_held(&self.directory.join(name), false)? {
                check_original_paths(&self.directory, &intent)?;
                file.check()?;
                root.check()?;
                fs::remove_file(&file.path).map_err(failed)?;
            }
        }
        root.file.sync_all().map_err(failed)?;
        marker.check()?;
        root.check()?;
        lease.check()?;
        fs::remove_file(&marker.path).map_err(failed)?;
        if root.file.sync_all().is_err() {
            // Restore the blocker if final durability cannot be established.
            let _ = write_intent(&root, &intent);
            return Err(CommandError::LocalDeletionFailed);
        }
        self.local_deletion_pending = false;
        Ok(self.bootstrap(vault))
    }
}

#[cfg(unix)]
fn failed(_: std::io::Error) -> CommandError {
    CommandError::LocalDeletionFailed
}

#[cfg(unix)]
fn resume_key(
    intent: &Intent,
    vault: Result<&impl KeyVault, VaultError>,
) -> Result<Option<crate::keyvault::DatabaseKey>, CommandError> {
    match intent.mode {
        Mode::Passphrase => Ok(None),
        Mode::OsStore => {
            let key = vault
                .map_err(|_| CommandError::KeyStoreUnavailable)?
                .load()
                .map_err(|_| CommandError::KeyStoreUnavailable)?;
            if key
                .as_ref()
                .is_some_and(|key| Some(fingerprint(key)) != intent.os_key_fingerprint)
            {
                return Err(CommandError::LocalDeletionUnsafe);
            }
            Ok(key)
        }
    }
}

#[cfg(unix)]
fn read_intent(marker: &Held) -> Result<Intent, CommandError> {
    if marker.file.metadata().map_err(failed)?.len() > LIMIT {
        return Err(CommandError::LocalDeletionUnsafe);
    }
    let mut bytes = Vec::new();
    (&marker.file)
        .take(LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(failed)?;
    let intent: Intent =
        serde_json::from_slice(&bytes).map_err(|_| CommandError::LocalDeletionUnsafe)?;
    if bytes.len() > LIMIT as usize || !intent.valid() {
        return Err(CommandError::LocalDeletionUnsafe);
    }
    marker.check()?;
    Ok(intent)
}

#[cfg(unix)]
fn write_intent(root: &Held, intent: &Intent) -> Result<Held, CommandError> {
    use std::os::unix::fs::OpenOptionsExt;
    root.check()?;
    let bytes = serde_json::to_vec(intent).map_err(|_| CommandError::LocalDeletionFailed)?;
    if bytes.len() > LIMIT as usize || !intent.valid() {
        return Err(CommandError::LocalDeletionUnsafe);
    }
    let path = root.path.join(MARKER);
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(&path)
        .map_err(failed)?;
    file.write_all(&bytes).map_err(failed)?;
    file.sync_all().map_err(failed)?;
    root.file.sync_all().map_err(failed)?;
    Held::open(&path, false)
}

#[cfg(unix)]
fn check_original(
    intent: &Intent,
    database: Option<&Held>,
    wrapped: Option<&Held>,
) -> Result<(), CommandError> {
    if database.is_some_and(|file| file.identity != intent.database_identity)
        || wrapped.is_some_and(|file| Some(file.identity) != intent.wrapped_identity)
    {
        return Err(CommandError::LocalDeletionUnsafe);
    }
    Ok(())
}

#[cfg(unix)]
fn check_original_paths(root: &Path, intent: &Intent) -> Result<(), CommandError> {
    check_original(
        intent,
        optional_held(&root.join(DATABASE_FILE), false)?.as_ref(),
        optional_held(&root.join(WRAPPED_KEY_FILE), false)?.as_ref(),
    )
}

#[cfg(unix)]
struct Held {
    path: PathBuf,
    file: File,
    identity: [u64; 2],
    directory: bool,
    mount: u64,
    private: bool,
}

#[cfg(unix)]
fn identity(metadata: &fs::Metadata) -> [u64; 2] {
    use std::os::unix::fs::MetadataExt;
    [metadata.dev(), metadata.ino()]
}

#[cfg(unix)]
impl Held {
    fn open(path: &Path, directory: bool) -> Result<Self, CommandError> {
        Self::open_inner(path, directory, true)
    }

    fn open_inner(path: &Path, directory: bool, private: bool) -> Result<Self, CommandError> {
        use std::os::unix::fs::OpenOptionsExt;
        let flags =
            libc::O_NOFOLLOW | libc::O_NONBLOCK | if directory { libc::O_DIRECTORY } else { 0 };
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(flags)
            .open(path)
            .map_err(|_| CommandError::LocalDeletionUnsafe)?;
        let held = Self {
            path: path.to_owned(),
            identity: identity(&file.metadata().map_err(failed)?),
            mount: mount_id(&file)?,
            file,
            directory,
            private,
        };
        held.check()?;
        Ok(held)
    }

    fn check(&self) -> Result<(), CommandError> {
        use std::os::unix::fs::MetadataExt;
        let path =
            fs::symlink_metadata(&self.path).map_err(|_| CommandError::LocalDeletionUnsafe)?;
        let held = self.file.metadata().map_err(failed)?;
        if path.file_type().is_symlink()
            || identity(&path) != self.identity
            || identity(&held) != self.identity
            || path.uid() != unsafe { libc::geteuid() }
            || (self.private && path.mode() & 0o077 != 0)
            || path.is_dir() != self.directory
            || (!self.directory && (!path.is_file() || path.nlink() != 1))
            || mount_id(&self.file)? != self.mount
        {
            return Err(CommandError::LocalDeletionUnsafe);
        }
        Ok(())
    }
}

#[cfg(target_os = "linux")]
fn mount_id(file: &File) -> Result<u64, CommandError> {
    use std::os::fd::AsRawFd;
    let mut stat: libc::statx = unsafe { std::mem::zeroed() };
    let result = unsafe {
        libc::statx(
            file.as_raw_fd(),
            c"".as_ptr(),
            libc::AT_EMPTY_PATH | libc::AT_SYMLINK_NOFOLLOW,
            libc::STATX_MNT_ID,
            &mut stat,
        )
    };
    if result != 0 || stat.stx_mask & libc::STATX_MNT_ID == 0 {
        return Err(CommandError::LocalDeletionUnsafe);
    }
    Ok(stat.stx_mnt_id)
}

#[cfg(all(unix, not(target_os = "linux")))]
fn mount_id(file: &File) -> Result<u64, CommandError> {
    use std::os::unix::fs::MetadataExt;
    Ok(file.metadata().map_err(failed)?.dev())
}

#[cfg(unix)]
fn optional_held(path: &Path, directory: bool) -> Result<Option<Held>, CommandError> {
    match fs::symlink_metadata(path) {
        Ok(_) => Held::open(path, directory).map(Some),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err(CommandError::LocalDeletionUnsafe),
    }
}

#[cfg(unix)]
fn inspect_root(root: &Held) -> Result<(), CommandError> {
    root.check()?;
    for entry in fs::read_dir(&root.path).map_err(failed)? {
        let entry = entry.map_err(failed)?;
        let name = entry.file_name();
        let name = name.to_str().ok_or(CommandError::LocalDeletionUnsafe)?;
        if MANAGED_DIRS.contains(&name) {
            let directory = Held::open(&entry.path(), true)?;
            inspect_tree(&directory, root, name == "profiles", 0)?;
        } else if MANAGED_FILES.contains(&name) || name == MARKER || name == ".instance.lock" {
            let file = Held::open(&entry.path(), false)?;
            same_filesystem(&file, root)?;
        } else {
            return Err(CommandError::LocalDeletionUnsafe);
        }
    }
    root.check()
}

#[cfg(unix)]
fn same_filesystem(path: &Held, root: &Held) -> Result<(), CommandError> {
    if path.identity[0] != root.identity[0] || path.mount != root.mount {
        return Err(CommandError::LocalDeletionUnsafe);
    }
    Ok(())
}

#[cfg(unix)]
fn inspect_tree(
    directory: &Held,
    root: &Held,
    profile_children: bool,
    depth: usize,
) -> Result<(), CommandError> {
    if depth > 128 {
        return Err(CommandError::LocalDeletionUnsafe);
    }
    same_filesystem(directory, root)?;
    directory.check()?;
    for entry in fs::read_dir(&directory.path).map_err(failed)? {
        let entry = entry.map_err(failed)?;
        let metadata = fs::symlink_metadata(entry.path()).map_err(failed)?;
        if profile_children {
            let name = entry.file_name();
            let name = name.to_str().ok_or(CommandError::LocalDeletionUnsafe)?;
            if uuid::Uuid::parse_str(name)
                .ok()
                .is_none_or(|id| id.to_string() != name)
                || !metadata.is_dir()
            {
                return Err(CommandError::LocalDeletionUnsafe);
            }
        }
        // Provider-created files may use normal modes beneath the private managed home.
        let child = Held::open_inner(&entry.path(), metadata.is_dir(), profile_children)?;
        same_filesystem(&child, root)?;
        if child.directory {
            inspect_tree(&child, root, false, depth + 1)?;
        }
    }
    directory.check()
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::keyvault::DatabaseKey;
    use std::{
        cell::{Cell, RefCell},
        os::unix::fs::{MetadataExt, PermissionsExt, symlink},
    };

    struct Vault {
        key: RefCell<Option<DatabaseKey>>,
        available: Cell<bool>,
        remove_available: Cell<bool>,
        removals: Cell<usize>,
    }
    impl Vault {
        fn new() -> Self {
            Self {
                key: RefCell::new(None),
                available: Cell::new(true),
                remove_available: Cell::new(true),
                removals: Cell::new(0),
            }
        }
    }
    impl KeyVault for Vault {
        fn load(&self) -> Result<Option<DatabaseKey>, VaultError> {
            if !self.available.get() {
                return Err(VaultError::Unavailable);
            }
            Ok(self.key.borrow().as_ref().map(|key| Zeroizing::new(**key)))
        }
        fn protect(&self, key: &[u8; 32]) -> Result<(), VaultError> {
            if !self.available.get() {
                return Err(VaultError::Unavailable);
            }
            *self.key.borrow_mut() = Some(Zeroizing::new(*key));
            Ok(())
        }
        fn remove(&self, expected: &[u8; 32]) -> Result<(), VaultError> {
            if !self.available.get() || !self.remove_available.get() {
                return Err(VaultError::Unavailable);
            }
            if self
                .key
                .borrow()
                .as_ref()
                .is_some_and(|key| **key != *expected)
            {
                return Err(VaultError::InvalidKey);
            }
            self.key.borrow_mut().take();
            self.removals.set(self.removals.get() + 1);
            Ok(())
        }
    }
    fn private_file(path: &Path, bytes: &[u8]) {
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)
            .unwrap();
        file.write_all(bytes).unwrap();
    }
    fn fallback_intent(root: &Path) -> Intent {
        Intent {
            version: 1,
            mode: Mode::Passphrase,
            os_key_fingerprint: None,
            database_identity: identity(&fs::metadata(root.join(DATABASE_FILE)).unwrap()),
            wrapped_identity: Some(identity(
                &fs::metadata(root.join(WRAPPED_KEY_FILE)).unwrap(),
            )),
        }
    }
    fn fallback_setup(workspace: &mut Workspace, vault: &Vault) {
        workspace
            .setup(
                Ok(vault),
                Some(Zeroizing::new("synthetic deletion passphrase".into())),
            )
            .unwrap();
    }

    #[test]
    fn confirmed_os_deletion_preserves_lease_and_external_files_then_allows_fresh_setup() {
        let temp = tempfile::tempdir().unwrap();
        let directory = temp.path().join("workspace");
        let outside = temp.path().join("export.csv");
        fs::write(&outside, b"retained external export").unwrap();
        let vault = Vault::new();
        let mut workspace = Workspace::new(directory.clone()).unwrap();
        workspace.setup(Ok(&vault), None).unwrap();
        let profile = workspace
            .create_profile("Synthetic account")
            .unwrap()
            .profiles
            .remove(0);
        let home = directory.join("profiles").join(profile.id);
        let nested = home.join("provider-files");
        fs::create_dir(&nested).unwrap();
        fs::set_permissions(&nested, fs::Permissions::from_mode(0o755)).unwrap();
        fs::write(nested.join("provider.json"), b"synthetic provider data").unwrap();
        fs::set_permissions(
            nested.join("provider.json"),
            fs::Permissions::from_mode(0o644),
        )
        .unwrap();
        let lease = fs::metadata(directory.join(".instance.lock"))
            .unwrap()
            .ino();
        assert!(matches!(
            workspace.delete_all_local_data(Ok(&vault), false),
            Err(CommandError::InvalidArguments)
        ));
        assert!(directory.join(DATABASE_FILE).exists());
        let result = workspace.delete_all_local_data(Ok(&vault), true).unwrap();
        assert_eq!(result.storage, StorageState::SetupRequired);
        assert_eq!(vault.removals.get(), 1);
        assert!(vault.key.borrow().is_none());
        assert_eq!(fs::read_dir(&directory).unwrap().count(), 1);
        assert_eq!(
            fs::metadata(directory.join(".instance.lock"))
                .unwrap()
                .ino(),
            lease
        );
        assert_eq!(fs::read(&outside).unwrap(), b"retained external export");
        assert!(!pending(&directory));
        workspace.setup(Ok(&vault), None).unwrap();
        assert!(workspace.list_profiles().unwrap().profiles.is_empty());
    }

    #[test]
    fn fallback_deletion_never_touches_unavailable_os_store() {
        let temp = tempfile::tempdir().unwrap();
        let directory = temp.path().join("workspace");
        let vault = Vault::new();
        vault.available.set(false);
        let mut workspace = Workspace::new(directory.clone()).unwrap();
        fallback_setup(&mut workspace, &vault);
        workspace.create_profile("Synthetic fallback").unwrap();
        workspace
            .delete_all_local_data(Err::<&Vault, _>(VaultError::Unavailable), true)
            .unwrap();
        assert_eq!(vault.removals.get(), 0);
        assert!(!directory.join(WRAPPED_KEY_FILE).exists());
        assert_eq!(fs::read_dir(directory).unwrap().count(), 1);
    }

    #[test]
    fn failed_os_removal_keeps_bound_intent_and_requires_explicit_restart_resume() {
        let temp = tempfile::tempdir().unwrap();
        let directory = temp.path().join("workspace");
        let vault = Vault::new();
        {
            let mut workspace = Workspace::new(directory.clone()).unwrap();
            workspace.setup(Ok(&vault), None).unwrap();
            workspace
                .create_profile("Synthetic partial cleanup")
                .unwrap();
            vault.remove_available.set(false);
            assert!(matches!(
                workspace.delete_all_local_data(Ok(&vault), true),
                Err(CommandError::KeyStoreUnavailable)
            ));
            assert!(!directory.join("profiles").exists());
            assert!(directory.join(DATABASE_FILE).exists());
            assert!(vault.key.borrow().is_some());
            assert!(pending(&directory));
            assert!(workspace.storage.is_none());
            assert_eq!(
                workspace.bootstrap(Ok(&vault)).storage_issue,
                Some(StorageIssue::LocalDeletionIncomplete)
            );
            assert!(workspace.setup(Ok(&vault), None).is_err());
            assert!(
                workspace
                    .unlock(Zeroizing::new("synthetic unused password".into()))
                    .is_err()
            );
        }
        let mut workspace = Workspace::new(directory.clone()).unwrap();
        assert_eq!(
            workspace.bootstrap(Ok(&vault)).storage_issue,
            Some(StorageIssue::LocalDeletionIncomplete)
        );
        assert!(directory.join(DATABASE_FILE).exists());
        vault.remove_available.set(true);
        assert!(workspace.delete_all_local_data(Ok(&vault), false).is_err());
        assert!(directory.join(DATABASE_FILE).exists());
        workspace.delete_all_local_data(Ok(&vault), true).unwrap();
        assert!(vault.key.borrow().is_none());
        assert_eq!(vault.removals.get(), 1);
        assert_eq!(fs::read_dir(directory).unwrap().count(), 1);
    }

    #[test]
    fn unrelated_os_key_is_preserved_before_intent_and_during_partial_resume() {
        let temp = tempfile::tempdir().unwrap();
        let directory = temp.path().join("workspace");
        let vault = Vault::new();
        let mut workspace = Workspace::new(directory.clone()).unwrap();
        workspace.setup(Ok(&vault), None).unwrap();
        workspace.create_profile("Synthetic key mismatch").unwrap();
        let expected = **vault.key.borrow().as_ref().unwrap();
        let unrelated = [expected[0].wrapping_add(1); 32];
        *vault.key.borrow_mut() = Some(Zeroizing::new(unrelated));
        assert!(matches!(
            workspace.delete_all_local_data(Ok(&vault), true),
            Err(CommandError::LocalDeletionUnsafe)
        ));
        assert!(!pending(&directory));
        assert!(directory.join("profiles").exists());
        assert!(workspace.storage.is_some());
        assert_eq!(**vault.key.borrow().as_ref().unwrap(), unrelated);
        assert_eq!(vault.removals.get(), 0);
        *vault.key.borrow_mut() = Some(Zeroizing::new(expected));
        vault.remove_available.set(false);
        assert!(workspace.delete_all_local_data(Ok(&vault), true).is_err());
        assert!(pending(&directory));
        vault.remove_available.set(true);
        *vault.key.borrow_mut() = Some(Zeroizing::new(unrelated));
        assert!(matches!(
            workspace.delete_all_local_data(Ok(&vault), true),
            Err(CommandError::LocalDeletionUnsafe)
        ));
        assert!(directory.join(DATABASE_FILE).exists());
        assert_eq!(**vault.key.borrow().as_ref().unwrap(), unrelated);
        assert_eq!(vault.removals.get(), 0);
        *vault.key.borrow_mut() = Some(Zeroizing::new(expected));
        workspace.delete_all_local_data(Ok(&vault), true).unwrap();
        assert_eq!(vault.removals.get(), 1);
    }

    #[test]
    fn deletion_rechecks_os_store_instead_of_requiring_the_previous_fallback_password() {
        let temp = tempfile::tempdir().unwrap();
        let directory = temp.path().join("workspace");
        let vault = Vault::new();
        vault.available.set(false);
        let mut workspace = Workspace::new(directory).unwrap();
        fallback_setup(&mut workspace, &vault);
        vault.available.set(true);
        let fresh = workspace.delete_all_local_data(Ok(&vault), true).unwrap();
        assert_eq!(fresh.storage, StorageState::SetupRequired);
        assert_eq!(fresh.key_protection, KeyProtection::OsStore);
        assert_eq!(fresh.storage_issue, None);
        assert_eq!(vault.removals.get(), 0);
        assert_eq!(
            workspace.setup(Ok(&vault), None).unwrap().storage,
            StorageState::Ready
        );
    }

    #[test]
    fn pending_restart_blocks_unlock_and_resumes_only_after_confirmation() {
        let temp = tempfile::tempdir().unwrap();
        let directory = temp.path().join("workspace");
        let vault = Vault::new();
        vault.available.set(false);
        {
            let mut workspace = Workspace::new(directory.clone()).unwrap();
            fallback_setup(&mut workspace, &vault);
            workspace.create_profile("Synthetic retained").unwrap();
        }
        let intent = fallback_intent(&directory);
        write_intent(&Held::open(&directory, true).unwrap(), &intent).unwrap();
        let mut workspace = Workspace::new(directory.clone()).unwrap();
        assert_eq!(
            workspace.bootstrap(Ok(&vault)).storage_issue,
            Some(StorageIssue::LocalDeletionIncomplete)
        );
        assert!(
            workspace
                .unlock(Zeroizing::new("synthetic deletion passphrase".into()))
                .is_err()
        );
        assert!(workspace.delete_all_local_data(Ok(&vault), false).is_err());
        assert!(directory.join(DATABASE_FILE).exists());
        workspace.delete_all_local_data(Ok(&vault), true).unwrap();
        assert_eq!(fs::read_dir(directory).unwrap().count(), 1);
    }

    #[test]
    fn changed_database_after_intent_is_preserved_and_deletion_stays_blocked() {
        let temp = tempfile::tempdir().unwrap();
        let directory = temp.path().join("workspace");
        let vault = Vault::new();
        vault.available.set(false);
        {
            let mut workspace = Workspace::new(directory.clone()).unwrap();
            fallback_setup(&mut workspace, &vault);
        }
        let intent = fallback_intent(&directory);
        write_intent(&Held::open(&directory, true).unwrap(), &intent).unwrap();
        // Hold the old inode while replacing the path, preventing inode reuse in this regression.
        let original = File::open(directory.join(DATABASE_FILE)).unwrap();
        fs::remove_file(directory.join(DATABASE_FILE)).unwrap();
        private_file(&directory.join(DATABASE_FILE), b"unrelated replacement");
        let mut workspace = Workspace::new(directory.clone()).unwrap();
        assert!(matches!(
            workspace.delete_all_local_data(Ok(&vault), true),
            Err(CommandError::LocalDeletionUnsafe)
        ));
        assert_eq!(
            fs::read(directory.join(DATABASE_FILE)).unwrap(),
            b"unrelated replacement"
        );
        assert!(pending(&directory));
        drop(original);
    }

    #[test]
    fn unknown_entries_links_and_replaced_lease_reject_before_intent() {
        let temp = tempfile::tempdir().unwrap();
        let directory = temp.path().join("workspace");
        let vault = Vault::new();
        let mut workspace = Workspace::new(directory.clone()).unwrap();
        workspace.setup(Ok(&vault), None).unwrap();
        private_file(&directory.join("unknown"), b"do not delete");
        assert!(workspace.delete_all_local_data(Ok(&vault), true).is_err());
        assert!(!pending(&directory));
        fs::remove_file(directory.join("unknown")).unwrap();
        let profile = workspace
            .create_profile("Synthetic links")
            .unwrap()
            .profiles
            .remove(0);
        let home = directory.join("profiles").join(profile.id);
        let external = temp.path().join("outside");
        private_file(&external, b"external retained");
        symlink(&external, home.join("link")).unwrap();
        assert!(workspace.delete_all_local_data(Ok(&vault), true).is_err());
        fs::remove_file(home.join("link")).unwrap();
        fs::hard_link(&external, home.join("hardlink")).unwrap();
        assert!(workspace.delete_all_local_data(Ok(&vault), true).is_err());
        fs::remove_file(home.join("hardlink")).unwrap();
        fs::remove_file(directory.join(".instance.lock")).unwrap();
        private_file(&directory.join(".instance.lock"), b"");
        assert!(workspace.delete_all_local_data(Ok(&vault), true).is_err());
        assert!(!pending(&directory));
        assert_eq!(fs::read(external).unwrap(), b"external retained");
        assert_eq!(vault.removals.get(), 0);
    }

    #[test]
    fn malformed_or_symlink_marker_is_pending_and_never_authorizes_deletion() {
        let temp = tempfile::tempdir().unwrap();
        let directory = temp.path().join("workspace");
        let vault = Vault::new();
        let mut workspace = Workspace::new(directory.clone()).unwrap();
        workspace.setup(Ok(&vault), None).unwrap();
        private_file(&directory.join(MARKER), b"invalid");
        assert!(pending(&directory));
        assert!(workspace.delete_all_local_data(Ok(&vault), true).is_err());
        assert!(directory.join(DATABASE_FILE).exists());
        fs::remove_file(directory.join(MARKER)).unwrap();
        symlink(temp.path().join("outside"), directory.join(MARKER)).unwrap();
        assert!(pending(&directory));
        assert!(workspace.delete_all_local_data(Ok(&vault), true).is_err());
        assert_eq!(vault.removals.get(), 0);
    }
}
