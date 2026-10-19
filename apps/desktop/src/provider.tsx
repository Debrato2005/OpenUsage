import { useEffect, useRef, useState, type ReactNode } from 'react';
import { Button } from './components/ui/button';
import { Input } from './components/ui/input';
import { Checkbox } from './components/ui/checkbox';
import { Radio, RadioGroup } from './components/ui/radio-group';
import { Field, FieldLabel, FieldDescription, FieldItem } from './components/ui/field';
import { CollapsiblePanel } from './components/ui/collapsible';
import { providerFailure } from './provider-errors';
import { ProfileQuotaRead } from './quota';
import { useActivityPreview } from './activity';
import { ProfileDeviceLogin } from './login';
import { invoke, type ProfileActivitySavedRead, type ProfileQuotaSavedRead, type ProviderConnectionDiagnostic, type ProviderExecutableApproval, type ProviderExecutableCandidate, type ProviderExecutableDiscovery } from './contracts';


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
    (focus === 'review' ? reviewPanel.current : focus === 'approval' ? approvalPanel.current : focus === 'found' ? foundPanel.current : focus === 'path' ? pathField.current : errorPanel.current)?.focus();
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

  return <section className="provider-panel" aria-labelledby="provider-heading">
    <details>
      <summary data-provider-settings><h2 id="provider-heading">Codex connection</h2></summary>
      <p className="provider-help">Review Codex for this session, then sign in or save quota and activity readings for a profile. Scheduled collection is not available yet.</p>
      {approval && <div ref={approvalPanel} className="backup-receipt provider-approval-result" role="status" tabIndex={-1}>
        <h3>Executable approved for this session</h3><p className="provider-path"><bdi>{approval.path}</bdi></p>
        <p className="provider-help">OpenUsage checks compatibility before each connection check.</p>
        {approval.sourceKind === 'test_fixture' && <p className="provider-fixture-note" data-provider-fixture>Synthetic provider fixture. Replies use controlled test data.</p>}
      </div>}
      <p className="provider-help" id="provider-find-help">OpenUsage checks known installation locations and folder metadata. It does not open sign-in files or run Codex.</p>
      <Button className="secondary-button" variant="outline" type="button" data-provider-find disabled={busy} aria-describedby="provider-find-help" onClick={() => { void find(); }}>{operation === 'find' ? 'Finding Codex…' : 'Find Codex'}</Button>
      {discovery && <div ref={foundPanel} className="provider-discovery" tabIndex={-1} role="region" aria-label="Codex search results">
        <p role="status">{discovery.candidates.length === 0 ? 'No executable was found in the checked locations. Enter a path to review Codex.' : discovery.candidates.length === 1 ? 'One executable found. Review its source below before you approve it.' : `${discovery.candidates.length} executables found. Choose one to review.`}</p>
        {discovery.truncated && <p className="provider-help">The search reached its limit. You can enter a different path below.</p>}
        {discovery.candidates.length > 1 && <form className="provider-discovery-form" onSubmit={event => { event.preventDefault(); void inspect(); }}>
          <Field disabled={busy} render={<fieldset disabled={busy} />}><legend id="discovered-codex-label">Codex executable</legend><RadioGroup name="discovered-codex" value={path} disabled={busy} aria-labelledby="discovered-codex-label" onValueChange={value => { if (typeof value === 'string' && discovery.candidates.some(found => found.path === value)) choose(value); }}>{discovery.candidates.map((found, index) => <FieldItem key={found.path}><FieldLabel className="backup-selection provider-discovery-choice"><Radio nativeButton render={<button type="button" />} id={`discovered-codex-${index}`} value={found.path} disabled={busy} /><span className="provider-path"><bdi>{found.path}</bdi></span></FieldLabel></FieldItem>)}</RadioGroup></Field>
          <Button className="secondary-button" variant="outline" type="submit" disabled={busy || !path}>{operation === 'inspect' ? 'Inspecting executable…' : 'Review selected executable'}</Button>
        </form>}
      </div>}
      <details className="provider-manual" open={manualOpen} onToggle={event => setManualOpen(event.currentTarget.open)}>
        <summary data-provider-manual>Enter executable path</summary>
        <Field className="unlock-form provider-inspection-form" render={<form noValidate onSubmit={event => { event.preventDefault(); void inspect(); }} />}>
          <FieldLabel htmlFor="provider-executable">Codex executable</FieldLabel>
          <Input ref={pathField} id="provider-executable" type="text" autoComplete="off" spellCheck={false} maxLength={4096} value={path} disabled={busy} aria-invalid={!!error && pathError} aria-describedby={`provider-executable-help${error && pathError ? ' provider-executable-error' : ''}`} onChange={event => choose(event.target.value)} />
          <FieldDescription id="provider-executable-help" render={<p />}>Enter an absolute file path. Inspection checks the source before you approve execution.</FieldDescription>
          <Button className="secondary-button" variant="outline" type="submit" disabled={busy}>{operation === 'inspect' ? 'Inspecting executable…' : 'Inspect executable'}</Button>
        </Field>
      </details>
      {candidate && <div ref={reviewPanel} className="backup-receipt provider-executable-review" tabIndex={-1} role="region" aria-labelledby="provider-review-heading">
        <h3 id="provider-review-heading">Review executable</h3>
        <dl className="backup-inspection-metadata"><dt>File</dt><dd className="provider-path"><bdi>{candidate.path}</bdi></dd><dt>Source</dt><dd>{candidate.sourceKind === 'official_artifact' ? 'Reviewed official artifact' : candidate.sourceKind === 'test_fixture' ? 'Reviewed synthetic provider fixture' : 'Not verified'}</dd>{candidate.sourceUrl && <><dt>Source reference</dt><dd><bdi>{candidate.sourceUrl}</bdi></dd></>}<dt>Supported Codex version</dt><dd>{candidate.testedVersion}</dd></dl>
        <details className="provider-advanced"><summary>Executable details</summary><dl className="backup-inspection-metadata"><dt>Size</dt><dd>{candidate.fileBytes} bytes</dd><dt>SHA-256</dt><dd className="provider-checksum">{candidate.sha256}</dd></dl></details>
        <p id="provider-trust-help" className="provider-help">{candidate.sourceVerified
          ? 'The checksum matches a reviewed source. Approve this file only if you trust it. Each profile uses its own Codex home.'
          : 'This file does not match a supported verified source. OpenUsage cannot approve it for execution.'}</p>
        <form className="provider-approval-form" onSubmit={event => { event.preventDefault(); void approve(); }}>
          <Field disabled={busy || !candidate.sourceVerified}><FieldLabel className="backup-selection"><Checkbox nativeButton render={<button type="button" />} id="provider-executable-approved" required disabled={busy || !candidate.sourceVerified} checked={confirmed} aria-describedby="provider-trust-help" onCheckedChange={checked => setConfirmed(checked === true)} />I trust this executable for account checks and previews.</FieldLabel></Field>
          <Button className="primary-button" type="submit" disabled={busy || !confirmed || !candidate.sourceVerified}>{operation === 'approve' ? 'Approving executable…' : 'Approve executable'}</Button>
        </form>
      </div>}
      {error && <p ref={errorPanel} id="provider-executable-error" className="operation-error" role="alert" tabIndex={-1}>{error}</p>}
    </details>
  </section>;
}

export function ProfileProviderCheck({ profileId, profileLabel, approval, busy, managementActions, managementTrigger, onActivitySaved, onActivityFailed, onQuotaSaved, onQuotaFailed }: { profileId: string; profileLabel: string; approval: ProviderExecutableApproval; busy: boolean; managementActions: ReactNode; managementTrigger: ReactNode; onActivitySaved: (value: ProfileActivitySavedRead) => Promise<void>; onActivityFailed: () => Promise<void>; onQuotaSaved: (value: ProfileQuotaSavedRead) => Promise<void>; onQuotaFailed: () => Promise<void> }) {
  const [checking, setChecking] = useState(false);
  const [loggingIn, setLoggingIn] = useState(false);
  const [readingQuota, setReadingQuota] = useState(false);
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

  const activity = useActivityPreview({ profileId, profileLabel, approval, busy: busy || checking || loggingIn || readingQuota, onSaved: onActivitySaved, onFailed: onActivityFailed, onStarted: () => {
    checkGeneration.current += 1; setDiagnostic(null); setError(null); setQuotaGeneration(value => value + 1); setLoginGeneration(value => value + 1);
  } });

  async function check() {
    if (busy || loggingIn || readingQuota || activity.working || pending.current) return;
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
    <ProfileDeviceLogin key={`login:${loginGeneration}`} profileId={profileId} profileLabel={profileLabel} approval={approval} busy={busy || checking || readingQuota || activity.working} onActivity={setLoggingIn} onStarted={() => { activity.clear(); checkGeneration.current += 1; setDiagnostic(null); setError(null); setQuotaGeneration(value => value + 1); }} />
    <ProfileQuotaRead onSaved={onQuotaSaved} onFailed={onQuotaFailed} key={`quota:${quotaGeneration}`} profileId={profileId} profileLabel={profileLabel} approval={approval} busy={busy || checking || loggingIn || activity.working} onActivity={setReadingQuota} onStarted={() => { activity.clear(); checkGeneration.current += 1; setDiagnostic(null); setError(null); setLoginGeneration(value => value + 1); }} actionSiblings={<>
      {managementTrigger}
      <CollapsiblePanel keepMounted className="profile-management">
        {managementActions}
        {activity.action}
        <Button className="text-button" variant="ghost" type="button" data-provider-check={profileId} aria-describedby={`profile-name-${profileId}`} disabled={busy || checking || loggingIn || readingQuota || activity.working} onClick={() => { void check(); }}>Check connection</Button>
      </CollapsiblePanel>
    </>} />
    {activity.content}
    {(diagnostic || error) && <div ref={result} className="provider-diagnostic" data-provider-profile={profileId} tabIndex={-1} role={error ? 'alert' : 'status'}>
      {error ? <p>{error}</p> : diagnostic && <>{diagnostic.sourceKind === 'test_fixture' && <p className="provider-fixture-note" data-provider-fixture>Synthetic provider fixture</p>}<p><strong>{title}</strong></p><p>{diagnostic.authentication === 'authentication_required'
        ? 'Codex reported no authorized account in this profile. Choose Sign in to connect this profile.'
        : diagnostic.authentication === 'chatgpt' ? 'Identity has not been verified. This check does not collect usage readings.'
        : 'This check supports ChatGPT account connections.'}</p><p className="provider-help">Codex {diagnostic.providerVersion} · Monitoring stopped</p></>}
    </div>}
  </div>;
}
