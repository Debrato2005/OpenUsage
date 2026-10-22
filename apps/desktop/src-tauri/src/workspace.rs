use crate::keyvault::{KeyVault, VaultError};
use crate::provider::{self, ProviderCoordinator};
use crate::restore::{self, Cancelled, RestoreCache, RestorePreview};
use openusage_storage::{
    ProfileConnectionState, ProfileEnvironment, ProfileError, Storage, StorageError, Theme, keywrap,
};
use serde::Serialize;
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};
use zeroize::Zeroizing;

mod activity_storage;
mod alert_history;
mod collection_storage;
mod monitoring;
mod operational_alerts;
mod profile_plan;
mod quota_history;
mod quota_storage;
mod reset_notifications;
pub(crate) use activity_storage::{
    CombinedHeatmapEligibility, ProfileActivityHistory, ProfileHeatmap, ReadAssociationReview,
    SavedProfileActivity,
};
pub(crate) use alert_history::SavedAlertHistory;
#[cfg(test)]
use monitoring::MonitoringState;
pub(crate) use monitoring::{MonitoringIssue, MonitoringStatus};
pub(crate) use profile_plan::SavedProfilePlan;
pub(crate) use quota_history::ProfileQuotaHistory;
pub(crate) use quota_storage::SavedProfileQuota;
pub(crate) use reset_notifications::{NotificationPreferences, ResetNotificationPreferences};

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
    DisplayScaleUnavailable,
    ProviderPlanUnavailable,
    StorageUnavailable,
    WorkspaceInUse,
    AlreadyInitialized,
    InvalidPassphrase,
    PassphraseTooShort,
    PassphraseTooLong,
    OsStoreRequired,
    WorkspaceLocked,
    BackupUnavailable,
    ExportUnavailable,
    ExportTooLarge,
    ExportUnsafeText,
    InvalidExportSelection,
    BackupNotFound,
    InvalidBackup,
    BackupAuthenticationFailed,
    InsufficientSpace,
    InvalidProfileId,
    InvalidProfileLabel,
    ProfileCapacityReached,
    ProfileNotFound,
    ProfileUnavailable,
    UnsupportedBackupContents,
    InvalidBackupContents,
    RestoreCheckpointFailed,
    StaleRestorePreview,
    RestoreFailed,
    CheckpointNotFound,
    RestoreHomeUnavailable,
    InvalidProviderExecutable,
    ProviderExecutableNotApproved,
    ProviderSourceUnverified,
    StaleProviderApproval,
    ProviderUnavailable,
    ProviderUnsupported,
    ProviderAuthenticationRequired,
    ProviderCheckInProgress,
    ProviderCapacityReached,
    ProviderRefreshCooldown,
    ProviderRefreshBackoff,
    ProviderCheckCancelled,
    ProviderCheckTimedOut,
    ProviderProtocolFailed,
    ProviderCleanupFailed,
    ProviderHomeUnavailable,
    ProviderProfileArchived,
    ProviderShuttingDown,
    ProviderLoginNotFound,
    ProviderLoginInProgress,
    ProviderLoginPromptPending,
    ProviderLoginCancelled,
    ProviderLoginTimedOut,
    ProviderLoginFailed,
    ProviderBrowserUnavailable,
    ProviderDiscoveryUnavailable,
    ProviderSetupUnavailable,
    ProviderActivityNotFound,
    ProviderActivityInProgress,
    ProviderActivityCancelled,
    ProviderActivityTimedOut,
    ProviderActivityFailed,
    ProviderActivitySaveFailed,
    ProviderActivityAssociationChanged,
    ProviderActivityReviewUnavailable,
    ProviderActivityLoadFailed,
    ProviderQuotaNotFound,
    ProviderQuotaInProgress,
    ProviderQuotaCancelled,
    ProviderQuotaTimedOut,
    ProviderQuotaFailed,
    ProviderQuotaSaveFailed,
    ProviderQuotaLoadFailed,
    ProviderQuotaAssociationChanged,
    ProviderQuotaReviewUnavailable,
    ProviderCollectionNotFound,
    ProviderCollectionInProgress,
    ProviderCollectionCancelled,
    ProviderCollectionSaveFailed,
    ProviderCollectionAssociationChanged,
    ProviderCollectionReviewUnavailable,
    ProviderAccountChanged,
    ProviderAccountIdentityUnavailable,
    ProviderIncompatibleQuotaScope,
    MonitoringNotEligible,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileView {
    pub profiles: Vec<ProfileRecord>,
    pub active_profiles: u32,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileRecord {
    pub id: String,
    pub label: String,
    pub environment: &'static str,
    pub connection_state: &'static str,
    pub archived: bool,
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
    restore_preview: RestoreCache,
    provider: ProviderCoordinator,
    activity_sessions: std::collections::BTreeMap<String, (String, Option<std::time::Instant>)>,
    quota_sessions: std::collections::BTreeMap<String, (String, Option<std::time::Instant>)>,
    quota_requests: std::collections::BTreeMap<String, String>,
    activity_requests: std::collections::BTreeMap<String, String>,
    activity_reviews: std::collections::BTreeMap<String, activity_storage::CachedAssociationReview>,
    monitoring: monitoring::RuntimeMonitoring,
}

impl Workspace {
    #[cfg(test)]
    pub fn new(directory: PathBuf) -> Result<Self, CommandError> {
        Self::with_restore_cache(directory, RestoreCache::default())
    }

    #[cfg(test)]
    pub fn with_restore_cache(
        directory: PathBuf,
        restore_preview: RestoreCache,
    ) -> Result<Self, CommandError> {
        Self::with_services(directory, restore_preview, ProviderCoordinator::default())
    }

    pub fn with_services(
        directory: PathBuf,
        restore_preview: RestoreCache,
        provider: ProviderCoordinator,
    ) -> Result<Self, CommandError> {
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
            monitoring: monitoring::RuntimeMonitoring::new()?,
            restore_preview,
            provider,
            activity_sessions: Default::default(),
            activity_requests: Default::default(),
            quota_sessions: Default::default(),
            quota_requests: Default::default(),
            activity_reviews: Default::default(),
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
        self.restore_preview.clear();
        Ok(self.ready())
    }

    pub fn backup_snapshot(&mut self) -> Result<openusage_storage::BackupSnapshot, CommandError> {
        self.storage
            .as_mut()
            .ok_or(CommandError::WorkspaceLocked)?
            .backup_snapshot()
            .map_err(|_| CommandError::BackupUnavailable)
    }

    pub fn readings_export(
        &mut self,
        profile_ids: &[String],
        include_identifiers: bool,
    ) -> Result<openusage_storage::ReadingsExport, CommandError> {
        self.storage
            .as_mut()
            .ok_or(CommandError::WorkspaceLocked)?
            .readings_export(profile_ids, include_identifiers)
            .map_err(|error| match error {
                openusage_storage::ExportError::InvalidSelection => {
                    CommandError::InvalidExportSelection
                }
                openusage_storage::ExportError::TooLarge => CommandError::ExportTooLarge,
                openusage_storage::ExportError::ReadFailed => CommandError::ExportUnavailable,
            })
    }

    pub fn list_profiles(&self) -> Result<ProfileView, CommandError> {
        let profiles = self
            .storage
            .as_ref()
            .ok_or(CommandError::WorkspaceLocked)?
            .list_profiles()
            .map_err(profile_error)?;
        let active_profiles = profiles.iter().filter(|profile| !profile.archived).count();
        if active_profiles > 10 {
            return Err(CommandError::ProfileUnavailable);
        }
        Ok(ProfileView {
            active_profiles: active_profiles as u32,
            profiles: profiles
                .into_iter()
                .map(|profile| ProfileRecord {
                    id: profile.id,
                    label: profile.label,
                    environment: match profile.environment {
                        ProfileEnvironment::Native => "native",
                        ProfileEnvironment::Wsl => "wsl",
                    },
                    connection_state: match profile.connection_state {
                        ProfileConnectionState::Connected => "connected",
                        ProfileConnectionState::AuthenticationRequired => "authentication_required",
                        ProfileConnectionState::Unauthorized => "unauthorized",
                        ProfileConnectionState::Offline => "offline",
                        ProfileConnectionState::Unsupported => "unsupported",
                        ProfileConnectionState::Error => "error",
                    },
                    archived: profile.archived,
                })
                .collect(),
        })
    }

    pub fn create_profile(&mut self, label: &str) -> Result<ProfileView, CommandError> {
        self.storage.as_ref().ok_or(CommandError::WorkspaceLocked)?;
        validate_profile_label(label)?;
        let storage = self.storage.as_mut().ok_or(CommandError::WorkspaceLocked)?;
        let mut random = [0u8; 16];
        getrandom::fill(&mut random).map_err(|_| CommandError::ProfileUnavailable)?;
        let id = uuid::Builder::from_random_bytes(random)
            .into_uuid()
            .to_string();
        let parent = self.directory.join("profiles");
        prepare_directory(&parent).map_err(|_| CommandError::ProfileUnavailable)?;
        let home = parent.join(&id);
        let mut builder = fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder
            .create(&home)
            .map_err(|_| CommandError::ProfileUnavailable)?;
        let result = (|| {
            prepare_directory(&home).map_err(|_| CommandError::ProfileUnavailable)?;
            #[cfg(unix)]
            for directory in [&home, &parent, &self.directory] {
                File::open(directory)
                    .and_then(|file| file.sync_all())
                    .map_err(|_| CommandError::ProfileUnavailable)?;
            }
            storage
                .create_profile(&id, label, &id)
                .map_err(profile_error)?;
            Ok(())
        })();
        if let Err(error) = result {
            // Remove only this newly created empty home. Never remove existing files or homes.
            let _ = fs::remove_dir(&home);
            return Err(error);
        }
        self.restore_preview.clear();
        self.list_profiles()
    }

    pub fn rename_profile(&mut self, id: &str, label: &str) -> Result<ProfileView, CommandError> {
        validate_profile_id(id)?;
        validate_profile_label(label)?;
        self.storage.as_ref().ok_or(CommandError::WorkspaceLocked)?;
        self.storage
            .as_mut()
            .ok_or(CommandError::WorkspaceLocked)?
            .rename_profile(id, label)
            .map_err(profile_error)?;
        self.restore_preview.clear();
        self.list_profiles()
    }

    pub fn set_profile_archived(
        &mut self,
        id: &str,
        archived: bool,
    ) -> Result<ProfileView, CommandError> {
        validate_profile_id(id)?;
        self.storage.as_ref().ok_or(CommandError::WorkspaceLocked)?;
        self.storage
            .as_mut()
            .ok_or(CommandError::WorkspaceLocked)?
            .set_profile_archived(id, archived)
            .map_err(profile_error)?;
        if archived {
            self.revoke_monitoring_profile(id, MonitoringIssue::ProfileUnavailable);
            self.activity_reviews.remove(id);
            self.quota_requests.remove(id);
            self.activity_requests.remove(id);
            // Keep archive and ticket invalidation serialized. These bounded joins wait only
            // for core provider workers, which never acquire the Workspace lock.
            self.provider.cancel_profile_operations(id)?;
            self.monitoring_profile_cancelled(id);
        }
        self.restore_preview.clear();
        self.list_profiles()
    }

    pub fn backup_inspection_directory(&self) -> Result<PathBuf, CommandError> {
        self.storage.as_ref().ok_or(CommandError::WorkspaceLocked)?;
        Ok(self.directory.join("backup-inspection"))
    }

    pub fn prepare_restore_preview(
        &mut self,
        source_directory: &Path,
        file_name: &str,
        password: Zeroizing<String>,
    ) -> Result<RestorePreview, CommandError> {
        self.restore_preview.require_open()?;
        self.restore_preview.clear();
        let storage = self.storage.as_mut().ok_or(CommandError::WorkspaceLocked)?;
        let prepared = restore::prepare(
            storage,
            &self.directory,
            source_directory,
            file_name,
            password,
        )?;
        self.restore_preview.install(prepared)
    }

    pub fn list_restore_checkpoints(&self) -> Result<restore::CheckpointList, CommandError> {
        self.storage.as_ref().ok_or(CommandError::WorkspaceLocked)?;
        restore::list_checkpoints(&self.directory)
    }

    pub fn prepare_checkpoint_restore_preview(
        &mut self,
        id: &str,
    ) -> Result<restore::CheckpointRestorePreview, CommandError> {
        restore::validate_preview_id(id)?;
        self.restore_preview.require_open()?;
        self.restore_preview.clear();
        let storage = self.storage.as_mut().ok_or(CommandError::WorkspaceLocked)?;
        let prepared = restore::prepare_checkpoint(storage, &self.directory, id)?;
        let preview = self.restore_preview.install(prepared)?;
        Ok(restore::CheckpointRestorePreview::from_preview(
            id.into(),
            preview,
        ))
    }

    pub fn apply_restore(
        &mut self,
        id: &str,
        confirmed: bool,
    ) -> Result<restore::Applied, CommandError> {
        restore::validate_preview_id(id)?;
        if !confirmed {
            return Err(CommandError::InvalidArguments);
        }
        self.storage.as_ref().ok_or(CommandError::WorkspaceLocked)?;
        let prepared = self.restore_preview.take(id)?;
        // Owned children must finish cleanup before records can point at different managed homes.
        self.revoke_all_monitoring(MonitoringIssue::SourceChanged);
        self.provider.quiesce()?;
        self.activity_reviews.clear();
        let storage = self.storage.as_mut().ok_or(CommandError::WorkspaceLocked)?;
        let applied = restore::apply(storage, &self.directory, prepared)?;
        // Restored descriptive connection states never restore runtime monitoring permission.
        self.monitoring.reset_after_restore();
        self.activity_sessions.clear();
        self.quota_sessions.clear();
        self.quota_requests.clear();
        self.activity_requests.clear();
        self.activity_reviews.clear();
        Ok(applied)
    }

    pub fn cancel_restore_preview(&mut self, id: &str) -> Result<Cancelled, CommandError> {
        restore::validate_preview_id(id)?;
        self.storage.as_ref().ok_or(CommandError::WorkspaceLocked)?;
        self.restore_preview.cancel(id)
    }

    pub fn inspect_provider_executable(
        &self,
        path: &str,
    ) -> Result<provider::ExecutableCandidate, CommandError> {
        self.storage.as_ref().ok_or(CommandError::WorkspaceLocked)?;
        self.provider.inspect(path)
    }

    pub fn provider_discovery_profiles(&self) -> Result<PathBuf, CommandError> {
        self.storage.as_ref().ok_or(CommandError::WorkspaceLocked)?;
        Ok(self.directory.join("profiles"))
    }

    pub fn approve_provider_executable(
        &self,
        id: &str,
        confirmed: bool,
    ) -> Result<provider::ExecutableApproval, CommandError> {
        self.storage.as_ref().ok_or(CommandError::WorkspaceLocked)?;
        self.provider.approve(id, confirmed)
    }

    pub fn start_profile_check(
        &mut self,
        id: &str,
    ) -> Result<provider::PendingCheck, CommandError> {
        let (home, revision, path) = self.selected_provider_home(id)?;
        self.provider.start(id, &home, revision, &path)
    }

    fn selected_provider_home(
        &mut self,
        id: &str,
    ) -> Result<(String, openusage_storage::StorageRevision, PathBuf), CommandError> {
        validate_profile_id(id)?;
        let storage = self.storage.as_mut().ok_or(CommandError::WorkspaceLocked)?;
        let state = storage
            .restore_state()
            .map_err(|_| CommandError::StorageUnavailable)?;
        let profile = storage
            .list_profiles()
            .map_err(profile_error)?
            .into_iter()
            .find(|profile| profile.id == id)
            .ok_or(CommandError::ProfileNotFound)?;
        if profile.archived {
            return Err(CommandError::ProviderProfileArchived);
        }
        if profile.environment != ProfileEnvironment::Native {
            return Err(CommandError::ProviderUnsupported);
        }
        let mappings = storage
            .restore_home_mappings(&state.revision)
            .map_err(|_| CommandError::ProviderHomeUnavailable)?;
        let home = mappings
            .into_iter()
            .find(|home| home.profile_id == id)
            .ok_or(CommandError::ProviderHomeUnavailable)?;
        let parent = self.directory.join("profiles");
        let path = parent.join(&home.managed_home_reference);
        for directory in [&parent, &path] {
            if !existing_private_directory(directory)
                .map_err(|_| CommandError::ProviderHomeUnavailable)?
            {
                return Err(CommandError::ProviderHomeUnavailable);
            }
        }
        Ok((home.managed_home_reference, state.revision, path))
    }

    pub fn begin_profile_login(
        &mut self,
        id: &str,
        approval_id: &str,
        confirmed: bool,
    ) -> Result<provider::LoginReceipt, CommandError> {
        if !confirmed {
            return Err(CommandError::InvalidArguments);
        }
        restore::validate_preview_id(approval_id)?;
        self.revoke_monitoring_profile(id, MonitoringIssue::SignInRequired);
        let (home, revision, path) = self.selected_provider_home(id)?;
        let mut receipt = self
            .provider
            .begin_login(id, approval_id, &home, revision, &path)?;
        // A newly accepted sign-in supersedes replies waiting for the Workspace lock.
        self.quota_requests.remove(id);
        self.activity_requests.remove(id);
        receipt.monitoring = self.monitoring_state().as_str();
        Ok(receipt)
    }

    pub fn await_profile_login_prompt(
        &self,
        id: &str,
        attempt: &str,
    ) -> Result<provider::AwaitingLoginPrompt, CommandError> {
        self.storage.as_ref().ok_or(CommandError::WorkspaceLocked)?;
        self.provider.await_login_prompt(id, attempt)
    }

    pub fn open_profile_login_page(
        &mut self,
        id: &str,
        attempt: &str,
        dispatch: impl FnOnce(&'static str) -> Result<(), CommandError>,
    ) -> Result<(), CommandError> {
        self.storage.as_ref().ok_or(CommandError::WorkspaceLocked)?;
        let binding = self.provider.login_browser_binding(id, attempt)?;
        self.require_login_binding(&binding)?;
        self.provider.open_login_browser(id, attempt, dispatch)
    }

    pub fn complete_profile_login(
        &self,
        id: &str,
        attempt: &str,
    ) -> Result<provider::CompletingLogin, CommandError> {
        self.storage.as_ref().ok_or(CommandError::WorkspaceLocked)?;
        self.provider.complete_login(id, attempt)
    }

    pub fn finish_profile_login_prompt(
        &mut self,
        ready: provider::ReadyLoginPrompt,
    ) -> Result<provider::LoginPromptReceipt, CommandError> {
        if let Err(error) = self.require_login_binding(&ready.binding) {
            ready.abort();
            return Err(error);
        }
        ready.publish()
    }

    pub fn finish_profile_login(
        &mut self,
        completed: provider::CompletedLogin,
    ) -> Result<provider::LoginOutcome, CommandError> {
        if let Err(error) = self.require_login_binding(&completed.binding) {
            completed.abort();
            return Err(error);
        }
        let eligibility = completed.binding.clone();
        let storage = self.storage.as_mut().ok_or(CommandError::WorkspaceLocked)?;
        let mut changed = false;
        let mut outcome = completed.publish(|binding, diagnostic| {
            let previous = storage
                .list_profiles()
                .map_err(profile_error)?
                .into_iter()
                .find(|profile| profile.id == binding.profile_id)
                .ok_or(CommandError::ProviderLoginCancelled)?;
            let connection = match diagnostic.identity_confidence {
                provider::DiagnosticIdentity::Provisional => ProfileConnectionState::Connected,
                provider::DiagnosticIdentity::Unknown => ProfileConnectionState::Error,
            };
            let committed = storage
                .record_profile_connection(&binding.profile_id, &binding.home_reference, connection)
                .map_err(|error| match error {
                    ProfileError::NotFound | ProfileError::StaleConnectionTarget => {
                        CommandError::ProviderLoginCancelled
                    }
                    other => profile_error(other),
                })?;
            changed = committed.connection_state != previous.connection_state;
            Ok(())
        })?;
        if changed {
            self.restore_preview.clear();
        }
        if outcome.diagnostic.identity_confidence == provider::DiagnosticIdentity::Provisional {
            self.monitoring_login_committed(eligibility, outcome.attempt_id.clone());
        } else {
            self.revoke_monitoring_profile(
                &eligibility.profile_id,
                MonitoringIssue::SignInRequired,
            );
        }
        outcome.diagnostic.monitoring = self.monitoring_state().as_str();
        Ok(outcome)
    }

    fn require_login_binding(
        &mut self,
        binding: &provider::LoginBinding,
    ) -> Result<(), CommandError> {
        self.require_provider_binding(
            &binding.profile_id,
            &binding.home_reference,
            &binding.revision,
            CommandError::ProviderLoginCancelled,
        )
    }

    /// Revalidate a selected native profile if application records changed during an owned wait.
    fn require_provider_binding(
        &mut self,
        profile_id: &str,
        home_reference: &str,
        revision: &openusage_storage::StorageRevision,
        cancelled: CommandError,
    ) -> Result<(), CommandError> {
        let storage = self.storage.as_mut().ok_or(CommandError::WorkspaceLocked)?;
        let state = storage
            .restore_state()
            .map_err(|_| CommandError::StorageUnavailable)?;
        if &state.revision != revision {
            let profile = storage
                .list_profiles()
                .map_err(profile_error)?
                .into_iter()
                .find(|profile| profile.id == profile_id)
                .ok_or(cancelled)?;
            if profile.archived || profile.environment != ProfileEnvironment::Native {
                return Err(cancelled);
            }
            let homes = storage
                .restore_home_mappings(&state.revision)
                .map_err(|_| CommandError::ProviderHomeUnavailable)?;
            if !homes.iter().any(|home| {
                home.profile_id == profile_id && home.managed_home_reference == home_reference
            }) {
                return Err(cancelled);
            }
        }
        Ok(())
    }

    /// This method runs only after the owned provider worker has joined.
    pub fn finish_profile_check(
        &mut self,
        check: provider::JoinedCheck,
    ) -> Result<provider::ConnectionDiagnostic, CommandError> {
        if let Err(error) = self.require_provider_binding(
            &check.profile_id,
            &check.home_reference,
            &check.revision,
            CommandError::ProviderCheckCancelled,
        ) {
            check.abort();
            return Err(error);
        }
        let mut diagnostic = check.finish()?;
        if diagnostic.authentication != provider::DiagnosticAuthentication::Chatgpt
            || diagnostic.identity_confidence != provider::DiagnosticIdentity::Provisional
        {
            self.revoke_monitoring_profile(&diagnostic.profile_id, MonitoringIssue::SignInRequired);
        }
        diagnostic.monitoring = self.monitoring_state().as_str();
        Ok(diagnostic)
    }

    pub fn load_profile_refresh_status(
        &self,
        id: &str,
    ) -> Result<provider::ProfileRefreshStatus, CommandError> {
        validate_profile_id(id)?;
        let storage = self.storage.as_ref().ok_or(CommandError::WorkspaceLocked)?;
        if !storage
            .list_profiles()
            .map_err(profile_error)?
            .iter()
            .any(|profile| profile.id == id)
        {
            return Err(CommandError::ProfileNotFound);
        }
        self.provider.refresh_status(id)
    }

    pub fn begin_profile_collection_read(
        &mut self,
        id: &str,
        approval_id: &str,
    ) -> Result<provider::CollectionReceipt, CommandError> {
        let receipt = self.begin_profile_collection(id, approval_id, None)?;
        self.activity_reviews.remove(id);
        Ok(receipt)
    }

    pub fn review_profile_collection_association(
        &mut self,
        id: &str,
        approval_id: &str,
    ) -> Result<ReadAssociationReview, CommandError> {
        self.review_profile_association(id, approval_id, activity_storage::ReadKind::Collection)
    }

    pub fn begin_profile_collection_reassociation(
        &mut self,
        id: &str,
        approval_id: &str,
        review_id: &str,
        confirmed: bool,
    ) -> Result<provider::CollectionReceipt, CommandError> {
        if !confirmed {
            return Err(CommandError::InvalidArguments);
        }
        restore::validate_preview_id(review_id)?;
        let review =
            self.valid_profile_review(id, approval_id, activity_storage::ReadKind::Collection)?;
        if review.review_id != review_id {
            return Err(CommandError::ProviderCollectionReviewUnavailable);
        }
        let receipt = self.begin_profile_collection(id, approval_id, Some(review))?;
        self.activity_reviews.remove(id);
        Ok(receipt)
    }

    fn begin_profile_collection(
        &mut self,
        id: &str,
        approval_id: &str,
        review: Option<activity_storage::CachedAssociationReview>,
    ) -> Result<provider::CollectionReceipt, CommandError> {
        restore::validate_preview_id(approval_id)?;
        let (home, revision, path) = self.selected_provider_home(id)?;
        let expected_binding = self
            .storage
            .as_ref()
            .ok_or(CommandError::WorkspaceLocked)?
            .profile_binding(id)
            .map_err(|_| CommandError::ProviderCollectionSaveFailed)?
            .map(|b| b.binding_id);
        if review
            .as_ref()
            .is_some_and(|r| Some(&r.previous_binding.binding_id) != expected_binding.as_ref())
        {
            return Err(CommandError::ProviderCollectionReviewUnavailable);
        }
        let mut receipt = self.provider.begin_collection(
            id,
            approval_id,
            &home,
            revision,
            &path,
            provider::ReadAssociation {
                expected_binding,
                review: review.map(|r| r.fence),
            },
        )?;
        self.quota_requests
            .insert(id.into(), receipt.request_id.clone());
        self.activity_requests
            .insert(id.into(), receipt.request_id.clone());
        self.record_monitoring_read(id, &receipt.request_id);
        receipt.monitoring = self.monitoring_state().as_str();
        Ok(receipt)
    }

    pub fn complete_profile_collection_read(
        &self,
        id: &str,
        request_id: &str,
    ) -> Result<provider::CompletingCollection, CommandError> {
        self.storage.as_ref().ok_or(CommandError::WorkspaceLocked)?;
        self.provider.complete_collection(id, request_id)
    }

    pub fn begin_profile_quota_read(
        &mut self,
        id: &str,
        approval_id: &str,
    ) -> Result<provider::QuotaReceipt, CommandError> {
        let receipt = self.begin_profile_quota(id, approval_id, None)?;
        self.activity_reviews.remove(id);
        Ok(receipt)
    }

    pub fn review_profile_quota_association(
        &mut self,
        id: &str,
        approval_id: &str,
    ) -> Result<ReadAssociationReview, CommandError> {
        self.review_profile_association(id, approval_id, activity_storage::ReadKind::Quota)
    }

    pub fn begin_profile_quota_reassociation(
        &mut self,
        id: &str,
        approval_id: &str,
        review_id: &str,
        confirmed: bool,
    ) -> Result<provider::QuotaReceipt, CommandError> {
        if !confirmed {
            return Err(CommandError::InvalidArguments);
        }
        restore::validate_preview_id(review_id)?;
        let review =
            self.valid_profile_review(id, approval_id, activity_storage::ReadKind::Quota)?;
        if review.review_id != review_id {
            return Err(CommandError::ProviderQuotaReviewUnavailable);
        }
        let receipt = self.begin_profile_quota(id, approval_id, Some(review))?;
        self.activity_reviews.remove(id);
        Ok(receipt)
    }

    fn begin_profile_quota(
        &mut self,
        id: &str,
        approval_id: &str,
        review: Option<activity_storage::CachedAssociationReview>,
    ) -> Result<provider::QuotaReceipt, CommandError> {
        restore::validate_preview_id(approval_id)?;
        let (home, revision, path) = self.selected_provider_home(id)?;
        let expected_binding = self
            .storage
            .as_ref()
            .ok_or(CommandError::WorkspaceLocked)?
            .profile_binding(id)
            .map_err(|_| CommandError::ProviderQuotaLoadFailed)?
            .map(|b| b.binding_id);
        if review
            .as_ref()
            .is_some_and(|r| Some(&r.previous_binding.binding_id) != expected_binding.as_ref())
        {
            return Err(CommandError::ProviderQuotaReviewUnavailable);
        }
        let mut receipt = self.provider.begin_quota(
            id,
            approval_id,
            &home,
            revision,
            &path,
            provider::ReadAssociation {
                expected_binding,
                review: review.map(|r| r.fence),
            },
        )?;
        self.quota_requests
            .insert(id.into(), receipt.request_id.clone());
        self.record_monitoring_read(id, &receipt.request_id);
        receipt.monitoring = self.monitoring_state().as_str();
        Ok(receipt)
    }

    pub fn complete_profile_quota_read(
        &self,
        id: &str,
        request_id: &str,
    ) -> Result<provider::CompletingQuota, CommandError> {
        self.storage.as_ref().ok_or(CommandError::WorkspaceLocked)?;
        self.provider.complete_quota(id, request_id)
    }

    pub fn begin_profile_activity_read(
        &mut self,
        id: &str,
        approval_id: &str,
    ) -> Result<provider::ActivityReceipt, CommandError> {
        let receipt = self.begin_profile_activity(id, approval_id, None)?;
        self.activity_reviews.remove(id);
        Ok(receipt)
    }

    pub fn begin_profile_activity_reassociation(
        &mut self,
        id: &str,
        approval_id: &str,
        review_id: &str,
        confirmed: bool,
    ) -> Result<provider::ActivityReceipt, CommandError> {
        if !confirmed {
            return Err(CommandError::InvalidArguments);
        }
        restore::validate_preview_id(review_id)?;
        let review = self.valid_activity_review(id, approval_id)?;
        if review.review_id != review_id {
            return Err(CommandError::ProviderActivityReviewUnavailable);
        }
        let receipt = self.begin_profile_activity(id, approval_id, Some(review))?;
        // Only a successful start consumes the one-use approval.
        self.activity_reviews.remove(id);
        Ok(receipt)
    }

    fn begin_profile_activity(
        &mut self,
        id: &str,
        approval_id: &str,
        review: Option<activity_storage::CachedAssociationReview>,
    ) -> Result<provider::ActivityReceipt, CommandError> {
        restore::validate_preview_id(approval_id)?;
        let (home, revision, path) = self.selected_provider_home(id)?;
        let expected_binding = self
            .storage
            .as_ref()
            .ok_or(CommandError::WorkspaceLocked)?
            .profile_binding(id)
            .map_err(|_| CommandError::ProviderActivityLoadFailed)?
            .map(|binding| binding.binding_id);
        if review.as_ref().is_some_and(|review| {
            Some(&review.previous_binding.binding_id) != expected_binding.as_ref()
        }) {
            return Err(CommandError::ProviderActivityReviewUnavailable);
        }
        let mut receipt = self.provider.begin_activity(
            id,
            approval_id,
            &home,
            revision,
            &path,
            provider::ReadAssociation {
                expected_binding,
                review: review.map(|review| review.fence),
            },
        )?;
        self.activity_requests
            .insert(id.into(), receipt.request_id.clone());
        self.record_monitoring_read(id, &receipt.request_id);
        receipt.monitoring = self.monitoring_state().as_str();
        Ok(receipt)
    }

    pub fn complete_profile_activity_read(
        &self,
        id: &str,
        request_id: &str,
    ) -> Result<provider::CompletingActivity, CommandError> {
        self.storage.as_ref().ok_or(CommandError::WorkspaceLocked)?;
        self.provider.complete_activity(id, request_id)
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
                    monitoring: self.monitoring_state().as_str(),
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

impl Drop for Workspace {
    fn drop(&mut self) {
        // Stop owned processes before field cleanup releases the workspace lease or database.
        self.restore_preview.close();
        let _ = self.provider.close();
    }
}

pub fn validate_profile_id(id: &str) -> Result<(), CommandError> {
    let parsed = uuid::Uuid::parse_str(id).map_err(|_| CommandError::InvalidProfileId)?;
    if parsed.hyphenated().to_string() != id {
        return Err(CommandError::InvalidProfileId);
    }
    Ok(())
}

pub fn validate_profile_label(label: &str) -> Result<(), CommandError> {
    if label.len() > 1024
        || label.chars().any(char::is_control)
        || !(1..=100).contains(&label.trim().chars().count())
    {
        return Err(CommandError::InvalidProfileLabel);
    }
    Ok(())
}

fn profile_error(error: ProfileError) -> CommandError {
    match error {
        ProfileError::InvalidId => CommandError::InvalidProfileId,
        ProfileError::InvalidLabel => CommandError::InvalidProfileLabel,
        ProfileError::CapacityReached => CommandError::ProfileCapacityReached,
        ProfileError::NotFound => CommandError::ProfileNotFound,
        _ => CommandError::ProfileUnavailable,
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
    check_directory(path, true).map(|_| ())
}

pub(crate) fn existing_private_directory(path: &Path) -> Result<bool, CommandError> {
    check_directory(path, false)
}

fn check_directory(path: &Path, create: bool) -> Result<bool, CommandError> {
    if !path.is_absolute()
        || path
            .components()
            .any(|part| matches!(part, std::path::Component::ParentDir))
    {
        return Err(CommandError::StorageUnavailable);
    }
    for ancestor in path.ancestors().collect::<Vec<_>>().into_iter().rev() {
        let metadata = match fs::symlink_metadata(ancestor) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound && !create => {
                return Ok(false);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let mut builder = fs::DirBuilder::new();
                #[cfg(unix)]
                {
                    use std::os::unix::fs::DirBuilderExt;
                    builder.mode(0o700);
                }
                builder
                    .create(ancestor)
                    .map_err(|_| CommandError::StorageUnavailable)?;
                fs::symlink_metadata(ancestor).map_err(|_| CommandError::StorageUnavailable)?
            }
            Err(_) => return Err(CommandError::StorageUnavailable),
        };
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
    Ok(true)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(crate) fn require_local_filesystem(path: &Path) -> Result<(), CommandError> {
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
pub(crate) fn require_local_filesystem(path: &Path) -> Result<(), CommandError> {
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

#[cfg(all(test, unix))]
mod login_tests;

#[cfg(all(test, unix))]
mod quota_tests;

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
            workspace.backup_inspection_directory(),
            Err(CommandError::WorkspaceLocked)
        );
        assert!(!directory.join("backup-inspection").exists());
        assert_eq!(
            workspace
                .prepare_restore_preview(
                    &temporary.path().join("missing-downloads"),
                    "openusage-backup-0123456789abcdef.oubackup",
                    Zeroizing::new("synthetic archive password".into()),
                )
                .err(),
            Some(CommandError::WorkspaceLocked)
        );
        assert_eq!(
            workspace
                .cancel_restore_preview("00000000-0000-4000-8000-000000000001")
                .err(),
            Some(CommandError::WorkspaceLocked)
        );
        assert!(!directory.join("restore-staging").exists());
        assert!(!directory.join("restore-checkpoints").exists());
        assert_eq!(
            workspace.setup(Ok(&vault), None).unwrap().storage,
            StorageState::Ready
        );
        assert_eq!(
            workspace.backup_inspection_directory().unwrap(),
            directory.join("backup-inspection")
        );
        assert!(!directory.join("backup-inspection").exists());
        assert!(vault.key.borrow().is_some());
        assert!(
            !fs::read(&vault.database)
                .unwrap()
                .starts_with(b"SQLite format 3\0")
        );
        assert_eq!(workspace.set_theme(Theme::Dark).unwrap().theme, "dark");
        drop(workspace);
        let mut restarted = Workspace::new(directory).unwrap();
        assert_eq!(
            restarted.backup_inspection_directory(),
            Err(CommandError::WorkspaceLocked)
        );
        let ready = restarted.bootstrap(Ok(&vault));
        assert_eq!(ready.storage, StorageState::Ready);
        assert_eq!(ready.theme, "dark");
        assert_eq!(ready.active_profiles, Some(0));
        assert_eq!(
            ready.schema_version,
            Some(openusage_storage::CURRENT_SCHEMA_VERSION)
        );
        assert_eq!(ready.monitoring, "stopped");
    }

    #[test]
    fn failed_preview_replacement_and_live_mutation_invalidate_cached_confirmation() {
        let temporary = tempfile::tempdir().unwrap();
        let directory = temporary.path().join("workspace");
        let source = temporary.path().join("Downloads/OpenUsage");
        let vault = FakeVault::new(&directory, true);
        let mut workspace = Workspace::new(directory.clone()).unwrap();
        workspace.setup(Ok(&vault), None).unwrap();
        let profiles = workspace
            .create_profile("Synthetic current profile")
            .unwrap();
        let id = &profiles.profiles[0].id;
        let archive = crate::backup::create(
            &source,
            workspace.backup_snapshot().unwrap(),
            Zeroizing::new("synthetic restore password".into()),
        )
        .unwrap();
        let preview = workspace
            .prepare_restore_preview(
                &source,
                &archive.file_name,
                Zeroizing::new("synthetic restore password".into()),
            )
            .unwrap();
        assert_eq!(
            workspace.apply_restore(&preview.preview_id, false).err(),
            Some(CommandError::InvalidArguments)
        );
        assert_eq!(
            workspace
                .prepare_restore_preview(
                    &source,
                    "openusage-backup-0000000000000000.oubackup",
                    Zeroizing::new("synthetic restore password".into()),
                )
                .err(),
            Some(CommandError::BackupNotFound)
        );
        assert!(
            !workspace
                .cancel_restore_preview(&preview.preview_id)
                .unwrap()
                .cancelled
        );
        let preview = workspace
            .prepare_restore_preview(
                &source,
                &archive.file_name,
                Zeroizing::new("synthetic restore password".into()),
            )
            .unwrap();
        workspace
            .rename_profile(id, "Synthetic changed profile")
            .unwrap();
        assert!(
            !workspace
                .cancel_restore_preview(&preview.preview_id)
                .unwrap()
                .cancelled
        );
        assert_eq!(
            workspace.list_profiles().unwrap().profiles[0].label,
            "Synthetic changed profile"
        );
        assert_eq!(
            fs::read_dir(directory.join("restore-checkpoints"))
                .unwrap()
                .count(),
            2
        );
        assert_eq!(
            fs::read_dir(directory.join("restore-staging"))
                .unwrap()
                .count(),
            0
        );
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
        assert_eq!(
            restarted.backup_inspection_directory(),
            Err(CommandError::WorkspaceLocked)
        );
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

    #[test]
    fn native_profiles_use_empty_private_homes_and_survive_restart() {
        let temporary = tempfile::tempdir().unwrap();
        let directory = temporary.path().join("workspace");
        let vault = FakeVault::new(&directory, true);
        let mut workspace = Workspace::new(directory.clone()).unwrap();
        assert_eq!(
            workspace.list_profiles(),
            Err(CommandError::WorkspaceLocked)
        );
        assert_eq!(
            workspace.create_profile("Locked"),
            Err(CommandError::WorkspaceLocked)
        );
        assert!(!directory.join("profiles").exists());
        workspace.setup(Ok(&vault), None).unwrap();
        let created = workspace
            .create_profile(" \u{2003} Synthetic profile\u{a0} ")
            .unwrap();
        let id = created.profiles[0].id.clone();
        assert_eq!(created.active_profiles, 1);
        assert_eq!(created.profiles[0].label, "Synthetic profile");
        assert_eq!(created.profiles[0].environment, "native");
        assert_eq!(
            created.profiles[0].connection_state,
            "authentication_required"
        );
        assert_eq!(
            uuid::Uuid::parse_str(&id).unwrap().get_version(),
            Some(uuid::Version::Random)
        );
        let home = directory.join("profiles").join(&id);
        assert!(home.is_dir());
        assert_eq!(fs::read_dir(&home).unwrap().count(), 0);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&home).unwrap().permissions().mode() & 0o777,
                0o700
            );
        }
        let padded_boundary = format!("{}A", " ".repeat(1023));
        assert_eq!(
            workspace
                .rename_profile(&id, &padded_boundary)
                .unwrap()
                .profiles[0]
                .label,
            "A"
        );
        let oversized = format!("{}A", " ".repeat(1024));
        assert_eq!(
            workspace.create_profile(&oversized),
            Err(CommandError::InvalidProfileLabel)
        );
        assert_eq!(
            workspace.rename_profile(&id, &oversized),
            Err(CommandError::InvalidProfileLabel)
        );
        let unicode_boundary = "é".repeat(100);
        assert_eq!(
            workspace
                .rename_profile(&id, &unicode_boundary)
                .unwrap()
                .profiles[0]
                .label,
            unicode_boundary
        );
        workspace.rename_profile(&id, "Renamed profile").unwrap();
        let second = workspace.create_profile("Second profile").unwrap();
        assert_ne!(second.profiles[0].id, second.profiles[1].id);
        let expected = workspace.set_profile_archived(&id, true).unwrap();
        assert_eq!(expected.active_profiles, 1);
        assert!(home.is_dir());
        let serialized = serde_json::to_string(&expected).unwrap();
        assert!(!serialized.contains("home"));
        assert!(!serialized.contains(directory.to_str().unwrap()));
        assert!(
            !fs::read(&vault.database)
                .unwrap()
                .starts_with(b"SQLite format 3\0")
        );
        let unkeyed = rusqlite::Connection::open(&vault.database).unwrap();
        assert!(
            unkeyed
                .query_row("SELECT count(*) FROM profiles", [], |row| row
                    .get::<_, u32>(0))
                .is_err()
        );
        drop(unkeyed);
        drop(workspace);

        let mut restarted = Workspace::new(directory).unwrap();
        assert_eq!(
            restarted.list_profiles(),
            Err(CommandError::WorkspaceLocked)
        );
        assert_eq!(restarted.bootstrap(Ok(&vault)).active_profiles, Some(1));
        assert_eq!(restarted.list_profiles().unwrap(), expected);
        assert_eq!(fs::read_dir(home).unwrap().count(), 0);
    }

    #[test]
    fn native_profile_capacity_failure_removes_only_the_new_empty_home() {
        let temporary = tempfile::tempdir().unwrap();
        let directory = temporary.path().join("workspace");
        let vault = FakeVault::new(&directory, true);
        let mut workspace = Workspace::new(directory.clone()).unwrap();
        workspace.setup(Ok(&vault), None).unwrap();
        for invalid in ["", " \u{2003} ", "bad\nlabel", "bad\0label"] {
            assert_eq!(
                workspace.create_profile(invalid),
                Err(CommandError::InvalidProfileLabel)
            );
        }
        assert!(!directory.join("profiles").exists());
        for number in 1..=10 {
            workspace
                .create_profile(&format!("Synthetic profile {number}"))
                .unwrap();
        }
        let parent = directory.join("profiles");
        let original = workspace.list_profiles().unwrap();
        let id = original.profiles[0].id.clone();
        let sentinel = parent.join(&id).join("synthetic-owned-marker");
        fs::write(&sentinel, b"preserve this synthetic file").unwrap();
        assert_eq!(
            workspace.create_profile("Eleventh profile"),
            Err(CommandError::ProfileCapacityReached)
        );
        assert_eq!(fs::read_dir(&parent).unwrap().count(), 10);
        assert_eq!(workspace.list_profiles().unwrap(), original);
        assert_eq!(
            fs::read(&sentinel).unwrap(),
            b"preserve this synthetic file"
        );
        workspace.set_profile_archived(&id, true).unwrap();
        assert_eq!(
            workspace
                .create_profile("New active profile")
                .unwrap()
                .active_profiles,
            10
        );
        assert_eq!(fs::read_dir(&parent).unwrap().count(), 11);
        assert_eq!(
            workspace.set_profile_archived(&id, false),
            Err(CommandError::ProfileCapacityReached)
        );
        workspace
            .set_profile_archived(&original.profiles[1].id, true)
            .unwrap();
        let current = workspace.set_profile_archived(&id, false).unwrap();
        assert_eq!(current.active_profiles, 10);
        assert_eq!(current.profiles.len(), 11);
        assert_eq!(
            fs::read(&sentinel).unwrap(),
            b"preserve this synthetic file"
        );
        for invalid in ["../external", "ABCDEF01-0000-4000-8000-000000000000"] {
            assert_eq!(
                workspace.rename_profile(invalid, "Name"),
                Err(CommandError::InvalidProfileId)
            );
            assert_eq!(
                workspace.set_profile_archived(invalid, true),
                Err(CommandError::InvalidProfileId)
            );
        }
        assert_eq!(
            workspace.rename_profile(&id, "bad\tlabel"),
            Err(CommandError::InvalidProfileLabel)
        );
    }

    #[test]
    fn native_profile_write_contention_preserves_records_and_cleans_new_home() {
        use std::{collections::BTreeSet, fmt::Write as _};

        let temporary = tempfile::tempdir().unwrap();
        let directory = temporary.path().join("workspace");
        let vault = FakeVault::new(&directory, true);
        let mut workspace = Workspace::new(directory.clone()).unwrap();
        workspace.setup(Ok(&vault), None).unwrap();
        let original = workspace
            .create_profile("Existing synthetic profile")
            .unwrap();
        let parent = directory.join("profiles");
        let marker = parent
            .join(&original.profiles[0].id)
            .join("synthetic-marker");
        fs::write(&marker, b"preserve this existing synthetic home").unwrap();
        let homes_before: BTreeSet<_> = fs::read_dir(&parent)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();

        let holder = rusqlite::Connection::open_with_flags(
            &vault.database,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE | rusqlite::OpenFlags::SQLITE_OPEN_NOFOLLOW,
        )
        .unwrap();
        let key = vault.load().unwrap().unwrap();
        let mut raw_key = Zeroizing::new(String::with_capacity(67));
        raw_key.push_str("x'");
        for byte in key.iter() {
            write!(&mut *raw_key, "{byte:02x}").unwrap();
        }
        raw_key.push('\'');
        holder.pragma_update(None, "key", raw_key.as_str()).unwrap();
        drop(raw_key);
        drop(key);
        holder.execute_batch("BEGIN IMMEDIATE;").unwrap();

        assert_eq!(
            workspace.create_profile("Contended synthetic profile"),
            Err(CommandError::ProfileUnavailable)
        );
        assert_eq!(workspace.list_profiles().unwrap(), original);
        let homes_after: BTreeSet<_> = fs::read_dir(&parent)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(homes_after, homes_before);
        assert_eq!(
            fs::read(&marker).unwrap(),
            b"preserve this existing synthetic home"
        );
        assert_eq!(
            holder
                .query_row("SELECT count(*) FROM profiles", [], |row| row
                    .get::<_, u32>(0))
                .unwrap(),
            1
        );

        holder.execute_batch("ROLLBACK;").unwrap();
        let recovered = workspace
            .create_profile("Retried synthetic profile")
            .unwrap();
        assert_eq!(recovered.active_profiles, 2);
        assert_eq!(recovered.profiles.len(), 2);
        assert_eq!(
            recovered
                .profiles
                .iter()
                .find(|profile| profile.id == original.profiles[0].id),
            Some(&original.profiles[0])
        );
        assert_eq!(fs::read_dir(&parent).unwrap().count(), 2);
        let new_profile = recovered
            .profiles
            .iter()
            .find(|profile| profile.id != original.profiles[0].id)
            .unwrap();
        assert_eq!(
            fs::read_dir(parent.join(&new_profile.id)).unwrap().count(),
            0
        );
        assert_eq!(
            holder
                .query_row("SELECT count(*) FROM profiles", [], |row| row
                    .get::<_, u32>(0))
                .unwrap(),
            2
        );
    }
}

#[cfg(all(test, unix))]
mod activity_tests;

#[cfg(all(test, unix))]
mod collection_tests;

#[cfg(all(test, unix))]
mod monitoring_tests;
