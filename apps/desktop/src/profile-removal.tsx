import { useEffect, useId, useRef, useState } from 'react';
import { Button } from './components/ui/button';
import { Checkbox } from './components/ui/checkbox';
import { Field, FieldLabel } from './components/ui/field';
import { Radio, RadioGroup } from './components/ui/radio-group';
import { invoke, type ProfileHistoryChoice, type ProfileRecord, type ProfileRemovalPreview, type ProfileView } from './contracts';
import './profile-removal.css';

type Props = {
  profile: ProfileRecord;
  busy: boolean;
  setBusy: (value: boolean) => void;
  onStarted: () => void;
  onRemoved: (view: ProfileView, history: ProfileHistoryChoice) => void;
  onRechecked: (view: ProfileView) => void;
};

function message(error: unknown, preparing: boolean) {
  if (error === 'stale_profile_removal' || error === 'profile_not_found') return 'The profile changed. Review removal again.';
  if (error === 'workspace_locked') return 'Unlock OpenUsage before removing a profile.';
  if (error === 'provider_cleanup_failed' || error === 'provider_shutting_down') return 'Profile work could not stop. Reload profiles before trying again.';
  return preparing ? 'Could not prepare removal. Review again.' : 'The removal result is unavailable. Reload profiles before trying again.';
}

function cancelPreview(preview: ProfileRemovalPreview | null) {
  if (preview) void invoke('cancel_profile_removal', { approvalId: preview.approvalId }).catch(() => undefined);
}

export function ProfileRemoval({ profile, busy, setBusy, onStarted, onRemoved, onRechecked }: Props) {
  const id = useId();
  const [open, setOpen] = useState(false);
  const [history, setHistory] = useState<ProfileHistoryChoice>('archive');
  const [preview, setPreview] = useState<ProfileRemovalPreview | null>(null);
  const [confirmed, setConfirmed] = useState(false);
  const [preparing, setPreparing] = useState(false);
  const [saving, setSaving] = useState(false);
  const [failure, setFailure] = useState<string | null>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const dialog = useRef<HTMLDialogElement>(null);
  const cancel = useRef<HTMLButtonElement>(null);
  const error = useRef<HTMLParagraphElement>(null);
  const approval = useRef<ProfileRemovalPreview | null>(null);
  const generation = useRef(0);
  const pending = useRef(false);
  const ownsBusy = useRef(false);

  useEffect(() => {
    generation.current += 1;
    return () => {
      generation.current += 1;
      cancelPreview(approval.current);
      approval.current = null;
      if (ownsBusy.current) { ownsBusy.current = false; setBusy(false); }
    };
  }, [profile.id, setBusy]);
  useEffect(() => {
    const node = dialog.current;
    if (!node) return;
    if (open && !node.open) { node.showModal(); cancel.current?.focus(); }
    else if (!open && node.open) { node.close(); trigger.current?.focus({ preventScroll: true }); }
  }, [open]);
  useEffect(() => { if (failure && !saving && !preparing) error.current?.focus(); }, [failure, saving, preparing]);

  async function prepare(choice: ProfileHistoryChoice) {
    const epoch = ++generation.current;
    cancelPreview(approval.current);
    approval.current = null;
    setPreview(null); setConfirmed(false); setFailure(null); setPreparing(true);
    try {
      const value = await invoke('prepare_profile_removal', { profileId: profile.id, history: choice });
      if (generation.current !== epoch) { cancelPreview(value); return; }
      approval.current = value;
      setPreview(value);
    } catch (error: unknown) {
      if (generation.current === epoch) setFailure(message(error, true));
    } finally {
      if (generation.current === epoch) setPreparing(false);
    }
  }

  function dismiss() {
    if (pending.current) return;
    generation.current += 1;
    cancelPreview(approval.current);
    approval.current = null;
    setPreview(null); setPreparing(false); setConfirmed(false); setFailure(null); setOpen(false);
  }

  async function apply() {
    const selected = approval.current;
    if (!open || busy || pending.current || preparing || !confirmed || !selected || selected.history !== history) return;
    const epoch = generation.current;
    approval.current = null;
    setPreview(null); setConfirmed(false); setFailure(null);
    pending.current = true; ownsBusy.current = true;
    setSaving(true); setBusy(true); onStarted();
    let next: ProfileView | null = null;
    try {
      const value = await invoke('apply_profile_removal', { profileId: selected.profileId, approvalId: selected.approvalId, history: selected.history, confirmed: true });
      if (generation.current === epoch) next = value;
    } catch (error: unknown) {
      if (generation.current === epoch) setFailure(message(error, false));
    } finally {
      if (generation.current === epoch) {
        pending.current = false; setSaving(false);
        if (ownsBusy.current) { ownsBusy.current = false; setBusy(false); }
      }
    }
    if (generation.current !== epoch || !next) return;
    setOpen(false); onRemoved(next, selected.history);
  }

  async function reload() {
    if (busy || pending.current || preparing) return;
    const epoch = generation.current;
    pending.current = true; ownsBusy.current = true;
    setSaving(true); setBusy(true);
    let next: ProfileView | null = null;
    try {
      const value = await invoke('list_profiles');
      if (generation.current === epoch) next = value;
    } catch (error: unknown) {
      if (generation.current === epoch) setFailure(message(error, false));
    } finally {
      if (generation.current === epoch) {
        pending.current = false; setSaving(false);
        if (ownsBusy.current) { ownsBusy.current = false; setBusy(false); }
      }
    }
    if (generation.current !== epoch || !next) return;
    dismiss(); onRechecked(next);
  }

  return <>
    <Button ref={trigger} variant="ghost" className="text-button" type="button" data-profile-action="remove" aria-label="Remove profile" aria-describedby={`profile-name-${profile.id}`} disabled={busy || profile.environment !== 'native'} onClick={() => {
      const choice = profile.archived ? 'delete' : 'archive';
      setHistory(choice); setOpen(true); void prepare(choice);
    }}>{profile.archived ? 'Delete history' : 'Remove'}</Button>
    <dialog ref={dialog} className="profile-removal-dialog" role="alertdialog" aria-modal="true" aria-labelledby={`${id}-title`} aria-describedby={`${id}-scope`} data-profile-removal-dialog={profile.id} onCancel={event => { event.preventDefault(); event.stopPropagation(); dismiss(); }} onClose={event => { event.stopPropagation(); dismiss(); }}>
      <h2 id={`${id}-title`}>Remove <bdi>{preview?.label ?? profile.label}</bdi>?</h2>
      <RadioGroup aria-label="History choice" name={`${id}-history`} value={history} disabled={saving || preparing} onValueChange={value => {
        if (value !== 'archive' && value !== 'delete' || value === history) return;
        setHistory(value); void prepare(value);
      }}>
        <label><Radio value="archive" disabled={profile.archived} />Archive History</label>
        <label><Radio value="delete" />Delete History</label>
      </RadioGroup>
      <p id={`${id}-scope`}>{history === 'archive'
        ? 'Keeps saved history and tool sign-in. Stops monitoring and frees an active slot.'
        : "Deletes this profile's readings, alerts, preferences, and local subscription records. Tool homes, sign-in, and provider subscriptions stay unchanged. Saved account identity and repeat-alert records remain in exports and backups. Earlier backups can restore deleted history."}</p>
      <Field disabled={saving || preparing || !preview}>
        <FieldLabel className="profile-removal-consent"><Checkbox nativeButton render={<button type="button" />} checked={confirmed} disabled={saving || preparing || !preview} data-profile-removal-consent onCheckedChange={checked => setConfirmed(checked === true)} />{history === 'archive' ? 'I approve archiving this profile.' : "I approve deleting this profile's saved history."}</FieldLabel>
      </Field>
      <div className="profile-actions" aria-busy={saving || preparing}>
        <Button ref={cancel} variant="outline" type="button" disabled={saving} data-profile-removal-cancel onClick={dismiss}>Cancel</Button>
        <Button variant={history === 'delete' ? 'destructive' : 'default'} type="button" disabled={busy || preparing || !preview || !confirmed || !!failure} data-profile-removal-submit onClick={() => { void apply(); }}>{saving ? 'Removing…' : preparing ? 'Preparing…' : history === 'archive' ? 'Archive History' : 'Delete History'}</Button>
      </div>
      {failure && <>
        <p ref={error} className="operation-error" role="alert" tabIndex={-1} data-profile-removal-error>{failure}</p>
        <div className="profile-actions"><Button variant="outline" type="button" disabled={busy || preparing} onClick={() => { void prepare(history); }}>Review again</Button><Button variant="outline" type="button" disabled={busy || preparing} onClick={() => { void reload(); }}>Reload profiles</Button></div>
      </>}
    </dialog>
  </>;
}
