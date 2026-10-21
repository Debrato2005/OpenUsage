//! Test-only, read-only oracle for synthetic activity and quota records.
//! Never opens Storage, migrates an input, or writes plaintext database bytes to disk.
use openusage_storage::keywrap::unwrap_key;
use rusqlite::{Connection, OpenFlags, types::ValueRef};
use serde_json::{Map, Value, json};
use std::{
    env, fs,
    io::{self, Cursor, Read},
    path::{Path, PathBuf},
};
use zeroize::Zeroizing;

fn private_bytes(path: &Path) -> Vec<u8> {
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
    let mut file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .expect("Cannot read the synthetic private file.");
    let metadata = file.metadata().expect("Cannot check the synthetic file.");
    assert!(
        metadata.is_file()
            && metadata.uid() == unsafe { libc::geteuid() }
            && metadata.mode() & 0o777 == 0o600
            && metadata.nlink() == 1,
        "Unsafe synthetic private file."
    );
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .expect("Cannot read synthetic file bytes.");
    bytes
}
fn sidecar(path: &Path, suffix: &str) -> PathBuf {
    let mut value = path.as_os_str().to_owned();
    value.push(suffix);
    PathBuf::from(value)
}
fn free_bytes(path: &Path) -> u64 {
    use std::os::unix::ffi::OsStrExt;
    let name = std::ffi::CString::new(path.as_os_str().as_bytes()).unwrap();
    let mut value = std::mem::MaybeUninit::<libc::statvfs>::uninit();
    assert_eq!(
        unsafe { libc::statvfs(name.as_ptr(), value.as_mut_ptr()) },
        0
    );
    let value = unsafe { value.assume_init() };
    value.f_bavail.checked_mul(value.f_frsize).unwrap()
}
fn optional_bytes(path: &Path) -> Option<Vec<u8>> {
    match fs::symlink_metadata(path) {
        Ok(_) => Some(private_bytes(path)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(_) => panic!("Cannot check the synthetic sidecar."),
    }
}
fn constrain(connection: &Connection) {
    connection
        .execute_batch(
            "PRAGMA cipher_memory_security=ON; PRAGMA temp_store=MEMORY; PRAGMA query_only=ON;",
        )
        .expect("Cannot constrain the oracle.");
    let version: String = connection
        .query_row("PRAGMA cipher_version", [], |r| r.get(0))
        .unwrap();
    assert!(version.starts_with("4."));
    assert!([2, 3, 4, 5, 6].contains(&schema_version(connection)));
    assert_eq!(
        connection
            .query_row("PRAGMA integrity_check", [], |r| r.get::<_, String>(0))
            .unwrap(),
        "ok"
    );
    assert!(
        !connection
            .prepare("PRAGMA foreign_key_check")
            .unwrap()
            .exists([])
            .unwrap()
    );
}
fn schema_version(connection: &Connection) -> u32 {
    connection
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap()
}
fn state(connection: &Connection, include_profile_state: bool) -> Value {
    let mut tables = Map::new();
    let mut selected = vec![
        ("profile_identity_events", "id"),
        ("collection_runs", "id"),
        ("activity_summaries", "id"),
        ("current_activity", "profile_id"),
        (
            "current_activity_days",
            "profile_id,provider_date,source_scope",
        ),
        ("activity_day_history", "id"),
    ];
    if schema_version(connection) >= 3 {
        selected.extend([
            ("profile_bindings", "profile_id"),
            ("quota_snapshots", "id"),
            ("quota_limits", "collection_run_id,limit_id"),
            ("credit_snapshots", "collection_run_id,ordinal"),
            ("current_quota_readings", "profile_id"),
            ("current_quota", "profile_id,limit_id,window_role"),
        ]);
    }
    if schema_version(connection) >= 4 {
        selected.push((
            "quota_reset_assessments",
            "collection_run_id,limit_id,window_role",
        ));
    }
    if schema_version(connection) >= 5 {
        selected.push(("alert_events", "id"));
    }
    if schema_version(connection) >= 6 {
        selected.extend([
            ("reset_alert_preferences", "id"),
            ("reset_alert_overrides", "profile_id"),
            ("reset_notification_outbox", "event_id"),
            ("reset_notification_receipts", "event_key"),
        ]);
    }
    if include_profile_state {
        selected.extend([("profiles", "id"), ("settings", "id"), ("accounts", "id")]);
    }
    for (table, order) in selected {
        // Both identifiers and ordering expressions are fixed trusted test source, never request data.
        let mut statement = connection
            .prepare(&format!("SELECT * FROM {table} ORDER BY {order}"))
            .expect("Missing persistent record table.");
        let names = statement
            .column_names()
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>();
        let mut query = statement.query([]).unwrap();
        let mut records = Vec::new();
        while let Some(row) = query.next().unwrap() {
            let mut record = Map::new();
            for (index, name) in names.iter().enumerate() {
                let value = match row.get_ref(index).unwrap() {
                    ValueRef::Null => Value::Null,
                    ValueRef::Integer(value) => json!(value),
                    ValueRef::Text(value) => Value::String(
                        std::str::from_utf8(value)
                            .expect("Invalid activity text.")
                            .into(),
                    ),
                    _ => panic!("Unexpected activity field encoding."),
                };
                record.insert(name.clone(), value);
            }
            records.push(Value::Object(record));
        }
        tables.insert(table.into(), Value::Array(records));
    }
    let profiles=connection.prepare("SELECT id,approved_home_reference,archived FROM profiles ORDER BY id").unwrap().query_map([],|r|Ok(json!({"id":r.get::<_,String>(0)?,"homeReference":r.get::<_,String>(1)?,"archived":r.get::<_,bool>(2)?}))).unwrap().collect::<Result<Vec<_>,_>>().unwrap();
    let monitoring: String = connection
        .query_row("SELECT monitoring FROM settings WHERE id=1", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(monitoring, "stopped");
    let serialized = serde_json::to_string(&tables).unwrap();
    for forbidden in [
        "synthetic-private-activity-id",
        "workTokens",
        "modelCounts",
        "synthetic-private-model",
        "threadUsage",
    ] {
        assert!(
            !serialized.contains(forbidden),
            "Private provider-only field reached persistent records."
        );
    }
    let accounts: i64 = connection
        .query_row("SELECT count(*) FROM accounts", [], |r| r.get(0))
        .unwrap();
    json!({"tables":tables,"profiles":profiles,"monitoring":monitoring,"accounts":accounts})
}
fn main() {
    let args = env::args_os().collect::<Vec<_>>();
    assert!(
        args.len() == 4 || args.len() == 5,
        "Supply database|checkpoint|backup, synthetic input, protected-key path, and optional --profile-state."
    );
    let include_profile_state = args.len() == 5;
    if include_profile_state {
        assert_eq!(args[4], "--profile-state", "Unknown synthetic oracle flag.");
    }
    let mode = args[1].to_str().unwrap();
    assert!(["database", "checkpoint", "backup"].contains(&mode));
    assert!(
        !include_profile_state || mode == "database",
        "Profile-state verification requires database mode."
    );
    let input_path = Path::new(&args[2]);
    let envelope_path = Path::new(&args[3]);
    let original = private_bytes(input_path);
    let envelope = private_bytes(envelope_path);
    let wal_before = optional_bytes(&sidecar(input_path, "-wal"));
    let shm_before = optional_bytes(&sidecar(input_path, "-shm"));
    let mut input = Zeroizing::new(Vec::new());
    io::stdin()
        .take(1024 * 1024)
        .read_to_end(&mut input)
        .unwrap();
    let mut request: Value =
        serde_json::from_slice(&input).expect("Invalid synthetic oracle request.");
    let passphrase = match request
        .get_mut("passphrase")
        .expect("Missing synthetic passphrase.")
        .take()
    {
        Value::String(value) => Zeroizing::new(value),
        _ => panic!("Invalid synthetic passphrase."),
    };
    let key = unwrap_key(&envelope, passphrase.as_bytes())
        .expect("Cannot unlock synthetic protected key.");
    let mut payload = Zeroizing::new(Vec::new());
    let connection = if mode == "backup" {
        let password = match request
            .get_mut("backupPassword")
            .expect("Missing synthetic archive password.")
            .take()
        {
            Value::String(value) => Zeroizing::new(value),
            _ => panic!("Invalid synthetic archive password."),
        };
        let stage = input_path.parent().unwrap();
        let mut verified = openusage_import::prepare_restore(
            &mut Cursor::new(&original),
            password.as_bytes(),
            stage,
            free_bytes(stage),
        )
        .expect("Synthetic archive did not authenticate.");
        assert!([2, 3, 4, 5, 6].contains(&verified.manifest().schema_version));
        assert_eq!(verified.manifest().entries.len(), 1);
        assert_eq!(verified.manifest().entries[0].name, "records.sqlite");
        verified.copy_payload(&mut *payload).unwrap();
        drop(verified);
        let mut connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch("PRAGMA cipher_memory_security=ON; PRAGMA temp_store=MEMORY;")
            .unwrap();
        connection
            .deserialize_read_exact("main", Cursor::new(payload.as_slice()), payload.len(), true)
            .expect("Cannot read authenticated records in memory.");
        connection
    } else {
        assert!(!original.starts_with(b"SQLite format 3\0"));
        // No immutable mode: the derived live copy must incorporate any existing WAL records.
        let connection = Connection::open_with_flags(
            input_path,
            OpenFlags::SQLITE_OPEN_READ_ONLY
                | OpenFlags::SQLITE_OPEN_NO_MUTEX
                | OpenFlags::SQLITE_OPEN_NOFOLLOW,
        )
        .unwrap();
        let mut encoded = Zeroizing::new(String::from("x'"));
        for byte in key.iter() {
            use std::fmt::Write;
            write!(&mut *encoded, "{byte:02x}").unwrap();
        }
        encoded.push('\'');
        connection
            .pragma_update(None, "key", encoded.as_str())
            .unwrap();
        drop(encoded);
        connection
    };
    constrain(&connection);
    if mode != "backup" {
        assert!(
            !connection
                .prepare("PRAGMA cipher_integrity_check")
                .unwrap()
                .exists([])
                .unwrap()
        );
    }
    if mode == "database" {
        assert_eq!(
            connection
                .pragma_query_value(None, "journal_mode", |r| r.get::<_, String>(0))
                .unwrap(),
            "wal"
        );
    }
    let schema_version = schema_version(&connection);
    if include_profile_state {
        assert!(
            [4, 5, 6].contains(&schema_version),
            "Profile-state verification requires schema 4, 5 or 6."
        );
    }
    let actual = state(&connection, include_profile_state);
    if let Some(expected) = request.get("expectedTables") {
        assert_eq!(
            &actual["tables"], expected,
            "Persistent record fields changed."
        );
    }
    drop(connection);
    drop(payload);
    drop(key);
    assert!(
        private_bytes(input_path) == original,
        "The oracle changed encrypted input bytes."
    );
    assert!(
        private_bytes(envelope_path) == envelope,
        "The oracle changed protected-key bytes."
    );
    let wal_after = optional_bytes(&sidecar(input_path, "-wal"));
    let empty_wal_created = wal_before.is_none() && wal_after.as_ref().is_some_and(Vec::is_empty);
    assert!(
        wal_after == wal_before || (mode == "database" && empty_wal_created),
        "The oracle changed WAL records."
    );
    if mode == "checkpoint" {
        for suffix in ["-wal", "-shm", "-journal"] {
            assert!(
                fs::symlink_metadata(sidecar(input_path, suffix)).is_err(),
                "A standalone checkpoint gained a sidecar."
            );
        }
    }
    println!(
        "{}",
        json!({"verified":true,"readOnly":true,"mode":mode,"schemaVersion":schema_version,"encryptedPagesAuthenticated":mode!="backup","archiveAuthenticated":mode=="backup","originalProtectedKeyUsed":mode!="backup","plaintextDiskStaging":false,"inputUnchanged":true,"keyEnvelopeUnchanged":true,"walRecordsUnchanged":true,"emptyWalSidecarCreated":empty_wal_created,"sharedMemoryMetadataChanged":optional_bytes(&sidecar(input_path,"-shm"))!=shm_before,"state":actual})
    );
}
