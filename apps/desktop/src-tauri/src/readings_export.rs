//! Explicit, private exports of selected saved readings. No database image reaches disk or IPC.
use crate::{
    backup,
    workspace::{CommandError, prepare_directory},
};
use openusage_storage::ReadingsExport;
use serde::{
    Deserialize, Serialize, Serializer,
    ser::{SerializeMap, SerializeSeq},
};
use std::{
    fs,
    io::{self, Write},
    path::Path,
    time::{Duration, Instant},
};

const MAX_EXPORT_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExportFormat {
    Csv,
    Json,
}
impl ExportFormat {
    fn extension(self) -> &'static str {
        match self {
            Self::Csv => "csv",
            Self::Json => "json",
        }
    }
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportReceipt {
    pub file_name: String,
    pub directory: String,
    pub format: ExportFormat,
    pub profile_count: u32,
    pub row_count: usize,
    pub identifiers_included: bool,
    pub exported_at: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Selection {
    profile_count: u32,
    identifiers_included: bool,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Document<'a> {
    schema_version: u32,
    exported_at: &'a str,
    selection: Selection,
    records: Records<'a>,
}
struct Records<'a>(&'a ReadingsExport);
struct Record<'a> {
    columns: &'a [&'static str],
    values: &'a [Option<String>],
}
impl Serialize for Record<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(self.columns.len()))?;
        for (column, value) in self.columns.iter().zip(self.values) {
            map.serialize_entry(column, &value.as_deref())?;
        }
        map.end()
    }
}
impl Serialize for Records<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut seq = serializer.serialize_seq(Some(self.0.rows.len()))?;
        for values in self.0.rows.iter() {
            seq.serialize_element(&Record {
                columns: self.0.columns,
                values,
            })?;
        }
        seq.end()
    }
}
struct LimitedWriter<'a> {
    writer: &'a mut fs::File,
    count: u64,
    exceeded: bool,
    limit: u64,
    deadline: Instant,
}
impl Write for LimitedWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if Instant::now() >= self.deadline
            || bytes.len() as u64 > self.limit.saturating_sub(self.count)
        {
            self.exceeded = true;
            return Err(io::Error::new(
                io::ErrorKind::FileTooLarge,
                "Export exceeds its size limit.",
            ));
        }
        let count = self.writer.write(bytes)?;
        self.count += count as u64;
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }
}

fn unsafe_spreadsheet_text(value: &str) -> bool {
    value.starts_with(['\t', '\r', '\n'])
        || value
            .trim_start_matches(|c: char| c.is_whitespace() || c == '\u{feff}')
            .starts_with(['=', '+', '-', '@', '＝', '＋', '－', '＠'])
}
fn csv_cell(writer: &mut (impl Write + ?Sized), value: &str) -> io::Result<()> {
    writer.write_all(b"\"")?;
    for part in value.split_inclusive('"') {
        writer.write_all(part.as_bytes())?;
        if part.ends_with('"') {
            writer.write_all(b"\"")?;
        }
    }
    writer.write_all(b"\"")
}
fn csv(writer: &mut (impl Write + ?Sized), data: &ReadingsExport) -> io::Result<()> {
    for (index, column) in data.columns.iter().enumerate() {
        if index != 0 {
            writer.write_all(b",")?;
        }
        csv_cell(writer, column)?;
    }
    writer.write_all(b"\r\n")?;
    for row in data.rows.iter() {
        for (index, value) in row.iter().enumerate() {
            if index != 0 {
                writer.write_all(b",")?;
            }
            csv_cell(writer, value.as_deref().unwrap_or(""))?;
        }
        writer.write_all(b"\r\n")?;
    }
    Ok(())
}

pub fn create(
    directory: &Path,
    data: ReadingsExport,
    format: ExportFormat,
    identifiers_included: bool,
    exported_at: String,
) -> Result<ExportReceipt, CommandError> {
    if !(1..=10).contains(&data.profile_count)
        || data.rows.len() > 50_000
        || data.rows.iter().any(|row| row.len() != data.columns.len())
    {
        return Err(CommandError::ExportTooLarge);
    }
    // Quoting does not prevent formula execution. Refuse ambiguous CSV instead of altering values.
    // JSON retains original text. https://community.owasp.org/attacks/CSV_Injection
    if format == ExportFormat::Csv
        && data.rows.iter().any(|row| {
            row.iter().enumerate().any(|(index, value)| {
                value.as_ref().is_some_and(|value| {
                    let numeric_time = index == 6
                        && row.get(7).and_then(Option::as_deref) == Some("unix_seconds")
                        && value
                            .parse::<i64>()
                            .is_ok_and(|number| number.to_string() == *value);
                    !numeric_time && unsafe_spreadsheet_text(value)
                })
            })
        })
    {
        return Err(CommandError::ExportUnsafeText);
    }
    let (saved, ()) = publish(
        directory,
        "openusage-readings-",
        format.extension(),
        MAX_EXPORT_BYTES,
        |writer| {
            match format {
                ExportFormat::Csv => csv(writer, &data),
                ExportFormat::Json => serde_json::to_writer(
                    writer,
                    &Document {
                        schema_version: 1,
                        exported_at: &exported_at,
                        selection: Selection {
                            profile_count: data.profile_count,
                            identifiers_included,
                        },
                        records: Records(&data),
                    },
                )
                .map_err(io::Error::other),
            }
            .map_err(io_error)
        },
    )?;
    Ok(ExportReceipt {
        file_name: saved.file_name,
        directory: saved.directory,
        format,
        profile_count: data.profile_count,
        row_count: data.rows.len(),
        identifiers_included,
        exported_at,
    })
}

pub(crate) struct PublishedExport {
    pub file_name: String,
    pub directory: String,
    pub byte_count: u64,
}

pub(crate) fn io_error(error: io::Error) -> CommandError {
    if error.kind() == io::ErrorKind::StorageFull {
        CommandError::InsufficientSpace
    } else {
        CommandError::ExportUnavailable
    }
}

pub(crate) fn publish<T>(
    directory: &Path,
    prefix: &str,
    extension: &str,
    limit: u64,
    operation: impl FnOnce(&mut dyn Write) -> Result<T, CommandError>,
) -> Result<(PublishedExport, T), CommandError> {
    prepare_directory(directory).map_err(|_| CommandError::ExportUnavailable)?;
    let directory_name = directory
        .to_str()
        .ok_or(CommandError::ExportUnavailable)?
        .to_owned();
    let original_directory =
        fs::metadata(directory).map_err(|_| CommandError::ExportUnavailable)?;
    #[cfg(unix)]
    let held_directory = fs::File::open(directory).map_err(|_| CommandError::ExportUnavailable)?;
    if backup::available_space(directory).map_err(|_| CommandError::ExportUnavailable)?
        < limit.min(MAX_EXPORT_BYTES)
    {
        return Err(CommandError::InsufficientSpace);
    }
    let mut pending = tempfile::Builder::new()
        .prefix(prefix)
        .rand_bytes(16)
        .suffix(&format!(".{extension}.pending"))
        .tempfile_in(directory)
        .map_err(|_| CommandError::ExportUnavailable)?;
    let file_name = pending
        .path()
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.strip_suffix(".pending"))
        .ok_or(CommandError::ExportUnavailable)?
        .to_owned();
    let destination = directory.join(&file_name);
    let mut writer = LimitedWriter {
        writer: pending.as_file_mut(),
        count: 0,
        exceeded: false,
        limit,
        deadline: Instant::now() + Duration::from_secs(120),
    };
    let result = {
        let mut buffered = io::BufWriter::with_capacity(64 * 1024, &mut writer);
        let result = operation(&mut buffered);
        match result {
            Ok(value) => buffered.flush().map(|()| value).map_err(io_error),
            Err(error) => Err(error),
        }
    };
    if writer.exceeded {
        return Err(CommandError::ExportTooLarge);
    }
    let result = result?;
    let byte_count = writer.count;
    writer
        .flush()
        .map_err(|_| CommandError::ExportUnavailable)?;
    pending
        .as_file()
        .sync_all()
        .map_err(|_| CommandError::ExportUnavailable)?;
    backup::require_private_file(pending.as_file()).map_err(|_| CommandError::ExportUnavailable)?;
    prepare_directory(directory).map_err(|_| CommandError::ExportUnavailable)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let current = fs::metadata(directory).map_err(|_| CommandError::ExportUnavailable)?;
        let held = held_directory
            .metadata()
            .map_err(|_| CommandError::ExportUnavailable)?;
        if current.dev() != original_directory.dev()
            || current.ino() != original_directory.ino()
            || held.dev() != original_directory.dev()
            || held.ino() != original_directory.ino()
        {
            return Err(CommandError::ExportUnavailable);
        }
    }
    #[cfg(not(unix))]
    let _ = &original_directory;
    backup::publish_noclobber(pending.path(), &destination)
        .map_err(|_| CommandError::ExportUnavailable)?;
    pending.disable_cleanup(true);
    if backup::sync_directory(directory).is_err() {
        let _ = fs::remove_file(&destination);
        let _ = backup::sync_directory(directory);
        return Err(CommandError::ExportUnavailable);
    }
    Ok((
        PublishedExport {
            file_name,
            directory: directory_name,
            byte_count,
        },
        result,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use zeroize::Zeroizing;
    fn private_directory() -> tempfile::TempDir {
        let mut builder = tempfile::Builder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            builder.permissions(fs::Permissions::from_mode(0o700));
        }
        builder.tempdir().unwrap()
    }
    fn data(value: &str) -> ReadingsExport {
        ReadingsExport {
            columns: &["value", "status"],
            rows: Zeroizing::new(vec![
                vec![Some(value.into()), Some("available".into())],
                vec![None, Some("unavailable".into())],
            ]),
            profile_count: 1,
        }
    }
    fn when() -> String {
        "2026-10-20T00:00:00.000Z".into()
    }
    #[test]
    fn streamed_publication_removes_failed_and_oversized_pending_files() {
        let dir = private_directory();
        fs::write(dir.path().join("keep.json"), b"existing export").unwrap();
        let failed: Result<(PublishedExport, ()), _> =
            publish(dir.path(), "openusage-application-", "json", 32, |writer| {
                writer.write_all(b"partial").unwrap();
                Err(CommandError::ExportUnavailable)
            });
        assert!(matches!(failed, Err(CommandError::ExportUnavailable)));
        let oversized = publish(dir.path(), "openusage-application-", "json", 4, |writer| {
            writer.write_all(b"too large").map_err(io_error)
        });
        assert!(matches!(oversized, Err(CommandError::ExportTooLarge)));
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
        assert_eq!(
            fs::read(dir.path().join("keep.json")).unwrap(),
            b"existing export"
        );
    }
    #[test]
    fn files_preserve_exact_values_nulls_and_csv_escaping() {
        let dir = private_directory();
        let receipt = create(
            dir.path(),
            data("9007199254740993"),
            ExportFormat::Json,
            false,
            when(),
        )
        .unwrap();
        let json: serde_json::Value =
            serde_json::from_slice(&fs::read(dir.path().join(receipt.file_name)).unwrap()).unwrap();
        assert_eq!(json["records"][0]["value"], "9007199254740993");
        assert!(json["records"][1]["value"].is_null());
        assert_eq!(json["records"][1]["status"], "unavailable");
        assert_eq!(json["schemaVersion"], 1);
        let receipt = create(
            dir.path(),
            data("a,\"b\"\nnext"),
            ExportFormat::Csv,
            true,
            when(),
        )
        .unwrap();
        let path = dir.path().join(receipt.file_name);
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "\"value\",\"status\"\r\n\"a,\"\"b\"\"\nnext\",\"available\"\r\n\"\",\"unavailable\"\r\n"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::{MetadataExt, PermissionsExt};
            let m = fs::metadata(path).unwrap();
            assert_eq!(m.permissions().mode() & 0o777, 0o600);
            assert_eq!(m.nlink(), 1);
        }
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
    }
    #[test]
    fn unsafe_csv_refuses_before_any_file_and_json_preserves_the_original_text() {
        let dir = private_directory();
        for value in [
            "=1+2",
            " +formula",
            "-formula",
            "@formula",
            "\ttext",
            "\rtext",
            "\ntext",
            "\u{feff}＝1",
            "＋1",
            "－1",
            "＠text",
        ] {
            assert!(matches!(
                create(dir.path(), data(value), ExportFormat::Csv, true, when()),
                Err(CommandError::ExportUnsafeText)
            ));
        }
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
        let receipt = create(dir.path(), data("=1+2"), ExportFormat::Json, true, when()).unwrap();
        let json: serde_json::Value =
            serde_json::from_slice(&fs::read(dir.path().join(receipt.file_name)).unwrap()).unwrap();
        assert_eq!(json["records"][0]["value"], "=1+2");
    }
    #[test]
    fn canonical_negative_unix_times_remain_numbers_without_permitting_formula_text() {
        let dir = private_directory();
        let mut row = vec![None; 8];
        row[6] = Some("-62135596800".into());
        row[7] = Some("unix_seconds".into());
        let data = ReadingsExport {
            columns: &[
                "profile", "id", "label", "email", "kind", "metric", "value", "unit",
            ],
            rows: Zeroizing::new(vec![row]),
            profile_count: 1,
        };
        let receipt = create(dir.path(), data, ExportFormat::Csv, false, when()).unwrap();
        assert!(
            fs::read_to_string(dir.path().join(receipt.file_name))
                .unwrap()
                .contains("\"-62135596800\",\"unix_seconds\"")
        );
    }

    #[test]
    fn oversized_serialization_removes_the_private_pending_file() {
        let dir = private_directory();
        let large = "x".repeat(MAX_EXPORT_BYTES as usize);
        assert!(matches!(
            create(dir.path(), data(&large), ExportFormat::Json, false, when()),
            Err(CommandError::ExportTooLarge)
        ));
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
    }
    #[cfg(unix)]
    #[test]
    fn symlink_or_shared_destination_is_refused_without_writing_elsewhere() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let dir = private_directory();
        let target = private_directory();
        let link = dir.path().join("export");
        symlink(target.path(), &link).unwrap();
        assert!(matches!(
            create(&link, data("1"), ExportFormat::Json, false, when()),
            Err(CommandError::ExportUnavailable)
        ));
        assert_eq!(fs::read_dir(target.path()).unwrap().count(), 0);
        let shared = dir.path().join("shared");
        fs::create_dir(&shared).unwrap();
        fs::set_permissions(&shared, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(matches!(
            create(&shared, data("1"), ExportFormat::Json, false, when()),
            Err(CommandError::ExportUnavailable)
        ));
        assert_eq!(fs::read_dir(shared).unwrap().count(), 0);
    }
}
