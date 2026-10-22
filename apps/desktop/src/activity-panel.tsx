import { useEffect, useState } from 'react';
import { Button } from './components/ui/button';
import { Checkbox } from './components/ui/checkbox';
import { Select, SelectTrigger, SelectValue, SelectPopup, SelectItem } from './components/ui/select';
import { invoke, type ProfileRecord, type CombinedHeatmapEligibility } from './contracts';
import { TokenHeatmap } from './token-heatmap';

type Props = { profiles: ProfileRecord[]; profileId: string | null; revision: number; onSelect: (id: string) => void };
export function ActivityPanel({ profiles, profileId, revision, onSelect }: Props) {
  const [scope, setScope] = useState<'individual' | 'combined'>('individual');
  const [selection, setSelection] = useState<string[] | null>(null);
  const [result, setResult] = useState<{ key: string; value: CombinedHeatmapEligibility | null; failed: boolean } | null>(null);
  const [attempt, setAttempt] = useState(0);
  const ids = selection === null ? profiles.filter(profile => profile.connectionState === 'connected').map(profile => profile.id) : selection.filter(id => profiles.some(profile => profile.id === id));
  const key = `${ids.join(',')}:${profiles.map(profile => `${profile.id}:${profile.connectionState}`).join(',')}:${revision}:${attempt}`;
  useEffect(() => {
    if (scope !== 'combined' || !ids.length || !profileId) return;
    let cancelled = false;
    void invoke('load_combined_heatmap', { profileIds: ids }).then(value => {
      if (!cancelled) setResult({ key, value, failed: false });
    }).catch(() => { if (!cancelled) setResult({ key, value: null, failed: true }); });
    return () => { cancelled = true; };
  }, [scope, key, profileId]);
  const current = result?.key === key ? result : null;
  const reasons = { not_connected: 'Not connected', activity_unavailable: 'No saved activity', identity_unverified: 'Account identity and activity scope cannot be verified' };
  return <div className="activity-panel" data-activity-scope={scope}>
    <div className="activity-scope-controls">
      <div role="group" aria-label="Activity accounts"><Button variant="ghost" aria-pressed={scope === 'individual'} onClick={() => setScope('individual')}>Individual</Button><Button variant="ghost" aria-pressed={scope === 'combined'} onClick={() => setScope('combined')}>Combined</Button></div>
      {scope === 'individual' && profiles.length > 0 && <Select modal={false} value={profileId} items={Object.fromEntries(profiles.map(profile => [profile.id, profile.label]))} onValueChange={id => { if (id) onSelect(id); }}><SelectTrigger aria-label="Activity account"><SelectValue /></SelectTrigger><SelectPopup alignItemWithTrigger={false}>{profiles.map(profile => <SelectItem key={profile.id} value={profile.id}>{profile.label}</SelectItem>)}</SelectPopup></Select>}
    </div>
    {scope === 'individual' ? <TokenHeatmap compact profileId={profileId} profileLabel={profiles.find(profile => profile.id === profileId)?.label ?? 'Account'} revision={revision} />
      : <section className="token-heatmap" data-combined-heatmap aria-label="Combined token activity">
        <header><h2>Combined token activity</h2><p className="overview-meta">Select the accounts to include.</p></header>
        <div className="combined-account-selection">{profiles.map(profile => <label key={profile.id}><Checkbox checked={ids.includes(profile.id)} onCheckedChange={checked => setSelection(checked ? [...ids, profile.id] : ids.filter(id => id !== profile.id))} /><span>{profile.label}</span></label>)}</div>
        {!ids.length ? <p>Select at least one account.</p> : !current ? <p role="status">Checking saved account data…</p> : current.failed ? <div role="alert"><p>Account data could not be checked.</p><Button variant="outline" onClick={() => setAttempt(value => value + 1)}>Retry</Button></div> : <>
          <p data-combined-availability>These accounts cannot be combined yet.</p>
          <ul className="combined-exclusions">{current.value?.exclusions.map(exclusion => <li key={exclusion.profileId}><bdi>{profiles.find(profile => profile.id === exclusion.profileId)?.label ?? 'Account'}</bdi><span>{reasons[exclusion.reason]}</span></li>)}</ul>
          <Button variant="outline" onClick={() => setScope('individual')}>View individual activity</Button>
          <details className="combined-explanation"><summary>Why are totals unavailable?</summary><p>The current OpenAI connection does not provide the verified account identity needed to prevent counting the same account twice. Each account’s reported activity remains available.</p></details>
        </>}
      </section>}
  </div>;
}
