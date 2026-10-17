import { useEffect, useRef, useState } from 'react';
import { invoke, type RestorePreview } from './contracts';

type Props = { fileName: string; busy: boolean; setBusy: (busy: boolean) => void };
type Failure = { message: string; password: boolean };

function failureFor(error: unknown): Failure {
  const messages: Record<string, string> = {
    invalid_arguments: 'OpenUsage could not read this request. Inspect the backup before you prepare a new preview.',
    invalid_passphrase: 'Enter the backup password.',
    passphrase_too_long: 'Use a shorter backup password.',
    workspace_locked: 'Unlock your workspace before you prepare a preview.',
    backup_not_found: 'That backup is no longer available. Refresh the backup list.',
    backup_authentication_failed: 'OpenUsage could not authenticate this archive. Check the password and file.',
    invalid_backup: 'OpenUsage could not read this archive. Keep the backup file and inspect it again.',
    invalid_backup_contents: 'The database checks failed. Keep the backup file and inspect it again.',
    unsupported_backup_contents: 'This version cannot prepare a restore preview for these records.',
    insufficient_space: 'There is not enough free space to prepare this restore. Free space on the local drive, then try again.',
    restore_checkpoint_failed: 'OpenUsage could not save a verified recovery checkpoint. Your current records stay unchanged.',
    stale_restore_preview: 'Your workspace changed while the preview was prepared. Prepare a new preview.',
  };
  return {
    message: typeof error === 'string' && messages[error]
      ? messages[error] : 'OpenUsage could not prepare this preview. Your current records stay unchanged.',
    password: typeof error === 'string'
      && ['invalid_passphrase', 'passphrase_too_long', 'backup_authentication_failed'].includes(error),
  };
}

export function BackupRestorePreview({ fileName, busy, setBusy }: Props) {
  const [open, setOpen] = useState(false);
  const [password, setPassword] = useState('');
  const [preview, setPreview] = useState<RestorePreview | null>(null);
  const [failure, setFailure] = useState<Failure | null>(null);
  const [preparing, setPreparing] = useState(false);
  const [focusTarget, setFocusTarget] = useState<'toggle' | 'password' | 'error' | 'preview' | null>(null);
  const toggle = useRef<HTMLButtonElement>(null);
  const passwordInput = useRef<HTMLInputElement>(null);
  const errorMessage = useRef<HTMLParagraphElement>(null);
  const previewPanel = useRef<HTMLDivElement>(null);
  const pending = useRef(false);
  const alive = useRef(true);
  const retainedPreview = useRef<string | null>(null);

  useEffect(() => {
    alive.current = true;
    return () => {
      alive.current = false;
      if (retainedPreview.current) {
        void invoke('cancel_restore_preview', { previewId: retainedPreview.current }).catch(() => {});
      }
    };
  }, []);

  useEffect(() => {
    if (busy || !focusTarget) return;
    const target = focusTarget === 'toggle' ? toggle.current
      : focusTarget === 'password' ? passwordInput.current
      : focusTarget === 'error' ? errorMessage.current : previewPanel.current;
    target?.focus();
    setFocusTarget(null);
  }, [busy, focusTarget, preview, open, failure]);

  async function prepare() {
    if (busy || pending.current) return;
    const entered = password;
    setPassword('');
    if (!entered || new TextEncoder().encode(entered).length > 1024) {
      setFailure(failureFor(entered ? 'passphrase_too_long' : 'invalid_passphrase'));
      setFocusTarget('password');
      return;
    }
    pending.current = true;
    setBusy(true);
    setPreparing(true);
    setFailure(null);
    try {
      const result = await invoke('prepare_restore_preview', { fileName, password: entered });
      if (!alive.current) {
        await invoke('cancel_restore_preview', { previewId: result.previewId });
        return;
      }
      retainedPreview.current = result.previewId;
      setPreview(result);
      setFocusTarget('preview');
    } catch (error: unknown) {
      if (alive.current) {
        const next = failureFor(error);
        setFailure(next);
        setFocusTarget(next.password ? 'password' : 'error');
      }
    } finally {
      pending.current = false;
      setPreparing(false);
      setBusy(false);
    }
  }

  async function close() {
    if (busy || pending.current) return;
    pending.current = true;
    setBusy(true);
    setFailure(null);
    try {
      if (retainedPreview.current) await invoke('cancel_restore_preview', { previewId: retainedPreview.current });
      retainedPreview.current = null;
      setPreview(null);
      setPassword('');
      setOpen(false);
      setFocusTarget('toggle');
    } catch {
      setFailure({ message: 'OpenUsage could not close this preview. Try again.', password: false });
      setFocusTarget('error');
    } finally {
      pending.current = false;
      setBusy(false);
    }
  }

  return <section className="restore-preparation" aria-label="Restore preview preparation" aria-busy={preparing}>
    {!open && <button ref={toggle} type="button" className="secondary-button" disabled={busy} onClick={() => { setOpen(true); setFailure(null); setFocusTarget('password'); }}>Preview restore</button>}
    {open && !preview && <>
      <h4>Prepare a restore preview</h4>
      <p className="restore-help">OpenUsage will check this backup and save an encrypted checkpoint of your current workspace before showing the preview.</p>
      <form className="unlock-form restore-preview-form" noValidate onSubmit={event => { event.preventDefault(); void prepare(); }}>
        <label htmlFor="restore-preview-password">Backup password</label>
        <input ref={passwordInput} id="restore-preview-password" name="restore-preview-password" type="password" autoComplete="current-password" required maxLength={1024} value={password} disabled={busy} aria-invalid={!!failure?.password} aria-describedby={`restore-preview-password-help${failure?.password ? ' restore-preview-error' : ''}`} onChange={event => { setPassword(event.target.value); if (failure?.password) setFailure(null); }} />
        <p id="restore-preview-password-help">Enter the password used when this backup was created.</p>
        <div className="restore-actions"><button className="primary-button" type="submit" disabled={busy}>{preparing ? 'Preparing preview…' : 'Prepare preview'}</button><button className="text-button" type="button" disabled={busy} onClick={() => { void close(); }}>Cancel</button></div>
      </form>
    </>}
    {preview && <div ref={previewPanel} className="backup-receipt restore-preview" tabIndex={-1} role="region" aria-labelledby="restore-preview-title" data-preview-id={preview.previewId} data-checkpoint-id={preview.checkpoint.id}>
      <h4 id="restore-preview-title">Restore preview</h4>
      <p className="restore-source"><bdi>{preview.fileName}</bdi></p>
      <table className="restore-comparison" role="table"><caption>Records in the saved checkpoint and backup</caption><thead role="rowgroup"><tr role="row"><th scope="col" role="columnheader">Records</th><th scope="col" role="columnheader">Current</th><th scope="col" role="columnheader">Backup</th></tr></thead><tbody role="rowgroup">
        {[
          ['Active profiles', preview.current.activeProfiles, preview.incoming.activeProfiles],
          ['Archived profiles', preview.current.archivedProfiles, preview.incoming.archivedProfiles],
          ['Accounts', preview.current.accounts, preview.incoming.accounts],
        ].map(([label, current, incoming]) => <tr key={label} role="row"><th scope="row" role="rowheader">{label}</th><td role="cell"><span className="comparison-label" aria-hidden="true">Current</span><span data-restore-count>{current}</span></td><td role="cell"><span className="comparison-label" aria-hidden="true">Backup</span><span data-restore-count>{incoming}</span></td></tr>)}
      </tbody></table>
      <p>Recovery checkpoint saved on this device with your workspace key.</p>
      <p className="restore-help">Applying this backup is not available in this build.</p>
      <button className="secondary-button" type="button" disabled={busy} onClick={() => { void close(); }}>Close preview</button>
    </div>}
    {failure && <p ref={errorMessage} id="restore-preview-error" className="operation-error" role="alert" tabIndex={-1}>{failure.message}</p>}
  </section>;
}
