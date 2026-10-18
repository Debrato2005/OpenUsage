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
        require_empty_pragma(connection, pragma)?;
    }
    Ok(())
}

fn require_empty_pragma(connection: &Connection, pragma: &str) -> Result<(), CheckpointError> {
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckpointVerification {
    pub schema_version: u32,
    pub file_bytes: u64,
    pub summary: crate::BackupContentSummary,
}

impl Storage {
    /// Only closed, private, standalone checkpoints can use immutable mode.
    /// Verification streams encrypted pages. It does not allocate a full database image.
    pub fn verify_encrypted_checkpoint(
        &mut self,
        path: &Path,
    ) -> Result<CheckpointVerification, CheckpointError> {
        self.with_verified_checkpoint(path, |_, verified| Ok(verified))
    }
    /// Recovery copies plaintext only into bounded Rust/SQLite memory. Large-history RAM remains unqualified.
    pub fn read_encrypted_checkpoint(
        &mut self,
        path: &Path,
    ) -> Result<crate::BackupSnapshot, CheckpointError> {
        self.with_verified_checkpoint(path, |connection, verified| {
            let read_only: bool = connection
                .pragma_query_value(None, "query_only", |r| r.get(0))
                .map_err(|_| CheckpointError::Unavailable)?;
            if read_only {
                return Err(CheckpointError::Unavailable);
            }
            // Build from trusted schema and bound known fields, never source catalog SQL.
            let copy = crate::backup_validation::trusted_memory_copy(
                connection,
                "openusage_checkpoint",
                verified.schema_version,
            )
            .map_err(|_| CheckpointError::VerificationFailed)?;
            let bytes = (|| {
                let serialized = copy
                    .serialize("main")
                    .map_err(|_| CheckpointError::Unavailable)?;
                if serialized.len() as u64 > crate::MAX_BACKUP_SNAPSHOT_SIZE {
                    return Err(CheckpointError::Unavailable);
                }
                let mut bytes = zeroize::Zeroizing::new(Vec::new());
                bytes
                    .try_reserve_exact(serialized.len())
                    .map_err(|_| CheckpointError::Unavailable)?;
                bytes.extend_from_slice(&serialized);
                Ok(bytes)
            })();
            copy.close().map_err(|_| CheckpointError::Unavailable)?;
            Ok(crate::BackupSnapshot {
                schema_version: verified.schema_version,
                bytes: bytes?,
            })
        })
    }

    fn with_verified_checkpoint<T>(
        &mut self,
        path: &Path,
        operation: impl FnOnce(&Connection, CheckpointVerification) -> Result<T, CheckpointError>,
    ) -> Result<T, CheckpointError> {
        #[cfg(not(unix))]
        {
            let _ = (path, operation);
            Err(CheckpointError::Unavailable)
        }
        #[cfg(unix)]
        {
            let held = ReadCheckpoint::open(path)?;
            let uri = immutable_uri(path)?;
            self.connection
                .execute("ATTACH DATABASE ?1 AS openusage_checkpoint", [uri])
                .map_err(|_| CheckpointError::VerificationFailed)?;
            let result = (|| {
                held.require_identity()?;
                require_attached_identity(&self.connection)?;
                let status = unsafe {
                    rusqlite::ffi::sqlite3_db_readonly(
                        self.connection.handle(),
                        c"openusage_checkpoint".as_ptr(),
                    )
                };
                if status != 1 {
                    return Err(CheckpointError::VerificationFailed);
                }
                let mut options = ValidationOptions::new(&self.connection)?;
                let checked = (|| {
                    require_encryption(&self.connection)?;
                    require_empty_pragma(
                        &self.connection,
                        "PRAGMA openusage_checkpoint.cipher_integrity_check",
                    )?;
                    let schema_version: u32 = self
                        .connection
                        .pragma_query_value(Some("openusage_checkpoint"), "user_version", |r| {
                            r.get(0)
                        })
                        .map_err(|_| CheckpointError::VerificationFailed)?;
                    if !crate::backup_validation::supported_schema_version(schema_version) {
                        return Err(CheckpointError::UnsupportedSchema);
                    }
                    let summary = crate::backup_validation::validate_schema_connection(
                        &self.connection,
                        schema_version,
                        "openusage_checkpoint",
                    )
                    .map_err(|error| match error {
                        crate::BackupValidationError::UnsupportedSchema => {
                            CheckpointError::UnsupportedSchema
                        }
                        _ => CheckpointError::VerificationFailed,
                    })?;
                    held.require_identity()?;
                    require_attached_identity(&self.connection)?;
                    Ok(CheckpointVerification {
                        schema_version,
                        file_bytes: held
                            .file
                            .metadata()
                            .map_err(|_| CheckpointError::InvalidPath)?
                            .len(),
                        summary,
                    })
                })();
                options.restore()?;
                let verified = checked?;
                let result = operation(&self.connection, verified);
                held.require_identity()?;
                require_attached_identity(&self.connection)?;
                result
            })();
            self.connection
                .execute_batch("DETACH DATABASE openusage_checkpoint")
                .map_err(|_| CheckpointError::Unavailable)?;
            require_no_sidecars(path)?;
            result
        }
    }
}

#[cfg(unix)]
fn immutable_uri(path: &Path) -> Result<String, CheckpointError> {
    use std::os::unix::ffi::OsStrExt;
    let mut uri = String::from("file:");
    for byte in path.as_os_str().as_bytes() {
        use std::fmt::Write;
        if byte.is_ascii_alphanumeric() || b"/-_.~".contains(byte) {
            uri.push(char::from(*byte));
        } else {
            write!(&mut uri, "%{byte:02X}").map_err(|_| CheckpointError::InvalidPath)?;
        }
    }
    uri.push_str("?mode=ro&immutable=1");
    Ok(uri)
}
#[cfg(unix)]
fn require_attached_identity(connection: &Connection) -> Result<(), CheckpointError> {
    let mut moved: i32 = 1;
    let status = unsafe {
        rusqlite::ffi::sqlite3_file_control(
            connection.handle(),
            c"openusage_checkpoint".as_ptr(),
            rusqlite::ffi::SQLITE_FCNTL_HAS_MOVED,
            (&mut moved as *mut i32).cast(),
        )
    };
    if status != rusqlite::ffi::SQLITE_OK || moved != 0 {
        return Err(CheckpointError::InvalidPath);
    }
    Ok(())
}
#[cfg(unix)]
struct ReadCheckpoint {
    path: std::path::PathBuf,
    file: File,
    directory: File,
    original: fs::Metadata,
}
#[cfg(unix)]
impl ReadCheckpoint {
    fn open(path: &Path) -> Result<Self, CheckpointError> {
        use std::os::unix::fs::OpenOptionsExt;
        validate_path(path).map_err(|_| CheckpointError::InvalidPath)?;
        require_no_sidecars(path)?;
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(path)
            .map_err(|_| CheckpointError::InvalidPath)?;
        let directory = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_DIRECTORY)
            .open(path.parent().ok_or(CheckpointError::InvalidPath)?)
            .map_err(|_| CheckpointError::InvalidPath)?;
        let original = file.metadata().map_err(|_| CheckpointError::InvalidPath)?;
        let held = Self {
            path: path.to_owned(),
            file,
            directory,
            original,
        };
        held.require_identity()?;
        Ok(held)
    }
    fn require_identity(&self) -> Result<(), CheckpointError> {
        use std::os::unix::fs::MetadataExt;
        let actual = fs::symlink_metadata(&self.path).map_err(|_| CheckpointError::InvalidPath)?;
        let held = self
            .file
            .metadata()
            .map_err(|_| CheckpointError::InvalidPath)?;
        let parent = fs::symlink_metadata(self.path.parent().ok_or(CheckpointError::InvalidPath)?)
            .map_err(|_| CheckpointError::InvalidPath)?;
        let directory = self
            .directory
            .metadata()
            .map_err(|_| CheckpointError::InvalidPath)?;
        if !actual.is_file()
            || !parent.is_dir()
            || parent.file_type().is_symlink()
            || actual.dev() != held.dev()
            || actual.ino() != held.ino()
            || held.len() != self.original.len()
            || held.mtime() != self.original.mtime()
            || held.mtime_nsec() != self.original.mtime_nsec()
            || held.ctime() != self.original.ctime()
            || held.ctime_nsec() != self.original.ctime_nsec()
            || parent.dev() != directory.dev()
            || parent.ino() != directory.ino()
            || held.uid() != unsafe { libc::geteuid() }
            || directory.uid() != held.uid()
            || held.mode() & 0o777 != 0o600
            || directory.mode() & 0o777 != 0o700
            || held.nlink() != 1
            || held.len() == 0
        {
            return Err(CheckpointError::InvalidPath);
        }
        require_no_sidecars(&self.path)
    }
}

#[cfg(all(test, unix))]
mod readonly_tests {
    use super::*;
    use crate::{Theme, validate_checkpoint_restore_input};
    const KEY: [u8; 32] = [0x79; 32];
    #[test]
    fn verification_and_recovery_preserve_checkpoint_bytes_and_create_no_sidecars() {
        let directory = crate::tests::private_directory();
        let path = directory.path().join("checkpoint ?#%ü");
        let mut live = Storage::create(&directory.path().join("live"), &KEY).unwrap();
        live.set_theme(Theme::Dark).unwrap();
        live.create_encrypted_checkpoint(&path).unwrap();
        let original = fs::read(&path).unwrap();
        let revision = live.restore_state().unwrap().revision;
        let verified = live.verify_encrypted_checkpoint(&path).unwrap();
        assert_eq!(verified.file_bytes, original.len() as u64);
        live.connection
            .pragma_update(None, "query_only", true)
            .unwrap();
        assert!(live.read_encrypted_checkpoint(&path).is_err());
        assert!(
            live.connection
                .pragma_query_value(None, "query_only", |r| r.get::<_, bool>(0))
                .unwrap()
        );
        live.connection
            .pragma_update(None, "query_only", false)
            .unwrap();
        let snapshot = live.read_encrypted_checkpoint(&path).unwrap();
        assert!(snapshot.bytes.starts_with(b"SQLite format 3\0"));
        assert_eq!(
            validate_checkpoint_restore_input(&snapshot.bytes, snapshot.schema_version)
                .unwrap()
                .summary,
            verified.summary
        );
        assert_eq!(fs::read(&path).unwrap(), original);
        require_no_sidecars(&path).unwrap();
        assert_eq!(live.restore_state().unwrap().revision, revision);
        live.set_theme(Theme::Light).unwrap();
        assert_eq!(live.metadata().unwrap().theme, Theme::Light);
    }
    #[test]
    fn verification_rejects_corruption_links_and_sidecars_and_restores_query_only() {
        use std::os::unix::fs::symlink;
        let directory = crate::tests::private_directory();
        let path = directory.path().join("checkpoint");
        let mut live = Storage::create(&directory.path().join("live"), &KEY).unwrap();
        live.create_encrypted_checkpoint(&path).unwrap();
        let link = directory.path().join("link");
        symlink(&path, &link).unwrap();
        assert!(live.verify_encrypted_checkpoint(&link).is_err());
        let hardlink = directory.path().join("hardlink");
        fs::hard_link(&path, &hardlink).unwrap();
        assert!(live.verify_encrypted_checkpoint(&path).is_err());
        fs::remove_file(hardlink).unwrap();
        fs::write(directory.path().join("checkpoint-wal"), b"preserved").unwrap();
        assert!(live.verify_encrypted_checkpoint(&path).is_err());
        assert_eq!(
            fs::read(directory.path().join("checkpoint-wal")).unwrap(),
            b"preserved"
        );
        fs::remove_file(directory.path().join("checkpoint-wal")).unwrap();
        let mut bytes = fs::read(&path).unwrap();
        bytes[100] ^= 1;
        fs::write(&path, bytes).unwrap();
        assert_eq!(
            live.verify_encrypted_checkpoint(&path),
            Err(CheckpointError::VerificationFailed)
        );
        assert!(
            !live
                .connection
                .pragma_query_value(None, "query_only", |r| r.get::<_, bool>(0))
                .unwrap()
        );
        live.set_theme(Theme::Dark).unwrap();
    }
}

#[cfg(unix)]
struct ValidationOptions<'a> {
    connection: &'a Connection,
    configs: Vec<(rusqlite::config::DbConfig, bool)>,
    query_only: bool,
    active: bool,
}
#[cfg(unix)]
impl<'a> ValidationOptions<'a> {
    fn new(connection: &'a Connection) -> Result<Self, CheckpointError> {
        use rusqlite::config::DbConfig;
        let query_only = connection
            .pragma_query_value(None, "query_only", |r| r.get(0))
            .map_err(|_| CheckpointError::Unavailable)?;
        let mut options = Self {
            connection,
            configs: Vec::new(),
            query_only,
            active: true,
        };
        for (config, enabled) in [
            (DbConfig::SQLITE_DBCONFIG_DEFENSIVE, true),
            (DbConfig::SQLITE_DBCONFIG_TRUSTED_SCHEMA, false),
            (DbConfig::SQLITE_DBCONFIG_ENABLE_TRIGGER, false),
            (DbConfig::SQLITE_DBCONFIG_ENABLE_VIEW, false),
        ] {
            let previous = connection
                .db_config(config)
                .map_err(|_| CheckpointError::Unavailable)?;
            options.configs.push((config, previous));
            if connection
                .set_db_config(config, enabled)
                .map_err(|_| CheckpointError::Unavailable)?
                != enabled
            {
                return Err(CheckpointError::Unavailable);
            }
        }
        connection
            .pragma_update(None, "query_only", true)
            .map_err(|_| CheckpointError::Unavailable)?;
        let started = std::time::Instant::now();
        connection
            .progress_handler(
                10_000,
                Some(move || started.elapsed() > std::time::Duration::from_secs(60)),
            )
            .map_err(|_| CheckpointError::Unavailable)?;
        Ok(options)
    }
    fn restore(&mut self) -> Result<(), CheckpointError> {
        let mut failed = false;
        failed |= self
            .connection
            .progress_handler(0, None::<fn() -> bool>)
            .is_err();
        failed |= self
            .connection
            .pragma_update(None, "query_only", self.query_only)
            .is_err();
        for (config, enabled) in self.configs.iter().rev() {
            failed |= self.connection.set_db_config(*config, *enabled).is_err();
        }
        self.active = failed;
        if failed {
            Err(CheckpointError::Unavailable)
        } else {
            Ok(())
        }
    }
}
#[cfg(unix)]
impl Drop for ValidationOptions<'_> {
    fn drop(&mut self) {
        if self.active {
            let _ = self.restore();
        }
    }
}

#[cfg(all(test, unix))]
mod recovery_tests {
    use crate::{RestoreHomeMapping, Storage, Theme, validate_checkpoint_restore_input};
    const KEY: [u8; 32] = [0x32; 32];
    #[test]
    fn original_encrypted_checkpoint_reverses_a_restore_after_reopen() {
        let directory = crate::tests::private_directory();
        let live_path = directory.path().join("live");
        let checkpoint = directory.path().join("checkpoint");
        let original_id = "00000001-0000-4000-8000-000000000000";
        let mut live = Storage::create(&live_path, &KEY).unwrap();
        live.create_profile(original_id, "Original checkpoint records", original_id)
            .unwrap();
        live.set_theme(Theme::Dark).unwrap();
        live.create_encrypted_checkpoint(&checkpoint).unwrap();
        let original_file = std::fs::read(&checkpoint).unwrap();
        let mut incoming = Storage::create(&directory.path().join("incoming"), &KEY).unwrap();
        incoming.set_theme(Theme::Light).unwrap();
        incoming
            .create_profile(original_id, "Incoming records", original_id)
            .unwrap();
        let archive = incoming.backup_snapshot().unwrap();
        let revision = live.restore_state().unwrap().revision;
        live.replace_records_from_backup(
            &archive.bytes,
            archive.schema_version,
            &revision,
            &[RestoreHomeMapping {
                profile_id: original_id.into(),
                managed_home_reference: "00000002-0000-4000-8000-000000000000".into(),
            }],
        )
        .unwrap();
        drop(live);
        let mut live = Storage::open(&live_path, &KEY).unwrap();
        assert_eq!(live.metadata().unwrap().theme, Theme::Light);
        let saved = live.read_encrypted_checkpoint(&checkpoint).unwrap();
        let metadata =
            validate_checkpoint_restore_input(&saved.bytes, saved.schema_version).unwrap();
        let revision = live.restore_state().unwrap().revision;
        live.replace_records_from_checkpoint(
            &saved.bytes,
            saved.schema_version,
            &revision,
            &metadata.homes,
        )
        .unwrap();
        assert_eq!(live.metadata().unwrap().theme, Theme::Dark);
        assert_eq!(
            live.list_profiles().unwrap()[0].label,
            "Original checkpoint records"
        );
        let recovered_revision = live.restore_state().unwrap().revision;
        assert_eq!(
            live.restore_home_mappings(&recovered_revision).unwrap()[0].managed_home_reference,
            original_id
        );
        assert_eq!(std::fs::read(&checkpoint).unwrap(), original_file);
        super::require_no_sidecars(&checkpoint).unwrap();
    }
    #[test]
    fn legacy_encrypted_checkpoint_keeps_original_version_bytes_and_needs_no_disk_migration() {
        use super::{CURRENT_SCHEMA_VERSION, require_no_sidecars};
        use std::fs;
        let directory = crate::tests::private_directory();
        let path = directory.path().join("legacy-checkpoint");
        let mut live = Storage::create(&directory.path().join("live-current"), &KEY).unwrap();
        live.create_profile(
            "00000001-0000-4000-8000-000000000000",
            "Legacy synthetic",
            "00000002-0000-4000-8000-000000000000",
        )
        .unwrap();
        live.create_encrypted_checkpoint(&path).unwrap();
        // A historical checkpoint shares this workspace's inherited SQLCipher key/salt.
        // Only trusted fixture DDL changes the checkpoint to the exact original v1 catalog.
        live.connection
            .execute(
                "ATTACH DATABASE ?1 AS old_fixture",
                [path.to_str().unwrap()],
            )
            .unwrap();
        live.connection.execute_batch("DROP TABLE old_fixture.activity_day_history; DROP TABLE old_fixture.current_activity_days; DROP TABLE old_fixture.current_activity; DROP TABLE old_fixture.activity_summaries; DROP TABLE old_fixture.collection_runs; DROP TABLE old_fixture.profile_identity_events; PRAGMA old_fixture.user_version=1; DETACH DATABASE old_fixture;").unwrap();
        require_no_sidecars(&path).unwrap();
        let original = fs::read(&path).unwrap();
        let verified = live.verify_encrypted_checkpoint(&path).unwrap();
        assert_eq!(verified.schema_version, 1);
        assert_eq!(verified.summary.active_profiles, 1);
        let snapshot = live.read_encrypted_checkpoint(&path).unwrap();
        assert_eq!(snapshot.schema_version, 1);
        crate::backup_validation::with_validated_snapshot::<_, crate::BackupValidationError>(
            &snapshot.bytes,
            1,
            true,
            |normalized, _| {
                assert_eq!(
                    normalized
                        .pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
                        .unwrap(),
                    CURRENT_SCHEMA_VERSION
                );
                assert!(
                    crate::backup_validation::activity_records(normalized)
                        .iter()
                        .all(Vec::is_empty)
                );
                Ok(())
            },
        )
        .unwrap();
        let metadata =
            validate_checkpoint_restore_input(&snapshot.bytes, snapshot.schema_version).unwrap();
        let revision = live.restore_state().unwrap().revision;
        live.replace_records_from_checkpoint(
            &snapshot.bytes,
            snapshot.schema_version,
            &revision,
            &metadata.homes,
        )
        .unwrap();
        assert_eq!(
            live.metadata().unwrap().schema_version,
            CURRENT_SCHEMA_VERSION
        );
        assert_eq!(fs::read(&path).unwrap(), original);
        require_no_sidecars(&path).unwrap();
    }
}
