//! The real openusage process: help, exit codes and one-line errors (OU-CLI-005, OU-CLI-008).

use std::process::{Command, Stdio};

fn openusage(home: &std::path::Path, args: &[&str]) -> (i32, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_openusage"))
        .args(args)
        .env_clear()
        .env("HOME", home)
        .env("PATH", "/usr/bin:/bin")
        .env("TZ", "UTC")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    (
        output.status.code().unwrap(),
        String::from_utf8(output.stdout).unwrap(),
        String::from_utf8(output.stderr).unwrap(),
    )
}

#[test]
fn help_exit_codes_and_errors_are_stable() {
    let home = tempfile::tempdir().unwrap();
    let (code, out, _) = openusage(home.path(), &["--help"]);
    assert_eq!(code, 0);
    assert!(out.starts_with("OpenUsage shows what you used"));
    for command in ["report", "doctor", "app", "status", "statusline"] {
        let (code, out, _) = openusage(home.path(), &[command, "--help"]);
        assert_eq!(code, 0, "{command}");
        assert!(
            out.starts_with(&format!("Usage: openusage {command}")),
            "{command}"
        );
    }
    // No known log folder needs no consent and runs nothing.
    let (code, out, _) = openusage(home.path(), &["report"]);
    assert_eq!(code, 0);
    assert!(out.ends_with("No Claude Code or Codex logs on this device\n"));
    // With logs but no terminal, the report stops with one line and exit code 3.
    std::fs::create_dir_all(home.path().join(".claude/projects")).unwrap();
    let (code, out, err) = openusage(home.path(), &["report"]);
    assert_eq!((code, out.as_str()), (3, ""));
    assert_eq!(err.lines().count(), 1);
    assert!(err.ends_with("(consent_required)\n"));
    // JSON mode returns the same code as JSON.
    let (code, out, _) = openusage(home.path(), &["report", "--json"]);
    assert_eq!(code, 3);
    let error: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(error["schemaVersion"], 2);
    assert_eq!(error["error"]["code"], "consent_required");
    // Saved-data commands with no workspace give no_workspace, and statusline keeps one line.
    let (code, out, err) = openusage(home.path(), &[]);
    assert_eq!((code, out.as_str()), (8, ""));
    assert!(err.ends_with("(no_workspace)\n"));
    let (code, out, err) = openusage(home.path(), &["statusline"]);
    assert_eq!(
        (code, out.as_str(), err.as_str()),
        (8, "OpenUsage no data\n", "")
    );
    let (code, _, err) = openusage(home.path(), &["nope"]);
    assert_eq!(code, 2);
    assert!(err.ends_with("(usage)\n"));
}
