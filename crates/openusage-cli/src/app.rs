//! `openusage app`: opens the desktop app. When it cannot start, it prints the fix (OU-DIST-004).

use crate::{CliError, Env, known_options};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub const HELP: &str = "Usage: openusage app

Opens the OpenUsage desktop app. When the app or its libraries are missing,
it prints the fix and exits with code 6.

Options:
  --help    Show this help.
";

pub fn command(args: &[String], env: &Env) -> Result<String, CliError> {
    if args.iter().any(|a| a == "--help" || a == "-h") {
        return Ok(HELP.to_string());
    }
    known_options(args, &[])?;
    if !env.desktop.is_file() {
        return Err(CliError::app(
            "The desktop app is missing beside this command. Reinstall OpenUsage with: npm install -g openusage",
        ));
    }
    if !crate::doctor::webkit_present() {
        return Err(CliError::app(
            "The desktop app needs WebKitGTK 4.1. Install it, for example: sudo apt install libwebkit2gtk-4.1-0",
        ));
    }
    let mut child = Command::new(&env.desktop)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| {
            CliError::app("The desktop app could not start. Run 'openusage doctor' for the fix.")
        })?;
    // An app that ends at once could not start, such as when a library is missing.
    let deadline = Instant::now() + Duration::from_millis(1500);
    while Instant::now() < deadline {
        match child.try_wait() {
            Ok(Some(status)) if !status.success() => {
                return Err(CliError::app(
                    "The desktop app stopped at start. Run 'openusage doctor' for the fix.",
                ));
            }
            Ok(Some(_)) => break,
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(_) => break,
        }
    }
    Ok(String::new())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn app_prints_a_fix_when_it_cannot_start() {
        let dir = tempfile::tempdir().unwrap();
        let desktop = dir.path().join("openusage-desktop");
        let env = |desktop: std::path::PathBuf| Env {
            home: None,
            ccusage: dir.path().join("ccusage"),
            ccusage_digest: None,
            desktop,
            interactive: false,
            today: None,
            time_zone: "UTC".into(),
        };
        assert_eq!(
            command(&[], &env(desktop.clone())).unwrap_err().code,
            "app_unavailable"
        );
        if !crate::doctor::webkit_present() {
            return;
        }
        std::fs::write(&desktop, "#!/bin/sh\nexit 127\n").unwrap();
        std::fs::set_permissions(&desktop, std::fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(command(&[], &env(desktop.clone())).unwrap_err().exit, 6);
        std::fs::write(&desktop, "#!/bin/sh\nexit 0\n").unwrap();
        assert_eq!(command(&[], &env(desktop)), Ok(String::new()));
        assert_eq!(
            command(&["--loud".into()], &env(dir.path().into()))
                .unwrap_err()
                .code,
            "usage"
        );
    }
}
