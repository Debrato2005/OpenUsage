//! One saved-data upload builder for offline previews and explicit online commands.
use crate::{CliError, Env};
use jiff::Timestamp;
use openusage_storage::BoardUploadError;

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
                device_id = Some(
                    options
                        .next()
                        .filter(|id| openusage_board::DeviceId::parse(id).is_ok())
                        .ok_or_else(|| {
                            CliError::usage("--device-id needs a lowercase random version 4 UUID.")
                        })?,
                );
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
    crate::status::open(env)?
        .leaderboard_upload(device_id, days, env.now.unwrap_or_else(Timestamp::now))
        .map_err(|error| {
            let (code, exit) = match error {
                BoardUploadError::InvalidSelection => ("usage", 2),
                BoardUploadError::Incomplete(_) => ("board_incomplete_data", 11),
                BoardUploadError::InvalidData(_) => ("board_invalid_data", 12),
                BoardUploadError::TooLarge => ("board_too_large", 13),
                BoardUploadError::ReadFailed => ("read_failed", 10),
            };
            CliError {
                code,
                exit,
                message: error.to_string(),
            }
        })
}

#[cfg(all(test, unix))]
mod tests;
