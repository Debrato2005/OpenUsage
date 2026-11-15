//! One build configuration and private acceptance location for Rust callers.
use crate::{BoardClient, EnrollmentSession, Error, SessionError};
use std::path::PathBuf;

pub struct ClientContext {
    client: BoardClient,
    fixture_directory: Option<PathBuf>,
}
impl ClientContext {
    pub fn from_current_build() -> Result<Self, Error> {
        #[cfg(all(feature = "native-test", target_os = "linux"))]
        if let Some(origin) = std::env::var_os("OPENUSAGE_NATIVE_BOARD_ORIGIN") {
            let invalid = || Error::InvalidConfiguration;
            let root = openusage_keys::private_keyring_fixture().map_err(|_| invalid())?;
            let certificate_path = root.join("board-ca.pem");
            if certificate_path.canonicalize().map_err(|_| invalid())? != certificate_path
                || !std::fs::symlink_metadata(&certificate_path)
                    .map_err(|_| invalid())?
                    .is_file()
            {
                return Err(invalid());
            }
            let certificate = std::fs::read(certificate_path).map_err(|_| invalid())?;
            let client = BoardClient::for_native_fixture(
                origin.to_str().ok_or_else(invalid)?,
                &certificate,
            )?;
            return Ok(Self {
                client,
                fixture_directory: Some(root.join("leaderboard")),
            });
        }
        BoardClient::from_build_config().map(|client| Self {
            client,
            fixture_directory: None,
        })
    }

    pub fn client(&self) -> &BoardClient {
        &self.client
    }

    pub fn open_session(&self) -> Result<EnrollmentSession, SessionError> {
        let origin = self
            .client
            .service_origin()
            .ok_or(SessionError::Configuration)?;
        #[cfg(all(feature = "native-test", target_os = "linux"))]
        if let Some(directory) = &self.fixture_directory {
            return EnrollmentSession::open_fixture(origin, directory);
        }
        let _ = &self.fixture_directory;
        EnrollmentSession::open(origin)
    }
}
