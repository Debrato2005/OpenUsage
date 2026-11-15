//! One saved-data upload builder for offline previews and explicit online commands.
use crate::{CliError, Env};
use jiff::{Span, Timestamp, civil::Date, tz::TimeZone};
use openusage_locallogs::{AgentPeriod, UsagePeriod};
use openusage_storage::Storage;
use serde::Serialize;
use std::collections::BTreeSet;

mod online;
mod recovery;
const HELP: &str = "Usage: openusage board <join|sync|leave|forget> [options]

  join [--reconnect] [--yes] [--json]
      Join with GitHub device authorization. Reconnect reuses the saved device ID.
      An account change keeps the previous account's public data until it leaves.
  sync --dry-run --device-id UUID [--days N] [--json]
      Print the exact upload from saved local totals, without network or enrollment.
      Supply a lowercase random version 4 UUID. It does not establish identity.
  sync [--days N] [--yes] [--json]
      Upload saved local log totals for the enrolled device. Default: 30 days.
      Days range from 1 to 366 in each saved day zone. No scan runs.
  leave [--reauthorize] [--yes] [--json]
      Remove the account, all its devices, and all its public leaderboard data.
      Reauthorize uses GitHub when a local credential is lost or no longer accepted.
      The GitHub account you choose is the account that leaves.
  forget [--yes] [--json]
      Clear only this device's local enrollment. Public data is unchanged.

Options:
  --yes          Consent without a prompt. GitHub authorization still requires a person.
  --json         Print versioned results and failures. Dry-run success is the upload body.
  --help         Show this help. Also available for each board command.
";
const MAX_COUNT: u64 = 1_000_000_000_000;
const MAX_ROWS: usize = 400;
const MAX_BODY: usize = 64 * 1024;

fn invalid(message: &str) -> CliError {
    CliError {
        code: "board_invalid_data",
        exit: 12,
        message: message.into(),
    }
}
fn incomplete(message: &str) -> CliError {
    CliError {
        code: "board_incomplete_data",
        exit: 11,
        message: message.into(),
    }
}
fn too_large() -> CliError {
    CliError {
        code: "board_too_large",
        exit: 13,
        message: "The prospective upload exceeds the service limits. Select fewer days.".into(),
    }
}
fn read_failed() -> CliError {
    CliError {
        code: "read_failed",
        exit: 10,
        message: "OpenUsage could not read the saved local usage.".into(),
    }
}
fn uuid(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(i, b)| match i {
            8 | 13 | 18 | 23 => b == b'-',
            14 => b == b'4',
            19 => b"89ab".contains(&b),
            _ => b.is_ascii_digit() || (b'a'..=b'f').contains(&b),
        })
}

pub fn command(
    args: &[String],
    env: &Env,
    input: &mut dyn std::io::BufRead,
    prompt: &mut dyn std::io::Write,
) -> Result<String, CliError> {
    if args.iter().any(|a| a == "--help" || a == "-h") {
        return Ok(HELP.into());
    }
    let Some(action @ ("join" | "sync" | "leave" | "forget")) = args.first().map(String::as_str)
    else {
        return Err(CliError::usage("Use 'openusage board --help'."));
    };
    let mut dry_run = false;
    let mut device_id = None;
    let mut days = None;
    let mut json = false;
    let mut yes = false;
    let mut reconnect = false;
    let mut reauthorize = false;
    let mut options = args[1..].iter();
    while let Some(option) = options.next() {
        match option.as_str() {
            "--dry-run" if action == "sync" && !dry_run => dry_run = true,
            "--json" if !json => json = true,
            "--yes" if !yes => yes = true,
            "--reconnect" if action == "join" && !reconnect => reconnect = true,
            "--reauthorize" if action == "leave" && !reauthorize => reauthorize = true,
            "--device-id" if action == "sync" && device_id.is_none() => {
                device_id = Some(options.next().filter(|id| uuid(id)).ok_or_else(|| {
                    CliError::usage("--device-id needs a lowercase random version 4 UUID.")
                })?);
            }
            "--days" if action == "sync" && days.is_none() => {
                days = options.next().and_then(|v| v.parse::<i32>().ok());
                if !days.is_some_and(|n| (1..=366).contains(&n)) {
                    return Err(CliError::usage("--days needs an integer from 1 to 366."));
                }
            }
            _ => {
                return Err(CliError::usage(
                    "Unknown or repeated board option. Use 'openusage board --help'.",
                ));
            }
        }
    }
    if dry_run {
        if yes {
            return Err(CliError::usage("--yes does not apply to a dry run."));
        }
        let device_id = device_id.ok_or_else(|| {
            CliError::usage("Supply --device-id with a lowercase random version 4 UUID.")
        })?;
        return upload(env, device_id, days.unwrap_or(30));
    }
    if device_id.is_some() {
        return Err(CliError::usage(
            "--device-id applies only to --dry-run. Online sync uses the saved enrollment.",
        ));
    }
    let command = match action {
        "join" => online::Command::Join { reconnect },
        "sync" => online::Command::Sync {
            days: days.unwrap_or(30),
        },
        "leave" if reauthorize => online::Command::RecoverLeave,
        "leave" => online::Command::Leave,
        "forget" => online::Command::Forget,
        _ => unreachable!(),
    };
    online::command(command, yes, json, env, input, prompt)
}

pub(super) fn acquire_guard() -> Result<openusage_keys::BoardGuard, openusage_keys::BoardGuardError>
{
    #[cfg(all(feature = "native-test", target_os = "linux"))]
    {
        let root = openusage_keys::private_keyring_fixture()
            .map_err(|_| openusage_keys::BoardGuardError::UnsafePath)?;
        openusage_keys::BoardGuard::acquire(&root.join("leaderboard"))
    }
    #[cfg(not(all(feature = "native-test", target_os = "linux")))]
    openusage_keys::BoardGuard::for_current_user()
}

fn upload(env: &Env, device_id: &str, days: i32) -> Result<String, CliError> {
    let storage = crate::status::open(env)?;
    storage
        .read_snapshot(|snapshot| {
            serialize_sync(
                snapshot,
                device_id,
                days,
                env.now.unwrap_or_else(Timestamp::now),
            )
        })
        .map_err(|_| read_failed())?
}

/// The only upload fields. Future sync must send these serialized bytes unchanged.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SyncBody<'a> {
    device_id: &'a str,
    rows: Vec<SyncRow>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SyncRow {
    date: String,
    tool: String,
    model: String,
    input_tokens: u64,
    output_tokens: u64,
    cache_read_tokens: u64,
    cache_write_tokens: u64,
    total_tokens: u64,
    estimated_value_micros: Option<u64>,
}
impl SyncRow {
    fn validate(&self, today: Date) -> Result<(), CliError> {
        let date = self
            .date
            .parse::<Date>()
            .ok()
            .filter(|d| d.to_string() == self.date);
        let earliest = today
            .checked_sub(Span::new().days(400))
            .map_err(|_| invalid("The service date range is unavailable."))?;
        if date.is_none_or(|date| date < earliest || date > today) {
            return Err(invalid(
                "A saved local date falls outside the service UTC date range. Its pinned day zone cannot be changed.",
            ));
        }
        if !matches!(
            self.tool.as_str(),
            "claude"
                | "codex"
                | "opencode"
                | "amp"
                | "droid"
                | "codebuff"
                | "hermes"
                | "pi"
                | "goose"
                | "kilo"
                | "copilot"
                | "gemini"
                | "antigravity"
                | "kimi"
                | "qwen"
                | "openclaw"
                | "grok"
                | "zcode"
        ) || !(1..=64).contains(&self.model.len())
            || !self
                .model
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b".:-".contains(&b))
        {
            return Err(invalid(
                "A saved tool or model does not meet the service identifier rules.",
            ));
        }
        let parts = [
            self.input_tokens,
            self.output_tokens,
            self.cache_read_tokens,
            self.cache_write_tokens,
        ];
        if parts.iter().any(|n| *n > MAX_COUNT)
            || self.total_tokens > MAX_COUNT
            || self.estimated_value_micros.is_some_and(|n| n > MAX_COUNT)
            || parts.into_iter().try_fold(0u64, u64::checked_add) != Some(self.total_tokens)
        {
            return Err(invalid(
                "Saved token counts or estimates do not meet the service count rules.",
            ));
        }
        Ok(())
    }
}

fn range(now: Timestamp, zone: &str, days: i32) -> Result<(String, String), CliError> {
    let zone = TimeZone::get(zone).map_err(|_| invalid("A saved local day zone is invalid."))?;
    let end = now.to_zoned(zone).date();
    let start = end
        .checked_sub(Span::new().days(days - 1))
        .map_err(|_| invalid("The saved local date range is unavailable."))?;
    Ok((start.to_string(), end.to_string()))
}

fn grouped_rows(tool: &str, day: &str, period: &AgentPeriod) -> Result<Vec<SyncRow>, CliError> {
    if period.models.is_empty() {
        return Err(incomplete(
            "Saved local usage has no complete model attribution. No upload was produced.",
        ));
    }
    let mut rows = Vec::new();
    let mut total = 0u64;
    for model in &period.models {
        let details = &model.details;
        let (
            Some(input_tokens),
            Some(output_tokens),
            Some(cache_read_tokens),
            Some(cache_write_tokens),
            Some(total_tokens),
        ) = (
            details.input_tokens,
            details.output_tokens,
            details.cache_read_tokens,
            details.cache_write_tokens,
            model.total_tokens,
        )
        else {
            return Err(incomplete(
                "Saved model token categories are unavailable. No upload was produced.",
            ));
        };
        total = total
            .checked_add(total_tokens)
            .ok_or_else(|| invalid("Saved model token totals overflow."))?;
        rows.push(SyncRow {
            date: day.into(),
            tool: tool.into(),
            model: model.model.clone(),
            input_tokens,
            output_tokens,
            cache_read_tokens,
            cache_write_tokens,
            total_tokens,
            estimated_value_micros: model.estimated_value_micros,
        });
    }
    if total != period.total_tokens {
        return Err(incomplete(
            "Saved model totals do not cover the independent period total. No upload was produced.",
        ));
    }
    Ok(rows)
}

/// Reads only saved local aggregates. Enrollment and transport cannot change the request shape.
/// Errors contain no saved identifiers, paths, model text or account data.
pub(crate) fn serialize_sync(
    storage: &Storage,
    device_id: &str,
    days: i32,
    now: Timestamp,
) -> Result<String, CliError> {
    if !uuid(device_id) || !(1..=366).contains(&days) {
        return Err(CliError::usage(
            "A valid prospective device UUID and day count are required.",
        ));
    }
    let mut rows = Vec::new();
    let mut grouped_tools = BTreeSet::new();
    for definition in openusage_locallogs::tools() {
        let Some(scope) = storage
            .local_tool_scope(&definition.id)
            .map_err(|_| read_failed())?
        else {
            continue;
        };
        grouped_tools.insert(scope.tool.clone());
        let periods = storage
            .local_periods(&scope.tool)
            .map_err(|_| read_failed())?;
        if periods
            .iter()
            .any(|p| p.period.period == UsagePeriod::Unplaced)
        {
            return Err(incomplete(
                "Saved local usage has an undated period. An observation time cannot supply its date.",
            ));
        }
        let Some(zone) = scope.time_zone.as_deref() else {
            continue;
        };
        let (from, to) = range(now, zone, days)?;
        for period in periods {
            let UsagePeriod::Day(day) = &period.period.period else {
                continue;
            };
            if day >= &from && day <= &to {
                rows.extend(grouped_rows(&scope.tool, day, &period.period)?);
                if rows.len() > MAX_ROWS {
                    return Err(too_large());
                }
            }
        }
    }
    // Legacy rows are coherent snapshots, but retained model maxima and multiple folders
    // cannot establish one non-overlapping total. Grouped and legacy generations never add.
    let sources = storage.log_sources().map_err(|_| read_failed())?;
    let utc = now.to_zoned(TimeZone::UTC).date();
    let broad_from = utc
        .checked_sub(Span::new().days(days))
        .map_err(|_| invalid("The local date range is unavailable."))?
        .to_string();
    let broad_to = utc
        .checked_add(Span::new().days(1))
        .map_err(|_| invalid("The local date range is unavailable."))?
        .to_string();
    let legacy = storage
        .local_usage_days(&broad_from, &broad_to)
        .map_err(|_| read_failed())?;
    let mut legacy_sources = std::collections::BTreeMap::new();
    for saved in legacy {
        let source = sources
            .iter()
            .find(|s| s.id == saved.log_source_id)
            .ok_or_else(read_failed)?;
        let zone = source
            .time_zone
            .as_deref()
            .ok_or_else(|| invalid("Saved legacy usage has no pinned day zone."))?;
        let (from, to) = range(now, zone, days)?;
        let row = saved.counts;
        if row.local_date < from || row.local_date > to {
            continue;
        }
        if grouped_tools.contains(&row.tool) {
            return Err(incomplete(
                "Grouped and legacy local usage overlap for one tool. No upload was produced.",
            ));
        }
        if legacy_sources
            .insert(row.tool.clone(), saved.log_source_id.clone())
            .is_some_and(|id| id != saved.log_source_id)
        {
            return Err(incomplete(
                "Multiple saved folders can overlap for one tool. No upload was produced.",
            ));
        }
        if saved.coverage != openusage_storage::LocalUsageCoverage::Complete {
            return Err(incomplete(
                "Saved legacy model maxima retain missing or lower logs. No upload was produced.",
            ));
        }
        rows.push(SyncRow {
            date: row.local_date,
            tool: row.tool,
            model: row.model,
            input_tokens: row.input_tokens,
            output_tokens: row.output_tokens,
            cache_read_tokens: row.cache_read_tokens,
            cache_write_tokens: row.cache_write_tokens,
            total_tokens: row.total_tokens,
            estimated_value_micros: row.estimated_value_usd_micros,
        });
        if rows.len() > MAX_ROWS {
            return Err(too_large());
        }
    }
    rows.sort_by(|a, b| (&a.date, &a.tool, &a.model).cmp(&(&b.date, &b.tool, &b.model)));
    let mut keys = BTreeSet::new();
    for row in &rows {
        row.validate(utc)?;
        if !keys.insert((&row.date, &row.tool, &row.model)) {
            return Err(invalid(
                "Saved local usage repeats an upload key. No upload was produced.",
            ));
        }
    }
    let body = serde_json::to_string(&SyncBody { device_id, rows })
        .map_err(|_| invalid("The prospective upload could not be serialized."))?;
    if body.len() > MAX_BODY {
        return Err(too_large());
    }
    Ok(body)
}

#[cfg(all(test, unix))]
mod tests;
