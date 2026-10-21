//! Device-local display calibration. No database records or provider data are stored here.
use crate::workspace::{CommandError, prepare_directory};
use serde::Serialize;
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::Path,
    sync::{Arc, Mutex},
};
use tauri::{AppHandle, Manager};

const FILE_NAME: &str = "display-scale.v1";
const ALLOWED: &[u16] = &[90, 100, 110, 125, 150, 175, 200];

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct DisplayScale {
    pub percent: u16,
}

#[derive(Default)]
pub(crate) struct DisplayScaleService(pub Arc<Mutex<()>>);

pub(crate) fn validate(percent: u16) -> Result<(), CommandError> {
    if ALLOWED.contains(&percent) {
        Ok(())
    } else {
        Err(CommandError::InvalidArguments)
    }
}

fn private_file(file: &File) -> Result<(), CommandError> {
    let metadata = file
        .metadata()
        .map_err(|_| CommandError::DisplayScaleUnavailable)?;
    if !metadata.is_file() || metadata.len() > 4 {
        return Err(CommandError::DisplayScaleUnavailable);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        // geteuid reads the current process identity without pointer arguments.
        if metadata.uid() != unsafe { libc::geteuid() }
            || metadata.mode() & 0o077 != 0
            || metadata.nlink() != 1
        {
            return Err(CommandError::DisplayScaleUnavailable);
        }
    }
    Ok(())
}

fn read(directory: &Path) -> Result<u16, CommandError> {
    prepare_directory(directory).map_err(|_| CommandError::DisplayScaleUnavailable)?;
    let path = directory.join(FILE_NAME);
    match fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(100),
        _ => return Err(CommandError::DisplayScaleUnavailable),
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    let file = options
        .open(path)
        .map_err(|_| CommandError::DisplayScaleUnavailable)?;
    private_file(&file)?;
    let mut text = String::new();
    file.take(5)
        .read_to_string(&mut text)
        .map_err(|_| CommandError::DisplayScaleUnavailable)?;
    let text = text.strip_suffix('\n').unwrap_or(&text);
    let percent = text
        .parse::<u16>()
        .map_err(|_| CommandError::DisplayScaleUnavailable)?;
    if text != percent.to_string() || validate(percent).is_err() {
        return Err(CommandError::DisplayScaleUnavailable);
    }
    Ok(percent)
}

fn write(directory: &Path, percent: u16) -> Result<(), CommandError> {
    // Read again before replacement to refuse a newly unsafe destination.
    read(directory)?;
    let mut pending = tempfile::Builder::new()
        .prefix("display-scale-")
        .tempfile_in(directory)
        .map_err(|_| CommandError::DisplayScaleUnavailable)?;
    writeln!(pending, "{percent}").map_err(|_| CommandError::DisplayScaleUnavailable)?;
    pending
        .as_file()
        .sync_all()
        .map_err(|_| CommandError::DisplayScaleUnavailable)?;
    private_file(pending.as_file())?;
    pending
        .persist(directory.join(FILE_NAME))
        .map_err(|_| CommandError::DisplayScaleUnavailable)?;
    // Rename commits a complete value. Directory syncing improves crash persistence, but its
    // failure cannot make a successful atomic replacement into an unchanged failed operation.
    #[cfg(unix)]
    if let Ok(parent) = File::open(directory) {
        let _ = parent.sync_all();
    }
    Ok(())
}

fn change(
    directory: &Path,
    percent: u16,
    mut apply: impl FnMut(u16) -> Result<(), CommandError>,
) -> Result<DisplayScale, CommandError> {
    validate(percent)?;
    let previous = read(directory)?;
    apply(percent)?;
    if let Err(error) = write(directory, percent) {
        apply(previous)?;
        return Err(error);
    }
    Ok(DisplayScale { percent })
}

pub(crate) async fn run(
    app: AppHandle,
    gate: Arc<Mutex<()>>,
    percent: Option<u16>,
) -> Result<DisplayScale, CommandError> {
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = gate
            .lock()
            .map_err(|_| CommandError::DisplayScaleUnavailable)?;
        let directory = app
            .path()
            .app_config_dir()
            .map_err(|_| CommandError::DisplayScaleUnavailable)?;
        let window = app
            .get_webview_window("main")
            .ok_or(CommandError::DisplayScaleUnavailable)?;
        let apply = |percent| {
            window
                .set_zoom(f64::from(percent) / 100.0)
                .map_err(|_| CommandError::DisplayScaleUnavailable)
        };
        if let Some(percent) = percent {
            change(&directory, percent, apply)
        } else {
            let percent = read(&directory)?;
            apply(percent)?;
            Ok(DisplayScale { percent })
        }
    })
    .await
    .map_err(|_| CommandError::DisplayScaleUnavailable)?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_scale_atomic_write_reopens_and_failed_native_apply_preserves_file() {
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().join("preferences");
        assert_eq!(read(&directory), Ok(100));
        assert_eq!(
            change(&directory, 125, |_| Ok(())),
            Ok(DisplayScale { percent: 125 })
        );
        let original = fs::read(directory.join(FILE_NAME)).unwrap();
        assert_eq!(read(&directory), Ok(125));
        assert_eq!(
            change(&directory, 150, |_| Err(
                CommandError::DisplayScaleUnavailable
            )),
            Err(CommandError::DisplayScaleUnavailable)
        );
        assert_eq!(fs::read(directory.join(FILE_NAME)).unwrap(), original);
        assert_eq!(
            change(&directory, 101, |_| panic!(
                "Invalid scale must not reach the native webview"
            )),
            Err(CommandError::InvalidArguments)
        );
        assert_eq!(fs::read(directory.join(FILE_NAME)).unwrap(), original);
    }

    #[test]
    fn display_scale_refuses_invalid_or_oversized_values_without_replacement() {
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().join("preferences");
        prepare_directory(&directory).unwrap();
        for value in [b"101\n".as_slice(), b"10000", b"090\n", b"", b"null"] {
            let file = directory.join(FILE_NAME);
            fs::write(&file, value).unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&file, fs::Permissions::from_mode(0o600)).unwrap();
            }
            assert_eq!(
                change(&directory, 125, |_| panic!(
                    "Unsafe storage must fail before native apply"
                )),
                Err(CommandError::DisplayScaleUnavailable)
            );
            assert_eq!(fs::read(&file).unwrap(), value);
        }
    }

    #[cfg(unix)]
    #[test]
    fn display_scale_refuses_symlinks_hardlinks_and_shared_permissions() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().join("preferences");
        prepare_directory(&directory).unwrap();
        let target = root.path().join("outside");
        fs::write(&target, b"100\n").unwrap();
        fs::set_permissions(&target, fs::Permissions::from_mode(0o600)).unwrap();
        let file = directory.join(FILE_NAME);
        symlink(&target, &file).unwrap();
        assert_eq!(read(&directory), Err(CommandError::DisplayScaleUnavailable));
        fs::remove_file(&file).unwrap();
        fs::hard_link(&target, &file).unwrap();
        assert_eq!(read(&directory), Err(CommandError::DisplayScaleUnavailable));
        fs::remove_file(&file).unwrap();
        fs::write(&file, b"100\n").unwrap();
        fs::set_permissions(&file, fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(read(&directory), Err(CommandError::DisplayScaleUnavailable));
        assert_eq!(fs::read(target).unwrap(), b"100\n");
    }

    #[cfg(unix)]
    #[test]
    fn display_scale_failed_write_rolls_native_zoom_back_and_preserves_old_file() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().join("preferences");
        change(&directory, 110, |_| Ok(())).unwrap();
        let original = directory.join(FILE_NAME);
        let saved = directory.join("saved-scale");
        let mut applied = Vec::new();
        let result = change(&directory, 150, |percent| {
            applied.push(percent);
            if applied.len() == 1 {
                fs::rename(&original, &saved).unwrap();
                symlink(&saved, &original).unwrap();
            }
            Ok(())
        });
        assert_eq!(result, Err(CommandError::DisplayScaleUnavailable));
        assert_eq!(applied, [150, 110]);
        assert_eq!(fs::read(saved).unwrap(), b"110\n");
        assert_eq!(
            fs::metadata(directory).unwrap().permissions().mode() & 0o777,
            0o700
        );
    }
}
