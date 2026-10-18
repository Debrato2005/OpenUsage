import { useEffect, useRef, useState, type ReactNode } from 'react';
import { Button } from './components/ui/button';
import { Collapsible, CollapsiblePanel, CollapsibleTrigger } from './components/ui/collapsible';
import { invoke, type ActivityFreshness, type ProfileActivitySavedRead, type SavedProfileActivity, type ProviderExecutableApproval } from './contracts';
import { providerFailure } from './provider-errors';
import './activity.css';

type Props = { profileId: string; profileLabel: string; approval: ProviderExecutableApproval; busy: boolean; onStarted: () => void; onSaved: (value: ProfileActivitySavedRead) => Promise<void> };
type Outcome = { kind: 'error' | 'cancelled'; message: string };
const labels = { lifetimeTokens: 'Lifetime tokens', peakDailyTokens: 'Peak daily tokens', currentStreakDays: 'Current streak (days)', longestStreakDays: 'Longest streak (days)', longestRunningTurnSec: 'Longest turn (seconds)' };
const freshnessLabels = { unknown: 'Freshness unknown', fresh: 'Current', stale: 'Stale' };
const observedDate = new Intl.DateTimeFormat('en-US', { timeZone: 'UTC', year: 'numeric', month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit', timeZoneName: 'short' });
const observed = (value: string) => observedDate.format(new Date(value));
function Metric({ name, value, freshness }: { name: keyof typeof labels; value: string | null; freshness: ActivityFreshness }) {
  return <div><dt>{labels[name]}</dt><dd data-activity-metric={name} data-activity-state={value === null ? 'not_available' : freshness} className="activity-count" tabIndex={0} aria-label={labels[name]}><bdi>{value ?? 'Not available'}</bdi></dd></div>;
}
function ActivityValues({ value }: { value: SavedProfileActivity }) {
  const [page, setPage] = useState(0);
  const buckets = value.dailyUsageBuckets;
  const start = page * 50;
  return <>
    <dl className="activity-summary">{(['lifetimeTokens', 'peakDailyTokens', 'currentStreakDays'] as const).map(name => <Metric key={name} name={name} value={value.summary[name]} freshness={value.freshness} />)}</dl>
    <Collapsible className="activity-details">
      <CollapsibleTrigger render={<Button variant="ghost" className="text-button" />} type="button">Activity details</CollapsibleTrigger>
      <CollapsiblePanel>
        <dl className="activity-summary activity-secondary">{(['longestRunningTurnSec', 'longestStreakDays'] as const).map(name => <Metric key={name} name={name} value={value.summary[name]} freshness={value.freshness} />)}</dl>
        <h4>Saved daily totals</h4>
        <p className="provider-help">Dates are shown as returned. Missing dates do not mean zero usage.</p>
        {value.dailyState !== 'returned' && <p data-activity-daily-state={value.dailyState}>{value.dailyState === 'unavailable' ? 'The latest read did not supply daily usage.' : 'The latest read returned no daily rows.'}</p>}
        {value.dailyTruncated && <p className="provider-help" data-activity-daily-truncated>Only part of the saved daily data is shown.</p>}
        {buckets.length > 0 && <>
          <ol className="activity-days" aria-label="Saved daily totals">{buckets.slice(start, start + 50).map(row => <li data-activity-day data-activity-day-freshness={row.freshness} key={row.date}><time dateTime={row.date}>{row.date}</time><span className="activity-count" tabIndex={0} aria-label={`Tokens for ${row.date}`}><bdi>{row.tokens}</bdi></span><span className="provider-help activity-day-observed">{freshnessLabels[row.freshness]} · Observed <time data-activity-observed dateTime={row.observedAtUtc}>{observed(row.observedAtUtc)}</time></span></li>)}</ol>
          {buckets.length > 50 && <nav className="activity-pagination" aria-label="Daily usage pages"><p role="status">Rows {start + 1}–{Math.min(start + 50, buckets.length)} of {buckets.length} shown</p><Button type="button" variant="outline" disabled={page === 0} onClick={() => setPage(current => current - 1)}>Previous</Button><Button type="button" variant="outline" disabled={start + 50 >= buckets.length} onClick={() => setPage(current => current + 1)}>Next</Button></nav>}
        </>}
      </CollapsiblePanel>
    </Collapsible>
  </>;
}

// Cache reads do not require an approved executable or execute the provider.
export function ProfileActivity({ profileId, profileLabel, children }: { profileId: string; profileLabel: string; children: (onSaved: (receipt: ProfileActivitySavedRead) => Promise<void>) => ReactNode }) {
  const [value, setValue] = useState<SavedProfileActivity | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [focus, setFocus] = useState(false);
  const result = useRef<HTMLDivElement>(null);
  const alive = useRef(true);
  const loadGeneration = useRef(0);
  useEffect(() => {
    alive.current = true; void load();
    const interval = window.setInterval(() => { void load(); }, 60_000);
    return () => { alive.current = false; loadGeneration.current += 1; window.clearInterval(interval); };
  }, [profileId]);
  useEffect(() => { if (focus && result.current) { result.current.focus(); setFocus(false); } }, [focus, value, error]);
  async function load(committed?: ProfileActivitySavedRead, focusResult = false) {
    const operation = ++loadGeneration.current; setLoading(true);
    try {
      const next = await invoke('load_profile_activity', { profileId });
      if (next && next.profileId !== profileId) throw new Error('Incompatible saved activity profile.');
      if (committed && (!next || next.bindingId !== committed.bindingId || next.collectionRunId !== committed.collectionRunId)) throw new Error('Incompatible saved activity observation.');
      if (alive.current && operation === loadGeneration.current) { setValue(next); setError(null); }
    } catch {
      if (alive.current && operation === loadGeneration.current) setError(providerFailure('provider_activity_load_failed'));
    } finally {
      if (alive.current && operation === loadGeneration.current) { setLoading(false); if (focusResult || committed) setFocus(true); }
    }
  }
  return <>
    {children(receipt => load(receipt, true))}
    {(value || error) && <div ref={result} className="activity-saved" data-activity-saved={profileId} tabIndex={-1} role="region" aria-label={`Saved activity for ${profileLabel}`}>
      <h3>Saved activity</h3>
      {error && <div><p role="alert" data-activity-cache-error>{error}</p><Button type="button" variant="outline" disabled={loading} onClick={() => { void load(undefined, true); }}>Reload saved activity</Button></div>}
      {value && <>
        {value.sourceKind === 'test_fixture' && <p className="provider-fixture-note" data-provider-fixture>Synthetic test data</p>}
        <p className="provider-help">Unverified identity · Saved locally</p>
        <p className="provider-help"><span data-activity-freshness={value.freshness}>{freshnessLabels[value.freshness]}</span> · Last read <time dateTime={value.observedAtUtc}>{observed(value.observedAtUtc)}</time></p>
        <p className="provider-help">Account activity; application scope and coverage are unknown. Monitoring is stopped.</p>
        <ActivityValues key={`${value.bindingId}:${value.collectionRunId}`} value={value} />
      </>}
    </div>}
  </>;
}

// The request stays mounted in ProfileProviderCheck while its launch button is disclosed.
export function useActivityPreview({ profileId, approval, busy, onStarted, onSaved }: Props) {
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
    setWorking(true); setOutcome(current => current?.kind === 'error' ? current : null); onStarted();
    try {
      const begun = await invoke('begin_profile_activity_read', { profileId, approvalId: approval.approvalId });
      if (begun.profileId !== profileId || begun.approvalId !== approval.approvalId || begun.sourceKind !== approval.sourceKind) throw new Error('Incompatible activity binding.');
      activeRequest.current = begun.requestId;
      if (!alive.current) { await invoke('cancel_profile_activity_read', { profileId, requestId: begun.requestId }); return; }
      setRequestId(begun.requestId);
      const value = await invoke('complete_profile_activity_read', { profileId, requestId: begun.requestId });
      if (value.profileId !== profileId || value.requestId !== begun.requestId || value.approvalId !== approval.approvalId || value.sourceKind !== begun.sourceKind) throw new Error('Incompatible activity result.');
      if (alive.current) { setOutcome(null); await onSaved(value); if (alive.current && cancellationFailure.current) setOutcome({ kind: 'error', message: providerFailure(cancellationFailure.current) }); }
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
    clear: () => { generation.current += 1; setOutcome(current => current?.kind === 'error' ? current : null); setFocus(false); },
    action: <Button className="text-button" variant="ghost" type="button" data-activity-read={profileId} aria-describedby={`profile-name-${profileId}`} disabled={busy || working} onClick={() => { void read(); }}>Read activity</Button>,
    content: <>
      {working && <div className="profile-actions activity-pending"><span role="status">Reading activity…</span>{requestId && <Button ref={cancelButton} className="text-button" variant="ghost" type="button" data-activity-cancel={profileId} aria-describedby={`profile-name-${profileId}`} disabled={stopping} onClick={() => { void cancel(); }}>{stopping ? 'Stopping read…' : 'Cancel activity read'}</Button>}</div>}
      {outcome && <div ref={result} className="activity-result" data-activity-profile={profileId} tabIndex={-1} role={outcome.kind === 'error' ? 'alert' : 'status'}>
        {approval.sourceKind === 'test_fixture' && <p className="provider-fixture-note" data-provider-fixture>Synthetic test data</p>}
        <p>{outcome.message}</p>
      </div>}
    </>,
  };
}
