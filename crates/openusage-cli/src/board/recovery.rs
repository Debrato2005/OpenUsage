//! Explicit public departure and local forgetting have separate effects.
use super::online::{authorize, consent, failure, remote_error, session_error};
use crate::{CliError, Env, SCHEMA_VERSION};
use openusage_board::{BoardClient, Enrollment, SessionError};
use openusage_keys::{BoardCredential, BoardGuard, BoardGuardError, BoardVault, VaultError};
use serde::Serialize;
use std::io::{BufRead, Write};
use zeroize::Zeroizing;

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum CredentialState {
    Removed,
    NotPresent,
    Retained,
    Unavailable,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Outcome<'a> {
    schema_version: u32,
    command: &'static str,
    state: &'static str,
    login: Option<&'a str>,
    credential_state: CredentialState,
}
enum Finished<'a> {
    Departed(&'a str),
    Forgotten,
}
fn output(
    finished: Finished<'_>,
    credential_state: CredentialState,
    json: bool,
) -> Result<String, CliError> {
    let (command, state, login) = match finished {
        Finished::Departed(login) => ("board leave", "left", Some(login)),
        Finished::Forgotten => ("board forget", "forgotten", None),
    };
    let result = Outcome {
        schema_version: SCHEMA_VERSION,
        command,
        state,
        login,
        credential_state,
    };
    if json {
        return serde_json::to_string(&result)
            .map(|value| value + "\n")
            .map_err(|_| {
                failure(
                    "board_invalid_data",
                    12,
                    "The board result could not be serialized.",
                )
            });
    }
    let local = match credential_state {
        CredentialState::Removed => "The saved local credential was removed.",
        CredentialState::NotPresent => "No local enrollment is saved.",
        CredentialState::Retained => "The saved local enrollment was retained.",
        CredentialState::Unavailable => "Local enrollment status is unavailable.",
    };
    Ok(match login {
        Some(login) => format!(
            "Public departure confirmed for {login}. All account devices and public data were removed.\n{local}\n"
        ),
        None => "Local enrollment cleared. Public leaderboard data is unchanged.\n".into(),
    })
}
fn guard(env: &Env) -> Result<BoardGuard, CliError> {
    (env.board_guard)().map_err(|error| {
        session_error(match error {
            BoardGuardError::Busy => SessionError::Busy,
            BoardGuardError::UnsafePath => SessionError::UnsafePath,
            BoardGuardError::Unavailable => SessionError::StoreUnavailable,
            BoardGuardError::Unsupported => SessionError::Unsupported,
        })
    })
}

struct Active {
    vault: BoardVault,
    expected: BoardCredential,
    enrollment: Enrollment,
}
enum Local {
    Active(Box<Active>),
    Absent,
    Unverified,
    Unavailable,
}
impl Local {
    fn load(origin: &str) -> Self {
        let vault = match BoardVault::new() {
            Ok(vault) => vault,
            Err(_) => return Self::Unavailable,
        };
        let expected = match vault.load() {
            Ok(Some(record)) => record,
            Ok(None) => return Self::Absent,
            Err(VaultError::InvalidKey) => return Self::Unverified,
            Err(VaultError::Unavailable) => return Self::Unavailable,
        };
        match Enrollment::from_secret(origin, Zeroizing::new(expected.expose().to_vec())) {
            Ok(enrollment) => Self::Active(Box::new(Active {
                vault,
                expected,
                enrollment,
            })),
            Err(_) => Self::Unverified,
        }
    }
    fn token(&self) -> Option<&openusage_board::ServiceToken> {
        match self {
            Self::Active(active) => Some(active.enrollment.token()),
            _ => None,
        }
    }
    fn finish(&self, matched: Option<bool>) -> Result<CredentialState, CliError> {
        match self {
            Self::Active(active) if matched == Some(true) => {
                active.vault.remove(&active.expected).map_err(|_| failure("board_partial", 18,
                    "Public departure was confirmed, but local credential cleanup failed. Restore store access, then run board forget."))?;
                Ok(CredentialState::Removed)
            }
            Self::Active(_) | Self::Unverified => Ok(CredentialState::Retained),
            Self::Absent => Ok(CredentialState::NotPresent),
            Self::Unavailable => Ok(CredentialState::Unavailable),
        }
    }
}

pub(super) fn leave(
    client: BoardClient,
    yes: bool,
    json: bool,
    env: &Env,
    input: &mut dyn BufRead,
    prompt: &mut dyn Write,
) -> Result<String, CliError> {
    let origin = client
        .service_origin()
        .ok_or_else(|| remote_error(openusage_board::Error::Unconfigured))?;
    consent(
        "Use GitHub to leave the Self-reported leaderboard. This removes the account you choose, ALL its devices, and ALL its public data. An unrelated saved enrollment stays.",
        yes,
        env,
        input,
        prompt,
    )?;
    let _guard = guard(env)?;
    let local = Local::load(origin);
    let github = authorize(&client, prompt)?;
    let departure = client
        .leave_with_github(github, local.token())
        .map_err(remote_error)?;
    let state = local.finish(departure.local_token_revoked())?;
    output(Finished::Departed(departure.login()), state, json)
}

pub(super) fn forget(
    yes: bool,
    json: bool,
    env: &Env,
    input: &mut dyn BufRead,
    prompt: &mut dyn Write,
) -> Result<String, CliError> {
    consent(
        "Clear this device's saved leaderboard enrollment. Public leaderboard data is unchanged.",
        yes,
        env,
        input,
        prompt,
    )?;
    let guard = guard(env)?;
    let vault = BoardVault::new().map_err(|_| session_error(SessionError::StoreUnavailable))?;
    vault
        .erase_local(&guard)
        .map_err(|_| session_error(SessionError::StoreUnavailable))?;
    output(Finished::Forgotten, CredentialState::NotPresent, json)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn local_forgetting_requires_consent_and_uses_the_test_guard_without_store_access() {
        let env = Env::for_tests();
        assert_eq!(
            forget(
                false,
                true,
                &env,
                &mut std::io::empty(),
                &mut std::io::sink()
            )
            .unwrap_err()
            .exit,
            3
        );
        assert_eq!(
            forget(
                true,
                true,
                &env,
                &mut std::io::empty(),
                &mut std::io::sink()
            )
            .unwrap_err()
            .exit,
            15
        );
    }
}
