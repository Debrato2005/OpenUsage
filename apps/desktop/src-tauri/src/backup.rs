use crate::workspace::{CommandError, prepare_directory, validate_passphrase};
use openusage_import::{BackupError, Entry, Manifest, encoded_size, encrypt, prepare_restore};
use openusage_storage::BackupSnapshot;
use serde::Serialize;
use std::{
    fs::{self, File},
    io::{Cursor, Seek, SeekFrom, Write},
    path::Path,
};
use zeroize::Zeroizing;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupDestination {
    pub file_name: String,
    pub directory: String,
}

pub fn create(
    directory: &Path,
    snapshot: BackupSnapshot,
    password: Zeroizing<String>,
) -> Result<BackupDestination, CommandError> {
    validate_passphrase(&password)?;
    let directory_name = directory
        .to_str()
        .ok_or(CommandError::BackupUnavailable)?
        .to_owned();
    prepare_directory(directory).map_err(|_| CommandError::BackupUnavailable)?;
    let manifest = Manifest {
        schema_version: snapshot.schema_version,
        entries: vec![Entry {
            name: "records.sqlite".into(),
            size: snapshot.bytes.len() as u64,
        }],
    };
    let archive_size = encoded_size(&manifest).map_err(codec_error)?;
    require_space(
        available_space(directory)?,
        archive_size,
        snapshot.bytes.len() as u64,
    )?;
    let mut pending = tempfile::Builder::new()
        .prefix("openusage-backup-")
        .rand_bytes(16)
        .suffix(".oubackup.pending")
        .tempfile_in(directory)
        .map_err(|_| CommandError::BackupUnavailable)?;
    let file_name = pending
        .path()
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.strip_suffix(".pending"))
        .ok_or(CommandError::BackupUnavailable)?
        .to_owned();
    let destination = directory.join(&file_name);
    encrypt(
        &manifest,
        &mut Cursor::new(snapshot.bytes.as_slice()),
        pending.as_file_mut(),
        password.as_bytes(),
        available_space(directory)?,
    )
    .map_err(codec_error)?;
    pending
        .flush()
        .map_err(|_| CommandError::BackupUnavailable)?;
    pending
        .as_file()
        .sync_all()
        .map_err(|_| CommandError::BackupUnavailable)?;
    pending
        .seek(SeekFrom::Start(0))
        .map_err(|_| CommandError::BackupUnavailable)?;
    verify(
        pending.as_file_mut(),
        &manifest,
        password.as_bytes(),
        directory,
        available_space(directory)?,
    )?;
    // The snapshot and archive key stay in Rust memory; no plaintext file is created.
    drop(snapshot);
    prepare_directory(directory).map_err(|_| CommandError::BackupUnavailable)?;
    require_private_file(pending.as_file())?;
    publish_noclobber(pending.path(), &destination)?;
    pending.disable_cleanup(true);
    if sync_directory(directory).is_err() {
        let _ = fs::remove_file(&destination);
        let _ = sync_directory(directory);
        return Err(CommandError::BackupUnavailable);
    }
    Ok(BackupDestination {
        file_name,
        directory: directory_name,
    })
}

fn require_space(available: u64, archive: u64, restored: u64) -> Result<(), CommandError> {
    let required = archive
        .checked_mul(2)
        .and_then(|size| size.checked_add(restored))
        .ok_or(CommandError::BackupUnavailable)?;
    if available < required {
        return Err(CommandError::InsufficientSpace);
    }
    Ok(())
}

fn verify(
    archive: &mut File,
    expected: &Manifest,
    password: &[u8],
    directory: &Path,
    available: u64,
) -> Result<(), CommandError> {
    let verified = prepare_restore(archive, password, directory, available).map_err(codec_error)?;
    if verified.manifest() != expected {
        return Err(CommandError::BackupUnavailable);
    }
    Ok(())
}

fn codec_error(error: BackupError) -> CommandError {
    match error {
        BackupError::InsufficientSpace => CommandError::InsufficientSpace,
        _ => CommandError::BackupUnavailable,
    }
}

fn require_private_file(file: &File) -> Result<(), CommandError> {
    let metadata = file
        .metadata()
        .map_err(|_| CommandError::BackupUnavailable)?;
    if !metadata.is_file() {
        return Err(CommandError::BackupUnavailable);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        // geteuid reads the process identity without pointer arguments.
        if metadata.nlink() != 1
            || metadata.permissions().mode() & 0o077 != 0
            || metadata.uid() != unsafe { libc::geteuid() }
        {
            return Err(CommandError::BackupUnavailable);
        }
    }
    Ok(())
}

#[cfg(unix)]
fn available_space(directory: &Path) -> Result<u64, CommandError> {
    use std::os::unix::ffi::OsStrExt;
    let path = std::ffi::CString::new(directory.as_os_str().as_bytes())
        .map_err(|_| CommandError::BackupUnavailable)?;
    let mut result = std::mem::MaybeUninit::<libc::statvfs>::uninit();
    // statvfs initializes the structure on success; the C path remains alive during the call.
    if unsafe { libc::statvfs(path.as_ptr(), result.as_mut_ptr()) } != 0 {
        return Err(CommandError::BackupUnavailable);
    }
    // The successful call initialized all fields.
    let result = unsafe { result.assume_init() };
    let available = u128::from(result.f_bavail) * u128::from(result.f_frsize);
    u64::try_from(available).map_err(|_| CommandError::BackupUnavailable)
}

#[cfg(target_os = "windows")]
fn available_space(directory: &Path) -> Result<u64, CommandError> {
    use std::os::windows::ffi::OsStrExt;
    let path: Vec<u16> = directory.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut available = 0;
    // The path is NUL terminated and the output pointer remains valid during the call.
    if unsafe {
        windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW(
            path.as_ptr(),
            &mut available,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    } == 0
    {
        return Err(CommandError::BackupUnavailable);
    }
    Ok(available)
}

#[cfg(unix)]
fn publish_noclobber(source: &Path, destination: &Path) -> Result<(), CommandError> {
    use std::os::unix::ffi::OsStrExt;
    let source = std::ffi::CString::new(source.as_os_str().as_bytes())
        .map_err(|_| CommandError::BackupUnavailable)?;
    let destination = std::ffi::CString::new(destination.as_os_str().as_bytes())
        .map_err(|_| CommandError::BackupUnavailable)?;
    #[cfg(target_os = "linux")]
    // Both paths remain valid; RENAME_NOREPLACE atomically refuses any existing destination.
    let result = unsafe {
        libc::renameat2(
            libc::AT_FDCWD,
            source.as_ptr(),
            libc::AT_FDCWD,
            destination.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    #[cfg(target_os = "macos")]
    // RENAME_EXCL refuses an existing destination without a hard-link fallback.
    let result =
        unsafe { libc::renamex_np(source.as_ptr(), destination.as_ptr(), libc::RENAME_EXCL) };
    if result != 0 {
        return Err(CommandError::BackupUnavailable);
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn publish_noclobber(source: &Path, destination: &Path) -> Result<(), CommandError> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_ATTRIBUTE_NORMAL, MOVEFILE_WRITE_THROUGH, MoveFileExW, SetFileAttributesW,
    };
    let source: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
    let destination: Vec<u16> = destination
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    // Clear tempfile's temporary-file hint before publishing a durable archive.
    if unsafe { SetFileAttributesW(source.as_ptr(), FILE_ATTRIBUTE_NORMAL) } == 0 {
        return Err(CommandError::BackupUnavailable);
    }
    // Both paths are NUL terminated. No replacement or cross-volume copy flag is enabled.
    if unsafe {
        MoveFileExW(
            source.as_ptr(),
            destination.as_ptr(),
            MOVEFILE_WRITE_THROUGH,
        )
    } == 0
    {
        return Err(CommandError::BackupUnavailable);
    }
    Ok(())
}

fn sync_directory(directory: &Path) -> Result<(), CommandError> {
    #[cfg(unix)]
    File::open(directory)
        .and_then(|file| file.sync_all())
        .map_err(|_| CommandError::BackupUnavailable)?;
    #[cfg(not(unix))]
    let _ = directory;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use openusage_storage::{Storage, Theme};
    use std::io::Read;

    const PASSWORD: &str = "synthetic backup password";

    #[test]
    fn encrypted_snapshot_roundtrip_rejects_tampering_and_insufficient_verification_space() {
        let temporary = tempfile::tempdir().unwrap();
        let workspace = temporary.path().join("workspace");
        prepare_directory(&workspace).unwrap();
        let mut storage = Storage::create(&workspace.join("openusage.db"), &[0x2a; 32]).unwrap();
        storage.set_theme(Theme::Dark).unwrap();
        let snapshot = storage.backup_snapshot().unwrap();
        let manifest = Manifest {
            schema_version: snapshot.schema_version,
            entries: vec![Entry {
                name: "records.sqlite".into(),
                size: snapshot.bytes.len() as u64,
            }],
        };
        let directory = temporary.path().join("Downloads").join("OpenUsage");
        let result = create(&directory, snapshot, Zeroizing::new(PASSWORD.into())).unwrap();
        assert_eq!(result.directory, directory.to_str().unwrap());
        let path = directory.join(result.file_name);
        let bytes = fs::read(&path).unwrap();
        assert!(bytes.starts_with(b"OUARCH01"));
        assert!(!bytes.windows(16).any(|part| part == b"SQLite format 3\0"));
        assert_eq!(fs::read_dir(&directory).unwrap().count(), 1);
        let mut archive = File::open(&path).unwrap();
        let mut verified = prepare_restore(
            &mut archive,
            PASSWORD.as_bytes(),
            &directory,
            available_space(&directory).unwrap(),
        )
        .unwrap();
        assert_eq!(verified.manifest(), &manifest);
        let mut restored = Zeroizing::new(Vec::new());
        verified.copy_payload(&mut *restored).unwrap();
        drop(verified);
        let mut connection = rusqlite::Connection::open_in_memory().unwrap();
        connection
            .deserialize_read_exact(
                "main",
                Cursor::new(restored.as_slice()),
                restored.len(),
                true,
            )
            .unwrap();
        let theme: String = connection
            .query_row("SELECT theme FROM settings WHERE id=1", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(theme, "dark");
        archive.seek(SeekFrom::Start(0)).unwrap();
        assert_eq!(
            verify(&mut archive, &manifest, PASSWORD.as_bytes(), &directory, 0),
            Err(CommandError::InsufficientSpace)
        );
        assert_eq!(fs::read(&path).unwrap(), bytes);
        let mut tampered = bytes;
        *tampered.last_mut().unwrap() ^= 1;
        fs::write(&path, tampered).unwrap();
        let mut archive = File::open(&path).unwrap();
        assert_eq!(
            verify(
                &mut archive,
                &manifest,
                PASSWORD.as_bytes(),
                &directory,
                available_space(&directory).unwrap(),
            ),
            Err(CommandError::BackupUnavailable)
        );
        assert_eq!(fs::read_dir(&directory).unwrap().count(), 1);
        assert!(require_space(1023, 512, 1).is_err());
        #[cfg(unix)]
        {
            use std::os::unix::fs::{MetadataExt, PermissionsExt};
            let metadata = fs::metadata(path).unwrap();
            assert_eq!(metadata.permissions().mode() & 0o777, 0o600);
            assert_eq!(metadata.nlink(), 1);
        }
    }

    #[test]
    fn publication_preserves_existing_files_and_rejects_linked_destinations() {
        let temporary = tempfile::tempdir().unwrap();
        let source = temporary.path().join("pending");
        let destination = temporary.path().join("backup.oubackup");
        fs::write(&source, b"verified encrypted archive").unwrap();
        fs::write(&destination, b"keep existing archive").unwrap();
        assert_eq!(
            publish_noclobber(&source, &destination),
            Err(CommandError::BackupUnavailable)
        );
        assert_eq!(fs::read(&destination).unwrap(), b"keep existing archive");
        assert!(source.exists());
        fs::remove_file(&destination).unwrap();
        publish_noclobber(&source, &destination).unwrap();
        assert!(!source.exists());
        let mut file = File::open(&destination).unwrap();
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes, b"verified encrypted archive");
        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            let link = temporary.path().join("linked-directory");
            symlink(temporary.path(), &link).unwrap();
            assert_eq!(
                prepare_directory(&link.join("OpenUsage")),
                Err(CommandError::StorageUnavailable)
            );
        }
    }
}
