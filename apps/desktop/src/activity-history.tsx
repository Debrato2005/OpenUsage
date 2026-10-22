import { useDateFormat } from './time-display';
import { useEffect, useState } from 'react';
import { DataExport } from './data-export';
import { Button } from './components/ui/button';
import { Collapsible, CollapsiblePanel, CollapsibleTrigger } from './components/ui/collapsible';
import { invoke, type ActivityHistoryEntry, type ActivityHistoryPage, type ProfileRecord } from './contracts';
import './activity-history.css';

const labels = { lifetimeTokens: 'Lifetime tokens', peakDailyTokens: 'Peak daily tokens', longestRunningTurnSec: 'Longest turn (seconds)', currentStreakDays: 'Current streak (days)', longestStreakDays: 'Longest streak (days)' };
const dailyLabels = { unavailable: 'Daily totals were not supplied.', empty: 'The provider returned no daily totals.', returned: 'Daily totals were supplied with this reading.' };

function Snapshot({ value }: { value: ActivityHistoryEntry }) {
  const dates = useDateFormat({ second: '2-digit' });
  return <Collapsible className="activity-history-entry" data-activity-history-run={value.collectionRunId}>
    <CollapsibleTrigger render={<Button variant="ghost" />} className="activity-history-entry-trigger"><time dateTime={value.observedAtUtc} title={value.observedAtUtc}>{dates.format(new Date(value.observedAtUtc))}</time><span>Saved summary</span><span aria-hidden="true">⌄</span></CollapsibleTrigger>
    <CollapsiblePanel>
      {value.sourceKind === 'test_fixture' && <p className="activity-history-meta">Synthetic test data</p>}
      <dl className="activity-history-summary">{(Object.keys(labels) as (keyof typeof labels)[]).map(name => <div key={name}><dt>{labels[name]}</dt><dd data-activity-history-metric={name}><bdi>{value.summary[name] ?? 'Not available'}</bdi></dd></div>)}</dl>
      <p className="activity-history-meta" data-activity-history-daily-state={value.dailyBucketsState}>{dailyLabels[value.dailyBucketsState]}</p>
      <Collapsible className="activity-history-source"><CollapsibleTrigger render={<Button variant="ghost" />}>Source details</CollapsibleTrigger><CollapsiblePanel><dl>
        <dt>Source</dt><dd>{value.sourceKind === 'test_fixture' ? 'Synthetic provider' : 'Official OpenAI companion'}</dd>
        <dt>Provider version</dt><dd><bdi>{value.providerVersion}</bdi></dd>
        <dt>Observed (UTC)</dt><dd><time dateTime={value.observedAtUtc}>{value.observedAtUtc}</time></dd>
        <dt>Application scope</dt><dd>Unknown</dd><dt>Coverage</dt><dd>Unknown</dd>
        <dt>Local connection reference</dt><dd><bdi>{value.bindingId}</bdi></dd><dt>Reading reference</dt><dd><bdi>{value.collectionRunId}</bdi></dd>
      </dl></CollapsiblePanel></Collapsible>
    </CollapsiblePanel>
  </Collapsible>;
}

export function ActivityHistory({ profile, revision }: { profile: ProfileRecord; revision: number }) {
  const profileId = profile.id;
  const [exportBusy, setExportBusy] = useState(false);
  const scope = `${profileId}:${revision}`;
  const [disclosure, setDisclosure] = useState({ profileId, open: false });
  const open = disclosure.profileId === profileId && disclosure.open;
  const [navigation, setNavigation] = useState<{ scope: string; cursors: (string | null)[] }>({ scope, cursors: [null] });
  const cursors = navigation.scope === scope ? navigation.cursors : [null];
  const cursor = cursors.at(-1) ?? null;
  const [attempt, setAttempt] = useState(0);
  const key = `${scope}:${cursor ?? 'latest'}:${attempt}`;
  const [result, setResult] = useState<{ key: string; page: ActivityHistoryPage | null; failed: boolean } | null>(null);
  const current = result?.key === key ? result : null;
  const page = current?.page;
  const loading = open && current === null;
  useEffect(() => {
    if (!open) return;
    let active = true;
    void invoke('load_profile_activity_history', { profileId, beforeRunId: cursor }).then(value => {
      if (value.profileId !== profileId || value.entries.some(entry => entry.profileId !== profileId || entry.collectionRunId === cursor)
        || (value.nextBeforeRunId !== null && cursors.includes(value.nextBeforeRunId))) throw new Error('Incompatible activity history page.');
      if (active) setResult({ key, page: value, failed: false });
    }).catch(() => { if (active) setResult({ key, page: null, failed: true }); });
    return () => { active = false; };
  }, [open, key, profileId, cursor]);
  return <Collapsible className="activity-history" data-activity-history={profileId} open={open} onOpenChange={next => { setDisclosure({ profileId, open: next }); setNavigation({ scope, cursors: [null] }); setResult(null); setAttempt(value => value + 1); }}>
    <CollapsibleTrigger render={<Button variant="outline" />} data-activity-history-open aria-expanded={open}>{open ? 'Hide activity history' : 'View activity history'}</CollapsibleTrigger>
    <CollapsiblePanel>
      <div className="activity-history-heading"><h3>Activity history</h3><p>Saved account summaries, newest first. Snapshots are separate observations, not individual usage events.</p></div>
      <DataExport profiles={[profile]} busy={exportBusy} setBusy={setExportBusy} activityHistory />
      {loading && <p className="activity-history-meta" role="status">Loading saved activity history…</p>}
      {current?.failed && <div className="activity-history-error" role="alert"><p>{cursor ? 'This activity history page could not be loaded.' : 'Saved activity history could not be loaded.'}</p><Button variant="outline" onClick={() => setAttempt(value => value + 1)}>Retry activity history</Button>{cursor && <Button variant="ghost" onClick={() => setNavigation({ scope, cursors: [null] })}>Back to latest readings</Button>}</div>}
      {page && <div className="activity-history-entries">{page.entries.length === 0 ? <p className="activity-history-meta">No accepted activity summaries are saved for this account.</p> : page.entries.map(value => <Snapshot key={value.collectionRunId} value={value} />)}</div>}
      <div className="activity-history-navigation"><Button variant="outline" disabled={loading || cursors.length === 1} onClick={() => setNavigation({ scope, cursors: cursors.slice(0, -1) })}>Newer readings</Button><span aria-live="polite">Page {cursors.length}</span><Button variant="outline" disabled={loading || !page?.nextBeforeRunId} onClick={() => { if (page?.nextBeforeRunId) setNavigation({ scope, cursors: [...cursors, page.nextBeforeRunId] }); }}>Older readings</Button></div>
    </CollapsiblePanel>
  </Collapsible>;
}
