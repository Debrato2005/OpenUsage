//! `openusage`, `openusage status --json` and `openusage statusline` read saved data only.
//! They open the database read-only, never prompt, and never use the network (OU-CLI-002, OU-CLI-006).

use crate::local_status;
use crate::{CliError, Env, SCHEMA_VERSION, grouped, known_options};
use openusage_keys::VaultError;
use openusage_storage::{ProfileConnectionState, QuotaWindow, Storage, StorageError};
use serde::Serialize;

pub const STATUS_HELP: &str = "Usage: openusage status [--json]

Runs when openusage has no command. Shows saved quotas and local usage by saved
scope. Each dated scope uses today in its saved time zone. Undated usage and
legacy folders stay separate. It reads
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

/// Quota readings stay Current for 10 minutes after observation (OU-STATE-005, OU-STATE-007).
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
struct Status {
    schema_version: u32,
    command: &'static str,
    observed_at_utc: String,
    profiles: Vec<Profile>,
    local: local_status::Local,
}

fn locked(message: &str) -> CliError {
    CliError {
        code: "locked",
        exit: 7,
        message: message.into(),
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
                let state = match saved.commit.observed_at_utc.parse::<jiff::Timestamp>() {
                    Ok(observed) if observed > now => "unknown",
                    Ok(observed)
                        if now.duration_since(observed)
                            < jiff::SignedDuration::from_secs(QUOTA_FRESH_SECONDS) =>
                    {
                        "current"
                    }
                    Ok(_) => "stale",
                    Err(_) => "unknown",
                };
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
                    state,
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
    Ok(Status {
        schema_version: SCHEMA_VERSION,
        command: "status",
        observed_at_utc: now.strftime("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
        profiles,
        local: local_status::read(storage, now)?,
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

fn connection_detail(profile: &Profile) -> String {
    let label = match profile.connection_state {
        "authentication_required" => "Authentication required",
        "unauthorized" => "Unauthorized",
        "offline" => "Offline",
        "unsupported" => "Unsupported",
        "error" => "Error",
        _ => return String::new(),
    };
    let observed = profile.quota.observed_at_utc.as_deref().filter(|value| {
        !value.chars().any(char::is_control) && value.parse::<jiff::Timestamp>().is_ok()
    });
    match observed {
        Some(observed) => format!(" {label} (observed {observed})"),
        None => format!(" {label}"),
    }
}

fn text(status: &Status, now: jiff::Timestamp) -> String {
    let mut out = String::new();
    if status.profiles.is_empty() {
        out.push_str("No accounts\n");
    }
    for profile in &status.profiles {
        out.push_str(&format!(
            "{}{}\n",
            profile.label,
            connection_detail(profile)
        ));
        if profile.quota.windows.is_empty() {
            out.push_str("  Quota Not available\n");
        }
        let freshness = match profile.quota.state {
            "stale" => "  Stale",
            "unknown" => "  Freshness unknown",
            _ => "",
        };
        for window in &profile.quota.windows {
            out.push_str(&format!(
                "  {:<10} {}% left, {}{}\n",
                window_name(window.duration_mins),
                window.remaining_percent,
                reset_text(window.resets_at_utc.as_deref(), now),
                freshness
            ));
        }
    }
    out.push_str("This device\n");
    if status.local.scopes.is_empty() {
        out.push_str("  No saved local usage today\n");
    }
    for (index, scope) in status.local.scopes.iter().enumerate() {
        let total = scope
            .metrics
            .total_tokens
            .value
            .as_deref()
            .and_then(|v| v.parse().ok())
            .map(grouped)
            .unwrap_or_else(|| "Not available".into());
        let period = match (&scope.date, &scope.time_zone) {
            (Some(day), Some(zone)) => format!("{day}, {zone}"),
            _ => "Undated".into(),
        };
        out.push_str(&format!(
            "  {}: {total} tokens ({period})\n",
            scope_label(scope, index)
        ));
    }
    out
}

fn scope_label(scope: &local_status::LocalScope, index: usize) -> String {
    if scope.kind == "legacy" {
        format!("{} legacy {}", scope.name, index + 1)
    } else {
        scope.name.clone()
    }
}

fn line(status: &Status) -> String {
    let mut parts: Vec<String> = status
        .profiles
        .iter()
        .map(|profile| {
            if profile.quota.windows.is_empty() {
                return format!(
                    "{} Not available{}",
                    profile.label,
                    connection_detail(profile)
                );
            }
            let windows: Vec<String> = profile
                .quota
                .windows
                .iter()
                .map(|w| format!("{} {}%", short_name(w.duration_mins), w.remaining_percent))
                .collect();
            let freshness = match profile.quota.state {
                "stale" => " Stale",
                "unknown" => " Freshness unknown",
                _ => "",
            };
            format!(
                "{} {}{}{}",
                profile.label,
                windows.join(" "),
                freshness,
                connection_detail(profile)
            )
        })
        .collect();
    parts.extend(
        status
            .local
            .scopes
            .iter()
            .enumerate()
            .map(|(index, scope)| {
                let total = scope
                    .metrics
                    .total_tokens
                    .value
                    .as_deref()
                    .and_then(|v| v.parse().ok())
                    .map(compact)
                    .unwrap_or_else(|| "Not available".into());
                let period = scope
                    .time_zone
                    .as_ref()
                    .map(|z| format!("today ({z})"))
                    .unwrap_or_else(|| "undated".into());
                format!("{} {total} {period}", scope_label(scope, index))
            }),
    );
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
    use openusage_storage::{
        ActivitySourceKind, LogConsent, ProfileBinding, ProfileBindingInput,
        ProfileIdentityEvidence, QuotaLimit, QuotaReading,
    };

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

    fn quota_workspace(observed: &str) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let workspace = private(dir.path());
        let mut storage = Storage::create(&workspace.join("openusage.db"), &KEY).unwrap();
        let binding = ProfileBinding {
            profile_id: "00000001-0000-4000-8000-000000000000".into(),
            home_reference: "00000002-0000-4000-8000-000000000000".into(),
            binding_id: "00000003-0000-4000-8000-000000000000".into(),
        };
        storage
            .create_profile(
                &binding.profile_id,
                "Synthetic quota",
                &binding.home_reference,
            )
            .unwrap();
        storage
            .record_profile_connection(
                &binding.profile_id,
                &binding.home_reference,
                ProfileConnectionState::Connected,
            )
            .unwrap();
        let evidence = ProfileIdentityEvidence {
            email: "quota@example.invalid".into(),
            plan: None,
        };
        let association = ProfileBindingInput {
            profile_id: binding.profile_id.clone(),
            expected_home_reference: binding.home_reference.clone(),
            expected_previous_binding_id: None,
            new_binding_id: binding.binding_id.clone(),
            evidence: evidence.clone(),
            observed_at_utc: observed.into(),
        };
        storage
            .accept_quota(
                QuotaReading {
                    run_id: "00000004-0000-4000-8000-000000000000".into(),
                    binding,
                    identity_evidence: evidence,
                    provider_version: "0.163.0".into(),
                    source_kind: ActivitySourceKind::TestFixture,
                    started_at_utc: observed.into(),
                    observed_at_utc: observed.into(),
                    completed_at_utc: observed.into(),
                    limits: vec![QuotaLimit {
                        limit_id: "codex".into(),
                        limit_name: Some("Synthetic limit".into()),
                        primary: Some(QuotaWindow {
                            used_percent: 37,
                            window_duration_mins: Some(300),
                            resets_at_unix: Some(0),
                        }),
                        secondary: Some(QuotaWindow {
                            used_percent: 100,
                            window_duration_mins: Some(10_080),
                            resets_at_unix: None,
                        }),
                        reached_type: None,
                        monthly_credit_limit: None,
                        spend_control_reached: None,
                    }],
                    available_reset_credits: None,
                    reset_credit_details: None,
                },
                Some(association),
            )
            .unwrap();
        drop(storage);
        (dir, workspace)
    }

    fn command(workspace: &std::path::Path, now: &str, args: &[&str]) -> String {
        let mut env = env(workspace, Ok(Some(KEY)));
        env.now = Some(now.parse().unwrap());
        let mut prompt = Vec::new();
        let (result, _) = crate::run(
            &args.iter().map(|arg| (*arg).into()).collect::<Vec<_>>(),
            &env,
            &mut std::io::empty(),
            &mut prompt,
        );
        assert!(prompt.is_empty());
        result.unwrap()
    }

    #[test]
    fn future_quota_commands_keep_values_with_unknown_freshness_without_writes() {
        let (_dir, workspace) = quota_workspace("2026-11-08T10:00:00.500Z");
        let database = workspace.join("openusage.db");
        let before = std::fs::read(&database).unwrap();
        let now = "2026-11-08T10:00:00.000Z";
        let json: serde_json::Value =
            serde_json::from_str(&command(&workspace, now, &["status", "--json"])).unwrap();
        assert_eq!(json["schemaVersion"], 2);
        assert_eq!(json["command"], "status");
        assert_eq!(json["observedAtUtc"], now);
        assert_eq!(
            json["profiles"][0]["quota"],
            serde_json::json!({
                "state": "unknown",
                "status": "official",
                "source": "codex app-server 0.163.0",
                "observedAtUtc": "2026-11-08T10:00:00.500Z",
                "windows": [
                    {
                        "limitId": "codex", "limitName": "Synthetic limit", "role": "primary",
                        "durationMins": 300, "usedPercent": 37, "remainingPercent": 63,
                        "resetsAtUtc": "1970-01-01T00:00:00Z"
                    },
                    {
                        "limitId": "codex", "limitName": "Synthetic limit", "role": "secondary",
                        "durationMins": 10080, "usedPercent": 100, "remainingPercent": 0,
                        "resetsAtUtc": null
                    }
                ]
            })
        );
        let text = "Synthetic quota\n  Five-hour  63% left, the expected reset time passed  Freshness unknown\n  Seven-day  0% left, reset time Not available  Freshness unknown\nThis device\n  No saved local usage today\n";
        assert_eq!(command(&workspace, now, &["status"]), text);
        assert_eq!(command(&workspace, now, &[]), text);
        assert_eq!(
            command(&workspace, now, &["statusline"]),
            "Synthetic quota 5h 63% 7d 0% Freshness unknown\n"
        );
        assert_eq!(std::fs::read(&database).unwrap(), before);
        assert_eq!(std::fs::read_dir(&workspace).unwrap().count(), 1);
    }

    #[test]
    fn quota_commands_use_precise_ten_minute_boundary() {
        let (_dir, workspace) = quota_workspace("2026-11-08T10:00:00.500Z");
        for (now, state, mark) in [
            (
                "2026-11-08T10:00:00.499999999Z",
                "unknown",
                " Freshness unknown",
            ),
            ("2026-11-08T10:00:00.500Z", "current", ""),
            ("2026-11-08T10:00:00.500000001Z", "current", ""),
            ("2026-11-08T10:10:00.499999999Z", "current", ""),
            ("2026-11-08T10:10:00.500Z", "stale", " Stale"),
            ("2026-11-08T10:10:00.500000001Z", "stale", " Stale"),
            ("2026-11-08T10:10:01.000Z", "stale", " Stale"),
        ] {
            let json: serde_json::Value =
                serde_json::from_str(&command(&workspace, now, &["status", "--json"])).unwrap();
            assert_eq!(json["profiles"][0]["quota"]["state"], state, "{now}");
            assert_eq!(
                json["profiles"][0]["quota"]["observedAtUtc"],
                "2026-11-08T10:00:00.500Z"
            );
            assert_eq!(
                command(&workspace, now, &["statusline"]),
                format!("Synthetic quota 5h 63% 7d 0%{mark}\n"),
                "{now}"
            );
            let text_mark = if mark.is_empty() {
                String::new()
            } else {
                format!(" {mark}")
            };
            assert_eq!(
                command(&workspace, now, &["status"]),
                format!(
                    "Synthetic quota\n  Five-hour  63% left, the expected reset time passed{text_mark}\n  Seven-day  0% left, reset time Not available{text_mark}\nThis device\n  No saved local usage today\n"
                ),
                "{now}"
            );
        }
    }

    #[test]
    fn offline_commands_show_connection_and_last_observation_without_writes() {
        let (_dir, workspace) = quota_workspace("2026-11-08T10:00:00.500Z");
        let now = "2026-11-08T10:01:00.500Z";
        let before: serde_json::Value =
            serde_json::from_str(&command(&workspace, now, &["status", "--json"])).unwrap();
        assert_eq!(before["profiles"][0]["connectionState"], "connected");
        assert_eq!(before["profiles"][0]["quota"]["state"], "current");
        let database = workspace.join("openusage.db");
        let mut storage = Storage::open(&database, &KEY).unwrap();
        storage
            .record_profile_connection(
                "00000001-0000-4000-8000-000000000000",
                "00000002-0000-4000-8000-000000000000",
                ProfileConnectionState::Offline,
            )
            .unwrap();
        drop(storage);
        let saved = std::fs::read(&database).unwrap();
        let after: serde_json::Value =
            serde_json::from_str(&command(&workspace, now, &["status", "--json"])).unwrap();
        assert_eq!(after["profiles"][0]["connectionState"], "offline");
        assert_eq!(
            after["profiles"][0]["quota"],
            before["profiles"][0]["quota"]
        );
        let text = "Synthetic quota Offline (observed 2026-11-08T10:00:00.500Z)\n  Five-hour  63% left, the expected reset time passed\n  Seven-day  0% left, reset time Not available\nThis device\n  No saved local usage today\n";
        assert_eq!(command(&workspace, now, &["status"]), text);
        assert_eq!(command(&workspace, now, &[]), text);
        assert_eq!(
            command(&workspace, now, &["statusline"]),
            "Synthetic quota 5h 63% 7d 0% Offline (observed 2026-11-08T10:00:00.500Z)\n"
        );
        assert_eq!(std::fs::read(&database).unwrap(), saved);
        assert_eq!(std::fs::read_dir(&workspace).unwrap().count(), 1);
    }

    #[test]
    fn connection_failures_remain_visible_with_saved_quota() {
        let (_dir, workspace) = quota_workspace("2026-11-08T10:00:00.500Z");
        let now = "2026-11-08T10:01:00.500Z";
        let database = workspace.join("openusage.db");
        let before: serde_json::Value =
            serde_json::from_str(&command(&workspace, now, &["status", "--json"])).unwrap();
        for (state, wire, label) in [
            (
                ProfileConnectionState::AuthenticationRequired,
                "authentication_required",
                "Authentication required",
            ),
            (
                ProfileConnectionState::Unauthorized,
                "unauthorized",
                "Unauthorized",
            ),
            (
                ProfileConnectionState::Unsupported,
                "unsupported",
                "Unsupported",
            ),
            (ProfileConnectionState::Error, "error", "Error"),
        ] {
            let mut storage = Storage::open(&database, &KEY).unwrap();
            storage
                .record_profile_connection(
                    "00000001-0000-4000-8000-000000000000",
                    "00000002-0000-4000-8000-000000000000",
                    state,
                )
                .unwrap();
            drop(storage);
            let saved = std::fs::read(&database).unwrap();
            let after: serde_json::Value =
                serde_json::from_str(&command(&workspace, now, &["status", "--json"])).unwrap();
            assert_eq!(after["profiles"][0]["connectionState"], wire);
            assert_eq!(
                after["profiles"][0]["quota"],
                before["profiles"][0]["quota"]
            );
            assert_eq!(
                command(&workspace, now, &["status"]),
                format!(
                    "Synthetic quota {label} (observed 2026-11-08T10:00:00.500Z)\n  Five-hour  63% left, the expected reset time passed\n  Seven-day  0% left, reset time Not available\nThis device\n  No saved local usage today\n"
                )
            );
            assert_eq!(
                command(&workspace, now, &["statusline"]),
                format!(
                    "Synthetic quota 5h 63% 7d 0% {label} (observed 2026-11-08T10:00:00.500Z)\n"
                )
            );
            assert_eq!(std::fs::read(&database).unwrap(), saved);
            assert_eq!(std::fs::read_dir(&workspace).unwrap().count(), 1);
        }
    }

    #[test]
    fn connection_marks_keep_clock_freshness_and_missing_quota_separate() {
        let (_dir, workspace) = quota_workspace("2026-11-08T10:00:00.500Z");
        let database = workspace.join("openusage.db");
        let mut storage = Storage::open(&database, &KEY).unwrap();
        storage
            .record_profile_connection(
                "00000001-0000-4000-8000-000000000000",
                "00000002-0000-4000-8000-000000000000",
                ProfileConnectionState::Offline,
            )
            .unwrap();
        drop(storage);
        let saved = std::fs::read(&database).unwrap();
        for (now, state, mark) in [
            ("2026-11-08T10:00:00.499Z", "unknown", " Freshness unknown"),
            ("2026-11-08T10:10:00.500Z", "stale", " Stale"),
        ] {
            let json: serde_json::Value =
                serde_json::from_str(&command(&workspace, now, &["status", "--json"])).unwrap();
            assert_eq!(json["profiles"][0]["connectionState"], "offline");
            assert_eq!(json["profiles"][0]["quota"]["state"], state);
            assert_eq!(
                command(&workspace, now, &["statusline"]),
                format!(
                    "Synthetic quota 5h 63% 7d 0%{mark} Offline (observed 2026-11-08T10:00:00.500Z)\n"
                )
            );
            let text = command(&workspace, now, &["status"]);
            assert!(
                text.starts_with("Synthetic quota Offline (observed 2026-11-08T10:00:00.500Z)\n")
            );
            assert_eq!(text.matches(mark.trim_start()).count(), 2);
        }
        assert_eq!(std::fs::read(&database).unwrap(), saved);
        assert_eq!(std::fs::read_dir(&workspace).unwrap().count(), 1);

        let empty = tempfile::tempdir().unwrap();
        let workspace = private(empty.path());
        let database = workspace.join("openusage.db");
        let mut storage = Storage::create(&database, &KEY).unwrap();
        storage
            .create_profile(
                "00000005-0000-4000-8000-000000000000",
                "Empty profile",
                "00000006-0000-4000-8000-000000000000",
            )
            .unwrap();
        drop(storage);
        let saved = std::fs::read(&database).unwrap();
        let now = "2026-11-08T10:01:00.500Z";
        let json: serde_json::Value =
            serde_json::from_str(&command(&workspace, now, &["status", "--json"])).unwrap();
        assert_eq!(
            json["profiles"][0]["connectionState"],
            "authentication_required"
        );
        assert_eq!(json["profiles"][0]["quota"]["state"], "not_available");
        assert_eq!(
            json["profiles"][0]["quota"]["observedAtUtc"],
            serde_json::Value::Null
        );
        assert_eq!(
            json["profiles"][0]["quota"]["windows"],
            serde_json::json!([])
        );
        assert_eq!(
            command(&workspace, now, &["status"]),
            "Empty profile Authentication required\n  Quota Not available\nThis device\n  No saved local usage today\n"
        );
        assert_eq!(
            command(&workspace, now, &["statusline"]),
            "Empty profile Not available Authentication required\n"
        );
        assert_eq!(std::fs::read(&database).unwrap(), saved);
        assert_eq!(std::fs::read_dir(&workspace).unwrap().count(), 1);
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
            "No accounts\nThis device\n  Claude Code legacy 1: 1,234,567 tokens (2026-11-08, UTC)\n"
        );
        let json: serde_json::Value = serde_json::from_str(
            &status(&["--json".into()], &env(&workspace, Ok(Some(KEY)))).unwrap(),
        )
        .unwrap();
        assert_eq!(json["schemaVersion"], 2);
        assert_eq!(
            json["local"]["scopes"][0]["metrics"]["totalTokens"]["value"],
            "1234567"
        );
        assert_eq!(
            json["local"]["scopes"][0]["metrics"]["estimatedValueUsdMicros"]["value"],
            serde_json::Value::Null
        );
        assert_eq!(
            statusline(&[], &env(&workspace, Ok(Some(KEY)))).unwrap(),
            "Claude Code legacy 1 1.2M today (UTC)\n"
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
