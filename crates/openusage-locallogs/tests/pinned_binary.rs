//! Runs the real pinned ccusage binary against synthetic logs in a private home.
//! Set OPENUSAGE_CCUSAGE_BIN to the provisioned binary; without it the test says so and passes.

use openusage_locallogs::{
    DEFAULT_MAX_OUTPUT, ScanRequest, parse_daily_by_agent, pinned_sha256, scan,
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

#[test]
fn pinned_binary_reads_synthetic_claude_and_codex_logs() {
    let Ok(binary) = std::env::var("OPENUSAGE_CCUSAGE_BIN") else {
        eprintln!("OPENUSAGE_CCUSAGE_BIN is not set; the pinned binary did not run");
        return;
    };
    let home = tempfile::tempdir().unwrap();
    copy_tree(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/home"),
        home.path(),
    );
    let pin = pinned_sha256("x86_64-unknown-linux-gnu").unwrap();
    let rows = scan(&ScanRequest {
        binary: Path::new(&binary),
        expected_sha256: pin,
        home: home.path(),
        timezone: "UTC",
        since: None,
        timeout: Duration::from_secs(60),
        max_output: DEFAULT_MAX_OUTPUT,
    })
    .unwrap();
    let expected = parse_daily_by_agent(include_bytes!(
        "fixtures/ccusage-20.0.48-daily-by-agent.json"
    ))
    .unwrap();
    assert_eq!(rows, expected);
    // The same logs give the same totals on a second scan (OU-LOG-012).
    let again = scan(&ScanRequest {
        binary: Path::new(&binary),
        expected_sha256: pin,
        home: home.path(),
        timezone: "UTC",
        since: None,
        timeout: Duration::from_secs(60),
        max_output: DEFAULT_MAX_OUTPUT,
    })
    .unwrap();
    assert_eq!(again, rows);
}
