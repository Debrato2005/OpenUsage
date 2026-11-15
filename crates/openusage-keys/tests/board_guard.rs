#![cfg(target_os = "linux")]
//! Filesystem and process checks use only new private synthetic directories.
use openusage_keys::{BoardGuard, BoardGuardError};
use std::fs::{self, DirBuilder, File};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt, symlink};
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "openusage-board-guard-{}-{nonce}",
            std::process::id()
        ));
        DirBuilder::new().mode(0o700).create(&path).unwrap();
        Self(path)
    }

    fn directory(&self) -> PathBuf {
        self.0.join("leaderboard")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct ChildGuard(Child);

impl ChildGuard {
    fn wait(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(status) = self.0.try_wait().unwrap() {
                assert!(status.success());
                return;
            }
            assert!(
                Instant::now() < deadline,
                "Private child exceeded its deadline"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn child(directory: &Path, action: &str) -> ChildGuard {
    ChildGuard(
        Command::new(std::env::current_exe().unwrap())
            .args(["--ignored", "--exact", "private_guard_child", "--nocapture"])
            .env("OPENUSAGE_PRIVATE_GUARD_DIRECTORY", directory)
            .env("OPENUSAGE_PRIVATE_GUARD_ACTION", action)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap(),
    )
}

#[test]
#[ignore = "Private child process entry point, called by the parent tests"]
fn private_guard_child() {
    let directory = PathBuf::from(std::env::var_os("OPENUSAGE_PRIVATE_GUARD_DIRECTORY").unwrap());
    assert!(directory.is_absolute());
    assert!(
        directory
            .parent()
            .unwrap()
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("openusage-board-guard-")
    );
    let action = std::env::var("OPENUSAGE_PRIVATE_GUARD_ACTION").unwrap();
    if action == "busy" {
        assert!(matches!(
            BoardGuard::acquire(&directory),
            Err(BoardGuardError::Busy)
        ));
        return;
    }
    let _guard = BoardGuard::acquire(&directory).unwrap();
    if action == "hold" {
        println!("private-guard-held");
        std::io::stdout().flush().unwrap();
        let (send, receive) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut line = String::new();
            let result = std::io::stdin().read_line(&mut line);
            let _ = send.send(result);
        });
        receive
            .recv_timeout(Duration::from_secs(5))
            .unwrap()
            .unwrap();
    } else {
        assert_eq!(action, "acquire");
    }
}

#[test]
fn live_process_refuses_then_release_reopens_the_same_empty_inode() {
    let fixture = Fixture::new();
    let directory = fixture.directory();
    let workspace = File::create(fixture.0.join("workspace-instance.lock")).unwrap();
    workspace.try_lock().unwrap();
    let guard = BoardGuard::acquire(&directory).unwrap();
    let lock_path = directory.join("enrollment.lock");
    let before = fs::metadata(&lock_path).unwrap();
    assert_eq!(fs::metadata(&directory).unwrap().mode() & 0o7777, 0o700);
    assert_eq!(before.mode() & 0o7777, 0o600);
    assert_eq!(before.nlink(), 1);
    assert_eq!(before.len(), 0);
    child(&directory, "busy").wait();
    assert!(matches!(
        BoardGuard::acquire(&directory),
        Err(BoardGuardError::Busy)
    ));
    drop(guard);
    child(&directory, "acquire").wait();
    let after = fs::metadata(&lock_path).unwrap();
    assert_eq!((before.dev(), before.ino()), (after.dev(), after.ino()));
    assert_eq!(after.len(), 0);
    BoardGuard::acquire(&directory).unwrap();
}

#[test]
fn missing_parents_are_private_and_do_not_require_a_workspace() {
    let fixture = Fixture::new();
    let app = fixture.0.join("data").join("app.openusage.desktop");
    let directory = app.join("leaderboard");
    let _guard = BoardGuard::acquire(&directory).unwrap();
    for path in [fixture.0.join("data"), app.clone(), directory] {
        assert_eq!(fs::metadata(path).unwrap().mode() & 0o7777, 0o700);
    }
    assert!(!app.join("workspace").exists());
}

#[test]
fn process_exit_releases_the_guard_without_removing_the_lock() {
    let fixture = Fixture::new();
    let directory = fixture.directory();
    let mut held = child(&directory, "hold");
    let mut output = BufReader::new(held.0.stdout.take().unwrap());
    let mut line = String::new();
    loop {
        line.clear();
        assert!(
            output.read_line(&mut line).unwrap() > 0,
            "Private child did not hold its guard"
        );
        if line.trim() == "private-guard-held" {
            break;
        }
    }
    assert!(matches!(
        BoardGuard::acquire(&directory),
        Err(BoardGuardError::Busy)
    ));
    let before = fs::metadata(directory.join("enrollment.lock")).unwrap();
    held.0.kill().unwrap();
    held.0.wait().unwrap();
    let _guard = BoardGuard::acquire(&directory).unwrap();
    let after = fs::metadata(directory.join("enrollment.lock")).unwrap();
    assert_eq!((before.dev(), before.ino()), (after.dev(), after.ino()));
}

#[test]
fn unsafe_directory_paths_and_modes_refuse_without_creating_a_lock() {
    let fixture = Fixture::new();
    let directory = fixture.directory();
    for path in [
        PathBuf::from("relative-guard"),
        PathBuf::from("/"),
        fixture.0.join("./leaderboard"),
        fixture.0.join("../leaderboard"),
        PathBuf::from(format!("{}//leaderboard", fixture.0.display())),
    ] {
        assert!(matches!(
            BoardGuard::acquire(&path),
            Err(BoardGuardError::UnsafePath)
        ));
    }
    assert!(!directory.exists());
    DirBuilder::new().mode(0o700).create(&directory).unwrap();
    for mode in [0o000, 0o755, 0o770, 0o1770, 0o1700] {
        fs::set_permissions(&directory, fs::Permissions::from_mode(mode)).unwrap();
        assert!(matches!(
            BoardGuard::acquire(&directory),
            Err(BoardGuardError::UnsafePath)
        ));
        assert!(!directory.join("enrollment.lock").exists());
    }
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).unwrap();
    let alias = fixture.0.join("alias");
    symlink(&directory, &alias).unwrap();
    assert!(matches!(
        BoardGuard::acquire(&alias),
        Err(BoardGuardError::UnsafePath)
    ));
    assert!(matches!(
        BoardGuard::acquire(&alias.join("child")),
        Err(BoardGuardError::UnsafePath)
    ));
    let file = fixture.0.join("not-directory");
    File::create(&file).unwrap();
    assert!(matches!(
        BoardGuard::acquire(&file),
        Err(BoardGuardError::UnsafePath)
    ));
    fs::set_permissions(&fixture.0, fs::Permissions::from_mode(0o777)).unwrap();
    assert!(matches!(
        BoardGuard::acquire(&directory),
        Err(BoardGuardError::UnsafePath)
    ));
    fs::set_permissions(&fixture.0, fs::Permissions::from_mode(0o700)).unwrap();
}

#[test]
fn unsafe_lock_modes_links_contents_and_types_refuse_without_mutation() {
    let fixture = Fixture::new();
    let directory = fixture.directory();
    drop(BoardGuard::acquire(&directory).unwrap());
    let lock = directory.join("enrollment.lock");
    for mode in [0o000, 0o644, 0o660, 0o1600, 0o4600] {
        fs::set_permissions(&lock, fs::Permissions::from_mode(mode)).unwrap();
        assert!(matches!(
            BoardGuard::acquire(&directory),
            Err(BoardGuardError::UnsafePath)
        ));
        assert_eq!(fs::metadata(&lock).unwrap().mode() & 0o7777, mode);
    }
    fs::set_permissions(&lock, fs::Permissions::from_mode(0o600)).unwrap();
    let hardlink = fixture.0.join("hardlink");
    fs::hard_link(&lock, &hardlink).unwrap();
    assert!(matches!(
        BoardGuard::acquire(&directory),
        Err(BoardGuardError::UnsafePath)
    ));
    assert_eq!(fs::metadata(&hardlink).unwrap().nlink(), 2);
    fs::remove_file(&hardlink).unwrap();
    fs::write(&lock, b"synthetic unexpected content").unwrap();
    assert!(matches!(
        BoardGuard::acquire(&directory),
        Err(BoardGuardError::UnsafePath)
    ));
    assert_eq!(fs::read(&lock).unwrap(), b"synthetic unexpected content");
    fs::remove_file(&lock).unwrap();
    let target = fixture.0.join("synthetic-target");
    fs::write(&target, b"keep this synthetic content").unwrap();
    symlink(&target, &lock).unwrap();
    assert!(matches!(
        BoardGuard::acquire(&directory),
        Err(BoardGuardError::UnsafePath)
    ));
    assert_eq!(fs::read(&target).unwrap(), b"keep this synthetic content");
    fs::remove_file(&lock).unwrap();
    fs::create_dir(&lock).unwrap();
    assert!(matches!(
        BoardGuard::acquire(&directory),
        Err(BoardGuardError::UnsafePath)
    ));
    fs::remove_dir(&lock).unwrap();
    let _socket = UnixListener::bind(&lock).unwrap();
    assert!(matches!(
        BoardGuard::acquire(&directory),
        Err(BoardGuardError::UnsafePath)
    ));
    fs::remove_file(&lock).unwrap();
    rustix::fs::mknodat(
        rustix::fs::CWD,
        &lock,
        rustix::fs::FileType::Fifo,
        rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
        0,
    )
    .unwrap();
    assert!(matches!(
        BoardGuard::acquire(&directory),
        Err(BoardGuardError::UnsafePath)
    ));
}
