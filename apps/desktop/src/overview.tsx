import { useDateFormat } from './time-display';
import type { AccountAction } from './account-actions';
import { useEffect, useState, type MouseEvent } from 'react';
import { Button } from './components/ui/button';
import { Collapsible, CollapsiblePanel, CollapsibleTrigger } from './components/ui/collapsible';
import { Meter, MeterIndicator, MeterLabel, MeterTrack } from './components/ui/meter';
import { Select, SelectItem, SelectPopup, SelectTrigger, SelectValue } from './components/ui/select';
import { invoke, type ProfileRecord, type SavedProfileActivity, type SavedProfileQuota, type SavedQuotaWindow } from './contracts';
import { ActivityPanel } from './activity-panel';
import { useActivityDashboard } from './activity-dashboard';
import './activity.css';
import './overview.css';
import { AccountDonut } from './account-donut';
import { ResetCountdown } from './reset-countdown';
import { ArrowIcon, ClockIcon, GaugeIcon, MoreIcon, PlusIcon, PulseIcon, RefreshIcon, SearchIcon, UsersIcon } from './icons';

type Props = { profiles: ProfileRecord[]; revision: number; onAccounts: () => void; onAccountAction?: (action: AccountAction) => void; busy?: boolean; active?: boolean; refreshingProfileIds?: string[]; refreshWaitSeconds?: Record<string, number>; expectedProfiles?: number };
type AccountQuota = { quota: SavedProfileQuota | null; loading: boolean; failed: boolean };
type Reading = { profileId: string; activity: SavedProfileActivity | null; quota: SavedProfileQuota | null; activityFailed: boolean; quotaFailed: boolean };
const scientificNumbers = new Intl.NumberFormat('en-US', { notation: 'scientific', maximumFractionDigits: 0 });
const compactNumbers = new Intl.NumberFormat('en-US', { notation: 'compact', maximumFractionDigits: 2 });
function CompactCount({ value }: { value: string | null }) {
  return <bdi title={value ?? undefined}>{value === null ? 'Not available' : (value.length > 15 ? scientificNumbers : compactNumbers).format(BigInt(value))}</bdi>;
}
const connectionLabels = { connected: 'Connected', authentication_required: 'Not connected', unauthorized: 'Session expired', offline: 'Offline', unsupported: 'Unavailable', error: 'Needs attention' };
function Connection({ profile }: { profile: ProfileRecord }) {
  return <span className="overview-connection" data-connected={profile.connectionState === 'connected'}><i aria-hidden="true" />{connectionLabels[profile.connectionState]}</span>;
}
function Avatar({ label }: { label: string }) {
  const initial = /^Account ([A-J])$/.exec(label)?.[1] ?? Array.from(label)[0]?.toUpperCase() ?? 'A';
  const tint = Array.from(label).reduce((sum, character) => sum + character.codePointAt(0)!, 0) % 6;
  return <span className="overview-avatar" data-tint={tint} aria-hidden="true">{initial}</span>;
}
const freshness = { fresh: 'Fresh', stale: 'Stale', unknown: 'Freshness unknown' };

function Observed({ value }: { value: SavedProfileActivity | SavedProfileQuota }) {
  const dates = useDateFormat();
  return <p className="overview-meta">{freshness[value.freshness]} · <time dateTime={value.observedAtUtc}>{dates.format(new Date(value.observedAtUtc))}</time></p>;
}

function QuotaWindow({ value, role }: { value: SavedQuotaWindow | null; role: string }) {
  const dates = useDateFormat();
  if (!value) return null;
  const reset = value.resetsAtUnix === null ? null : new Date(value.resetsAtUnix * 1000);
  const resetState = { expected: null, due: 'Reset due; awaiting confirmation', unconfirmed: 'Reset status not confirmed', confirmed: 'Reset confirmed' }[value.resetState];
  return <div className="overview-quota-window" data-overview-window={role} data-attention={attention(value.derivedRemainingPercent)}>
    <Meter min={0} max={100} value={value.derivedRemainingPercent} aria-label={`${value.label}: ${value.derivedRemainingPercent}% remaining (derived)`} aria-valuetext={`${value.derivedRemainingPercent}% remaining (derived)`}>
      <div className="overview-meter-heading"><MeterLabel>{value.label} quota</MeterLabel><strong>{value.derivedRemainingPercent}% left</strong></div>
      <MeterTrack><MeterIndicator /></MeterTrack>
    </Meter>
    <div className="overview-quota-footnote"><p><strong>{value.usedPercent}% used</strong><span>Derived from provider data</span></p><p className="overview-reset"><svg viewBox="0 0 24 24" aria-hidden="true"><rect x="4" y="5" width="16" height="16" rx="2"/><path d="M8 3v4m8-4v4M4 11h16"/></svg><span>Expected reset<time dateTime={reset?.toISOString()}>{reset ? dates.format(reset) : 'Not supplied'}</time><ResetCountdown resetsAtUnix={value.resetsAtUnix} /></span></p></div>
    {resetState && <p className="overview-meta" data-overview-reset-state={value.resetState}>{resetState}</p>}
  </div>;
}

const lowQuota = (window: SavedQuotaWindow) => window.derivedRemainingPercent <= 10;
const attention = (remaining: number) => remaining <= 10 ? 'low' : remaining <= 25 ? 'warn' : undefined;
function quotaWindows(quota: SavedProfileQuota | null | undefined) {
  return (quota?.limits ?? []).flatMap(limit => [limit.primary, limit.secondary]).filter((window): window is SavedQuotaWindow => window !== null);
}
function age(observedAtUtc: string) {
  const minutes = Math.floor((Date.now() - Date.parse(observedAtUtc)) / 60_000);
  if (minutes < 1) return 'Just now';
  if (minutes < 60) return `${minutes} min ago`;
  return minutes < 1440 ? `${Math.floor(minutes / 60)} hr ago` : `${Math.floor(minutes / 1440)} d ago`;
}

// Totals never cross windows: each tile reports one account's own reading, or a count of accounts.
function OverviewGlance({ profiles, quotas, plans }: { profiles: ProfileRecord[]; quotas: Record<string, SavedProfileQuota | null | undefined>; plans: string[] }) {
  const connected = profiles.filter(profile => profile.connectionState === 'connected');
  const signIn = profiles.length - connected.length;
  const readings = connected.flatMap(profile => quotaWindows(quotas[profile.id]).map(window => ({ profile, window })));
  const lowest = readings.reduce<(typeof readings)[number] | null>((best, entry) => !best || entry.window.derivedRemainingPercent < best.window.derivedRemainingPercent ? entry : best, null);
  const now = Date.now();
  const upcoming = readings.filter(({ window }) => window.resetState === 'expected' && window.resetsAtUnix !== null && window.resetsAtUnix * 1000 > now);
  const reset = upcoming.reduce<(typeof readings)[number] | null>((best, entry) => !best || entry.window.resetsAtUnix! < best.window.resetsAtUnix! ? entry : best, null);
  const saved = connected.map(profile => quotas[profile.id]).filter((quota): quota is SavedProfileQuota => !!quota);
  const latest = saved.reduce<string | null>((value, quota) => value === null || Date.parse(quota.observedAtUtc) > Date.parse(value) ? quota.observedAtUtc : value, null);
  const outdated = saved.filter(quota => quota.freshness !== 'fresh').length;
  const counts = new Map<string, number>();
  for (const plan of plans) counts.set(plan, (counts.get(plan) ?? 0) + 1);
  const breakdown = [...counts].map(([plan, count]) => `${count} ${plan}`).join(' · ');
  return <dl className="overview-glance" aria-label="Summary across accounts" data-overview-glance>
    <div><dt><UsersIcon />Connected accounts</dt><dd>{connected.length}<small> of {profiles.length}</small></dd><dd className="overview-glance-note" data-attention={signIn > 0 ? 'warn' : undefined}>{signIn > 0 ? `${signIn} need${signIn === 1 ? 's' : ''} sign-in` : breakdown || 'All connected'}</dd></div>
    <div data-attention={lowest ? attention(lowest.window.derivedRemainingPercent) : undefined}><dt><GaugeIcon />Lowest quota left</dt><dd>{lowest ? `${lowest.window.derivedRemainingPercent}%` : <span className="overview-glance-empty">Not available</span>}</dd><dd className="overview-glance-note">{lowest ? <><bdi>{lowest.profile.label}</bdi> · {lowest.window.label} · derived</> : 'No saved quota yet'}</dd></div>
    <div><dt><ClockIcon />Next expected reset</dt><dd>{reset ? <ResetCountdown detached resetsAtUnix={reset.window.resetsAtUnix} /> : <span className="overview-glance-empty">Not available</span>}</dd><dd className="overview-glance-note">{reset ? <><bdi>{reset.profile.label}</bdi> · {reset.window.label}</> : 'No reset time reported'}</dd></div>
    <div><dt><PulseIcon />Latest reading</dt><dd>{latest ? <time dateTime={latest}>{age(latest)}</time> : <span className="overview-glance-empty">Not available</span>}</dd><dd className="overview-glance-note" data-attention={outdated > 0 ? 'warn' : undefined}>{!latest ? 'Nothing saved yet' : outdated > 0 ? `${outdated} not fresh` : 'All readings fresh'}</dd></div>
  </dl>;
}

const accountFilters = { all: 'All accounts', connected: 'Connected', attention: 'Needs sign-in', low: 'Low quota' };
type AccountFilter = keyof typeof accountFilters;

function taskDuration(seconds: string | null | undefined) {
  if (seconds === null || seconds === undefined) return '—';
  const total = BigInt(seconds), hours = total / 3600n, minutes = total % 3600n / 60n;
  return hours ? `${hours}h ${minutes}m` : minutes ? `${minutes}m ${total % 60n}s` : `${total}s`;
}

function StatIcon({ kind }: { kind: 'tokens' | 'peak' | 'current' | 'longest' | 'task' | 'credit' }) {
  return <span className="overview-stat-icon" data-kind={kind} aria-hidden="true"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
    {kind === 'task' ? <><circle cx="12" cy="13" r="8"/><path d="M12 9v4l2.5 2M9 2h6"/></> : kind === 'credit' ? <><path d="M21 12a9 9 0 1 1-2.6-6.4L21 8"/><path d="M21 3v5h-5"/></> : kind === 'tokens' ? <><ellipse cx="12" cy="5" rx="7" ry="3"/><path d="M5 5v6c0 4 14 4 14 0V5M5 11v6c0 4 14 4 14 0v-6"/></> : kind === 'peak' ? <><rect x="4" y="12" width="3" height="8" rx=".5"/><rect x="10.5" y="5" width="3" height="15" rx=".5"/><rect x="17" y="9" width="3" height="11" rx=".5"/></> : kind === 'current' ? <path d="m13 3-8 11h6l-1 7 9-12h-6z"/> : <><rect x="4" y="5" width="16" height="16" rx="2"/><path d="M8 3v4m8-4v4M4 11h16m-11 4h2m2 0h2m-6 3h2"/></>}
  </svg></span>;
}

export function Overview({ profiles, revision, onAccounts, onAccountAction, busy = false, active = true, refreshingProfileIds = [], refreshWaitSeconds = {}, expectedProfiles = 0 }: Props) {
  const dates = useDateFormat();
  const activeProfiles = profiles.filter(profile => !profile.archived);
  const tableProfileIds = activeProfiles.map(profile => profile.id).join(',');
  const [accountQuotas, setAccountQuotas] = useState<Record<string, AccountQuota>>({});
  const [plans, setPlans] = useState<Record<string, { bindingId: string; plan: string | null }>>({});
  const [selection, setSelection] = useState<string | null>(null);
  const [limitSelection, setLimitSelection] = useState<string | null>(null);
  const [query, setQuery] = useState('');
  const [accountFilter, setAccountFilter] = useState<AccountFilter>('all');
  const selected = activeProfiles.find(profile => profile.id === selection) ?? activeProfiles[0];
  const profileId = selected?.id;
  const dashboard = useActivityDashboard(activeProfiles, profileId ?? null, revision, active);
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
  const act = (action: AccountAction) => onAccountAction ? onAccountAction(action) : onAccounts();
  const planFor = (id: string, bindingId: string | undefined) => {
    const plan = plans[id];
    if (!plan || plan.bindingId !== bindingId || !plan.plan) return 'Not reported';
    const names: Record<string, string> = { plus: 'ChatGPT Plus', pro: 'ChatGPT Pro', free: 'ChatGPT Free', team: 'ChatGPT Team', business: 'ChatGPT Business', enterprise: 'ChatGPT Enterprise', edu: 'ChatGPT Edu' };
    return names[plan.plan] ?? plan.plan;
  };
  const synthetic = activity?.sourceKind === 'test_fixture' || quota?.sourceKind === 'test_fixture';
  const loadedQuotas = Object.fromEntries(activeProfiles.map(profile => [profile.id, profile.id === profileId && saved ? quota : accountQuotas[profile.id]?.quota]));
  const search = query.trim().toLocaleLowerCase();
  const shownProfiles = activeProfiles.filter(profile => {
    if (search && !profile.label.toLocaleLowerCase().includes(search)) return false;
    if (accountFilter === 'connected') return profile.connectionState === 'connected';
    if (accountFilter === 'attention') return profile.connectionState !== 'connected';
    return accountFilter !== 'low' || quotaWindows(loadedQuotas[profile.id]).some(lowQuota);
  });
  const selectRow = (event: MouseEvent<HTMLTableRowElement>, id: string) => { if (!(event.target as HTMLElement).closest('button')) setSelection(id); };
  const selectedRefreshing = !!profileId && refreshingProfileIds.includes(profileId);
  const selectedWait = profileId ? refreshWaitSeconds[profileId] ?? 0 : 0;
  const needsSignIn = selected && (selected.connectionState === 'authentication_required' || selected.connectionState === 'unauthorized');
  return <section className="overview-dashboard" aria-label="Usage overview" data-overview data-overview-profile={profileId} aria-busy={pending}>
    {pending && <p className="overview-meta overview-loading" role="status" data-overview-loading>Loading saved readings…</p>}
    {synthetic && <p className="overview-fixture-label">Synthetic test data</p>}
    {activeProfiles.length === 0 ? expectedProfiles > 0 ? <p className="overview-meta overview-loading" role="status">Loading accounts…</p> : <section className="overview-welcome" aria-labelledby="overview-welcome-heading" data-overview-welcome>
      <span className="overview-welcome-mark" aria-hidden="true"><GaugeIcon /></span>
      <div className="overview-quota-empty"><h2 id="overview-welcome-heading">No accounts</h2><p>Add an account to see its quota, resets and token activity here.</p><Button disabled={busy} onClick={() => act({ kind: 'add' })} data-overview-add-account><PlusIcon />Add OpenAI account</Button></div>
    </section> : <>
    <OverviewGlance profiles={activeProfiles} quotas={loadedQuotas} plans={activeProfiles.map(profile => planFor(profile.id, loadedQuotas[profile.id]?.bindingId)).filter(plan => plan !== 'Not reported')} />
    <section className="overview-accounts" aria-labelledby="overview-accounts-heading" data-overview-accounts-table>
      <div className="overview-accounts-heading"><h2 id="overview-accounts-heading">Accounts <span>{activeProfiles.length}</span></h2>
        {activeProfiles.length > 2 && <div className="overview-accounts-tools" role="search" aria-label="Find accounts">
          <label className="overview-search"><SearchIcon /><span className="sr-only">Search accounts</span><input type="search" placeholder="Search accounts" autoComplete="off" value={query} onChange={event => setQuery(event.target.value)} /></label>
          <Select modal={false} value={accountFilter} items={accountFilters} onValueChange={value => { if (value) setAccountFilter(value); }}><SelectTrigger aria-label="Filter accounts"><SelectValue /></SelectTrigger><SelectPopup alignItemWithTrigger={false}>{(Object.keys(accountFilters) as AccountFilter[]).map(value => <SelectItem key={value} value={value}>{accountFilters[value]}</SelectItem>)}</SelectPopup></Select>
        </div>}
        <Button type="button" variant="default" disabled={busy || activeProfiles.length >= 10} onClick={() => act({ kind: 'add' })} data-overview-add-account><PlusIcon />Add account</Button></div>
      {shownProfiles.length > 0 ? <div className="overview-table-scroll" tabIndex={0} role="region" aria-label="Account readings"><table><thead><tr><th scope="col">Account</th><th scope="col">Quota remaining</th><th scope="col">Status</th><th scope="col">Plan</th><th scope="col">Last updated</th><th scope="col"><span className="sr-only">Actions</span></th></tr></thead>
        <tbody>{shownProfiles.map(profile => {
          const selectedRow = profile.id === profileId;
          const cached = accountQuotas[profile.id];
          const rowQuota = selectedRow ? quota : cached?.quota;
          const rowLimit = selectedRow ? limit : rowQuota?.limits.find(limit => limit.primary || limit.secondary);
          const rowWindows = rowLimit ? [rowLimit.primary, rowLimit.secondary].filter((window): window is SavedQuotaWindow => window !== null) : [];
          const rowLoading = selectedRow ? pending : active && (!cached || cached.loading);
          const rowFailed = selectedRow ? saved?.quotaFailed : cached?.failed;
          const refreshing = refreshingProfileIds.includes(profile.id);
          const refreshWait = refreshWaitSeconds[profile.id] ?? 0;
          const signin = profile.connectionState === 'authentication_required' || profile.connectionState === 'unauthorized';
          return <tr key={profile.id} data-overview-account={profile.id} data-selected={selectedRow} onClick={event => selectRow(event, profile.id)}>
            <th scope="row"><Button type="button" variant="ghost" onClick={() => setSelection(profile.id)} aria-pressed={selectedRow} className="overview-account-link"><Avatar label={profile.label} /><span>{profile.label}{rowQuota?.sourceKind === 'test_fixture' && <small>Synthetic test data</small>}</span></Button></th>
            <td data-overview-account-quota>{rowWindows.length ? <div className="overview-table-quotas">{rowWindows.map(window => <div className="overview-table-quota" key={window.label} data-attention={attention(window.derivedRemainingPercent)}><span title={`${window.usedPercent}% used; remaining is derived`}>{window.derivedRemainingPercent}% left <small>{window.label}</small></span><Meter min={0} max={100} value={window.derivedRemainingPercent} aria-label={`${profile.label}: ${window.derivedRemainingPercent}% remaining (derived)`}><MeterTrack><MeterIndicator /></MeterTrack></Meter></div>)}</div> : rowLoading ? 'Loading…' : 'Not available'}</td>
            <td><Connection profile={profile} /></td>
            <td className="overview-plan">{planFor(profile.id, rowQuota?.bindingId)}</td>
            <td data-overview-account-observed>{rowQuota ? <><time dateTime={rowQuota.observedAtUtc}>{dates.format(new Date(rowQuota.observedAtUtc))}</time>{rowQuota.freshness !== 'fresh' && <small>{freshness[rowQuota.freshness]}</small>}</> : rowLoading ? 'Loading…' : rowFailed ? 'Unavailable' : 'Never'}{rowFailed && <small className="overview-error">Could not reload saved quota</small>}</td>
            <td><div className="overview-row-actions"><Button variant={signin ? 'default' : 'outline'} disabled={busy} onClick={() => act({ kind: signin ? 'signin' : 'usage', profileId: profile.id })}>{signin ? profile.connectionState === 'unauthorized' ? 'Reconnect' : 'Sign in' : 'View usage'}</Button>{profile.connectionState === 'connected' && <Button variant="ghost" className="overview-row-refresh" disabled={busy || refreshing || refreshWait > 0} aria-label={`${refreshing ? 'Refreshing' : refreshWait > 0 ? `Refresh in ${refreshWait}s` : 'Refresh'} ${profile.label}`} title={refreshWait > 0 ? `Refresh in ${refreshWait}s` : 'Refresh'} aria-busy={refreshing} data-overview-row-refresh={profile.id} onClick={() => act({ kind: 'refresh', profileId: profile.id })}><RefreshIcon />{refreshing ? 'Refreshing…' : refreshWait > 0 ? `${refreshWait}s` : ''}</Button>}<Button variant="outline" className="overview-more" disabled={busy} aria-label={`Manage ${profile.label}`} title="Manage" onClick={() => act({ kind: 'manage', profileId: profile.id })}><MoreIcon /></Button></div></td>
          </tr>;
        })}</tbody>
      </table></div> : <div className="overview-no-match"><p role="status">No accounts match.</p><Button variant="outline" onClick={() => { setQuery(''); setAccountFilter('all'); }}>Clear filters</Button></div>}
    </section>
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
            {selected && <p className="overview-meta">{planFor(selected.id, quota?.bindingId)}</p>}
          </div>
          <div className="overview-account-state">{selected && <Connection profile={selected} />}{quota && <span className="overview-meta">{freshness[quota.freshness]}</span>}</div>
          <div className="overview-account-actions">{selected && <><Button variant={needsSignIn ? 'default' : 'outline'} disabled={busy || selectedRefreshing || (!needsSignIn && selectedWait > 0)} aria-busy={selectedRefreshing} data-overview-refresh={needsSignIn ? undefined : selected.id} onClick={() => act({ kind: needsSignIn ? 'signin' : 'refresh', profileId: selected.id })}>{!needsSignIn && <RefreshIcon />}{selectedRefreshing ? 'Refreshing…' : needsSignIn ? selected.connectionState === 'unauthorized' ? 'Reconnect' : 'Sign in' : selectedWait > 0 ? `Refresh in ${selectedWait}s` : 'Refresh'}</Button><Button variant="outline" className="overview-more" aria-label={`Manage ${selected.label}`} title="Manage" disabled={busy} onClick={() => act({ kind: 'manage', profileId: selected.id })}><MoreIcon /></Button></>}</div>
        </div>
        {limit ? <div className="overview-quota-block" data-window-count={Number(!!limit.primary) + Number(!!limit.secondary)}>
          <div className="overview-block-title"><h3 className="sr-only">Quota remaining</h3>

            {quota && quota.limits.length > 1 ? <div className="overview-limit-picker"><label id="overview-limit-label" htmlFor="overview-limit" className="sr-only">Quota limit</label>
              <Select modal={false} value={limit.limitId} items={Object.fromEntries(quota.limits.map(limit => [limit.limitId, limit.limitName ?? limit.limitId]))} onValueChange={value => { if (value && quota.limits.some(limit => limit.limitId === value)) setLimitSelection(`${profileId}:${value}`); }}>
                <SelectTrigger id="overview-limit" aria-labelledby="overview-limit-label"><SelectValue /></SelectTrigger><SelectPopup alignItemWithTrigger={false}>{quota.limits.map(limit => <SelectItem key={limit.limitId} value={limit.limitId}>{limit.limitName ?? limit.limitId}</SelectItem>)}</SelectPopup>
              </Select>
            </div> : <span className="sr-only"><bdi>{limit.limitName ?? limit.limitId}</bdi></span>}
          </div>
          <QuotaWindow value={limit.primary} role="Primary window" /><QuotaWindow value={limit.secondary} role="Secondary window" />
        </div> : <div className="overview-quota-empty"><h3>{selected ? needsSignIn ? 'Connect to view usage' : quota ? 'No quota windows reported' : 'No saved quota yet' : 'No accounts'}</h3>{!selected && <Button onClick={() => act({ kind: 'add' })}>Add OpenAI account</Button>}</div>}
        {saved?.quotaFailed && <p className="overview-error">Saved quotas could not be loaded. Try Refresh.</p>}
        <dl className="overview-summary" aria-label="Selected profile activity summary">
          <div><dt><StatIcon kind="tokens"/>Lifetime tokens</dt><dd><span data-overview-lifetime data-exact-value={lifetimeTokens ?? undefined}>{pending && lifetimeTokens === null ? 'Loading…' : <CompactCount value={lifetimeTokens} />}</span></dd></div>
          <div><dt><StatIcon kind="peak"/>Peak daily tokens</dt><dd><CompactCount value={activity?.summary.peakDailyTokens ?? null} /></dd></div>
          <div><dt><StatIcon kind="current"/>Current streak</dt><dd>{activity?.summary.currentStreakDays ?? '—'}{activity?.summary.currentStreakDays != null && <span> days</span>}</dd></div>
          <div><dt><StatIcon kind="longest"/>Longest streak</dt><dd>{activity?.summary.longestStreakDays ?? '—'}{activity?.summary.longestStreakDays != null && <span> days</span>}</dd></div>
          <div><dt><StatIcon kind="task"/>Longest task</dt><dd>{taskDuration(activity?.summary.longestRunningTurnSec)}</dd></div>
          <div><dt><StatIcon kind="credit"/>Reset credits</dt><dd>{quota?.availableResetCredits ?? '—'}</dd></div>
        </dl>
        {saved?.activityFailed && <p className="overview-error" role="alert">Saved activity could not be loaded. Try Refresh.</p>}
        <div className="overview-account-footer">{quota && <Observed value={quota} />}{selected && <Button variant="ghost" onClick={() => act({ kind: 'usage', profileId: selected.id })}>View usage<ArrowIcon /></Button>}
        {(quota || activity) && <Collapsible className="overview-source-details"><CollapsibleTrigger render={<Button variant="ghost" />} type="button">Reading details</CollapsibleTrigger><CollapsiblePanel><p className="overview-meta">Unverified identity · Saved locally. Remaining quota is derived from provider-reported usage. Combined totals are not available.</p>{quota && <Observed value={quota} />}{activity && <p className="overview-meta">Lifetime tokens: <bdi>{lifetimeTokens ?? 'Not available'}</bdi>. Peak daily tokens: <bdi>{activity.summary.peakDailyTokens ?? 'Not available'}</bdi>. Account activity scope and coverage are unknown.</p>}</CollapsiblePanel></Collapsible>}</div>
      </section>
      <AccountDonut label={selected?.label ?? 'No account selected'} window={limit?.primary ?? limit?.secondary ?? null} dashboard={dashboard} />
      <section className="overview-panel overview-activity-panel" aria-label="Token activity" data-overview-activity>
        <ActivityPanel profiles={activeProfiles} profileId={active ? profileId ?? null : null} revision={revision} dashboard={dashboard} />

      </section>
    </div>
    </>}
  </section>;
}
