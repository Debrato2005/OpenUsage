import { useEffect, useState } from 'react';
import { Button } from './components/ui/button';
import { Collapsible, CollapsiblePanel, CollapsibleTrigger } from './components/ui/collapsible';
import { invoke, type QuotaHistoryEntry, type QuotaHistoryPage, type QuotaWindow } from './contracts';
import './quota-history.css';

const dates = new Intl.DateTimeFormat('en-US', { timeZone: 'UTC', year: 'numeric', month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit', second: '2-digit', timeZoneName: 'short' });
const displayTime = (value: string) => dates.format(new Date(value));
function WindowRow({ limit, role, value }: { limit: string; role: string; value: QuotaWindow }) {
  const reset = value.resetsAtUnix === null ? null : new Date(value.resetsAtUnix * 1000).toISOString();
  return <tr><th scope="row"><bdi>{limit}</bdi><small>{role}</small></th><td>{value.label}{value.windowDurationMins !== null && <small>{value.windowDurationMins} minutes</small>}</td><td>{value.usedPercent}%</td><td>{value.derivedRemainingPercent}%</td><td>{reset ? <time dateTime={reset}>{displayTime(reset)}</time> : 'Not reported'}</td></tr>;
}
function Snapshot({ value }: { value: QuotaHistoryEntry }) {
  const first = value.limits.find(limit => limit.primary || limit.secondary);
  const window = first?.primary ?? first?.secondary;
  return <Collapsible className="quota-history-entry" data-quota-history-run={value.collectionRunId}>
    <CollapsibleTrigger render={<Button variant="ghost" />} className="quota-history-entry-trigger">
      <time dateTime={value.observedAtUtc}>{displayTime(value.observedAtUtc)}</time>
      <span>{window ? `${window.derivedRemainingPercent}% left · ${window.label}` : 'No quota windows'}</span><span aria-hidden="true">⌄</span>
    </CollapsibleTrigger>
    <CollapsiblePanel>
      {value.sourceKind === 'test_fixture' && <p className="quota-history-meta">Synthetic test data</p>}
      <div className="quota-history-table" tabIndex={0} role="region" aria-label={`Quota snapshot ${value.observedAtUtc}`}>
        <table><thead><tr><th scope="col">Limit</th><th scope="col">Window</th><th scope="col">Used</th><th scope="col">Remaining (derived)</th><th scope="col">Expected reset</th></tr></thead>
          <tbody>{value.limits.flatMap(limit => [limit.primary && <WindowRow key={`${limit.limitId}:primary`} limit={limit.limitName ?? limit.limitId} role="Primary" value={limit.primary} />, limit.secondary && <WindowRow key={`${limit.limitId}:secondary`} limit={limit.limitName ?? limit.limitId} role="Secondary" value={limit.secondary} />])}</tbody></table>
      </div>
      {!window && <p className="quota-history-meta">This accepted reading supplied no quota windows.</p>}
      <p className="quota-history-meta">Saved from {value.sourceKind === 'test_fixture' ? 'a synthetic provider' : 'the official OpenAI companion'} · Version {value.providerVersion}. Expected resets are recorded values, not reset confirmations.</p>
      <Collapsible className="quota-history-source"><CollapsibleTrigger render={<Button variant="ghost" />}>Source details</CollapsibleTrigger><CollapsiblePanel><dl><dt>Local connection reference</dt><dd><bdi>{value.bindingId}</bdi></dd><dt>Reading reference</dt><dd><bdi>{value.collectionRunId}</bdi></dd><dt>Provider operation</dt><dd>account/rateLimits/read</dd></dl><p className="quota-history-meta">Identity is unverified. Historical snapshots are separate observations, not a record of every request.</p></CollapsiblePanel></Collapsible>
    </CollapsiblePanel>
  </Collapsible>;
}
export function QuotaHistory({ profileId, revision }: { profileId: string; revision: number }) {
  const [open, setOpen] = useState(false);
  const [navigation, setNavigation] = useState<{ revision: number; cursors: (string | null)[] }>({ revision, cursors: [null] });
  const cursors = navigation.revision === revision ? navigation.cursors : [null];
  const cursor = cursors.at(-1) ?? null;
  const [loaded, setLoaded] = useState<{ revision: number; cursor: string | null; page: QuotaHistoryPage } | null>(null);
  const [loading, setLoading] = useState(false);
  const [failed, setFailed] = useState(false);
  const [attempt, setAttempt] = useState(0);
  const page = loaded?.revision === revision && loaded.cursor === cursor && loaded.page.profileId === profileId ? loaded.page : null;
  useEffect(() => {
    if (!open) return;
    let current = true;
    setLoading(true); setFailed(false);
    void invoke('load_profile_quota_history', { profileId, beforeRunId: cursor }).then(value => {
      if (value.profileId !== profileId || value.entries.some(entry => entry.collectionRunId === cursor)) throw new Error('Incompatible history page.');
      if (current) setLoaded({ revision, cursor, page: value });
    }).catch(() => { if (current) setFailed(true); }).finally(() => { if (current) setLoading(false); });
    return () => { current = false; };
  }, [profileId, revision, cursor, open, attempt]);
  return <Collapsible className="quota-history" open={open} onOpenChange={setOpen} data-quota-history={profileId}>
    <CollapsibleTrigger render={<Button variant="outline" />} data-quota-history-open aria-expanded={open}>{open ? 'Hide quota history' : 'View quota history'}</CollapsibleTrigger>
    <CollapsiblePanel>
      <div className="quota-history-heading"><h3>Quota history</h3><p>Saved readings for this profile, newest first. Earlier connections remain separate snapshots.</p></div>
      {loading && <p className="quota-history-meta" role="status">Loading saved history…</p>}
      {failed && <div className="quota-history-error" role="alert"><p>Saved quota history could not be loaded.</p><Button variant="outline" disabled={loading} onClick={() => setAttempt(value => value + 1)}>Retry history</Button>{cursor && <Button variant="ghost" disabled={loading} onClick={() => setNavigation({ revision, cursors: [null] })}>Back to latest readings</Button>}</div>}
      {page && <><div className="quota-history-entries">{page.entries.map(value => <Snapshot key={value.collectionRunId} value={value} />)}</div>
        {page.entries.length === 0 && <p className="quota-history-meta">No accepted quota readings are saved for this profile.</p>}
        <div className="quota-history-navigation"><Button variant="outline" disabled={loading || cursors.length === 1} onClick={() => setNavigation({ revision, cursors: cursors.slice(0, -1) })}>Newer readings</Button><span aria-live="polite">Page {cursors.length}</span><Button variant="outline" disabled={loading || !page.nextBeforeRunId} onClick={() => { if (page.nextBeforeRunId) setNavigation({ revision, cursors: [...cursors, page.nextBeforeRunId] }); }}>Older readings</Button></div>
      </>}
    </CollapsiblePanel>
  </Collapsible>;
}
