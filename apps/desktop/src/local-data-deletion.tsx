import { useEffect, useId, useRef, useState } from 'react';
import { Button } from './components/ui/button';
import { Checkbox } from './components/ui/checkbox';
import { Field, FieldLabel } from './components/ui/field';
import { invoke, type Bootstrap } from './contracts';

type Props = { busy: boolean; setBusy: (busy: boolean) => void; onChanged: (result: Bootstrap) => void; pending?: boolean };

function failureMessage(error: unknown) {
  if (error === 'local_deletion_unsupported') return 'Local deletion is not available on this operating system yet. OpenUsage did not remove your local data.';
  if (error === 'workspace_locked') return 'Unlock your workspace, then try deleting local data again.';
  if (error === 'key_store_unavailable') return 'Your system credential store is unavailable. Restore access, then try deleting local data again.';
  if (error === 'local_deletion_unsafe') return 'An unexpected file or path prevents deletion. Check OpenUsage diagnostics or contact support before trying again.';
  if (error === 'local_deletion_failed') return 'OpenUsage could not finish deleting local data. Try again, or reopen OpenUsage to finish deletion.';
  return 'Deletion could not be confirmed. Reopen OpenUsage to check its local data before you try again.';
}

export function LocalDataDeletion({ busy, setBusy, onChanged, pending = false }: Props) {
  const id = useId();
  const [open, setOpen] = useState(false);
  const [confirmed, setConfirmed] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const dialog = useRef<HTMLDialogElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const cancel = useRef<HTMLButtonElement>(null);
  const failure = useRef<HTMLParagraphElement>(null);
  const generation = useRef(0);
  const submitting = useRef(false);
  const ownsBusy = useRef(false);
  const title = pending ? 'Finish deleting local data' : 'Delete all local data';

  useEffect(() => {
    generation.current += 1;
    return () => {
      generation.current += 1;
      if (ownsBusy.current) { ownsBusy.current = false; setBusy(false); }
    };
  }, [setBusy]);
  useEffect(() => { setConfirmed(false); }, [pending]);
  useEffect(() => {
    const node = dialog.current;
    if (!node) return;
    if (open && !node.open) { node.showModal(); cancel.current?.focus(); }
    else if (!open && node.open) { node.close(); trigger.current?.focus({ preventScroll: true }); }
  }, [open]);
  useEffect(() => { if (error && !saving) failure.current?.focus(); }, [error, saving]);

  function dismiss() {
    if (submitting.current) return;
    setOpen(false); setConfirmed(false); setError(null);
  }
  async function remove() {
    if (!open || busy || submitting.current || !confirmed) return;
    const epoch = generation.current;
    submitting.current = true; ownsBusy.current = true;
    setBusy(true); setSaving(true); setConfirmed(false); setError(null);
    let updated: Bootstrap | null = null;
    let succeeded = false;
    let message: string | null = null;
    try {
      updated = await invoke('delete_all_local_data', { confirmed: true });
      if (updated.storage !== 'setup_required') { updated = null; throw 'local_deletion_failed'; }
      succeeded = true;
    } catch (error: unknown) {
      if (generation.current !== epoch) return;
      message = failureMessage(error);
      try { updated = await invoke('bootstrap'); }
      catch { message += ' Reopen OpenUsage to check its local state.'; }
    } finally {
      if (generation.current === epoch) {
        submitting.current = false; setSaving(false);
        if (ownsBusy.current) { ownsBusy.current = false; setBusy(false); }
      }
    }
    if (generation.current !== epoch) return;
    if (succeeded) setOpen(false);
    if (updated) onChanged(updated);
    if (message && generation.current === epoch) setError(message);
  }

  return <section className="backup-panel" data-local-data-deletion aria-label="Local data">
    <h2>Local data</h2>
    <p className="provider-help">{pending ? 'Deletion is incomplete. Some local data may remain. Finish deleting it before creating another workspace.' : 'Disconnect accounts and remove data saved by OpenUsage.'}</p>
    <Button ref={trigger} variant="destructive-outline" disabled={busy} data-local-data-deletion-open onClick={() => { setConfirmed(false); setError(null); setOpen(true); }}>{title}</Button>
    <dialog ref={dialog} role="alertdialog" aria-modal="true" aria-labelledby={`${id}-title`} aria-describedby={`${id}-scope ${id}-retained`} data-local-data-deletion-dialog onCancel={event => { event.preventDefault(); dismiss(); }} onClose={dismiss} style={{ position: 'fixed', inset: 0, width: 'min(32rem, calc(100vw - 2rem))', maxHeight: 'calc(100dvh - 2rem)', margin: 'auto', padding: 'var(--space-5)', border: '1px solid var(--line)', borderRadius: 'var(--radius-panel)', background: 'var(--surface)', color: 'var(--text)', font: 'inherit', overflow: 'auto', overflowWrap: 'anywhere' }}>
      <h2 id={`${id}-title`} style={{ fontSize: 'var(--text-section)' }}>{title}</h2>
      <form className="unlock-form" aria-label="Delete local data confirmation" aria-busy={saving} onSubmit={event => { event.preventDefault(); void remove(); }}>
        {pending && <p>Some local data may remain from the previous deletion. Confirm again to finish removing it.</p>}
        <p id={`${id}-scope`}>This removes OpenUsage account sessions, saved readings, alerts, account settings and managed recovery data from this device.</p>
        <p id={`${id}-retained`}>Display size, exported files and external backups remain. This does not delete your OpenAI records or data in other Codex installations.</p>
        <Field disabled={busy}><FieldLabel className="backup-selection"><Checkbox nativeButton render={<button type="button" />} checked={confirmed} disabled={busy} data-local-data-deletion-confirm onCheckedChange={checked => setConfirmed(checked === true)} />Delete my local OpenUsage data and account sessions.</FieldLabel></Field>
        <div className="profile-actions"><Button ref={cancel} variant="outline" type="button" disabled={saving} onClick={dismiss}>Cancel</Button><Button variant="destructive" type="submit" disabled={busy || !confirmed} data-local-data-deletion-submit>{saving ? 'Deleting local data…' : title}</Button></div>
        {error && <p ref={failure} className="operation-error" role="alert" tabIndex={-1} data-local-data-deletion-error>{error}</p>}
      </form>
    </dialog>
  </section>;
}
