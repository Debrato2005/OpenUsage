//! Test fixture writes must finish in every fork before the file can execute.
//! See https://github.com/rust-lang/rust/issues/114554 for the inherited-handle race.
use std::{
    fs::{self, File},
    io::{self, Write},
    os::fd::AsRawFd,
    path::Path,
    time::{Duration, Instant},
};

const WAIT_LIMIT: Duration = Duration::from_secs(5);

fn lock(file: &File, operation: libc::c_int, deadline: Instant) -> io::Result<()> {
    loop {
        // The borrowed file keeps this descriptor valid. Nonblocking flock bounds the wait.
        if unsafe { libc::flock(file.as_raw_fd(), operation | libc::LOCK_NB) } == 0 {
            return Ok(());
        }
        let error = io::Error::last_os_error();
        if !matches!(
            error.kind(),
            io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
        ) {
            return Err(error);
        }
        if Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "A test executable still has a writable handle.",
            ));
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn wait_for_writers(path: &Path, limit: Duration) -> io::Result<()> {
    let reader = File::open(path)?;
    lock(&reader, libc::LOCK_SH, Instant::now() + limit)
}

pub(crate) fn write(path: impl AsRef<Path>, bytes: impl AsRef<[u8]>) -> io::Result<()> {
    let path = path.as_ref();
    let mut writer = File::create(path)?;
    writer.write_all(bytes.as_ref())?;
    // A fork shares this open-file description, including a lock acquired after that fork.
    lock(&writer, libc::LOCK_EX, Instant::now() + WAIT_LIMIT)?;
    drop(writer);
    // A new read-only handle waits for inherited writers too. Later forks inherit no writer.
    wait_for_writers(path, WAIT_LIMIT)
}

pub(crate) fn copy(from: impl AsRef<Path>, to: impl AsRef<Path>) -> io::Result<()> {
    let permissions = fs::metadata(from.as_ref())?.permissions();
    write(to.as_ref(), fs::read(from)?)?;
    fs::set_permissions(to, permissions)
}

#[cfg(target_os = "linux")]
#[test]
fn duplicate_writer_blocks_execution_and_the_wait_has_a_deadline() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::Builder::new()
        .permissions(fs::Permissions::from_mode(0o700))
        .tempdir()
        .unwrap();
    let path = dir.path().join("synthetic-provider");
    let mut writer = File::create(&path).unwrap();
    writer.write_all(b"#!/bin/sh\nexit 0\n").unwrap();
    writer
        .set_permissions(fs::Permissions::from_mode(0o700))
        .unwrap();
    let duplicate = writer.try_clone().unwrap();
    lock(&writer, libc::LOCK_EX, Instant::now() + WAIT_LIMIT).unwrap();
    drop(writer);
    // A duplicate shares the same open-file description, as an inherited descriptor does.
    assert_eq!(
        std::process::Command::new(&path)
            .env_clear()
            .status()
            .unwrap_err()
            .raw_os_error(),
        Some(libc::ETXTBSY)
    );
    assert_eq!(
        wait_for_writers(&path, Duration::from_millis(20))
            .unwrap_err()
            .kind(),
        io::ErrorKind::TimedOut
    );
    drop(duplicate);
    wait_for_writers(&path, WAIT_LIMIT).unwrap();
    assert!(
        std::process::Command::new(&path)
            .env_clear()
            .status()
            .unwrap()
            .success()
    );
}
