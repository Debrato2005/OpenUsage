import { useEffect, useRef, useState } from 'react';
import { Button } from './components/ui/button';
import { invoke, type EvidenceLocation, type EvidenceMethod, type ProfileReadingEvidence, type SavedProfileActivity } from './contracts';

type Reading = Pick<SavedProfileActivity, 'profileId' | 'collectionRunId' | 'providerVersion' | 'sourceKind' | 'observedAtUtc'>;
type Loaded = { key: string; kind: 'loaded'; value: ProfileReadingEvidence | null };
type State = Loaded | { key: string; kind: 'loading' } | { key: string; kind: 'failed'; reason: 'changed' | 'unavailable' };
const locations: Record<EvidenceLocation, string> = {
  activity_result: 'Activity response', activity_summary: 'Activity summary', activity_daily_bucket: 'Daily bucket',
  quota_result: 'Quota response', quota_limit: 'Quota limit', quota_window: 'Quota window',
  quota_individual_limit: 'Individual limit', quota_reset_credits: 'Reset credits', quota_credit_detail: 'Credit detail',
};
const checks = { rpc_envelope: 'Envelope', request_id: 'Request', response_fields: 'Fields' };

export function ReadingEvidenceDetails({ reading, method }: { reading: Reading; method: EvidenceMethod }) {
  const [open, setOpen] = useState(false);
  const [attempt, setAttempt] = useState(0);
  const [state, setState] = useState<State | null>(null);
  const cached = useRef<Loaded | null>(null);
  const key = JSON.stringify([reading.profileId, method, reading.collectionRunId, reading.providerVersion, reading.sourceKind, reading.observedAtUtc]);
  useEffect(() => {
    if (!open) return;
    if (cached.current?.key === key) { setState(cached.current); return; }
    let current = true;
    setState({ key, kind: 'loading' });
    invoke('load_profile_reading_evidence', { profileId: reading.profileId, runId: reading.collectionRunId, method }).then(value => {
      if (!current) return;
      if (value?.availability === 'complete' && (value.record.providerVersion !== reading.providerVersion
        || value.record.sourceKind !== reading.sourceKind || value.record.observedAtUtc !== reading.observedAtUtc)) {
        setState({ key, kind: 'failed', reason: 'changed' }); return;
      }
      const result: Loaded = { key, kind: 'loaded', value };
      cached.current = result; setState(result);
    }).catch(() => { if (current) setState({ key, kind: 'failed', reason: 'unavailable' }); });
    return () => { current = false; };
  }, [open, key, attempt]);
  const active = state?.key === key ? state : null;
  const view = active?.kind === 'loaded' ? active.value : null;
  const record = view?.availability === 'complete' ? view.record : null;
  return <>
    <dt>Evidence</dt><dd><Button variant="ghost" aria-label={open ? 'Hide evidence' : 'Show evidence'} aria-expanded={open} onClick={() => setOpen(value => !value)}>{open ? 'Hide' : 'Show'}</Button></dd>
    {open && (!active || active.kind === 'loading') && <><dt>Evidence status</dt><dd role="status">Loading…</dd></>}
    {open && active?.kind === 'failed' && <>
      <dt>Evidence status</dt><dd><span role="alert">Not available</span>{' '}<Button variant="ghost" aria-label="Retry evidence" onClick={() => setAttempt(value => value + 1)}>Retry</Button></dd>
      <dt>Reason</dt><dd>{active.reason === 'changed' ? 'Reading changed' : 'Read failed'}</dd>
    </>}
    {open && active?.kind === 'loaded' && !record && <>
      <dt>Evidence status</dt><dd>Not available</dd>
      {view?.availability === 'legacy_gap' && <><dt>Reason</dt><dd>Historical reading</dd></>}
    </>}
    {open && record && <>
      <dt>Reading status</dt><dd>Official</dd>
      <dt>Source level</dt><dd>Official</dd>
      <dt>Evidence ID</dt><dd><code>{record.runId}</code></dd>
      <dt>Adapter</dt><dd><code>{record.adapterId}</code></dd>
      <dt>Adapter revision</dt><dd>{record.adapterRevision}</dd>
      <dt>Projection version</dt><dd>{record.projectionVersion}</dd>
      <dt>Format fingerprint</dt><dd><code>{record.formatFingerprint}</code></dd>
      <dt>Response hash</dt><dd><code>{record.redactedResponseHash}</code></dd>
      <dt>Checks</dt><dd>{record.checkResults.map(check => <div key={check.id}>{checks[check.id]}: Passed</div>)}</dd>
      <dt>Comparison</dt><dd>Not performed</dd>
      <dt>Unused fields</dt><dd>{record.unusedFields.length ? record.unusedFields.map(field => <div key={field.location}>{locations[field.location]}: {field.fieldCount}</div>) : 'None'}</dd>
    </>}
  </>;
}
