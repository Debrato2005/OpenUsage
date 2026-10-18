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
        "Supply a synthetic checkpoint and its wrapped database key."
    );
    let checkpoint = Path::new(&args[1]);
    let envelope_path = Path::new(&args[2]);
    let before = fs::read(checkpoint).expect("Cannot read the encrypted checkpoint.");
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
    let database = Connection::open_with_flags(checkpoint, flags)
        .expect("Cannot open the checkpoint read-only.");
    let mut encoded = Zeroizing::new(String::from("x'"));
    for byte in key.iter() {
        use std::fmt::Write;
        write!(&mut *encoded, "{byte:02x}").expect("Cannot encode the synthetic key.");
    }
    encoded.push('\'');
    database
        .pragma_update(None, "key", encoded.as_str())
        .expect("Cannot assign the checkpoint key.");
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
        .expect("Checkpoint integrity failed.");
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
            .expect("Cannot authenticate checkpoint pages.")
            .exists([])
            .expect("Cannot authenticate checkpoint pages.")
    );
    let theme: String = database
        .query_row("SELECT theme FROM settings WHERE id=1", [], |row| {
            row.get(0)
        })
        .expect("Missing theme.");
    let accounts: i64 = database
        .query_row("SELECT count(*) FROM accounts", [], |row| row.get(0))
        .expect("Missing accounts.");
    let profiles = database.prepare("SELECT id,name,archived,environment,connection_state FROM profiles ORDER BY id").expect("Missing profiles.").query_map([], |row| Ok(json!({"id":row.get::<_,String>(0)?,"label":row.get::<_,String>(1)?,"archived":row.get::<_,bool>(2)?,"environment":row.get::<_,String>(3)?,"connectionState":row.get::<_,String>(4)?}))).expect("Cannot read profiles.").collect::<Result<Vec<_>,_>>().expect("Cannot read profiles.");
    let actual = json!({"theme":theme,"profiles":profiles,"accounts":accounts.to_string()});
    assert!(
        actual == request["expected"],
        "Checkpoint records differ from the expected synthetic state."
    );
    drop(database);
    drop(key);
    assert!(
        fs::read(checkpoint).expect("Cannot recheck checkpoint.") == before,
        "The read-only verifier changed the checkpoint."
    );
    assert!(
        fs::read(envelope_path).expect("Cannot recheck key envelope.") == envelope,
        "The verifier changed the key envelope."
    );
    for suffix in ["-wal", "-shm", "-journal"] {
        assert!(
            !Path::new(&format!("{}{suffix}", checkpoint.display())).exists(),
            "The verifier left a checkpoint sidecar."
        );
    }
    println!(
        "{}",
        json!({"verified":true,"readOnly":true,"schemaVersion":1,"exactStateMatched":true,"selectedSemanticFieldsMatched":true,"stateMatchScope":"Theme, account count, profile IDs, labels, archival flags, environment, and connection state. Not every column or timestamp.","originalProtectedKeyUsed":true,"plaintextDiskStaging":false,"encryptedPagesAuthenticated":true})
    );
}
