import { useEffect, useState } from 'react';
import { Button } from './components/ui/button';
import { Collapsible, CollapsiblePanel, CollapsibleTrigger } from './components/ui/collapsible';
import { invoke, type SavedAlertHistory } from './contracts';
import './alert-history.css';

const dates = new Intl.DateTimeFormat('en-US', { dateStyle: 'medium', timeStyle: 'short', timeZone: 'UTC' });

function windowLabel(minutes: number) {
  return minutes === 300 ? '5-hour window' : minutes === 10080 ? '7-day window' : `${minutes}-minute window`;
}

export function AlertHistory({ revision = '', onViewUsage }: { revision?: string; onViewUsage: (profileId: string) => void }) {
  const [open, setOpen] = useState(false);
  const [attempt, setAttempt] = useState(0);
  const [result, setResult] = useState<{ key: string; history: SavedAlertHistory | null; failed: boolean } | null>(null);
  const key = `${revision}:${attempt}`;
  const current = result?.key === key ? result : null;
  const loading = open && current === null;
  useEffect(() => {
    if (!open) return;
    let active = true;
    void invoke('load_alert_history').then(history => {
      if (active) setResult({ key, history, failed: false });
    }).catch(() => { if (active) setResult({ key, history: null, failed: true }); });
    return () => { active = false; };
  }, [open, key]);
  return <Collapsible className="alert-history" data-alert-history open={open} onOpenChange={next => { setResult(null); setOpen(next); }}>
    <CollapsibleTrigger render={<Button variant="outline" />} aria-expanded={open}>Alert history</CollapsibleTrigger>
    <CollapsiblePanel>
      <div className="alert-history-heading"><p>Confirmed resets saved on this device.</p><Button variant="outline" disabled={loading} onClick={() => setAttempt(value => value + 1)}>Refresh list</Button></div>
      {loading && <p className="alert-history-note" role="status">Loading alert history…</p>}
      {current?.failed && <div role="alert"><p>Alert history could not be loaded.</p><Button variant="outline" onClick={() => setAttempt(value => value + 1)}>Retry alert history</Button></div>}
      {current?.history && <>
        {current.history.events.length === 0 ? <p className="alert-history-note">No confirmed resets are saved yet.</p> : <ol className="alert-history-events" aria-label="Saved reset confirmations" tabIndex={0}>
          {current.history.events.map(event => <li key={event.id} data-alert-history-event={event.id}>
            <div className="alert-history-event"><div><strong>Reset confirmed</strong><p><bdi>{event.profileLabel}</bdi> · {windowLabel(event.windowDurationMins)}</p><time dateTime={event.evidenceAtUtc} title={event.evidenceAtUtc}>{dates.format(new Date(event.evidenceAtUtc))} UTC</time></div><Button variant="outline" onClick={() => onViewUsage(event.profileId)}>View usage<span className="sr-only"> for {event.profileLabel}</span></Button></div>
            <Collapsible className="alert-history-source"><CollapsibleTrigger render={<Button variant="ghost" />}>Source details</CollapsibleTrigger><CollapsiblePanel><dl><dt>Source</dt><dd>{event.sourceKind === 'test_fixture' ? 'Synthetic test data' : 'Official OpenAI companion'}</dd><dt>Quota limit</dt><dd><bdi>{event.limitId}</bdi></dd><dt>Window</dt><dd>{event.windowRole === 'primary' ? 'Primary' : 'Secondary'} · {event.windowDurationMins} minutes</dd><dt>Evidence time (UTC)</dt><dd><time dateTime={event.evidenceAtUtc}>{event.evidenceAtUtc}</time></dd><dt>Delivery</dt><dd>No desktop notification sent.</dd></dl></CollapsiblePanel></Collapsible>
          </li>)}
        </ol>}
        {current.history.truncated && <p className="alert-history-note">Showing the latest 100 saved events.</p>}
      </>}
    </CollapsiblePanel>
  </Collapsible>;
}
