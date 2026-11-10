#![cfg(target_os = "linux")]

use openusage_locallogs::{
    DEFAULT_MAX_OUTPUT, GroupedScanRequest, GroupedScanResult, ScanError, TokenDetails,
    UsagePeriod, grouped::qualify_pi_candidate, scan_grouped,
};
use openusage_storage::{GroupedLocalScan, LocalLogError, LogConsent, Storage};
use std::env;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

const NOW: &str = "2026-11-10T12:00:00.000Z";
const ZONE: &str = "Asia/Kolkata";
const KEY: [u8; 32] = [71; 32];

fn writer(source: &Path, root: &Path, action: &str) -> Command {
    let harness =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/tools/pi_grouped_fixture.py");
    let mut command = Command::new("python3");
    command
        .arg(harness)
        .arg(source)
        .arg(root)
        .arg(action)
        .env_clear()
        .env("PATH", env::var_os("PATH").unwrap())
        .env("HOME", root)
        .env("PYTHONDONTWRITEBYTECODE", "1");
    command
}

fn produce(source: &Path, root: &Path, action: &str) -> PathBuf {
    let output = writer(source, root, action).output().unwrap();
    assert!(output.status.success(), "{action}: {:?}", output.stderr);
    PathBuf::from(String::from_utf8(output.stdout).unwrap().trim())
}

fn capture_prefix(source: &Path, root: &Path, stage: &str, capture: impl FnOnce()) -> PathBuf {
    let mut child = writer(source, root, "flush")
        .arg(stage)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let mut line = String::new();
    stdout.read_line(&mut line).unwrap();
    assert_eq!(line.trim(), stage);
    capture();
    child.stdin.take().unwrap().write_all(b"\n").unwrap();
    line.clear();
    stdout.read_line(&mut line).unwrap();
    assert!(child.wait().unwrap().success());
    PathBuf::from(line.trim())
}

fn assert_totals(report: &GroupedScanResult, expected: Option<u64>, sessions: &[(&str, u64)]) {
    assert_eq!(report.periods.len(), usize::from(expected.is_some()));
    if let Some(total) = expected {
        assert_eq!(
            report.periods[0].period,
            UsagePeriod::Day("2026-05-29".into())
        );
        assert_eq!(report.periods[0].total_tokens, total);
    }
    let mut actual: Vec<_> = report
        .sessions
        .iter()
        .map(|row| (row.session_key.as_str(), row.total_tokens))
        .collect();
    actual.sort_unstable();
    assert_eq!(actual, sessions);
    assert_eq!(
        report
            .sessions
            .iter()
            .map(|row| row.total_tokens)
            .sum::<u64>(),
        expected.unwrap_or(0)
    );
}

#[test]
#[ignore = "needs the pinned Linux candidate and Pi writer"]
fn pi_candidate_flows_from_pinned_writer_to_encrypted_history() {
    let binary = PathBuf::from(env::var_os("OPENUSAGE_PI_QUALIFICATION_BIN").unwrap());
    let source = PathBuf::from(env::var_os("OPENUSAGE_PI_PRODUCER_SOURCE").unwrap());
    let pin: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/tools/fixtures/pi-producer/pin.json"
    ))
    .unwrap();
    let temp = tempfile::Builder::new()
        .permissions(fs::Permissions::from_mode(0o700))
        .tempdir()
        .unwrap();
    let root = temp.path().canonicalize().unwrap();
    let sessions = root.join("sessions");
    let copies = root.join("copies");
    let empty_home = root.join("empty-home");
    let decoy = root.join("unselected");
    for path in [&sessions, &copies, &empty_home, &decoy] {
        fs::create_dir(path).unwrap();
    }
    let decoy_file = produce(&source, &decoy, "seed");
    let decoy_bytes = fs::read(&decoy_file).unwrap();
    let request = GroupedScanRequest {
        binary: &binary,
        expected_sha256: pin["scannerSha256"].as_str().unwrap(),
        tool: "pi",
        folders: &[copies.as_path(), sessions.as_path(), copies.as_path()],
        empty_home: &empty_home,
        timezone: ZONE,
        timeout: Duration::from_secs(10),
        max_output: DEFAULT_MAX_OUTPUT,
    };
    assert_eq!(scan_grouped(&request), Err(ScanError::InvalidRequest));
    let wrong_pin = GroupedScanRequest {
        expected_sha256: &"0".repeat(64),
        ..request
    };
    assert_eq!(
        qualify_pi_candidate(&wrong_pin, || true),
        Err(ScanError::Digest)
    );
    let capture = || qualify_pi_candidate(&request, || true).unwrap();

    let database = root.join("usage.db");
    let mut storage = Storage::create(&database, &KEY).unwrap();
    let consent = |storage: &mut Storage, path: &Path, state| {
        storage
            .set_log_source("pi", path.to_str().unwrap(), state, NOW)
            .unwrap()
            .id
    };
    let selected = [
        consent(&mut storage, &sessions, LogConsent::Granted),
        consent(&mut storage, &copies, LogConsent::Granted),
    ];
    let save = |storage: &mut Storage, report: &GroupedScanResult, zone| {
        storage.save_grouped_local_scan(&GroupedLocalScan {
            tool: "pi",
            source_ids: &selected,
            time_zone: Some(zone),
            scanner_version: "20.0.48",
            observed_at_utc: NOW,
            periods: &report.periods,
            sessions: Some(&report.sessions),
        })
    };
    let capture_and_save = |storage: &mut Storage, total, sessions: &[(&str, u64)]| {
        let report = capture();
        assert_totals(&report, total, sessions);
        save(storage, &report, ZONE).unwrap();
        report
    };

    capture_and_save(&mut storage, None, &[]);
    assert!(storage.local_periods("pi").unwrap().is_empty());
    assert!(storage.grouped_local_sessions("pi").unwrap().is_empty());

    let zero = produce(&source, &root, "zero");
    capture_and_save(&mut storage, Some(0), &[("root", 0)]);
    assert_eq!(
        storage.local_periods("pi").unwrap()[0].period.total_tokens,
        0
    );
    assert_eq!(
        storage.grouped_local_sessions("pi").unwrap()[0]
            .session
            .total_tokens,
        0
    );
    fs::remove_file(zero).unwrap();

    let completed = capture_prefix(&source, &root, "header", || {
        capture_and_save(&mut storage, None, &[]);
    });
    fs::remove_file(completed).unwrap();
    let completed = capture_prefix(&source, &root, "usage", || {
        capture_and_save(&mut storage, Some(17), &[("root", 17)]);
        assert_eq!(
            storage.local_periods("pi").unwrap()[0].period.total_tokens,
            17
        );
    });
    capture_and_save(&mut storage, Some(137), &[("root", 137)]);
    fs::remove_file(completed).unwrap();

    let full = produce(&source, &root, "total_only");
    let report = capture_and_save(&mut storage, Some(162), &[("root", 162)]);
    let scope = storage.local_tool_scope("pi").unwrap().unwrap();
    let days = storage.local_periods("pi").unwrap();
    let saved_sessions = storage.grouped_local_sessions("pi").unwrap();
    assert_eq!(scope.time_zone.as_deref(), Some(ZONE));
    assert_eq!(scope.latest_scan.source_ids.len(), 2);
    assert!(
        selected
            .iter()
            .all(|id| scope.latest_scan.source_ids.contains(id))
    );
    assert_eq!(days[0].scan, scope.latest_scan);
    assert_eq!(saved_sessions[0].scan, scope.latest_scan);
    assert_eq!(days[0].period, report.periods[0]);
    assert_eq!(saved_sessions[0].session, report.sessions[0]);
    assert_eq!(days[0].period.details, TokenDetails::default());
    assert_eq!(saved_sessions[0].session.details, TokenDetails::default());
    assert_eq!(days[0].period.estimated_value_micros, None);
    assert_eq!(saved_sessions[0].session.estimated_value_micros, None);

    let invalid = copies.join("invalid.jsonl");
    let original = fs::read_to_string(&full).unwrap();
    fs::write(&full, format!("{original}{{\"type\":")).unwrap();
    assert!(matches!(
        qualify_pi_candidate(&request, || true),
        Err(ScanError::Failed(_))
    ));
    fs::write(&full, &original).unwrap();
    fs::write(&invalid, b"{not json}\n").unwrap();
    assert!(matches!(
        qualify_pi_candidate(&request, || true),
        Err(ScanError::Failed(_))
    ));
    fs::remove_file(&invalid).unwrap();

    let permissions = fs::metadata(&full).unwrap().permissions();
    fs::set_permissions(&full, fs::Permissions::from_mode(0o000)).unwrap();
    let read_failure = fs::read(&full);
    let scan_failure = qualify_pi_candidate(&request, || true);
    fs::set_permissions(&full, permissions).unwrap();
    assert_eq!(
        read_failure.unwrap_err().kind(),
        std::io::ErrorKind::PermissionDenied
    );
    assert!(matches!(scan_failure, Err(ScanError::Failed(_))));

    symlink(decoy.join("missing.jsonl"), &invalid).unwrap();
    assert!(matches!(
        qualify_pi_candidate(&request, || true),
        Err(ScanError::Failed(_))
    ));
    fs::remove_file(&invalid).unwrap();
    let conflict = original.replace("\"totalTokens\":162", "\"totalTokens\":163");
    assert_ne!(conflict, original);
    fs::write(&invalid, conflict).unwrap();
    assert!(matches!(
        qualify_pi_candidate(&request, || true),
        Err(ScanError::Failed(_))
    ));
    fs::remove_file(&invalid).unwrap();

    let mut checks = 0;
    assert_eq!(
        qualify_pi_candidate(&request, || {
            checks += 1;
            if checks == 2 {
                consent(&mut storage, &copies, LogConsent::Withdrawn);
            }
            storage
                .log_sources()
                .unwrap()
                .iter()
                .filter(|s| s.consent == LogConsent::Granted)
                .count()
                == 2
        }),
        Err(ScanError::InvalidRequest)
    );
    assert_eq!(checks, 2);
    assert_eq!(
        save(&mut storage, &report, ZONE),
        Err(LocalLogError::NotConsented)
    );
    consent(&mut storage, &copies, LogConsent::Granted);
    assert_eq!(
        save(&mut storage, &report, "UTC"),
        Err(LocalLogError::TimeZoneMismatch)
    );
    assert_eq!(storage.local_tool_scope("pi").unwrap().unwrap(), scope);
    assert_eq!(storage.local_periods("pi").unwrap(), days);
    assert_eq!(
        storage.grouped_local_sessions("pi").unwrap(),
        saved_sessions
    );

    fs::remove_file(full).unwrap();
    let parent = produce(&source, &root, "seed");
    produce(&source, &root, "fork");
    fs::copy(parent, copies.join("renamed.jsonl")).unwrap();
    let fork = capture_and_save(&mut storage, Some(151), &[("fork", 31), ("root", 120)]);
    assert_eq!(capture(), fork);
    drop(storage);

    let storage = Storage::open(&database, &KEY).unwrap();
    assert_eq!(
        storage
            .local_tool_scope("pi")
            .unwrap()
            .unwrap()
            .time_zone
            .as_deref(),
        Some(ZONE)
    );
    let kept_day = &storage.local_periods("pi").unwrap()[0];
    assert!(kept_day.retained);
    assert_eq!(kept_day.period, days[0].period);
    let sessions = storage.grouped_local_sessions("pi").unwrap();
    let kept_session = sessions
        .iter()
        .find(|row| row.session.session_key == "root")
        .unwrap();
    assert!(kept_session.retained);
    assert_eq!(kept_session.session, saved_sessions[0].session);
    assert!(
        storage
            .local_scan_differences("pi")
            .unwrap()
            .iter()
            .any(|difference| {
                difference.saved_total_tokens == 162 && difference.new_total_tokens == 151
            })
    );
    assert_eq!(fs::read(decoy_file).unwrap(), decoy_bytes);
    assert!(fs::read_dir(empty_home).unwrap().next().is_none());
}
