import type { AccountAction } from './account-actions';
import { useEffect, useState, type CSSProperties } from 'react';
import { Button } from './components/ui/button';
import { Collapsible, CollapsiblePanel, CollapsibleTrigger } from './components/ui/collapsible';
import { Meter, MeterIndicator, MeterLabel, MeterTrack } from './components/ui/meter';
import { Select, SelectItem, SelectPopup, SelectTrigger, SelectValue } from './components/ui/select';
import { invoke, type ProfileRecord, type SavedProfileActivity, type SavedProfileQuota, type SavedQuotaWindow } from './contracts';
import './activity.css';
import './overview.css';

type Props = { profiles: ProfileRecord[]; revision: number; onAccounts: () => void; onAccountAction?: (action: AccountAction) => void; busy?: boolean; active?: boolean };
type AccountQuota = { quota: SavedProfileQuota | null; loading: boolean; failed: boolean };
type Reading = { profileId: string; activity: SavedProfileActivity | null; quota: SavedProfileQuota | null; activityFailed: boolean; quotaFailed: boolean };
const dates = new Intl.DateTimeFormat('en-US', { timeZone: 'UTC', month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit', timeZoneName: 'short' });
const scientificNumbers = new Intl.NumberFormat('en-US', { notation: 'scientific', maximumFractionDigits: 2 });
const compactNumbers = new Intl.NumberFormat('en-US', { notation: 'compact', maximumFractionDigits: 2 });
function CompactCount({ value }: { value: string | null }) {
  return <bdi title={value ?? undefined}>{value === null ? 'Not available' : (value.length > 15 ? scientificNumbers : compactNumbers).format(BigInt(value))}</bdi>;
}
const connectionLabels = { connected: 'Connected', authentication_required: 'Not connected', unauthorized: 'Session expired', offline: 'Offline', unsupported: 'Unavailable', error: 'Needs attention' };
function Connection({ profile }: { profile: ProfileRecord }) {
  return <span className="overview-connection" data-connected={profile.connectionState === 'connected'}><i aria-hidden="true" />{connectionLabels[profile.connectionState]}</span>;
}
function Avatar({ label }: { label: string }) { return <span className="overview-avatar" aria-hidden="true">{Array.from(label)[0]?.toUpperCase() ?? 'A'}</span>; }
const freshness = { fresh: 'Fresh', stale: 'Stale', unknown: 'Freshness unknown' };

function Observed({ value }: { value: SavedProfileActivity | SavedProfileQuota }) {
  return <p className="overview-meta">{freshness[value.freshness]} · <time dateTime={value.observedAtUtc}>{dates.format(new Date(value.observedAtUtc))}</time></p>;
}

function DailyChart({ value }: { value: SavedProfileActivity }) {
  const rows = value.dailyUsageBuckets.slice(-30);
  const bars = value.dailyChart.bars.slice(-30);
  if (rows.length === 0) return <p className="overview-chart-empty" data-overview-chart-empty>{value.dailyState === 'unavailable' ? 'Daily usage was not supplied.' : 'No saved daily usage.'}</p>;
  return <div className="overview-daily" data-overview-daily-chart>
    <figure aria-labelledby="overview-daily-heading" aria-describedby="overview-daily-scope">
      <figcaption><h3 id="overview-daily-heading" className="sr-only">Saved daily usage</h3><p id="overview-daily-scope">{rows.length} saved {rows.length === 1 ? 'date' : 'dates'}{value.dailyTruncated ? ' · Partial saved data' : ''}</p></figcaption>
      <div className="overview-daily-plot" role="img" aria-label="Daily token bars. Exact dates, counts and freshness are available in Daily values.">
        {bars.map((bar, index) => <span key={bar.date} className="overview-daily-column" aria-hidden="true" data-overview-day={bar.date}>
          <span className="overview-daily-bar" data-zero={rows[index]!.tokens === '0' ? 'true' : undefined} style={{ '--bar-height': bar.heightBasisPoints } as CSSProperties} title={`${bar.date}: ${rows[index]!.tokens} tokens · ${freshness[rows[index]!.freshness]}`} />
        </span>)}
      </div>
      <div className="overview-daily-axis" aria-hidden="true"><span>{rows[0]!.date}</span>{rows.length > 1 && <span>{rows.at(-1)!.date}</span>}</div>
    </figure>
    {value.dailyState !== 'returned' && <p className="overview-meta">{value.dailyState === 'unavailable' ? 'The latest read did not supply daily usage.' : 'The latest read returned no daily rows.'}</p>}
    <Collapsible className="overview-daily-details">
      <CollapsibleTrigger render={<Button variant="ghost" className="text-button" />} type="button">Daily values</CollapsibleTrigger>
      <CollapsiblePanel>
        <p className="overview-meta">Bars use the same scale. A baseline mark means zero tokens. Scope and coverage are unknown.</p>
        <p className="overview-meta">Scale maximum <span className="activity-count" tabIndex={0} aria-label="Daily chart scale maximum"><bdi>{value.dailyChart.maxTokens}</bdi></span></p>
        <div className="overview-daily-table" tabIndex={0} role="region" aria-label="Saved daily usage values">
          <table><thead><tr><th scope="col">Date</th><th scope="col">Tokens</th><th scope="col">Freshness</th><th scope="col">Last read</th></tr></thead>
            <tbody>{rows.map(row => <tr key={row.date}><th scope="row"><time dateTime={row.date}>{row.date}</time></th><td><span className="activity-count" tabIndex={0} aria-label={`Tokens for ${row.date}`}><bdi>{row.tokens}</bdi></span></td><td>{freshness[row.freshness]}</td><td><time dateTime={row.observedAtUtc}>{dates.format(new Date(row.observedAtUtc))}</time></td></tr>)}</tbody>
          </table>
        </div>
      </CollapsiblePanel>
    </Collapsible>
  </div>;
}

function QuotaWindow({ value, role }: { value: SavedQuotaWindow | null; role: string }) {
  if (!value) return null;
  const reset = value.resetsAtUnix === null ? null : new Date(value.resetsAtUnix * 1000);
  const resetState = { expected: null, due: 'Reset due; awaiting confirmation', unconfirmed: 'Reset status not confirmed', confirmed: 'Reset confirmed' }[value.resetState];
  return <div className="overview-quota-window" data-overview-window={role} data-attention={value.derivedRemainingPercent <= 10 ? 'low' : undefined}>
    <Meter min={0} max={100} value={value.derivedRemainingPercent} aria-label={`${value.label}: ${value.derivedRemainingPercent}% remaining (derived)`} aria-valuetext={`${value.derivedRemainingPercent}% remaining (derived)`}>
      <div className="overview-meter-heading"><MeterLabel>{value.label}</MeterLabel><strong>{value.derivedRemainingPercent}% left</strong></div>
      <MeterTrack><MeterIndicator /></MeterTrack>
    </Meter>

    <p className="overview-meta">Expected reset {reset ? <time dateTime={reset.toISOString()}>{dates.format(reset)}</time> : 'not supplied'}</p>
    {resetState && <p className="overview-meta" data-overview-reset-state={value.resetState}>{resetState}</p>}
  </div>;
}

function QuotaChart({ value }: { value: SavedQuotaWindow }) {
  return <figure className="overview-quota-chart" data-overview-quota-chart data-attention={value.derivedRemainingPercent <= 10 ? 'low' : undefined} aria-label={`${value.label}: ${value.derivedRemainingPercent}% remaining, derived from ${value.usedPercent}% used`}>
    <svg viewBox="0 0 144 144" aria-hidden="true" focusable="false">
      <circle className="overview-quota-ring-track" cx="72" cy="72" r="58" fill="none" strokeWidth="15" />
      <circle className="overview-quota-ring-value" cx="72" cy="72" r="58" fill="none" strokeWidth="15" pathLength="100" strokeDasharray={`${value.derivedRemainingPercent} 100`} transform="rotate(-90 72 72)" />
      <text x="72" y="71" textAnchor="middle" className="overview-quota-ring-count">{value.derivedRemainingPercent}%</text>
      <text x="72" y="91" textAnchor="middle" className="overview-quota-ring-caption">left</text>
    </svg>
    <figcaption><strong>{value.label}</strong><span><i className="quota-legend-used" />{value.usedPercent}% used</span><span><i className="quota-legend-left" />{value.derivedRemainingPercent}% remaining (derived)</span></figcaption>
  </figure>;
}

export function Overview({ profiles, revision, onAccounts, onAccountAction, busy = false, active = true }: Props) {
  const activeProfiles = profiles.filter(profile => !profile.archived);
  const tableProfileIds = activeProfiles.map(profile => profile.id).join(',');
  const [accountQuotas, setAccountQuotas] = useState<Record<string, AccountQuota>>({});
  const [plans, setPlans] = useState<Record<string, { bindingId: string; plan: string | null }>>({});
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
  useEffect(() => {
    if (!active) return;
    let current = true;
    const ids = tableProfileIds ? tableProfileIds.split(',') : [];
    setAccountQuotas(previous => Object.fromEntries(ids.map(id => [id, {
      quota: previous[id]?.quota ?? null, loading: true, failed: false,
    }])));
    for (const id of ids) {
      void invoke('load_profile_quota', { profileId: id }).then(async value => {
        if (value && value.profileId !== id) throw new Error('Incompatible saved quota profile.');
        if (current) setAccountQuotas(previous => ({ ...previous, [id]: { quota: value, loading: false, failed: false } }));
        if (value) {
          const bindingId = value.bindingId;
          try {
            const plan = await invoke('load_profile_plan', { profileId: id, bindingId });
            if (current && plan.profileId === id && plan.bindingId === bindingId) setPlans(previous => ({ ...previous, [id]: { bindingId, plan: plan.plan } }));
          } catch { if (current) setPlans(previous => ({ ...previous, [id]: { bindingId, plan: null } })); }
        }
      }).catch(() => {
        if (current) setAccountQuotas(previous => {
          const retained = previous[id]?.quota;
          return { ...previous, [id]: {
            quota: retained ? { ...retained, freshness: 'unknown' as const } : null,
            loading: false, failed: true,
          } };
        });
      });
    }
    return () => { current = false; };
  }, [tableProfileIds, revision, refresh, active]);
  const saved = reading?.profileId === profileId ? reading : null;
  const pending = loading || (active && !!profileId && !saved);
  const activity = saved?.activity;
  const lifetimeTokens = activity?.summary.lifetimeTokens ?? null;
  const quota = saved?.quota;
  const limit = quota?.limits.find(limit => `${profileId}:${limit.limitId}` === limitSelection) ?? quota?.limits[0];
  const firstWindow = limit?.primary ?? limit?.secondary;
  const act = (action: AccountAction) => onAccountAction ? onAccountAction(action) : onAccounts();
  const planFor = (id: string, bindingId: string | undefined) => {
    const plan = plans[id];
    if (!plan || plan.bindingId !== bindingId || !plan.plan) return 'Not reported';
    const names: Record<string, string> = { plus: 'ChatGPT Plus', pro: 'ChatGPT Pro', free: 'ChatGPT Free', team: 'ChatGPT Team', business: 'ChatGPT Business', enterprise: 'ChatGPT Enterprise', edu: 'ChatGPT Edu' };
    return names[plan.plan] ?? plan.plan;
  };
  const synthetic = activity?.sourceKind === 'test_fixture' || quota?.sourceKind === 'test_fixture';
  const needsSignIn = selected && (selected.connectionState === 'authentication_required' || selected.connectionState === 'unauthorized');
  return <section className="overview-dashboard" aria-label="Usage overview" data-overview data-overview-profile={profileId} aria-busy={pending}>
    {pending && <p className="overview-meta" role="status" data-overview-loading>Loading saved readings…</p>}
    {synthetic && <p className="overview-fixture-label">Synthetic test data</p>}
    <div className="overview-panels">
      <section className="overview-panel overview-account-panel" aria-labelledby="overview-account-heading" data-overview-quota>
        <div className="overview-account-heading">
          <Avatar label={selected?.label ?? 'A'} />
          <div className="overview-account-identity">
            <h2 id="overview-account-heading" className="sr-only">Selected account</h2>
            <div className="overview-profile-picker"><label id="overview-profile-label" htmlFor="overview-profile" className="sr-only">Profile</label>
              <Select modal={false} value={profileId ?? null} disabled={!selected} items={Object.fromEntries(activeProfiles.map(profile => [profile.id, profile.label]))} onValueChange={value => { if (value && activeProfiles.some(profile => profile.id === value)) setSelection(value); }}>
                <SelectTrigger id="overview-profile" aria-labelledby="overview-profile-label"><SelectValue placeholder="No accounts" /></SelectTrigger>
                <SelectPopup alignItemWithTrigger={false}>{activeProfiles.map(profile => <SelectItem key={profile.id} value={profile.id}>{profile.label}</SelectItem>)}</SelectPopup>
              </Select>
            </div>
            <p className="overview-meta">{selected ? planFor(selected.id, quota?.bindingId) : 'Add your first OpenAI account'}</p>
          </div>
          {selected && <Button variant="ghost" className="overview-more" aria-label={`Manage ${selected.label}`} disabled={busy} onClick={() => act({ kind: 'manage', profileId: selected.id })}>•••</Button>}
        </div>
        <div className="overview-account-state">{selected && <Connection profile={selected} />}{selected && <Button variant={needsSignIn ? 'default' : 'ghost'} disabled={busy} onClick={() => act({ kind: needsSignIn ? 'signin' : 'refresh', profileId: selected.id })}>{needsSignIn ? selected.connectionState === 'unauthorized' ? 'Reconnect' : 'Sign in' : 'Refresh'}</Button>}</div>
        {limit ? <div className="overview-quota-block">
          <div className="overview-block-title"><h3>Quota remaining</h3>
            {quota && quota.limits.length > 1 ? <div className="overview-limit-picker"><label id="overview-limit-label" htmlFor="overview-limit" className="sr-only">Quota limit</label>
              <Select modal={false} value={limit.limitId} items={Object.fromEntries(quota.limits.map(limit => [limit.limitId, limit.limitName ?? limit.limitId]))} onValueChange={value => { if (value && quota.limits.some(limit => limit.limitId === value)) setLimitSelection(`${profileId}:${value}`); }}>
                <SelectTrigger id="overview-limit" aria-labelledby="overview-limit-label"><SelectValue /></SelectTrigger><SelectPopup alignItemWithTrigger={false}>{quota.limits.map(limit => <SelectItem key={limit.limitId} value={limit.limitId}>{limit.limitName ?? limit.limitId}</SelectItem>)}</SelectPopup>
              </Select>
            </div> : <span className="overview-meta"><bdi>{limit.limitName ?? limit.limitId}</bdi></span>}
          </div>
          <QuotaWindow value={limit.primary} role="Primary window" /><QuotaWindow value={limit.secondary} role="Secondary window" />
        </div> : <div className="overview-quota-empty"><h3>{selected ? needsSignIn ? 'Connect to view usage' : 'No saved quota yet' : 'Your accounts, in one place'}</h3><p className="overview-meta">{selected ? needsSignIn ? 'Sign in with your OpenAI account.' : quota ? 'No quota windows were supplied.' : 'Refresh to retrieve available readings.' : 'Connect an authorized OpenAI account to get started.'}</p>{!selected && <Button onClick={() => act({ kind: 'add' })}>Add OpenAI account</Button>}</div>}
        {saved?.quotaFailed && <p className="overview-error">Saved quotas could not be loaded. Reload them from Accounts.</p>}
        <dl className="overview-summary" aria-label="Selected profile activity summary">
          <div><dt>Lifetime tokens</dt><dd data-overview-lifetime data-exact-value={lifetimeTokens ?? undefined}>{pending && lifetimeTokens === null ? 'Loading…' : <CompactCount value={lifetimeTokens} />}</dd></div>
          <div><dt>Peak daily tokens</dt><dd><CompactCount value={activity?.summary.peakDailyTokens ?? null} /></dd></div>
          <div><dt>Current streak</dt><dd>{activity?.summary.currentStreakDays ?? '—'}{activity?.summary.currentStreakDays != null && <span> days</span>}</dd></div>
          <div><dt>Longest streak</dt><dd>{activity?.summary.longestStreakDays ?? '—'}{activity?.summary.longestStreakDays != null && <span> days</span>}</dd></div>
        </dl>
        <div className="overview-account-footer">{quota && <Observed value={quota} />}{selected && <Button variant="ghost" onClick={() => act({ kind: 'usage', profileId: selected.id })}>View usage <span aria-hidden="true">↗</span></Button>}</div>
      </section>
      <section className="overview-panel overview-ring-panel" aria-labelledby="overview-quota-heading">
        <div className="overview-panel-heading"><h2 id="overview-quota-heading">Quota overview</h2></div>
        {firstWindow ? <QuotaChart value={firstWindow} /> : <div className="overview-chart-placeholder"><span aria-hidden="true" className="overview-empty-ring" /><p className="overview-meta">{pending ? 'Loading…' : 'No quota available'}</p></div>}
        {(quota || activity) && <Collapsible className="overview-source-details"><CollapsibleTrigger render={<Button variant="ghost" />} type="button">Reading details</CollapsibleTrigger><CollapsiblePanel><p className="overview-meta">Unverified identity · Saved locally. Remaining quota is derived from provider-reported usage. Combined totals are not available.</p>{quota && <Observed value={quota} />}{activity && <p className="overview-meta">Lifetime tokens: <bdi>{lifetimeTokens ?? 'Not available'}</bdi>. Peak daily tokens: <bdi>{activity.summary.peakDailyTokens ?? 'Not available'}</bdi>. Account activity scope and coverage are unknown.</p>}</CollapsiblePanel></Collapsible>}
      </section>
      <section className="overview-panel overview-activity-panel" aria-labelledby="overview-activity-heading" data-overview-activity>
        <div className="overview-panel-heading"><h2 id="overview-activity-heading">Token activity</h2></div>
        <p className="overview-activity-value" title={lifetimeTokens ?? undefined}><CompactCount value={lifetimeTokens} /></p><p className="overview-meta">Reported lifetime tokens</p>
        {activity ? <DailyChart value={activity} /> : <div className="overview-chart-placeholder"><p className="overview-meta">{selected ? 'No saved daily activity yet.' : 'Connect an account to view activity.'}</p><Button variant="ghost" onClick={onAccounts}>Open Accounts</Button></div>}
        {saved?.activityFailed && <p className="overview-error">Saved activity could not be loaded. Reload it from Accounts.</p>}
      </section>
    </div>
    <section className="overview-accounts" aria-labelledby="overview-accounts-heading" data-overview-accounts-table>
      <div className="overview-accounts-heading"><div><h2 id="overview-accounts-heading">Accounts <span>{activeProfiles.length}</span></h2></div><Button type="button" variant="outline" disabled={busy || activeProfiles.length >= 10} onClick={() => act({ kind: 'add' })} data-overview-add-account><span aria-hidden="true">＋</span> Add account</Button></div>
      {activeProfiles.length > 0 ? <div className="overview-table-scroll" tabIndex={0} role="region" aria-label="Account readings"><table><thead><tr><th scope="col">Account</th><th scope="col">Status</th><th scope="col">Plan</th><th scope="col">Quota remaining</th><th scope="col">Updated</th><th scope="col"><span className="sr-only">Actions</span></th></tr></thead>
        <tbody>{activeProfiles.map(profile => {
          const selectedRow = profile.id === profileId;
          const cached = accountQuotas[profile.id];
          const rowQuota = selectedRow ? quota : cached?.quota;
          const rowLimit = selectedRow ? limit : rowQuota?.limits.find(limit => limit.primary || limit.secondary);
          const rowWindow = rowLimit?.primary ?? rowLimit?.secondary;
          const rowLoading = selectedRow ? pending : active && (!cached || cached.loading);
          const rowFailed = selectedRow ? saved?.quotaFailed : cached?.failed;
          const signin = profile.connectionState === 'authentication_required' || profile.connectionState === 'unauthorized';
          return <tr key={profile.id} data-overview-account={profile.id} data-selected={selectedRow}>
            <th scope="row"><Button type="button" variant="ghost" onClick={() => setSelection(profile.id)} aria-pressed={selectedRow} className="overview-account-link"><Avatar label={profile.label} /><span>{profile.label}</span></Button></th>
            <td><Connection profile={profile} /></td>
            <td className="overview-plan">{planFor(profile.id, rowQuota?.bindingId)}</td>
            <td data-overview-account-quota>{rowWindow ? <div className="overview-table-quota"><span title={`${rowWindow.usedPercent}% used; remaining is derived`}>{rowWindow.derivedRemainingPercent}% left</span><Meter min={0} max={100} value={rowWindow.derivedRemainingPercent} aria-label={`${profile.label}: ${rowWindow.derivedRemainingPercent}% remaining (derived)`}><MeterTrack><MeterIndicator /></MeterTrack></Meter><small>{rowWindow.label}</small></div> : rowLoading ? 'Loading…' : 'Not available'}{rowQuota?.sourceKind === 'test_fixture' && <small>Synthetic test data</small>}</td>
            <td data-overview-account-observed>{rowQuota ? <><time dateTime={rowQuota.observedAtUtc}>{dates.format(new Date(rowQuota.observedAtUtc))}</time>{rowQuota.freshness !== 'fresh' && <small>{freshness[rowQuota.freshness]}</small>}</> : rowLoading ? 'Loading…' : '—'}{rowFailed && <small className="overview-error">Could not reload saved quota</small>}</td>
            <td><div className="overview-row-actions"><Button variant={signin ? 'default' : 'ghost'} disabled={busy} onClick={() => act({ kind: signin ? 'signin' : 'usage', profileId: profile.id })}>{signin ? profile.connectionState === 'unauthorized' ? 'Reconnect' : 'Sign in' : 'View usage'}</Button><Button variant="ghost" className="overview-more" disabled={busy} aria-label={`Manage ${profile.label}`} onClick={() => act({ kind: 'manage', profileId: profile.id })}>•••</Button></div></td>
          </tr>;
        })}</tbody>
      </table></div> : <p className="overview-meta">Your connected accounts will appear here.</p>}
    </section>
  </section>;
}
