//! Bounded opt-in leaderboard transport. No storage, credential discovery or scheduler.
mod device;
mod enrollment;
mod protocol;
mod transport;

pub use device::{DeviceFlow, PollOutcome};
pub use enrollment::Enrollment;
pub use protocol::{
    ApprovedSyncBody, DeviceId, GithubToken, Joined, Metric, OwnRank, Period, RankEntry,
    ServiceToken,
};

use std::{fmt, time::Duration};

/// Safe failure classes. No foreign error, response body or credential is retained.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Unconfigured,
    InvalidConfiguration,
    InvalidRequest,
    RequestTooLarge,
    ResponseTooLarge,
    UnexpectedResponse,
    Transport,
    Timeout,
    RedirectRefused,
    PollTooEarly { retry_at: Duration },
    ClockWentBackwards,
    FlowFinished,
    NotJoined,
    GithubRefused,
    RateLimited,
    TooManyDevices,
    ServiceUnavailable,
    Refused,
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Unconfigured => "The leaderboard is not configured in this build.",
            Self::InvalidConfiguration => "The leaderboard build configuration is invalid.",
            Self::InvalidRequest => "The leaderboard request is invalid.",
            Self::RequestTooLarge => "The leaderboard request exceeds 64 KiB.",
            Self::ResponseTooLarge => "The leaderboard response exceeds 64 KiB.",
            Self::UnexpectedResponse => "The remote response does not match the expected contract.",
            Self::Transport => "The secure request failed.",
            Self::Timeout => "The request exceeded its time limit.",
            Self::RedirectRefused => {
                "The remote endpoint returned a redirect. No redirect was followed."
            }
            Self::PollTooEarly { .. } => "Wait until the next permitted authorization poll.",
            Self::ClockWentBackwards => "Authorization requires a monotonic clock.",
            Self::FlowFinished => {
                "This authorization flow is finished. Start a new flow explicitly."
            }
            Self::NotJoined => "The service token is absent or no longer accepted.",
            Self::GithubRefused => "GitHub refused authorization.",
            Self::RateLimited => "The service rate limit was reached.",
            Self::TooManyDevices => "The account reached the service device limit.",
            Self::ServiceUnavailable => "The leaderboard service is unavailable.",
            Self::Refused => "The service refused the request.",
        })
    }
}
impl std::error::Error for Error {}

/// Configuration comes only from explicit build settings. Missing settings stay inert.
/// Requests are synchronous. Callers must keep them off an interactive UI thread.
pub struct BoardClient {
    transport: Option<transport::Transport>,
}
impl fmt::Debug for BoardClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BoardClient")
            .field("configured", &self.is_configured())
            .finish()
    }
}
impl BoardClient {
    pub fn from_build_config() -> Result<Self, Error> {
        match (
            option_env!("OPENUSAGE_BOARD_ORIGIN"),
            option_env!("OPENUSAGE_GITHUB_OAUTH_CLIENT_ID"),
        ) {
            (Some(origin), Some(client_id)) if !origin.is_empty() && !client_id.is_empty() => {
                Ok(Self {
                    transport: Some(transport::Transport::production(origin, client_id)?),
                })
            }
            _ => Ok(Self { transport: None }),
        }
    }
    pub fn is_configured(&self) -> bool {
        self.transport.is_some()
    }
    pub fn service_origin(&self) -> Option<&str> {
        self.transport.as_ref().map(|t| t.origin.as_str())
    }
    fn transport(&self) -> Result<&transport::Transport, Error> {
        self.transport.as_ref().ok_or(Error::Unconfigured)
    }

    /// The GitHub token is consumed and dropped after this one request.
    pub fn join(&self, github: GithubToken, device_id: &DeviceId) -> Result<Joined, Error> {
        self.transport()?.join(github, device_id)
    }
    /// Sends the approved original bytes. It never rebuilds or normalizes the payload.
    pub fn sync(&self, token: &ServiceToken, body: &ApprovedSyncBody) -> Result<u16, Error> {
        self.transport()?.sync(token, body)
    }
    pub fn leave(&self, token: &ServiceToken) -> Result<(), Error> {
        self.transport()?.leave(token)
    }
    pub fn own_rank(
        &self,
        token: &ServiceToken,
        period: Period,
        metric: Metric,
    ) -> Result<OwnRank, Error> {
        self.transport()?.own_rank(token, period, metric)
    }
}

#[cfg(test)]
mod tests;
