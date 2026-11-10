#![cfg(target_os = "linux")]

use openusage_locallogs::{
    CCUSAGE_VERSION, DEFAULT_MAX_OUTPUT, DEFAULT_TIMEOUT, GroupedScanRequest, ScanError,
    grouped::qualify_pi_candidate,
};
use openusage_storage::{GroupedLocalScan, LogConsent, Storage};
use serde_json::{Value, json};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};

const KEY: [u8; 32] = [73; 32];
const NOW: &str = "2026-11-10T12:00:00.000Z";

#[test]
#[ignore = "run through the Linux Pi scale fixture"]
fn pi_scale_workload() {
    let case: Value = serde_json::from_str(&env::var("OPENUSAGE_PI_SCALE_CASE").unwrap()).unwrap();
    let field = |key: &str| case[key].as_str().unwrap();
    let binary = PathBuf::from(env::var_os("OPENUSAGE_PI_QUALIFICATION_BIN").unwrap());
    let pin: Value = serde_json::from_str(include_str!(
        "../../../tests/tools/fixtures/pi-producer/pin.json"
    ))
    .unwrap();
    let digest_key = match env::var("OPENUSAGE_PI_QUALIFICATION_VARIANT").as_deref() {
        Err(env::VarError::NotPresent) => "scannerSha256",
        Ok("bounded") => "boundedScannerSha256",
        _ => panic!("Unknown Pi qualification variant"),
    };
    let root = PathBuf::from(field("root"));
    let home = root.join("empty-home");
    let database = root.join(field("database"));
    let mut roots: Vec<_> = fs::read_dir(root.join("selected"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    roots.sort_unstable();
    let folders: Vec<_> = roots.iter().map(|path| path.as_path()).collect();
    let request = GroupedScanRequest {
        binary: &binary,
        expected_sha256: pin[digest_key].as_str().unwrap(),
        tool: "pi",
        folders: &folders,
        empty_home: &home,
        timezone: "UTC",
        timeout: case["timeoutMillis"]
            .as_u64()
            .map_or(DEFAULT_TIMEOUT, Duration::from_millis),
        max_output: case["maxOutput"]
            .as_u64()
            .map_or(DEFAULT_MAX_OUTPUT, |bytes| usize::try_from(bytes).unwrap()),
    };
    let expected = case["tokens"].as_u64().unwrap();
    let started = Instant::now();
    let report = qualify_pi_candidate(&request, || true);
    let scan_seconds = started.elapsed().as_secs_f64();

    let outcome = field("outcome");
    if outcome != "accepted" {
        match outcome {
            "candidate" => assert!(matches!(report, Err(ScanError::Failed(Some(_))))),
            "roots" => assert_eq!(report, Err(ScanError::InvalidRequest)),
            "output" => assert_eq!(report, Err(ScanError::OutputTooLarge)),
            "deadline" => assert_eq!(report, Err(ScanError::Timeout)),
            _ => panic!("Unknown refusal case"),
        }
        let storage = Storage::open(&database, &KEY).unwrap();
        let scope = storage.local_tool_scope("pi").unwrap().unwrap();
        assert_eq!(scope.latest_scan.id, field("scanId"));
        assert_eq!(scope.time_zone.as_deref(), Some("UTC"));
        let days = storage.local_periods("pi").unwrap();
        let sessions = storage.grouped_local_sessions("pi").unwrap();
        assert_eq!(days.len() as u64, case["days"].as_u64().unwrap());
        assert_eq!(sessions.len() as u64, case["sessions"].as_u64().unwrap());
        assert_eq!(
            days.iter().map(|row| row.period.total_tokens).sum::<u64>(),
            expected
        );
        assert_eq!(
            sessions
                .iter()
                .map(|row| row.session.total_tokens)
                .sum::<u64>(),
            expected
        );
        assert!(days.iter().all(|row| row.scan == scope.latest_scan));
        assert!(sessions.iter().all(|row| row.scan == scope.latest_scan));
        assert!(storage.local_scan_differences("pi").unwrap().is_empty());
        println!(
            "PI_SCALE {}",
            json!({"scanSeconds":scan_seconds,"outcome":outcome})
        );
        return;
    }

    let report = report.unwrap();
    assert_eq!(report.periods.len() as u64, case["days"].as_u64().unwrap());
    assert_eq!(
        report.sessions.len() as u64,
        case["sessions"].as_u64().unwrap()
    );
    assert_eq!(
        report
            .periods
            .iter()
            .map(|row| row.total_tokens)
            .sum::<u64>(),
        expected
    );
    assert_eq!(
        report
            .sessions
            .iter()
            .map(|row| row.total_tokens)
            .sum::<u64>(),
        expected
    );

    let mut storage = Storage::create(&database, &KEY).unwrap();
    let source_ids: Vec<_> = roots
        .iter()
        .map(|path| {
            storage
                .set_log_source("pi", path.to_str().unwrap(), LogConsent::Granted, NOW)
                .unwrap()
                .id
        })
        .collect();
    let started = Instant::now();
    storage
        .save_grouped_local_scan(&GroupedLocalScan {
            tool: "pi",
            source_ids: &source_ids,
            time_zone: Some("UTC"),
            scanner_version: CCUSAGE_VERSION,
            observed_at_utc: NOW,
            periods: &report.periods,
            sessions: Some(&report.sessions),
        })
        .unwrap();
    let save_seconds = started.elapsed().as_secs_f64();
    drop(storage);
    let storage = Storage::open(&database, &KEY).unwrap();
    let scope = storage.local_tool_scope("pi").unwrap().unwrap();
    let days = storage.local_periods("pi").unwrap();
    let sessions = storage.grouped_local_sessions("pi").unwrap();
    assert_eq!(days.len(), report.periods.len());
    assert_eq!(sessions.len(), report.sessions.len());
    for (saved, parsed) in days.iter().zip(&report.periods) {
        assert_eq!(&saved.period, parsed);
    }
    let mut parsed = report.sessions;
    parsed.sort_unstable_by(|a, b| a.session_key.cmp(&b.session_key));
    for (saved, parsed) in sessions.iter().zip(parsed) {
        assert_eq!(saved.session, parsed);
    }
    println!(
        "PI_SCALE {}",
        json!({
            "scanSeconds": scan_seconds,
            "saveSeconds": save_seconds,
            "scanId": scope.latest_scan.id,
            "timeoutSeconds": DEFAULT_TIMEOUT.as_secs(),
            "outputBound": DEFAULT_MAX_OUTPUT,
            "schema": openusage_storage::CURRENT_SCHEMA_VERSION,
        })
    );
}
