import { Button } from './components/ui/button';
import { Checkbox } from './components/ui/checkbox';
import { Select, SelectTrigger, SelectValue, SelectPopup, SelectItem } from './components/ui/select';
import type { ProfileRecord } from './contracts';
import type { ActivityDashboardState } from './activity-dashboard';
import { TokenHeatmap } from './token-heatmap';

const reasons = {
  archived: 'Archived', not_connected: 'Not connected', activity_unavailable: 'No saved activity',
  truncated_history: 'Saved history exceeds the read limit', identity_unverified: 'Account identity is unverified',
  scope_unverified: 'Activity scope is unverified', incompatible_scope: 'Different activity definition', duplicate_identity: 'Duplicate account excluded',
};
type Props = { profiles: ProfileRecord[]; profileId: string | null; revision: number; onSelect: (id: string) => void; dashboard: ActivityDashboardState };
export function ActivityPanel({ profiles, profileId, revision, onSelect, dashboard: d }: Props) {
  return <div className="activity-panel" data-activity-scope={d.view}>
    <div className="activity-scope-controls">
      <div role="group" aria-label="Activity accounts">
        <Button variant="ghost" aria-pressed={d.view === 'individual'} onClick={() => d.setView('individual')}>Individual</Button>
        <Button variant="ghost" aria-pressed={d.view === 'profile_sum'} onClick={() => d.setView('profile_sum')}>Profile sum</Button>
        <Button variant="ghost" aria-pressed={d.view === 'verified'} onClick={() => d.setView('verified')}>Verified combined</Button>
      </div>
      {d.view === 'individual' && profiles.length > 0 && <Select modal={false} value={profileId} items={Object.fromEntries(profiles.map(p => [p.id,p.label]))} onValueChange={id => { if (id) onSelect(id); }}><SelectTrigger aria-label="Activity account"><SelectValue /></SelectTrigger><SelectPopup alignItemWithTrigger={false}>{profiles.map(p => <SelectItem key={p.id} value={p.id}>{p.label}</SelectItem>)}</SelectPopup></Select>}
    </div>
    {d.view !== 'individual' && <div className="combined-account-selection">{profiles.map(p => <label key={p.id}><Checkbox aria-label={`Include ${p.label}`} checked={d.ids.includes(p.id)} onCheckedChange={checked => d.setSelection(checked ? [...d.ids,p.id] : d.ids.filter(id => id !== p.id))} /><bdi>{p.label}</bdi></label>)}</div>}
    {d.view === 'profile_sum' && <details className="activity-sum-notice"><summary>Selected-profile sum · Unverified</summary><p>Account identities and activity coverage are unverified. Duplicate accounts or overlapping activity may be counted more than once. These are observed subtotals, not verified distinct-account totals.</p></details>}
    <details className="activity-date-range"><summary>Date range</summary><div><label>From<input aria-label="Activity from" type="date" value={d.range.start} max={d.range.end} onChange={e => d.setRange({ ...d.range,start:e.target.value })} /></label><label>To<input aria-label="Activity to" type="date" value={d.range.end} min={d.range.start} onChange={e => d.setRange({ ...d.range,end:e.target.value })} /></label><span>Up to 365 provider dates</span></div></details>
    {!d.validRange ? <p role="alert">Choose a valid range of 1–365 provider dates.</p> : !d.ids.length ? <p>Select at least one account.</p> : <TokenHeatmap compact profileId={profileId} profileLabel={d.view === 'individual' ? profiles.find(p => p.id === profileId)?.label ?? 'Account' : d.view === 'verified' ? 'Verified combined activity' : 'Selected-profile activity'} revision={revision} controller={d} />}
    {d.data?.status === 'unavailable' && <p data-combined-availability>{d.view === 'verified' ? 'Verified totals are unavailable for this selection.' : 'No eligible saved activity for this selection.'}</p>}
    {!!d.data?.exclusions.length && <details className="combined-explanation"><summary>{d.data.exclusions.length} excluded</summary><ul className="combined-exclusions">{d.data.exclusions.map(e => <li key={e.profileId}><bdi>{profiles.find(p => p.id === e.profileId)?.label ?? 'Account'}</bdi><span>{reasons[e.reason]}</span></li>)}</ul></details>}
  </div>;
}
