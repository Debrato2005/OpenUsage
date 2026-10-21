import { useEffect, useRef, useState } from 'react';
import { Button } from './components/ui/button';
import { Checkbox } from './components/ui/checkbox';
import { Collapsible, CollapsiblePanel, CollapsibleTrigger } from './components/ui/collapsible';
import { Field, FieldLabel } from './components/ui/field';
import { Select, SelectItem, SelectPopup, SelectTrigger, SelectValue } from './components/ui/select';
import { invoke, type ProfileRecord, type ReadingExportFormat, type ReadingExportReceipt } from './contracts';

type Props = { profiles: ProfileRecord[]; busy: boolean; setBusy: (busy: boolean) => void; quotaHistory?: boolean };

function exportFailure(error: unknown, history: boolean): string {
  const messages: Record<string, string> = {
    workspace_locked: 'Unlock your workspace before you export readings.',
    invalid_arguments: 'Choose a profile and confirm the unencrypted export, then try again.',
    invalid_export_selection: history ? 'This profile is no longer available. Choose another profile, then try again.' : 'Choose between one and ten active profiles, then try again.',
    insufficient_space: 'Free space in your Downloads folder, then try again.',
    export_too_large: history ? 'This history exceeds the 50,000-row or 64 MiB file limit. No partial file was saved.' : 'This export exceeds the file limit. Choose fewer profiles, then try again.',
    export_unsafe_text: 'A field could be treated as a spreadsheet formula. Omit names and email, or save as JSON.',
    export_unavailable: 'OpenUsage could not save the export. Check your Downloads folder and free space, then try again.',
  };
  return typeof error === 'string' && messages[error] ? messages[error]
    : 'OpenUsage could not confirm the saved export. Check Downloads/OpenUsage before you try again.';
}

export function DataExport({ profiles, busy, setBusy, quotaHistory = false }: Props) {
  const active = quotaHistory ? profiles.slice(0, 1) : profiles.filter(profile => !profile.archived);
  const prefix = quotaHistory ? 'quota-history-export' : 'reading-export';
  const title = quotaHistory ? 'Export quota history' : 'Export saved readings';
  const Heading = quotaHistory ? 'h4' : 'h2';
  const profileKey = active.map(profile => profile.id).join(',');
  const [excluded, setExcluded] = useState<string[]>([]);
  const selected = active.filter(profile => !excluded.includes(profile.id));
  const [format, setFormat] = useState<ReadingExportFormat>('csv');
  const [identifiers, setIdentifiers] = useState(false);
  const [confirmed, setConfirmed] = useState(false);
  const [saving, setSaving] = useState(false);
  const [receipt, setReceipt] = useState<ReadingExportReceipt | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [focus, setFocus] = useState(false);
  const result = useRef<HTMLDivElement>(null);
  const failure = useRef<HTMLParagraphElement>(null);
  const pending = useRef(false);
  const generation = useRef(0);

  useEffect(() => {
    generation.current += 1;
    return () => { generation.current += 1; };
  }, []);
  useEffect(() => { setConfirmed(false); }, [profileKey]);
  useEffect(() => {
    if (busy || !focus) return;
    const target = receipt ? result.current : failure.current;
    if (target && !target.closest('[hidden], [inert]')) target.focus();
    setFocus(false);
  }, [busy, focus, receipt, error]);

  function changeSelection() {
    setConfirmed(false);
    setReceipt(null);
    setError(null);
  }
  async function save() {
    if (busy || pending.current || !confirmed || selected.length < 1 || selected.length > 10) return;
    const epoch = generation.current;
    const profileIds = selected.map(profile => profile.id);
    pending.current = true;
    setSaving(true);
    setBusy(true);
    setReceipt(null);
    setError(null);
    setConfirmed(false);
    try {
      const saved = quotaHistory
        ? await invoke('export_quota_history', { profileId: profileIds[0]!, format, includeIdentifiers: identifiers, confirmed: true })
        : await invoke('export_saved_readings', { profileIds, format, includeIdentifiers: identifiers, confirmed: true });
      if (generation.current !== epoch) return;
      if (saved.format !== format || saved.profileCount !== profileIds.length || saved.identifiersIncluded !== identifiers) {
        throw new Error('The export receipt does not match the selected options.');
      }
      setReceipt(saved);
      setFocus(true);
    } catch (error: unknown) {
      if (generation.current !== epoch) return;
      setError(exportFailure(error, quotaHistory));
      setFocus(true);
    } finally {
      if (generation.current === epoch) {
        pending.current = false;
        setSaving(false);
        setBusy(false);
      }
    }
  }

  return <section className="backup-panel" aria-label={title} data-reading-export>
    <Collapsible>
      <Heading><CollapsibleTrigger render={<Button variant="outline" />} type="button" disabled={busy} data-export-toggle>{title}</CollapsibleTrigger></Heading>
      <CollapsiblePanel keepMounted>
        <form className="unlock-form" aria-label={quotaHistory ? 'Quota history export' : 'Saved readings export'} aria-busy={saving} onSubmit={event => { event.preventDefault(); void save(); }}>
          <p>{quotaHistory ? 'Export every saved quota snapshot for this profile, including earlier connections. This includes all history pages, with no aggregation.' : 'Export the latest saved quota and activity summaries and daily buckets for selected active profiles. This does not include reading history or all application records.'}</p>
          {!quotaHistory && <p>Profiles stay separate, with no aggregation. Missing metrics remain unavailable. Use an encrypted backup to keep application records.</p>}
          <p id={`${prefix}-privacy`}>{quotaHistory ? 'Exports are not encrypted. Activity may identify an account even without its name. Email and credentials are never included.' : 'Exports are not encrypted. Activity may identify an account even when names and email are omitted.'}</p>
          {active.length === 0 && <p>No active profiles are available. Add an account profile before you export readings.</p>}
          {!quotaHistory && <fieldset disabled={busy || active.length === 0}>
            <legend>Profiles to export</legend>
            {active.map(profile => <Field key={profile.id} disabled={busy}>
              <FieldLabel className="backup-selection">
                <Checkbox nativeButton render={<button type="button" />} data-export-profile={profile.id} checked={!excluded.includes(profile.id)} disabled={busy} onCheckedChange={checked => {
                  setExcluded(current => checked === true ? current.filter(id => id !== profile.id) : [...current, profile.id]);
                  changeSelection();
                }} />{profile.label}
              </FieldLabel>
            </Field>)}
          </fieldset>}
          <Field disabled={busy || active.length === 0}>
            <FieldLabel nativeLabel={false} render={<div id={`${prefix}-format-label`} />}>File format</FieldLabel>
            <Select modal={false} value={format} disabled={busy || active.length === 0} items={{ csv: 'CSV', json: 'JSON' }} onValueChange={next => {
              if (next === 'csv' || next === 'json') { setFormat(next); changeSelection(); }
            }}>
              <SelectTrigger id={`${prefix}-format`} aria-labelledby={`${prefix}-format-label`}><SelectValue /></SelectTrigger>
              <SelectPopup alignItemWithTrigger={false}><SelectItem value="csv">CSV</SelectItem><SelectItem value="json">JSON</SelectItem></SelectPopup>
            </Select>
          </Field>
          <Field disabled={busy || active.length === 0}>
            <FieldLabel className="backup-selection">
              <Checkbox nativeButton render={<button type="button" />} id={`${prefix}-identifiers`} checked={identifiers} disabled={busy || active.length === 0} aria-describedby={`${prefix}-privacy`} onCheckedChange={checked => { setIdentifiers(checked === true); changeSelection(); }} />{quotaHistory ? 'Include profile name and local references' : 'Include profile names and email addresses'}
            </FieldLabel>
          </Field>
          <Field disabled={busy || selected.length === 0}>
            <FieldLabel className="backup-selection">
              <Checkbox nativeButton render={<button type="button" />} id={`${prefix}-consent`} checked={confirmed} disabled={busy || selected.length === 0} aria-describedby={`${prefix}-privacy`} onCheckedChange={checked => setConfirmed(checked === true)} />I understand this file contains unencrypted activity.
            </FieldLabel>
          </Field>
          <Button className="primary-button" type="submit" data-export-save disabled={busy || !confirmed || selected.length === 0 || selected.length > 10}>{saving ? 'Saving readings…' : 'Save readings'}</Button>
          <p>Save one file in Downloads/OpenUsage.</p>
        </form>
      </CollapsiblePanel>
    </Collapsible>
    {error && <p ref={failure} className="operation-error" role="alert" tabIndex={-1} data-export-error>{error}</p>}
    {receipt && <div ref={result} className="backup-receipt" role="status" tabIndex={-1} data-export-result>
      <p>{quotaHistory ? 'Quota history exported. This file is not encrypted.' : 'Saved readings exported. This file is not encrypted.'}</p>
      <p><strong>{receipt.fileName}</strong><br />{receipt.directory}</p>
      <p>{receipt.format.toUpperCase()} · {receipt.profileCount} profiles · {receipt.rowCount} rows</p>
      <p>{quotaHistory ? (receipt.identifiersIncluded ? 'Profile name and local references included. Email omitted.' : 'Profile name, email and local identifiers omitted.') : (receipt.identifiersIncluded ? 'Profile names and email addresses included.' : 'Profile names and email addresses omitted.')}</p>
      <p>Exported at <time dateTime={receipt.exportedAt}>{receipt.exportedAt}</time></p>
    </div>}
  </section>;
}
