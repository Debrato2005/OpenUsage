use openusage_import::{Entry, Manifest, encrypt, prepare_restore};
use rusqlite::Connection;
use std::{
    env,
    ffi::CString,
    fs::{self, File, OpenOptions},
    io::{self, Cursor, Read},
    os::unix::{ffi::OsStrExt, fs::OpenOptionsExt, fs::PermissionsExt},
    path::Path,
};
use zeroize::Zeroizing;

fn free_bytes(path: &Path) -> u64 {
    let path = CString::new(path.as_os_str().as_bytes()).expect("Invalid staging path.");
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

fn private_directory(path: &Path) {
    let metadata = fs::symlink_metadata(path).expect("Missing private fixture directory.");
    assert!(metadata.is_dir() && !metadata.file_type().is_symlink());
    assert_eq!(metadata.permissions().mode() & 0o077, 0);
}

fn database(payload: &[u8]) -> Connection {
    let mut database = Connection::open_in_memory().expect("Cannot create the fixture database.");
    database
        .pragma_update(None, "cipher_memory_security", true)
        .expect("Cannot protect SQLite memory.");
    database
        .pragma_update(None, "temp_store", "MEMORY")
        .expect("Cannot constrain temporary records.");
    database
        .deserialize_read_exact("main", Cursor::new(payload), payload.len(), false)
        .expect("Cannot read the synthetic snapshot.");
    database
}

fn archive(
    output: &Path,
    staging: &Path,
    name: &str,
    schema: u32,
    payload: &[u8],
    password: &[u8],
) {
    assert_eq!(name.len(), 16);
    assert!(name.bytes().all(|byte| byte.is_ascii_alphanumeric()));
    let manifest = Manifest {
        schema_version: schema,
        entries: vec![Entry {
            name: "records.sqlite".into(),
            size: payload.len() as u64,
        }],
    };
    let path = output.join(format!("openusage-backup-{name}.oubackup"));
    let mut destination = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path)
        .expect("Cannot create the encrypted fixture.");
    encrypt(
        &manifest,
        &mut Cursor::new(payload),
        &mut destination,
        password,
        free_bytes(output),
    )
    .expect("Cannot encrypt the fixture.");
    destination
        .sync_all()
        .expect("Cannot save the encrypted fixture.");
    drop(destination);
    let mut verified = prepare_restore(
        &mut File::open(path).expect("Cannot open the encrypted fixture."),
        password,
        staging,
        free_bytes(staging),
    )
    .expect("The fixture did not authenticate.");
    assert_eq!(verified.manifest(), &manifest);
    let mut decoded = Zeroizing::new(Vec::new());
    verified
        .copy_payload(&mut *decoded)
        .expect("Cannot read the authenticated fixture.");
    assert!(
        decoded.as_slice() == payload,
        "The authenticated fixture payload changed."
    );
}

fn main() {
    let args: Vec<_> = env::args_os().collect();
    assert_eq!(
        args.len(),
        4,
        "Supply source archive, private staging, and private output directories."
    );
    let source = Path::new(&args[1]);
    let staging = Path::new(&args[2]);
    let output = Path::new(&args[3]);
    private_directory(staging);
    private_directory(output);
    let mut password = Zeroizing::new(Vec::new());
    io::stdin()
        .take(1025)
        .read_to_end(&mut password)
        .expect("Cannot read the synthetic password.");
    assert!(
        !password.is_empty() && password.len() <= 1024,
        "Invalid synthetic password size."
    );
    let mut verified = prepare_restore(
        &mut File::open(source).expect("Cannot read the source archive."),
        &password,
        staging,
        free_bytes(staging),
    )
    .expect("The source archive did not authenticate.");
    let schema = verified.manifest().schema_version;
    assert!((1..=openusage_storage::CURRENT_SCHEMA_VERSION).contains(&schema));
    assert_eq!(verified.manifest().entries.len(), 1);
    assert_eq!(verified.manifest().entries[0].name, "records.sqlite");
    assert!(
        verified.manifest().entries[0].size <= 1024 * 1024,
        "The synthetic snapshot exceeds the fixture limit."
    );
    let mut payload = Zeroizing::new(Vec::new());
    verified
        .copy_payload(&mut *payload)
        .expect("Cannot read the authenticated records.");
    drop(verified);

    let non_database =
        Zeroizing::new(b"Authenticated synthetic bytes that are not a SQLite database.".to_vec());
    archive(
        output,
        staging,
        "FixtureNonSqlite",
        schema,
        &non_database,
        &password,
    );
    archive(output, staging, "FixtureManifest2", 77, &payload, &password);

    let future = database(&payload);
    future
        .pragma_update(None, "user_version", 77)
        .expect("Cannot set the future fixture schema.");
    let serialized = future
        .serialize("main")
        .expect("Cannot serialize the future fixture.");
    let future_payload = Zeroizing::new(serialized.to_vec());
    drop(serialized);
    drop(future);
    archive(
        output,
        staging,
        "FixtureFutureDB1",
        schema,
        &future_payload,
        &password,
    );

    let invalid = database(&payload);
    invalid
        .pragma_update(None, "foreign_keys", false)
        .expect("Cannot prepare the relation fixture.");
    assert!(invalid.execute("UPDATE profiles SET account_id='00000001-0000-4000-8000-000000000099' WHERE id=(SELECT id FROM profiles LIMIT 1)", []).expect("Cannot create the synthetic broken relation.") > 0, "The source fixture must contain a profile.");
    let serialized = invalid
        .serialize("main")
        .expect("Cannot serialize the invalid fixture.");
    let invalid_payload = Zeroizing::new(serialized.to_vec());
    drop(serialized);
    drop(invalid);
    archive(
        output,
        staging,
        "FixtureBadRows01",
        schema,
        &invalid_payload,
        &password,
    );

    println!(
        "{{\"authenticatedFixtures\":4,\"plaintextDiskStaging\":false,\"files\":[\"openusage-backup-FixtureNonSqlite.oubackup\",\"openusage-backup-FixtureManifest2.oubackup\",\"openusage-backup-FixtureFutureDB1.oubackup\",\"openusage-backup-FixtureBadRows01.oubackup\"]}}"
    );
}
