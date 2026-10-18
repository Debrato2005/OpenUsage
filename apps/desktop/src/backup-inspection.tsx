import { useEffect, useRef, useState } from 'react';
import { Button } from './components/ui/button';
import { Input } from './components/ui/input';
import { Collapsible, CollapsibleTrigger, CollapsiblePanel } from './components/ui/collapsible';
import { Field, FieldLabel, FieldDescription } from './components/ui/field';
import { Select, SelectTrigger, SelectValue, SelectPopup, SelectItem } from './components/ui/select';
import { invoke, type BackupList, type BackupInspectionResult } from './contracts';
import { BackupRestorePreview } from './backup-restore';

type Props = { busy: boolean; setBusy: (busy: boolean) => void; onRestored: () => Promise<void> };
type Failure = { message: string; password: boolean };
type FocusTarget = 'file' | 'password' | 'error' | 'receipt';

function failureFor(error: unknown): Failure {
  const messages: Record<string, string> = {
    invalid_arguments: 'OpenUsage could not read this backup request. Refresh the list and try again.',
    invalid_passphrase: 'Enter the backup password.',
    passphrase_too_long: 'Use a shorter backup password.',
    workspace_locked: 'Unlock your workspace before you inspect a backup.',
    backup_unavailable: 'OpenUsage could not read this backup. Keep the file, then try again.',
    insufficient_space: 'There is not enough free space to inspect this archive. Free space on the local drive, then try again.',
    backup_not_found: 'That backup is no longer available. Refresh the list and choose a file.',
    invalid_backup: 'OpenUsage could not authenticate this archive. Check the password and file.',
    backup_authentication_failed: 'OpenUsage could not authenticate this archive. Check the password and file.',
  };
  return {
    message: typeof error === 'string' && messages[error]
      ? messages[error] : 'OpenUsage could not inspect this backup. Refresh the list and try again.',
    password: typeof error === 'string'
      && ['invalid_passphrase', 'passphrase_too_long', 'backup_authentication_failed'].includes(error),
  };
}

export function BackupInspection({ busy, setBusy, onRestored }: Props) {
  const [list, setList] = useState<BackupList | null>(null);
  const [loading, setLoading] = useState(true);
  const [attempt, setAttempt] = useState(0);
  const [fileName, setFileName] = useState('');
  const [password, setPassword] = useState('');
  const [inspecting, setInspecting] = useState(false);
  const [failure, setFailure] = useState<Failure | null>(null);
  const [receipt, setReceipt] = useState<BackupInspectionResult | null>(null);
  const [status, setStatus] = useState<string | null>(null);
  const [focusTarget, setFocusTarget] = useState<FocusTarget | null>(null);
  const fileInput = useRef<HTMLButtonElement>(null);
  const passwordInput = useRef<HTMLInputElement>(null);
  const refreshButton = useRef<HTMLButtonElement>(null);
  const errorMessage = useRef<HTMLParagraphElement>(null);
  const result = useRef<HTMLDivElement>(null);
  const pending = useRef(false);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setFailure(null);
    setStatus(null);
    void invoke('list_backups').then(next => {
      if (!cancelled) {
        setList(next);
        setFileName(current => next.backups.some(file => file.fileName === current)
          ? current : next.backups[0]?.fileName ?? '');
        setStatus(attempt > 0 ? 'Backup list refreshed.' : 'Backup list loaded.');
        if (attempt > 0) setFocusTarget('file');
      }
    }).catch((error: unknown) => {
      if (!cancelled) {
        setList(null);
        setFileName('');
        setFailure(failureFor(error));
        setFocusTarget('error');
      }
    }).finally(() => { if (!cancelled) setLoading(false); });
    return () => { cancelled = true; };
  }, [attempt]);

  useEffect(() => {
    if (busy || loading || !focusTarget) return;
    const target = focusTarget === 'file' ? fileInput.current
      : focusTarget === 'password' ? passwordInput.current
      : focusTarget === 'receipt' ? result.current : errorMessage.current;
    if (target && !('disabled' in target && target.disabled)) target.focus();
    else refreshButton.current?.focus();
    setFocusTarget(null);
  }, [busy, loading, focusTarget, receipt, failure]);

  function refresh() {
    if (busy || loading || pending.current) return;
    setPassword('');
    setReceipt(null);
    setAttempt(value => value + 1);
  }

  async function inspect() {
    if (busy || loading || pending.current) return;
    setReceipt(null);
    setStatus(null);
    const entered = password;
    setPassword('');
    if (!fileName) {
      setFailure({ message: 'Choose an available backup file. Refresh the list if needed.', password: false });
      setFocusTarget('file');
      return;
    }
    if (!entered || new TextEncoder().encode(entered).length > 1024) {
      setFailure(failureFor(entered ? 'passphrase_too_long' : 'invalid_passphrase'));
      setFocusTarget('password');
      return;
    }
    pending.current = true;
    setInspecting(true);
    setBusy(true);
    setFailure(null);
    try {
      setReceipt(await invoke('inspect_backup', { fileName, password: entered }));
      setFocusTarget('receipt');
    } catch (error: unknown) {
      const next = failureFor(error);
      setFailure(next);
      setFocusTarget(next.password ? 'password' : 'error');
    } finally {
      pending.current = false;
      setInspecting(false);
      setBusy(false);
    }
  }

  const available = !!list?.backups.length;
  return <div aria-busy={loading || inspecting}>
    <p id="backup-inspection-help">Listed files are not authenticated. Choose a file and enter its backup password.</p>
    <Button ref={refreshButton} variant="ghost" className="text-button" type="button" disabled={busy || loading} onClick={refresh}>Refresh backups</Button>
    <p className="backup-inspection-status" role="status">{loading ? 'Loading backup files…' : inspecting ? 'Authenticating archive…' : status}</p>
    {list?.truncated && <p className="backup-inspection-limit">The backup list reached its limit. Other backup files may be present.</p>}
    {list && !available && <p className="backup-inspection-empty">No available backup archives were found. Create a backup, then refresh this list.</p>}
    {available && <form className="unlock-form backup-inspection-form" noValidate onSubmit={event => { event.preventDefault(); void inspect(); }}>
      <Field className="w-full" disabled={busy || loading}>
        <FieldLabel nativeLabel={false} render={<div />}>Backup file</FieldLabel>
        <Select modal={false} name="backup-file" required disabled={busy || loading} value={fileName} items={list.backups.map(file => ({ value: file.fileName, label: file.fileName }))} onValueChange={next => {
          if (typeof next !== 'string' || !list.backups.some(file => file.fileName === next)) return;
          setFileName(next); setPassword(''); setReceipt(null); setFailure(null); setStatus(null);
        }}>
          <SelectTrigger ref={fileInput} id="backup-file" style={{ width: '100%' }} aria-describedby="backup-inspection-help"><SelectValue /></SelectTrigger>
          <SelectPopup alignItemWithTrigger={false}>
            {list.backups.map(file => <SelectItem key={file.fileName} value={file.fileName} data-option-value={file.fileName}>{file.fileName}</SelectItem>)}
          </SelectPopup>
        </Select>
      </Field>
      <Field className="w-full" disabled={busy || loading} invalid={!!failure?.password}>
        <FieldLabel htmlFor="backup-inspection-password">Backup password</FieldLabel>
        <Input ref={passwordInput} id="backup-inspection-password" name="backup-inspection-password" type="password" autoComplete="current-password" required maxLength={1024} disabled={busy || loading} value={password} aria-invalid={!!failure?.password} aria-describedby={`backup-inspection-password-help${failure?.password ? ' backup-inspection-error' : ''}`} onChange={event => { setPassword(event.target.value); if (failure?.password) setFailure(null); }} />
        <FieldDescription id="backup-inspection-password-help">Enter the password used when this backup was created.</FieldDescription>
      </Field>
      <Button className="primary-button" type="submit" disabled={busy || loading}>{inspecting ? 'Authenticating archive…' : 'Inspect backup'}</Button>
    </form>}
    {failure && <p ref={errorMessage} id="backup-inspection-error" className="operation-error backup-inspection-error" role="alert" tabIndex={-1}>{failure.message}</p>}
    {receipt && <div ref={result} className="backup-receipt backup-inspection-receipt" role="status" tabIndex={-1}>
      <p className={`backup-content-status${receipt.databaseValidation === 'invalid' ? ' operation-error' : ''}`}>
        {receipt.databaseValidation === 'valid' ? 'Backup contents checked. Preview the records before restoring.'
          : receipt.databaseValidation === 'unsupported' ? 'This version of OpenUsage cannot check these records. Restore is unavailable.'
          : 'These records cannot be restored. Keep the backup file.'}
      </p>
      <p>Archive authenticated</p>
      <dl className="backup-inspection-metadata">
        <dt>File</dt><dd><bdi>{receipt.fileName}</bdi></dd>
        {receipt.contentSummary && <>
          <dt>Active profiles</dt><dd>{receipt.contentSummary.activeProfiles}</dd>
          <dt>Archived profiles</dt><dd>{receipt.contentSummary.archivedProfiles}</dd>
          <dt>Accounts</dt><dd>{receipt.contentSummary.accounts}</dd>
        </>}
      </dl>
      <Collapsible>
        <CollapsibleTrigger render={<Button variant="ghost" className="text-button" />} type="button">Details</CollapsibleTrigger>
        <CollapsiblePanel>
          <dl className="backup-inspection-metadata">
            <dt>Archive version</dt><dd>{receipt.archiveVersion}</dd>
            <dt>Schema version</dt><dd>{receipt.manifestSchemaVersion}</dd>
            <dt>Files</dt><dd>{receipt.entryCount}</dd>
            <dt>Total size</dt><dd>{receipt.totalBytes} bytes</dd>
          </dl>
        </CollapsiblePanel>
      </Collapsible>
    </div>}
    {receipt?.databaseValidation === 'valid' && <BackupRestorePreview key={receipt.fileName} fileName={receipt.fileName} busy={busy} setBusy={setBusy} onRestored={onRestored} />}
  </div>;
}
