//! Runs the real pinned ccusage binary against synthetic logs, one consented folder at a time.
//! Set OPENUSAGE_CCUSAGE_BIN to the provisioned binary; without it the test says so and passes.

use openusage_locallogs::{
    DEFAULT_MAX_OUTPUT, ScanRequest, UsageRow, parse_daily_by_agent, pinned_sha256, scan,
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

fn scan_tool(binary: &Path, home: &Path, tool: &str, folder: &str) -> Vec<UsageRow> {
    let empty = tempfile::tempdir().unwrap();
    scan(&ScanRequest {
        binary,
        expected_sha256: pinned_sha256("x86_64-unknown-linux-gnu").unwrap(),
        tool,
        folder: &home.join(folder),
        empty_home: empty.path(),
        timezone: "UTC",
        since: None,
        timeout: Duration::from_secs(60),
        max_output: DEFAULT_MAX_OUTPUT,
    })
    .unwrap()
}

#[test]
fn pinned_binary_reads_synthetic_claude_and_codex_logs() {
    let Ok(binary) = std::env::var("OPENUSAGE_CCUSAGE_BIN") else {
        eprintln!("OPENUSAGE_CCUSAGE_BIN is not set; the pinned binary did not run");
        return;
    };
    let binary = Path::new(&binary);
    let home = tempfile::tempdir().unwrap();
    copy_tree(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/home"),
        home.path(),
    );
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
}
