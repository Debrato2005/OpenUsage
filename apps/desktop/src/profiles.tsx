import { useEffect, useRef, useState } from 'react';
import { invoke, type ProfileRecord, type ProfileView, type ProviderExecutableApproval } from './contracts';
import { ProfileProviderCheck } from './provider';

type Props = {
  busy: boolean;
  setBusy: (busy: boolean) => void;
  onCount: (count: number) => void;
  providerApproval: ProviderExecutableApproval | null;
};

type Failure = {
  message: string;
  target: 'load' | 'create' | 'rename' | 'row';
  id: string | null;
  invalidLabel: boolean;
};

type FocusTarget = 'create' | 'rename' | 'error'
  | { id: string; action: 'rename' | 'archive' | 'restore' };


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

export function ProfileManager({ busy, setBusy, onCount, providerApproval }: Props) {
  const [view, setView] = useState<ProfileView | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [attempt, setAttempt] = useState(0);
  const [label, setLabel] = useState('');
  const [editing, setEditing] = useState<{ id: string; label: string } | null>(null);
  const [failure, setFailure] = useState<Failure | null>(null);
  const [status, setStatus] = useState<string | null>(null);
  const [focusTarget, setFocusTarget] = useState<FocusTarget | null>(null);
  const section = useRef<HTMLElement>(null);
  const createInput = useRef<HTMLInputElement>(null);
  const renameInput = useRef<HTMLInputElement>(null);
  const pending = useRef(false);
  const reportCount = useRef(onCount);

  useEffect(() => { reportCount.current = onCount; }, [onCount]);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setFailure(null);
    setStatus(null);
    void invoke('list_profiles').then(next => {
      if (!cancelled) {
        setView(next);
        reportCount.current(next.activeProfiles);
        setStatus(`Profiles loaded. ${next.activeProfiles} of 10 active.`);
        if (attempt > 0) setFocusTarget('create');
      }
    }).catch((error: unknown) => {
      if (!cancelled) {
        setFailure({ message: errorMessage(error), target: 'load', id: null, invalidLabel: false });
        if (attempt > 0) setFocusTarget('error');
      }
    }).finally(() => { if (!cancelled) setLoading(false); });
    return () => { cancelled = true; };
  }, [attempt]);

  useEffect(() => {
    if (busy || loading || !focusTarget) return;
    const target = typeof focusTarget === 'object'
      ? section.current?.querySelector<HTMLButtonElement>(`[data-profile-id="${CSS.escape(focusTarget.id)}"] [data-profile-action="${focusTarget.action}"]`)
      : focusTarget === 'create' ? createInput.current
      : focusTarget === 'rename' ? renameInput.current
      : section.current?.querySelector<HTMLElement>('#profiles-error');
    if (target?.getClientRects().length) target.focus();
    else createInput.current?.focus();
    setFocusTarget(null);
  }, [busy, loading, focusTarget, view, editing, failure]);

  async function change(
    operation: () => Promise<ProfileView>,
    context: Pick<Failure, 'target' | 'id'>,
    done: (next: ProfileView) => void,
  ) {
    if (busy || loading || pending.current) return;
    pending.current = true;
    setSaving(true);
    setBusy(true);
    setFailure(null);
    setStatus(null);
    try {
      const next = await operation();
      setView(next);
      reportCount.current(next.activeProfiles);
      done(next);
    } catch (error: unknown) {
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

  function rename() {
    if (!editing || !checkLabel(editing.label, 'rename', editing.id)) return;
    const current = editing;
    void change(() => invoke('rename_profile', { id: current.id, label: current.label.trim() }), { target: 'rename', id: current.id }, next => {
      setEditing(null);
      setStatus(`Profile renamed. ${next.activeProfiles} of 10 active.`);
      setFocusTarget({ id: current.id, action: 'rename' });
    });
  }

  function archive(profile: ProfileRecord) {
    const active = view?.profiles.filter(item => !item.archived) ?? [];
    const index = active.findIndex(item => item.id === profile.id);
    void change(() => invoke('set_profile_archived', { id: profile.id, archived: !profile.archived }), { target: 'row', id: profile.id }, next => {
      setStatus(`Profile ${profile.archived ? 'restored' : 'archived'}. ${next.activeProfiles} of 10 active.`);
      if (profile.archived) setFocusTarget({ id: profile.id, action: 'rename' });
      else {
        const remaining = next.profiles.filter(item => !item.archived);
        const adjacent = remaining[Math.min(Math.max(index, 0), remaining.length - 1)];
        setFocusTarget(adjacent ? { id: adjacent.id, action: 'archive' } : 'create');
      }
    });
  }

  function error() {
    return failure && <p id="profiles-error" className="operation-error profile-error" role="alert" tabIndex={-1}>{failure.message}</p>;
  }

  function row(profile: ProfileRecord) {
    const current = editing?.id === profile.id;
    const rowError = failure?.id === profile.id;
    const nameId = `profile-name-${profile.id}`;
    return <li className="profile-row" data-profile-id={profile.id} key={profile.id}>
      <div className="profile-details"><bdi id={nameId} className="profile-label">{profile.label}</bdi><p className="profile-meta">{profile.environment === 'native' ? 'On this device' : 'WSL profile'}</p></div>
      {current ? <form className="profile-rename-form" onSubmit={event => { event.preventDefault(); rename(); }}>
        <label htmlFor={`profile-label-${profile.id}`}>New profile label</label>
        <input ref={renameInput} id={`profile-label-${profile.id}`} name="profile-label" autoComplete="off" maxLength={1024} disabled={busy || loading} value={editing.label} aria-invalid={rowError && !!failure?.invalidLabel} aria-describedby={rowError ? 'profiles-error' : undefined} onChange={event => { setEditing({ id: profile.id, label: event.target.value }); setFailure(null); }} />
        <div className="profile-actions"><button className="primary-button" type="submit" disabled={busy || loading}>Save</button><button className="secondary-button" type="button" disabled={busy || loading} onClick={() => { setEditing(null); setFailure(null); setFocusTarget({ id: profile.id, action: 'rename' }); }}>Cancel rename</button></div>
        {rowError && error()}
      </form> : <div className="profile-actions">
        <button className="secondary-button" type="button" data-profile-action="rename" aria-label="Rename profile" aria-describedby={nameId} disabled={busy || loading} onClick={() => { setEditing({ id: profile.id, label: profile.label }); setFailure(null); setFocusTarget('rename'); }}>Rename</button>
        <button className="secondary-button" type="button" data-profile-action={profile.archived ? 'restore' : 'archive'} aria-label={profile.archived ? 'Restore profile' : 'Archive profile'} aria-describedby={nameId} disabled={busy || loading} onClick={() => archive(profile)}>{profile.archived ? 'Restore' : 'Archive'}</button>
      </div>}
      {!profile.archived && providerApproval && <ProfileProviderCheck key={`${profile.id}:${providerApproval.approvalId}`} profileId={profile.id} profileLabel={profile.label} approval={providerApproval} busy={busy || loading || current} />}
      {rowError && !current && error()}
    </li>;
  }

  const active = view?.profiles.filter(profile => !profile.archived) ?? [];
  const archived = view?.profiles.filter(profile => profile.archived) ?? [];
  return <section ref={section} className="accounts-section" aria-labelledby="accounts-heading" aria-busy={busy || loading}>
    <div className="section-heading"><div><h2 id="accounts-heading">Accounts</h2><p>Each profile uses a separate Codex environment.</p></div><span className="muted-tag">{view ? `${view.activeProfiles} of 10 active` : loading ? 'Loading profiles…' : 'Profiles unavailable'}</span></div>
    <p id="profile-capacity" className="profile-capacity">Up to 10 profiles can be active. Archiving frees a slot and keeps local data.</p>
    <p className="profile-status" role="status">{loading ? 'Loading profiles…' : saving ? 'Saving profile changes…' : status}</p>
    {failure?.target === 'load' && error()}
    {failure && !failure.invalidLabel && <button className="text-button" type="button" disabled={busy || loading} onClick={() => { setEditing(null); setAttempt(value => value + 1); }}>Reload profiles</button>}
    {view && <>
      <form className="profile-create-form" onSubmit={event => { event.preventDefault(); create(); }}>
        <label htmlFor="profile-label">Profile label</label>
        <input ref={createInput} id="profile-label" name="profile-label" autoComplete="off" maxLength={1024} disabled={busy || loading} value={label} aria-invalid={failure?.target === 'create' && failure.invalidLabel} aria-describedby={`profile-capacity${failure?.target === 'create' ? ' profiles-error' : ''}`} onChange={event => { setLabel(event.target.value); setFailure(null); }} />
        <button className="primary-button" type="submit" disabled={busy || loading}>Create profile</button>
        {failure?.target === 'create' && error()}
      </form>
      {active.length ? <ul className="profile-list" aria-label="Active profiles">{active.map(row)}</ul> : <p className="profile-empty">No active profiles. Create a profile or restore an archived one.</p>}
      {archived.length > 0 && <details className="archived-profiles"><summary>Archived profiles ({archived.length})</summary><p>Archived profiles keep their local data.</p><ul className="profile-list" aria-label="Archived profiles">{archived.map(row)}</ul></details>}
      <p className="profile-meta">Sign in to a profile, then preview its quota readings. Scheduled collection is not available yet.</p>
    </>}
  </section>;
}
