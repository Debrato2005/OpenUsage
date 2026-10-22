import { useEffect, useId, useRef, useState } from 'react';
import { Button } from './components/ui/button';
import { invoke, type ProfileRecord, type ProfileView, type ProviderExecutableApproval } from './contracts';
import { providerFailure } from './provider-errors';

type Props = {
  profile: ProfileRecord; approval: ProviderExecutableApproval; busy: boolean;
  setBusy: (value: boolean) => void; onStarted: () => void; onSignedOut: (value: ProfileView) => void; onRechecked: (value: ProfileView) => void;
};

function failureMessage(error: unknown) {
  if (error === 'provider_sign_out_save_failed') return 'Signed out, but OpenUsage could not save the account status. Reopen account details before signing in again.';
  return providerFailure(error);
}

export function AccountSignout({ profile, approval, busy, setBusy, onStarted, onSignedOut, onRechecked }: Props) {
  const id = useId();
  const [open, setOpen] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const dialog = useRef<HTMLDialogElement>(null);
  const cancel = useRef<HTMLButtonElement>(null);
  const failure = useRef<HTMLParagraphElement>(null);
  const generation = useRef(0);
  const pending = useRef(false);
  const ownsBusy = useRef(false);

  useEffect(() => {
    generation.current += 1;
    return () => { generation.current += 1; if (ownsBusy.current) { ownsBusy.current = false; setBusy(false); } };
  }, [profile.id, approval.approvalId, setBusy]);
  useEffect(() => {
    const node = dialog.current;
    if (!node) return;
    if (open && !node.open) { node.showModal(); cancel.current?.focus(); }
    else if (!open && node.open) { node.close(); trigger.current?.focus({ preventScroll: true }); }
  }, [open]);
  useEffect(() => { if (error && !saving) failure.current?.focus(); }, [error, saving]);

  function dismiss() {
    if (pending.current) return;
    setOpen(false); setError(null);
  }
  async function signOut() {
    if (!open || busy || pending.current || profile.connectionState !== 'connected') return;
    const epoch = generation.current;
    pending.current = true; ownsBusy.current = true;
    setBusy(true); setSaving(true); setError(null); onStarted();
    let next: ProfileView | null = null;
    try {
      const value = await invoke('sign_out_profile', { profileId: profile.id, approvalId: approval.approvalId, confirmed: true });
      if (generation.current !== epoch) return;
      const signedOut = value.profiles.find(item => item.id === profile.id);
      if (!signedOut || signedOut.connectionState !== 'authentication_required' || signedOut.archived !== profile.archived) throw new Error('Invalid sign-out result');
      next = value;
    } catch (error: unknown) {
      if (generation.current !== epoch) return;
      if (error === 'provider_sign_out_save_failed') {
        try {
          const latest = await invoke('list_profiles');
          if (generation.current === epoch) onRechecked(latest);
        } catch { /* The explicit save error remains the recovery instruction. */ }
      }
      if (generation.current === epoch) setError(failureMessage(error));
    } finally {
      if (generation.current === epoch) {
        pending.current = false; setSaving(false);
        if (ownsBusy.current) { ownsBusy.current = false; setBusy(false); }
      }
    }
    if (generation.current !== epoch || !next) return;
    setOpen(false); onSignedOut(next);
  }

  return <>
    <Button ref={trigger} variant="ghost" className="text-button" disabled={busy} data-account-signout-open={profile.id} onClick={() => { setError(null); setOpen(true); }}>Sign out</Button>
    <dialog ref={dialog} role="alertdialog" aria-modal="true" aria-labelledby={`${id}-title`} aria-describedby={`${id}-scope`} data-account-signout-dialog={profile.id} onCancel={event => { event.preventDefault(); event.stopPropagation(); dismiss(); }} onClose={event => { event.stopPropagation(); dismiss(); }} style={{ position: 'fixed', inset: 0, width: 'min(30rem, calc(100vw - 2rem))', maxHeight: 'calc(100dvh - 2rem)', margin: 'auto', padding: 'var(--space-5)', border: '1px solid var(--line)', borderRadius: 'var(--radius-panel)', background: 'var(--surface)', color: 'var(--text)', font: 'inherit', overflow: 'auto', overflowWrap: 'anywhere' }}>
      <h2 id={`${id}-title`} style={{ fontSize: 'var(--text-section)' }}>Sign out of {profile.label}?</h2>
      <p id={`${id}-scope`}>OpenAI may revoke this app session. Saved readings and history will remain on this device.</p>
      <div className="profile-actions" aria-busy={saving}>
        <Button ref={cancel} variant="outline" disabled={saving} data-account-signout-cancel onClick={dismiss}>Cancel</Button>
        <Button variant="destructive" disabled={busy} data-account-signout-submit onClick={() => { void signOut(); }}>{saving ? 'Signing out…' : 'Sign out'}</Button>
      </div>
      {error && <p ref={failure} className="operation-error" role="alert" tabIndex={-1} data-account-signout-error>{error}</p>}
    </dialog>
  </>;
}
