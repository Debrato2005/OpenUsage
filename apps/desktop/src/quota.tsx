import { useEffect, useRef, useState } from 'react';
import { invoke, type ProviderExecutableApproval, type ProfileQuotaPreview, type QuotaWindow } from './contracts';
import { providerFailure } from './provider-errors';

type Props = {
  profileId: string;
  profileLabel: string;
  approval: ProviderExecutableApproval;
  busy: boolean;
  onActivity: (active: boolean) => void;
  onStarted: () => void;
};
type Outcome = { kind: 'error' | 'cancelled'; message: string } | { kind: 'preview'; value: ProfileQuotaPreview };
const dateFormat = new Intl.DateTimeFormat('en-US', { timeZone: 'UTC', year: 'numeric', month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit', timeZoneName: 'short' });
function date(value: string) { return dateFormat.format(new Date(value)); }
function WindowValues({ value }: { value: QuotaWindow }) {
  const reset = value.resetsAtUnix === null ? null : new Date(value.resetsAtUnix * 1000).toISOString();
  return <div className="quota-window">
    <h4>{value.label}</h4>
    <dl className="quota-values"><div><dt>Remaining (derived)</dt><dd>{value.derivedRemainingPercent}%</dd></div><div><dt>Used</dt><dd>{value.usedPercent}%</dd></div><div><dt>Expected reset</dt><dd>{reset ? <time dateTime={reset}>{date(reset)}</time> : 'Not available'}</dd></div></dl>
    <meter min={0} max={100} value={value.usedPercent} aria-label={`${value.label}: ${value.usedPercent}% used`} />
  </div>;
}

export function ProfileQuotaRead({ profileId, profileLabel, approval, busy, onActivity, onStarted }: Props) {
  const [working, setWorking] = useState(false);
  const [stopping, setStopping] = useState(false);
  const [requestId, setRequestId] = useState<string | null>(null);
  const [outcome, setOutcome] = useState<Outcome | null>(null);
  const [focus, setFocus] = useState(false);
  const result = useRef<HTMLDivElement>(null);
  const cancelButton = useRef<HTMLButtonElement>(null);
  const alive = useRef(true);
  const pending = useRef(false);
  const stopped = useRef(false);
  const cancellationFailure = useRef<unknown>(null);
  const activeRequest = useRef<string | null>(null);
  const generation = useRef(0);
  useEffect(() => {
    alive.current = true;
    return () => {
      alive.current = false;
      const requestId = activeRequest.current;
      if (requestId) void invoke('cancel_profile_quota_read', { profileId, requestId }).catch(() => {});
    };
  }, [profileId]);
  useEffect(() => { if (working && requestId) cancelButton.current?.focus(); }, [working, requestId]);
  useEffect(() => { if (focus && !working && result.current) { result.current.focus(); setFocus(false); } }, [focus, working, outcome]);

  async function read() {
    if (busy || pending.current) return;
    generation.current += 1;
    pending.current = true;
    stopped.current = false;
    cancellationFailure.current = null;
    setWorking(true); setOutcome(null); onActivity(true); onStarted();
    try {
      const begun = await invoke('begin_profile_quota_read', { profileId, approvalId: approval.approvalId });
      if (begun.profileId !== profileId || begun.approvalId !== approval.approvalId || begun.sourceKind !== approval.sourceKind) throw new Error('Incompatible quota binding.');
      activeRequest.current = begun.requestId;
      if (!alive.current) { await invoke('cancel_profile_quota_read', { profileId, requestId: begun.requestId }); return; }
      setRequestId(begun.requestId);
      const value = await invoke('complete_profile_quota_read', { profileId, requestId: begun.requestId });
      if (value.profileId !== profileId || value.requestId !== begun.requestId || value.approvalId !== approval.approvalId || value.sourceKind !== begun.sourceKind) throw new Error('Incompatible quota result.');
      if (alive.current) setOutcome(cancellationFailure.current ? { kind: 'error', message: providerFailure(cancellationFailure.current) } : stopped.current ? { kind: 'cancelled', message: 'Quota read stopped.' } : { kind: 'preview', value });
    } catch (failure) {
      const cancelled = failure === 'provider_quota_cancelled' || (stopped.current && failure === 'provider_quota_not_found');
      if (alive.current) setOutcome(!cancellationFailure.current && cancelled ? { kind: 'cancelled', message: 'Quota read stopped.' } : { kind: 'error', message: providerFailure(cancellationFailure.current ?? failure) });
    } finally {
      pending.current = false; activeRequest.current = null;
      if (alive.current) { setWorking(false); setStopping(false); setRequestId(null); setFocus(true); onActivity(false); }
    }
  }

  async function cancel() {
    if (!requestId || !pending.current || stopping) return;
    const operation = generation.current;
    stopped.current = true; setStopping(true);
    try { await invoke('cancel_profile_quota_read', { profileId, requestId }); }
    catch (failure) {
      if (alive.current && generation.current === operation) { cancellationFailure.current = failure; setOutcome({ kind: 'error', message: providerFailure(failure) }); setFocus(true); }
    } finally { if (alive.current && generation.current === operation) setStopping(false); }
  }

  return <div className="profile-quota-read">
    <div className="profile-actions"><button type="button" className="secondary-button" data-quota-read={profileId} aria-describedby={`profile-name-${profileId}`} disabled={busy || working} onClick={() => { void read(); }}>{working ? 'Reading quotas…' : 'Read quotas'}</button>{working && requestId && <button ref={cancelButton} type="button" className="text-button" data-quota-cancel={profileId} aria-describedby={`profile-name-${profileId}`} disabled={stopping} onClick={() => { void cancel(); }}>{stopping ? 'Stopping read…' : 'Cancel read'}</button>}</div>
    {outcome && <div ref={result} className="quota-result" data-quota-profile={profileId} role={outcome.kind === 'error' ? 'alert' : outcome.kind === 'preview' ? 'region' : 'status'} aria-label={outcome.kind === 'preview' ? `Quota preview for ${profileLabel}` : undefined} tabIndex={-1}>
      {approval.sourceKind === 'test_fixture' && <p className="provider-fixture-note" data-provider-fixture>Synthetic provider fixture. These values are controlled test data.</p>}
      {outcome.kind !== 'preview' ? <p>{outcome.message}</p> : <>
        <h3>Quota preview for <bdi>{profileLabel}</bdi></h3>
        <p className="provider-help">Identity is unverified. These readings are not saved or included in combined totals. Monitoring remains stopped.</p>
        <p className="quota-observed">Read <time dateTime={outcome.value.observedAtUtc}>{date(outcome.value.observedAtUtc)}</time></p>
        {outcome.value.limits.length === 0 ? <p>Codex returned no quota limits for this profile. Missing readings do not mean zero usage.</p> : outcome.value.limits.map((limit, index) => <details className="quota-limit" open={index === 0} key={`${outcome.value.requestId}:${limit.limitId}`}>
          <summary><bdi>{limit.limitName ?? limit.limitId}</bdi></summary>
          {limit.primary && <WindowValues value={limit.primary} />}{limit.secondary && <WindowValues value={limit.secondary} />}
          {!limit.primary && !limit.secondary && <p>No quota windows were supplied for this limit.</p>}
          {(limit.primary === null || limit.secondary === null) && (limit.primary || limit.secondary) && <p className="provider-help">{limit.primary === null ? 'Primary window' : 'Secondary window'}: Not available</p>}
          <details className="quota-source-details"><summary>Reading details</summary><dl className="backup-inspection-metadata"><dt>Provider limit ID</dt><dd><bdi>{limit.limitId}</bdi></dd><dt>Provider status</dt><dd><bdi>{limit.reachedType ?? 'Not supplied'}</bdi></dd></dl></details>
        </details>)}
        <p className="provider-help">Reset times are estimates. This preview does not confirm that a reset occurred. Credits and activity history are not available in this preview.</p>
      </>}
    </div>}
  </div>;
}
