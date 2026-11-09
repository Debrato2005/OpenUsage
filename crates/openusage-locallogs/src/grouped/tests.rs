use super::*;
use crate::{UsagePeriod, tools::RootMode};
use std::os::unix::fs::{PermissionsExt, symlink};

fn scan_grouped_inner(request: &GroupedScanRequest<'_>) -> Result<GroupedScanResult, ScanError> {
    scan_grouped_checked(request, || true)
}

fn script(root: &Path, body: &str) -> (std::path::PathBuf, String) {
    let path = root.join("scanner");
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
    let hash = crate::sha256_hex(&std::fs::read(&path).unwrap());
    (path, hash)
}

#[test]
fn grouped_reports_keep_scope_policy_isolation_and_failure_boundaries() {
    let _test = crate::runner::TEST_RUNS.lock().unwrap();
    let temp = tempfile::Builder::new()
        .permissions(std::fs::Permissions::from_mode(0o700))
        .tempdir()
        .unwrap();
    let root = temp.path();
    let first = root.join("a");
    let second = root.join("b");
    let home = root.join("home");
    for dir in [&first, &second, &home] {
        std::fs::create_dir(dir).unwrap();
    }
    let folders = [second.as_path(), first.as_path(), first.as_path()];
    let fixture: serde_json::Value = serde_json::from_slice(include_bytes!(
        "../../tests/fixtures/remaining-tools/expected.json"
    ))
    .unwrap();
    for definition in crate::tools().iter().filter(|t| t.agent_report.is_some()) {
        let id = definition.id.as_str();
        for kind in ["daily", "session"] {
            std::fs::write(
                root.join(kind),
                serde_json::to_vec(&fixture[id][kind]).unwrap(),
            )
            .unwrap();
        }
        std::fs::write(root.join("snapshot"), serde_json::to_vec(&serde_json::json!({
            "schemaVersion":1,"tool":"pi","daily":fixture[id]["daily"],"session":fixture[id]["session"]
        })).unwrap()).unwrap();
        let variable = definition.folder_variable.as_deref().unwrap();
        let (binary, digest) = script(
            root,
            &format!(
                "printf '%s\\n' \"${variable}\" >> {0}/roots; printf '%s\\n' \"$@\" >> {0}/args; env > {0}/env; if [ \"$2\" = snapshot ]; then cat {0}/snapshot; elif [ \"$1\" = daily ]; then cat {0}/daily; else cat {0}/session; fi",
                root.display()
            ),
        );
        let single = [first.as_path()];
        let request = GroupedScanRequest {
            binary: &binary,
            expected_sha256: &digest,
            tool: id,
            folders: if definition.root_mode == RootMode::Single {
                &single
            } else {
                &folders
            },
            empty_home: &home,
            timezone: "Asia/Kolkata",
            timeout: Duration::from_secs(5),
            max_output: 1 << 20,
        };
        // Public entry points keep every candidate disabled. The private runner qualifies the mechanics only.
        assert_eq!(scan_grouped(&request), Err(ScanError::InvalidRequest));
        let report = scan_grouped_inner(&request).unwrap();
        assert_eq!(scan_grouped_inner(&request).unwrap(), report);
        assert!(!report.periods.is_empty() && !report.sessions.is_empty());
        let dated = definition.agent_report.as_ref().unwrap().period == PeriodPolicy::Day;
        assert_eq!(
            matches!(report.periods[0].period, UsagePeriod::Day(_)),
            dated
        );
        let roots = std::fs::read_to_string(root.join("roots")).unwrap();
        let expected = if definition.root_mode == RootMode::Single {
            first.display().to_string()
        } else {
            format!("{},{}", first.display(), second.display())
        };
        assert!(roots.lines().all(|line| line == expected));
        assert_eq!(
            roots.lines().count(),
            if dated && id != "pi" { 4 } else { 2 }
        );
        let env = std::fs::read_to_string(root.join("env")).unwrap();
        assert!(env.contains(&format!("HOME={}\n", home.display())));
        for other in crate::tools().iter().filter(|t| t.id != id) {
            assert!(!env.contains(&format!("{}=", other.folder_variable.as_deref().unwrap())));
        }
        let args = std::fs::read_to_string(root.join("args")).unwrap();
        assert!(args.contains("--timezone\nAsia/Kolkata\n"));
        assert_eq!(args.contains("daily\n"), dated && id != "pi");
        assert!(!args.contains("--since"));
        std::fs::remove_file(root.join("roots")).unwrap();
        std::fs::remove_file(root.join("args")).unwrap();
        if definition.root_mode == RootMode::Single {
            assert_eq!(
                scan_grouped_inner(&GroupedScanRequest {
                    folders: &folders,
                    ..request
                }),
                Err(ScanError::InvalidRequest)
            );
        }
    }
    // All required reports must parse. A valid daily report cannot hide a failed session report.
    std::fs::write(
        root.join("daily"),
        serde_json::to_vec(&fixture["amp"]["daily"]).unwrap(),
    )
    .unwrap();
    let (binary, digest) = script(
        root,
        &format!(
            "if [ \"$1\" = daily ]; then cat {}/daily; else echo broken; fi",
            root.display()
        ),
    );
    let mut request = GroupedScanRequest {
        binary: &binary,
        expected_sha256: &digest,
        tool: "amp",
        folders: &folders,
        empty_home: &home,
        timezone: "UTC",
        timeout: Duration::from_secs(5),
        max_output: 1 << 20,
    };
    assert!(matches!(
        scan_grouped_inner(&request),
        Err(ScanError::Report(_))
    ));
    request.max_output = 1;
    assert_eq!(scan_grouped_inner(&request), Err(ScanError::OutputTooLarge));
    request.max_output = 1 << 20;
    let empty: [&Path; 0] = [];
    request.folders = &empty;
    assert_eq!(scan_grouped_inner(&request), Err(ScanError::InvalidRequest));
    let alias = root.join("alias");
    symlink(&first, &alias).unwrap();
    let aliases = [alias.as_path()];
    request.folders = &aliases;
    assert_eq!(scan_grouped_inner(&request), Err(ScanError::InvalidRequest));
    let comma = root.join("a,b");
    std::fs::create_dir(&comma).unwrap();
    let invalid = [comma.as_path()];
    request.folders = &invalid;
    assert_eq!(scan_grouped_inner(&request), Err(ScanError::InvalidRequest));

    // Empty full reports are not dated zeros or fabricated sessions.
    let counts = serde_json::json!({"inputTokens":0,"outputTokens":0,"cacheCreationTokens":0,"cacheReadTokens":0,"totalTokens":0,"totalCost":0});
    std::fs::write(
        root.join("daily"),
        serde_json::to_vec(&serde_json::json!({"daily":[],"totals":counts})).unwrap(),
    )
    .unwrap();
    std::fs::write(
        root.join("session"),
        serde_json::to_vec(&serde_json::json!({"sessions":[],"totals":counts})).unwrap(),
    )
    .unwrap();
    let (binary, digest) = script(
        root,
        &format!(
            "if [ \"$1\" = daily ]; then cat {0}/daily; else cat {0}/session; fi",
            root.display()
        ),
    );
    for id in ["amp", "opencode"] {
        let empty = scan_grouped_inner(&GroupedScanRequest {
            binary: &binary,
            expected_sha256: &digest,
            tool: id,
            folders: &folders,
            ..request
        })
        .unwrap();
        assert!(empty.periods.is_empty() && empty.sessions.is_empty());
    }
    // Revocation after the daily process prevents the session process from starting.
    let mut checks = 0;
    assert_eq!(
        scan_grouped_checked(
            &GroupedScanRequest {
                binary: &binary,
                expected_sha256: &digest,
                tool: "amp",
                folders: &folders,
                ..request
            },
            || {
                checks += 1;
                checks == 1
            }
        ),
        Err(ScanError::InvalidRequest)
    );
    assert_eq!(checks, 2);
}

#[test]
fn pi_grouped_scan_uses_one_snapshot_process() {
    let _test = crate::runner::TEST_RUNS.lock().unwrap();
    let temp = tempfile::Builder::new()
        .permissions(std::fs::Permissions::from_mode(0o700))
        .tempdir()
        .unwrap();
    let root = temp.path();
    let folder = root.join("sessions");
    let home = root.join("home");
    for path in [&folder, &home] {
        std::fs::create_dir(path).unwrap();
    }
    let fixture: serde_json::Value = serde_json::from_slice(include_bytes!(
        "../../tests/fixtures/remaining-tools/pi-identity-candidate.json"
    ))
    .unwrap();
    let baseline = &fixture["baseline"];
    let snapshot = serde_json::json!({"schemaVersion":1,"tool":"pi","daily":baseline["daily"],"session":baseline["session"]});
    for (name, report) in [
        ("daily", &baseline["daily"]),
        ("session", &baseline["session"]),
        ("snapshot", &snapshot),
    ] {
        std::fs::write(root.join(name), serde_json::to_vec(report).unwrap()).unwrap();
    }
    let (binary, digest) = script(
        root,
        &format!(
            "printf '%s\\n' \"$*\" >> {0}/calls; if [ \"$2\" = snapshot ]; then cat {0}/snapshot; elif [ \"$1\" = daily ]; then cat {0}/daily; else cat {0}/session; fi",
            root.display()
        ),
    );
    let request = GroupedScanRequest {
        binary: &binary,
        expected_sha256: &digest,
        tool: "pi",
        folders: &[folder.as_path()],
        empty_home: &home,
        timezone: "UTC",
        timeout: Duration::from_secs(5),
        max_output: 1 << 20,
    };
    let result = scan_grouped_inner(&request).unwrap();
    assert_eq!(result.periods[0].total_tokens, 162);
    assert_eq!(result.sessions[0].total_tokens, 162);
    let calls = std::fs::read_to_string(root.join("calls")).unwrap();
    assert_eq!(
        calls.lines().count(),
        1,
        "Both reports must use one scanner load."
    );
    assert!(calls.starts_with("pi snapshot "));
    std::fs::remove_file(root.join("calls")).unwrap();
    let mut checks = 0;
    assert_eq!(
        scan_grouped_checked(&request, || {
            checks += 1;
            checks == 1
        }),
        Err(ScanError::InvalidRequest)
    );
    assert_eq!(checks, 2);
    assert_eq!(
        std::fs::read_to_string(root.join("calls"))
            .unwrap()
            .lines()
            .count(),
        1
    );
    std::fs::remove_file(root.join("calls")).unwrap();
    std::fs::write(root.join("snapshot"), b"{\"schemaVersion\":2}").unwrap();
    assert!(matches!(
        scan_grouped_inner(&request),
        Err(ScanError::Report(_))
    ));
    assert_eq!(
        std::fs::read_to_string(root.join("calls"))
            .unwrap()
            .lines()
            .count(),
        1
    );
    std::fs::write(
        root.join("snapshot"),
        serde_json::to_vec(&snapshot).unwrap(),
    )
    .unwrap();
    let limited = GroupedScanRequest {
        max_output: 1,
        ..request
    };
    assert_eq!(scan_grouped_inner(&limited), Err(ScanError::OutputTooLarge));
}
