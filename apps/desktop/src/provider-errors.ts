export function providerFailure(error: unknown): string {
  const messages: Record<string, string> = {
    provider_discovery_unavailable: 'OpenUsage could not safely check executable locations. Enter the path to Codex to review it.',
    provider_login_not_found: 'This sign-in attempt has ended. Start a new sign-in if you still need it.',
    provider_login_in_progress: 'A sign-in is already in progress for this profile. Finish it or cancel it first.',
    provider_login_prompt_pending: 'Codex is preparing your sign-in code. Wait for the code before you continue.',
    provider_login_cancelled: 'Sign-in stopped. Credentials already saved by Codex may remain in this profile.',
    provider_login_timed_out: 'OpenUsage reached its sign-in wait limit. Start a new attempt if you still need to sign in.',
    provider_login_failed: 'Codex could not confirm a ChatGPT account after sign-in. Start a new attempt or check the connection.',
    invalid_arguments: 'OpenUsage could not read this request. Review the executable and try again.',
    workspace_locked: 'Unlock your workspace before you configure Codex.',
    provider_executable_not_approved: 'Review and approve a Codex executable before you check a connection.',
    invalid_provider_executable: 'OpenUsage could not verify this executable. Check its path and file permissions, then inspect it again.',
    stale_provider_approval: 'The executable or its review changed. Inspect the file again before you approve it.',
    provider_source_unverified: 'This file does not match a supported verified source. Use a supported official release, then inspect it again.',
    provider_check_cancelled: 'Connection check cancelled.',
    provider_check_in_progress: 'A provider operation is already running for this profile.',
    provider_capacity_reached: 'Two connection checks are already running. Wait for one to finish or cancel a check.',
    provider_unsupported: 'This Codex version or environment is not supported by the connection check.',
    provider_check_timed_out: 'Codex did not finish the connection check in time. Try again.',
    provider_protocol_failed: 'Codex returned an unsupported response. Review its version before you try again.',
    provider_home_unavailable: 'The private profile home is unavailable or changed. Keep your workspace files and reload profiles.',
    provider_profile_archived: 'Restore this archived profile before you check its connection.',
    provider_cleanup_failed: 'OpenUsage could not confirm that the provider process stopped. Restart OpenUsage before another check.',
    provider_unavailable: 'The connection service is unavailable. Restart OpenUsage before you try again.',
    provider_shutting_down: 'OpenUsage is closing. Reopen the application before you start another check.',
  };
  return typeof error === 'string' && messages[error]
    ? messages[error] : 'OpenUsage could not complete this request. Review the executable and try again.';
}

