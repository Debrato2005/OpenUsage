use rusqlite::{Connection, OpenFlags, TransactionBehavior};
use std::{fmt, fs, path::Path, time::Duration};
use zeroize::Zeroizing;

pub mod keywrap;

const MIGRATIONS: &[&str] = &[include_str!("schema.sql")];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Theme {
    System,
    Light,
    Dark,
}

impl Theme {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StorageMetadata {
    pub schema_version: u32,
    pub profile_count: u32,
    pub theme: Theme,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StorageError {
    InvalidPath,
    CreateFailed,
    OpenFailed,
    EncryptionUnavailable,
    UnlockFailed,
    ConfigurationFailed,
    UnsupportedSchema,
    MigrationFailed,
    ReadFailed,
    WriteFailed,
}

impl fmt::Display for StorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidPath => {
                "Use a regular database file inside a private application directory."
            }
            Self::CreateFailed => {
                "The database could not be created. Existing files were preserved."
            }
            Self::OpenFailed => "The existing database could not be opened. No database was reset.",
            Self::EncryptionUnavailable => "SQLCipher 4 encryption is unavailable.",
            Self::UnlockFailed => {
                "The database could not be unlocked. Keep the database and recover its key."
            }
            Self::ConfigurationFailed => {
                "The database could not enable the required storage protections."
            }
            Self::UnsupportedSchema => "This database requires a newer version of OpenUsage.",
            Self::MigrationFailed => {
                "The database migration failed. The previous data was preserved."
            }
            Self::ReadFailed => "The database records could not be read.",
            Self::WriteFailed => "The database change could not be saved.",
        })
    }
}

impl std::error::Error for StorageError {}

pub struct Storage {
    connection: Connection,
}

impl Storage {
    /// The caller must first protect the random key with the OS store or passphrase wrapper.
    /// The caller must select its managed local application directory outside cloud-sync folders.
    pub fn create(path: &Path, key: &[u8; 32]) -> Result<Self, StorageError> {
        validate_path(path)?;
        let mut options = fs::OpenOptions::new();
        options.read(true).write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let file = options.open(path).map_err(|_| StorageError::CreateFailed)?;
        drop(file);
        Self::connect(path, key)
    }

    /// Opening never creates, replaces, repairs, or deletes a database.
    pub fn open(path: &Path, key: &[u8; 32]) -> Result<Self, StorageError> {
        validate_path(path)?;
        let metadata = fs::symlink_metadata(path).map_err(|_| StorageError::OpenFailed)?;
        if !metadata.is_file() || metadata.len() == 0 {
            return Err(StorageError::OpenFailed);
        }
        Self::connect(path, key)
    }

    fn connect(path: &Path, key: &[u8; 32]) -> Result<Self, StorageError> {
        let flags = OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_NO_MUTEX
            | OpenFlags::SQLITE_OPEN_NOFOLLOW;
        let mut connection =
            Connection::open_with_flags(path, flags).map_err(|_| StorageError::OpenFailed)?;

        // SQLCipher raw keys use a 64-digit hexadecimal string inside x'...'.
        let mut encoded = Zeroizing::new(String::with_capacity(67));
        encoded.push_str("x'");
        for byte in key {
            use std::fmt::Write;
            write!(&mut *encoded, "{byte:02x}").map_err(|_| StorageError::UnlockFailed)?;
        }
        encoded.push('\'');
        connection
            .pragma_update(None, "key", encoded.as_str())
            .map_err(|_| StorageError::UnlockFailed)?;
        drop(encoded);

        let cipher_version: String = connection
            .query_row("PRAGMA cipher_version", [], |row| row.get(0))
            .map_err(|_| StorageError::EncryptionUnavailable)?;
        if !cipher_version.starts_with("4.") {
            return Err(StorageError::EncryptionUnavailable);
        }
        connection
            .pragma_update(None, "cipher_memory_security", true)
            .map_err(|_| StorageError::ConfigurationFailed)?;
        // Key assignment alone does not authenticate an existing encrypted database.
        connection
            .query_row("SELECT count(*) FROM sqlite_schema", [], |row| {
                row.get::<_, i64>(0)
            })
            .map_err(|_| StorageError::UnlockFailed)?;
        let version: u32 = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .map_err(|_| StorageError::UnlockFailed)?;
        if version as usize > MIGRATIONS.len() {
            return Err(StorageError::UnsupportedSchema);
        }

        connection
            .busy_timeout(Duration::from_millis(5000))
            .and_then(|()| connection.pragma_update(None, "foreign_keys", true))
            .and_then(|()| connection.pragma_update(None, "synchronous", "FULL"))
            .and_then(|()| connection.pragma_update(None, "temp_store", "MEMORY"))
            .map_err(|_| StorageError::ConfigurationFailed)?;
        let journal: String = connection
            .query_row("PRAGMA journal_mode=WAL", [], |row| row.get(0))
            .map_err(|_| StorageError::ConfigurationFailed)?;
        let foreign_keys: bool = connection
            .pragma_query_value(None, "foreign_keys", |row| row.get(0))
            .map_err(|_| StorageError::ConfigurationFailed)?;
        let synchronous: i64 = connection
            .pragma_query_value(None, "synchronous", |row| row.get(0))
            .map_err(|_| StorageError::ConfigurationFailed)?;
        let busy_timeout: i64 = connection
            .pragma_query_value(None, "busy_timeout", |row| row.get(0))
            .map_err(|_| StorageError::ConfigurationFailed)?;
        let memory_security: String = connection
            .pragma_query_value(None, "cipher_memory_security", |row| row.get(0))
            .map_err(|_| StorageError::ConfigurationFailed)?;
        let temporary_storage: i64 = connection
            .pragma_query_value(None, "temp_store", |row| row.get(0))
            .map_err(|_| StorageError::ConfigurationFailed)?;
        if journal != "wal"
            || !foreign_keys
            || synchronous != 2
            || busy_timeout != 5000
            || memory_security != "1"
            || temporary_storage != 2
        {
            return Err(StorageError::ConfigurationFailed);
        }
        migrate(&mut connection, MIGRATIONS)?;
        Ok(Self { connection })
    }

    pub fn metadata(&self) -> Result<StorageMetadata, StorageError> {
        self.connection
            .query_row(
                "SELECT (SELECT user_version FROM pragma_user_version), \
                 (SELECT count(*) FROM profiles WHERE archived = 0), theme \
                 FROM settings WHERE id = 1",
                [],
                |row| {
                    let theme = match row.get::<_, String>(2)?.as_str() {
                        "system" => Theme::System,
                        "light" => Theme::Light,
                        "dark" => Theme::Dark,
                        _ => return Err(rusqlite::Error::InvalidQuery),
                    };
                    Ok(StorageMetadata {
                        schema_version: row.get(0)?,
                        profile_count: row.get(1)?,
                        theme,
                    })
                },
            )
            .map_err(|_| StorageError::ReadFailed)
    }

    pub fn set_theme(&mut self, theme: Theme) -> Result<(), StorageError> {
        let changed = self.connection
            .execute(
                "UPDATE settings SET theme = ?1, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = 1",
                [theme.as_str()],
            )
            .map_err(|_| StorageError::WriteFailed)?;
        if changed != 1 {
            return Err(StorageError::WriteFailed);
        }
        Ok(())
    }
}

fn migrate(connection: &mut Connection, migrations: &[&str]) -> Result<(), StorageError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_| StorageError::MigrationFailed)?;
    let version: u32 = transaction
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(|_| StorageError::MigrationFailed)?;
    if version as usize > migrations.len() {
        return Err(StorageError::UnsupportedSchema);
    }
    for (index, migration) in migrations.iter().enumerate().skip(version as usize) {
        transaction
            .execute_batch(migration)
            .and_then(|()| transaction.pragma_update(None, "user_version", (index + 1) as u32))
            .map_err(|_| StorageError::MigrationFailed)?;
    }
    transaction
        .commit()
        .map_err(|_| StorageError::MigrationFailed)
}

fn validate_path(path: &Path) -> Result<(), StorageError> {
    if !path.is_absolute()
        || path.file_name().is_none()
        || path
            .components()
            .any(|part| matches!(part, std::path::Component::ParentDir))
    {
        return Err(StorageError::InvalidPath);
    }
    for ancestor in path.ancestors().skip(1) {
        let metadata = fs::symlink_metadata(ancestor).map_err(|_| StorageError::InvalidPath)?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(StorageError::InvalidPath);
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let parent = path.parent().ok_or(StorageError::InvalidPath)?;
        let mode = fs::metadata(parent)
            .map_err(|_| StorageError::InvalidPath)?
            .permissions()
            .mode();
        if mode & 0o077 != 0 {
            return Err(StorageError::InvalidPath);
        }
    }
    for suffix in ["", "-wal", "-shm", "-journal"] {
        let mut name = path.as_os_str().to_os_string();
        name.push(suffix);
        match fs::symlink_metadata(&name) {
            Ok(metadata) if !metadata.is_file() || metadata.file_type().is_symlink() => {
                return Err(StorageError::InvalidPath);
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(StorageError::InvalidPath),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: [u8; 32] = [0x2a; 32];

    fn private_directory() -> tempfile::TempDir {
        let mut builder = tempfile::Builder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            builder.permissions(fs::Permissions::from_mode(0o700));
        }
        builder.tempdir().unwrap()
    }

    #[test]
    fn encrypted_database_and_wal_reopen_without_losing_settings() {
        let directory = private_directory();
        let path = directory.path().join("usage.db");
        let mut storage = Storage::create(&path, &KEY).unwrap();
        storage.set_theme(Theme::Dark).unwrap();
        assert_eq!(storage.metadata().unwrap().profile_count, 0);

        for file in [path.clone(), path.with_file_name("usage.db-wal")] {
            let bytes = fs::read(file).unwrap();
            assert!(!bytes.starts_with(b"SQLite format 3\0"));
            let schema_marker = b"CREATE TABLE settings";
            assert!(
                !bytes
                    .windows(schema_marker.len())
                    .any(|part| part == schema_marker)
            );
        }
        let unkeyed = Connection::open(&path).unwrap();
        assert!(
            unkeyed
                .query_row("SELECT count(*) FROM sqlite_schema", [], |row| row
                    .get::<_, i64>(0))
                .is_err()
        );
        drop(unkeyed);
        drop(storage);

        let reopened = Storage::open(&path, &KEY).unwrap();
        assert_eq!(
            reopened.metadata().unwrap(),
            StorageMetadata {
                schema_version: 1,
                profile_count: 0,
                theme: Theme::Dark,
            }
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }

    #[test]
    fn wrong_key_or_missing_database_never_creates_or_resets_records() {
        let directory = private_directory();
        let path = directory.path().join("usage.db");
        let mut storage = Storage::create(&path, &KEY).unwrap();
        storage.set_theme(Theme::Light).unwrap();
        drop(storage);
        let before = fs::read(&path).unwrap();
        assert!(matches!(
            Storage::open(&path, &[0x17; 32]),
            Err(StorageError::UnlockFailed)
        ));
        assert_eq!(fs::read(&path).unwrap(), before);
        assert_eq!(
            Storage::open(&path, &KEY)
                .unwrap()
                .metadata()
                .unwrap()
                .theme,
            Theme::Light
        );
        assert!(matches!(
            Storage::create(&path, &KEY),
            Err(StorageError::CreateFailed)
        ));
        let missing = directory.path().join("missing.db");
        assert!(matches!(
            Storage::open(&missing, &KEY),
            Err(StorageError::OpenFailed)
        ));
        assert!(!missing.exists());
    }

    #[test]
    fn failed_migration_rolls_back_changes_and_version() {
        let directory = private_directory();
        let path = directory.path().join("usage.db");
        let mut storage = Storage::create(&path, &KEY).unwrap();
        storage.set_theme(Theme::Dark).unwrap();
        let migrations = [
            MIGRATIONS[0],
            "UPDATE settings SET theme = 'light'; CREATE TABLE pending(id INTEGER PRIMARY KEY); SELECT missing_column FROM settings;",
        ];
        assert_eq!(
            migrate(&mut storage.connection, &migrations),
            Err(StorageError::MigrationFailed)
        );
        assert_eq!(storage.metadata().unwrap().theme, Theme::Dark);
        assert_eq!(storage.metadata().unwrap().schema_version, 1);
        let pending: i64 = storage
            .connection
            .query_row(
                "SELECT count(*) FROM sqlite_schema WHERE name = 'pending'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(pending, 0);
        drop(storage);
        assert_eq!(
            Storage::open(&path, &KEY)
                .unwrap()
                .metadata()
                .unwrap()
                .theme,
            Theme::Dark
        );
    }

    #[test]
    fn newer_schema_and_malicious_path_links_preserve_existing_files() {
        let directory = private_directory();
        let path = directory.path().join("usage.db");
        let storage = Storage::create(&path, &KEY).unwrap();
        storage
            .connection
            .pragma_update(None, "user_version", 99)
            .unwrap();
        drop(storage);
        let before = fs::read(&path).unwrap();
        assert!(matches!(
            Storage::open(&path, &KEY),
            Err(StorageError::UnsupportedSchema)
        ));
        assert_eq!(fs::read(&path).unwrap(), before);
        #[cfg(unix)]
        {
            let link = directory.path().join("link.db");
            std::os::unix::fs::symlink(&path, &link).unwrap();
            assert!(matches!(
                Storage::open(&link, &KEY),
                Err(StorageError::InvalidPath)
            ));
            let wal_link = directory.path().join("new.db-wal");
            std::os::unix::fs::symlink(&path, wal_link).unwrap();
            assert!(matches!(
                Storage::create(&directory.path().join("new.db"), &KEY),
                Err(StorageError::InvalidPath)
            ));
            assert_eq!(fs::read(&path).unwrap(), before);
        }
    }
}
