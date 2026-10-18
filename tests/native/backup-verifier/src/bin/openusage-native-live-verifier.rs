use openusage_storage::keywrap::unwrap_key;
use rusqlite::{Connection, OpenFlags};
use serde_json::{Value, json};
use std::{
    env, fs,
    io::{self, Read},
    path::Path,
};
use zeroize::Zeroizing;

fn main() {
    let args: Vec<_> = env::args_os().collect();
    assert_eq!(
        args.len(),
        3,
        "Supply a synthetic database and its wrapped database key."
    );
    let database_path = Path::new(&args[1]);
    let envelope_path = Path::new(&args[2]);
    let sidecar =
        |suffix: &str| std::path::PathBuf::from(format!("{}{suffix}", database_path.display()));
    let wal_before = fs::read(sidecar("-wal")).ok();
    let shm_before = fs::read(sidecar("-shm")).ok();
    let before = fs::read(database_path).expect("Cannot read the encrypted database.");
    assert!(!before.starts_with(b"SQLite format 3\0"));
    let envelope = fs::read(envelope_path).expect("Cannot read the synthetic key envelope.");
    let mut input = Zeroizing::new(Vec::new());
    io::stdin()
        .take(64 * 1024)
        .read_to_end(&mut input)
        .expect("Cannot read the synthetic request.");
    let mut request: Value = serde_json::from_slice(&input).expect("Invalid synthetic request.");
    let passphrase = match request
        .get_mut("passphrase")
        .expect("Missing synthetic passphrase.")
        .take()
    {
        Value::String(value) => Zeroizing::new(value),
        _ => panic!("Invalid synthetic passphrase."),
    };
    let key = unwrap_key(&envelope, passphrase.as_bytes())
        .expect("Cannot unlock the synthetic key envelope.");
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY
        | OpenFlags::SQLITE_OPEN_NO_MUTEX
        | OpenFlags::SQLITE_OPEN_NOFOLLOW;
    let database = Connection::open_with_flags(database_path, flags)
        .expect("Cannot open the database read-only.");
    let mut encoded = Zeroizing::new(String::from("x'"));
    for byte in key.iter() {
        use std::fmt::Write;
        write!(&mut *encoded, "{byte:02x}").expect("Cannot encode the synthetic key.");
    }
    encoded.push('\'');
    database
        .pragma_update(None, "key", encoded.as_str())
        .expect("Cannot assign the database key.");
    drop(encoded);
    database
        .execute_batch(
            "PRAGMA cipher_memory_security=ON; PRAGMA temp_store=MEMORY; PRAGMA query_only=ON;",
        )
        .expect("Cannot constrain the read-only verifier.");
    let cipher: String = database
        .query_row("PRAGMA cipher_version", [], |row| row.get(0))
        .expect("SQLCipher is unavailable.");
    assert!(cipher.starts_with("4."));
    let version: u32 = database
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("Missing schema version.");
    assert_eq!(version, 1);
    let integrity: String = database
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .expect("Database integrity failed.");
    assert_eq!(integrity, "ok");
    assert!(
        !database
            .prepare("PRAGMA foreign_key_check")
            .expect("Cannot check relations.")
            .exists([])
            .expect("Cannot check relations.")
    );
    assert!(
        !database
            .prepare("PRAGMA cipher_integrity_check")
            .expect("Cannot authenticate encrypted pages.")
            .exists([])
            .expect("Cannot authenticate encrypted pages.")
    );
    let journal: String = database
        .pragma_query_value(None, "journal_mode", |row| row.get(0))
        .expect("Missing journal mode.");
    assert_eq!(journal, "wal");
    let theme: String = database
        .query_row("SELECT theme FROM settings WHERE id=1", [], |row| {
            row.get(0)
        })
        .expect("Missing theme.");
    let accounts: i64 = database
        .query_row("SELECT count(*) FROM accounts", [], |row| row.get(0))
        .expect("Missing accounts.");
    let monitoring: String = database
        .query_row("SELECT monitoring FROM settings WHERE id=1", [], |row| {
            row.get(0)
        })
        .expect("Missing monitoring.");
    assert_eq!(monitoring, "stopped");
    let home_references: Vec<String> = database
        .prepare("SELECT approved_home_reference FROM profiles ORDER BY id")
        .expect("Missing home references.")
        .query_map([], |row| row.get(0))
        .expect("Cannot read home references.")
        .collect::<Result<_, _>>()
        .expect("Cannot read home references.");
    let mut unique = std::collections::HashSet::new();
    let forbidden = request["forbiddenHomes"]
        .as_array()
        .expect("Missing forbidden synthetic homes.");
    for reference in &home_references {
        assert!(reference.len() == 36, "Invalid managed reference.");
        let id = reference;
        assert!(
            id.bytes()
                .enumerate()
                .all(|(i, b)| if [8, 13, 18, 23].contains(&i) {
                    b == b'-'
                } else {
                    b.is_ascii_digit() || (b'a'..=b'f').contains(&b)
                }),
            "Invalid home UUID."
        );
        assert!(unique.insert(reference.clone()), "Duplicate managed home.");
        assert!(
            !forbidden
                .iter()
                .any(|value| value.as_str() == Some(reference)),
            "Restored records reused an old credential home."
        );
        let home_root = request["homeRoot"]
            .as_str()
            .map(Path::new)
            .unwrap_or_else(|| {
                database_path
                    .parent()
                    .expect("Missing synthetic workspace.")
            });
        let path = home_root.join("profiles").join(reference);
        let metadata = fs::symlink_metadata(&path).expect("Missing restored home.");
        use std::os::unix::fs::MetadataExt;
        assert!(
            metadata.is_dir()
                && !metadata.file_type().is_symlink()
                && metadata.mode() & 0o777 == 0o700
                && metadata.uid() == unsafe { libc::geteuid() },
            "Unsafe restored home."
        );
        assert!(
            fs::read_dir(path)
                .expect("Cannot read restored home.")
                .next()
                .is_none(),
            "Restored credential home is not empty."
        );
    }
    let profiles = database.prepare("SELECT id,name,archived,environment,connection_state FROM profiles ORDER BY id").expect("Missing profiles.").query_map([], |row| Ok(json!({"id":row.get::<_,String>(0)?,"label":row.get::<_,String>(1)?,"archived":row.get::<_,bool>(2)?,"environment":row.get::<_,String>(3)?,"connectionState":row.get::<_,String>(4)?}))).expect("Cannot read profiles.").collect::<Result<Vec<_>,_>>().expect("Cannot read profiles.");
    let actual = json!({"theme":theme,"profiles":profiles,"accounts":accounts.to_string()});
    assert!(
        actual == request["expected"],
        "Database records differ from the expected synthetic state."
    );
    drop(database);
    drop(key);
    assert!(
        fs::read(database_path).expect("Cannot recheck database.") == before,
        "The read-only verifier changed the database."
    );
    assert!(
        fs::read(envelope_path).expect("Cannot recheck key envelope.") == envelope,
        "The verifier changed the key envelope."
    );
    let wal_after = fs::read(sidecar("-wal")).ok();
    let wal_unchanged = wal_after == wal_before;
    let empty_wal_created = wal_before.is_none() && wal_after.as_ref().is_some_and(Vec::is_empty);
    assert!(
        wal_unchanged || (request["allowEmptyWalCreation"] == true && empty_wal_created),
        "The read-only verifier changed WAL records."
    );
    let shm_changed = fs::read(sidecar("-shm")).ok() != shm_before;
    println!(
        "{}",
        json!({"verified":true,"readOnly":true,"schemaVersion":1,"exactStateMatched":true,"originalProtectedKeyUsed":true,"plaintextDiskStaging":false,"walUnchanged":wal_unchanged,"walRecordsUnchanged":true,"emptyWalSidecarCreated":empty_wal_created,"sharedMemoryMetadataChanged":shm_changed,"freshPrivateEmptyHomes":home_references.len(),"monitoringStopped":true})
    );
}
