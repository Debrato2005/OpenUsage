import { useEffect, useRef, useState } from 'react';
import { Button } from './components/ui/button';
import { Collapsible, CollapsiblePanel, CollapsibleTrigger } from './components/ui/collapsible';
import { invoke, type ProfileActivityPreview, type ProviderExecutableApproval } from './contracts';
import { providerFailure } from './provider-errors';
import './activity.css';

type Props = { profileId: string; profileLabel: string; approval: ProviderExecutableApproval; busy: boolean; onStarted: () => void };
type Outcome = { kind: 'error' | 'cancelled'; message: string } | { kind: 'preview'; value: ProfileActivityPreview };
const labels = { lifetimeTokens: 'Lifetime tokens', peakDailyTokens: 'Peak daily tokens', currentStreakDays: 'Current streak (days)', longestStreakDays: 'Longest streak (days)', longestRunningTurnSec: 'Longest turn (seconds)' };
const observedDate = new Intl.DateTimeFormat('en-US', { timeZone: 'UTC', year: 'numeric', month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit', timeZoneName: 'short' });
function Metric({ name, value }: { name: keyof typeof labels; value: string | null }) {
  return <div><dt>{labels[name]}</dt><dd data-activity-metric={name} className="activity-count" tabIndex={0} aria-label={labels[name]}><bdi>{value ?? 'Not supplied'}</bdi></dd></div>;
}
function ActivityValues({ value }: { value: ProfileActivityPreview }) {
  const [page, setPage] = useState(0);
  const buckets = value.dailyUsageBuckets;
  const start = page * 50;
  return <>
    <dl className="activity-summary">{(['lifetimeTokens', 'peakDailyTokens', 'currentStreakDays'] as const).map(name => <Metric key={name} name={name} value={value.summary[name]} />)}</dl>
    <Collapsible className="activity-details">
      <CollapsibleTrigger render={<Button variant="ghost" className="text-button" />} type="button">Activity details</CollapsibleTrigger>
      <CollapsiblePanel>
        <dl className="activity-summary activity-secondary">{(['longestRunningTurnSec', 'longestStreakDays'] as const).map(name => <Metric key={name} name={name} value={value.summary[name]} />)}</dl>
        <h4>Returned daily usage</h4>
        <p className="provider-help">Dates are shown as returned. Missing dates do not mean zero usage.</p>
        {buckets === null ? <p data-activity-daily-state="unavailable">Daily usage was not supplied.</p> : buckets.length === 0 ? <p data-activity-daily-state="empty">No daily rows were returned.</p> : <>
          <ol className="activity-days" aria-label="Returned daily usage">{buckets.slice(start, start + 50).map((row, index) => <li data-activity-day key={start + index}><time dateTime={row.startDate}>{row.startDate}</time><span className="activity-count" tabIndex={0} aria-label={`Tokens for ${row.startDate}`}><bdi>{row.tokens}</bdi></span></li>)}</ol>
          {buckets.length > 50 && <nav className="activity-pagination" aria-label="Daily usage pages"><p role="status">Rows {start + 1}–{Math.min(start + 50, buckets.length)} of {buckets.length} returned</p><Button type="button" variant="outline" disabled={page === 0} onClick={() => setPage(current => current - 1)}>Previous</Button><Button type="button" variant="outline" disabled={start + 50 >= buckets.length} onClick={() => setPage(current => current + 1)}>Next</Button></nav>}
        </>}
      </CollapsiblePanel>
    </Collapsible>
  </>;
}

// The request stays mounted in ProfileProviderCheck while its launch button is disclosed.
export function useActivityPreview({ profileId, profileLabel, approval, busy, onStarted }: Props) {
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
    return () => { alive.current = false; const requestId = activeRequest.current; if (requestId) void invoke('cancel_profile_activity_read', { profileId, requestId }).catch(() => {}); };
  }, [profileId]);
  useEffect(() => { if (working && requestId) cancelButton.current?.focus(); }, [working, requestId]);
  useEffect(() => { if (focus && !working && result.current) { result.current.focus(); setFocus(false); } }, [focus, working, outcome]);

  async function read() {
    if (busy || pending.current) return;
    generation.current += 1; pending.current = true; stopped.current = false; cancellationFailure.current = null;
    setWorking(true); setOutcome(null); onStarted();
    try {
      const begun = await invoke('begin_profile_activity_read', { profileId, approvalId: approval.approvalId });
      if (begun.profileId !== profileId || begun.approvalId !== approval.approvalId || begun.sourceKind !== approval.sourceKind) throw new Error('Incompatible activity binding.');
      activeRequest.current = begun.requestId;
      if (!alive.current) { await invoke('cancel_profile_activity_read', { profileId, requestId: begun.requestId }); return; }
      setRequestId(begun.requestId);
      const value = await invoke('complete_profile_activity_read', { profileId, requestId: begun.requestId });
      if (value.profileId !== profileId || value.requestId !== begun.requestId || value.approvalId !== approval.approvalId || value.sourceKind !== begun.sourceKind) throw new Error('Incompatible activity result.');
      if (alive.current) setOutcome(cancellationFailure.current ? { kind: 'error', message: providerFailure(cancellationFailure.current) } : stopped.current ? { kind: 'cancelled', message: 'Activity read stopped.' } : { kind: 'preview', value });
    } catch (failure) {
      const cancelled = failure === 'provider_activity_cancelled' || (stopped.current && failure === 'provider_activity_not_found');
      if (alive.current) setOutcome(!cancellationFailure.current && cancelled ? { kind: 'cancelled', message: 'Activity read stopped.' } : { kind: 'error', message: providerFailure(cancellationFailure.current ?? failure) });
    } finally {
      pending.current = false; activeRequest.current = null;
      if (alive.current) { setWorking(false); setStopping(false); setRequestId(null); setFocus(true); }
    }
  }
  async function cancel() {
    if (!requestId || !pending.current || stopping) return;
    const operation = generation.current; stopped.current = true; setStopping(true);
    try { await invoke('cancel_profile_activity_read', { profileId, requestId }); }
    catch (failure) { if (alive.current && generation.current === operation) { cancellationFailure.current = failure; setOutcome({ kind: 'error', message: providerFailure(failure) }); setFocus(true); } }
    finally { if (alive.current && generation.current === operation) setStopping(false); }
  }
  return {
    working,
    clear: () => { setOutcome(null); setFocus(false); },
    action: <Button className="text-button" variant="ghost" type="button" data-activity-read={profileId} aria-describedby={`profile-name-${profileId}`} disabled={busy || working} onClick={() => { void read(); }}>Read activity</Button>,
    content: <>
      {working && <div className="profile-actions activity-pending"><span role="status">Reading activity…</span>{requestId && <Button ref={cancelButton} className="text-button" variant="ghost" type="button" data-activity-cancel={profileId} aria-describedby={`profile-name-${profileId}`} disabled={stopping} onClick={() => { void cancel(); }}>{stopping ? 'Stopping read…' : 'Cancel activity read'}</Button>}</div>}
      {outcome && <div ref={result} className="activity-result" data-activity-profile={profileId} tabIndex={-1} role={outcome.kind === 'error' ? 'alert' : outcome.kind === 'preview' ? 'region' : 'status'} aria-label={outcome.kind === 'preview' ? `Activity preview for ${profileLabel}` : undefined}>
        {approval.sourceKind === 'test_fixture' && <p className="provider-fixture-note" data-provider-fixture>Synthetic test data</p>}
        {outcome.kind !== 'preview' ? <p>{outcome.message}</p> : <>
          <h3>Activity preview</h3><p className="provider-help">Unverified identity · Not saved</p>
          <p className="provider-help">Read <time dateTime={outcome.value.observedAtUtc}>{observedDate.format(new Date(outcome.value.observedAtUtc))}</time></p>
          <p className="provider-help">Account activity; application scope and coverage are unknown. Monitoring is stopped.</p>
          <ActivityValues key={outcome.value.requestId} value={outcome.value} />
        </>}
      </div>}
    </>,
  };
}
