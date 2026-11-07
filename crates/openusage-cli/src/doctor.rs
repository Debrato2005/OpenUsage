//! `openusage doctor`: checks this machine and prints each fix. It reads no log and no database.

use crate::{CliError, Env, SCHEMA_VERSION, known_options};
use openusage_locallogs::{self as locallogs, CCUSAGE_VERSION};
use serde::Serialize;

pub const HELP: &str = "Usage: openusage doctor [--json]

Checks this machine and prints each fix. It reads no log and no database.
The exit code is 0 when every check passes and 1 when one needs a fix.

Options:
  --json    Print versioned JSON.
  --help    Show this help.
";

/// Library folders where a Linux package manager installs WebKitGTK.
const LIBRARY_DIRS: &[&str] = &[
    "/usr/lib/x86_64-linux-gnu",
    "/usr/lib64",
    "/usr/lib",
    "/lib/x86_64-linux-gnu",
];

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct Check {
    pub name: &'static str,
    pub ok: bool,
    pub detail: String,
    pub fix: Option<&'static str>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Doctor<'a> {
    schema_version: u32,
    command: &'static str,
    checks: &'a [Check],
}

pub fn webkit_present() -> bool {
    !cfg!(target_os = "linux")
        || LIBRARY_DIRS.iter().any(|dir| {
            std::path::Path::new(dir)
                .join("libwebkit2gtk-4.1.so.0")
                .exists()
        })
}

const REINSTALL: &str = "Reinstall OpenUsage with: npm install -g openusage";

pub fn checks(env: &Env) -> Vec<Check> {
    let digest = env.ccusage_digest.as_deref();
    let ccusage = std::fs::read(&env.ccusage)
        .ok()
        .map(|bytes| Some(locallogs::sha256_hex(&bytes).as_str()) == digest);
    let detected = env
        .home
        .as_deref()
        .map(locallogs::detect)
        .unwrap_or_default();
    let mut tools: Vec<&str> = detected
        .iter()
        .map(|d| {
            if d.tool == "claude" {
                "Claude Code"
            } else {
                "Codex"
            }
        })
        .collect();
    tools.dedup();
    vec![
        Check {
            name: "platform",
            ok: digest.is_some(),
            detail: format!("{} {}", std::env::consts::OS, std::env::consts::ARCH),
            fix: digest.is_none().then_some(
                "This release supports Linux x86_64. Windows and macOS builds are previews.",
            ),
        },
        Check {
            name: "ccusage",
            ok: ccusage == Some(true),
            detail: match ccusage {
                Some(true) => format!("ccusage {CCUSAGE_VERSION} matches its pinned digest"),
                Some(false) => "ccusage does not match its pinned digest, so it never runs".into(),
                None => "ccusage is missing beside this command".into(),
            },
            fix: (ccusage != Some(true)).then_some(REINSTALL),
        },
        Check {
            name: "desktop app",
            ok: env.desktop.is_file(),
            detail: if env.desktop.is_file() {
                "The desktop app is installed".into()
            } else {
                "The desktop app is missing beside this command".into()
            },
            fix: (!env.desktop.is_file()).then_some(REINSTALL),
        },
        Check {
            name: "WebKitGTK",
            ok: webkit_present(),
            detail: if webkit_present() {
                "The desktop runtime is installed".into()
            } else {
                "libwebkit2gtk-4.1.so.0 is missing".into()
            },
            fix: (!webkit_present()).then_some(
                "Install WebKitGTK 4.1, for example: sudo apt install libwebkit2gtk-4.1-0",
            ),
        },
        Check {
            name: "local logs",
            ok: true,
            detail: if tools.is_empty() {
                "No Claude Code or Codex logs on this device".into()
            } else {
                format!("Logs found for {}", tools.join(" and "))
            },
            fix: None,
        },
    ]
}

pub fn command(args: &[String], env: &Env) -> Result<String, CliError> {
    if args.iter().any(|a| a == "--help" || a == "-h") {
        return Ok(HELP.to_string());
    }
    known_options(args, &["--json"])?;
    let checks = checks(env);
    let output = if args.iter().any(|a| a == "--json") {
        format!(
            "{}\n",
            serde_json::to_string_pretty(&Doctor {
                schema_version: SCHEMA_VERSION,
                command: "doctor",
                checks: &checks
            })
            .unwrap_or_default()
        )
    } else {
        checks
            .iter()
            .map(|c| match c.fix {
                None => format!("ok   {}: {}\n", c.name, c.detail),
                Some(fix) => format!("fix  {}: {}. {}\n", c.name, c.detail, fix),
            })
            .collect()
    };
    if checks.iter().all(|c| c.ok) {
        Ok(output)
    } else {
        // The report still goes to the reader, with exit code 1.
        Err(CliError {
            code: "doctor_found_problems",
            exit: 1,
            message: output.trim_end().to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doctor_names_each_problem_and_its_fix() {
        let dir = tempfile::tempdir().unwrap();
        let ccusage = dir.path().join("ccusage");
        std::fs::write(&ccusage, "synthetic").unwrap();
        let env = Env {
            home: Some(dir.path().to_path_buf()),
            ccusage_digest: Some(locallogs::sha256_hex(b"other")),
            ccusage,
            desktop: dir.path().join("openusage-desktop"),
            interactive: false,
            today: None,
            time_zone: "UTC".into(),
            ..Env::for_tests()
        };
        let all = checks(&env);
        let by = |name: &str| all.iter().find(|c| c.name == name).unwrap();
        assert!(!by("ccusage").ok && by("ccusage").detail.contains("does not match"));
        assert!(!by("desktop app").ok && by("desktop app").fix.is_some());
        assert_eq!(
            by("local logs").detail,
            "No Claude Code or Codex logs on this device"
        );
        let error = command(&["--json".into()], &env).unwrap_err();
        assert_eq!((error.code, error.exit), ("doctor_found_problems", 1));
        assert!(error.message.contains("\"name\": \"ccusage\""));
        assert_eq!(command(&["--loud".into()], &env).unwrap_err().code, "usage");
    }
}
