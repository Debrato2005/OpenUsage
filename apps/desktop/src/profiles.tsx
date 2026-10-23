import { useEffect, useRef, useState } from 'react';
import type { AccountActionRequest } from './account-actions';
import { Button } from './components/ui/button';
import { Input } from './components/ui/input';
import { Field, FieldLabel } from './components/ui/field';
import { Collapsible, CollapsibleTrigger, CollapsiblePanel } from './components/ui/collapsible';
import './profile-actions.css';
import './profile-layout.css';
import { invoke, type MonitoringStatus, type ProfileRecord, type ProfileView, type ProviderExecutableApproval } from './contracts';
import { ProfileProviderCheck } from './provider';
import { SavedProfileQuota } from './quota';
import { ProfileActivity } from './activity';
import { ProfileMonitoringControls, type MonitoringAction } from './monitoring';
import { AccountSignout } from './account-signout';

type Props = {
  busy: boolean;
  setBusy: (busy: boolean) => void;
  onCount: (count: number) => void;
  providerApproval: ProviderExecutableApproval | null;
  onProfilesChange?: (profiles: ProfileRecord[]) => void;
  onReadingsChanged?: () => void;
  monitoring: MonitoringStatus | null;
  monitoringUnavailable: boolean;
  onMonitoringAction: MonitoringAction;
  onConnectAccount?: () => void;
  requestedAction?: AccountActionRequest | null;
  onActionHandled?: (sequence: number) => void;
  embedded?: boolean;
  selectedProfileId?: string | null;
  onSelectProfile?: (profileId: string | null) => void;
  onAccountOperation?: (event: { profileId: string; outcome: 'saved' | 'failed' }) => void;
  onRefreshState?: (profileId: string, working: boolean, waitSeconds: number) => void;
};

type Failure = {
  message: string;
  target: 'load' | 'create' | 'rename' | 'row';
  id: string | null;
  invalidLabel: boolean;
};

type FocusTarget = 'create' | 'rename' | 'error'
  | { id: string; action: 'options' | 'rename' | 'restore' };


function invalidLabel(value: string): boolean {
  if (new TextEncoder().encode(value).length > 1024 || /\p{Cc}/u.test(value)) return true;
  const length = Array.from(value.trim()).length;
  return length < 1 || length > 100;
}

function errorMessage(error: unknown): string {
  const messages: Record<string, string> = {
    invalid_profile_label: 'Use 1 to 100 characters without control characters.',
    invalid_profile_id: 'OpenUsage could not select this profile. Reload profiles and try again.',
    profile_capacity_reached: 'All 10 active profile slots are in use. Archive an active profile to free a slot.',
    profile_not_found: 'This profile no longer exists. Reload profiles and try again.',
    workspace_locked: 'Unlock your workspace before you manage profiles.',
    profile_unavailable: 'OpenUsage could not complete this profile request. Keep your workspace files, then try again.',
  };
  return typeof error === 'string' && messages[error]
    ? messages[error] : 'OpenUsage could not complete this profile request. Reload profiles and try again.';
}

export function ProfileManager({ busy, setBusy, onCount, providerApproval, onProfilesChange, onReadingsChanged, monitoring, monitoringUnavailable, onMonitoringAction, onConnectAccount, requestedAction, onActionHandled, embedded = false, selectedProfileId = null, onSelectProfile, onAccountOperation, onRefreshState }: Props) {
  const focused = embedded && selectedProfileId !== null;
  const [view, setView] = useState<ProfileView | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [attempt, setAttempt] = useState(0);
  const [label, setLabel] = useState('');
  const [createOpen, setCreateOpen] = useState(false);
  const [archivedOpen, setArchivedOpen] = useState(false);
  const [readings, setReadings] = useState<Record<string, boolean>>({});
  const [reconnectProfile, setReconnectProfile] = useState<string | null>(null);
  const [managed, setManaged] = useState<string | null>(null);
  const [providerWorking, setProviderWorking] = useState<Record<string, boolean>>({});
  const [providerRevision, setProviderRevision] = useState<Record<string, number>>({});
  const [editing, setEditing] = useState<{ id: string; label: string } | null>(null);
  const [failure, setFailure] = useState<Failure | null>(null);
  const [status, setStatus] = useState<string | null>(null);
  const [focusTarget, setFocusTarget] = useState<FocusTarget | null>(null);
  const section = useRef<HTMLElement>(null);
  const createInput = useRef<HTMLInputElement>(null);
  const createToggle = useRef<HTMLButtonElement>(null);
  const renameInput = useRef<HTMLInputElement>(null);
  const pending = useRef(false);
  const viewRequest = useRef(0);
  const mounted = useRef(true);
  const handledAction = useRef(0);
  const [focusIntent, setFocusIntent] = useState(0);
  const connectingProfile = useRef<string | null>(null);
  const startRequestedProfile = useRef<string | null>(null);
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; viewRequest.current += 1; }; }, []);
  const reportCount = useRef(onCount);
  const reportProfiles = useRef(onProfilesChange);
  const reportReading = useRef(onReadingsChanged);
  const reportSelection = useRef(onSelectProfile);
  const reportOperation = useRef(onAccountOperation);
  const reportRefresh = useRef(onRefreshState);

  useEffect(() => { reportCount.current = onCount; }, [onCount]);
  useEffect(() => { reportProfiles.current = onProfilesChange; }, [onProfilesChange]);
  useEffect(() => { reportReading.current = onReadingsChanged; }, [onReadingsChanged]);
  useEffect(() => { reportSelection.current = onSelectProfile; }, [onSelectProfile]);
  useEffect(() => { reportOperation.current = onAccountOperation; }, [onAccountOperation]);
  useEffect(() => { reportRefresh.current = onRefreshState; }, [onRefreshState]);
  useEffect(() => {
    if (!providerApproval || busy || !connectingProfile.current) return;
    let attempts = 0;
    let frame: number;
    const focusReady = () => {
      const profileId = connectingProfile.current;
      if (!profileId) return;
      const button = section.current?.querySelector<HTMLButtonElement>(`[data-login-start="${CSS.escape(profileId)}"]`);
      if (button?.getClientRects().length && !button.closest('[hidden]')) {
        button.focus();
        const start = startRequestedProfile.current === profileId;
        connectingProfile.current = null;
        startRequestedProfile.current = null;
        if (start) button.click();
      } else if (++attempts < 2) frame = requestAnimationFrame(focusReady);
    };
    // The row slot mounts separately from its provider controls.
    frame = requestAnimationFrame(focusReady);
    return () => cancelAnimationFrame(frame);
  }, [providerApproval, busy, view, focusIntent, selectedProfileId]);

  useEffect(() => {
    let cancelled = false;
    const request = ++viewRequest.current;
    setLoading(true);
    setFailure(null);
    setStatus(null);
    void invoke('list_profiles').then(next => {
      if (!cancelled && request === viewRequest.current) {
        setView(next);
        reportCount.current(next.activeProfiles);
        reportProfiles.current?.(next.profiles);
        setStatus(`Profiles loaded. ${next.activeProfiles} of 10 active.`);
        if (attempt > 0) setFocusTarget('create');
      }
    }).catch((error: unknown) => {
      if (!cancelled && request === viewRequest.current) {
        setFailure({ message: errorMessage(error), target: 'load', id: null, invalidLabel: false });
        if (attempt > 0) setFocusTarget('error');
      }
    }).finally(() => { if (!cancelled && request === viewRequest.current) setLoading(false); });
    return () => { cancelled = true; };
  }, [attempt]);

  useEffect(() => {
    if (busy || loading || !focusTarget) return;
    const target = typeof focusTarget === 'object'
      ? section.current?.querySelector<HTMLButtonElement>(`[data-profile-id="${CSS.escape(focusTarget.id)}"] [data-profile-action="${focusTarget.action}"]`)
      : focusTarget === 'create' ? createInput.current
      : focusTarget === 'rename' ? renameInput.current
      : section.current?.querySelector<HTMLElement>('#profiles-error');
    if (target?.getClientRects().length && !target.closest('[hidden]')) target.focus();
    else if (createToggle.current?.getClientRects().length && !createToggle.current.closest('[hidden]')) createToggle.current.focus();
    setFocusTarget(null);
  }, [busy, loading, focusTarget, view, editing, failure, createOpen, readings]);

  async function change(
    operation: () => Promise<ProfileView>,
    context: Pick<Failure, 'target' | 'id'>,
    done: (next: ProfileView) => void,
  ) {
    if (busy || loading || pending.current) return;
    pending.current = true;
    setReconnectProfile(null);
    viewRequest.current += 1;
    setSaving(true);
    setBusy(true);
    setFailure(null);
    setStatus(null);
    try {
      const next = await operation();
      if (!mounted.current) return;
      setView(next);
      reportCount.current(next.activeProfiles);
      reportProfiles.current?.(next.profiles);
      done(next);
    } catch (error: unknown) {
      if (!mounted.current) return;
      const fieldError = error === 'invalid_profile_label';
      setFailure({ ...context, message: errorMessage(error), invalidLabel: fieldError });
      setFocusTarget(fieldError && context.target === 'create' ? 'create'
        : fieldError && context.target === 'rename' ? 'rename' : 'error');
    } finally {
      pending.current = false;
      setSaving(false);
      setBusy(false);
    }
  }

  async function refreshConnection(profileId: string) {
    if (pending.current) throw new Error('A profile change is still in progress.');
    const request = ++viewRequest.current;
    const next = await invoke('list_profiles');
    if (!mounted.current || request !== viewRequest.current) throw new Error('The connection status request was replaced.');
    if (!next.profiles.some(profile => profile.id === profileId && !profile.archived)) throw new Error('The selected profile is no longer available.');
    setView(next);
    reportCount.current(next.activeProfiles);
    reportProfiles.current?.(next.profiles);
  }

  function checkLabel(value: string, target: 'create' | 'rename', id: string | null): boolean {
    if (!invalidLabel(value)) return true;
    setFailure({ message: errorMessage('invalid_profile_label'), target, id, invalidLabel: true });
    setFocusTarget(target);
    return false;
  }

  function create() {
    if (!checkLabel(label, 'create', null)) return;
    void change(() => invoke('create_profile', { label: label.trim() }), { target: 'create', id: null }, next => {
      setLabel('');
      setStatus(`Profile created. ${next.activeProfiles} of 10 active.`);
      setFocusTarget('create');
    });
  }

  function addAccount() {
    if (!view) return;
    const previousIds = new Set(view.profiles.map(profile => profile.id));
    const names = new Set(view.profiles.map(profile => profile.label));
    const name = Array.from('ABCDEFGHIJ', suffix => `Account ${suffix}`).find(value => !names.has(value)) ?? `Account ${view.profiles.length + 1}`;
    void change(() => invoke('create_profile', { label: name }), { target: 'create', id: null }, next => {
      const added = next.profiles.filter(profile => !previousIds.has(profile.id));
      setCreateOpen(false);
      setStatus('Account added. Sign in to connect it.');
      if (added.length === 1) {
        reportSelection.current?.(added[0]!.id);
        connectingProfile.current = added[0]!.id;
      }
      if (!providerApproval) onConnectAccount?.();
    });
  }

  function reconnect(profile: ProfileRecord) {
    if (busy || loading || profile.archived) return;
    setReadings(current => ({ ...current, [profile.id]: true }));
    setReconnectProfile(profile.id);
    if (!providerApproval) onConnectAccount?.();
  }

  useEffect(() => {
    if (!requestedAction || handledAction.current === requestedAction.sequence || busy || loading || !view) return;
    handledAction.current = requestedAction.sequence;
    onActionHandled?.(requestedAction.sequence);
    if (requestedAction.kind === 'add') { addAccount(); return; }
    const profile = view.profiles.find(item => item.id === requestedAction.profileId && (!item.archived || requestedAction.kind === 'manage' || requestedAction.kind === 'usage'));
    if (!profile) return;
    reportSelection.current?.(profile.id);
    if (requestedAction.kind === 'signin') {
      connectingProfile.current = profile.id;
      startRequestedProfile.current = profile.id;
      setFocusIntent(value => value + 1);
      if (!providerApproval) onConnectAccount?.();
    } else if (requestedAction.kind === 'refresh') reconnect(profile);
    else {
      setReadings(current => ({ ...current, [profile.id]: true }));
      if (requestedAction.kind === 'manage') setManaged(profile.id);
      setFocusTarget({ id: profile.id, action: 'options' });
    }
  }, [requestedAction, busy, loading, view, providerApproval, onActionHandled]);

  function rename() {
    if (!editing || !checkLabel(editing.label, 'rename', editing.id)) return;
    const current = editing;
    void change(() => invoke('rename_profile', { id: current.id, label: current.label.trim() }), { target: 'rename', id: current.id }, next => {
      setEditing(null);
      setStatus(`Profile renamed. ${next.activeProfiles} of 10 active.`);
      setManaged(null);
      setFocusTarget({ id: current.id, action: next.profiles.find(profile => profile.id === current.id)?.archived ? 'rename' : 'options' });
    });
  }

  function archive(profile: ProfileRecord) {
    const active = view?.profiles.filter(item => !item.archived) ?? [];
    const index = active.findIndex(item => item.id === profile.id);
    void change(() => invoke('set_profile_archived', { id: profile.id, archived: !profile.archived }), { target: 'row', id: profile.id }, next => {
      setStatus(`Profile ${profile.archived ? 'restored' : 'archived'}. ${next.activeProfiles} of 10 active.`);
      setManaged(null);
      if (profile.archived) {
        if (focused) reportSelection.current?.(profile.id);
        setReadings(current => ({ ...current, [profile.id]: true }));
        setFocusTarget({ id: profile.id, action: 'options' });
      }
      else {
        const remaining = next.profiles.filter(item => !item.archived);
        const adjacent = remaining[Math.min(Math.max(index, 0), remaining.length - 1)];
        if (focused) reportSelection.current?.(adjacent?.id ?? null);
        if (adjacent) setReadings(current => ({ ...current, [adjacent.id]: true }));
        setFocusTarget(adjacent ? { id: adjacent.id, action: 'options' } : 'create');
      }
    });
  }

  function error() {
    return failure && <p id="profiles-error" className="operation-error profile-error" role="alert" tabIndex={-1}>{failure.message}</p>;
  }

  function startSignout(profileId: string) {
    viewRequest.current += 1;
    if (connectingProfile.current === profileId) connectingProfile.current = null;
    if (startRequestedProfile.current === profileId) startRequestedProfile.current = null;
    setReconnectProfile(value => value === profileId ? null : value);
    setFailure(null); setStatus(null);
  }

  function signedOut(profileId: string, next: ProfileView) {
    if (!mounted.current) return;
    viewRequest.current += 1;
    setView(next); reportCount.current(next.activeProfiles); reportProfiles.current?.(next.profiles);
    reportReading.current?.(); reportRefresh.current?.(profileId, false, 0);
    setProviderRevision(value => ({ ...value, [profileId]: (value[profileId] ?? 0) + 1 }));
    setManaged(null); setStatus('Signed out. Saved readings and history remain available.');
    if (!next.profiles.find(profile => profile.id === profileId)?.archived) {
      connectingProfile.current = profileId;
      setFocusIntent(value => value + 1);
    }
  }

  function row(profile: ProfileRecord) {
    const monitoringStatus = monitoring?.profiles.find(value => value.profileId === profile.id) ?? null;
    const current = editing?.id === profile.id;
    const rowHidden = focused ? selectedProfileId !== profile.id : embedded && profile.archived && !archivedOpen;
    const rowError = failure?.id === profile.id;
    const nameId = `profile-name-${profile.id}`;
    const loginContainerId = `profile-sign-in-${profile.id}`;
    const needsSignIn = profile.connectionState !== 'connected';
    const connection = {
      connected: 'Connected', authentication_required: 'Not connected', unauthorized: 'Session expired',
      offline: 'Offline', unsupported: 'Unsupported account', error: 'Connection error',
    }[profile.connectionState];
    const managementActions = current ? <Field className="profile-rename-form" render={<form onSubmit={event => { event.preventDefault(); rename(); }} />}>
      <FieldLabel htmlFor={`profile-label-${profile.id}`}>New profile label</FieldLabel>
      <Input ref={renameInput} id={`profile-label-${profile.id}`} name="profile-label" autoComplete="off" maxLength={1024} disabled={busy || loading} value={editing.label} aria-invalid={rowError && !!failure?.invalidLabel} aria-describedby={rowError ? 'profiles-error' : undefined} onChange={event => { setEditing({ id: profile.id, label: event.target.value }); setFailure(null); }} />
      <div className="profile-actions"><Button className="primary-button" type="submit" disabled={busy || loading}>Save</Button><Button className="secondary-button" variant="outline" type="button" disabled={busy || loading} onClick={() => { setEditing(null); setManaged(null); setFailure(null); setFocusTarget({ id: profile.id, action: profile.archived ? 'rename' : 'options' }); }}>Cancel rename</Button></div>
      {rowError && error()}
    </Field> : <div className="profile-actions">
      <Button className="text-button" variant="ghost" type="button" data-profile-action="rename" aria-label="Rename profile" aria-describedby={nameId} disabled={busy || loading} onClick={() => { setManaged(profile.id); setEditing({ id: profile.id, label: profile.label }); setFailure(null); setFocusTarget('rename'); }}>Rename</Button>
      <Button className="text-button" variant="ghost" type="button" data-profile-action={profile.archived ? 'restore' : 'archive'} aria-label={profile.archived ? 'Restore profile' : 'Archive profile'} aria-describedby={nameId} disabled={busy || loading} onClick={() => archive(profile)}>{profile.archived ? 'Restore' : 'Archive'}</Button>
      {profile.connectionState === 'connected' && providerApproval && !rowHidden && (focused || !!readings[profile.id]) && (profile.archived || managed === profile.id) && <AccountSignout key={`${profile.id}:${providerApproval.approvalId}`} profile={profile} approval={providerApproval} busy={busy || loading || !!providerWorking[profile.id] || !!monitoringStatus?.readingActive} setBusy={setBusy} onStarted={() => startSignout(profile.id)} onSignedOut={next => signedOut(profile.id, next)} onRechecked={next => { if (mounted.current) { viewRequest.current += 1; setView(next); reportCount.current(next.activeProfiles); reportProfiles.current?.(next.profiles); reportReading.current?.(); } }} />}
    </div>;
    const managementTrigger = <CollapsibleTrigger render={<Button variant="ghost" className="text-button" />} type="button" data-profile-action="options" aria-describedby={nameId} disabled={busy || loading || current}>Manage profile</CollapsibleTrigger>;
    return <Collapsible render={<li />} className="profile-row" id={`managed-profile-${profile.id}`} data-profile-id={profile.id} key={profile.id} hidden={rowHidden} inert={rowHidden} open={focused && selectedProfileId === profile.id || !!readings[profile.id]} onOpenChange={open => setReadings(previous => ({ ...previous, [profile.id]: open }))}>
      <div className="profile-row-summary">
        <div className="profile-details"><bdi id={nameId} className="profile-label">{profile.label}</bdi><p className="profile-meta">{profile.environment === 'native' ? 'On this device' : 'WSL profile'}</p></div>
        <span className="profile-connection-state">{profile.archived ? 'Archived' : connection}</span>
        <div id={`profile-quota-summary-${profile.id}`} />
        <div className="profile-row-actions profile-actions" hidden={focused}>
          {!profile.archived && !providerApproval && <Button type="button" variant={needsSignIn ? 'default' : 'ghost'} className={needsSignIn ? 'primary-button' : 'text-button'} data-profile-connect={profile.id} aria-describedby={nameId} disabled={busy || loading || !!monitoringStatus?.readingActive} onClick={() => { if (needsSignIn) { connectingProfile.current = profile.id; startRequestedProfile.current = profile.id; onConnectAccount?.(); } else reconnect(profile); }}>{needsSignIn ? profile.connectionState === 'unauthorized' ? 'Reconnect' : 'Sign in' : 'Refresh'}</Button>}
        <CollapsibleTrigger render={<Button variant="ghost" className="text-button" />} type="button" data-profile-action="readings" aria-describedby={nameId}>{readings[profile.id] ? 'Hide usage' : 'View usage'}</CollapsibleTrigger>
        </div>
      </div>
      {!profile.archived && <div id={loginContainerId} className="account-sign-in" aria-label={`Sign-in for ${profile.label}`} />}
      <CollapsiblePanel keepMounted className="profile-reading-panel">
      <Collapsible className="profile-management-root" open={managed === profile.id || current} onOpenChange={open => setManaged(open ? profile.id : null)}>
      <ProfileActivity profileId={profile.id} profileLabel={profile.label} readingRevision={monitoring?.readingRevision}>{(onActivitySaved, onActivityFailed) => <SavedProfileQuota summaryContainerId={`profile-quota-summary-${profile.id}`} profileId={profile.id} profileLabel={profile.label} readingRevision={monitoring?.readingRevision}>{(onQuotaSaved, onQuotaFailed) => profile.archived ? managementActions : providerApproval
        ? <ProfileProviderCheck onRefreshState={(working, waitSeconds) => { setProviderWorking(value => value[profile.id] === working ? value : { ...value, [profile.id]: working }); reportRefresh.current?.(profile.id, working, waitSeconds); }} reconnectRequested={reconnectProfile === profile.id} onReconnectRequested={() => setReconnectProfile(null)} loginContainerId={loginContainerId} connected={!needsSignIn} loginActionLabel={profile.connectionState === 'unauthorized' ? 'Reconnect' : 'Sign in'} monitoring={monitoringStatus} monitoringUnavailable={monitoringUnavailable} onMonitoringAction={onMonitoringAction} onConnectionChanged={() => refreshConnection(profile.id)} key={`${profile.id}:${providerApproval.approvalId}:${providerRevision[profile.id] ?? 0}`} profileId={profile.id} profileLabel={profile.label} approval={providerApproval} busy={busy || loading || current || !!monitoringStatus?.readingActive} managementActions={managementActions} managementTrigger={managementTrigger} onActivitySaved={async value => { await Promise.all([onActivitySaved(value), onQuotaFailed()]); reportReading.current?.(); reportOperation.current?.({ profileId: profile.id, outcome: 'saved' }); }} onActivityFailed={async () => { await onActivityFailed(); reportReading.current?.(); reportOperation.current?.({ profileId: profile.id, outcome: 'failed' }); }} onQuotaSaved={async value => { await Promise.all([onQuotaSaved(value), onActivityFailed()]); reportReading.current?.(); reportOperation.current?.({ profileId: profile.id, outcome: 'saved' }); }} onQuotaFailed={async () => { await onQuotaFailed(); reportReading.current?.(); reportOperation.current?.({ profileId: profile.id, outcome: 'failed' }); }} onCollectionSaved={async value => {
          await Promise.all([value.quota ? onQuotaSaved({ profileId: profile.id, ...value.quota }, false) : onQuotaFailed(), value.activity ? onActivitySaved({ profileId: profile.id, ...value.activity }, false) : onActivityFailed()]);
          reportReading.current?.();
          reportOperation.current?.({ profileId: profile.id, outcome: value.accepted ? 'saved' : 'failed' });
        }} onCollectionFailed={async () => { await Promise.all([onQuotaFailed(), onActivityFailed()]); reportReading.current?.(); reportOperation.current?.({ profileId: profile.id, outcome: 'failed' }); }} />
        : <>{managementTrigger}<CollapsiblePanel keepMounted className="profile-management">{managementActions}<ProfileMonitoringControls profileId={profile.id} status={monitoringStatus} unavailable={monitoringUnavailable} approval={null} busy={busy || loading || current} onAction={onMonitoringAction} /></CollapsiblePanel></>}</SavedProfileQuota>}</ProfileActivity>
      {rowError && !current && error()}
      </Collapsible>
      </CollapsiblePanel>
    </Collapsible>;
  }

  const active = view?.profiles.filter(profile => !profile.archived) ?? [];
  const archived = view?.profiles.filter(profile => profile.archived) ?? [];
  return <section ref={section} className="accounts-section" data-embedded={focused || undefined} aria-labelledby={focused ? `profile-name-${selectedProfileId}` : 'accounts-heading'} aria-busy={busy || loading}>
    <div className="section-heading" hidden={focused}><h2 id="accounts-heading" className="sr-only">Accounts</h2><div className="profile-heading-actions"><span className="muted-tag">{view ? `${view.activeProfiles} of 10 active` : loading ? 'Loading profiles…' : 'Profiles unavailable'}</span><Button type="button" className="primary-button" data-account-connect disabled={busy || loading || !view || view.activeProfiles >= 10} onClick={addAccount}>Add OpenAI account</Button><Button ref={createToggle} type="button" variant="ghost" className="text-button" data-profile-add aria-expanded={createOpen} aria-controls="profile-add-panel" disabled={busy || loading || !view} onClick={() => { setCreateOpen(open => !open); if (!createOpen) setFocusTarget('create'); }}>Choose a name</Button></div></div>
    <p id="profile-capacity" hidden={focused} className={createOpen ? 'profile-capacity' : 'sr-only'}>Up to 10 profiles can be active. Archiving frees a slot and keeps local data.</p>
    <p className="profile-status" role="status" hidden={focused && !saving && !failure}>{loading ? 'Loading profiles…' : saving ? 'Saving profile changes…' : status}</p>
    {failure?.target === 'load' && error()}
    {failure?.target === 'create' && !createOpen && error()}
    {failure && !failure.invalidLabel && <Button className="text-button" variant="ghost" type="button" disabled={busy || loading} onClick={() => { setEditing(null); setAttempt(value => value + 1); }}>Reload profiles</Button>}
    {view && <>
      <Collapsible open={createOpen} onOpenChange={setCreateOpen} className="profile-add-disclosure" hidden={focused}>
      <CollapsiblePanel keepMounted id="profile-add-panel">
      <Field className="profile-create-form" render={<form onSubmit={event => { event.preventDefault(); create(); }} />}>
        <FieldLabel htmlFor="profile-label">Profile label</FieldLabel>
        <Input ref={createInput} id="profile-label" name="profile-label" autoComplete="off" maxLength={1024} disabled={busy || loading} value={label} aria-invalid={failure?.target === 'create' && failure.invalidLabel} aria-describedby={`profile-capacity${failure?.target === 'create' ? ' profiles-error' : ''}`} onChange={event => { setLabel(event.target.value); setFailure(null); }} />
        <Button className="primary-button" type="submit" disabled={busy || loading}>Create profile</Button>
        <Button type="button" variant="ghost" className="text-button" disabled={busy || loading} onClick={() => { setCreateOpen(false); createToggle.current?.focus(); }}>Cancel add account</Button>
        {failure?.target === 'create' && error()}
      </Field>
      </CollapsiblePanel>
      </Collapsible>
      {embedded ? <>
        {!focused && <div className="profile-table-heading" aria-hidden="true"><span>Account</span><span>Connection</span><span>Quota</span><span>Usage</span></div>}
        <details className="archived-profiles" hidden={focused || archived.length === 0} open={archivedOpen} onToggle={event => setArchivedOpen(event.currentTarget.open)}>
          <summary aria-controls={archived.map(profile => `managed-profile-${profile.id}`).join(' ')}>Archived profiles ({archived.length})</summary>
          <p>Archived profiles keep their local data.</p>
        </details>
        <ul className="profile-list" aria-label={focused ? 'Selected profile' : 'Profiles'}>{view.profiles.map(row)}</ul>
        {!view.profiles.length && <p className="profile-empty">No profiles. Add an OpenAI account to get started.</p>}
      </> : <>
        {active.length ? <><div className="profile-table-heading" aria-hidden="true"><span>Account</span><span>Connection</span><span>Quota</span><span>Usage</span></div><ul className="profile-list" aria-label="Active profiles">{active.map(row)}</ul></> : <p className="profile-empty">No active profiles. Add an account or restore an archived one.</p>}
        {archived.length > 0 && <details className="archived-profiles"><summary>Archived profiles ({archived.length})</summary><p>Archived profiles keep their local data.</p><ul className="profile-list" aria-label="Archived profiles">{archived.map(row)}</ul></details>}
      </>}
    </>}
  </section>;
}
