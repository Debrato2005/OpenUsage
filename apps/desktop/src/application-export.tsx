import { useEffect, useRef, useState } from 'react';
import { Button } from './components/ui/button';
import { Checkbox } from './components/ui/checkbox';
import { Collapsible, CollapsiblePanel, CollapsibleTrigger } from './components/ui/collapsible';
import { Field, FieldLabel } from './components/ui/field';
import { invoke, type ApplicationExportReceipt } from './contracts';

type Props = { busy: boolean; setBusy: (busy: boolean) => void };

function exportFailure(error: unknown): string {
  const messages: Record<string, string> = {
    workspace_locked: 'Unlock your workspace before you export application data.',
    export_unavailable: 'OpenUsage could not save the export. Check your Downloads folder and free space, then try again.',
    export_too_large: 'This export exceeded the 5 GiB size limit or two-minute time limit. No partial file was saved.',
    insufficient_space: 'Free space in your Downloads folder, then try again.',
  };
  return typeof error === 'string' && messages[error] ? messages[error]
    : 'OpenUsage could not confirm the saved export. Check Downloads/OpenUsage before you try again.';
}

export function ApplicationExport({ busy, setBusy }: Props) {
  const [confirmed, setConfirmed] = useState(false);
  const [saving, setSaving] = useState(false);
  const [receipt, setReceipt] = useState<ApplicationExportReceipt | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [focus, setFocus] = useState(false);
  const result = useRef<HTMLDivElement>(null);
  const failure = useRef<HTMLParagraphElement>(null);
  const pending = useRef(false);
  const generation = useRef(0);
  const ownsBusy = useRef(false);

  useEffect(() => {
    generation.current += 1;
    return () => {
      generation.current += 1;
      if (ownsBusy.current) { ownsBusy.current = false; setBusy(false); }
    };
  }, [setBusy]);
  useEffect(() => {
    if (busy || !focus) return;
    const target = receipt ? result.current : failure.current;
    if (target && !target.closest('[hidden], [inert]')) target.focus();
    setFocus(false);
  }, [busy, focus, receipt, error]);

  async function save() {
    if (busy || pending.current || !confirmed) return;
    const epoch = generation.current;
    pending.current = true; ownsBusy.current = true;
    setSaving(true); setBusy(true); setReceipt(null); setError(null); setConfirmed(false);
    try {
      const saved = await invoke('export_application_data', { confirmed: true });
      if (generation.current !== epoch) return;
      setReceipt(saved); setFocus(true);
    } catch (error: unknown) {
      if (generation.current !== epoch) return;
      setError(exportFailure(error)); setFocus(true);
    } finally {
      if (generation.current === epoch) {
        pending.current = false; setSaving(false);
        if (ownsBusy.current) { ownsBusy.current = false; setBusy(false); }
      }
    }
  }

  return <section className="backup-panel" aria-label="Export all application data" data-application-export style={{ minWidth: 0, overflowWrap: 'anywhere' }}>
    <Collapsible onOpenChange={() => setConfirmed(false)}>
      <h2><CollapsibleTrigger render={<Button variant="outline" style={{ whiteSpace: 'normal', overflowWrap: 'anywhere' }} />} type="button" disabled={busy} data-application-export-toggle>Export all application data</CollapsibleTrigger></h2>
      <CollapsiblePanel keepMounted>
        <form className="unlock-form" aria-label="Application data export" aria-busy={saving} onSubmit={event => { event.preventDefault(); void save(); }}>
          <p>Save all application records as JSON, including archived accounts, names, email addresses, saved readings and history, settings, alerts and local references.</p>
          <p id="application-export-privacy">This file is not encrypted. Credentials, encryption keys and provider home files are excluded. Use an encrypted backup to recover your workspace.</p>
          <Field disabled={busy}><FieldLabel className="backup-selection"><Checkbox nativeButton render={<button type="button" />} id="application-export-consent" data-application-export-consent checked={confirmed} disabled={busy} aria-describedby="application-export-privacy" onCheckedChange={checked => setConfirmed(checked === true)} />I understand this file contains unencrypted account and activity data.</FieldLabel></Field>
          <Button className="primary-button" type="submit" disabled={busy || !confirmed} data-application-export-save style={{ whiteSpace: 'normal', overflowWrap: 'anywhere' }}>{saving ? 'Saving application data…' : 'Save application data'}</Button>
          <p>Save one JSON file in Downloads/OpenUsage.</p>
        </form>
      </CollapsiblePanel>
    </Collapsible>
    {error && <p ref={failure} className="operation-error" role="alert" tabIndex={-1} data-application-export-error>{error}</p>}
    {receipt && <div ref={result} className="backup-receipt" role="status" tabIndex={-1} data-application-export-result>
      <p>Application data exported. This file is not encrypted.</p>
      <p><strong>{receipt.fileName}</strong><br />{receipt.directory}</p>
      <p>{receipt.recordCount} records · JSON</p>
      <p>Exported at <time dateTime={receipt.exportedAt}>{receipt.exportedAt}</time></p>
      <details><summary>Export details</summary><p>Schema version {receipt.schemaVersion} · {receipt.tableCount} tables · {receipt.byteCount} bytes</p></details>
    </div>}
  </section>;
}
