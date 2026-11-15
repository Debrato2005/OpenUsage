use crate::{CliError, Env, SCHEMA_VERSION};
use openusage_board::{ApprovedSyncBody, BoardClient, ClientContext, PollOutcome, SessionError};
use serde::Serialize;
use std::{
    io::{BufRead, Read, Write},
    time::Instant,
};

#[derive(Clone, Copy)]
pub(super) enum Command {
    Join { reconnect: bool },
    Sync { days: i32 },
    Leave,
    RecoverLeave,
    Forget,
}
impl Command {
    fn name(self) -> &'static str {
        match self {
            Self::Join { .. } => "board join",
            Self::Sync { .. } => "board sync",
            Self::Leave | Self::RecoverLeave => "board leave",
            Self::Forget => "board forget",
        }
    }
}

pub(super) fn failure(code: &'static str, exit: u8, message: impl Into<String>) -> CliError {
    CliError {
        code,
        exit,
        message: message.into(),
    }
}
pub(super) fn session_error(error: SessionError) -> CliError {
    match error {
        SessionError::Busy => failure("board_busy", 16, error.to_string()),
        SessionError::SaveFailed => failure(
            "board_partial",
            18,
            "The service joined, but the credential save failed. Run join --reconnect to recover the saved device.",
        ),
        _ => failure("board_credential", 15, error.to_string()),
    }
}
pub(super) fn remote_error(error: openusage_board::Error) -> CliError {
    match error {
        openusage_board::Error::Unconfigured | openusage_board::Error::InvalidConfiguration => {
            failure("board_unconfigured", 14, error.to_string())
        }
        _ => failure("board_remote", 17, error.to_string()),
    }
}
pub(super) fn consent(
    message: &str,
    yes: bool,
    env: &Env,
    input: &mut dyn BufRead,
    prompt: &mut dyn Write,
) -> Result<(), CliError> {
    if yes {
        return Ok(());
    }
    if !env.interactive {
        return Err(CliError::consent(
            "This board command requires consent. Supply --yes for scripts.",
        ));
    }
    writeln!(prompt, "{message}\nContinue? [y/N]")
        .map_err(|_| CliError::consent("The confirmation prompt failed."))?;
    prompt
        .flush()
        .map_err(|_| CliError::consent("The confirmation prompt failed."))?;
    let mut bytes = Vec::new();
    input
        .take(129)
        .read_until(b'\n', &mut bytes)
        .map_err(|_| CliError::consent("The confirmation could not be read."))?;
    if bytes.len() > 128
        || !matches!(
            bytes.as_slice(),
            b"y\n" | b"Y\n" | b"yes\n" | b"YES\n" | b"y" | b"Y" | b"yes" | b"YES"
        )
    {
        return Err(CliError::consent("The board command was not approved."));
    }
    Ok(())
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Outcome<'a> {
    schema_version: u32,
    command: &'a str,
    state: &'a str,
    device_id: Option<&'a str>,
    accepted: Option<u16>,
}
fn outcome(
    command: Command,
    state: &str,
    device_id: Option<&str>,
    accepted: Option<u16>,
    json: bool,
) -> Result<String, CliError> {
    if json {
        serde_json::to_string(&Outcome {
            schema_version: SCHEMA_VERSION,
            command: command.name(),
            state,
            device_id,
            accepted,
        })
        .map(|s| s + "\n")
        .map_err(|_| {
            failure(
                "board_invalid_data",
                12,
                "The board result could not be serialized.",
            )
        })
    } else {
        Ok(match (command, accepted, device_id) {
            (Command::Sync { .. }, Some(count), _) => format!("Synced {count} local usage rows.\n"),
            (Command::Join { .. }, _, Some(id)) => {
                format!("Joined the Self-reported leaderboard. Device {id}\n")
            }
            (Command::Leave, _, _) => {
                "Left the leaderboard. All account devices and public data were removed.\n".into()
            }
            _ => {
                return Err(failure(
                    "board_invalid_data",
                    12,
                    "The board result is incomplete.",
                ));
            }
        })
    }
}
pub(super) fn command(
    command: Command,
    yes: bool,
    json: bool,
    env: &Env,
    input: &mut dyn BufRead,
    prompt: &mut dyn Write,
) -> Result<String, CliError> {
    if matches!(command, Command::Forget) {
        return super::recovery::forget(yes, json, env, input, prompt);
    }
    let context = ClientContext::from_current_build().map_err(remote_error)?;
    let client = context.client();
    if matches!(command, Command::RecoverLeave) {
        return super::recovery::leave(client, yes, json, env, input, prompt);
    }
    if !client.is_configured() {
        return Err(remote_error(openusage_board::Error::Unconfigured));
    }
    if matches!(command, Command::Join { .. }) {
        consent(
            "Join the Self-reported leaderboard with GitHub. Sync shares local dates, tools, models, token counts, estimates, and a random device ID. Reconnect keeps the previous account's public data until that account leaves.",
            yes,
            env,
            input,
            prompt,
        )?;
    } else if matches!(command, Command::Leave) {
        consent(
            "Leave the leaderboard and remove this account, ALL its devices, and ALL its public leaderboard data.",
            yes,
            env,
            input,
            prompt,
        )?;
    } else if !yes && !env.interactive {
        return Err(CliError::consent(
            "Sync requires consent. Supply --yes for scripts.",
        ));
    }
    let mut session = context.open_session().map_err(session_error)?;
    match command {
        Command::Join { reconnect } => {
            let id = session.prepare_join(reconnect).map_err(session_error)?;
            let token = authorize(client, prompt)?;
            let joined = client.join(token, &id).map_err(remote_error)?;
            session.finish_join(joined).map_err(session_error)?;
            outcome(command, "joined", Some(id.as_str()), None, json)
        }
        Command::Sync { days } => {
            let enrollment = session.enrollment().map_err(session_error)?;
            let body = super::upload(env, enrollment.device_id().as_str(), days)?;
            let approved = ApprovedSyncBody::from_json(
                body.as_bytes().to_vec(),
                env.now.unwrap_or_else(jiff::Timestamp::now),
            )
            .map_err(remote_error)?;
            consent(
                &format!(
                    "Upload these exact local usage rows to the Self-reported leaderboard?\n{body}"
                ),
                yes,
                env,
                input,
                prompt,
            )?;
            let accepted = client
                .sync(enrollment.token(), &approved)
                .map_err(remote_error)?;
            outcome(
                command,
                "synced",
                Some(enrollment.device_id().as_str()),
                Some(accepted),
                json,
            )
        }
        Command::Leave => {
            client
                .leave(session.enrollment().map_err(session_error)?.token())
                .map_err(remote_error)?;
            session.clear_after_leave().map_err(|_| failure("board_partial", 18,
                "The service confirmed removal of all account data. The local credential could not be removed."))?;
            outcome(command, "left", None, None, json)
        }
        Command::RecoverLeave | Command::Forget => {
            unreachable!("handled before the enrollment session")
        }
    }
}
pub(super) fn authorize(
    client: &BoardClient,
    prompt: &mut dyn Write,
) -> Result<openusage_board::GithubToken, CliError> {
    let clock = Instant::now();
    let mut flow = client.device_start(clock.elapsed()).map_err(remote_error)?;
    writeln!(
        prompt,
        "Open {} and enter {}",
        flow.verification_uri(),
        flow.user_code()
    )
    .and_then(|_| prompt.flush())
    .map_err(|_| {
        failure(
            "board_remote",
            17,
            "The authorization code could not be shown.",
        )
    })?;
    let token = loop {
        let delay = flow.retry_at().saturating_sub(clock.elapsed());
        std::thread::sleep(delay);
        match client
            .device_poll(&mut flow, clock.elapsed())
            .map_err(remote_error)?
        {
            PollOutcome::Authorized(token) => break token,
            PollOutcome::Pending { .. } | PollOutcome::SlowDown { .. } => (),
            PollOutcome::Denied => {
                return Err(failure(
                    "board_remote",
                    17,
                    "GitHub authorization was denied.",
                ));
            }
            PollOutcome::Expired => {
                return Err(failure(
                    "board_remote",
                    17,
                    "GitHub authorization expired. Start the command again.",
                ));
            }
        }
    };
    Ok(token)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn consent_is_bounded_explicit_and_shows_the_exact_body() {
        let mut env = Env::for_tests();
        let mut prompt = Vec::new();
        assert_eq!(
            consent("body", false, &env, &mut std::io::empty(), &mut prompt)
                .unwrap_err()
                .exit,
            3
        );
        assert!(prompt.is_empty());
        consent("body", true, &env, &mut std::io::empty(), &mut prompt).unwrap();
        assert!(prompt.is_empty());
        env.interactive = true;
        let body = "{\"deviceId\":\"synthetic\",\"rows\":[]}";
        consent(body, false, &env, &mut &b"yes\n"[..], &mut prompt).unwrap();
        assert!(String::from_utf8(prompt.clone()).unwrap().contains(body));
        for bytes in [b"no\n".to_vec(), b"\n".to_vec(), vec![b'y'; 1024]] {
            assert_eq!(
                consent(body, false, &env, &mut bytes.as_slice(), &mut prompt)
                    .unwrap_err()
                    .exit,
                3
            );
        }
    }
}
