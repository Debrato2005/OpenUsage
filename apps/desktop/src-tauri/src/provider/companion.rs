//! Fixed bundled companion first; reviewed installation discovery only when it is absent.
use super::*;

impl ProviderCoordinator {
    pub fn prepare_connection(
        &self,
        companion: &Path,
        managed_profiles: &Path,
    ) -> Result<ExecutableApproval, CommandError> {
        {
            let mut state = self
                .0
                .lock()
                .map_err(|_| CommandError::ProviderUnavailable)?;
            require_open(&state)?;
            if let Some(approved) = state.approved.as_mut() {
                if approved.metadata.source_kind != ExecutableSourceKind::OfficialArtifact {
                    return Err(CommandError::ProviderSourceUnverified);
                }
                approved.held.recheck()?;
                return Ok(approved.metadata.clone());
            }
        }
        // A present but corrupt companion must not silently fall back to a different installation.
        let paths = match fs::symlink_metadata(companion) {
            Ok(_) => vec![companion.to_owned()],
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                discovery::discover(managed_profiles)?
                    .candidates
                    .into_iter()
                    .map(|value| PathBuf::from(value.path))
                    .collect()
            }
            Err(_) => return Err(CommandError::ProviderSetupUnavailable),
        };
        for path in paths {
            if path.starts_with(managed_profiles)
                || std::env::var_os("CODEX_HOME")
                    .is_some_and(|home| path.starts_with(PathBuf::from(home)))
            {
                return Err(CommandError::ProviderSourceUnverified);
            }
            let candidate = self.inspect(
                path.to_str()
                    .ok_or(CommandError::InvalidProviderExecutable)?,
            )?;
            if candidate.source_kind == ExecutableSourceKind::OfficialArtifact {
                // All accepted bytes match the fixed official pin. Prefer the first safe location.
                // Add/Connect authorizes this pinned source; account sign-in has separate consent.
                return self.approve(&candidate.candidate_id, true);
            }
        }
        Err(CommandError::ProviderSetupUnavailable)
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    #[test]
    fn present_unverified_companion_is_never_approved_or_executed() {
        use std::os::unix::fs::PermissionsExt;
        let directory = tempfile::tempdir().unwrap();
        let companion = directory.path().join("openusage-codex");
        fs::write(&companion, b"#!/bin/sh\nexit 77\n").unwrap();
        fs::set_permissions(&companion, fs::Permissions::from_mode(0o700)).unwrap();
        let coordinator = ProviderCoordinator::default();
        assert!(matches!(
            coordinator.prepare_connection(&companion, &directory.path().join("profiles")),
            Err(CommandError::ProviderSetupUnavailable)
        ));
        assert!(coordinator.0.lock().unwrap().approved.is_none());
    }
    #[test]
    fn symlink_companion_is_rejected_before_approval() {
        let directory = tempfile::tempdir().unwrap();
        let companion = directory.path().join("openusage-codex");
        std::os::unix::fs::symlink("missing", &companion).unwrap();
        assert!(
            ProviderCoordinator::default()
                .prepare_connection(&companion, &directory.path().join("profiles"))
                .is_err()
        );
    }
}
