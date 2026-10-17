use crate::keyvault::{KeyVault, VaultError};
use openusage_storage::{Storage, StorageError, Theme, keywrap};
use serde::Serialize;
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};
use zeroize::Zeroizing;

const DATABASE_FILE: &str = "openusage.db";
const WRAPPED_KEY_FILE: &str = "database-key.v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StorageState {
    SetupRequired,
    Locked,
    Ready,
    RecoveryRequired,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyProtection {
    OsStore,
    Passphrase,
    PassphraseRequired,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StorageIssue {
    KeyStoreUnavailable,
    MissingKey,
    MissingDatabase,
    InvalidKey,
    StorageUnavailable,
    WorkspaceInUse,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandError {
    InvalidArguments,
    StorageUnavailable,
    WorkspaceInUse,
    AlreadyInitialized,
    InvalidPassphrase,
    PassphraseTooShort,
    PassphraseTooLong,
    OsStoreRequired,
    WorkspaceLocked,
    BackupUnavailable,
    InsufficientSpace,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Bootstrap {
    pub storage: StorageState,
    pub theme: &'static str,
    pub schema_version: Option<u32>,
    pub active_profiles: Option<u32>,
    pub monitoring: &'static str,
    pub provider: &'static str,
    pub key_protection: KeyProtection,
    pub storage_issue: Option<StorageIssue>,
}

impl Bootstrap {
    fn unavailable(
        storage: StorageState,
        protection: KeyProtection,
        issue: Option<StorageIssue>,
    ) -> Self {
        Self {
            storage,
            theme: "system",
            schema_version: None,
            active_profiles: None,
            monitoring: "stopped",
            provider: "not_configured",
            key_protection: protection,
            storage_issue: issue,
        }
    }

    pub fn blocked(error: CommandError) -> Self {
        let issue = if error == CommandError::WorkspaceInUse {
            StorageIssue::WorkspaceInUse
        } else {
            StorageIssue::StorageUnavailable
        };
        Self::unavailable(
            StorageState::RecoveryRequired,
            KeyProtection::Unavailable,
            Some(issue),
        )
    }
}

pub struct Workspace {
    directory: PathBuf,
    _lock: File,
    storage: Option<Storage>,
    protection: KeyProtection,
}

impl Workspace {
    pub fn new(directory: PathBuf) -> Result<Self, CommandError> {
        prepare_directory(&directory)?;
        let lock_path = directory.join(".instance.lock");
        let lock = secure_open(&lock_path, true, false)?;
        lock.try_lock().map_err(|error| match error {
            std::fs::TryLockError::WouldBlock => CommandError::WorkspaceInUse,
            _ => CommandError::StorageUnavailable,
        })?;
        Ok(Self {
            directory,
            _lock: lock,
            storage: None,
            protection: KeyProtection::Unavailable,
        })
    }

    pub fn bootstrap(&mut self, vault: Result<&impl KeyVault, VaultError>) -> Bootstrap {
        if self.storage.is_some() {
            return self.ready();
        }
        let database = self.directory.join(DATABASE_FILE);
        let database_exists = match regular_file_exists(&database) {
            Ok(exists) => exists,
            Err(_) => return self.recovery(StorageIssue::StorageUnavailable),
        };
        let wrapped_exists = match regular_file_exists(&self.directory.join(WRAPPED_KEY_FILE)) {
            Ok(exists) => exists,
            Err(_) => return self.recovery(StorageIssue::StorageUnavailable),
        };
        if wrapped_exists {
            self.protection = KeyProtection::Passphrase;
            if fs::metadata(self.directory.join(WRAPPED_KEY_FILE))
                .map(|metadata| metadata.len())
                .ok()
                != Some(keywrap::WRAPPED_KEY_SIZE as u64)
            {
                return self.recovery(StorageIssue::InvalidKey);
            }
            return if database_exists {
                Bootstrap::unavailable(StorageState::Locked, self.protection, None)
            } else {
                self.recovery(StorageIssue::MissingDatabase)
            };
        }
        self.protection = KeyProtection::OsStore;
        let key = match vault.and_then(|vault| vault.load()) {
            Ok(Some(key)) if database_exists => key,
            Ok(Some(_)) => return self.recovery(StorageIssue::MissingDatabase),
            Ok(None) if database_exists => return self.recovery(StorageIssue::MissingKey),
            Ok(None) => {
                return Bootstrap::unavailable(StorageState::SetupRequired, self.protection, None);
            }
            Err(VaultError::InvalidKey) => return self.recovery(StorageIssue::InvalidKey),
            Err(VaultError::Unavailable) => {
                self.protection = if database_exists {
                    KeyProtection::OsStore
                } else {
                    KeyProtection::PassphraseRequired
                };
                return Bootstrap::unavailable(
                    if database_exists {
                        StorageState::Locked
                    } else {
                        StorageState::SetupRequired
                    },
                    self.protection,
                    Some(StorageIssue::KeyStoreUnavailable),
                );
            }
        };
        match Storage::open(&database, &key) {
            Ok(storage) => {
                self.storage = Some(storage);
                self.ready()
            }
            Err(StorageError::UnlockFailed) => self.recovery(StorageIssue::InvalidKey),
            Err(_) => self.recovery(StorageIssue::StorageUnavailable),
        }
    }

    pub fn setup(
        &mut self,
        vault: Result<&impl KeyVault, VaultError>,
        passphrase: Option<Zeroizing<String>>,
    ) -> Result<Bootstrap, CommandError> {
        if let Some(passphrase) = &passphrase {
            validate_passphrase(passphrase)?;
        }
        if self.storage.is_some()
            || regular_file_exists(&self.directory.join(DATABASE_FILE))?
            || regular_file_exists(&self.directory.join(WRAPPED_KEY_FILE))?
        {
            return Err(CommandError::AlreadyInitialized);
        }
        let os_store_unavailable = match vault
            .as_ref()
            .map_err(|error| *error)
            .and_then(|vault| vault.load())
        {
            Ok(Some(_)) | Err(VaultError::InvalidKey) => {
                return Err(CommandError::AlreadyInitialized);
            }
            Err(VaultError::Unavailable) => true,
            Ok(None) => false,
        };
        let key = keywrap::generate_database_key().map_err(|_| CommandError::StorageUnavailable)?;
        if let Some(passphrase) = passphrase {
            if !os_store_unavailable {
                return Err(CommandError::OsStoreRequired);
            }
            let wrapped = keywrap::wrap_key(&key, passphrase.as_bytes())
                .map_err(|_| CommandError::StorageUnavailable)?;
            let mut file = secure_open(&self.directory.join(WRAPPED_KEY_FILE), true, true)?;
            file.write_all(&wrapped)
                .and_then(|()| file.sync_all())
                .map_err(|_| CommandError::StorageUnavailable)?;
            #[cfg(unix)]
            File::open(&self.directory)
                .and_then(|directory| directory.sync_all())
                .map_err(|_| CommandError::StorageUnavailable)?;
            self.protection = KeyProtection::Passphrase;
        } else {
            let vault = match vault {
                Ok(vault) => vault,
                Err(_) => return Ok(self.passphrase_required()),
            };
            if vault.protect(&key).is_err() {
                return Ok(self.passphrase_required());
            }
            let checked = vault
                .load()
                .map_err(|_| CommandError::StorageUnavailable)?
                .ok_or(CommandError::StorageUnavailable)?;
            if *checked != *key {
                return Err(CommandError::StorageUnavailable);
            }
            self.protection = KeyProtection::OsStore;
        }
        let storage = Storage::create(&self.directory.join(DATABASE_FILE), &key)
            .map_err(|_| CommandError::StorageUnavailable)?;
        self.storage = Some(storage);
        Ok(self.ready())
    }

    pub fn unlock(&mut self, passphrase: Zeroizing<String>) -> Result<Bootstrap, CommandError> {
        validate_unlock_passphrase(&passphrase)?;
        if self.storage.is_some() {
            return Ok(self.ready());
        }
        if !regular_file_exists(&self.directory.join(DATABASE_FILE))?
            || !regular_file_exists(&self.directory.join(WRAPPED_KEY_FILE))?
        {
            return Err(CommandError::WorkspaceLocked);
        }
        let mut file = secure_open(&self.directory.join(WRAPPED_KEY_FILE), false, false)?;
        if file
            .metadata()
            .map_err(|_| CommandError::StorageUnavailable)?
            .len()
            != keywrap::WRAPPED_KEY_SIZE as u64
        {
            return Ok(self.recovery(StorageIssue::InvalidKey));
        }
        let mut wrapped = [0_u8; keywrap::WRAPPED_KEY_SIZE];
        file.read_exact(&mut wrapped)
            .map_err(|_| CommandError::StorageUnavailable)?;
        let key = match keywrap::unwrap_key(&wrapped, passphrase.as_bytes()) {
            Ok(key) => key,
            Err(
                keywrap::KeywrapError::InvalidEnvelope | keywrap::KeywrapError::UnsupportedFormat,
            ) => return Ok(self.recovery(StorageIssue::InvalidKey)),
            Err(_) => return Err(CommandError::InvalidPassphrase),
        };
        self.protection = KeyProtection::Passphrase;
        self.storage = Some(
            match Storage::open(&self.directory.join(DATABASE_FILE), &key) {
                Ok(storage) => storage,
                Err(StorageError::UnlockFailed) => {
                    return Ok(self.recovery(StorageIssue::InvalidKey));
                }
                Err(_) => return Ok(self.recovery(StorageIssue::StorageUnavailable)),
            },
        );
        Ok(self.ready())
    }

    pub fn set_theme(&mut self, theme: Theme) -> Result<Bootstrap, CommandError> {
        self.storage
            .as_mut()
            .ok_or(CommandError::WorkspaceLocked)?
            .set_theme(theme)
            .map_err(|_| CommandError::StorageUnavailable)?;
        Ok(self.ready())
    }

    pub fn backup_snapshot(&mut self) -> Result<openusage_storage::BackupSnapshot, CommandError> {
        self.storage
            .as_mut()
            .ok_or(CommandError::WorkspaceLocked)?
            .backup_snapshot()
            .map_err(|_| CommandError::BackupUnavailable)
    }

    fn ready(&self) -> Bootstrap {
        match self
            .storage
            .as_ref()
            .and_then(|storage| storage.metadata().ok())
        {
            Some(metadata) if metadata.schema_version > 0 && metadata.profile_count <= 10 => {
                Bootstrap {
                    storage: StorageState::Ready,
                    theme: metadata.theme.as_str(),
                    schema_version: Some(metadata.schema_version),
                    active_profiles: Some(metadata.profile_count),
                    monitoring: "stopped",
                    provider: "not_configured",
                    key_protection: self.protection,
                    storage_issue: None,
                }
            }
            _ => self.recovery(StorageIssue::StorageUnavailable),
        }
    }

    fn recovery(&self, issue: StorageIssue) -> Bootstrap {
        Bootstrap::unavailable(StorageState::RecoveryRequired, self.protection, Some(issue))
    }

    fn passphrase_required(&self) -> Bootstrap {
        Bootstrap::unavailable(
            StorageState::SetupRequired,
            KeyProtection::PassphraseRequired,
            Some(StorageIssue::KeyStoreUnavailable),
        )
    }
}

pub fn validate_passphrase(passphrase: &str) -> Result<(), CommandError> {
    if passphrase.len() > 1024 {
        return Err(CommandError::PassphraseTooLong);
    }
    if passphrase.chars().count() < 12 {
        return Err(CommandError::PassphraseTooShort);
    }
    Ok(())
}

pub fn validate_unlock_passphrase(passphrase: &str) -> Result<(), CommandError> {
    if passphrase.is_empty() {
        return Err(CommandError::InvalidPassphrase);
    }
    if passphrase.len() > 1024 {
        return Err(CommandError::PassphraseTooLong);
    }
    Ok(())
}

fn regular_file_exists(path: &Path) -> Result<bool, CommandError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => {
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                if metadata.nlink() != 1 {
                    return Err(CommandError::StorageUnavailable);
                }
            }
            Ok(true)
        }
        Ok(_) => Err(CommandError::StorageUnavailable),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(_) => Err(CommandError::StorageUnavailable),
    }
}

fn secure_open(path: &Path, create: bool, exclusive: bool) -> Result<File, CommandError> {
    if !exclusive {
        regular_file_exists(path)?;
    }
    let mut options = OpenOptions::new();
    options
        .read(true)
        .write(true)
        .create(create)
        .create_new(exclusive)
        .truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    }
    options
        .open(path)
        .map_err(|_| CommandError::StorageUnavailable)
}

pub(crate) fn prepare_directory(path: &Path) -> Result<(), CommandError> {
    if !path.is_absolute()
        || path
            .components()
            .any(|part| matches!(part, std::path::Component::ParentDir))
    {
        return Err(CommandError::StorageUnavailable);
    }
    for ancestor in path.ancestors().collect::<Vec<_>>().into_iter().rev() {
        if !ancestor.exists() {
            let mut builder = fs::DirBuilder::new();
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                builder.mode(0o700);
            }
            builder
                .create(ancestor)
                .map_err(|_| CommandError::StorageUnavailable)?;
        }
        let metadata =
            fs::symlink_metadata(ancestor).map_err(|_| CommandError::StorageUnavailable)?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(CommandError::StorageUnavailable);
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        let metadata = fs::metadata(path).map_err(|_| CommandError::StorageUnavailable)?;
        // geteuid reads the process identity and has no pointer arguments.
        if metadata.permissions().mode() & 0o077 != 0
            || metadata.uid() != unsafe { libc::geteuid() }
        {
            return Err(CommandError::StorageUnavailable);
        }
    }
    require_local_filesystem(path)?;
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn require_local_filesystem(path: &Path) -> Result<(), CommandError> {
    use std::os::unix::ffi::OsStrExt;
    let path = std::ffi::CString::new(path.as_os_str().as_bytes())
        .map_err(|_| CommandError::StorageUnavailable)?;
    let mut result = std::mem::MaybeUninit::<libc::statfs>::uninit();
    // statfs initializes the supplied structure on success. The C path stays alive during the call.
    if unsafe { libc::statfs(path.as_ptr(), result.as_mut_ptr()) } != 0 {
        return Err(CommandError::StorageUnavailable);
    }
    // A successful statfs call initializes every structure field.
    let result = unsafe { result.assume_init() };
    #[cfg(target_os = "linux")]
    let local = matches!(
        result.f_type as u64,
        0xef53
            | 0x0102_1994
            | 0x9123_683e
            | 0x5846_5342
            | 0x2fc1_2fc1
            | 0x794c_7630
            | 0x8584_58f6
            | 0xf2f5_2010
    );
    #[cfg(target_os = "macos")]
    let local = result.f_flags & libc::MNT_LOCAL as u32 != 0;
    if !local {
        return Err(CommandError::StorageUnavailable);
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn require_local_filesystem(path: &Path) -> Result<(), CommandError> {
    use std::path::{Component, Prefix};
    use windows_sys::Win32::Storage::FileSystem::{DRIVE_FIXED, GetDriveTypeW};
    let drive = match path.components().next() {
        Some(Component::Prefix(prefix)) => match prefix.kind() {
            Prefix::Disk(drive) | Prefix::VerbatimDisk(drive) => drive,
            _ => return Err(CommandError::StorageUnavailable),
        },
        _ => return Err(CommandError::StorageUnavailable),
    };
    let root = [u16::from(drive), u16::from(b':'), u16::from(b'\\'), 0];
    // The root is a live, null-terminated Windows drive path.
    if unsafe { GetDriveTypeW(root.as_ptr()) } != DRIVE_FIXED {
        return Err(CommandError::StorageUnavailable);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keyvault::DatabaseKey;
    use std::cell::{Cell, RefCell};

    struct FakeVault {
        key: RefCell<Option<DatabaseKey>>,
        available: Cell<bool>,
        database: PathBuf,
    }

    impl FakeVault {
        fn new(directory: &Path, available: bool) -> Self {
            Self {
                key: RefCell::new(None),
                available: Cell::new(available),
                database: directory.join(DATABASE_FILE),
            }
        }
    }

    impl KeyVault for FakeVault {
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
            assert!(
                !self.database.exists(),
                "Key protection must precede database creation"
            );
            *self.key.borrow_mut() = Some(Zeroizing::new(*key));
            Ok(())
        }
    }

    #[test]
    fn os_key_precedes_encrypted_database_and_saved_theme_survives_restart() {
        let temporary = tempfile::tempdir().unwrap();
        let directory = temporary.path().join("workspace");
        let vault = FakeVault::new(&directory, true);
        let mut workspace = Workspace::new(directory.clone()).unwrap();
        assert_eq!(
            workspace.bootstrap(Ok(&vault)).storage,
            StorageState::SetupRequired
        );
        assert!(!vault.database.exists());
        assert_eq!(
            workspace.setup(Ok(&vault), None).unwrap().storage,
            StorageState::Ready
        );
        assert!(vault.key.borrow().is_some());
        assert!(
            !fs::read(&vault.database)
                .unwrap()
                .starts_with(b"SQLite format 3\0")
        );
        assert_eq!(workspace.set_theme(Theme::Dark).unwrap().theme, "dark");
        drop(workspace);
        let mut restarted = Workspace::new(directory).unwrap();
        let ready = restarted.bootstrap(Ok(&vault));
        assert_eq!(ready.storage, StorageState::Ready);
        assert_eq!(ready.theme, "dark");
        assert_eq!(ready.active_profiles, Some(0));
        assert_eq!(ready.schema_version, Some(1));
        assert_eq!(ready.monitoring, "stopped");
    }

    #[test]
    fn missing_key_or_database_requires_recovery_without_reset_or_replacement() {
        let temporary = tempfile::tempdir().unwrap();
        let directory = temporary.path().join("workspace");
        let vault = FakeVault::new(&directory, true);
        let mut workspace = Workspace::new(directory.clone()).unwrap();
        workspace.setup(Ok(&vault), None).unwrap();
        drop(workspace);
        let saved_key = vault.key.borrow_mut().take().unwrap();
        let before = fs::read(&vault.database).unwrap();
        let mut restarted = Workspace::new(directory.clone()).unwrap();
        let state = restarted.bootstrap(Ok(&vault));
        assert_eq!(state.storage, StorageState::RecoveryRequired);
        assert_eq!(state.storage_issue, Some(StorageIssue::MissingKey));
        assert!(matches!(
            restarted.setup(Ok(&vault), None),
            Err(CommandError::AlreadyInitialized)
        ));
        assert_eq!(fs::read(&vault.database).unwrap(), before);
        drop(restarted);

        let missing_directory = temporary.path().join("missing-database");
        let protected = FakeVault::new(&missing_directory, true);
        *protected.key.borrow_mut() = Some(saved_key);
        let mut missing = Workspace::new(missing_directory).unwrap();
        assert_eq!(
            missing.bootstrap(Ok(&protected)).storage_issue,
            Some(StorageIssue::MissingDatabase)
        );
        assert!(matches!(
            missing.setup(Ok(&protected), None),
            Err(CommandError::AlreadyInitialized)
        ));
        assert!(!protected.database.exists());
    }

    #[test]
    fn passphrase_fallback_preserves_encryption_and_requires_unlock_after_restart() {
        let temporary = tempfile::tempdir().unwrap();
        let directory = temporary.path().join("workspace");
        let vault = FakeVault::new(&directory, false);
        let mut workspace = Workspace::new(directory.clone()).unwrap();
        let unavailable = workspace.bootstrap(Ok(&vault));
        assert_eq!(unavailable.storage, StorageState::SetupRequired);
        assert_eq!(
            unavailable.key_protection,
            KeyProtection::PassphraseRequired
        );
        assert_eq!(
            workspace.setup(Ok(&vault), None).unwrap().key_protection,
            KeyProtection::PassphraseRequired
        );
        assert!(!vault.database.exists());
        let passphrase = "synthetic passphrase for private tests";
        assert_eq!(
            workspace
                .setup(Ok(&vault), Some(Zeroizing::new(passphrase.into())))
                .unwrap()
                .storage,
            StorageState::Ready
        );
        workspace.set_theme(Theme::Light).unwrap();
        drop(workspace);
        let wrapped_path = directory.join(WRAPPED_KEY_FILE);
        let wrapped = fs::read(&wrapped_path).unwrap();
        let database = fs::read(&vault.database).unwrap();
        assert_eq!(wrapped.len(), keywrap::WRAPPED_KEY_SIZE);
        assert!(!database.starts_with(b"SQLite format 3\0"));
        assert!(
            !wrapped
                .windows(passphrase.len())
                .any(|part| part == passphrase.as_bytes())
        );
        let mut restarted = Workspace::new(directory).unwrap();
        let locked = restarted.bootstrap(Ok(&vault));
        assert_eq!(locked.storage, StorageState::Locked);
        assert_eq!(locked.key_protection, KeyProtection::Passphrase);
        assert!(matches!(
            restarted.unlock(Zeroizing::new("x".into())),
            Err(CommandError::InvalidPassphrase)
        ));
        assert_eq!(fs::read(&wrapped_path).unwrap(), wrapped);
        assert_eq!(fs::read(&vault.database).unwrap(), database);
        let ready = restarted.unlock(Zeroizing::new(passphrase.into())).unwrap();
        assert_eq!(ready.storage, StorageState::Ready);
        assert_eq!(ready.theme, "light");
    }

    #[test]
    #[cfg(unix)]
    fn private_workspace_preserves_shared_webview_directory_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let temporary = tempfile::tempdir().unwrap();
        let shared = temporary.path().join("app.openusage.desktop");
        fs::create_dir(&shared).unwrap();
        fs::set_permissions(&shared, fs::Permissions::from_mode(0o755)).unwrap();
        let directory = shared.join("workspace");
        let workspace = Workspace::new(directory.clone()).unwrap();
        assert_eq!(
            fs::metadata(&shared).unwrap().permissions().mode() & 0o777,
            0o755
        );
        assert_eq!(
            fs::metadata(&directory).unwrap().permissions().mode() & 0o777,
            0o700
        );
        drop(workspace);
    }

    #[test]
    fn only_one_private_workspace_opens_and_existing_links_are_rejected() {
        let temporary = tempfile::tempdir().unwrap();
        let directory = temporary.path().join("workspace");
        let first = Workspace::new(directory.clone()).unwrap();
        assert!(matches!(
            Workspace::new(directory.clone()),
            Err(CommandError::WorkspaceInUse)
        ));
        drop(first);
        #[cfg(unix)]
        {
            use std::os::unix::fs::{PermissionsExt, symlink};
            assert_eq!(
                fs::metadata(&directory).unwrap().permissions().mode() & 0o777,
                0o700
            );
            let external = temporary.path().join("external");
            fs::create_dir(&external).unwrap();
            let link = temporary.path().join("linked-directory");
            symlink(&external, &link).unwrap();
            assert!(matches!(
                Workspace::new(link),
                Err(CommandError::StorageUnavailable)
            ));
            symlink(&external, directory.join(WRAPPED_KEY_FILE)).unwrap();
            let vault = FakeVault::new(&directory, false);
            let mut workspace = Workspace::new(directory).unwrap();
            assert_eq!(
                workspace.bootstrap(Ok(&vault)).storage,
                StorageState::RecoveryRequired
            );
        }
    }

    #[test]
    fn authenticated_passphrase_with_a_newer_database_schema_returns_recovery() {
        let temporary = tempfile::tempdir().unwrap();
        let directory = temporary.path().join("workspace");
        let mut workspace = Workspace::new(directory.clone()).unwrap();
        let key = [0x2a_u8; 32];
        let passphrase = "x";
        let wrapped = keywrap::wrap_key(&key, passphrase.as_bytes()).unwrap();
        let mut file = secure_open(&directory.join(WRAPPED_KEY_FILE), true, true).unwrap();
        file.write_all(&wrapped).unwrap();
        file.sync_all().unwrap();
        drop(file);
        let database = directory.join(DATABASE_FILE);
        drop(Storage::create(&database, &key).unwrap());
        let connection = rusqlite::Connection::open(&database).unwrap();
        let synthetic_raw_key = format!("x'{}'", "2a".repeat(32));
        connection
            .pragma_update(None, "key", synthetic_raw_key)
            .unwrap();
        connection.pragma_update(None, "user_version", 99).unwrap();
        drop(connection);
        let before = fs::read(&database).unwrap();
        let state = workspace.unlock(Zeroizing::new(passphrase.into())).unwrap();
        assert_eq!(state.storage, StorageState::RecoveryRequired);
        assert_eq!(state.storage_issue, Some(StorageIssue::StorageUnavailable));
        assert_eq!(fs::read(&database).unwrap(), before);
        assert_eq!(fs::read(directory.join(WRAPPED_KEY_FILE)).unwrap(), wrapped);
    }

    #[test]
    fn available_os_store_prevents_unnecessary_fallback_and_short_setup_passphrases_fail() {
        let temporary = tempfile::tempdir().unwrap();
        let directory = temporary.path().join("workspace");
        let vault = FakeVault::new(&directory, true);
        let mut workspace = Workspace::new(directory.clone()).unwrap();
        assert!(matches!(
            workspace.setup(
                Ok(&vault),
                Some(Zeroizing::new("synthetic long passphrase".into()))
            ),
            Err(CommandError::OsStoreRequired)
        ));
        vault.available.set(false);
        assert!(matches!(
            workspace.setup(Ok(&vault), Some(Zeroizing::new("short".into()))),
            Err(CommandError::PassphraseTooShort)
        ));
        assert!(!vault.database.exists());
        assert!(!directory.join(WRAPPED_KEY_FILE).exists());
        assert!(vault.key.borrow().is_none());
        assert_eq!(
            workspace.bootstrap(Ok(&vault)).key_protection,
            KeyProtection::PassphraseRequired
        );
        vault.available.set(true);
        assert!(matches!(
            workspace.setup(
                Ok(&vault),
                Some(Zeroizing::new("synthetic long passphrase".into()))
            ),
            Err(CommandError::OsStoreRequired)
        ));
        assert!(!vault.database.exists());
        assert!(!directory.join(WRAPPED_KEY_FILE).exists());
    }
}
