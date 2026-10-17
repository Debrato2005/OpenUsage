//! Prepare a frozen restore comparison after authentication and an encrypted live checkpoint.
//! This phase applies no records. It does not complete AT-32.
//! Cancel, replacement, workspace changes, and the app exit hook drop the cached archive key.
//! Forced process termination can skip Rust cleanup, including any in-flight local archive.
//! Encrypted checkpoints remain private recovery artifacts. Windows fails closed.

use crate::{
    backup::{available_space, publish_noclobber, require_private_file, sync_directory},
    backup_inspection::{self, ContentCheck, ContentSummary},
    workspace::{CommandError, prepare_directory},
};
use openusage_import::VerifiedArchive;
use openusage_storage::{RestoreState, Storage, StorageRevision};
use serde::Serialize;
use std::{
    fs::{self, File, OpenOptions},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use zeroize::Zeroizing;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestorePreview {
    pub preview_id: String,
    pub file_name: String,
    pub current: ContentSummary,
    pub incoming: ContentSummary,
    pub checkpoint: Checkpoint,
    pub restore_available: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Checkpoint {
    pub id: String,
    pub file_bytes: String,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct Cancelled {
    pub cancelled: bool,
}

pub(crate) struct PreparedRestore {
    pub preview: RestorePreview,
    _archive: VerifiedArchive,
    _revision: StorageRevision,
}

#[derive(Clone, Default)]
pub(crate) struct RestoreCache(Arc<Mutex<CacheState>>);

#[derive(Default)]
struct CacheState {
    closed: bool,
    prepared: Option<PreparedRestore>,
}

impl RestoreCache {
    pub fn require_open(&self) -> Result<(), CommandError> {
        let state = self.0.lock().map_err(|_| CommandError::BackupUnavailable)?;
        if state.closed {
            return Err(CommandError::StorageUnavailable);
        }
        Ok(())
    }

    pub fn clear(&self) {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .prepared = None;
    }

    pub fn install(&self, prepared: PreparedRestore) -> Result<RestorePreview, CommandError> {
        let mut state = self.0.lock().map_err(|_| CommandError::BackupUnavailable)?;
        if state.closed {
            return Err(CommandError::StorageUnavailable);
        }
        let preview = prepared.preview.clone();
        state.prepared = Some(prepared);
        Ok(preview)
    }

    pub fn cancel(&self, id: &str) -> Result<Cancelled, CommandError> {
        let mut state = self.0.lock().map_err(|_| CommandError::BackupUnavailable)?;
        let cancelled = state
            .prepared
            .as_ref()
            .is_some_and(|prepared| prepared.preview.preview_id == id);
        if cancelled {
            state.prepared = None;
        }
        Ok(Cancelled { cancelled })
    }

    pub fn close(&self) {
        // No worker holds this mutex during archive, storage, or checkpoint operations.
        let mut state = self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.closed = true;
        state.prepared = None;
    }
}

pub fn validate_preview_id(id: &str) -> Result<(), CommandError> {
    if id.len() != 36
        || uuid::Uuid::parse_str(id)
            .map(|uuid| uuid.hyphenated().to_string() != id)
            .unwrap_or(true)
    {
        return Err(CommandError::InvalidArguments);
    }
    Ok(())
}

pub(crate) fn prepare(
    storage: &mut Storage,
    workspace: &Path,
    source_directory: &Path,
    file_name: &str,
    password: Zeroizing<String>,
) -> Result<PreparedRestore, CommandError> {
    if !cfg!(unix) {
        return Err(CommandError::BackupUnavailable);
    }
    let before = storage
        .restore_state()
        .map_err(|_| CommandError::StorageUnavailable)?;
    let checkpoint_budget = checkpoint_budget(before.logical_bytes)?;
    let authenticated = backup_inspection::authenticate_retained(
        source_directory,
        &workspace.join("restore-staging"),
        file_name,
        password,
        checkpoint_budget,
        ContentCheck::NativeRestore,
    )?;
    let incoming = match authenticated.inspection.database_validation {
        "valid" => authenticated
            .inspection
            .content_summary
            .ok_or(CommandError::InvalidBackupContents)?,
        "unsupported" => return Err(CommandError::UnsupportedBackupContents),
        _ => return Err(CommandError::InvalidBackupContents),
    };
    require_unchanged(&before, storage)?;
    let incoming_bytes = authenticated
        .archive
        .manifest()
        .entries
        .iter()
        .try_fold(0u64, |total, entry| total.checked_add(entry.size))
        .ok_or(CommandError::InvalidBackup)?;
    let directory = workspace.join("restore-checkpoints");
    prepare_directory(&directory).map_err(|_| CommandError::RestoreCheckpointFailed)?;
    require_space(
        available_space(&directory)?,
        checkpoint_budget,
        incoming_bytes,
    )?;
    // Child folders can be separate local mounts. Budget the live destination independently.
    require_space(
        available_space(workspace)?,
        checkpoint_budget,
        incoming_bytes,
    )?;
    let id = new_id()?;
    let preview_id = new_id()?;
    let pending = directory.join(format!(".{id}.sqlite.pending"));
    let destination = directory.join(format!("{id}.sqlite"));
    // Storage exclusively creates, authenticates, and fsyncs this encrypted checkpoint.
    // On detach failure it retains a private pending file still owned by SQLite.
    let checkpoint = storage
        .create_encrypted_checkpoint(&pending)
        .map_err(|_| CommandError::RestoreCheckpointFailed)?;
    let mut created = CreatedCheckpoint::open(pending)?;
    require_unchanged(&before, storage)?;
    require_space(available_space(&directory)?, 0, incoming_bytes)?;
    require_space(available_space(workspace)?, 0, incoming_bytes)?;
    created.publish(&destination, &directory)?;
    Ok(PreparedRestore {
        preview: RestorePreview {
            preview_id,
            file_name: file_name.into(),
            current: ContentSummary {
                active_profiles: before.summary.active_profiles,
                archived_profiles: before.summary.archived_profiles.to_string(),
                accounts: before.summary.accounts.to_string(),
            },
            incoming,
            checkpoint: Checkpoint {
                id,
                file_bytes: checkpoint.file_bytes.to_string(),
            },
            restore_available: false,
        },
        _archive: authenticated.archive,
        _revision: before.revision,
    })
}

fn require_unchanged(before: &RestoreState, storage: &mut Storage) -> Result<(), CommandError> {
    let after = storage
        .restore_state()
        .map_err(|_| CommandError::StorageUnavailable)?;
    if before.revision != after.revision {
        return Err(CommandError::StaleRestorePreview);
    }
    Ok(())
}

fn new_id() -> Result<String, CommandError> {
    let mut random = [0u8; 16];
    getrandom::fill(&mut random).map_err(|_| CommandError::RestoreCheckpointFailed)?;
    Ok(uuid::Builder::from_random_bytes(random)
        .into_uuid()
        .to_string())
}

fn checkpoint_budget(logical_bytes: u64) -> Result<u64, CommandError> {
    // Reserve journal/page overhead. This estimate is not a promised output-size bound.
    logical_bytes
        .checked_mul(2)
        .and_then(|size| size.checked_add(16 * 1024 * 1024))
        .ok_or(CommandError::InsufficientSpace)
}

fn require_space(available: u64, checkpoint: u64, incoming: u64) -> Result<(), CommandError> {
    if checkpoint
        .checked_add(incoming)
        .is_none_or(|needed| available < needed)
    {
        return Err(CommandError::InsufficientSpace);
    }
    Ok(())
}

/// Cleanup only the checkpoint inode opened after storage returns.
struct CreatedCheckpoint {
    path: PathBuf,
    file: File,
    keep: bool,
}

impl CreatedCheckpoint {
    fn open(path: PathBuf) -> Result<Self, CommandError> {
        let mut options = OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
        }
        let file = options
            .open(&path)
            .map_err(|_| CommandError::RestoreCheckpointFailed)?;
        require_private_file(&file).map_err(|_| CommandError::RestoreCheckpointFailed)?;
        Ok(Self {
            path,
            file,
            keep: false,
        })
    }

    fn owns_path(&self) -> bool {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            match (self.file.metadata(), fs::symlink_metadata(&self.path)) {
                (Ok(open), Ok(path)) => {
                    path.is_file()
                        && !path.file_type().is_symlink()
                        && open.dev() == path.dev()
                        && open.ino() == path.ino()
                }
                _ => false,
            }
        }
        #[cfg(not(unix))]
        false
    }

    fn publish(&mut self, destination: &Path, directory: &Path) -> Result<(), CommandError> {
        if !self.owns_path() {
            return Err(CommandError::RestoreCheckpointFailed);
        }
        publish_noclobber(&self.path, destination)
            .map_err(|_| CommandError::RestoreCheckpointFailed)?;
        self.path = destination.into();
        sync_directory(directory).map_err(|_| CommandError::RestoreCheckpointFailed)?;
        self.keep = true;
        Ok(())
    }
}

impl Drop for CreatedCheckpoint {
    fn drop(&mut self) {
        if !self.keep && self.owns_path() {
            let _ = fs::remove_file(&self.path);
            if let Some(parent) = self.path.parent() {
                let _ = sync_directory(parent);
            }
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::backup;
    use openusage_import::{Entry, Manifest, encrypt};
    use openusage_storage::Theme;
    use std::{
        io::Cursor,
        os::unix::fs::{OpenOptionsExt, PermissionsExt, symlink},
    };

    const PASSWORD: &str = "synthetic restore archive password";
    const LIVE_KEY: [u8; 32] = [31; 32];
    const FIRST_ID: &str = "00000000-0000-4000-8000-000000000001";
    const SECOND_ID: &str = "00000000-0000-4000-8000-000000000002";

    struct Fixture {
        _temporary: tempfile::TempDir,
        workspace: PathBuf,
        source: PathBuf,
        file_name: String,
        storage: Storage,
    }

    fn fixture() -> Fixture {
        let temporary = tempfile::tempdir().unwrap();
        let workspace = temporary.path().join("workspace");
        let incoming = temporary.path().join("synthetic-incoming");
        let source = temporary.path().join("Downloads/OpenUsage");
        prepare_directory(&workspace).unwrap();
        prepare_directory(&incoming).unwrap();
        let mut storage = Storage::create(&workspace.join("openusage.db"), &LIVE_KEY).unwrap();
        storage
            .create_profile(FIRST_ID, "Current one", FIRST_ID)
            .unwrap();
        storage
            .create_profile(SECOND_ID, "Current two", SECOND_ID)
            .unwrap();
        storage.set_theme(Theme::Dark).unwrap();
        let mut incoming_storage =
            Storage::create(&incoming.join("openusage.db"), &[17; 32]).unwrap();
        incoming_storage
            .create_profile(FIRST_ID, "Incoming one", FIRST_ID)
            .unwrap();
        let result = backup::create(
            &source,
            incoming_storage.backup_snapshot().unwrap(),
            Zeroizing::new(PASSWORD.into()),
        )
        .unwrap();
        Fixture {
            _temporary: temporary,
            workspace,
            source,
            file_name: result.file_name,
            storage,
        }
    }

    fn prepare_fixture(
        fixture: &mut Fixture,
        password: &str,
    ) -> Result<PreparedRestore, CommandError> {
        prepare(
            &mut fixture.storage,
            &fixture.workspace,
            &fixture.source,
            &fixture.file_name,
            Zeroizing::new(password.into()),
        )
    }

    #[test]
    fn preview_retains_authenticated_archive_and_checkpoints_current_encrypted_records() {
        let mut fixture = fixture();
        let before = fixture.storage.restore_state().unwrap();
        let mut prepared = prepare_fixture(&mut fixture, PASSWORD).unwrap();
        assert_eq!(prepared.preview.current.active_profiles, 2);
        assert_eq!(prepared.preview.incoming.active_profiles, 1);
        assert_eq!(prepared.preview.current.archived_profiles, "0");
        assert_eq!(prepared.preview.incoming.accounts, "0");
        assert!(!prepared.preview.restore_available);
        assert_eq!(
            before.revision,
            fixture.storage.restore_state().unwrap().revision
        );
        let checkpoint = fixture
            .workspace
            .join("restore-checkpoints")
            .join(format!("{}.sqlite", prepared.preview.checkpoint.id));
        let bytes = fs::read(&checkpoint).unwrap();
        assert!(!bytes.starts_with(b"SQLite format 3\0"));
        assert_eq!(
            prepared.preview.checkpoint.file_bytes,
            bytes.len().to_string()
        );
        assert_eq!(
            fs::metadata(&checkpoint).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            fs::metadata(checkpoint.parent().unwrap())
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        let checkpoint_storage = Storage::open(&checkpoint, &LIVE_KEY).unwrap();
        assert_eq!(checkpoint_storage.metadata().unwrap().theme, Theme::Dark);
        assert_eq!(checkpoint_storage.list_profiles().unwrap().len(), 2);
        assert_eq!(
            fs::read_dir(fixture.workspace.join("restore-staging"))
                .unwrap()
                .count(),
            0
        );
        // Source replacement cannot change the private authenticated ciphertext already retained.
        fs::write(
            fixture.source.join(&fixture.file_name),
            b"synthetic replaced source",
        )
        .unwrap();
        let mut payload = Zeroizing::new(Vec::new());
        prepared._archive.copy_payload(&mut *payload).unwrap();
        let incoming = openusage_storage::validate_native_restore_snapshot(&payload, 1).unwrap();
        assert_eq!(incoming.active_profiles, 1);
        let cache = RestoreCache::default();
        let preview = cache.install(prepared).unwrap();
        assert!(!cache.cancel(SECOND_ID).unwrap().cancelled);
        assert!(cache.cancel(&preview.preview_id).unwrap().cancelled);
        assert!(!cache.cancel(&preview.preview_id).unwrap().cancelled);
        assert!(checkpoint.exists());
        drop(checkpoint_storage);
    }

    #[test]
    fn wrong_password_tampering_and_bad_contents_never_create_a_checkpoint() {
        let mut fixture = fixture();
        let before = fixture.storage.restore_state().unwrap();
        assert_eq!(
            prepare_fixture(&mut fixture, "synthetic wrong password").err(),
            Some(CommandError::BackupAuthenticationFailed)
        );
        let path = fixture.source.join(&fixture.file_name);
        let mut original = fs::read(&path).unwrap();
        *original.last_mut().unwrap() ^= 1;
        fs::write(&path, &original).unwrap();
        assert_eq!(
            prepare_fixture(&mut fixture, PASSWORD).err(),
            Some(CommandError::BackupAuthenticationFailed)
        );
        fs::write(&path, b"malformed synthetic archive").unwrap();
        assert_eq!(
            prepare_fixture(&mut fixture, PASSWORD).err(),
            Some(CommandError::InvalidBackup)
        );
        for (schema, expected) in [
            (77, CommandError::UnsupportedBackupContents),
            (1, CommandError::InvalidBackupContents),
        ] {
            fs::remove_file(&path).unwrap();
            let payload = b"synthetic non-SQLite contents";
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&path)
                .unwrap();
            encrypt(
                &Manifest {
                    schema_version: schema,
                    entries: vec![Entry {
                        name: "records.sqlite".into(),
                        size: payload.len() as u64,
                    }],
                },
                &mut Cursor::new(payload),
                &mut file,
                PASSWORD.as_bytes(),
                available_space(&fixture.source).unwrap(),
            )
            .unwrap();
            drop(file);
            assert_eq!(
                prepare_fixture(&mut fixture, PASSWORD).err(),
                Some(expected)
            );
        }
        assert!(!fixture.workspace.join("restore-checkpoints").exists());
        assert_eq!(
            before.revision,
            fixture.storage.restore_state().unwrap().revision
        );
        assert_eq!(fixture.storage.list_profiles().unwrap().len(), 2);
        assert_eq!(
            fs::read_dir(fixture.workspace.join("restore-staging"))
                .unwrap()
                .count(),
            0
        );
    }

    #[test]
    fn checkpoint_path_failure_preserves_current_workspace_and_closes_staging() {
        let mut fixture = fixture();
        let before = fixture.storage.restore_state().unwrap();
        let outside = fixture._temporary.path().join("synthetic-outside");
        prepare_directory(&outside).unwrap();
        symlink(&outside, fixture.workspace.join("restore-checkpoints")).unwrap();
        assert_eq!(
            prepare_fixture(&mut fixture, PASSWORD).err(),
            Some(CommandError::RestoreCheckpointFailed)
        );
        assert_eq!(fs::read_dir(outside).unwrap().count(), 0);
        assert_eq!(
            before.revision,
            fixture.storage.restore_state().unwrap().revision
        );
        assert_eq!(
            fs::read_dir(fixture.workspace.join("restore-staging"))
                .unwrap()
                .count(),
            0
        );
    }

    #[test]
    fn exit_cleanup_drops_cached_archive_and_rejects_a_late_worker_result() {
        let mut fixture = fixture();
        let prepared = prepare_fixture(&mut fixture, PASSWORD).unwrap();
        let cache = RestoreCache::default();
        let preview = cache.install(prepared).unwrap();
        cache.close();
        assert!(!cache.cancel(&preview.preview_id).unwrap().cancelled);
        assert_eq!(cache.require_open(), Err(CommandError::StorageUnavailable));
        assert!(cache.0.lock().unwrap().prepared.is_none());
        // Simulate a worker that authenticated before the exit callback closed its cache.
        let late = prepare_fixture(&mut fixture, PASSWORD).unwrap();
        assert_eq!(cache.install(late), Err(CommandError::StorageUnavailable));
        assert!(cache.0.lock().unwrap().prepared.is_none());
    }

    #[test]
    fn space_revision_and_publication_guards_fail_closed() {
        assert_eq!(
            checkpoint_budget(u64::MAX),
            Err(CommandError::InsufficientSpace)
        );
        assert_eq!(require_space(9, 5, 5), Err(CommandError::InsufficientSpace));
        assert_eq!(
            require_space(u64::MAX, u64::MAX, 1),
            Err(CommandError::InsufficientSpace)
        );
        assert_eq!(require_space(10, 5, 5), Ok(()));
        let temporary = tempfile::tempdir().unwrap();
        let workspace = temporary.path().join("workspace");
        prepare_directory(&workspace).unwrap();
        let mut storage = Storage::create(&workspace.join("openusage.db"), &LIVE_KEY).unwrap();
        let before = storage.restore_state().unwrap();
        storage.set_theme(Theme::Light).unwrap();
        assert_eq!(
            require_unchanged(&before, &mut storage),
            Err(CommandError::StaleRestorePreview)
        );
        let pending = workspace.join("synthetic.pending");
        let existing = workspace.join("synthetic.sqlite");
        for (path, contents) in [
            (&pending, b"pending".as_slice()),
            (&existing, b"existing".as_slice()),
        ] {
            use std::io::Write;
            OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(path)
                .unwrap()
                .write_all(contents)
                .unwrap();
        }
        let mut created = CreatedCheckpoint::open(pending.clone()).unwrap();
        assert_eq!(
            created.publish(&existing, &workspace),
            Err(CommandError::RestoreCheckpointFailed)
        );
        drop(created);
        assert!(!pending.exists());
        assert_eq!(fs::read(existing).unwrap(), b"existing");
        for invalid in ["", "../checkpoint", "00000000-0000-4000-8000-00000000000A"] {
            assert_eq!(
                validate_preview_id(invalid),
                Err(CommandError::InvalidArguments)
            );
        }
    }
}
