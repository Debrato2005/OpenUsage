#![cfg(target_os = "linux")]
//! Real owned processes exercise cancellation through the public scanner API.
use openusage_locallogs::{
    ScanCancellation, ScanError, ScanRequest, scan_with_cancellation, sha256_hex,
};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const FIXTURE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/native/fixtures/synthetic-scanner.py"
));

fn wait_for(mut check: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !check() {
        assert!(
            Instant::now() < deadline,
            "private fixture did not reach its boundary"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn events(folder: &Path) -> Vec<serde_json::Value> {
    fs::read_to_string(folder.join("synthetic-scanner-events.jsonl"))
        .unwrap_or_default()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

struct Keeper(Child);
impl Drop for Keeper {
    fn drop(&mut self) {
        // The helper owns the inherited-descriptor holder and reaps it on SIGTERM.
        let _ = Command::new("/bin/kill")
            .args(["-TERM", &self.0.id().to_string()])
            .status();
        let _ = self.0.wait();
    }
}

fn keeper(binary: &Path, folder: &Path) -> Keeper {
    let child = Command::new(binary)
        .arg("--pipe-keeper")
        .arg(folder)
        .env_clear()
        .env("HOME", folder)
        .env("PATH", "/usr/bin:/bin")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    wait_for(|| {
        events(folder)
            .iter()
            .any(|row| row["kind"] == "keeper-ready")
    });
    Keeper(child)
}

#[test]
fn public_scan_cancels_owned_children_with_inherited_output_and_continuous_output() {
    let root = tempfile::tempdir().unwrap();
    fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700)).unwrap();
    let binary = root.path().join("synthetic-scanner");
    fs::write(&binary, FIXTURE).unwrap();
    fs::set_permissions(&binary, fs::Permissions::from_mode(0o700)).unwrap();
    let digest = sha256_hex(FIXTURE);
    let home = root.path().join("home");
    fs::create_dir(&home).unwrap();
    for mode in ["slow", "held-exit", "flood", "deadline"] {
        let folder: PathBuf = root.path().join(mode);
        fs::create_dir(&folder).unwrap();
        fs::write(
            folder.join("synthetic-scanner-mode"),
            if mode == "deadline" {
                "held-exit"
            } else {
                mode
            },
        )
        .unwrap();
        let mut keeper = (mode != "flood").then(|| keeper(&binary, &folder));
        let cancellation = ScanCancellation::default();
        let request = ScanRequest {
            binary: &binary,
            expected_sha256: &digest,
            tool: "claude",
            folder: &folder,
            empty_home: &home,
            timezone: "UTC",
            since: None,
            timeout: if mode == "deadline" {
                Duration::from_millis(300)
            } else {
                Duration::from_secs(120)
            },
            max_output: 64 * 1024 * 1024,
        };
        // A cancelled request never starts a child, even with an invalid executable digest.
        let before = ScanCancellation::default();
        before.cancel();
        assert_eq!(
            scan_with_cancellation(&request, &before),
            Err(ScanError::Cancelled)
        );
        assert!(!events(&folder).iter().any(|row| row["kind"] == "start"));
        let started = Instant::now();
        let result = std::thread::scope(|scope| {
            let worker = scope.spawn(|| scan_with_cancellation(&request, &cancellation));
            wait_for(|| {
                events(&folder).iter().any(|row| {
                    row["kind"]
                        == if mode == "flood" {
                            "flood-started"
                        } else {
                            "inherited-pipe-ready"
                        }
                })
            });
            if mode != "deadline" {
                cancellation.cancel();
            }
            worker.join().unwrap()
        });
        assert_eq!(
            result,
            Err(if mode == "deadline" {
                ScanError::Timeout
            } else {
                ScanError::Cancelled
            })
        );
        assert!(started.elapsed() < Duration::from_secs(3));
        assert!(!cancellation.cleanup_failed());
        let rows = events(&folder);
        let scanner = rows.iter().find(|row| row["kind"] == "start").unwrap()["pid"]
            .as_u64()
            .unwrap();
        wait_for(|| !Path::new(&format!("/proc/{scanner}")).exists());
        if let Some(keeper) = keeper.as_mut() {
            assert!(
                keeper.0.try_wait().unwrap().is_none(),
                "external FD keeper must survive"
            );
            let holder = rows.iter().find(|row| row["kind"] == "pipe-held").unwrap()["holderPid"]
                .as_u64()
                .unwrap();
            assert!(
                Path::new(&format!("/proc/{holder}")).exists(),
                "test-owned inherited FD holder must survive"
            );
        }
    }
}
