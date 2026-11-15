//! Local erasure owns no HTTP client and never ends public membership.
use crate::workspace::{CommandError, LocalCredentialErasure};
#[cfg(target_os = "linux")]
use openusage_keys::{BoardGuard, BoardGuardError, BoardVault};

pub(crate) struct Prepared {
    #[cfg(target_os = "linux")]
    vault: BoardVault,
    #[cfg(target_os = "linux")]
    guard: BoardGuard,
}

pub(crate) fn prepare() -> Result<Prepared, CommandError> {
    #[cfg(target_os = "linux")]
    {
        #[cfg(feature = "native-test")]
        let guard = {
            let root = openusage_keys::private_keyring_fixture()
                .map_err(|_| CommandError::LocalDeletionUnsafe)?;
            BoardGuard::acquire(&root.join("leaderboard"))
        };
        #[cfg(not(feature = "native-test"))]
        let guard = BoardGuard::for_current_user();
        let guard = guard.map_err(|error| match error {
            BoardGuardError::Busy => CommandError::LocalDeletionBusy,
            BoardGuardError::UnsafePath => CommandError::LocalDeletionUnsafe,
            BoardGuardError::Unavailable | BoardGuardError::Unsupported => {
                CommandError::LocalDeletionCoordinationUnavailable
            }
        })?;
        let vault = BoardVault::new().map_err(|_| CommandError::KeyStoreUnavailable)?;
        Ok(Prepared { vault, guard })
    }
    #[cfg(not(target_os = "linux"))]
    {
        // Product enrollment is Linux-only. Other platforms do not create this slot.
        Ok(Prepared {})
    }
}

impl LocalCredentialErasure for Prepared {
    fn erase(&self) -> Result<(), CommandError> {
        #[cfg(target_os = "linux")]
        self.vault
            .erase_local(&self.guard)
            .map_err(|_| CommandError::KeyStoreUnavailable)?;
        Ok(())
    }
}
