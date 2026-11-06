//! Runs the pinned ccusage binary once (OU-LOG-005, OU-LOG-006): checked digest, fixed arguments,
//! no shell, an empty environment apart from HOME, offline pricing, a deadline and an output limit.

use crate::report::{ReportError, UsageRow, parse_daily_by_agent};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Mutex;
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

pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);
pub const DEFAULT_MAX_OUTPUT: usize = 64 * 1024 * 1024;

static ONE_RUN: Mutex<()> = Mutex::new(());

pub struct ScanRequest<'a> {
    pub binary: &'a Path,
    pub expected_sha256: &'a str,
    /// The home whose tool folders ccusage reads. The user consented to these tools (OU-LOG-001).
    pub home: &'a Path,
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

pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub fn scan(request: &ScanRequest<'_>) -> Result<Vec<UsageRow>, ScanError> {
    let _one = ONE_RUN.try_lock().map_err(|_| ScanError::Busy)?;
    if !valid_timezone(request.timezone)
        || request.since.is_some_and(|since| !valid_since(since))
        || !request.home.is_absolute()
    {
        return Err(ScanError::InvalidRequest);
    }
    let bytes = std::fs::read(request.binary).map_err(|_| ScanError::Unavailable)?;
    if sha256_hex(&bytes) != request.expected_sha256 {
        return Err(ScanError::Digest);
    }
    drop(bytes);
    let mut args = vec![
        "daily",
        "--json",
        "--offline",
        "--by-agent",
        "--no-color",
        "--timezone",
        request.timezone,
    ];
    if let Some(since) = request.since {
        args.extend(["--since", since]);
    }
    let mut child = Command::new(request.binary)
        .args(&args)
        .env_clear()
        .env("HOME", request.home)
        .env("NO_COLOR", "1")
        .env("TZ", "UTC")
        .current_dir(request.home)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| ScanError::Spawn)?;
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
    finish(status, output)
}

fn finish(
    status: std::process::ExitStatus,
    output: Result<Vec<u8>, ()>,
) -> Result<Vec<UsageRow>, ScanError> {
    if !status.success() {
        return Err(ScanError::Failed(status.code()));
    }
    let output = output.map_err(|_| ScanError::OutputTooLarge)?;
    parse_daily_by_agent(&output).map_err(ScanError::Report)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn script(dir: &Path, name: &str, body: &str) -> (std::path::PathBuf, String) {
        let path = dir.join(name);
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
        let digest = sha256_hex(&std::fs::read(&path).unwrap());
        (path, digest)
    }

    fn request<'a>(binary: &'a Path, digest: &'a str, home: &'a Path) -> ScanRequest<'a> {
        ScanRequest {
            binary,
            expected_sha256: digest,
            home,
            timezone: "UTC",
            since: None,
            timeout: Duration::from_secs(5),
            max_output: 1 << 20,
        }
    }

    // One test, because scans share one process-wide lock.
    #[test]
    fn runner_refuses_unsafe_runs_and_returns_parsed_rows() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let fixture = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/ccusage-20.0.48-daily-by-agent.json"
        );

        // A digest mismatch never runs the binary.
        let marker = home.join("ran");
        let (path, _) = script(home, "mismatch", &format!("touch {}", marker.display()));
        assert_eq!(scan(&request(&path, "00", home)), Err(ScanError::Digest));
        assert!(!marker.exists());

        // Fixed arguments, an empty environment apart from HOME, and parsed output.
        let (path, digest) = script(
            home,
            "ok",
            &format!(
                "printf '%s\\n' \"$@\" > {0}/args; env > {0}/env; cat {1}",
                home.display(),
                fixture
            ),
        );
        let rows = scan(&request(&path, &digest, home)).unwrap();
        assert_eq!(rows.len(), 4);
        assert_eq!(
            std::fs::read_to_string(home.join("args")).unwrap(),
            "daily\n--json\n--offline\n--by-agent\n--no-color\n--timezone\nUTC\n"
        );
        let env = std::fs::read_to_string(home.join("env")).unwrap();
        let names: Vec<&str> = env
            .lines()
            .filter_map(|line| line.split('=').next())
            .filter(|name| !["PWD", "SHLVL", "_", "OLDPWD"].contains(name))
            .collect();
        assert_eq!(
            names
                .iter()
                .copied()
                .collect::<std::collections::BTreeSet<_>>(),
            ["HOME", "NO_COLOR", "TZ"].into_iter().collect()
        );

        // Failures stay distinct.
        let (path, digest) = script(home, "fail", "exit 3");
        assert_eq!(
            scan(&request(&path, &digest, home)),
            Err(ScanError::Failed(Some(3)))
        );
        let (path, digest) = script(home, "slow", "sleep 10");
        let mut slow = request(&path, &digest, home);
        slow.timeout = Duration::from_millis(300);
        let started = Instant::now();
        assert_eq!(scan(&slow), Err(ScanError::Timeout));
        assert!(started.elapsed() < Duration::from_secs(5));
        let (path, digest) = script(home, "loud", "head -c 3000000 /dev/zero");
        let mut loud = request(&path, &digest, home);
        loud.max_output = 1000;
        assert_eq!(scan(&loud), Err(ScanError::OutputTooLarge));
        let (path, digest) = script(home, "garbage", "echo not-json");
        assert!(matches!(
            scan(&request(&path, &digest, home)),
            Err(ScanError::Report(_))
        ));

        // Invalid requests never run anything.
        let (path, digest) = script(home, "never", &format!("touch {}", marker.display()));
        let mut bad = request(&path, &digest, home);
        bad.timezone = "../etc";
        assert_eq!(scan(&bad), Err(ScanError::InvalidRequest));
        bad.timezone = "UTC";
        bad.since = Some("2026-1-1");
        assert_eq!(scan(&bad), Err(ScanError::InvalidRequest));
        assert!(!marker.exists());

        // Only one scan runs at a time.
        let held = ONE_RUN.lock().unwrap();
        assert_eq!(scan(&request(&path, &digest, home)), Err(ScanError::Busy));
        drop(held);
    }
}
