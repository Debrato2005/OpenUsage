//! `openusage`, `openusage status --json` and `openusage statusline` read saved data only.
//! They open the database read-only, never prompt, and never use the network (OU-CLI-002, OU-CLI-006).

use crate::{CliError, Env, SCHEMA_VERSION, grouped, known_options};
use openusage_keys::VaultError;
use openusage_storage::{ProfileConnectionState, QuotaWindow, Storage, StorageError};
use serde::Serialize;
use std::collections::BTreeMap;

pub const STATUS_HELP: &str = "Usage: openusage status [--json]

Runs when openusage has no command. Shows the saved quotas of each account and today's local log usage. It reads
saved data only: it never writes the database, starts a reading, or uses the
network. A missing value shows Not available, and in JSON it is null.

Options:
  --json    Print versioned JSON.
  --help    Show this help.
";

pub const STATUSLINE_HELP: &str = "Usage: openusage statusline

Prints one line for tmux and shell prompts. It never prompts. When the
workspace is locked, it prints 'OpenUsage locked' and exits with code 7.

Options:
  --help    Show this help.
";

/// Quota readings stay Current for 10 minutes (OU-STATE-005).
const QUOTA_FRESH_SECONDS: i64 = 600;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Window {
    limit_id: String,
    limit_name: Option<String>,
    role: &'static str,
    duration_mins: Option<u64>,
    used_percent: u8,
    /// Derived: 100 minus the used percentage (OU-NUM-002).
    remaining_percent: u8,
    resets_at_utc: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Quota {
    state: &'static str,
    status: Option<&'static str>,
    source: Option<String>,
    observed_at_utc: Option<String>,
    windows: Vec<Window>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Profile {
    id: String,
    label: String,
    connection_state: &'static str,
    quota: Quota,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LocalTool {
    tool: String,
    name: &'static str,
    total_tokens: String,
    estimated_value_usd_micros: Option<String>,
    estimate_complete: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LocalToday {
    date: String,
    time_zone: String,
    source_level: &'static str,
    /// Local log totals have no reading status (SC-019).
    status: Option<&'static str>,
    tools: Vec<LocalTool>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Status {
    schema_version: u32,
    command: &'static str,
    observed_at_utc: String,
    profiles: Vec<Profile>,
    local: LocalToday,
}

fn locked(message: &str) -> CliError {
    CliError {
        code: "locked",
        exit: 7,
        message: message.into(),
    }
}

fn tool_name(tool: &str) -> &'static str {
    match tool {
        "claude" => "Claude Code",
        "codex" => "Codex",
        _ => "Unknown tool",
    }
}

fn connection(state: ProfileConnectionState) -> &'static str {
    match state {
        ProfileConnectionState::Connected => "connected",
        ProfileConnectionState::AuthenticationRequired => "authentication_required",
        ProfileConnectionState::Unauthorized => "unauthorized",
        ProfileConnectionState::Offline => "offline",
        ProfileConnectionState::Unsupported => "unsupported",
        ProfileConnectionState::Error => "error",
    }
}

/// Opens the saved workspace read-only. A password workspace is Locked: the CLI never prompts.
fn open(env: &Env) -> Result<Storage, CliError> {
    let directory = env.workspace.as_deref().ok_or_else(|| CliError {
        code: "no_workspace",
        exit: 8,
        message: "No OpenUsage workspace on this device. Open the desktop app first.".into(),
    })?;
    let database = directory.join("openusage.db");
    if !database.is_file() {
        return Err(CliError {
            code: "no_workspace",
            exit: 8,
            message: "No OpenUsage workspace on this device. Open the desktop app first.".into(),
        });
    }
    if directory.join(".local-deletion.v1").exists() {
        return Err(locked(
            "Local data deletion did not finish. Open the desktop app to finish it.",
        ));
    }
    if directory.join("database-key.v1").exists() {
        return Err(locked(
            "This workspace uses an app password, so the CLI cannot open it without a prompt.",
        ));
    }
    let key = match (env.key)() {
        Ok(Some(key)) => key,
        Ok(None) | Err(VaultError::InvalidKey) => {
            return Err(locked("The workspace key is not in the system key store."));
        }
        Err(VaultError::Unavailable) => {
            return Err(locked("The system key store is locked or unavailable."));
        }
    };
    Storage::open_read_only(&database, &key).map_err(|error| match error {
        StorageError::UnsupportedSchema => CliError {
            code: "unsupported_schema",
            exit: 9,
            message: "Open the desktop app once to update the saved data.".into(),
        },
        _ => locked("OpenUsage could not open the saved data."),
    })
}

fn utc(unix: i64) -> Option<String> {
    jiff::Timestamp::from_second(unix)
        .ok()
        .map(|t| t.strftime("%Y-%m-%dT%H:%M:%SZ").to_string())
}

fn snapshot(storage: &Storage, now: jiff::Timestamp) -> Result<Status, CliError> {
    let unreadable = |_| CliError {
        code: "read_failed",
        exit: 10,
        message: "OpenUsage could not read the saved data.".into(),
    };
    let mut profiles = Vec::new();
    for profile in storage
        .list_profiles()
        .map_err(|_| unreadable(()))?
        .into_iter()
        .filter(|p| !p.archived)
    {
        let saved = storage
            .current_quota(&profile.id)
            .map_err(|_| unreadable(()))?;
        let quota = match saved {
            None => Quota {
                state: "not_available",
                status: None,
                source: None,
                observed_at_utc: None,
                windows: Vec::new(),
            },
            Some(saved) => {
                let fresh = saved
                    .commit
                    .observed_at_utc
                    .parse::<jiff::Timestamp>()
                    .is_ok_and(|observed| {
                        now.as_second() - observed.as_second() <= QUOTA_FRESH_SECONDS
                    });
                let mut windows = Vec::new();
                for limit in &saved.limits {
                    for (role, window) in
                        [("primary", &limit.primary), ("secondary", &limit.secondary)]
                    {
                        let Some(&QuotaWindow {
                            used_percent,
                            window_duration_mins,
                            resets_at_unix,
                        }) = window.as_ref()
                        else {
                            continue;
                        };
                        windows.push(Window {
                            limit_id: limit.limit_id.clone(),
                            limit_name: limit.limit_name.clone(),
                            role,
                            duration_mins: window_duration_mins,
                            used_percent,
                            remaining_percent: 100 - used_percent.min(100),
                            resets_at_utc: resets_at_unix.and_then(utc),
                        });
                    }
                }
                Quota {
                    state: if fresh { "current" } else { "stale" },
                    status: Some("official"),
                    source: Some(format!("codex app-server {}", saved.provider_version)),
                    observed_at_utc: Some(saved.commit.observed_at_utc.clone()),
                    windows,
                }
            }
        };
        profiles.push(Profile {
            id: profile.id,
            label: profile.label,
            connection_state: connection(profile.connection_state),
            quota,
        });
    }
    let zone = storage.local_usage_zone().map_err(|_| unreadable(()))?;
    let today = jiff::tz::TimeZone::get(&zone)
        .map(|tz| now.to_zoned(tz).date().to_string())
        .map_err(|_| unreadable(()))?;
    let mut tools: BTreeMap<String, (u128, u128, usize)> = BTreeMap::new();
    for day in storage
        .local_usage_days(&today, &today)
        .map_err(|_| unreadable(()))?
    {
        let entry = tools.entry(day.counts.tool.clone()).or_default();
        entry.0 += u128::from(day.counts.total_tokens);
        match day.counts.estimated_value_usd_micros {
            Some(value) => entry.1 += u128::from(value),
            None => entry.2 += 1,
        }
    }
    let order = |tool: &str| {
        openusage_locallogs::FOLDER_VARIABLES
            .iter()
            .position(|(t, _)| *t == tool)
    };
    let mut tools: Vec<_> = tools.into_iter().collect();
    tools.sort_by_key(|(tool, _)| order(tool));
    Ok(Status {
        schema_version: SCHEMA_VERSION,
        command: "status",
        observed_at_utc: now.strftime("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
        profiles,
        local: LocalToday {
            date: today,
            time_zone: zone,
            source_level: "local_log",
            status: None,
            tools: tools
                .into_iter()
                .map(|(tool, (total, priced, unpriced))| LocalTool {
                    name: tool_name(&tool),
                    tool,
                    total_tokens: total.to_string(),
                    estimated_value_usd_micros: (unpriced == 0 || priced > 0)
                        .then(|| priced.to_string()),
                    estimate_complete: unpriced == 0,
                })
                .collect(),
        },
    })
}

/// Five-hour and Seven-day are the names that OU-QUOTA-005 and OU-QUOTA-006 give.
fn window_name(duration: Option<u64>) -> String {
    match duration {
        Some(300) => "Five-hour".into(),
        Some(10_080) => "Seven-day".into(),
        Some(minutes) if minutes % 1440 == 0 => format!("{}-day", minutes / 1440),
        Some(minutes) if minutes % 60 == 0 => format!("{}-hour", minutes / 60),
        Some(minutes) => format!("{minutes}-minute"),
        None => "Window".into(),
    }
}

fn short_name(duration: Option<u64>) -> String {
    match duration {
        Some(minutes) if minutes % 1440 == 0 => format!("{}d", minutes / 1440),
        Some(minutes) if minutes % 60 == 0 => format!("{}h", minutes / 60),
        Some(minutes) => format!("{minutes}m"),
        None => "?".into(),
    }
}

/// A reset time is not proof of a reset, so a past time says only that it passed.
fn reset_text(resets: Option<&str>, now: jiff::Timestamp) -> String {
    let Some(at) = resets.and_then(|r| r.parse::<jiff::Timestamp>().ok()) else {
        return "reset time Not available".into();
    };
    let seconds = at.as_second() - now.as_second();
    if seconds <= 0 {
        return "the expected reset time passed".into();
    }
    let (days, hours, minutes) = (
        seconds / 86_400,
        seconds % 86_400 / 3_600,
        seconds % 3_600 / 60,
    );
    if days > 0 {
        format!("resets in {days}d {hours}h")
    } else {
        format!("resets in {hours}h {minutes}m")
    }
}

/// A short count for one prompt line, cut to tenths with integers. JSON keeps every digit.
fn compact(value: u128) -> String {
    for (unit, suffix) in [(1_000_000_000, "B"), (1_000_000, "M"), (1_000, "K")] {
        if value >= unit {
            let tenths = value * 10 / unit;
            return format!("{}.{}{suffix}", tenths / 10, tenths % 10);
        }
    }
    value.to_string()
}

fn text(status: &Status, now: jiff::Timestamp) -> String {
    let mut out = String::new();
    if status.profiles.is_empty() {
        out.push_str("No accounts\n");
    }
    for profile in &status.profiles {
        out.push_str(&format!("{}\n", profile.label));
        if profile.quota.windows.is_empty() {
            out.push_str("  Quota Not available\n");
        }
        let stale = if profile.quota.state == "stale" {
            "  Stale"
        } else {
            ""
        };
        for window in &profile.quota.windows {
            out.push_str(&format!(
                "  {:<10} {}% left, {}{}\n",
                window_name(window.duration_mins),
                window.remaining_percent,
                reset_text(window.resets_at_utc.as_deref(), now),
                stale
            ));
        }
    }
    out.push_str(&format!("This device today, {}\n", status.local.date));
    if status.local.tools.is_empty() {
        out.push_str("  No saved local usage today\n");
    }
    for tool in &status.local.tools {
        out.push_str(&format!(
            "  {:<11} {} tokens\n",
            tool.name,
            grouped(tool.total_tokens.parse().unwrap_or(0))
        ));
    }
    out
}

fn line(status: &Status) -> String {
    let mut parts: Vec<String> = status
        .profiles
        .iter()
        .map(|profile| {
            if profile.quota.windows.is_empty() {
                return format!("{} Not available", profile.label);
            }
            let windows: Vec<String> = profile
                .quota
                .windows
                .iter()
                .map(|w| format!("{} {}%", short_name(w.duration_mins), w.remaining_percent))
                .collect();
            let stale = if profile.quota.state == "stale" {
                " Stale"
            } else {
                ""
            };
            format!("{} {}{}", profile.label, windows.join(" "), stale)
        })
        .collect();
    parts.extend(status.local.tools.iter().map(|t| {
        format!(
            "{} {} today",
            t.name,
            compact(t.total_tokens.parse().unwrap_or(0))
        )
    }));
    if parts.is_empty() {
        return "OpenUsage no data\n".into();
    }
    format!("{}\n", parts.join(" | "))
}

pub fn status(args: &[String], env: &Env) -> Result<String, CliError> {
    if args.iter().any(|a| a == "--help" || a == "-h") {
        return Ok(STATUS_HELP.to_string());
    }
    known_options(args, &["--json"])?;
    let now = env.now.unwrap_or_else(jiff::Timestamp::now);
    let status = snapshot(&open(env)?, now)?;
    Ok(if args.iter().any(|a| a == "--json") {
        format!(
            "{}\n",
            serde_json::to_string_pretty(&status).unwrap_or_default()
        )
    } else {
        text(&status, now)
    })
}

pub fn statusline(args: &[String], env: &Env) -> Result<String, CliError> {
    if args.iter().any(|a| a == "--help" || a == "-h") {
        return Ok(STATUSLINE_HELP.to_string());
    }
    known_options(args, &[])?;
    let now = env.now.unwrap_or_else(jiff::Timestamp::now);
    match open(env) {
        Ok(storage) => Ok(line(&snapshot(&storage, now)?)),
        // A prompt line stays one short line for each state.
        Err(error) if error.code == "locked" => Err(CliError {
            message: "OpenUsage locked".into(),
            ..error
        }),
        Err(error) if error.code == "no_workspace" => Err(CliError {
            message: "OpenUsage no data".into(),
            ..error
        }),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openusage_storage::LogConsent;

    const KEY: [u8; 32] = [71; 32];

    fn env(workspace: &std::path::Path, key: Result<Option<[u8; 32]>, VaultError>) -> Env {
        Env {
            workspace: Some(workspace.to_path_buf()),
            key: Box::new(move || key.map(|k| k.map(zeroize::Zeroizing::new))),
            now: Some("2026-11-08T10:00:00Z".parse().unwrap()),
            ..Env::for_tests()
        }
    }

    fn private(dir: &std::path::Path) -> std::path::PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let workspace = dir.join("workspace");
        std::fs::create_dir(&workspace).unwrap();
        std::fs::set_permissions(&workspace, std::fs::Permissions::from_mode(0o700)).unwrap();
        workspace
    }

    #[test]
    fn saved_data_prints_exact_values_and_never_prompts() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = private(dir.path());
        // No workspace and a locked key store give their own codes and exit codes.
        assert_eq!(
            status(&[], &env(&workspace, Ok(Some(KEY))))
                .unwrap_err()
                .exit,
            8
        );
        let mut storage = Storage::create(&workspace.join("openusage.db"), &KEY).unwrap();
        let source = storage
            .set_log_source(
                "claude",
                "/home/synthetic/.claude",
                LogConsent::Granted,
                "2026-11-08T08:00:00.000Z",
            )
            .unwrap();
        storage
            .save_local_usage_scan(&openusage_storage::LocalUsageScan {
                log_source_ids: &[source.id],
                scanner_version: "20.0.48",
                time_zone: "UTC",
                since: None,
                scanned_at_utc: "2026-11-08T09:00:00.000Z",
                rows: &[openusage_storage::LocalUsageRow {
                    local_date: "2026-11-08".into(),
                    tool: "claude".into(),
                    model: "claude-sonnet-4".into(),
                    input_tokens: 1_200_000,
                    output_tokens: 34_567,
                    cache_write_tokens: 0,
                    cache_read_tokens: 0,
                    total_tokens: 1_234_567,
                    estimated_value_usd_micros: None,
                }],
            })
            .unwrap();
        drop(storage);
        let locked_store = status(&[], &env(&workspace, Err(VaultError::Unavailable))).unwrap_err();
        assert_eq!((locked_store.code, locked_store.exit), ("locked", 7));
        assert_eq!(
            statusline(&[], &env(&workspace, Ok(None)))
                .unwrap_err()
                .message,
            "OpenUsage locked"
        );
        // With the key, the saved data prints, and no account means No accounts, not zero.
        let text = status(&[], &env(&workspace, Ok(Some(KEY)))).unwrap();
        assert_eq!(
            text,
            "No accounts\nThis device today, 2026-11-08\n  Claude Code 1,234,567 tokens\n"
        );
        let json: serde_json::Value = serde_json::from_str(
            &status(&["--json".into()], &env(&workspace, Ok(Some(KEY)))).unwrap(),
        )
        .unwrap();
        assert_eq!(json["schemaVersion"], 1);
        assert_eq!(json["local"]["tools"][0]["totalTokens"], "1234567");
        assert_eq!(
            json["local"]["tools"][0]["estimatedValueUsdMicros"],
            serde_json::Value::Null
        );
        assert_eq!(
            statusline(&[], &env(&workspace, Ok(Some(KEY)))).unwrap(),
            "Claude Code 1.2M today\n"
        );
        // A password workspace is Locked, because the CLI never prompts.
        std::fs::write(workspace.join("database-key.v1"), "synthetic").unwrap();
        assert_eq!(
            statusline(&[], &env(&workspace, Ok(Some(KEY))))
                .unwrap_err()
                .exit,
            7
        );
        assert_eq!(
            status(&["--loud".into()], &env(&workspace, Ok(Some(KEY))))
                .unwrap_err()
                .code,
            "usage"
        );
    }

    #[test]
    fn windows_and_reset_times_use_their_names_and_never_claim_a_reset() {
        let now: jiff::Timestamp = "2026-11-08T10:00:00Z".parse().unwrap();
        assert_eq!(window_name(Some(300)), "Five-hour");
        assert_eq!(window_name(Some(10_080)), "Seven-day");
        assert_eq!(window_name(Some(90)), "90-minute");
        assert_eq!(short_name(Some(300)), "5h");
        assert_eq!(short_name(Some(10_080)), "7d");
        assert_eq!(
            reset_text(Some("2026-11-08T12:13:00Z"), now),
            "resets in 2h 13m"
        );
        assert_eq!(
            reset_text(Some("2026-11-12T13:00:00Z"), now),
            "resets in 4d 3h"
        );
        assert_eq!(
            reset_text(Some("2026-11-08T09:00:00Z"), now),
            "the expected reset time passed"
        );
        assert_eq!(reset_text(None, now), "reset time Not available");
        assert_eq!(compact(1_234_567), "1.2M");
        assert_eq!(compact(999), "999");
    }
}
