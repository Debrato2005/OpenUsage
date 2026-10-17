//! Inspect archive metadata after complete authentication. This operation does not validate
//! database contents, create a restore checkpoint, or restore records. It does not complete AT-32.
//! Only current export filenames in the managed Downloads folder are selectable.
//! Renamed and imported archives are outside this slice.
//! The list returns at most 100 candidates and scans at most 1024 entries.
//! It creates no missing source directories.
//! Windows inspection fails closed until file access controls pass a separate review.

use crate::{
    backup::{available_space, require_private_file},
    workspace::{
        CommandError, existing_private_directory, prepare_directory, validate_unlock_passphrase,
    },
};
use openusage_import::{BackupError, prepare_restore};
use serde::Serialize;
use std::{
    fs::{self, File, OpenOptions},
    io,
    path::Path,
};
use zeroize::Zeroizing;

const PREFIX: &str = "openusage-backup-";
const SUFFIX: &str = ".oubackup";
const MAX_CANDIDATES: usize = 100;
const MAX_SCANNED: usize = 1024;
// Version 1 permits 5 GiB of contents plus bounded manifest and frame overhead.
const MAX_ARCHIVE_BYTES: u64 = 5 * 1024 * 1024 * 1024 + 4 * 1024 * 1024;

#[derive(Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupCandidate {
    pub file_name: String,
    pub file_bytes: String,
}

/// The list contains unverified candidates. A true truncated flag means a result or scan limit was reached.
#[derive(Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupList {
    pub backups: Vec<BackupCandidate>,
    pub truncated: bool,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InspectedEntry {
    pub name: String,
    pub bytes: String,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupInspection {
    pub file_name: String,
    pub archive_version: u8,
    pub manifest_schema_version: u32,
    pub entry_count: usize,
    pub total_bytes: String,
    pub entries: Vec<InspectedEntry>,
    pub database_validation: &'static str,
    pub restore_available: bool,
}

pub fn validate_file_name(name: &str) -> Result<(), CommandError> {
    if name.len() != PREFIX.len() + 16 + SUFFIX.len() {
        return Err(CommandError::InvalidArguments);
    }
    let random = name
        .strip_prefix(PREFIX)
        .and_then(|name| name.strip_suffix(SUFFIX))
        .ok_or(CommandError::InvalidArguments)?;
    if random.len() != 16 || !random.bytes().all(|byte| byte.is_ascii_alphanumeric()) {
        return Err(CommandError::InvalidArguments);
    }
    Ok(())
}

fn require_supported_platform() -> Result<(), CommandError> {
    // Windows inspection requires a separate review of file ownership and access controls.
    if cfg!(unix) {
        Ok(())
    } else {
        Err(CommandError::BackupUnavailable)
    }
}

pub fn list(directory: &Path) -> Result<BackupList, CommandError> {
    require_supported_platform()?;
    let mut result = BackupList {
        backups: Vec::new(),
        truncated: false,
    };
    if !existing_private_directory(directory).map_err(|_| CommandError::BackupUnavailable)? {
        return Ok(result);
    }
    let entries = fs::read_dir(directory).map_err(|_| CommandError::BackupUnavailable)?;
    let mut scanned = 0;
    for entry in entries.take(MAX_SCANNED) {
        scanned += 1;
        let entry = entry.map_err(|_| CommandError::BackupUnavailable)?;
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if validate_file_name(&name).is_err() {
            continue;
        }
        let Ok(file) = open_candidate(directory, &name) else {
            continue;
        };
        let metadata = file
            .metadata()
            .map_err(|_| CommandError::BackupUnavailable)?;
        if result.backups.len() == MAX_CANDIDATES {
            result.truncated = true;
            continue;
        }
        result.backups.push(BackupCandidate {
            file_name: name,
            file_bytes: metadata.len().to_string(),
        });
    }
    result.truncated |= scanned == MAX_SCANNED;
    result
        .backups
        .sort_unstable_by(|left, right| left.file_name.cmp(&right.file_name));
    Ok(result)
}

pub fn inspect(
    directory: &Path,
    staging_directory: &Path,
    file_name: &str,
    password: Zeroizing<String>,
) -> Result<BackupInspection, CommandError> {
    validate_file_name(file_name)?;
    validate_unlock_passphrase(&password)?;
    require_supported_platform()?;
    if !existing_private_directory(directory).map_err(|_| CommandError::BackupUnavailable)? {
        return Err(CommandError::BackupNotFound);
    }
    let mut source = open_candidate(directory, file_name)?;
    if source
        .metadata()
        .map_err(|_| CommandError::BackupUnavailable)?
        .len()
        < 60
    {
        return Err(CommandError::InvalidBackup);
    }
    prepare_directory(staging_directory).map_err(|_| CommandError::BackupUnavailable)?;
    authenticate(
        &mut source,
        staging_directory,
        file_name,
        password.as_bytes(),
        available_space(staging_directory)?,
    )
}

fn open_candidate(directory: &Path, file_name: &str) -> Result<File, CommandError> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = options.open(directory.join(file_name)).map_err(|error| {
        if error.kind() == io::ErrorKind::NotFound {
            CommandError::BackupNotFound
        } else {
            CommandError::BackupUnavailable
        }
    })?;
    require_private_file(&file)?;
    if file
        .metadata()
        .map_err(|_| CommandError::BackupUnavailable)?
        .len()
        > MAX_ARCHIVE_BYTES
    {
        return Err(CommandError::InvalidBackup);
    }
    Ok(file)
}

fn authenticate(
    source: &mut File,
    staging_directory: &Path,
    file_name: &str,
    password: &[u8],
    available: u64,
) -> Result<BackupInspection, CommandError> {
    let original_size = source
        .metadata()
        .map_err(|_| CommandError::BackupUnavailable)?
        .len();
    let verified = prepare_restore(source, password, staging_directory, available)
        .map_err(inspection_error)?;
    let manifest = verified.manifest();
    let total = manifest.entries.iter().try_fold(0u64, |total, entry| {
        total
            .checked_add(entry.size)
            .ok_or(CommandError::InvalidBackup)
    })?;
    let result = BackupInspection {
        file_name: file_name.into(),
        archive_version: 1,
        manifest_schema_version: manifest.schema_version,
        entry_count: manifest.entries.len(),
        total_bytes: total.to_string(),
        entries: manifest
            .entries
            .iter()
            .map(|entry| InspectedEntry {
                name: entry.name.clone(),
                bytes: entry.size.to_string(),
            })
            .collect(),
        database_validation: "not_checked",
        restore_available: false,
    };
    drop(verified);
    require_private_file(source)?;
    if source
        .metadata()
        .map_err(|_| CommandError::BackupUnavailable)?
        .len()
        != original_size
    {
        return Err(CommandError::InvalidBackup);
    }
    Ok(result)
}

fn inspection_error(error: BackupError) -> CommandError {
    match error {
        BackupError::InsufficientSpace => CommandError::InsufficientSpace,
        BackupError::AuthenticationFailed => CommandError::BackupAuthenticationFailed,
        BackupError::InvalidArchive | BackupError::InvalidManifest | BackupError::LimitExceeded => {
            CommandError::InvalidBackup
        }
        _ => CommandError::BackupUnavailable,
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use openusage_import::{Entry, Manifest, encrypt};
    use std::{
        io::{Cursor, Seek, SeekFrom, Write},
        os::unix::fs::{OpenOptionsExt, PermissionsExt, symlink},
    };

    const FILE_NAME: &str = "openusage-backup-0123456789abcdef.oubackup";
    const PASSWORD: &str = "synthetic archive password";

    struct Fixture {
        _temporary: tempfile::TempDir,
        directory: std::path::PathBuf,
        staging: std::path::PathBuf,
        original: Vec<u8>,
    }

    fn fixture() -> Fixture {
        let temporary = tempfile::tempdir().unwrap();
        let directory = temporary.path().join("Downloads/OpenUsage");
        let staging = temporary.path().join("workspace/backup-inspection");
        prepare_directory(&directory).unwrap();
        let payload = b"synthetic content, deliberately not a SQLite database";
        let manifest = Manifest {
            schema_version: 77,
            entries: vec![Entry {
                name: "records.sqlite".into(),
                size: payload.len() as u64,
            }],
        };
        let mut file = private_file(&directory.join(FILE_NAME));
        encrypt(
            &manifest,
            &mut Cursor::new(payload),
            &mut file,
            PASSWORD.as_bytes(),
            available_space(&directory).unwrap(),
        )
        .unwrap();
        let original = fs::read(directory.join(FILE_NAME)).unwrap();
        Fixture {
            _temporary: temporary,
            directory,
            staging,
            original,
        }
    }

    fn private_file(path: &Path) -> File {
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)
            .unwrap()
    }

    #[test]
    fn complete_authentication_returns_manifest_without_claiming_database_validity() {
        let fixture = fixture();
        let source = fixture.directory.join(FILE_NAME);
        fs::set_permissions(&source, fs::Permissions::from_mode(0o400)).unwrap();
        let listed = list(&fixture.directory).unwrap();
        assert_eq!(listed.backups.len(), 1);
        assert_eq!(listed.backups[0].file_name, FILE_NAME);
        assert_eq!(
            listed.backups[0].file_bytes,
            fixture.original.len().to_string()
        );
        assert!(!listed.truncated);
        assert!(!fixture.staging.exists());
        let inspection = inspect(
            &fixture.directory,
            &fixture.staging,
            FILE_NAME,
            Zeroizing::new(PASSWORD.into()),
        )
        .unwrap();
        assert_eq!(inspection.archive_version, 1);
        assert_eq!(inspection.manifest_schema_version, 77);
        assert_eq!(inspection.entry_count, 1);
        assert_eq!(inspection.entries[0].name, "records.sqlite");
        assert_eq!(inspection.total_bytes, inspection.entries[0].bytes);
        assert_eq!(inspection.database_validation, "not_checked");
        assert!(!inspection.restore_available);
        let response = serde_json::to_value(inspection).unwrap();
        assert_eq!(response.as_object().unwrap().len(), 8);
        assert!(
            !response
                .to_string()
                .contains(fixture.directory.to_str().unwrap())
        );
        assert_eq!(fs::read(&source).unwrap(), fixture.original);
        assert_eq!(fs::read_dir(&fixture.directory).unwrap().count(), 1);
        assert_eq!(fs::read_dir(&fixture.staging).unwrap().count(), 0);
        assert_eq!(
            fs::metadata(&source).unwrap().permissions().mode() & 0o777,
            0o400
        );
    }

    #[test]
    fn failed_authentication_and_structure_checks_preserve_sources_and_clean_staging() {
        let fixture = fixture();
        let source = fixture.directory.join(FILE_NAME);
        prepare_directory(&fixture.staging).unwrap();
        let sentinel = fixture.staging.join("keep-existing-file");
        private_file(&sentinel)
            .write_all(b"existing private data")
            .unwrap();
        assert_eq!(
            inspect(
                &fixture.directory,
                &fixture.staging,
                FILE_NAME,
                Zeroizing::new("incorrect password".into()),
            ),
            Err(CommandError::BackupAuthenticationFailed)
        );
        assert_eq!(fs::read(&source).unwrap(), fixture.original);
        let mut archive = File::open(&source).unwrap();
        assert_eq!(
            authenticate(
                &mut archive,
                &fixture.staging,
                FILE_NAME,
                PASSWORD.as_bytes(),
                0
            ),
            Err(CommandError::InsufficientSpace)
        );
        let mut tampered = fixture.original.clone();
        *tampered.last_mut().unwrap() ^= 1;
        let mut trailing = fixture.original.clone();
        trailing.push(0);
        for (bytes, error) in [
            (tampered, CommandError::BackupAuthenticationFailed),
            (
                fixture.original[..fixture.original.len() - 1].to_vec(),
                CommandError::InvalidBackup,
            ),
            (trailing, CommandError::InvalidBackup),
            (vec![0u8; 60], CommandError::InvalidBackup),
        ] {
            fs::write(&source, &bytes).unwrap();
            assert_eq!(
                inspect(
                    &fixture.directory,
                    &fixture.staging,
                    FILE_NAME,
                    Zeroizing::new(PASSWORD.into()),
                ),
                Err(error)
            );
            assert_eq!(fs::read(&source).unwrap(), bytes);
            assert_eq!(fs::read_dir(&fixture.directory).unwrap().count(), 1);
            assert_eq!(fs::read_dir(&fixture.staging).unwrap().count(), 1);
            assert_eq!(fs::read(&sentinel).unwrap(), b"existing private data");
        }
    }

    #[test]
    fn listing_is_bounded_and_does_not_create_missing_download_directories() {
        let temporary = tempfile::tempdir().unwrap();
        let missing = temporary.path().join("missing/Downloads/OpenUsage");
        assert_eq!(
            list(&missing).unwrap(),
            BackupList {
                backups: vec![],
                truncated: false
            }
        );
        assert!(!temporary.path().join("missing").exists());
        let directory = temporary.path().join("Downloads/OpenUsage");
        prepare_directory(&directory).unwrap();
        for index in 0..105 {
            private_file(&directory.join(format!("openusage-backup-{index:016x}.oubackup")));
        }
        let result = list(&directory).unwrap();
        assert_eq!(result.backups.len(), MAX_CANDIDATES);
        assert!(result.truncated);
        assert!(
            result
                .backups
                .iter()
                .all(|candidate| candidate.file_bytes == "0")
        );
        assert!(
            result
                .backups
                .windows(2)
                .all(|pair| pair[0].file_name < pair[1].file_name)
        );
        let noise = temporary.path().join("noise");
        prepare_directory(&noise).unwrap();
        for index in 0..MAX_SCANNED {
            private_file(&noise.join(index.to_string()));
        }
        let result = list(&noise).unwrap();
        assert!(result.backups.is_empty());
        assert!(result.truncated);
        assert_eq!(fs::read_dir(&directory).unwrap().count(), 105);
    }

    #[test]
    fn unsafe_paths_links_permissions_and_sizes_fail_before_staging() {
        let temporary = tempfile::tempdir().unwrap();
        let directory = temporary.path().join("Downloads/OpenUsage");
        let staging = temporary.path().join("workspace/backup-inspection");
        prepare_directory(&directory).unwrap();
        for name in [
            "../openusage-backup-0123456789abcdef.oubackup",
            "/openusage-backup-0123456789abcdef.oubackup",
            "openusage-backup-0123456789abcde/.oubackup",
            "openusage-backup-0123456789abcdeé.oubackup",
            "openusage-backup-0123456789abcdef.oubackup.pending",
            "openusage-backup-0123456789abcde\0.oubackup",
        ] {
            assert_eq!(
                inspect(&directory, &staging, name, Zeroizing::new(PASSWORD.into())),
                Err(CommandError::InvalidArguments)
            );
        }
        for password in [String::new(), "x".repeat(1025)] {
            assert!(inspect(&directory, &staging, FILE_NAME, Zeroizing::new(password)).is_err());
        }
        let source = directory.join(FILE_NAME);
        assert_eq!(
            inspect(
                &directory,
                &staging,
                FILE_NAME,
                Zeroizing::new(PASSWORD.into())
            ),
            Err(CommandError::BackupNotFound)
        );
        let mut file = private_file(&source);
        file.set_len(MAX_ARCHIVE_BYTES + 1).unwrap();
        assert_eq!(
            inspect(
                &directory,
                &staging,
                FILE_NAME,
                Zeroizing::new(PASSWORD.into())
            ),
            Err(CommandError::InvalidBackup)
        );
        assert!(list(&directory).unwrap().backups.is_empty());
        file.set_len(60).unwrap();
        file.seek(SeekFrom::Start(0)).unwrap();
        file.write_all(&[0u8; 60]).unwrap();
        fs::set_permissions(&source, fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(
            inspect(
                &directory,
                &staging,
                FILE_NAME,
                Zeroizing::new(PASSWORD.into())
            ),
            Err(CommandError::BackupUnavailable)
        );
        fs::set_permissions(&source, fs::Permissions::from_mode(0o600)).unwrap();
        let alias = temporary.path().join("hardlink");
        fs::hard_link(&source, &alias).unwrap();
        assert!(list(&directory).unwrap().backups.is_empty());
        assert_eq!(
            inspect(
                &directory,
                &staging,
                FILE_NAME,
                Zeroizing::new(PASSWORD.into())
            ),
            Err(CommandError::BackupUnavailable)
        );
        fs::remove_file(&source).unwrap();
        symlink(&alias, &source).unwrap();
        assert!(list(&directory).unwrap().backups.is_empty());
        assert_eq!(
            inspect(
                &directory,
                &staging,
                FILE_NAME,
                Zeroizing::new(PASSWORD.into())
            ),
            Err(CommandError::BackupUnavailable)
        );
        let linked_directory = temporary.path().join("linked-directory");
        symlink(&directory, &linked_directory).unwrap();
        assert_eq!(
            list(&linked_directory),
            Err(CommandError::BackupUnavailable)
        );
        assert!(!staging.exists());
        assert_eq!(fs::read(alias).unwrap(), vec![0u8; 60]);
    }
}
