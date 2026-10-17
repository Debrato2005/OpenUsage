//! Encrypted, consistent checkpoints for a later restore workflow.
//! The caller selects an approved local directory outside cloud-sync folders and checks free space.
//! This API does not publish a checkpoint, approve a restore, or replace records.

use crate::{CURRENT_SCHEMA_VERSION, Storage, validate_path};
use rusqlite::Connection;
use std::{
    fmt,
    fs::{self, File, OpenOptions},
    io,
    path::Path,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckpointMetadata {
    pub schema_version: u32,
    pub file_bytes: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CheckpointError {
    InvalidPath,
    AlreadyExists,
    UnsupportedSchema,
    Unavailable,
    VerificationFailed,
}

impl fmt::Display for CheckpointError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidPath => "Use a new checkpoint file in an approved private directory.",
            Self::AlreadyExists => "The checkpoint destination already exists.",
            Self::UnsupportedSchema => "This database schema does not support checkpoints.",
            Self::Unavailable => "The encrypted checkpoint could not be saved.",
            Self::VerificationFailed => "The encrypted checkpoint did not pass verification.",
        })
    }
}

impl std::error::Error for CheckpointError {}

impl Storage {
    /// The caller owns local-filesystem approval, disk-space checks, and the unique pending filename.
    /// Existing key protection must remain available to reopen this encrypted checkpoint.
    /// Windows requires a separate ACL review and returns Unavailable before file creation.
    /// A detach or filesystem failure can leave an incomplete private file. Publish only an Ok result.
    pub fn create_encrypted_checkpoint(
        &mut self,
        destination: &Path,
    ) -> Result<CheckpointMetadata, CheckpointError> {
        if !cfg!(unix) {
            return Err(CheckpointError::Unavailable);
        }
        validate_path(destination).map_err(|_| CheckpointError::InvalidPath)?;
        let name = destination
            .to_str()
            .filter(|name| !name.contains('\0'))
            .ok_or(CheckpointError::InvalidPath)?;
        // A stale sidecar can belong to another database. Never ask SQLite to open one.
        require_no_sidecars(destination)?;
        let parent = destination.parent().ok_or(CheckpointError::InvalidPath)?;
        let directory = File::open(parent).map_err(|_| CheckpointError::InvalidPath)?;
        let mut options = OpenOptions::new();
        options.read(true).write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let file = options.open(destination).map_err(|error| {
            if error.kind() == io::ErrorKind::AlreadyExists {
                CheckpointError::AlreadyExists
            } else {
                CheckpointError::Unavailable
            }
        })?;
        let mut created = CreatedCheckpoint {
            destination,
            file,
            directory,
            keep: false,
        };
        let mut attached = false;
        let result = (|| {
            created.require_identity()?;
            require_no_sidecars(destination)?;
            // SQLCipher inherits the main raw key and salt internally when KEY is omitted.
            // The connection's NOFOLLOW open flag also applies to ATTACH.
            self.connection
                .execute("ATTACH DATABASE ?1 AS openusage_checkpoint", [name])
                .map_err(|_| CheckpointError::Unavailable)?;
            attached = true;
            created.require_identity()?;
            configure_checkpoint(&self.connection)?;
            let transaction = self
                .connection
                .transaction()
                .map_err(|_| CheckpointError::Unavailable)?;
            let schema_version: u32 = transaction
                .pragma_query_value(Some("main"), "user_version", |row| row.get(0))
                .map_err(|_| CheckpointError::VerificationFailed)?;
            if schema_version != CURRENT_SCHEMA_VERSION {
                return Err(CheckpointError::UnsupportedSchema);
            }
            transaction
                .execute_batch("SELECT sqlcipher_export('openusage_checkpoint','main');")
                .map_err(|_| CheckpointError::Unavailable)?;
            transaction
                .pragma_update(Some("openusage_checkpoint"), "user_version", schema_version)
                .map_err(|_| CheckpointError::Unavailable)?;
            transaction
                .commit()
                .map_err(|_| CheckpointError::Unavailable)?;
            // The HMAC pragma reads disk pages, so run it only after the target transaction commits.
            verify_checkpoint(&self.connection, schema_version)?;
            Ok(schema_version)
        })();
        if attached
            && self
                .connection
                .execute_batch("DETACH DATABASE openusage_checkpoint")
                .is_err()
        {
            // Retain the private pending file if SQLite still owns it. Never unlink an attached file.
            created.keep = true;
            return Err(CheckpointError::Unavailable);
        }
        let result = result.and_then(|schema_version| {
            created.require_identity()?;
            require_no_sidecars(destination)?;
            created
                .file
                .sync_all()
                .map_err(|_| CheckpointError::Unavailable)?;
            created
                .directory
                .sync_all()
                .map_err(|_| CheckpointError::Unavailable)?;
            let file_bytes = created
                .file
                .metadata()
                .map_err(|_| CheckpointError::Unavailable)?
                .len();
            if file_bytes == 0 {
                return Err(CheckpointError::VerificationFailed);
            }
            Ok(CheckpointMetadata {
                schema_version,
                file_bytes,
            })
        });
        match result {
            Ok(metadata) => {
                created.keep = true;
                Ok(metadata)
            }
            Err(error) => {
                created.remove_owned()?;
                Err(error)
            }
        }
    }
}

fn configure_checkpoint(connection: &Connection) -> Result<(), CheckpointError> {
    let journal: String = connection
        .query_row(
            "PRAGMA openusage_checkpoint.journal_mode=DELETE",
            [],
            |row| row.get(0),
        )
        .map_err(|_| CheckpointError::Unavailable)?;
    connection
        .pragma_update(Some("openusage_checkpoint"), "synchronous", "FULL")
        .map_err(|_| CheckpointError::Unavailable)?;
    let synchronous: i64 = connection
        .pragma_query_value(Some("openusage_checkpoint"), "synchronous", |row| {
            row.get(0)
        })
        .map_err(|_| CheckpointError::Unavailable)?;
    if journal != "delete" || synchronous != 2 {
        return Err(CheckpointError::VerificationFailed);
    }
    require_encryption(connection)
}

fn require_encryption(connection: &Connection) -> Result<(), CheckpointError> {
    for (pragma, expected) in [
        ("cipher_status", "1"),
        ("cipher_use_hmac", "1"),
        ("cipher_plaintext_header_size", "0"),
    ] {
        let value: String = connection
            .pragma_query_value(Some("openusage_checkpoint"), pragma, |row| row.get(0))
            .map_err(|_| CheckpointError::VerificationFailed)?;
        if value != expected {
            return Err(CheckpointError::VerificationFailed);
        }
    }
    Ok(())
}

fn verify_checkpoint(connection: &Connection, schema_version: u32) -> Result<(), CheckpointError> {
    require_encryption(connection)?;
    let version: u32 = connection
        .pragma_query_value(Some("openusage_checkpoint"), "user_version", |row| {
            row.get(0)
        })
        .map_err(|_| CheckpointError::VerificationFailed)?;
    let foreign_keys: bool = connection
        .pragma_query_value(None, "foreign_keys", |row| row.get(0))
        .map_err(|_| CheckpointError::VerificationFailed)?;
    let integrity: String = connection
        .query_row(
            "PRAGMA openusage_checkpoint.integrity_check(1)",
            [],
            |row| row.get(0),
        )
        .map_err(|_| CheckpointError::VerificationFailed)?;
    if version != schema_version || !foreign_keys || integrity != "ok" {
        return Err(CheckpointError::VerificationFailed);
    }
    for pragma in [
        "PRAGMA openusage_checkpoint.cipher_integrity_check",
        "PRAGMA openusage_checkpoint.foreign_key_check",
    ] {
        let mut statement = connection
            .prepare(pragma)
            .map_err(|_| CheckpointError::VerificationFailed)?;
        if statement
            .query([])
            .map_err(|_| CheckpointError::VerificationFailed)?
            .next()
            .map_err(|_| CheckpointError::VerificationFailed)?
            .is_some()
        {
            return Err(CheckpointError::VerificationFailed);
        }
    }
    Ok(())
}

fn require_no_sidecars(destination: &Path) -> Result<(), CheckpointError> {
    for suffix in ["-wal", "-shm", "-journal"] {
        let mut name = destination.as_os_str().to_owned();
        name.push(suffix);
        match fs::symlink_metadata(name) {
            Ok(_) => return Err(CheckpointError::AlreadyExists),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(_) => return Err(CheckpointError::InvalidPath),
        }
    }
    Ok(())
}

struct CreatedCheckpoint<'path> {
    destination: &'path Path,
    file: File,
    directory: File,
    keep: bool,
}

impl CreatedCheckpoint<'_> {
    fn require_identity(&self) -> Result<(), CheckpointError> {
        self.require_owned_path()?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let actual = self
                .file
                .metadata()
                .map_err(|_| CheckpointError::InvalidPath)?;
            let parent = self
                .directory
                .metadata()
                .map_err(|_| CheckpointError::InvalidPath)?;
            if actual.nlink() != 1
                || actual.mode() & 0o777 != 0o600
                || parent.mode() & 0o777 != 0o700
                || parent.uid() != actual.uid()
            {
                return Err(CheckpointError::InvalidPath);
            }
        }
        Ok(())
    }

    fn require_owned_path(&self) -> Result<(), CheckpointError> {
        let actual =
            fs::symlink_metadata(self.destination).map_err(|_| CheckpointError::InvalidPath)?;
        let parent = fs::symlink_metadata(
            self.destination
                .parent()
                .ok_or(CheckpointError::InvalidPath)?,
        )
        .map_err(|_| CheckpointError::InvalidPath)?;
        if !actual.is_file() || !parent.is_dir() || parent.file_type().is_symlink() {
            return Err(CheckpointError::InvalidPath);
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let original = self
                .file
                .metadata()
                .map_err(|_| CheckpointError::InvalidPath)?;
            let directory = self
                .directory
                .metadata()
                .map_err(|_| CheckpointError::InvalidPath)?;
            if actual.dev() != original.dev()
                || actual.ino() != original.ino()
                || parent.dev() != directory.dev()
                || parent.ino() != directory.ino()
            {
                return Err(CheckpointError::InvalidPath);
            }
        }
        Ok(())
    }

    fn remove_owned(&mut self) -> Result<(), CheckpointError> {
        self.require_owned_path()?;
        fs::remove_file(self.destination).map_err(|_| CheckpointError::Unavailable)?;
        self.keep = true;
        self.directory
            .sync_all()
            .map_err(|_| CheckpointError::Unavailable)
    }
}

impl Drop for CreatedCheckpoint<'_> {
    fn drop(&mut self) {
        if !self.keep {
            let _ = self.remove_owned();
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::{Storage, Theme};
    use std::os::unix::fs::{PermissionsExt, symlink};

    const KEY: [u8; 32] = [0x58; 32];

    #[test]
    fn encrypted_checkpoint_captures_active_wal_and_reopens_with_the_existing_key() {
        let temporary = crate::tests::private_directory();
        let live = temporary.path().join("live.db");
        let checkpoint = temporary.path().join("checkpoint.pending");
        let mut storage = Storage::create(&live, &KEY).unwrap();
        storage
            .connection
            .pragma_update(None, "wal_autocheckpoint", 0)
            .unwrap();
        storage.set_theme(Theme::Dark).unwrap();
        storage
            .create_profile(
                "00000001-0000-4000-8000-000000000000",
                "Synthetic checkpoint profile",
                "00000001-0000-4000-8000-000000000000",
            )
            .unwrap();
        assert!(live.with_extension("db-wal").metadata().unwrap().len() > 0);
        let before = storage.metadata().unwrap();
        let metadata = storage.create_encrypted_checkpoint(&checkpoint).unwrap();
        assert_eq!(metadata.schema_version, CURRENT_SCHEMA_VERSION);
        assert_eq!(metadata.file_bytes, checkpoint.metadata().unwrap().len());
        assert_eq!(
            checkpoint.metadata().unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert!(
            !fs::read(&checkpoint)
                .unwrap()
                .starts_with(b"SQLite format 3\0")
        );
        let unkeyed = Connection::open(&checkpoint).unwrap();
        assert!(
            unkeyed
                .query_row("SELECT count(*) FROM sqlite_schema", [], |row| row
                    .get::<_, i64>(0))
                .is_err()
        );
        drop(unkeyed);
        assert_eq!(storage.metadata().unwrap(), before);
        storage.set_theme(Theme::Light).unwrap();
        let restored = Storage::open(&checkpoint, &KEY).unwrap();
        assert_eq!(restored.metadata().unwrap(), before);
        assert_eq!(
            restored.list_profiles().unwrap()[0].label,
            "Synthetic checkpoint profile"
        );
        assert_eq!(storage.metadata().unwrap().theme, Theme::Light);
    }

    #[test]
    fn preexisting_files_links_and_sidecars_are_preserved() {
        let temporary = crate::tests::private_directory();
        let mut storage = Storage::create(&temporary.path().join("live.db"), &KEY).unwrap();
        let existing = temporary.path().join("existing");
        fs::write(&existing, b"synthetic existing marker").unwrap();
        assert_eq!(
            storage.create_encrypted_checkpoint(&existing),
            Err(CheckpointError::AlreadyExists)
        );
        let linked = temporary.path().join("linked");
        symlink(&existing, &linked).unwrap();
        assert_eq!(
            storage.create_encrypted_checkpoint(&linked),
            Err(CheckpointError::InvalidPath)
        );
        assert!(
            fs::symlink_metadata(&linked)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        let hardlink = temporary.path().join("hardlink");
        fs::hard_link(&existing, &hardlink).unwrap();
        assert_eq!(
            storage.create_encrypted_checkpoint(&hardlink),
            Err(CheckpointError::AlreadyExists)
        );
        assert_eq!(fs::read(&hardlink).unwrap(), b"synthetic existing marker");
        assert_eq!(fs::read(existing).unwrap(), b"synthetic existing marker");
        let target = temporary.path().join("pending");
        let journal = temporary.path().join("pending-journal");
        fs::write(&journal, b"synthetic unrelated journal").unwrap();
        assert_eq!(
            storage.create_encrypted_checkpoint(&target),
            Err(CheckpointError::AlreadyExists)
        );
        assert!(!target.exists());
        assert_eq!(fs::read(journal).unwrap(), b"synthetic unrelated journal");
    }

    #[test]
    fn failed_export_removes_only_the_new_file_and_allows_retry() {
        let temporary = crate::tests::private_directory();
        let mut storage = Storage::create(&temporary.path().join("live.db"), &KEY).unwrap();
        let target = temporary.path().join("pending");
        let sentinel = temporary.path().join("preserve");
        fs::write(&sentinel, b"synthetic preserved marker").unwrap();
        storage
            .connection
            .pragma_update(None, "query_only", true)
            .unwrap();
        assert!(storage.create_encrypted_checkpoint(&target).is_err());
        assert!(!target.exists());
        require_no_sidecars(&target).unwrap();
        assert_eq!(fs::read(sentinel).unwrap(), b"synthetic preserved marker");
        assert_eq!(storage.metadata().unwrap().profile_count, 0);
        storage
            .connection
            .pragma_update(None, "query_only", false)
            .unwrap();
        storage.create_encrypted_checkpoint(&target).unwrap();
        assert!(target.exists());
    }

    #[test]
    fn page_hmac_checks_reject_disk_corruption_with_warm_sqlite_pages() {
        use std::io::{Read, Seek, SeekFrom, Write};
        let temporary = crate::tests::private_directory();
        let mut storage = Storage::create(&temporary.path().join("live.db"), &KEY).unwrap();
        let target = temporary.path().join("checkpoint");
        storage.create_encrypted_checkpoint(&target).unwrap();
        storage
            .connection
            .execute(
                "ATTACH DATABASE ?1 AS openusage_checkpoint",
                [target.to_str().unwrap()],
            )
            .unwrap();
        storage.connection.execute_batch("BEGIN").unwrap();
        verify_checkpoint(&storage.connection, CURRENT_SCHEMA_VERSION).unwrap();
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&target)
            .unwrap();
        let mut byte = [0];
        file.seek(SeekFrom::Start(100)).unwrap();
        file.read_exact(&mut byte).unwrap();
        byte[0] ^= 1;
        file.seek(SeekFrom::Start(100)).unwrap();
        file.write_all(&byte).unwrap();
        file.sync_all().unwrap();
        let cached_integrity: String = storage
            .connection
            .query_row(
                "PRAGMA openusage_checkpoint.integrity_check(1)",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(cached_integrity, "ok");
        assert_eq!(
            verify_checkpoint(&storage.connection, CURRENT_SCHEMA_VERSION),
            Err(CheckpointError::VerificationFailed)
        );
        storage
            .connection
            .execute_batch("ROLLBACK; DETACH DATABASE openusage_checkpoint")
            .unwrap();
        storage.set_theme(Theme::Dark).unwrap();
        assert_eq!(storage.metadata().unwrap().theme, Theme::Dark);
    }
}
