//! The openusage command (SRS Section 11A). People, scripts and agents get the same data (OU-DES-012).
//! Each failure has one line of text, a stable code and a documented exit code (OU-CLI-005).

mod app;
mod doctor;
mod report;

use serde::Serialize;
use std::io::{IsTerminal, Write};
use std::path::PathBuf;
use std::process::ExitCode;

pub const SCHEMA_VERSION: u32 = 1;

/// A failure with a stable code. JSON mode returns the same code as JSON.
#[derive(Debug, PartialEq, Eq)]
pub struct CliError {
    pub code: &'static str,
    pub exit: u8,
    pub message: String,
}
impl CliError {
    pub fn usage(message: impl Into<String>) -> Self {
        Self {
            code: "usage",
            exit: 2,
            message: message.into(),
        }
    }
    pub fn consent(message: impl Into<String>) -> Self {
        Self {
            code: "consent_required",
            exit: 3,
            message: message.into(),
        }
    }
    pub fn ccusage(message: impl Into<String>) -> Self {
        Self {
            code: "ccusage_unavailable",
            exit: 4,
            message: message.into(),
        }
    }
    pub fn scan(message: impl Into<String>) -> Self {
        Self {
            code: "scan_failed",
            exit: 5,
            message: message.into(),
        }
    }
    pub fn app(message: impl Into<String>) -> Self {
        Self {
            code: "app_unavailable",
            exit: 6,
            message: message.into(),
        }
    }
}

/// What a command needs from the machine. Tests replace each part.
pub struct Env {
    /// The home whose known log folders `report` detects.
    pub home: Option<PathBuf>,
    pub ccusage: PathBuf,
    pub ccusage_digest: Option<String>,
    pub desktop: PathBuf,
    /// True when a person can answer the consent prompt.
    pub interactive: bool,
    /// The date that ends the report. None uses today in the report zone.
    pub today: Option<jiff::civil::Date>,
    pub time_zone: String,
}
impl Env {
    fn from_system() -> Self {
        let dir = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(PathBuf::from))
            .unwrap_or_default();
        let mut ccusage = dir.join(openusage_locallogs::BINARY_NAME);
        let mut desktop = dir.join(if cfg!(windows) {
            "openusage-desktop.exe"
        } else {
            "openusage-desktop"
        });
        // A development build can name its binaries. The pinned digest still applies to ccusage.
        if cfg!(debug_assertions) {
            if let Some(path) = std::env::var_os("OPENUSAGE_CCUSAGE") {
                ccusage = path.into();
            }
            if let Some(path) = std::env::var_os("OPENUSAGE_DESKTOP") {
                desktop = path.into();
            }
        }
        let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
            .map(PathBuf::from)
            .filter(|home| home.is_absolute());
        let time_zone = jiff::tz::TimeZone::system()
            .iana_name()
            .map(str::to_string)
            .unwrap_or_else(|| "UTC".into());
        Self {
            home,
            ccusage,
            ccusage_digest: openusage_locallogs::pinned_sha256_here().map(str::to_string),
            desktop,
            interactive: std::io::stdin().is_terminal() && std::io::stderr().is_terminal(),
            today: None,
            time_zone,
        }
    }
}

const HELP: &str =
    "OpenUsage shows what you used and what you have left across your AI coding tools.

Usage: openusage <command> [options]

Commands:
  report    Local log totals for this device, with no database
  doctor    Check this machine and print each fix
  app       Open the desktop app

Run 'openusage <command> --help' for the options of one command.

Exit codes:
  0  Success
  1  doctor found a problem
  2  usage: an unknown command or option
  3  consent_required: report needs your consent, or --yes
  4  ccusage_unavailable: the pinned ccusage is missing or changed
  5  scan_failed: a ccusage run failed
  6  app_unavailable: the desktop app cannot start here
";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ErrorBody<'a> {
    schema_version: u32,
    error: ErrorFields<'a>,
}
#[derive(Serialize)]
struct ErrorFields<'a> {
    code: &'a str,
    message: &'a str,
}

/// Runs one command and returns its text or JSON output.
pub fn run(
    args: &[String],
    env: &Env,
    input: &mut dyn std::io::BufRead,
    prompt: &mut dyn Write,
) -> (Result<String, CliError>, bool) {
    let json = args.iter().any(|a| a == "--json");
    let result = match args.first().map(String::as_str) {
        None | Some("--help" | "-h" | "help") => Ok(HELP.to_string()),
        Some("report") => report::command(&args[1..], env, input, prompt),
        Some("doctor") => doctor::command(&args[1..], env),
        Some("app") => app::command(&args[1..], env),
        Some(other) => Err(CliError::usage(format!(
            "Unknown command '{other}'. Run 'openusage --help'."
        ))),
    };
    (result, json)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let env = Env::from_system();
    let stdin = std::io::stdin();
    let (result, json) = run(&args, &env, &mut stdin.lock(), &mut std::io::stderr());
    match result {
        Ok(output) => {
            print!("{output}");
            ExitCode::SUCCESS
        }
        Err(error) if json => {
            let body = ErrorBody {
                schema_version: SCHEMA_VERSION,
                error: ErrorFields {
                    code: error.code,
                    message: &error.message,
                },
            };
            println!("{}", serde_json::to_string(&body).unwrap_or_default());
            ExitCode::from(error.exit)
        }
        Err(error) => {
            let _ = writeln!(
                std::io::stderr(),
                "openusage: {} ({})",
                error.message,
                error.code
            );
            ExitCode::from(error.exit)
        }
    }
}

/// Groups digits in threes, such as 14,950.
pub fn grouped(value: u128) -> String {
    let digits = value.to_string();
    let mut out = String::new();
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

/// Rounds millionths of a dollar to cents for text. JSON keeps every digit.
pub fn dollars(micros: u128) -> String {
    let cents = (micros + 5_000) / 10_000;
    format!("${}.{:02}", grouped(cents / 100), cents % 100)
}

/// Returns an error for any option that a command does not know.
pub fn known_options(args: &[String], known: &[&str]) -> Result<(), CliError> {
    match args.iter().find(|a| !known.contains(&a.as_str())) {
        Some(other) => Err(CliError::usage(format!("Unknown option '{other}'."))),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_and_unknown_input() {
        assert_eq!(grouped(0), "0");
        assert_eq!(grouped(14_950), "14,950");
        assert_eq!(grouped(1_234_567_890), "1,234,567,890");
        assert_eq!(dollars(108_525), "$0.11");
        assert_eq!(dollars(1_234_565_000), "$1,234.57");
        let env = Env::from_system();
        let (result, json) = run(
            &["nope".into(), "--json".into()],
            &env,
            &mut std::io::empty(),
            &mut std::io::sink(),
        );
        assert!(json);
        assert_eq!(result.unwrap_err().code, "usage");
        assert!(
            run(&[], &env, &mut std::io::empty(), &mut std::io::sink())
                .0
                .unwrap()
                .contains("Exit codes:")
        );
    }
}
