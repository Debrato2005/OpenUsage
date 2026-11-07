//! `openusage report`: local log totals with no database. It asks once for consent, accepts
//! `--yes` for scripts and saves nothing (OU-CLI-002, OU-CLI-003).

use crate::{CliError, Env, SCHEMA_VERSION, dollars, grouped, known_options};
use jiff::{Span, civil::Date, tz::TimeZone};
use openusage_locallogs::{self as locallogs, CCUSAGE_VERSION, Detected, ScanError, ScanRequest};
use serde::Serialize;
use std::collections::BTreeMap;
use std::io::{BufRead, Write};

pub const HELP: &str = "Usage: openusage report [--days N] [--yes] [--json]

Reads token counts from the local logs of Claude Code and Codex on this device
and prints the totals. It saves nothing and needs no desktop app.

Options:
  --days N  Days to include, ending today, from 1 to 365. The default is 30.
  --yes     Give consent without a prompt, for scripts.
  --json    Print versioned JSON. A missing value is null, never zero.
  --help    Show this help.
";

fn name(tool: &str) -> &'static str {
    match tool {
        "claude" => "Claude Code",
        "codex" => "Codex",
        _ => "Unknown tool",
    }
}

#[derive(Debug, Default)]
struct Sum {
    input: u128,
    output: u128,
    cache_write: u128,
    cache_read: u128,
    total: u128,
    priced: u128,
    unpriced: usize,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct ToolReport {
    tool: &'static str,
    name: &'static str,
    state: &'static str,
    /// Local log totals have no reading status. See SC-019.
    status: Option<&'static str>,
    input_tokens: String,
    output_tokens: String,
    cache_write_tokens: String,
    cache_read_tokens: String,
    total_tokens: String,
    estimated_value_usd_micros: Option<String>,
    estimate_complete: bool,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct Report {
    schema_version: u32,
    command: &'static str,
    start_date: String,
    end_date: String,
    time_zone: String,
    observed_at_utc: String,
    source: String,
    source_level: &'static str,
    tools: Vec<ToolReport>,
}

impl Report {
    fn text(&self) -> String {
        let mut out = format!(
            "Local logs from {} to {}, days in {}\n",
            self.start_date, self.end_date, self.time_zone
        );
        if self.tools.is_empty() {
            out.push_str("No Claude Code or Codex logs on this device\n");
        }
        let width = self
            .tools
            .iter()
            .map(|t| grouped(t.total_tokens.parse().unwrap_or(0)).len())
            .max()
            .unwrap_or(0);
        for tool in &self.tools {
            let total = grouped(tool.total_tokens.parse().unwrap_or(0));
            let estimate = match &tool.estimated_value_usd_micros {
                None => "Estimated value not available".to_string(),
                Some(micros) => format!(
                    "Estimated {}{}",
                    dollars(micros.parse().unwrap_or(0)),
                    if tool.estimate_complete {
                        ""
                    } else {
                        ", some models have no price"
                    }
                ),
            };
            out.push_str(&format!(
                "{:<12} {:>width$} tokens  {}\n",
                tool.name, total, estimate
            ));
        }
        out
    }
}

struct Options {
    days: u16,
    yes: bool,
    json: bool,
}

fn options(args: &[String]) -> Result<Option<Options>, CliError> {
    if args.iter().any(|a| a == "--help" || a == "-h") {
        return Ok(None);
    }
    let mut days = 30;
    let mut rest = Vec::new();
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if arg == "--days" {
            days = iter
                .next()
                .and_then(|v| v.parse::<u16>().ok())
                .filter(|v| (1..=365).contains(v))
                .ok_or_else(|| CliError::usage("--days needs a whole number from 1 to 365."))?;
        } else {
            rest.push(arg.clone());
        }
    }
    known_options(&rest, &["--yes", "--json"])?;
    Ok(Some(Options {
        days,
        yes: rest.iter().any(|a| a == "--yes"),
        json: rest.iter().any(|a| a == "--json"),
    }))
}

fn ask(
    detected: &[Detected],
    env: &Env,
    input: &mut dyn BufRead,
    prompt: &mut dyn Write,
) -> Result<(), CliError> {
    let mut tools: Vec<&str> = detected.iter().map(|d| name(d.tool)).collect();
    tools.dedup();
    if !env.interactive {
        return Err(CliError::consent(
            "Reading local logs needs consent. Run again in a terminal, or add --yes.",
        ));
    }
    let _ = write!(
        prompt,
        "OpenUsage will read token counts from the local logs of {}. It saves nothing. Continue? [y/N] ",
        tools.join(" and ")
    );
    let _ = prompt.flush();
    let mut answer = String::new();
    input
        .read_line(&mut answer)
        .map_err(|_| CliError::consent("No answer was read."))?;
    if matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes") {
        Ok(())
    } else {
        Err(CliError::consent(
            "You did not give consent, so no log was read.",
        ))
    }
}

pub fn command(
    args: &[String],
    env: &Env,
    input: &mut dyn BufRead,
    prompt: &mut dyn Write,
) -> Result<String, CliError> {
    let Some(options) = options(args)? else {
        return Ok(HELP.to_string());
    };
    let zone = TimeZone::get(&env.time_zone)
        .map_err(|_| CliError::usage(format!("The time zone '{}' is not known.", env.time_zone)))?;
    let today = env
        .today
        .unwrap_or_else(|| jiff::Timestamp::now().to_zoned(zone).date());
    let start = today
        .checked_sub(Span::new().days(i64::from(options.days) - 1))
        .map_err(|_| CliError::usage("--days goes before the calendar start."))?;
    let detected = env
        .home
        .as_deref()
        .map(locallogs::detect)
        .unwrap_or_default();
    if !detected.is_empty() && !options.yes {
        ask(&detected, env, input, prompt)?;
    }
    let digest = env
        .ccusage_digest
        .as_deref()
        .ok_or_else(|| CliError::ccusage("No pinned ccusage exists for this platform yet."))?;
    let mut sums: BTreeMap<&'static str, Sum> = BTreeMap::new();
    for source in &detected {
        let empty_home = tempfile::tempdir().map_err(|_| {
            CliError::scan("OpenUsage could not create a private folder for ccusage.")
        })?;
        let since = start.to_string();
        let rows = locallogs::scan(&ScanRequest {
            binary: &env.ccusage,
            expected_sha256: digest,
            tool: source.tool,
            folder: std::path::Path::new(&source.folder),
            empty_home: empty_home.path(),
            timezone: &env.time_zone,
            since: Some(&since),
            timeout: locallogs::DEFAULT_TIMEOUT,
            max_output: locallogs::DEFAULT_MAX_OUTPUT,
        })
        .map_err(|error| match error {
            ScanError::Unavailable | ScanError::Digest => CliError::ccusage(format!(
                "The pinned ccusage {CCUSAGE_VERSION} is missing or changed. Reinstall OpenUsage."
            )),
            _ => CliError::scan(format!(
                "The {} log scan failed. Nothing was saved.",
                name(source.tool)
            )),
        })?;
        for row in rows {
            let Ok(date) = row.date.parse::<Date>() else {
                continue;
            };
            if date < start || date > today {
                continue;
            }
            let sum = sums.entry(row.tool).or_default();
            sum.input += u128::from(row.input_tokens);
            sum.output += u128::from(row.output_tokens);
            sum.cache_write += u128::from(row.cache_write_tokens);
            sum.cache_read += u128::from(row.cache_read_tokens);
            sum.total += u128::from(row.total_tokens);
            match row.estimated_value_micros {
                Some(value) => sum.priced += u128::from(value),
                None => sum.unpriced += 1,
            }
        }
    }
    let order = |tool: &str| {
        locallogs::FOLDER_VARIABLES
            .iter()
            .position(|(t, _)| *t == tool)
    };
    let mut tools: Vec<_> = sums.into_iter().collect();
    tools.sort_by_key(|(tool, _)| order(tool));
    let report = Report {
        schema_version: SCHEMA_VERSION,
        command: "report",
        start_date: start.to_string(),
        end_date: today.to_string(),
        time_zone: env.time_zone.clone(),
        observed_at_utc: jiff::Timestamp::now()
            .strftime("%Y-%m-%dT%H:%M:%S%.3fZ")
            .to_string(),
        source: format!("ccusage {CCUSAGE_VERSION}"),
        source_level: "local_log",
        tools: tools
            .into_iter()
            .map(|(tool, s)| ToolReport {
                tool,
                name: name(tool),
                state: "current",
                status: None,
                input_tokens: s.input.to_string(),
                output_tokens: s.output.to_string(),
                cache_write_tokens: s.cache_write.to_string(),
                cache_read_tokens: s.cache_read.to_string(),
                total_tokens: s.total.to_string(),
                // With no priced row there is no estimate at all, not an estimate of zero.
                estimated_value_usd_micros: (s.unpriced == 0 || s.priced > 0)
                    .then(|| s.priced.to_string()),
                estimate_complete: s.unpriced == 0,
            })
            .collect(),
    };
    Ok(if options.json {
        format!(
            "{}\n",
            serde_json::to_string_pretty(&report)
                .map_err(|_| CliError::scan("The report could not be written."))?
        )
    } else {
        report.text()
    })
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    const CLAUDE_REPORT: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../openusage-locallogs/tests/fixtures/ccusage-20.0.48-daily-by-agent-claude.json"
    );

    fn make_env(dir: &std::path::Path, body: &str) -> Env {
        let binary = dir.join("ccusage");
        std::fs::write(&binary, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o700)).unwrap();
        let home = dir.join("home");
        std::fs::create_dir_all(home.join(".claude/projects")).unwrap();
        Env {
            home: Some(home),
            ccusage_digest: Some(locallogs::sha256_hex(&std::fs::read(&binary).unwrap())),
            ccusage: binary,
            desktop: dir.join("missing-desktop"),
            interactive: false,
            today: Some("2026-11-08".parse().unwrap()),
            time_zone: "UTC".into(),
        }
    }
    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|v| v.to_string()).collect()
    }

    // One test, because ccusage runs share one process-wide lock.
    #[test]
    fn report_asks_once_saves_nothing_and_prints_exact_totals() {
        let dir = tempfile::tempdir().unwrap();
        let mut env = make_env(dir.path(), &format!("cat {CLAUDE_REPORT}"));
        // A script without --yes and without a terminal gets an error, never a hang.
        let error = command(
            &args(&["--json"]),
            &env,
            &mut std::io::empty(),
            &mut std::io::sink(),
        )
        .unwrap_err();
        assert_eq!((error.code, error.exit), ("consent_required", 3));
        // A person who answers no reads nothing.
        env.interactive = true;
        let mut prompt = Vec::new();
        let error = command(&[], &env, &mut "n\n".as_bytes(), &mut prompt).unwrap_err();
        assert_eq!(error.code, "consent_required");
        assert!(
            String::from_utf8(prompt)
                .unwrap()
                .contains("local logs of Claude Code. It saves nothing. Continue? [y/N]")
        );
        let text = command(&[], &env, &mut "y\n".as_bytes(), &mut std::io::sink()).unwrap();
        assert_eq!(
            text,
            "Local logs from 2026-10-10 to 2026-11-08, days in UTC\nClaude Code  14,950 tokens  Estimated $0.11, some models have no price\n"
        );
        let json: serde_json::Value = serde_json::from_str(
            &command(
                &args(&["--yes", "--json"]),
                &env,
                &mut std::io::empty(),
                &mut std::io::sink(),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(json["schemaVersion"], 1);
        assert_eq!(json["sourceLevel"], "local_log");
        assert_eq!(json["tools"][0]["totalTokens"], "14950");
        assert_eq!(json["tools"][0]["estimatedValueUsdMicros"], "108525");
        assert_eq!(json["tools"][0]["estimateComplete"], false);
        assert_eq!(json["tools"][0]["status"], serde_json::Value::Null);
        // A shorter range leaves out older days.
        let short = command(
            &args(&["--yes", "--days", "3", "--json"]),
            &env,
            &mut std::io::empty(),
            &mut std::io::sink(),
        )
        .unwrap();
        assert!(short.contains("\"startDate\": \"2026-11-06\""));
        assert!(!short.contains("\"tool\""));
        // Nothing to read needs no consent.
        let empty = Env {
            home: Some(dir.path().join("nobody")),
            ..make_env(dir.path(), "exit 9")
        };
        assert!(
            command(&[], &empty, &mut std::io::empty(), &mut std::io::sink())
                .unwrap()
                .ends_with("No Claude Code or Codex logs on this device\n")
        );
        // A changed binary never runs, and a failed run reports one line.
        let mut changed = make_env(dir.path(), &format!("touch {}/ran", dir.path().display()));
        changed.ccusage_digest = Some("0".repeat(64));
        assert_eq!(
            command(
                &args(&["--yes"]),
                &changed,
                &mut std::io::empty(),
                &mut std::io::sink()
            )
            .unwrap_err()
            .code,
            "ccusage_unavailable"
        );
        assert!(!dir.path().join("ran").exists());
        assert_eq!(
            command(
                &args(&["--yes"]),
                &make_env(dir.path(), "exit 3"),
                &mut std::io::empty(),
                &mut std::io::sink()
            )
            .unwrap_err()
            .code,
            "scan_failed"
        );
        for bad in [&["--days", "0"][..], &["--days"], &["--loud"]] {
            assert_eq!(
                command(
                    &args(bad),
                    &env,
                    &mut std::io::empty(),
                    &mut std::io::sink()
                )
                .unwrap_err()
                .code,
                "usage"
            );
        }
        assert!(
            command(
                &args(&["--help"]),
                &env,
                &mut std::io::empty(),
                &mut std::io::sink()
            )
            .unwrap()
            .contains("--days N")
        );
    }
}
