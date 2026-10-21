//! Test-only oracle for one retained synthetic v1 archive; plaintext stays in Rust/SQLite RAM.
use openusage_import::prepare_restore;
use rusqlite::Connection;
use serde_json::json;
use std::{
    env, fs,
    io::{self, Cursor, Read},
    path::Path,
};
use zeroize::Zeroizing;

fn private_archive(path: &Path) -> Vec<u8> {
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
    assert_eq!(
        path.file_name().unwrap(),
        "openusage-backup-UuiQgym2XwpgOoj3.oubackup"
    );
    let parent = fs::symlink_metadata(path.parent().unwrap()).unwrap();
    assert!(
        parent.is_dir()
            && !parent.file_type().is_symlink()
            && parent.mode() & 0o777 == 0o700
            && parent.uid() == unsafe { libc::geteuid() }
    );
    let mut file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .expect("Cannot read private synthetic archive.");
    let metadata = file.metadata().unwrap();
    assert!(
        metadata.is_file()
            && metadata.mode() & 0o777 == 0o600
            && metadata.uid() == unsafe { libc::geteuid() }
            && metadata.nlink() == 1
    );
    assert_eq!(
        metadata.len(),
        28785,
        "This oracle accepts only the retained synthetic fixture size."
    );
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).unwrap();
    bytes
}
fn available_bytes(path: &Path) -> u64 {
    use std::os::unix::ffi::OsStrExt;
    let name = std::ffi::CString::new(path.as_os_str().as_bytes()).unwrap();
    let mut space = std::mem::MaybeUninit::<libc::statvfs>::uninit();
    assert_eq!(
        unsafe { libc::statvfs(name.as_ptr(), space.as_mut_ptr()) },
        0
    );
    let space = unsafe { space.assume_init() };
    space.f_bavail.checked_mul(space.f_frsize).unwrap()
}
fn main() {
    let args = env::args_os().collect::<Vec<_>>();
    assert_eq!(args.len(), 2);
    let path = Path::new(&args[1]);
    let original = private_archive(path);
    let mut password = Zeroizing::new(Vec::new());
    io::stdin().take(1025).read_to_end(&mut password).unwrap();
    assert!(!password.is_empty() && password.len() <= 1024);
    // The only disk staging holds the original authenticated ciphertext, never SQLite plaintext.
    let mut archive = prepare_restore(
        &mut Cursor::new(&original),
        password.as_slice(),
        path.parent().unwrap(),
        available_bytes(path.parent().unwrap()),
    )
    .expect("Retained synthetic archive did not authenticate.");
    assert_eq!(archive.manifest().schema_version, 1);
    assert_eq!(archive.manifest().entries.len(), 1);
    assert_eq!(archive.manifest().entries[0].name, "records.sqlite");
    assert_eq!(archive.manifest().entries[0].size, 28672);
    let mut bytes = Zeroizing::new(Vec::new());
    archive.copy_payload(&mut *bytes).unwrap();
    drop(archive);
    let summary = openusage_storage::validate_native_restore_snapshot(&bytes, 1)
        .expect("Retained v1 schema/content check failed.");
    assert_eq!(summary.active_profiles, 2);
    assert_eq!(summary.archived_profiles, 1);
    assert_eq!(summary.accounts, 0);
    // Read the original version directly as a separate oracle, without Storage or migrations.
    let mut connection = Connection::open_in_memory().unwrap();
    connection
        .execute_batch("PRAGMA cipher_memory_security=ON; PRAGMA temp_store=MEMORY;")
        .unwrap();
    connection
        .deserialize_read_exact("main", Cursor::new(bytes.as_slice()), bytes.len(), true)
        .unwrap();
    connection
        .execute_batch("PRAGMA query_only=ON; PRAGMA foreign_keys=ON;")
        .unwrap();
    assert_eq!(
        connection
            .pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
            .unwrap(),
        1
    );
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
    let profiles=connection.prepare("SELECT id,account_id,name,environment,approved_home_reference,connection_state,archived,created_at,updated_at FROM profiles ORDER BY name").unwrap().query_map([],|r|Ok(json!({
        "id":r.get::<_,String>(0)?,"account_id":r.get::<_,Option<String>>(1)?,"name":r.get::<_,String>(2)?,"environment":r.get::<_,String>(3)?,"approved_home_reference":r.get::<_,String>(4)?,"connection_state":r.get::<_,String>(5)?,"archived":r.get::<_,i64>(6)?,"created_at":r.get::<_,String>(7)?,"updated_at":r.get::<_,String>(8)?
    }))).unwrap().collect::<Result<Vec<_>,_>>().unwrap();
    let accounts=connection.prepare("SELECT id,verified_identity_key,identity_confidence,returned_email,returned_plan,first_observed_at,last_observed_at FROM accounts ORDER BY id").unwrap().query_map([],|r|Ok(json!({
        "id":r.get::<_,String>(0)?,"verified_identity_key":r.get::<_,Option<String>>(1)?,"identity_confidence":r.get::<_,String>(2)?,"returned_email":r.get::<_,Option<String>>(3)?,"returned_plan":r.get::<_,Option<String>>(4)?,"first_observed_at":r.get::<_,String>(5)?,"last_observed_at":r.get::<_,String>(6)?
    }))).unwrap().collect::<Result<Vec<_>,_>>().unwrap();
    let settings=connection.prepare("SELECT id,timezone,locale,theme,monitoring,retention_policy,created_at,updated_at FROM settings ORDER BY id").unwrap().query_map([],|r|Ok(json!({
        "id":r.get::<_,i64>(0)?,"timezone":r.get::<_,String>(1)?,"locale":r.get::<_,String>(2)?,"theme":r.get::<_,String>(3)?,"monitoring":r.get::<_,String>(4)?,"retention_policy":r.get::<_,String>(5)?,"created_at":r.get::<_,String>(6)?,"updated_at":r.get::<_,String>(7)?
    }))).unwrap().collect::<Result<Vec<_>,_>>().unwrap();
    assert_eq!(profiles.len(), 3);
    assert!(accounts.is_empty());
    assert_eq!(settings.len(), 1);
    for (profile, name) in profiles.iter().zip([
        "Content synthetic active one",
        "Content synthetic active two",
        "Content synthetic archived",
    ]) {
        assert_eq!(profile["name"], name);
        assert!(profile["account_id"].is_null());
        assert_eq!(profile["environment"], "native");
        assert_eq!(profile["archived"], i64::from(name.ends_with("archived")));
    }
    assert_eq!(settings[0]["theme"], "dark");
    assert_eq!(settings[0]["monitoring"], "stopped");
    let version: String = connection
        .query_row("PRAGMA cipher_version", [], |r| r.get(0))
        .unwrap();
    assert!(version.starts_with("4."));
    drop(connection);
    drop(bytes);
    drop(password);
    assert_eq!(private_archive(path), original);
    println!(
        "{}",
        json!({"verified":true,"archiveAuthenticated":true,"readOnlyOriginalV1":true,"manifestSchemaVersion":1,"databaseSchemaVersion":1,"activeProfiles":2,"archivedProfiles":1,"accountsCount":0,"plaintextDiskStaging":false,"originalArchiveUnchanged":true,"tables":{"profiles":profiles,"accounts":accounts,"settings":settings}})
    );
}
