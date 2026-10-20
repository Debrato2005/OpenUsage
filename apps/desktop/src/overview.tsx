import { useEffect, useState } from 'react';
import { Button } from './components/ui/button';
import { Meter, MeterIndicator, MeterLabel, MeterTrack } from './components/ui/meter';
import { Select, SelectItem, SelectPopup, SelectTrigger, SelectValue } from './components/ui/select';
import { invoke, type ProfileRecord, type SavedProfileActivity, type SavedProfileQuota, type SavedQuotaWindow } from './contracts';
import './overview.css';

type Props = { profiles: ProfileRecord[]; revision: number; onAccounts: () => void; active?: boolean };
type Reading = { profileId: string; activity: SavedProfileActivity | null; quota: SavedProfileQuota | null; activityFailed: boolean; quotaFailed: boolean };
const dates = new Intl.DateTimeFormat('en-US', { timeZone: 'UTC', month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit', timeZoneName: 'short' });
const freshness = { fresh: 'Fresh', stale: 'Stale', unknown: 'Freshness unknown' };

function Observed({ value }: { value: SavedProfileActivity | SavedProfileQuota }) {
  return <p className="overview-meta">{freshness[value.freshness]} · <time dateTime={value.observedAtUtc}>{dates.format(new Date(value.observedAtUtc))}</time></p>;
}

function QuotaWindow({ value, role }: { value: SavedQuotaWindow | null; role: string }) {
  if (!value) return <p className="overview-meta">{role}: Not available</p>;
  const reset = value.resetsAtUnix === null ? null : new Date(value.resetsAtUnix * 1000);
  const resetState = { expected: null, due: 'Reset due; awaiting confirmation', unconfirmed: 'Reset status not confirmed', confirmed: 'Reset confirmed' }[value.resetState];
  return <div className="overview-quota-window" data-overview-window={role}>
    <Meter min={0} max={100} value={value.derivedRemainingPercent} aria-label={`${value.label}: ${value.derivedRemainingPercent}% remaining (derived)`} aria-valuetext={`${value.derivedRemainingPercent}% remaining (derived)`}>
      <div className="overview-meter-heading"><MeterLabel>{value.label}</MeterLabel><strong>{value.derivedRemainingPercent}% left</strong></div>
      <MeterTrack><MeterIndicator /></MeterTrack>
    </Meter>
    <p className="overview-meta">Derived remaining · {value.usedPercent}% used</p>
    <p className="overview-meta">Expected reset {reset ? <time dateTime={reset.toISOString()}>{dates.format(reset)}</time> : 'not supplied'}</p>
    {resetState && <p className="overview-meta" data-overview-reset-state={value.resetState}>{resetState}</p>}
  </div>;
}

export function Overview({ profiles, revision, onAccounts, active = true }: Props) {
  const activeProfiles = profiles.filter(profile => !profile.archived);
  const [selection, setSelection] = useState<string | null>(null);
  const [limitSelection, setLimitSelection] = useState<string | null>(null);
  const selected = activeProfiles.find(profile => profile.id === selection) ?? activeProfiles[0];
  const profileId = selected?.id;
  const [reading, setReading] = useState<Reading | null>(null);
  const [loading, setLoading] = useState(false);
  const [refresh, setRefresh] = useState(0);
  useEffect(() => {
    if (!active || !profileId) return;
    const timer = window.setInterval(() => setRefresh(value => value + 1), 60_000);
    return () => window.clearInterval(timer);
  }, [active, profileId]);
  useEffect(() => {
    if (!profileId) { setReading(null); setLoading(false); return; }
    if (!active) { setLoading(false); return; }
    let current = true;
    setLoading(true);
    const activity = invoke('load_profile_activity', { profileId }).then(value => {
      if (value && value.profileId !== profileId) throw new Error('Incompatible saved activity profile.');
      return value;
    });
    const quota = invoke('load_profile_quota', { profileId }).then(value => {
      if (value && value.profileId !== profileId) throw new Error('Incompatible saved quota profile.');
      return value;
    });
    void Promise.allSettled([activity, quota]).then(([activity, quota]) => {
      if (!current) return;
      setReading(previous => {
        const retained = previous?.profileId === profileId ? previous : null;
        let nextActivity = activity.status === 'fulfilled' ? activity.value : retained?.activity ? { ...retained.activity, freshness: 'unknown' as const } : null;
        let nextQuota = quota.status === 'fulfilled' ? quota.value : retained?.quota ? { ...retained.quota, freshness: 'unknown' as const } : null;
        if (activity.status === 'rejected' && quota.status === 'fulfilled' && (!quota.value || nextActivity?.bindingId !== quota.value.bindingId)) nextActivity = null;
        if (quota.status === 'rejected' && activity.status === 'fulfilled' && (!activity.value || nextQuota?.bindingId !== activity.value.bindingId)) nextQuota = null;
        const mixed = !!nextActivity && !!nextQuota && nextActivity.bindingId !== nextQuota.bindingId;
        return {
          profileId,
          activity: mixed ? null : nextActivity,
          quota: mixed ? null : nextQuota,
          activityFailed: mixed || activity.status === 'rejected', quotaFailed: mixed || quota.status === 'rejected',
        };
      });
      setLoading(false);
    });
    return () => { current = false; };
  }, [profileId, revision, refresh, active]);
  const saved = reading?.profileId === profileId ? reading : null;
  const pending = loading || (active && !!profileId && !saved);
  const activity = saved?.activity;
  const quota = saved?.quota;
  const limit = quota?.limits.find(limit => `${profileId}:${limit.limitId}` === limitSelection) ?? quota?.limits[0];
  const firstWindow = limit?.primary ?? limit?.secondary;
  const connectionLabels = { connected: 'Connected', authentication_required: 'Authentication required', unauthorized: 'Unauthorized', offline: 'Offline', unsupported: 'Unsupported', error: 'Error' };
  return <section className="overview-dashboard" aria-label="Usage overview" data-overview data-overview-profile={profileId} aria-busy={pending}>
    {pending && <p className="overview-meta" role="status" data-overview-loading>Loading saved readings…</p>}
    <div className="overview-panels">
      <section className="overview-panel" aria-labelledby="overview-activity-heading" data-overview-activity>
        <div className="overview-panel-heading"><h2 id="overview-activity-heading">Token activity</h2><span className="overview-meta">{selected?.label ?? 'No profile selected'}</span></div>
        <div className="overview-total"><p className="overview-total-label">Lifetime tokens</p><p className={`overview-total-value${activity?.summary.lifetimeTokens == null ? ' overview-unavailable' : ''}`} data-overview-lifetime><bdi>{activity?.summary.lifetimeTokens ?? (pending ? 'Loading…' : 'Not available')}</bdi></p></div>
        {activity ? <><Observed value={activity} /><p className="overview-meta">Account activity · Scope and coverage unknown</p></> : <p className="overview-meta">{selected ? 'Read activity from Accounts to see saved values.' : 'Create a profile in Accounts to get started.'}</p>}
        {activity && <p className="overview-meta">{activity.sourceKind === 'test_fixture' && <span data-provider-fixture>Synthetic test data · </span>}Unverified identity · Saved locally</p>}
        {!activity && <Button type="button" variant="ghost" className="overview-empty-action" onClick={onAccounts}>Open Accounts</Button>}
        {saved?.activityFailed && <p className="overview-error">Saved activity could not be loaded. Reload it from Accounts.</p>}
        <p className="overview-note">Combined totals are not available. Profile readings are shown separately.</p>
      </section>
      <section className="overview-panel" aria-labelledby="overview-quota-heading" data-overview-quota>
        <div className="overview-panel-heading"><h2 id="overview-quota-heading">Account quota</h2>      <div className="overview-profile-picker"><label id="overview-profile-label" htmlFor="overview-profile">Profile</label>
        <Select modal={false} value={profileId ?? null} disabled={!selected} items={Object.fromEntries(activeProfiles.map(profile => [profile.id, profile.label]))} onValueChange={value => { if (value && activeProfiles.some(profile => profile.id === value)) setSelection(value); }}>
          <SelectTrigger id="overview-profile" aria-labelledby="overview-profile-label"><SelectValue placeholder="No active profiles" /></SelectTrigger>
          <SelectPopup alignItemWithTrigger={false}>{activeProfiles.map(profile => <SelectItem key={profile.id} value={profile.id}>{profile.label}</SelectItem>)}</SelectPopup>
        </Select>
      </div>
</div>
        {limit ? <div className="overview-limits"><div className="overview-limit" key={`${profileId}:${limit.limitId}`}>
          {quota && quota.limits.length > 1 ? <div className="overview-limit-picker"><label id="overview-limit-label" htmlFor="overview-limit">Quota limit</label>
            <Select modal={false} value={limit.limitId} items={Object.fromEntries(quota.limits.map(limit => [limit.limitId, limit.limitName ?? limit.limitId]))} onValueChange={value => { if (value && quota.limits.some(limit => limit.limitId === value)) setLimitSelection(`${profileId}:${value}`); }}>
              <SelectTrigger id="overview-limit" aria-labelledby="overview-limit-label"><SelectValue /></SelectTrigger><SelectPopup alignItemWithTrigger={false}>{quota.limits.map(limit => <SelectItem key={limit.limitId} value={limit.limitId}>{limit.limitName ?? limit.limitId}</SelectItem>)}</SelectPopup>
            </Select>
          </div> : <h3><bdi>{limit.limitName ?? limit.limitId}</bdi></h3>}
          <QuotaWindow value={limit.primary} role="Primary window" /><QuotaWindow value={limit.secondary} role="Secondary window" />
        </div></div> : <div className="overview-quota-empty"><p className="overview-unavailable">{pending ? 'Loading…' : 'Not available'}</p><p className="overview-meta">{quota ? 'No quota windows were supplied.' : 'Read quotas from Accounts to see saved limits.'}</p></div>}
        {quota && <Observed value={quota} />}
        {quota && <p className="overview-meta">{quota.sourceKind === 'test_fixture' && <span data-provider-fixture>Synthetic test data · </span>}Unverified identity · Saved locally</p>}
        {saved?.quotaFailed && <p className="overview-error">Saved quotas could not be loaded. Reload them from Accounts.</p>}
      </section>
    </div>
    <dl className="overview-summary" aria-label="Selected profile activity summary">
      <div><dt>Peak daily tokens</dt><dd><bdi>{activity?.summary.peakDailyTokens ?? 'Not available'}</bdi></dd></div>
      <div><dt>Current streak</dt><dd><bdi>{activity?.summary.currentStreakDays ?? 'Not available'}</bdi>{activity?.summary.currentStreakDays != null && <span> days</span>}</dd></div>
      <div><dt>Longest streak</dt><dd><bdi>{activity?.summary.longestStreakDays ?? 'Not available'}</bdi>{activity?.summary.longestStreakDays != null && <span> days</span>}</dd></div>
    </dl>
    {(activity || quota) && <p className="overview-meta overview-provenance">{activity?.sourceKind === 'test_fixture' || quota?.sourceKind === 'test_fixture' ? 'Synthetic test data · ' : ''}Unverified identity · Saved locally · Monitoring stopped</p>}
    <section className="overview-accounts" aria-labelledby="overview-accounts-heading" data-overview-accounts-table>
      <div className="overview-accounts-heading"><h2 id="overview-accounts-heading">Accounts</h2><Button type="button" variant="outline" onClick={onAccounts} data-overview-accounts>View accounts</Button></div>
      {activeProfiles.length > 0 ? <div className="overview-table-scroll"><table><thead><tr><th scope="col">Profile</th><th scope="col">Connection</th><th scope="col">Quota</th><th scope="col">Last quota read</th></tr></thead>
        <tbody>{activeProfiles.map(profile => <tr key={profile.id} data-overview-account={profile.id}>
          <th scope="row"><Button type="button" variant="ghost" onClick={() => setSelection(profile.id)} aria-pressed={profile.id === profileId} className="overview-account-link">{profile.label}</Button></th>
          <td>{connectionLabels[profile.connectionState]}</td>
          <td>{profile.id !== profileId ? 'Select profile' : firstWindow ? <><span>{firstWindow.derivedRemainingPercent}% left (derived)</span><small>{firstWindow.label}</small></> : pending ? 'Loading…' : 'Not available'}</td>
          <td>{profile.id !== profileId ? 'Select profile' : quota ? <><time dateTime={quota.observedAtUtc}>{dates.format(new Date(quota.observedAtUtc))}</time><small>{freshness[quota.freshness]}</small></> : pending ? 'Loading…' : 'Not available'}</td>
        </tr>)}</tbody>
      </table></div> : <p className="overview-meta">No active profiles. Create a profile in Accounts to get started.</p>}
    </section>
  </section>;
}
