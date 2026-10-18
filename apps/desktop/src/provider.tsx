import { useEffect, useRef, useState } from 'react';
import { invoke, type ProviderConnectionDiagnostic, type ProviderExecutableApproval, type ProviderExecutableCandidate } from './contracts';

function failureFor(error: unknown): string {
  const messages: Record<string, string> = {
    invalid_arguments: 'OpenUsage could not read this request. Review the executable and try again.',
    workspace_locked: 'Unlock your workspace before you configure Codex.',
    provider_executable_not_approved: 'Review and approve a Codex executable before you check a connection.',
    invalid_provider_executable: 'OpenUsage could not verify this executable. Check its path and file permissions, then inspect it again.',
    stale_provider_approval: 'The executable or its review changed. Inspect the file again before you approve it.',
    provider_source_unverified: 'This file does not match a supported verified source. Use a supported official release, then inspect it again.',
    provider_check_cancelled: 'Connection check cancelled.',
    provider_check_in_progress: 'This profile already has a connection check in progress.',
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

type SettingsProps = {
  busy: boolean;
  setBusy: (busy: boolean) => void;
  approval: ProviderExecutableApproval | null;
  onApproved: (approval: ProviderExecutableApproval) => void;
};

export function ProviderSettings({ busy, setBusy, approval, onApproved }: SettingsProps) {
  const [path, setPath] = useState('');
  const [candidate, setCandidate] = useState<ProviderExecutableCandidate | null>(null);
  const [confirmed, setConfirmed] = useState(false);
  const [operation, setOperation] = useState<'inspect' | 'approve' | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [pathError, setPathError] = useState(false);
  const [focus, setFocus] = useState<'review' | 'approval' | 'error' | null>(null);
  const reviewPanel = useRef<HTMLDivElement>(null);
  const approvalPanel = useRef<HTMLDivElement>(null);
  const errorPanel = useRef<HTMLParagraphElement>(null);
  const pending = useRef(false);
  const alive = useRef(true);
  useEffect(() => { alive.current = true; return () => { alive.current = false; }; }, []);
  useEffect(() => {
    if (busy || !focus) return;
    (focus === 'review' ? reviewPanel.current : focus === 'approval' ? approvalPanel.current : errorPanel.current)?.focus();
    setFocus(null);
  }, [busy, focus, candidate, approval]);

  async function inspect() {
    if (busy || pending.current) return;
    setCandidate(null);
    setConfirmed(false);
    setError(null);
    setPathError(false);
    if (!path.trim()) { setError('Enter the absolute path to your Codex executable.'); setPathError(true); setFocus('error'); return; }
    pending.current = true;
    setBusy(true);
    setOperation('inspect');
    try {
      const value = await invoke('inspect_provider_executable', { path });
      if (alive.current) { setCandidate(value); setFocus('review'); }
    } catch (failure) {
      if (alive.current) { setError(failureFor(failure)); setPathError(failure === 'invalid_provider_executable' || failure === 'invalid_arguments'); setFocus('error'); }
    } finally { pending.current = false; setOperation(null); setBusy(false); }
  }

  async function approve() {
    if (busy || pending.current || !candidate?.sourceVerified || !confirmed) return;
    pending.current = true;
    setBusy(true);
    setOperation('approve');
    setError(null);
    setPathError(false);
    try {
      const value = await invoke('approve_provider_executable', { candidateId: candidate.candidateId, confirmed: true });
      if (alive.current) { onApproved(value); setCandidate(null); setConfirmed(false); setPath(''); setFocus('approval'); }
    } catch (failure) {
      if (alive.current) { setCandidate(null); setConfirmed(false); setError(failureFor(failure)); setFocus('error'); }
    } finally { pending.current = false; setOperation(null); setBusy(false); }
  }

  return <section className="provider-panel" aria-labelledby="provider-heading">
    <details>
      <summary data-provider-settings><h2 id="provider-heading">Codex connection</h2></summary>
      <p className="provider-help">Approve a local Codex executable to check profile connections. Approval lasts for this application session. Usage collection and sign-in are not available yet.</p>
      {approval && <div ref={approvalPanel} className="backup-receipt provider-approval-result" role="status" tabIndex={-1}>
        <h3>Executable approved for this session</h3><p className="provider-path"><bdi>{approval.path}</bdi></p>
        <p className="provider-help">OpenUsage checks compatibility before each connection check.</p>
        {approval.sourceKind === 'test_fixture' && <p className="provider-fixture-note" data-provider-fixture>Synthetic provider fixture. Replies use controlled test data.</p>}
      </div>}
      <form className="unlock-form provider-inspection-form" noValidate onSubmit={event => { event.preventDefault(); void inspect(); }}>
        <label htmlFor="provider-executable">Codex executable</label>
        <input id="provider-executable" type="text" autoComplete="off" spellCheck={false} maxLength={4096} value={path} disabled={busy} aria-invalid={!!error && pathError} aria-describedby={`provider-executable-help${error && pathError ? ' provider-executable-error' : ''}`} onChange={event => { setPath(event.target.value); setCandidate(null); setConfirmed(false); setError(null); setPathError(false); }} />
        <p id="provider-executable-help">Enter an absolute file path. Inspection shows the path and checksum. Approval permits version and connection checks.</p>
        <button className="secondary-button" type="submit" disabled={busy}>{operation === 'inspect' ? 'Inspecting executable…' : 'Inspect executable'}</button>
      </form>
      {candidate && <div ref={reviewPanel} className="backup-receipt provider-executable-review" tabIndex={-1} role="region" aria-labelledby="provider-review-heading">
        <h3 id="provider-review-heading">Review executable</h3>
        <dl className="backup-inspection-metadata"><dt>File</dt><dd className="provider-path"><bdi>{candidate.path}</bdi></dd><dt>Size</dt><dd>{candidate.fileBytes} bytes</dd><dt>SHA-256</dt><dd className="provider-checksum">{candidate.sha256}</dd><dt>Source</dt><dd>{candidate.sourceKind === 'official_artifact' ? 'Reviewed official artifact' : candidate.sourceKind === 'test_fixture' ? 'Reviewed synthetic provider fixture' : 'Not verified'}</dd>{candidate.sourceUrl && <><dt>Source reference</dt><dd><bdi>{candidate.sourceUrl}</bdi></dd></>}<dt>Supported Codex version</dt><dd>{candidate.testedVersion}</dd></dl>
        <p id="provider-trust-help" className="provider-help">{candidate.sourceVerified
          ? 'The checksum matches a reviewed source. Approve this file only if you trust it. Each profile uses its own Codex home.'
          : 'This file does not match a supported verified source. OpenUsage cannot approve it for execution.'}</p>
        <form className="provider-approval-form" onSubmit={event => { event.preventDefault(); void approve(); }}>
          <label className="backup-selection"><input id="provider-executable-approved" type="checkbox" required disabled={busy || !candidate.sourceVerified} checked={confirmed} aria-describedby="provider-trust-help" onChange={event => setConfirmed(event.target.checked)} />I trust this executable and allow version and connection checks.</label>
          <button className="primary-button" type="submit" disabled={busy || !confirmed || !candidate.sourceVerified}>{operation === 'approve' ? 'Approving executable…' : 'Approve executable'}</button>
        </form>
      </div>}
      {error && <p ref={errorPanel} id="provider-executable-error" className="operation-error" role="alert" tabIndex={-1}>{error}</p>}
    </details>
  </section>;
}

export function ProfileProviderCheck({ profileId, approval, busy }: { profileId: string; approval: ProviderExecutableApproval; busy: boolean }) {
  const [checking, setChecking] = useState(false);
  const [cancelling, setCancelling] = useState(false);
  const [diagnostic, setDiagnostic] = useState<ProviderConnectionDiagnostic | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [focus, setFocus] = useState(false);
  const result = useRef<HTMLDivElement>(null);
  const pending = useRef(false);
  const alive = useRef(true);
  useEffect(() => { alive.current = true; return () => { alive.current = false; }; }, []);
  useEffect(() => {
    if (!checking && focus) { result.current?.focus(); setFocus(false); }
  }, [checking, focus, diagnostic, error]);

  async function check() {
    if (busy || pending.current) return;
    pending.current = true;
    setChecking(true);
    setDiagnostic(null);
    setError(null);
    try {
      const next = await invoke('check_profile_connection', { profileId });
      if (next.profileId !== profileId || next.approvalId !== approval.approvalId) throw new Error('Stale connection check.');
      if (alive.current) { setDiagnostic(next); setFocus(true); }
    } catch (failure) {
      if (alive.current) { setError(failureFor(failure)); setFocus(true); }
    } finally { pending.current = false; if (alive.current) { setChecking(false); setCancelling(false); } }
  }

  async function cancel() {
    if (!pending.current || cancelling) return;
    setCancelling(true);
    try { await invoke('cancel_profile_check', { profileId }); }
    catch (failure) { if (alive.current) setError(failureFor(failure)); }
    finally { if (alive.current) setCancelling(false); }
  }

  const title = diagnostic?.authentication === 'authentication_required' ? 'Sign-in required'
    : diagnostic?.authentication === 'chatgpt' ? 'Account response received' : 'Account type not supported';
  return <div className="profile-provider-check">
    <div className="profile-actions"><button className="secondary-button" type="button" data-provider-check={profileId} aria-describedby={`profile-name-${profileId}`} disabled={busy || checking} onClick={() => { void check(); }}>{checking ? 'Checking connection…' : 'Check connection'}</button>
      {checking && <button className="text-button" type="button" data-provider-cancel={profileId} aria-describedby={`profile-name-${profileId}`} disabled={cancelling} onClick={() => { void cancel(); }}>{cancelling ? 'Cancelling…' : 'Cancel check'}</button>}
    </div>
    {(diagnostic || error) && <div ref={result} className="provider-diagnostic" data-provider-profile={profileId} tabIndex={-1} role={error ? 'alert' : 'status'}>
      {error ? <p>{error}</p> : diagnostic && <>{diagnostic.sourceKind === 'test_fixture' && <p className="provider-fixture-note" data-provider-fixture>Synthetic provider fixture</p>}<p><strong>{title}</strong></p><p>{diagnostic.authentication === 'authentication_required'
        ? 'Codex reported no authorized account in this profile. Sign-in is not available in this build.'
        : diagnostic.authentication === 'chatgpt' ? 'Identity has not been verified. This check does not collect usage readings.'
        : 'This check supports ChatGPT account connections.'}</p><p className="provider-help">Codex {diagnostic.providerVersion} · Monitoring stopped</p></>}
    </div>}
  </div>;
}
