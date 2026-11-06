import type { ReactNode } from 'react';
import { Checkbox } from './components/ui/checkbox';
import type { ProfileRecord } from './contracts';
import type { ActivityDashboardState } from './activity-dashboard';
import { Mark } from './detail-panel';
import { TokenHeatmap } from './token-heatmap';

const reasons = {
  archived: 'Archived', not_connected: 'Not connected', activity_unavailable: 'No saved activity',
  truncated_history: 'Saved history exceeds the read limit', identity_unverified: 'Account identity is unverified',
  scope_unverified: 'Activity scope is unverified', incompatible_scope: 'Different activity definition', duplicate_identity: 'Duplicate account excluded',
};
type Props = { profiles: ProfileRecord[]; profileId: string | null; revision: number; dashboard: ActivityDashboardState; footer?: ReactNode };
// The year of activity for the accounts chosen at the top of Overview. The sum stays an explicit, marked choice (SC-010, SC-015).
export function ActivityPanel({ profiles, profileId, revision, dashboard: d, footer }: Props) {
  const dateRange = <details className="activity-date-range"><summary>Date range</summary><div><label>From<input aria-label="Activity from" type="date" value={d.range.start} max={d.range.end} onChange={e => d.setRange({ ...d.range,start:e.target.value })} /></label><label>To<input aria-label="Activity to" type="date" value={d.range.end} min={d.range.start} onChange={e => d.setRange({ ...d.range,end:e.target.value })} /></label><span>Up to 365 provider dates</span></div></details>;
  const unavailable = d.data?.status === 'unavailable' ? <span data-combined-availability>{d.view === 'verified' ? 'Verified totals are unavailable for this selection.' : 'No eligible saved activity for this selection.'}</span> : undefined;
  const selection = <>
    {d.view !== 'individual' && <div className="combined-account-selection">{profiles.map(p => <label key={p.id}><Checkbox aria-label={`Include ${p.label}`} checked={d.ids.includes(p.id)} onCheckedChange={checked => d.setSelection(checked ? [...d.ids,p.id] : d.ids.filter(id => id !== p.id))} /><bdi>{p.label}</bdi></label>)}</div>}
    {d.view === 'profile_sum' && <details className="activity-sum-notice"><summary>Sum of chosen accounts <Mark>Unverified</Mark></summary><p>Account identities and activity coverage are unverified. Duplicate accounts or overlapping activity may be counted more than once. These are observed subtotals, not verified distinct-account totals.</p></details>}
  </>;
  return <div className="activity-panel" data-activity-scope={d.view}>
    {!d.validRange ? <>{dateRange}<p role="alert">Choose a valid range of 1–365 provider dates.</p></> : !d.ids.length ? <>{selection}<p>Select at least one account.</p></> : <TokenHeatmap compact profileId={profileId} profileLabel={d.view === 'individual' ? profiles.find(p => p.id === profileId)?.label ?? 'Account' : d.view === 'verified' ? 'Verified accounts' : 'All chosen accounts'} revision={revision} controller={d} headerRows={selection} extraControls={dateRange} emptyMessage={unavailable} />}
    {unavailable && !!d.data?.cells.length && <p>{unavailable}</p>}
    {footer}
    {!!d.data?.exclusions.length && <details className="combined-explanation"><summary>{d.data.exclusions.length} excluded</summary><ul className="combined-exclusions">{d.data.exclusions.map(e => <li key={e.profileId}><bdi>{profiles.find(p => p.id === e.profileId)?.label ?? 'Account'}</bdi><span>{reasons[e.reason]}</span></li>)}</ul></details>}
  </div>;
}
