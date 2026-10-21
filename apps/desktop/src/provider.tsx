import { useEffect, useRef, useState, type ReactNode } from 'react';
import { Button } from './components/ui/button';
import { Input } from './components/ui/input';
import { Checkbox } from './components/ui/checkbox';
import { Radio, RadioGroup } from './components/ui/radio-group';
import { Field, FieldLabel, FieldDescription, FieldItem } from './components/ui/field';
import { CollapsiblePanel } from './components/ui/collapsible';
import { providerFailure } from './provider-errors';
import { ProfileQuotaRead } from './quota';
import { ProfileCollectionRead } from './collection';
import { ProfileMonitoringControls, type MonitoringAction } from './monitoring';
import { useActivityPreview } from './activity';
import { ProfileDeviceLogin } from './login';
import { invoke, type ProfileMonitoringStatus, type ProfileCollectionOutcome, type ProfileReadingReference, type ProviderConnectionDiagnostic, type ProviderExecutableApproval, type ProviderExecutableCandidate, type ProviderExecutableDiscovery } from './contracts';


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
  const [operation, setOperation] = useState<'find' | 'inspect' | 'approve' | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [pathError, setPathError] = useState(false);
  const [discovery, setDiscovery] = useState<ProviderExecutableDiscovery | null>(null);
  const [manualOpen, setManualOpen] = useState(false);
  const pathField = useRef<HTMLInputElement>(null);
  const foundPanel = useRef<HTMLDivElement>(null);
  const [focus, setFocus] = useState<'review' | 'approval' | 'error' | 'found' | 'path' | null>(null);
  const reviewPanel = useRef<HTMLDivElement>(null);
  const approvalPanel = useRef<HTMLDivElement>(null);
  const errorPanel = useRef<HTMLParagraphElement>(null);
  const pending = useRef(false);
  const alive = useRef(true);
  useEffect(() => { alive.current = true; return () => { alive.current = false; }; }, []);
  useEffect(() => {
    if (busy || !focus) return;
    const target = focus === 'review' ? reviewPanel.current : focus === 'approval' ? approvalPanel.current : focus === 'found' ? foundPanel.current : focus === 'path' ? pathField.current : errorPanel.current;
    if (target?.getClientRects().length && !target.closest('[hidden]')) target.focus();
    setFocus(null);
  }, [busy, focus, candidate, approval, discovery, manualOpen]);

  async function find() {
    if (busy || pending.current) return;
    pending.current = true;
    setBusy(true);
    setOperation('find');
    setDiscovery(null);
    setCandidate(null);
    setConfirmed(false);
    setError(null);
    setPathError(false);
    try {
      const found = await invoke('discover_provider_executables');
      if (!alive.current) return;
      setDiscovery(found);
      setManualOpen(found.candidates.length === 0);
      if (found.candidates.length === 1) {
        const selected = found.candidates[0]!.path;
        setPath(selected);
        setOperation('inspect');
        const reviewed = await invoke('inspect_provider_executable', { path: selected });
        if (alive.current) { setCandidate(reviewed); setFocus('review'); }
      } else {
        setPath('');
        setFocus(found.candidates.length === 0 ? 'path' : 'found');
      }
    } catch (failure) {
      if (alive.current) { setError(providerFailure(failure)); setManualOpen(true); setFocus('error'); }
    } finally { pending.current = false; setOperation(null); setBusy(false); }
  }

  function choose(value: string) {
    setPath(value); setCandidate(null); setConfirmed(false); setError(null); setPathError(false);
  }

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
      if (alive.current) { setError(providerFailure(failure)); setPathError(failure === 'invalid_provider_executable' || failure === 'invalid_arguments'); setFocus('error'); }
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
      if (alive.current) { setCandidate(null); setConfirmed(false); setError(providerFailure(failure)); setFocus('error'); }
    } finally { pending.current = false; setOperation(null); setBusy(false); }
  }

  const searchControls = <>
      <p className="provider-help" id="provider-find-help">Find your installed Codex to sign in and read usage.</p>
      <Button className="secondary-button" variant="outline" type="button" data-provider-find disabled={busy} aria-describedby="provider-find-help" onClick={() => { void find(); }}>{operation === 'find' ? 'Finding Codex…' : 'Find Codex'}</Button>
      {discovery && !candidate && <div ref={foundPanel} className="provider-discovery" tabIndex={-1} role="region" aria-label="Codex search results">
        <p role="status">{discovery.candidates.length === 0 ? 'Codex was not found. Enter its path under Advanced.' : discovery.candidates.length === 1 ? 'Codex found.' : `${discovery.candidates.length} installations found. Choose one.`}</p>
        {discovery.truncated && <p className="provider-help">More installations may be available under Advanced.</p>}
        {discovery.candidates.length > 1 && <form className="provider-discovery-form" onSubmit={event => { event.preventDefault(); void inspect(); }}>
          <Field disabled={busy} render={<fieldset disabled={busy} />}><legend id="discovered-codex-label">Codex executable</legend><RadioGroup name="discovered-codex" value={path} disabled={busy} aria-labelledby="discovered-codex-label" onValueChange={value => { if (typeof value === 'string' && discovery.candidates.some(found => found.path === value)) choose(value); }}>{discovery.candidates.map((found, index) => <FieldItem key={found.path}><FieldLabel className="backup-selection provider-discovery-choice"><Radio nativeButton render={<button type="button" />} id={`discovered-codex-${index}`} value={found.path} disabled={busy} /><span className="provider-path"><bdi>{found.path}</bdi></span></FieldLabel></FieldItem>)}</RadioGroup></Field>
          <Button className="secondary-button" variant="outline" type="submit" disabled={busy || !path}>{operation === 'inspect' ? 'Inspecting executable…' : 'Review selected executable'}</Button>
        </form>}
      </div>}
  </>;

  return <section className="provider-panel" aria-labelledby="provider-heading">
    <details>
      <summary data-provider-settings><h2 id="provider-heading">Codex connection</h2></summary>
      {!candidate && !approval && searchControls}
      {approval && !candidate && <div ref={approvalPanel} className="provider-approval-result" role="status" tabIndex={-1}>
        <h3>Ready to sign in</h3>
        <p className="provider-help">Open Accounts and sign in to a profile.</p>
        {approval.sourceKind === 'test_fixture' && <p className="provider-fixture-note" data-provider-fixture>Synthetic test data</p>}
        <details className="provider-advanced"><summary>Codex details</summary><p className="provider-path"><bdi>{approval.path}</bdi></p></details>
      </div>}
      {candidate && <div ref={reviewPanel} className="provider-executable-review" tabIndex={-1} role="region" aria-labelledby="provider-review-heading">
        <h3 id="provider-review-heading">Codex found</h3>
        {candidate.sourceKind === 'test_fixture' && <p className="provider-fixture-note" data-provider-fixture>Synthetic test data</p>}
        <p id="provider-trust-help" className="provider-help">{candidate.sourceVerified
          ? 'Sign in to your accounts after this step.'
          : 'This Codex installation could not be verified. Choose a supported installation under Advanced.'}</p>
        <form className="provider-approval-form" onSubmit={event => { event.preventDefault(); void approve(); }}>
          <Field disabled={busy || !candidate.sourceVerified}><FieldLabel className="backup-selection"><Checkbox nativeButton render={<button type="button" />} id="provider-executable-approved" required disabled={busy || !candidate.sourceVerified} checked={confirmed} aria-describedby="provider-trust-help" onCheckedChange={checked => setConfirmed(checked === true)} />Allow Codex to sign in and read usage.</FieldLabel></Field>
          <Button className="primary-button" type="submit" disabled={busy || !confirmed || !candidate.sourceVerified}>{operation === 'approve' ? 'Checking Codex…' : 'Continue'}</Button>
        </form>
        <details className="provider-advanced"><summary>Codex details</summary><dl className="backup-inspection-metadata"><dt>Version</dt><dd>{candidate.testedVersion}</dd><dt>Source</dt><dd>{candidate.sourceKind === 'official_artifact' ? 'Reviewed official artifact' : candidate.sourceKind === 'test_fixture' ? 'Reviewed synthetic provider fixture' : 'Not verified'}</dd><dt>File</dt><dd className="provider-path"><bdi>{candidate.path}</bdi></dd>{candidate.sourceUrl && <><dt>Source reference</dt><dd><bdi>{candidate.sourceUrl}</bdi></dd></>}<dt>Size</dt><dd>{candidate.fileBytes} bytes</dd><dt>SHA-256</dt><dd className="provider-checksum">{candidate.sha256}</dd></dl></details>
      </div>}
      <details className="provider-manual provider-connection-advanced" open={manualOpen} onToggle={event => setManualOpen(event.currentTarget.open)}>
          <summary data-provider-manual data-provider-advanced>Advanced</summary>
          {(candidate || approval) && searchControls}
          <Field className="unlock-form provider-inspection-form" render={<form noValidate onSubmit={event => { event.preventDefault(); void inspect(); }} />}>
            <FieldLabel htmlFor="provider-executable">Codex executable</FieldLabel>
            <Input ref={pathField} id="provider-executable" type="text" autoComplete="off" spellCheck={false} maxLength={4096} value={path} disabled={busy} aria-invalid={!!error && pathError} aria-describedby={`provider-executable-help${error && pathError ? ' provider-executable-error' : ''}`} onChange={event => choose(event.target.value)} />
            <FieldDescription id="provider-executable-help" render={<p />}>Enter an absolute file path. Inspection checks the source before you approve execution.</FieldDescription>
            <Button className="secondary-button" variant="outline" type="submit" disabled={busy}>{operation === 'inspect' ? 'Inspecting executable…' : 'Inspect executable'}</Button>
          </Field>
      </details>
      {error && <p ref={errorPanel} id="provider-executable-error" className="operation-error" role="alert" tabIndex={-1}>{error}</p>}
    </details>
  </section>;
}

export function ProfileProviderCheck({ profileId, profileLabel, approval, busy, managementActions, managementTrigger, onConnectionChanged, reconnectRequested = false, onReconnectRequested, autoInitialRead = false, onInitialReadRequested, onActivitySaved, onActivityFailed, onQuotaSaved, onQuotaFailed, onCollectionSaved, onCollectionFailed, monitoring, monitoringUnavailable, onMonitoringAction }: { profileId: string; profileLabel: string; approval: ProviderExecutableApproval; busy: boolean; managementActions: ReactNode; managementTrigger: ReactNode; onConnectionChanged: () => Promise<void>; reconnectRequested?: boolean; onReconnectRequested?: () => void; autoInitialRead?: boolean; onInitialReadRequested?: () => void; onActivitySaved: (value: ProfileReadingReference) => Promise<void>; onActivityFailed: () => Promise<void>; onQuotaSaved: (value: ProfileReadingReference) => Promise<void>; onQuotaFailed: () => Promise<void>; onCollectionSaved: (value: ProfileCollectionOutcome) => Promise<void>; onCollectionFailed: () => Promise<void>; monitoring: ProfileMonitoringStatus | null; monitoringUnavailable: boolean; onMonitoringAction: MonitoringAction }) {
  const [checking, setChecking] = useState(false);
  const [loggingIn, setLoggingIn] = useState(false);
  const [readingQuota, setReadingQuota] = useState(false);
  const [readingCollection, setReadingCollection] = useState(false);
  const [automaticRequest, setAutomaticRequest] = useState<number | null>(null);
  const automaticSequence = useRef(0);
  const [refreshWait, setRefreshWait] = useState(0);
  const [quotaGeneration, setQuotaGeneration] = useState(0);
  const [loginGeneration, setLoginGeneration] = useState(0);
  const checkGeneration = useRef(0);
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

  const activity = useActivityPreview({ profileId, profileLabel, approval, busy: busy || checking || loggingIn || readingQuota || readingCollection || refreshWait > 0, onSaved: onActivitySaved, onFailed: onActivityFailed, onStarted: () => {
    checkGeneration.current += 1; setDiagnostic(null); setError(null); setQuotaGeneration(value => value + 1); setLoginGeneration(value => value + 1);
  } });

  const reconnectConsumed = useRef(false);
  useEffect(() => {
    if (!reconnectRequested) { reconnectConsumed.current = false; return; }
    if (busy || reconnectConsumed.current) return;
    if (refreshWait > 0 || checking || loggingIn || readingQuota || readingCollection || activity.working) { onReconnectRequested?.(); return; }
    reconnectConsumed.current = true;
    // The paired read checks the selected managed account before and after both capabilities.
    setAutomaticRequest(++automaticSequence.current);
    onReconnectRequested?.();
  }, [reconnectRequested, busy, refreshWait, checking, loggingIn, readingQuota, readingCollection, activity.working, onReconnectRequested]);

  async function refreshReadingConnection() {
    try { await onConnectionChanged(); }
    catch { setError('Account status could not be refreshed. Reload accounts to check it.'); }
  }

  useEffect(() => {
    if (readingQuota || activity.working || readingCollection) return;
    let current = true;
    let timer: number | undefined;
    void invoke('load_profile_refresh_status', { profileId }).then(status => {
      if (!current || status.profileId !== profileId) return;
      const deadline = performance.now() + status.retryAfterSeconds * 1000;
      const update = () => {
        const remaining = Math.max(0, Math.ceil((deadline - performance.now()) / 1000));
        setRefreshWait(remaining);
        if (remaining === 0) window.clearInterval(timer);
      };
      update();
      if (status.retryAfterSeconds > 0) timer = window.setInterval(update, 250);
    }).catch(() => { /* Rust still checks eligibility before every provider start. */ });
    return () => { current = false; window.clearInterval(timer); };
  }, [profileId, readingQuota, activity.working, readingCollection]);

  async function check() {
    if (busy || loggingIn || readingQuota || readingCollection || activity.working || pending.current) return;
    activity.clear();
    checkGeneration.current += 1;
    pending.current = true;
    setQuotaGeneration(value => value + 1);
    setChecking(true);
    setDiagnostic(null);
    setError(null);
    try {
      const next = await invoke('check_profile_connection', { profileId });
      if (next.profileId !== profileId || next.approvalId !== approval.approvalId) throw new Error('Stale connection check.');
      if (alive.current) { setDiagnostic(next); setFocus(true); }
    } catch (failure) {
      if (alive.current) { setError(providerFailure(failure)); setFocus(true); }
    } finally { pending.current = false; if (alive.current) { setChecking(false); setCancelling(false); } }
  }

  async function cancel() {
    if (!pending.current || cancelling) return;
    const operation = checkGeneration.current;
    setCancelling(true);
    try { await invoke('cancel_profile_check', { profileId }); }
    catch (failure) { if (alive.current && checkGeneration.current === operation) setError(providerFailure(failure)); }
    finally { if (alive.current && checkGeneration.current === operation) setCancelling(false); }
  }

  const title = diagnostic?.authentication === 'authentication_required' ? 'Sign-in required'
    : diagnostic?.authentication === 'chatgpt' ? 'Account response received' : 'Account type not supported';
  return <div className="profile-provider-check">
    {checking && <div className="profile-actions"><span role="status">Checking connection…</span><Button className="text-button" variant="ghost" type="button" data-provider-cancel={profileId} aria-describedby={`profile-name-${profileId}`} disabled={cancelling} onClick={() => { void cancel(); }}>{cancelling ? 'Cancelling…' : 'Cancel check'}</Button></div>}
    <ProfileDeviceLogin onAuthenticated={() => {
      if (!autoInitialRead) return;
      setAutomaticRequest(++automaticSequence.current);
      onInitialReadRequested?.();
    }} onConnectionChanged={onConnectionChanged} key={`login:${loginGeneration}`} profileId={profileId} profileLabel={profileLabel} approval={approval} busy={busy || checking || readingQuota || readingCollection || activity.working} onActivity={setLoggingIn} onStarted={() => { setAutomaticRequest(null); activity.clear(); checkGeneration.current += 1; setDiagnostic(null); setError(null); setQuotaGeneration(value => value + 1); }} />
    <ProfileCollectionRead automaticRequest={automaticRequest} profileId={profileId} approval={approval} busy={busy || checking || loggingIn || readingQuota || activity.working} retryAfterSeconds={refreshWait} onActivity={setReadingCollection} onSaved={async value => { await onCollectionSaved(value); await refreshReadingConnection(); }} onFailed={async () => { await onCollectionFailed(); await refreshReadingConnection(); }} onStarted={() => { activity.clear(); checkGeneration.current += 1; setDiagnostic(null); setError(null); setQuotaGeneration(value => value + 1); setLoginGeneration(value => value + 1); }}>
      {(collectionAction, collectionCancel) => <ProfileQuotaRead retryAfterSeconds={refreshWait} onSaved={onQuotaSaved} onFailed={onQuotaFailed} key={`quota:${quotaGeneration}`} profileId={profileId} profileLabel={profileLabel} approval={approval} busy={busy || checking || loggingIn || readingCollection || activity.working} onActivity={setReadingQuota} onStarted={() => { activity.clear(); checkGeneration.current += 1; setDiagnostic(null); setError(null); setLoginGeneration(value => value + 1); }} renderActions={(quotaAction, quotaCancel) => <>
        <div className="profile-actions">{collectionAction}{collectionCancel}{quotaCancel}{managementTrigger}</div>
        <CollapsiblePanel keepMounted className="profile-management">
          {managementActions}
          <ProfileMonitoringControls profileId={profileId} status={monitoring} unavailable={monitoringUnavailable} approval={approval} busy={busy || checking || loggingIn || readingQuota || readingCollection || activity.working} onAction={onMonitoringAction} />
          {quotaAction}
          {activity.action}
          <Button className="text-button" variant="ghost" type="button" data-provider-check={profileId} aria-describedby={`profile-name-${profileId}`} disabled={busy || checking || loggingIn || readingQuota || readingCollection || activity.working} onClick={() => { void check(); }}>Check connection</Button>
        </CollapsiblePanel>
      </>} />}
    </ProfileCollectionRead>
    {refreshWait > 0 && <p className="provider-help" data-profile-refresh-status={profileId}>Next reading in {refreshWait < 60 ? `${refreshWait}s` : `${Math.floor(refreshWait / 60)}m ${refreshWait % 60}s`}</p>}
    {activity.content}
    {(diagnostic || error) && <div ref={result} className="provider-diagnostic" data-provider-profile={profileId} tabIndex={-1} role={error ? 'alert' : 'status'}>
      {error ? <p>{error}</p> : diagnostic && <>{diagnostic.sourceKind === 'test_fixture' && <p className="provider-fixture-note" data-provider-fixture>Synthetic provider fixture</p>}<p><strong>{title}</strong></p><p>{diagnostic.authentication === 'authentication_required'
        ? 'Codex reported no authorized account in this profile. Choose Sign in to connect this profile.'
        : diagnostic.authentication === 'chatgpt' ? 'Identity has not been verified. This check does not collect usage readings.'
        : 'This check supports ChatGPT account connections.'}</p><p className="provider-help">Codex {diagnostic.providerVersion}</p></>}
    </div>}
  </div>;
}
