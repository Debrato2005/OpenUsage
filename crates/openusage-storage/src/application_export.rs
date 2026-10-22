//! Lossless application records, without provider homes or external files.
use crate::{CURRENT_SCHEMA_VERSION, Storage, backup_validation};
use rusqlite::{Connection, TransactionBehavior, types::ValueRef};
use std::{
    fmt,
    io::{self, Write},
    time::{Duration, Instant},
};

const MAX_BYTES: u64 = 5 * 1024 * 1024 * 1024;
const MAX_DURATION: Duration = Duration::from_secs(120);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ApplicationExportSummary {
    pub schema_version: u32,
    pub table_count: u32,
    pub record_count: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ApplicationExportError {
    ReadFailed,
    InvalidData,
    TooLarge,
    WriteFailed(io::ErrorKind),
}

impl fmt::Display for ApplicationExportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::ReadFailed => "The application records could not be read.",
            Self::InvalidData => "The application records did not pass validation.",
            Self::TooLarge => "The application export exceeds the 5 GiB or 120-second limit.",
            Self::WriteFailed(_) => "The application export could not be written.",
        })
    }
}
impl std::error::Error for ApplicationExportError {}
type Result<T> = std::result::Result<T, ApplicationExportError>;

impl Storage {
    /// Stream every managed record from one validated WAL snapshot. The caller
    /// must publish the destination only after this method and its final sync succeed.
    pub fn write_application_export(
        &mut self,
        writer: &mut dyn Write,
        exported_at: &str,
    ) -> Result<ApplicationExportSummary> {
        if !crate::activity::activity_utc_timestamp(exported_at) {
            return Err(ApplicationExportError::InvalidData);
        }
        let started = Instant::now();
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(read_error)?;
        tx.progress_handler(10_000, Some(move || started.elapsed() >= MAX_DURATION))
            .map_err(read_error)?;
        let progress = Progress(&tx);
        let result = export_snapshot(&tx, writer, exported_at, started);
        // There is no previous handler on normal Storage connections. Restore that
        // state on failure too; a failed export must not poison later operations.
        let cleared = tx
            .progress_handler(0, None::<fn() -> bool>)
            .map_err(read_error);
        drop(progress);
        cleared?;
        if started.elapsed() >= MAX_DURATION {
            Err(ApplicationExportError::TooLarge)
        } else {
            result
        }
    }
}

struct Progress<'a>(&'a Connection);
impl Drop for Progress<'_> {
    fn drop(&mut self) {
        let _ = self.0.progress_handler(0, None::<fn() -> bool>);
    }
}

fn read_error(_: rusqlite::Error) -> ApplicationExportError {
    ApplicationExportError::ReadFailed
}

fn export_snapshot(
    connection: &Connection,
    writer: &mut dyn Write,
    exported_at: &str,
    started: Instant,
) -> Result<ApplicationExportSummary> {
    backup_validation::validate_schema_connection(connection, CURRENT_SCHEMA_VERSION, "main")
        .map_err(|error| match error {
            crate::BackupValidationError::Unavailable => ApplicationExportError::ReadFailed,
            _ => ApplicationExportError::InvalidData,
        })?;
    // Catalog equality above proves these names and declared types came from our
    // trusted migrations. Keep quoting and type checks as a local boundary too.
    let mut tables = connection
        .prepare("SELECT name FROM main.sqlite_schema WHERE type='table' ORDER BY name")
        .map_err(read_error)?;
    let names = tables
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(read_error)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(read_error)?;
    let table_count =
        u32::try_from(names.len()).map_err(|_| ApplicationExportError::InvalidData)?;
    let mut out = Output {
        writer,
        started,
        bytes: 0,
        too_large: false,
    };
    out.raw(
        b"{\"format\":\"openusage.application-records\",\"formatVersion\":1,\"schemaVersion\":",
    )?;
    out.raw(CURRENT_SCHEMA_VERSION.to_string().as_bytes())?;
    out.raw(b",\"exportedAt\":")?;
    out.json(exported_at)?;
    out.raw(b",\"integerEncoding\":\"decimal-string\",\"tables\":[")?;
    let mut record_count = 0u64;
    for (table_index, name) in names.iter().enumerate() {
        if !identifier(name) || name.starts_with("sqlite_") {
            return Err(ApplicationExportError::InvalidData);
        }
        let mut metadata = connection
            .prepare("SELECT name,type,hidden FROM pragma_table_xinfo(?1,'main') ORDER BY cid")
            .map_err(read_error)?;
        let columns = metadata
            .query_map([name], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            })
            .map_err(read_error)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(read_error)?;
        if columns.is_empty()
            || columns.iter().any(|(name, kind, hidden)| {
                !identifier(name) || !matches!(kind.as_str(), "TEXT" | "INTEGER") || *hidden != 0
            })
        {
            return Err(ApplicationExportError::InvalidData);
        }
        if table_index > 0 {
            out.raw(b",")?;
        }
        out.raw(b"{\"name\":")?;
        out.json(name)?;
        out.raw(b",\"columns\":[")?;
        for (index, (column, kind, _)) in columns.iter().enumerate() {
            if index > 0 {
                out.raw(b",")?;
            }
            out.raw(b"{\"name\":")?;
            out.json(column)?;
            out.raw(b",\"type\":")?;
            out.json(kind)?;
            out.raw(b"}")?;
        }
        out.raw(b"],\"rows\":[")?;
        // All current canonical tables have rowid. Scanning it avoids a potentially
        // unbounded sort or buffering a table merely to produce stable output.
        let mut statement = connection
            .prepare(&format!("SELECT * FROM main.\"{name}\" ORDER BY rowid"))
            .map_err(read_error)?;
        if statement.column_count() != columns.len() {
            return Err(ApplicationExportError::InvalidData);
        }
        let mut rows = statement.query([]).map_err(read_error)?;
        let mut first = true;
        while let Some(row) = rows.next().map_err(read_error)? {
            if !first {
                out.raw(b",")?;
            }
            first = false;
            out.raw(b"[")?;
            for (index, (_, kind, _)) in columns.iter().enumerate() {
                if index > 0 {
                    out.raw(b",")?;
                }
                match (kind.as_str(), row.get_ref(index).map_err(read_error)?) {
                    (_, ValueRef::Null) => out.raw(b"null")?,
                    ("INTEGER", ValueRef::Integer(value)) => out.json(&value.to_string())?,
                    ("TEXT", ValueRef::Text(value)) => out.json(
                        std::str::from_utf8(value)
                            .map_err(|_| ApplicationExportError::InvalidData)?,
                    )?,
                    _ => return Err(ApplicationExportError::InvalidData),
                }
            }
            out.raw(b"]")?;
            record_count = record_count
                .checked_add(1)
                .ok_or(ApplicationExportError::TooLarge)?;
        }
        out.raw(b"]}")?;
    }
    out.raw(b"]}")?;
    Ok(ApplicationExportSummary {
        schema_version: CURRENT_SCHEMA_VERSION,
        table_count,
        record_count,
    })
}

fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

struct Output<'a> {
    writer: &'a mut dyn Write,
    started: Instant,
    bytes: u64,
    too_large: bool,
}
impl Output<'_> {
    fn raw(&mut self, value: &[u8]) -> Result<()> {
        self.write_all(value)
            .map_err(|error| self.error(error.kind()))
    }
    fn json(&mut self, value: &str) -> Result<()> {
        serde_json::to_writer(&mut *self, value).map_err(|error| {
            if error.is_io() {
                self.error(error.io_error_kind().unwrap_or(io::ErrorKind::Other))
            } else {
                ApplicationExportError::InvalidData
            }
        })
    }
    fn error(&self, kind: io::ErrorKind) -> ApplicationExportError {
        if self.too_large {
            ApplicationExportError::TooLarge
        } else {
            ApplicationExportError::WriteFailed(kind)
        }
    }
}
impl Write for Output<'_> {
    fn write(&mut self, value: &[u8]) -> io::Result<usize> {
        if self.started.elapsed() >= MAX_DURATION {
            self.too_large = true;
            return Err(io::ErrorKind::TimedOut.into());
        }
        if (value.len() as u64) > MAX_BYTES.saturating_sub(self.bytes) {
            self.too_large = true;
            return Err(io::ErrorKind::Other.into());
        }
        let written = self.writer.write(value)?;
        self.bytes += written as u64;
        Ok(written)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Theme;
    use serde_json::{Value, json};
    const KEY: [u8; 32] = [0x6e; 32];
    const WHEN: &str = "2026-10-22T12:00:00.000Z";
    fn id(n: u32) -> String {
        format!("{n:08x}-0000-4000-8000-000000000000")
    }
    fn setup() -> (tempfile::TempDir, Storage) {
        let dir = crate::tests::private_directory();
        let mut storage = Storage::create(&dir.path().join("export.db"), &KEY).unwrap();
        storage
            .create_profile(&id(1), "Private archived account", &id(2))
            .unwrap();
        crate::backup_validation::activity_fixture(&mut storage, &id(1), &id(2), 100);
        storage.set_profile_archived(&id(1), true).unwrap();
        storage.set_theme(Theme::Dark).unwrap();
        storage.connection.execute_batch("INSERT INTO reset_alert_preferences VALUES(1,1,1,'Asia/Kolkata',1320,420); INSERT INTO reset_alert_overrides VALUES('00000001-0000-4000-8000-000000000000',0); INSERT INTO threshold_alert_preferences VALUES(1,1); INSERT INTO threshold_alert_overrides VALUES('00000001-0000-4000-8000-000000000000',1); INSERT INTO operational_alert_preferences VALUES('authentication_required',1),('collection_failure',0);").unwrap();
        (dir, storage)
    }
    fn exported(storage: &mut Storage) -> (ApplicationExportSummary, Value) {
        let mut bytes = Vec::new();
        let summary = storage.write_application_export(&mut bytes, WHEN).unwrap();
        (summary, serde_json::from_slice(&bytes).unwrap())
    }
    fn table<'a>(document: &'a Value, name: &str) -> &'a Value {
        document["tables"]
            .as_array()
            .unwrap()
            .iter()
            .find(|table| table["name"] == name)
            .unwrap()
    }
    fn bytes(dir: &std::path::Path) -> (Vec<u8>, Vec<u8>) {
        (
            std::fs::read(dir.join("export.db")).unwrap(),
            std::fs::read(dir.join("export.db-wal")).unwrap(),
        )
    }

    #[test]
    fn full_export_preserves_every_declared_field_archived_history_exact_counts_and_preferences() {
        let (dir, mut storage) = setup();
        let before = bytes(dir.path());
        let (summary, document) = exported(&mut storage);
        assert_eq!(document["format"], "openusage.application-records");
        assert_eq!(document["formatVersion"], 1);
        assert_eq!(document["schemaVersion"], crate::CURRENT_SCHEMA_VERSION);
        assert_eq!(document["exportedAt"], WHEN);
        assert_eq!(document["integerEncoding"], "decimal-string");
        assert_eq!(summary.table_count, 31);
        let mut records = 0;
        for table in document["tables"].as_array().unwrap() {
            let name = table["name"].as_str().unwrap();
            let mut statement = storage
                .connection
                .prepare(&format!("SELECT * FROM \"{name}\" ORDER BY rowid"))
                .unwrap();
            let column_count = statement.column_count();
            assert_eq!(table["columns"].as_array().unwrap().len(), column_count);
            let expected = statement
                .query_map([], |row| {
                    (0..column_count)
                        .map(|index| {
                            Ok(match row.get_ref(index)? {
                                ValueRef::Null => Value::Null,
                                ValueRef::Integer(value) => json!(value.to_string()),
                                ValueRef::Text(value) => json!(std::str::from_utf8(value).unwrap()),
                                _ => panic!("unsupported fixture value"),
                            })
                        })
                        .collect::<rusqlite::Result<Vec<_>>>()
                })
                .unwrap()
                .collect::<rusqlite::Result<Vec<_>>>()
                .unwrap();
            assert_eq!(table["rows"], json!(expected), "{name}");
            for (index, column) in table["columns"].as_array().unwrap().iter().enumerate() {
                assert_eq!(column["name"], statement.column_name(index).unwrap());
                assert!(matches!(column["type"].as_str(), Some("TEXT" | "INTEGER")));
            }
            records += expected.len() as u64;
        }
        assert_eq!(summary.record_count, records);
        assert_eq!(
            table(&document, "profile_identity_events")["rows"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        let summaries = table(&document, "activity_summaries")["rows"]
            .as_array()
            .unwrap();
        let cells = summaries
            .iter()
            .flat_map(|row| row.as_array().unwrap())
            .collect::<Vec<_>>();
        assert!(cells.contains(&&json!("9223372036854775807")));
        assert!(cells.contains(&&json!("9007199254740993")));
        assert!(cells.contains(&&json!("0")));
        assert!(cells.contains(&&Value::Null));
        assert_eq!(table(&document, "profiles")["rows"][0][6], "1");
        assert_eq!(table(&document, "settings")["rows"][0][3], "dark");
        assert_eq!(before, bytes(dir.path()));
        drop(storage);
        let mut reopened = Storage::open(&dir.path().join("export.db"), &KEY).unwrap();
        let (again, saved) = exported(&mut reopened);
        assert_eq!(summary, again);
        assert_eq!(document, saved);
    }

    #[test]
    fn concurrent_wal_commit_cannot_mix_labels_settings_or_records_into_pinned_export() {
        let (dir, mut storage) = setup();
        let mut other = Storage::open(&dir.path().join("export.db"), &KEY).unwrap();
        let (begin_tx, begin_rx) = std::sync::mpsc::channel();
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let thread = std::thread::spawn(move || {
            begin_rx.recv_timeout(Duration::from_secs(10)).unwrap();
            other
                .rename_profile(&id(1), "Concurrent new label")
                .unwrap();
            other.set_theme(Theme::Light).unwrap();
            done_tx.send(()).unwrap();
        });
        struct ConcurrentWriter {
            bytes: Vec<u8>,
            begin: Option<std::sync::mpsc::Sender<()>>,
            done: std::sync::mpsc::Receiver<()>,
        }
        impl Write for ConcurrentWriter {
            fn write(&mut self, value: &[u8]) -> io::Result<usize> {
                if let Some(begin) = self.begin.take() {
                    begin.send(()).unwrap();
                    self.done.recv_timeout(Duration::from_secs(10)).unwrap();
                }
                self.bytes.write(value)
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let mut writer = ConcurrentWriter {
            bytes: vec![],
            begin: Some(begin_tx),
            done: done_rx,
        };
        storage.write_application_export(&mut writer, WHEN).unwrap();
        thread.join().unwrap();
        let document: Value = serde_json::from_slice(&writer.bytes).unwrap();
        assert_eq!(
            table(&document, "profiles")["rows"][0][2],
            "Private archived account"
        );
        assert_eq!(table(&document, "settings")["rows"][0][3], "dark");
        let (_, current) = exported(&mut storage);
        assert_eq!(
            table(&current, "profiles")["rows"][0][2],
            "Concurrent new label"
        );
        assert_eq!(table(&current, "settings")["rows"][0][3], "light");
    }

    #[test]
    fn write_and_midstream_read_failures_leave_database_bytes_and_future_operations_intact() {
        let (dir, mut storage) = setup();
        let before = bytes(dir.path());
        struct Failure;
        impl Write for Failure {
            fn write(&mut self, _: &[u8]) -> io::Result<usize> {
                Err(io::ErrorKind::StorageFull.into())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        assert_eq!(
            storage.write_application_export(&mut Failure, WHEN),
            Err(ApplicationExportError::WriteFailed(
                io::ErrorKind::StorageFull
            ))
        );
        assert_eq!(before, bytes(dir.path()));
        let deny = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let check = deny.clone();
        storage
            .connection
            .authorizer(Some(move |context: rusqlite::hooks::AuthContext<'_>| {
                if check.load(std::sync::atomic::Ordering::SeqCst)
                    && matches!(context.action, rusqlite::hooks::AuthAction::Read { .. })
                {
                    rusqlite::hooks::Authorization::Deny
                } else {
                    rusqlite::hooks::Authorization::Allow
                }
            }))
            .unwrap();
        struct ReadFailure(std::sync::Arc<std::sync::atomic::AtomicBool>);
        impl Write for ReadFailure {
            fn write(&mut self, value: &[u8]) -> io::Result<usize> {
                self.0.store(true, std::sync::atomic::Ordering::SeqCst);
                Ok(value.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        assert_eq!(
            storage.write_application_export(&mut ReadFailure(deny), WHEN),
            Err(ApplicationExportError::ReadFailed)
        );
        storage
            .connection
            .authorizer(
                None::<fn(rusqlite::hooks::AuthContext<'_>) -> rusqlite::hooks::Authorization>,
            )
            .unwrap();
        assert_eq!(before, bytes(dir.path()));
        exported(&mut storage);
        storage.set_theme(Theme::Light).unwrap();
    }

    #[test]
    fn invalid_catalog_or_schema_never_starts_output() {
        let (_dir, mut storage) = setup();
        let mut output = vec![];
        assert_eq!(
            storage.write_application_export(&mut output, "invalid date"),
            Err(ApplicationExportError::InvalidData)
        );
        storage
            .connection
            .execute_batch("CREATE TABLE unexpected(secret TEXT) STRICT;")
            .unwrap();
        assert_eq!(
            storage.write_application_export(&mut output, WHEN),
            Err(ApplicationExportError::InvalidData)
        );
        assert!(output.is_empty());
        storage
            .connection
            .execute_batch(&format!(
                "DROP TABLE unexpected; PRAGMA user_version={};",
                CURRENT_SCHEMA_VERSION + 1
            ))
            .unwrap();
        assert_eq!(
            storage.write_application_export(&mut output, WHEN),
            Err(ApplicationExportError::InvalidData)
        );
        assert!(output.is_empty());
    }

    #[test]
    fn complete_export_has_no_selected_readings_fifty_thousand_row_limit() {
        let dir = crate::tests::private_directory();
        let mut storage = Storage::create(&dir.path().join("export.db"), &KEY).unwrap();
        let tx = storage.connection.transaction().unwrap();
        {
            let mut insert = tx.prepare("INSERT INTO profiles(id,name,environment,approved_home_reference,connection_state,archived,created_at,updated_at) VALUES(?1,'Archived synthetic','native',?1,'authentication_required',1,?2,?2)").unwrap();
            for n in 1..=50_001 {
                insert.execute(rusqlite::params![id(n), WHEN]).unwrap();
            }
        }
        tx.commit().unwrap();
        let summary = storage
            .write_application_export(&mut io::sink(), WHEN)
            .unwrap();
        assert_eq!(summary.table_count, 31);
        assert_eq!(summary.record_count, 50_002);
    }
}
