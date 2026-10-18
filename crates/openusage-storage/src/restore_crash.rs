//! Owned-process crash checks against the production restore transaction.
//! These checks do not simulate power loss or qualify release durability.

use crate::{RestoreHomeMapping, Storage, Theme, validate_restore_input};
use rusqlite::{Connection, hooks::Action, params, types::Value};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    os::unix::{fs::OpenOptionsExt, process::ExitStatusExt},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

const KEY: [u8; 32] = [0x4d; 32];
const CHILD_ENTRY: &str = "restore_crash::owned_restore_crash_child";
const ROOT_VARIABLE: &str = "OPENUSAGE_STORAGE_CRASH_FIXTURE";
const PHASE_VARIABLE: &str = "OPENUSAGE_STORAGE_CRASH_PHASE";

fn id(number: u32) -> String {
    format!("{number:08x}-0000-4000-8000-000000000000")
}

#[derive(Debug, PartialEq)]
struct Records {
    accounts: Vec<Vec<Value>>,
    profiles: Vec<Vec<Value>>,
    settings: Vec<Vec<Value>>,
}

fn records(connection: &Connection) -> Records {
    fn rows(connection: &Connection, sql: &str) -> Vec<Vec<Value>> {
        let mut statement = connection.prepare(sql).unwrap();
        let columns = statement.column_count();
        statement
            .query_map([], |row| (0..columns).map(|index| row.get(index)).collect())
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap()
    }
    Records {
        accounts: rows(connection, "SELECT * FROM accounts ORDER BY id"),
        profiles: rows(connection, "SELECT * FROM profiles ORDER BY id"),
        settings: rows(connection, "SELECT * FROM settings ORDER BY id"),
    }
}

fn incoming_homes(storage: &mut Storage) -> Vec<RestoreHomeMapping> {
    let snapshot = storage.backup_snapshot().unwrap();
    validate_restore_input(&snapshot.bytes, snapshot.schema_version)
        .unwrap()
        .profile_ids
        .into_iter()
        .enumerate()
        .map(|(index, profile_id)| RestoreHomeMapping {
            profile_id,
            managed_home_reference: id(1_000_000 + index as u32),
        })
        .collect()
}

fn account(storage: &Storage, number: u32, label: &str) {
    storage.connection.execute(
        "INSERT INTO accounts VALUES (?1,NULL,'unknown',?2,NULL,'2024-02-29T00:00:00Z','2024-02-29T01:00:00Z')",
        params![id(number), format!("{label}@example.invalid")],
    ).unwrap();
}

fn fixture(directory: &Path) -> (Records, Records, Vec<RestoreHomeMapping>, String) {
    let mut live = Storage::create(&directory.join("live.sqlite"), &KEY).unwrap();
    live.create_profile(&id(1), "Original synthetic profile", &id(1))
        .unwrap();
    live.set_theme(Theme::Dark).unwrap();
    account(&live, 90, "original");
    live.connection.execute(
        "UPDATE profiles SET account_id=?1,connection_state='connected',created_at='2024-01-01T00:00:00Z',updated_at='2024-01-01T00:00:00Z'",
        [id(90)],
    ).unwrap();
    live.connection.execute_batch("UPDATE settings SET monitoring='paused',created_at='2024-01-01T00:00:00Z',updated_at='2024-01-01T00:00:00Z'").unwrap();
    let old = records(&live.connection);
    live.create_encrypted_checkpoint(&directory.join("original-checkpoint.sqlite"))
        .unwrap();
    let lower = live
        .connection
        .query_row("SELECT strftime('%Y-%m-%dT%H:%M:%fZ','now')", [], |row| {
            row.get(0)
        })
        .unwrap();
    drop(live);

    let mut source = Storage::create(&directory.join("incoming.sqlite"), &KEY).unwrap();
    source.set_theme(Theme::Light).unwrap();
    account(&source, 99, "incoming");
    // More dirty pages than the child's tiny cache force actual uncommitted WAL frames.
    for number in 100..300 {
        source
            .create_profile(
                &id(number),
                &format!("Incoming synthetic {number} {}", "😀".repeat(60)),
                &id(number),
            )
            .unwrap();
        if number != 299 {
            source.set_profile_archived(&id(number), true).unwrap();
        }
    }
    source.connection.execute(
        "UPDATE profiles SET account_id=?1,connection_state='connected',created_at='2024-02-29T00:00:00Z',updated_at='2024-02-29T01:00:00Z'",
        [id(99)],
    ).unwrap();
    source.connection.execute_batch("UPDATE settings SET monitoring='active',created_at='2024-02-29T00:00:00Z',updated_at='2024-02-29T01:00:00Z'").unwrap();
    let incoming = records(&source.connection);
    let homes = incoming_homes(&mut source);
    (old, incoming, homes, lower)
}

fn barrier(directory: &Path, phase: &str) -> ! {
    let mut marker = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(directory.join("phase.pending"))
        .unwrap();
    marker.write_all(phase.as_bytes()).unwrap();
    marker.sync_all().unwrap();
    drop(marker);
    fs::rename(
        directory.join("phase.pending"),
        directory.join("reached-phase"),
    )
    .unwrap();
    // The owning parent kills this child after checking the acknowledged phase.
    loop {
        thread::park();
    }
}

/// Only the owning parent invokes this test entry with fresh private synthetic paths.
#[test]
#[ignore = "Invoked only as an owned child by the process-crash acceptance test."]
fn owned_restore_crash_child() {
    let directory =
        PathBuf::from(std::env::var_os(ROOT_VARIABLE).expect("Missing synthetic fixture."));
    let phase = std::env::var(PHASE_VARIABLE).expect("Missing synthetic phase.");
    assert!(matches!(phase.as_str(), "before-commit" | "after-commit"));
    let mut live = Storage::open(&directory.join("live.sqlite"), &KEY).unwrap();
    let mut source = Storage::open(&directory.join("incoming.sqlite"), &KEY).unwrap();
    let snapshot = source.backup_snapshot().unwrap();
    let homes = incoming_homes(&mut source);
    live.connection
        .execute_batch("PRAGMA wal_autocheckpoint=0; PRAGMA cache_size=1; PRAGMA cache_spill=ON;")
        .unwrap();
    let expected = live.restore_state().unwrap().revision;
    if phase == "before-commit" {
        let directory = directory.clone();
        live.connection
            .update_hook(Some(move |action, database: &str, table: &str, _row_id| {
                if action == Action::SQLITE_INSERT && database == "main" && table == "settings" {
                    // This callback follows the real DELETE and account/profile INSERT statements.
                    barrier(&directory, "before-commit");
                }
            }))
            .unwrap();
    }
    live.replace_records_from_backup(&snapshot.bytes, snapshot.schema_version, &expected, &homes)
        .unwrap();
    assert_eq!(phase, "after-commit");
    // The actual production function committed, but the child never reports successful completion.
    barrier(&directory, "after-commit");
}

struct OwnedChild(Child);
impl Drop for OwnedChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn kill_at(directory: &Path, phase: &str) {
    let mut child = OwnedChild(
        Command::new(std::env::current_exe().unwrap())
            .args(["--exact", CHILD_ENTRY, "--ignored", "--nocapture"])
            .env_clear()
            .env(ROOT_VARIABLE, directory)
            .env(PHASE_VARIABLE, phase)
            .env("RUST_TEST_THREADS", "1")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if directory.join("reached-phase").exists() {
            break;
        }
        assert!(
            child.0.try_wait().unwrap().is_none(),
            "Owned child exited before {phase}."
        );
        assert!(
            Instant::now() < deadline,
            "Owned child did not reach {phase}."
        );
        thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(
        fs::read_to_string(directory.join("reached-phase")).unwrap(),
        phase
    );
    let wal = fs::read(directory.join("live.sqlite-wal")).unwrap();
    assert!(
        wal.len() > 32,
        "The actual restore must create WAL frames before the kill."
    );
    let page_size = u32::from_be_bytes(wal[8..12].try_into().unwrap()) as usize;
    assert!((512..=65536).contains(&page_size) && page_size.is_power_of_two());
    assert_eq!((wal.len() - 32) % (page_size + 24), 0);
    let commits = wal[32..]
        .chunks_exact(page_size + 24)
        .filter(|frame| u32::from_be_bytes(frame[4..8].try_into().unwrap()) != 0)
        .count();
    assert_eq!(
        commits,
        usize::from(phase == "after-commit"),
        "The WAL commit marker must match the acknowledged phase."
    );
    println!(
        "owned SQLCipher restore phase={phase}, WAL bytes={}, frames={}, commit markers={commits}",
        wal.len(),
        (wal.len() - 32) / (page_size + 24)
    );
    for plaintext in [
        b"Original synthetic profile".as_slice(),
        b"Incoming synthetic".as_slice(),
    ] {
        assert!(!wal.windows(plaintext.len()).any(|bytes| bytes == plaintext));
    }
    child.0.kill().unwrap();
    assert_eq!(child.0.wait().unwrap().signal(), Some(libc::SIGKILL));
}

#[test]
fn owned_process_kills_preserve_old_or_fully_committed_new_records() {
    for phase in ["before-commit", "after-commit"] {
        let directory = crate::tests::private_directory();
        let (old, mut incoming, homes, lower) = fixture(directory.path());
        let checkpoint = directory.path().join("original-checkpoint.sqlite");
        let saved_checkpoint = fs::read(&checkpoint).unwrap();
        kill_at(directory.path(), phase);
        let mut reopened = Storage::open(&directory.path().join("live.sqlite"), &KEY).unwrap();
        crate::backup_validation::validate_connection(&reopened.connection, 1).unwrap();
        let actual = records(&reopened.connection);
        if phase == "before-commit" {
            assert_eq!(
                actual, old,
                "Every prior record column must survive an uncommitted process crash."
            );
        } else {
            let Value::Text(when) = &actual.settings[0][7] else {
                panic!("Missing restore timestamp.")
            };
            let upper: String = reopened
                .connection
                .query_row("SELECT strftime('%Y-%m-%dT%H:%M:%fZ','now')", [], |row| {
                    row.get(0)
                })
                .unwrap();
            assert!(when >= &lower && when <= &upper);
            for (row, home) in incoming.profiles.iter_mut().zip(&homes) {
                assert_eq!(row[0], Value::Text(home.profile_id.clone()));
                row[4] = Value::Text(home.managed_home_reference.clone());
                row[5] = Value::Text("authentication_required".into());
                row[8] = Value::Text(when.clone());
            }
            incoming.settings[0][4] = Value::Text("stopped".into());
            incoming.settings[0][7] = Value::Text(when.clone());
            assert_eq!(
                actual, incoming,
                "The committed state must contain every complete incoming record."
            );
        }
        assert_eq!(
            reopened
                .connection
                .pragma_query_value(None, "journal_mode", |row| row.get::<_, String>(0))
                .unwrap(),
            "wal"
        );
        assert_eq!(
            reopened
                .connection
                .pragma_query_value(None, "synchronous", |row| row.get::<_, i64>(0))
                .unwrap(),
            2
        );
        assert_eq!(fs::read(&checkpoint).unwrap(), saved_checkpoint);
        let verified = reopened.verify_encrypted_checkpoint(&checkpoint).unwrap();
        assert_eq!(verified.summary.active_profiles, 1);
        assert_eq!(verified.summary.accounts, 1);
        let snapshot = reopened.read_encrypted_checkpoint(&checkpoint).unwrap();
        crate::backup_validation::with_validated_snapshot::<_, crate::BackupValidationError>(
            &snapshot.bytes,
            1,
            true,
            |source, _| {
                assert_eq!(records(source), old);
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(fs::read(&checkpoint).unwrap(), saved_checkpoint);
    }
}
