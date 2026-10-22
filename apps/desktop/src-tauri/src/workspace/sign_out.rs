use super::*;

impl Workspace {
    pub(crate) fn begin_profile_sign_out(
        &mut self,
        id: &str,
        approval_id: &str,
    ) -> Result<provider::PendingCheck, CommandError> {
        restore::validate_preview_id(approval_id)?;
        let (home, revision, path) = self.selected_provider_home_for(id, true)?;
        let pending = self
            .provider
            .start_logout(id, approval_id, &home, revision, &path)?;
        // The provider lease now excludes new selected-profile reads. Other profiles retain theirs.
        self.revoke_monitoring_profile(id, MonitoringIssue::SignInRequired);
        self.quota_sessions.remove(id);
        self.activity_sessions.remove(id);
        self.quota_requests.remove(id);
        self.activity_requests.remove(id);
        self.activity_reviews.remove(id);
        self.restore_preview.clear();
        Ok(pending)
    }

    pub(crate) fn finish_profile_sign_out(
        &mut self,
        joined: provider::JoinedCheck,
    ) -> Result<ProfileView, CommandError> {
        let id = joined.profile_id.clone();
        if let Err(error) = self.require_provider_binding_for(
            &id,
            &joined.home_reference,
            &joined.revision,
            CommandError::ProviderCheckCancelled,
            true,
        ) {
            joined.abort();
            return Err(error);
        }
        let result = joined.finish()?;
        if result.authentication != provider::DiagnosticAuthentication::AuthenticationRequired
            || result.identity_confidence != provider::DiagnosticIdentity::Unknown
            || !result.requires_openai_auth
        {
            return Err(CommandError::ProviderProtocolFailed);
        }
        self.storage
            .as_mut()
            .ok_or(CommandError::WorkspaceLocked)?
            .record_profile_sign_out(&id)
            .map_err(|_| CommandError::ProviderSignOutSaveFailed)?;
        self.restore_preview.clear();
        self.list_profiles()
    }
}
