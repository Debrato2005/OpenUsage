//! Runs the real pinned ccusage binary against synthetic logs, one consented folder at a time.
//! Set OPENUSAGE_CCUSAGE_BIN to the provisioned binary; without it the test says so and passes.

use openusage_locallogs::{
    DEFAULT_MAX_OUTPUT, ScanError, ScanRequest, UsageRow, parse_daily_by_agent, parse_hours,
    parse_sessions, pinned_sha256, scan, scan_hours, scan_sessions,
};
use std::path::Path;
use std::time::Duration;

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target)
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

fn fixture_home(to: &Path) {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    copy_tree(&fixtures.join("home/.claude"), &to.join(".claude"));
    copy_tree(&fixtures.join("codex"), &to.join(".codex"));
}

fn request<'a>(
    binary: &'a Path,
    folder: &'a Path,
    empty: &'a Path,
    tool: &'a str,
) -> ScanRequest<'a> {
    ScanRequest {
        binary,
        expected_sha256: pinned_sha256("x86_64-unknown-linux-gnu").unwrap(),
        tool,
        folder,
        empty_home: empty,
        timezone: "UTC",
        since: None,
        timeout: Duration::from_secs(60),
        max_output: DEFAULT_MAX_OUTPUT,
    }
}

fn scan_tool(binary: &Path, home: &Path, tool: &str, folder: &str) -> Vec<UsageRow> {
    let empty = tempfile::tempdir().unwrap();
    scan(&request(binary, &home.join(folder), empty.path(), tool)).unwrap()
}

fn check_folder_boundaries(binary: &Path) {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    for (tool, source, child) in [
        ("claude", "home/.claude", "projects"),
        ("codex", "codex", "sessions"),
    ] {
        let root = tempfile::tempdir().unwrap();
        let first = root.path().join("first");
        let second = root.path().join("second");
        copy_tree(&fixtures.join(source), &first);
        copy_tree(&fixtures.join(source), &second);
        // Each literal folder is empty. Splitting or trimming it selects populated decoys.
        for folder in [
            std::path::PathBuf::from(format!("{},{}", first.display(), second.display())),
            std::path::PathBuf::from(format!("{} ", first.display())),
            std::path::PathBuf::from(format!("{}\u{2003}", first.display())),
        ] {
            std::fs::create_dir_all(folder.join(child)).unwrap();
            let empty = tempfile::tempdir().unwrap();
            let request = request(binary, &folder, empty.path(), tool);
            assert_eq!(
                scan(&request),
                Err(ScanError::InvalidRequest),
                "{tool}: {folder:?}"
            );
            assert_eq!(scan_sessions(&request), Err(ScanError::InvalidRequest));
            if tool == "claude" {
                assert_eq!(scan_hours(&request), Err(ScanError::InvalidRequest));
            }
        }
        // Spaces inside a path and Unicode names remain valid, with exact repeated results.
        let supported = root.path().join("usage logs café");
        copy_tree(&fixtures.join(source), &supported);
        let empty = tempfile::tempdir().unwrap();
        let expected = scan(&request(binary, &first, empty.path(), tool)).unwrap();
        let empty = tempfile::tempdir().unwrap();
        let supported_request = request(binary, &supported, empty.path(), tool);
        assert_eq!(scan(&supported_request).unwrap(), expected);
        assert_eq!(scan(&supported_request).unwrap(), expected);
        assert!(!scan_sessions(&supported_request).unwrap().is_empty());
        #[cfg(unix)]
        {
            let link = root.path().join("alias");
            std::os::unix::fs::symlink(&supported, &link).unwrap();
            // Detection resolves links before consent; the resolved root remains valid.
            let resolved = std::fs::canonicalize(link).unwrap();
            let empty = tempfile::tempdir().unwrap();
            assert_eq!(
                scan(&request(binary, &resolved, empty.path(), tool)).unwrap(),
                expected
            );
        }
    }
}

#[test]
fn pinned_binary_reads_synthetic_claude_and_codex_logs() {
    let Ok(binary) = std::env::var("OPENUSAGE_CCUSAGE_BIN") else {
        eprintln!("OPENUSAGE_CCUSAGE_BIN is not set; the pinned binary did not run");
        return;
    };
    let binary = Path::new(&binary);
    let home = tempfile::tempdir().unwrap();
    fixture_home(home.path());
    // One run for each consented folder. Together they equal one run over the whole home.
    let mut rows = scan_tool(binary, home.path(), "claude", ".claude");
    assert!(rows.iter().all(|row| row.tool == "claude"));
    let codex = scan_tool(binary, home.path(), "codex", ".codex");
    assert!(!codex.is_empty() && codex.iter().all(|row| row.tool == "codex"));
    rows.extend(codex);
    let mut expected = parse_daily_by_agent(include_bytes!(
        "fixtures/ccusage-20.0.48-daily-by-agent.json"
    ))
    .unwrap();
    let key = |row: &UsageRow| (row.date.clone(), row.tool, row.model.clone());
    rows.sort_by_key(key);
    expected.sort_by_key(key);
    assert_eq!(rows, expected);
    // The same logs give the same totals on a second scan (OU-LOG-012).
    let mut again = scan_tool(binary, home.path(), "claude", ".claude");
    again.extend(scan_tool(binary, home.path(), "codex", ".codex"));
    again.sort_by_key(key);
    assert_eq!(again, rows);
    // The session report of each tool matches its recorded output (SC-021).
    for (tool, folder, recorded) in [
        (
            "claude",
            ".claude",
            &include_bytes!("fixtures/ccusage-20.0.48-session-claude.json")[..],
        ),
        (
            "codex",
            ".codex",
            &include_bytes!("fixtures/ccusage-20.0.48-session-codex.json")[..],
        ),
    ] {
        let empty = tempfile::tempdir().unwrap();
        let folder = home.path().join(folder);
        let sessions = scan_sessions(&request(binary, &folder, empty.path(), tool)).unwrap();
        assert!(!sessions.is_empty());
        assert_eq!(sessions, parse_sessions(tool, recorded).unwrap());
    }
    // The hours of Claude Code match its recorded blocks report (SC-021).
    let empty = tempfile::tempdir().unwrap();
    let folder = home.path().join(".claude");
    let hours = scan_hours(&request(binary, &folder, empty.path(), "claude")).unwrap();
    assert_eq!(
        hours,
        parse_hours(include_bytes!(
            "fixtures/ccusage-20.0.48-blocks-claude.json"
        ))
        .unwrap()
    );

    // Stored charges, including zero, must not replace API-price estimates (OU-LOG-014).
    let projects = folder.join("projects/-home-dev-alpha");
    for name in [
        "11111111-aaaa-4aaa-8aaa-111111111111.jsonl",
        "33333333-cccc-4ccc-8ccc-333333333333.jsonl",
    ] {
        let path = projects.join(name);
        let contents = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<String> = contents
            .lines()
            .enumerate()
            .map(|(index, line)| {
                let mut row: serde_json::Value = serde_json::from_str(line).unwrap();
                row["costUSD"] = if name.starts_with('1') && index == 0 {
                    serde_json::json!(0)
                } else {
                    serde_json::json!(999)
                };
                serde_json::to_string(&row).unwrap()
            })
            .collect();
        std::fs::write(path, lines.join("\n") + "\n").unwrap();
    }
    assert_eq!(
        scan_tool(binary, home.path(), "claude", ".claude"),
        parse_daily_by_agent(include_bytes!(
            "fixtures/ccusage-20.0.48-daily-by-agent-claude.json"
        ))
        .unwrap()
    );
    let empty = tempfile::tempdir().unwrap();
    assert_eq!(
        scan_sessions(&request(binary, &folder, empty.path(), "claude")).unwrap(),
        parse_sessions(
            "claude",
            include_bytes!("fixtures/ccusage-20.0.48-session-claude.json")
        )
        .unwrap()
    );
    let empty = tempfile::tempdir().unwrap();
    assert_eq!(
        scan_hours(&request(binary, &folder, empty.path(), "claude")).unwrap(),
        hours
    );

    check_folder_boundaries(binary);
}
