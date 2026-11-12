//! Runs the pinned ccusage binary once (OU-LOG-005, OU-LOG-006): checked digest, fixed arguments,
//! no shell, a minimal environment, offline pricing, a deadline and an output limit.
//! Each run reads one consented folder of one tool. HOME is an empty private directory, so the
//! default folders of other tools do not exist for ccusage (OU-LOG-001).

use crate::hours::{HourRow, parse_hours};
use crate::report::{ReportError, UsageRow, parse_daily_by_agent};
use crate::session::{SessionRow, parse_sessions};
use crate::tools::tool;
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::Path;
#[cfg(unix)]
use std::process::Child;
use std::process::{Command, Stdio};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::thread;
use std::time::{Duration, Instant};

pub const CCUSAGE_VERSION: &str = "20.0.48";

/// SHA-256 of the binary in the official `@ccusage/ccusage-<platform>@20.0.48` npm package.
/// Other targets stay unsupported until their binary is provisioned and pinned.
pub fn pinned_sha256(target: &str) -> Option<&'static str> {
    match target {
        "x86_64-unknown-linux-gnu" => {
            Some("55a3fd41b29727672bef1242311758d1605a07765781d7a80a41d1b485d29ed1")
        }
        _ => None,
    }
}

/// The file name of the pinned binary beside the app and the CLI.
pub const BINARY_NAME: &str = if cfg!(windows) {
    "openusage-ccusage.exe"
} else {
    "openusage-ccusage"
};

/// The pinned digest for the platform that runs this code. None where no binary is pinned yet.
pub fn pinned_sha256_here() -> Option<&'static str> {
    if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        pinned_sha256("x86_64-unknown-linux-gnu")
    } else {
        None
    }
}

pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);
pub const DEFAULT_MAX_OUTPUT: usize = 64 * 1024 * 1024;
#[cfg(unix)]
const CLEANUP_TIMEOUT: Duration = Duration::from_secs(2);

/// One-way cancellation for an owned scan. It does not replace source-consent checks.
#[derive(Default)]
struct CancellationState {
    cancelled: AtomicBool,
    cleanup_failed: AtomicBool,
}

#[derive(Clone, Default)]
pub struct ScanCancellation(Arc<CancellationState>);

impl ScanCancellation {
    pub fn cancel(&self) {
        self.0.cancelled.store(true, Ordering::Release);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.cancelled.load(Ordering::Acquire)
    }
    pub fn cleanup_failed(&self) -> bool {
        self.0.cleanup_failed.load(Ordering::Acquire)
    }
    #[cfg(unix)]
    fn note_cleanup_failure(&self) {
        self.0.cleanup_failed.store(true, Ordering::Release);
        self.cancel();
    }
}

pub(super) fn check_cancelled(cancellation: Option<&ScanCancellation>) -> Result<(), ScanError> {
    if cancellation.is_some_and(ScanCancellation::is_cancelled) {
        Err(ScanError::Cancelled)
    } else {
        Ok(())
    }
}

pub(super) static ONE_RUN: Mutex<()> = Mutex::new(());
#[cfg(test)]
pub(super) static TEST_RUNS: Mutex<()> = Mutex::new(());

pub struct ScanRequest<'a> {
    pub binary: &'a Path,
    pub expected_sha256: &'a str,
    /// An enabled tool and its resolved folder. The user consented to both (OU-LOG-001).
    pub tool: &'a str,
    pub folder: &'a Path,
    /// An empty private directory that becomes HOME and the working directory.
    pub empty_home: &'a Path,
    /// The IANA zone pinned at the first scan for day boundaries (OU-LOG-010).
    pub timezone: &'a str,
    pub since: Option<&'a str>,
    pub timeout: Duration,
    pub max_output: usize,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ScanError {
    /// Another scan is running.
    Busy,
    /// The request itself is invalid, such as a malformed time zone.
    InvalidRequest,
    /// The binary is missing or unreadable.
    Unavailable,
    /// The binary does not match its pin, so it was not run.
    Digest,
    Spawn,
    Cancelled,
    /// The owned child did not reap within the cleanup deadline.
    CleanupFailed,
    Timeout,
    OutputTooLarge,
    Failed(Option<i32>),
    Report(ReportError),
}

fn valid_timezone(zone: &str) -> bool {
    !zone.is_empty()
        && zone.len() <= 64
        && zone.starts_with(|c: char| c.is_ascii_alphabetic())
        && zone
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "/_+-".contains(c))
        && !zone.contains("..")
}

fn valid_since(since: &str) -> bool {
    since.len() == 10
        && since.chars().enumerate().all(|(i, c)| {
            if i == 4 || i == 7 {
                c == '-'
            } else {
                c.is_ascii_digit()
            }
        })
}

pub fn valid_resolved_folder(folder: &Path) -> bool {
    let Some(text) = folder.to_str() else {
        return false;
    };
    // ccusage splits folder variables on commas and trims each entry (OU-LOG-001).
    // The resolved path saved with consent must still identify that root (OU-DUP-008).
    folder.is_absolute()
        && !text.contains(',')
        && text.trim() == text
        && folder.is_dir()
        && std::fs::canonicalize(folder)
            .is_ok_and(|resolved| resolved.as_os_str() == folder.as_os_str())
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// The daily report of the granted tool, one row for each day and model.
pub fn scan(request: &ScanRequest<'_>) -> Result<Vec<UsageRow>, ScanError> {
    scan_daily_roots(request, &[request.folder])
}

pub fn scan_with_cancellation(
    request: &ScanRequest<'_>,
    cancellation: &ScanCancellation,
) -> Result<Vec<UsageRow>, ScanError> {
    scan_daily_roots_controlled(request, &[request.folder], Some(cancellation))
}

/// One complete daily report for all consented roots of an enabled legacy tool.
/// The pinned scanner receives the roots together so copied events share one scan.
pub fn scan_daily_roots(
    request: &ScanRequest<'_>,
    folders: &[&Path],
) -> Result<Vec<UsageRow>, ScanError> {
    scan_daily_roots_controlled(request, folders, None)
}

fn scan_daily_roots_controlled(
    request: &ScanRequest<'_>,
    folders: &[&Path],
    cancellation: Option<&ScanCancellation>,
) -> Result<Vec<UsageRow>, ScanError> {
    check_cancelled(cancellation)?;
    let _one = ONE_RUN.try_lock().map_err(|_| ScanError::Busy)?;
    tool(request.tool)
        .filter(|t| {
            t.enabled
                && matches!(
                    t.session_shape,
                    Some(crate::tools::SessionShape::Claude | crate::tools::SessionShape::Codex)
                )
        })
        .ok_or(ScanError::InvalidRequest)?;
    let output = run_roots(
        request,
        &["daily", "--json", "--offline", "--by-agent", "--no-color"],
        folders,
        cancellation,
    )?;
    let rows = parse_daily_by_agent(&output).map_err(ScanError::Report)?;
    // A row of another tool means that ccusage read a folder that it was not given.
    if rows.iter().any(|row| row.tool != request.tool) {
        return Err(ScanError::Report(ReportError::Shape(
            "a tool that the run did not ask for".into(),
        )));
    }
    check_cancelled(cancellation)?;
    Ok(rows)
}

/// The session report of the granted tool (OU-LOG-017, SC-021).
pub fn scan_sessions(request: &ScanRequest<'_>) -> Result<Vec<SessionRow>, ScanError> {
    scan_sessions_controlled(request, None)
}

pub fn scan_sessions_with_cancellation(
    request: &ScanRequest<'_>,
    cancellation: &ScanCancellation,
) -> Result<Vec<SessionRow>, ScanError> {
    scan_sessions_controlled(request, Some(cancellation))
}

fn scan_sessions_controlled(
    request: &ScanRequest<'_>,
    cancellation: Option<&ScanCancellation>,
) -> Result<Vec<SessionRow>, ScanError> {
    let tool = tool(request.tool)
        .filter(|tool| tool.enabled && tool.session_shape.is_some())
        .map(|tool| tool.id.as_str())
        .ok_or(ScanError::InvalidRequest)?;
    let output = run(
        request,
        &[tool, "session", "--json", "--offline", "--no-color"],
        cancellation,
    )?;
    let rows = parse_sessions(tool, &output).map_err(ScanError::Report)?;
    check_cancelled(cancellation)?;
    Ok(rows)
}

/// The one-hour blocks of Claude Code (OU-LOG-017, SC-021). Blocks take no start date here.
pub fn scan_hours(request: &ScanRequest<'_>) -> Result<Vec<HourRow>, ScanError> {
    scan_hours_controlled(request, None)
}

pub fn scan_hours_with_cancellation(
    request: &ScanRequest<'_>,
    cancellation: &ScanCancellation,
) -> Result<Vec<HourRow>, ScanError> {
    scan_hours_controlled(request, Some(cancellation))
}

fn scan_hours_controlled(
    request: &ScanRequest<'_>,
    cancellation: Option<&ScanCancellation>,
) -> Result<Vec<HourRow>, ScanError> {
    if !tool(request.tool).is_some_and(|tool| tool.enabled && tool.hours) || request.since.is_some()
    {
        return Err(ScanError::InvalidRequest);
    }
    let output = run(
        request,
        &["blocks", "--json", "--offline", "--no-color", "-n", "1"],
        cancellation,
    )?;
    let rows = parse_hours(&output).map_err(ScanError::Report)?;
    check_cancelled(cancellation)?;
    Ok(rows)
}

/// Runs the pinned binary once with fixed arguments and returns its output (OU-LOG-006).
fn run(
    request: &ScanRequest<'_>,
    fixed: &[&str],
    cancellation: Option<&ScanCancellation>,
) -> Result<Vec<u8>, ScanError> {
    check_cancelled(cancellation)?;
    let _one = ONE_RUN.try_lock().map_err(|_| ScanError::Busy)?;
    tool(request.tool)
        .filter(|tool| tool.enabled)
        .ok_or(ScanError::InvalidRequest)?;
    run_roots(request, fixed, &[request.folder], cancellation)
}

// The caller holds ONE_RUN and checks tool enablement. Tests can exercise disabled adapters here.
pub(super) fn run_roots(
    request: &ScanRequest<'_>,
    fixed: &[&str],
    folders: &[&Path],
    cancellation: Option<&ScanCancellation>,
) -> Result<Vec<u8>, ScanError> {
    check_cancelled(cancellation)?;
    // No scanner is pinned on non-Unix platforms. Keep their existing plain API,
    // but refuse a cancellable run before spawning until capture is qualified there.
    #[cfg(not(unix))]
    if cancellation.is_some() {
        return Err(ScanError::Unavailable);
    }
    let definition = tool(request.tool).ok_or(ScanError::InvalidRequest)?;
    let variable = definition.folder_variable.as_deref();
    let empty =
        std::fs::read_dir(request.empty_home).is_ok_and(|mut entries| entries.next().is_none());
    let (Some(variable), true) = (variable, empty) else {
        return Err(ScanError::InvalidRequest);
    };
    if !valid_timezone(request.timezone)
        || request.since.is_some_and(|since| !valid_since(since))
        || !request.empty_home.is_absolute()
        || !(1..=64).contains(&folders.len())
        || folders.iter().any(|folder| !valid_resolved_folder(folder))
    {
        return Err(ScanError::InvalidRequest);
    }
    let roots: std::collections::BTreeSet<_> = folders
        .iter()
        .map(|folder| folder.to_str().ok_or(ScanError::InvalidRequest))
        .collect::<Result<_, _>>()?;
    if definition.root_mode == crate::tools::RootMode::Single && roots.len() != 1 {
        return Err(ScanError::InvalidRequest);
    }
    let roots = roots.into_iter().collect::<Vec<_>>().join(",");
    let bytes = std::fs::read(request.binary).map_err(|_| ScanError::Unavailable)?;
    if sha256_hex(&bytes) != request.expected_sha256 {
        return Err(ScanError::Digest);
    }
    drop(bytes);
    check_cancelled(cancellation)?;
    let mut args = fixed.to_vec();
    // Estimated value always uses token prices, never a stored charge (OU-LOG-014).
    args.extend(["--mode", "calculate"]);
    args.extend(["--timezone", request.timezone]);
    if let Some(since) = request.since {
        args.extend(["--since", since]);
    }
    let mut command = Command::new(request.binary);
    command
        .args(&args)
        .env_clear()
        .env("HOME", request.empty_home)
        .env(variable, roots)
        .env("NO_COLOR", "1")
        .env("TZ", "UTC")
        .current_dir(request.empty_home)
        .stdin(Stdio::null())
        .stderr(Stdio::null());
    capture(command, request, cancellation)
}

// Only the retained Child is signaled. No process name, process group, or borrowed FD is used.
#[cfg(unix)]
fn stop_owned(child: &mut Child) -> Result<(), ScanError> {
    if child
        .try_wait()
        .map_err(|_| ScanError::CleanupFailed)?
        .is_some()
    {
        return Ok(());
    }
    let _ = child.kill();
    let deadline = Instant::now() + CLEANUP_TIMEOUT;
    loop {
        if child
            .try_wait()
            .map_err(|_| ScanError::CleanupFailed)?
            .is_some()
        {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(ScanError::CleanupFailed);
        }
        thread::sleep(Duration::from_millis(10));
    }
}

#[cfg(unix)]
fn capture(
    mut command: Command,
    request: &ScanRequest<'_>,
    cancellation: Option<&ScanCancellation>,
) -> Result<Vec<u8>, ScanError> {
    use std::io::ErrorKind;
    use std::os::{fd::OwnedFd, unix::net::UnixStream};
    let (mut output, writer) = UnixStream::pair().map_err(|_| ScanError::Spawn)?;
    output.set_nonblocking(true).map_err(|_| ScanError::Spawn)?;
    let mut child = command
        .stdout(Stdio::from(OwnedFd::from(writer)))
        .spawn()
        .map_err(|_| ScanError::Spawn)?;
    // Command retains its configured stdout descriptor. Drop it before waiting for EOF.
    drop(command);
    let deadline = Instant::now() + request.timeout;
    let mut bytes = Vec::new();
    let mut chunk = [0u8; 64 * 1024];
    let mut status = None;
    let mut eof = false;
    let result = loop {
        if let Err(error) = check_cancelled(cancellation) {
            break Err(error);
        }
        if Instant::now() >= deadline {
            break Err(ScanError::Timeout);
        }
        if status.is_none() {
            match child.try_wait() {
                Ok(found) => status = found,
                Err(_) => break Err(ScanError::Spawn),
            }
        }
        // One bounded chunk per turn keeps cancellation effective under continuous output.
        let mut received = false;
        if !eof {
            match output.read(&mut chunk) {
                Ok(0) => eof = true,
                Ok(length) if length <= request.max_output.saturating_sub(bytes.len()) => {
                    bytes.extend_from_slice(&chunk[..length]);
                    received = true;
                }
                Ok(_) => break Err(ScanError::OutputTooLarge),
                Err(error)
                    if error.kind() == ErrorKind::WouldBlock
                        || error.kind() == ErrorKind::Interrupted => {}
                Err(_) => break Err(ScanError::OutputTooLarge),
            }
        }
        if let Some(status) = status
            && eof
        {
            break if status.success() {
                Ok(bytes)
            } else {
                Err(ScanError::Failed(status.code()))
            };
        }
        if !received {
            thread::sleep(Duration::from_millis(20));
        }
    };
    if result.is_err()
        && let Err(error) = stop_owned(&mut child)
    {
        if let Some(cancellation) = cancellation {
            cancellation.note_cleanup_failure();
        }
        return Err(error);
    }
    result
}

// Existing non-cancellable capture is retained on platforms without a pinned scanner.
#[cfg(not(unix))]
fn capture(
    mut command: Command,
    request: &ScanRequest<'_>,
    _cancellation: Option<&ScanCancellation>,
) -> Result<Vec<u8>, ScanError> {
    let mut child = command
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|_| ScanError::Spawn)?;
    drop(command);
    let mut stdout = child.stdout.take().ok_or(ScanError::Spawn)?;
    let limit = request.max_output;
    let reader = thread::spawn(move || -> Result<Vec<u8>, ()> {
        let mut output = Vec::new();
        let mut chunk = [0u8; 64 * 1024];
        loop {
            match stdout.read(&mut chunk) {
                Ok(0) => return Ok(output),
                Ok(read) if output.len() + read <= limit => {
                    output.extend_from_slice(&chunk[..read])
                }
                _ => return Err(()),
            }
        }
    });
    let deadline = Instant::now() + request.timeout;
    let mut reader = Some(reader);
    let mut output = None;
    let status = loop {
        // The reader stops before the exit only at end of output, past the limit, or on a pipe error.
        if reader.as_ref().is_some_and(|reader| reader.is_finished()) {
            let read = reader
                .take()
                .and_then(|reader| reader.join().ok())
                .unwrap_or(Err(()));
            if read.is_err() {
                let _ = child.kill();
                let _ = child.wait();
                return Err(ScanError::OutputTooLarge);
            }
            output = Some(read);
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(ScanError::Timeout);
            }
            Ok(None) => thread::sleep(Duration::from_millis(20)),
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(ScanError::Spawn);
            }
        }
    };
    let output = match output {
        Some(output) => output,
        None => reader
            .take()
            .and_then(|reader| reader.join().ok())
            .unwrap_or(Err(())),
    };
    if !status.success() {
        return Err(ScanError::Failed(status.code()));
    }
    output.map_err(|_| ScanError::OutputTooLarge)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::ffi::OsStringExt;
    use std::os::unix::fs::{PermissionsExt, symlink};

    fn script(dir: &Path, name: &str, body: &str) -> (std::path::PathBuf, String) {
        let path = dir.join(name);
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
        let digest = sha256_hex(&std::fs::read(&path).unwrap());
        (path, digest)
    }

    struct Dirs {
        _root: tempfile::TempDir,
        bin: std::path::PathBuf,
        home: std::path::PathBuf,
        logs: std::path::PathBuf,
    }

    fn dirs() -> Dirs {
        let root = tempfile::tempdir().unwrap();
        let [bin, home, logs] = ["bin", "home", "logs"].map(|name| root.path().join(name));
        for dir in [&bin, &home, &logs] {
            std::fs::create_dir(dir).unwrap();
        }
        Dirs {
            _root: root,
            bin,
            home,
            logs,
        }
    }

    fn request<'a>(binary: &'a Path, digest: &'a str, dirs: &'a Dirs) -> ScanRequest<'a> {
        ScanRequest {
            binary,
            expected_sha256: digest,
            tool: "claude",
            folder: &dirs.logs,
            empty_home: &dirs.home,
            timezone: "UTC",
            since: None,
            timeout: Duration::from_secs(5),
            max_output: 1 << 20,
        }
    }

    const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");

    // One test, because scans share one process-wide lock.
    #[test]
    fn runner_refuses_unsafe_runs_and_returns_parsed_rows() {
        let _test = TEST_RUNS.lock().unwrap();
        let d = dirs();
        let bin = d.bin.as_path();

        // A digest mismatch never runs the binary.
        let marker = bin.join("ran");
        let (path, _) = script(bin, "mismatch", &format!("touch {}", marker.display()));
        assert_eq!(scan(&request(&path, "00", &d)), Err(ScanError::Digest));
        assert!(!marker.exists());

        // Fixed arguments, a minimal environment that names only the consented folder, and parsed output.
        let (path, digest) = script(
            bin,
            "ok",
            &format!(
                "printf '%s\\n' \"$@\" > {0}/args; env > {0}/env; pwd > {0}/pwd; cat {FIXTURES}/ccusage-20.0.48-daily-by-agent-claude.json",
                bin.display(),
            ),
        );
        let rows = scan(&request(&path, &digest, &d)).unwrap();
        assert_eq!(rows.len(), 3);
        assert!(rows.iter().all(|row| row.tool == "claude"));
        assert_eq!(
            std::fs::read_to_string(bin.join("args")).unwrap(),
            "daily\n--json\n--offline\n--by-agent\n--no-color\n--mode\ncalculate\n--timezone\nUTC\n"
        );
        let env = std::fs::read_to_string(bin.join("env")).unwrap();
        let vars: std::collections::BTreeMap<&str, &str> = env
            .lines()
            .filter_map(|line| line.split_once('='))
            .filter(|(name, _)| !["PWD", "SHLVL", "_", "OLDPWD"].contains(name))
            .collect();
        let home = d.home.to_str().unwrap();
        let logs = d.logs.to_str().unwrap();
        assert_eq!(
            vars,
            [
                ("CLAUDE_CONFIG_DIR", logs),
                ("HOME", home),
                ("NO_COLOR", "1"),
                ("TZ", "UTC")
            ]
            .into_iter()
            .collect()
        );
        assert_eq!(
            std::fs::read_to_string(bin.join("pwd")).unwrap().trim(),
            home
        );

        // A row of a tool that the run did not ask for rejects the whole run.
        let (path, digest) = script(
            bin,
            "other",
            &format!("cat {FIXTURES}/ccusage-20.0.48-daily-by-agent.json"),
        );
        assert!(matches!(
            scan(&request(&path, &digest, &d)),
            Err(ScanError::Report(_))
        ));

        // Failures stay distinct.
        let (path, digest) = script(bin, "fail", "exit 3");
        assert_eq!(
            scan(&request(&path, &digest, &d)),
            Err(ScanError::Failed(Some(3)))
        );
        let (path, digest) = script(bin, "slow", "sleep 10");
        let mut slow = request(&path, &digest, &d);
        slow.timeout = Duration::from_millis(300);
        let started = Instant::now();
        assert_eq!(scan(&slow), Err(ScanError::Timeout));
        assert!(started.elapsed() < Duration::from_secs(5));
        let (path, digest) = script(bin, "loud", "head -c 3000000 /dev/zero");
        let mut loud = request(&path, &digest, &d);
        loud.max_output = 1000;
        assert_eq!(scan(&loud), Err(ScanError::OutputTooLarge));
        let (path, digest) = script(bin, "garbage", "echo not-json");
        assert!(matches!(
            scan(&request(&path, &digest, &d)),
            Err(ScanError::Report(_))
        ));

        // Invalid requests never run anything.
        let (path, digest) = script(bin, "never", &format!("touch {}", marker.display()));
        let mut bad = request(&path, &digest, &d);
        bad.timezone = "../etc";
        assert_eq!(scan(&bad), Err(ScanError::InvalidRequest));
        bad.timezone = "UTC";
        bad.since = Some("2026-1-1");
        assert_eq!(scan(&bad), Err(ScanError::InvalidRequest));
        bad.since = None;
        for tool in crate::tools::tools().iter().filter(|tool| !tool.enabled) {
            bad.tool = &tool.id;
            assert_eq!(scan(&bad), Err(ScanError::InvalidRequest));
            assert_eq!(scan_sessions(&bad), Err(ScanError::InvalidRequest));
            assert_eq!(scan_hours(&bad), Err(ScanError::InvalidRequest));
            assert!(
                !marker.exists(),
                "A disabled tool must not start the scanner."
            );
        }
        bad.tool = "claude";
        let missing = d.bin.join("missing");
        bad.folder = &missing;
        assert_eq!(scan(&bad), Err(ScanError::InvalidRequest));
        let invalid = [
            d.bin.join("one,two"),
            d.bin.join("trailing "),
            d.bin.join("trailing\u{2003}"),
            d.bin
                .join(std::ffi::OsString::from_vec(b"invalid-\xff".to_vec())),
        ];
        for folder in &invalid {
            std::fs::create_dir(folder).unwrap();
        }
        let linked = d.bin.join("replaced-root");
        std::fs::create_dir(&linked).unwrap();
        std::fs::remove_dir(&linked).unwrap();
        symlink(&d.logs, &linked).unwrap();
        for tool in ["claude", "codex"] {
            bad.tool = tool;
            for folder in invalid.iter().chain([&linked]) {
                bad.folder = folder;
                assert_eq!(scan(&bad), Err(ScanError::InvalidRequest));
                assert_eq!(scan_sessions(&bad), Err(ScanError::InvalidRequest));
                if tool == "claude" {
                    assert_eq!(scan_hours(&bad), Err(ScanError::InvalidRequest));
                }
                assert!(
                    !marker.exists(),
                    "An invalid path must not start the scanner."
                );
            }
        }
        bad.tool = "claude";
        bad.folder = &d.logs;
        // HOME must be empty, so that no default folder of another tool exists.
        std::fs::create_dir(d.home.join(".codex")).unwrap();
        assert_eq!(scan(&bad), Err(ScanError::InvalidRequest));
        std::fs::remove_dir(d.home.join(".codex")).unwrap();
        assert!(!marker.exists());

        // Only one scan runs at a time.
        let held = ONE_RUN.lock().unwrap();
        assert_eq!(scan(&request(&path, &digest, &d)), Err(ScanError::Busy));
        drop(held);
    }
}
