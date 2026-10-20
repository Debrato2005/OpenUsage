use openusage_import::{BackupError, prepare_restore};
use rusqlite::Connection;
use serde_json::{Value, json};
use std::{
    env,
    ffi::CString,
    fs::File,
    io::{self, Cursor, Read},
    path::Path,
};
use zeroize::Zeroizing;

fn free_bytes(path: &Path) -> u64 {
    use std::os::unix::ffi::OsStrExt;
    let path = CString::new(path.as_os_str().as_bytes()).expect("Invalid private staging path.");
    let mut result = std::mem::MaybeUninit::<libc::statvfs>::uninit();
    // statvfs initializes the structure on success. The C path stays alive for the call.
    assert_eq!(
        unsafe { libc::statvfs(path.as_ptr(), result.as_mut_ptr()) },
        0
    );
    // A successful statvfs call initializes every structure field.
    let result = unsafe { result.assume_init() };
    result
        .f_bavail
        .checked_mul(result.f_frsize)
        .expect("Disk-space overflow.")
}

fn rejected(result: Result<openusage_import::VerifiedArchive, BackupError>) {
    assert!(matches!(
        result,
        Err(BackupError::AuthenticationFailed | BackupError::InvalidArchive)
    ));
}

fn main() {
    let args: Vec<_> = env::args_os().collect();
    assert_eq!(
        args.len(),
        4,
        "Supply archive, private staging directory, and expected theme."
    );
    let archive = Path::new(&args[1]);
    let staging = Path::new(&args[2]);
    let expected_theme = args[3].to_str().expect("Invalid expected theme.");
    let available = free_bytes(staging);
    let mut input = Zeroizing::new(Vec::new());
    io::stdin()
        .take(64 * 1024 + 1)
        .read_to_end(&mut input)
        .expect("Cannot read the synthetic request.");
    assert!(input.len() <= 64 * 1024, "Synthetic request is too large.");
    let mut request: Value = serde_json::from_slice(&input).expect("Invalid synthetic request.");
    let expected_schema_version = request
        .get("expectedSchemaVersion")
        .and_then(Value::as_u64)
        .and_then(|version| u32::try_from(version).ok())
        .expect("Missing or invalid expected schema version.");
    let password = match request
        .get_mut("password")
        .expect("Missing synthetic password.")
        .take()
    {
        Value::String(value) => Zeroizing::new(value),
        _ => panic!("Invalid synthetic password."),
    };
    assert!(
        !password.is_empty() && password.len() <= 1024,
        "Invalid synthetic password size."
    );

    rejected(prepare_restore(
        &mut File::open(archive).expect("Cannot read the archive."),
        b"A wrong synthetic backup password",
        staging,
        available,
    ));
    let mut tampered = std::fs::read(archive).expect("Cannot read the archive.");
    *tampered.last_mut().expect("The archive is empty.") ^= 1;
    rejected(prepare_restore(
        &mut Cursor::new(tampered),
        password.as_bytes(),
        staging,
        available,
    ));

    let mut verified = prepare_restore(
        &mut File::open(archive).expect("Cannot read the archive."),
        password.as_bytes(),
        staging,
        available,
    )
    .expect("The exported archive did not authenticate.");
    assert_eq!(
        verified.manifest().schema_version,
        expected_schema_version,
        "Unexpected manifest schema version."
    );
    assert_eq!(verified.manifest().entries.len(), 1);
    assert_eq!(verified.manifest().entries[0].name, "records.sqlite");
    let expected_size = verified.manifest().entries[0].size;
    let mut payload = Zeroizing::new(Vec::new());
    assert_eq!(
        verified
            .copy_payload(&mut *payload)
            .expect("Cannot read authenticated records."),
        expected_size
    );
    drop(verified);
    assert!(payload.starts_with(b"SQLite format 3\0"));
    let mut database =
        Connection::open_in_memory().expect("Cannot create an in-memory verification database.");
    database
        .pragma_update(None, "cipher_memory_security", true)
        .expect("Cannot protect SQLite memory.");
    database
        .pragma_update(None, "temp_store", "MEMORY")
        .expect("Cannot constrain temporary records.");
    database
        .deserialize_read_exact("main", Cursor::new(payload.as_slice()), payload.len(), true)
        .expect("Cannot read the authenticated SQLite snapshot.");
    let version: u32 = database
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("Missing schema version.");
    let theme: String = database
        .query_row("SELECT theme FROM settings WHERE id=1", [], |row| {
            row.get(0)
        })
        .expect("Missing appearance setting.");
    let profiles: u32 = database
        .query_row("SELECT count(*) FROM profiles", [], |row| row.get(0))
        .expect("Missing profile table.");
    assert_eq!(
        version, expected_schema_version,
        "Unexpected schema version."
    );
    assert_eq!(theme, expected_theme);
    assert_eq!(profiles, 0);
    drop(database);
    drop(payload);
    println!(
        "{}",
        json!({"authenticated":true,"wrongPasswordRejected":true,"tamperRejected":true,"schemaVersion":version,"profiles":0,"plaintextDiskStaging":false})
    );
}
