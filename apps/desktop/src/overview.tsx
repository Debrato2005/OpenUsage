import type { AccountAction } from './account-actions';
import { Fragment, useEffect, useRef, useState, type MouseEvent, type ReactNode } from 'react';
import { Button } from './components/ui/button';
import { Meter, MeterIndicator, MeterLabel, MeterTrack } from './components/ui/meter';
import { Select, SelectItem, SelectPopup, SelectTrigger, SelectValue } from './components/ui/select';
import { invoke, type ProfileRecord, type SavedProfileActivity, type SavedProfileQuota, type SavedQuotaWindow } from './contracts';
import { ActivityPanel } from './activity-panel';
import { useActivityDashboard, type ActivityDashboardState } from './activity-dashboard';
import './activity.css';
import './overview.css';
import { AccountDonut } from './account-donut';
import { Mark, useOpenDetail } from './detail-panel';
import { OverviewGlance, ReadingDetails } from './overview-glance';
import { ResetCountdown } from './reset-countdown';
import { useDateFormat } from './time-display';
import { GaugeIcon, MoreIcon, PlusIcon, RefreshIcon, SearchIcon } from './icons';

type Props = { profiles: ProfileRecord[]; revision: number; onAccounts: () => void; onAccountAction?: (action: AccountAction) => void; busy?: boolean; active?: boolean; refreshingProfileIds?: string[]; refreshWaitSeconds?: Record<string, number>; expectedProfiles?: number; notices?: ReactNode };
type AccountQuota = { quota: SavedProfileQuota | null; loading: boolean; failed: boolean };
type Reading = { profileId: string; activity: SavedProfileActivity | null; quota: SavedProfileQuota | null; activityFailed: boolean; quotaFailed: boolean };
const exactNumbers = new Intl.NumberFormat('en-US');
const compactNumbers = new Intl.NumberFormat('en-US', { notation: 'compact', maximumFractionDigits: 2 });
function ExactCount({ value }: { value: string | null }) {
  if (value === null) return <bdi>Not available</bdi>;
  // DM Sans tnum widens commas, so only the digit groups use tabular figures.
  return <bdi>{exactNumbers.format(BigInt(value)).split(',').map((group, index) => <Fragment key={index}>{index > 0 && ','}<span className="tabular-digits">{group}</span></Fragment>)}</bdi>;
}
const connectionLabels = { connected: 'Connected', authentication_required: 'Not connected', unauthorized: 'Session expired', offline: 'Offline', unsupported: 'Unavailable', error: 'Needs attention' };
function Avatar({ label }: { label: string }) {
  const initial = /^Account ([A-J])$/.exec(label)?.[1] ?? Array.from(label)[0]?.toUpperCase() ?? 'A';
  const tint = Array.from(label).reduce((sum, character) => sum + character.codePointAt(0)!, 0) % 6;
  return <span className="overview-avatar" data-tint={tint} aria-hidden="true">{initial}</span>;
}
const freshness = { fresh: 'Current', stale: 'Stale', unknown: 'Freshness unknown' };

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
    <dl className="detail-rows"><dt>Used</dt><dd>{value.usedPercent}%</dd><dt>Expected reset</dt><dd>{reset ? <time dateTime={reset.toISOString()}>{dates.format(reset)}</time> : 'Not available'}<ResetCountdown resetsAtUnix={value.resetsAtUnix} /></dd></dl>
    {resetState && <p data-overview-reset-state={value.resetState}><Mark>{resetState}</Mark></p>}
  </div>;
}

const lowQuota = (window: SavedQuotaWindow) => window.derivedRemainingPercent <= 10;
const attention = (remaining: number) => remaining <= 10 ? 'low' : remaining <= 25 ? 'warn' : undefined;
function quotaWindows(quota: SavedProfileQuota | null | undefined) {
  return (quota?.limits ?? []).flatMap(limit => [limit.primary, limit.secondary]).filter((window): window is SavedQuotaWindow => window !== null);
}
const accountFilters = { all: 'All accounts', connected: 'Connected', attention: 'Needs sign-in', low: 'Low quota' };
type AccountFilter = keyof typeof accountFilters;

function taskDuration(seconds: string | null | undefined) {
  if (seconds === null || seconds === undefined) return 'Not available';
  const total = BigInt(seconds), hours = total / 3600n, minutes = total % 3600n / 60n;
  return hours ? `${hours}h ${minutes}m` : minutes ? `${minutes}m ${total % 60n}s` : `${total}s`;
}

// Daily mode only: peak, lowest and active days among the reported days. Missing days are not zero.
function ActivityStats({ dashboard }: { dashboard: ActivityDashboardState }) {
  const openDetail = useOpenDetail();
  const data = dashboard.data;
  if (!data || data.mode !== 'daily') return null;
  const known = data.cells.filter(cell => cell.tokens !== null);
  if (!known.length) return null;
  const pick = (better: (a: bigint, b: bigint) => boolean) => known.reduce((best, cell) => better(BigInt(cell.tokens!), BigInt(best.tokens!)) ? cell : best);
  const peak = pick((a, b) => a > b), lowest = pick((a, b) => a < b);
  const active = known.filter(cell => BigInt(cell.tokens!) > 0n).length;
  const day = (name: string, cell: typeof peak) => <button type="button" className="overview-stat" aria-haspopup="dialog" data-overview-activity-stat={name} onClick={() => openDetail({ label: `${name} details`, title: name, content: <dl className="detail-rows"><dt>Date</dt><dd><time dateTime={cell.startDate}>{cell.startDate}</time></dd><dt>Tokens</dt><dd><bdi>{exactNumbers.format(BigInt(cell.tokens!))}</bdi></dd><dt>Period</dt><dd>{data.periodStart} – {data.periodEnd}</dd></dl> })}>
    <span className="overview-stat-name">{name}</span><span className="overview-stat-value"><bdi title={exactNumbers.format(BigInt(cell.tokens!))}>{compactNumbers.format(BigInt(cell.tokens!))}</bdi></span>
  </button>;
  return <div className="overview-stats overview-activity-stats" role="group" aria-label="Activity summary">
    {day('Peak day', peak)}{day('Lowest day', lowest)}
    <button type="button" className="overview-stat" aria-haspopup="dialog" data-overview-activity-stat="Active days" onClick={() => openDetail({ label: 'Active days details', title: 'Active days', content: <dl className="detail-rows"><dt>Days with tokens</dt><dd>{active}</dd><dt>Days reported</dt><dd>{known.length}</dd><dt>Days in period</dt><dd>{data.cells.length}</dd><dt>Period</dt><dd>{data.periodStart} – {data.periodEnd}</dd></dl> })}>
      <span className="overview-stat-name">Active days</span><span className="overview-stat-value">{active} / {known.length}</span>
    </button>
  </div>;
}

export function Overview({ profiles, revision, onAccounts, onAccountAction, busy = false, active = true, refreshingProfileIds = [], refreshWaitSeconds = {}, expectedProfiles = 0, notices }: Props) {
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
    if (!plan || plan.bindingId !== bindingId || !plan.plan) return 'Not available';
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
  const dates = useDateFormat({ year: undefined, timeZoneName: undefined });
  const openDetail = useOpenDetail();
  const card = useRef<HTMLElement>(null);
  // Choosing an account fills the card above the table; numbers in the card open the detail panel.
  const openAccount = (id: string) => { setSelection(id); window.requestAnimationFrame(() => card.current?.scrollIntoView({ block: 'nearest', behavior: 'instant' })); };
  const openRow = (event: MouseEvent<HTMLTableRowElement>, id: string) => { if (!(event.target as HTMLElement).closest('button')) openAccount(id); };
  const tableRows = shownProfiles.map(profile => {
    const selectedRow = profile.id === profileId;
    const cached = accountQuotas[profile.id];
    const rowQuota = selectedRow ? quota : cached?.quota;
    const lowest = quotaWindows(rowQuota).reduce<SavedQuotaWindow | null>((best, window) => !best || window.derivedRemainingPercent < best.derivedRemainingPercent ? window : best, null);
    return { profile, rowQuota, selectedRow, lowest,
      rowLoading: selectedRow ? pending : active && (!cached || cached.loading),
      rowFailed: selectedRow ? saved?.quotaFailed : cached?.failed };
  });
  const showShare = dashboard.view !== 'individual' && !!dashboard.data?.members.length;
  const selectedRefreshing = !!profileId && refreshingProfileIds.includes(profileId);
  const selectedWait = profileId ? refreshWaitSeconds[profileId] ?? 0 : 0;
  const needsSignIn = selected && (selected.connectionState === 'authentication_required' || selected.connectionState === 'unauthorized');
  const stat = (id: string, name: string, value: ReactNode, details: ReactNode, attributes: Record<string, string | undefined> = {}) => <button key={id} type="button" className="overview-stat" aria-haspopup="dialog" data-overview-stat={id} onClick={() => openDetail({ label: `${name} details`, title: name, content: <dl className="detail-rows">{details}</dl> })}>
    <span className="overview-stat-name">{name}</span><span className="overview-stat-value" {...attributes}>{value}</span>
  </button>;
  const countDetails = (value: string | null | undefined) => <><dt>Exact value</dt><dd><ExactCount value={value ?? null} /></dd>{quota ? <ReadingDetails quota={quota} /> : activity ? <><dt>Observed</dt><dd><time dateTime={activity.observedAtUtc}>{dates.format(new Date(activity.observedAtUtc))}</time></dd></> : null}<dt>Activity coverage</dt><dd>Unknown</dd></>;
  const readingDetails = selected && <>
    <dt>Plan</dt><dd>{planFor(selected.id, quota?.bindingId)}</dd>
    <dt>Connection</dt><dd>{connectionLabels[selected.connectionState]}</dd>
    <dt>Combined totals</dt><dd>Not available</dd>
    {quota ? <ReadingDetails quota={quota} /> : <><dt>Observed</dt><dd>Not available</dd></>}
  </>;
  const streak = (days: string | null | undefined) => days === null || days === undefined ? <bdi>Not available</bdi> : `${days} days`;
  return <section className="overview-dashboard" aria-label="Usage overview" data-overview data-overview-profile={profileId} aria-busy={pending}>
    {notices}
    {synthetic && <p className="overview-fixture-label">Synthetic test data</p>}
    {activeProfiles.length === 0 ? expectedProfiles > 0 ? <p className="overview-meta overview-loading" role="status">Loading accounts…</p> : <section className="overview-welcome" aria-labelledby="overview-welcome-heading" data-overview-welcome>
      <span className="overview-welcome-mark" aria-hidden="true"><GaugeIcon /></span>
      <div className="overview-quota-empty"><h2 id="overview-welcome-heading">No accounts</h2><Button disabled={busy} onClick={() => act({ kind: 'add' })} data-overview-add-account><PlusIcon />Add OpenAI account</Button></div>
    </section> : <>
    <OverviewGlance profiles={activeProfiles} quotas={loadedQuotas} plans={activeProfiles.map(profile => planFor(profile.id, loadedQuotas[profile.id]?.bindingId)).filter(plan => plan !== 'Not available')} />
    <div className="overview-top" data-share={activeProfiles.length > 1 || undefined}>
      {selected && <section ref={card} className="overview-panel overview-account-card" aria-labelledby="overview-account-heading" data-overview-quota data-overview-account-card={selected.id}>
        <div className="overview-account-head">
          <Avatar label={selected.label} />
          <div className="overview-account-name"><h2 id="overview-account-heading"><bdi>{selected.label}</bdi></h2>{planFor(selected.id, quota?.bindingId) !== 'Not available' && <span className="overview-meta">{planFor(selected.id, quota?.bindingId)}</span>}</div>
          <span className="overview-connection" data-connection={selected.connectionState}><span className="status-dot" aria-hidden="true" />{connectionLabels[selected.connectionState]}</span>
          <div className="overview-account-actions"><Button variant={needsSignIn ? 'default' : 'outline'} disabled={busy || selectedRefreshing || (!needsSignIn && selectedWait > 0)} aria-busy={selectedRefreshing} data-overview-refresh={needsSignIn ? undefined : selected.id} onClick={() => act({ kind: needsSignIn ? 'signin' : 'refresh', profileId: selected.id })}>{!needsSignIn && <RefreshIcon />}{selectedRefreshing ? 'Refreshing…' : needsSignIn ? selected.connectionState === 'unauthorized' ? 'Reconnect' : 'Sign in' : selectedWait > 0 ? `Refresh in ${selectedWait}s` : 'Refresh'}</Button><Button variant="outline" data-overview-view-usage onClick={() => act({ kind: 'usage', profileId: selected.id })}>View usage</Button><Button variant="outline" className="overview-more" aria-label={`Manage ${selected.label}`} title="Manage" disabled={busy} onClick={() => act({ kind: 'manage', profileId: selected.id })}><MoreIcon /></Button></div>
        </div>
        {pending && <p className="overview-meta" role="status" data-overview-loading>Loading saved readings…</p>}
        {limit ? <div className="overview-quota-block" data-window-count={Number(!!limit.primary) + Number(!!limit.secondary)}>
          <h3 className="sr-only">Quota remaining</h3>
          {quota && quota.limits.length > 1 ? <div className="overview-limit-picker"><label id="overview-limit-label" htmlFor="overview-limit" className="sr-only">Quota limit</label>
            <Select modal={false} value={limit.limitId} items={Object.fromEntries(quota.limits.map(limit => [limit.limitId, limit.limitName ?? limit.limitId]))} onValueChange={value => { if (value && quota.limits.some(limit => limit.limitId === value)) setLimitSelection(`${profileId}:${value}`); }}>
              <SelectTrigger id="overview-limit" aria-labelledby="overview-limit-label"><SelectValue /></SelectTrigger><SelectPopup alignItemWithTrigger={false}>{quota.limits.map(limit => <SelectItem key={limit.limitId} value={limit.limitId}>{limit.limitName ?? limit.limitId}</SelectItem>)}</SelectPopup>
            </Select>
          </div> : <span className="sr-only"><bdi>{limit.limitName ?? limit.limitId}</bdi></span>}
          <QuotaWindow value={limit.primary} role="Primary window" /><QuotaWindow value={limit.secondary} role="Secondary window" />
        </div> : <div className="overview-quota-empty"><h3>{needsSignIn ? 'Connect to view usage' : quota ? 'No quota windows reported' : 'No saved quota yet'}</h3></div>}
        {saved?.quotaFailed && <p className="overview-error">Saved quotas could not be loaded. Try Refresh.</p>}
        <div className="overview-stats" role="group" aria-label="Selected profile activity summary">
          {stat('lifetime', 'Lifetime tokens', pending && lifetimeTokens === null ? 'Loading…' : <ExactCount value={lifetimeTokens} />, countDetails(lifetimeTokens), { 'data-overview-lifetime': '', 'data-exact-value': lifetimeTokens ?? undefined })}
          {stat('peak', 'Peak daily tokens', <ExactCount value={activity?.summary.peakDailyTokens ?? null} />, countDetails(activity?.summary.peakDailyTokens))}
          {stat('streak', 'Current streak', streak(activity?.summary.currentStreakDays), <><dt>Current streak</dt><dd>{streak(activity?.summary.currentStreakDays)}</dd><dt>Activity coverage</dt><dd>Unknown</dd></>)}
          {stat('longest-streak', 'Longest streak', streak(activity?.summary.longestStreakDays), <><dt>Longest streak</dt><dd>{streak(activity?.summary.longestStreakDays)}</dd><dt>Activity coverage</dt><dd>Unknown</dd></>)}
          {stat('task', 'Longest task', taskDuration(activity?.summary.longestRunningTurnSec), <><dt>Longest task</dt><dd>{taskDuration(activity?.summary.longestRunningTurnSec)}</dd></>)}
          {stat('credits', 'Reset credits', quota?.availableResetCredits ?? <bdi>Not available</bdi>, <><dt>Reset credits</dt><dd>{quota?.availableResetCredits ?? 'Not available'}</dd>{quota && <ReadingDetails quota={quota} />}</>)}
        </div>
        {saved?.activityFailed && <p className="overview-error" role="alert">Saved activity could not be loaded. Try Refresh.</p>}
        <Button variant="ghost" className="text-button overview-reading-details" aria-haspopup="dialog" data-overview-reading-details onClick={() => openDetail({ label: 'Reading details', title: <><Avatar label={selected.label} /><bdi>{selected.label}</bdi></>, content: <dl className="detail-rows" aria-label="Reading details">{readingDetails}</dl> })}>Reading details</Button>
      </section>}
      {showShare ? <AccountDonut mode="share" dashboard={dashboard} /> : activeProfiles.length > 1 && <section className="overview-panel overview-share-panel" aria-labelledby="overview-share-heading" data-overview-share-compare>
        <div className="overview-panel-heading"><h2 id="overview-share-heading">Account share</h2></div>
        <div className="overview-share-compare"><div className="overview-donut-plot overview-donut-unavailable" aria-hidden="true" /><Button variant="outline" data-overview-compare onClick={() => dashboard.setView('profile_sum')}>Compare accounts</Button></div>
      </section>}
    </div>
    <section className="overview-activity-panel" aria-label="Token activity" data-overview-activity>
      <ActivityPanel profiles={activeProfiles} profileId={active ? profileId ?? null : null} revision={revision} dashboard={dashboard} footer={<ActivityStats dashboard={dashboard} />} />
    </section>
    <section className="overview-accounts" aria-labelledby="overview-accounts-heading" data-overview-accounts-table>
      <div className="overview-accounts-heading"><h2 id="overview-accounts-heading">Accounts <span>{activeProfiles.length}</span></h2>
        {activeProfiles.length > 2 && <div className="overview-accounts-tools" role="search" aria-label="Find accounts">
          <label className="overview-search"><SearchIcon /><span className="sr-only">Search accounts</span><input type="search" placeholder="Search accounts" autoComplete="off" value={query} onChange={event => setQuery(event.target.value)} /></label>
          <Select modal={false} value={accountFilter} items={accountFilters} onValueChange={value => { if (value) setAccountFilter(value); }}><SelectTrigger aria-label="Filter accounts"><SelectValue /></SelectTrigger><SelectPopup alignItemWithTrigger={false}>{(Object.keys(accountFilters) as AccountFilter[]).map(value => <SelectItem key={value} value={value}>{accountFilters[value]}</SelectItem>)}</SelectPopup></Select>
        </div>}
        <Button type="button" variant="default" disabled={busy || activeProfiles.length >= 10} onClick={() => act({ kind: 'add' })} data-overview-add-account><PlusIcon />Add account</Button></div>
      {shownProfiles.length > 0 ? <div className="overview-table-scroll" tabIndex={0} role="region" aria-label="Account readings"><table role="table"><thead role="rowgroup"><tr role="row"><th role="columnheader" scope="col">Account</th><th role="columnheader" scope="col">Status</th><th role="columnheader" scope="col">Plan</th><th role="columnheader" scope="col">Quota left</th><th role="columnheader" scope="col">Last updated</th><th role="columnheader" scope="col"><span className="sr-only">Actions</span></th></tr></thead>
        <tbody role="rowgroup">{tableRows.map(({ profile, rowQuota, selectedRow, lowest, rowLoading, rowFailed }) => {
          const signin = profile.connectionState === 'authentication_required' || profile.connectionState === 'unauthorized';
          const freshnessMark = rowQuota && rowQuota.freshness !== 'fresh' ? freshness[rowQuota.freshness] : null;
          return <tr role="row" key={profile.id} data-overview-account={profile.id} data-selected={selectedRow} onClick={event => openRow(event, profile.id)}>
            <th role="rowheader" scope="row"><Button type="button" variant="ghost" aria-pressed={selectedRow} onClick={() => openAccount(profile.id)} className="overview-account-link"><Avatar label={profile.label} /><span>{profile.label}</span></Button>{rowFailed && <small className="overview-error">Could not reload saved quota</small>}</th>
            <td role="cell" data-label="Status" data-overview-account-status><span className="overview-connection" data-connection={profile.connectionState}><span className="status-dot" aria-hidden="true" />{connectionLabels[profile.connectionState]}</span></td>
            <td role="cell" data-label="Plan">{planFor(profile.id, rowQuota?.bindingId)}</td>
            <td role="cell" data-label="Quota left" data-overview-account-quota data-window={lowest?.label}>{lowest ? <div className="overview-table-quota" data-attention={attention(lowest.derivedRemainingPercent)}><span title={`${lowest.label}: ${lowest.usedPercent}% used; remaining is derived`}>{lowest.derivedRemainingPercent}% left</span>{freshnessMark && <Mark>{freshnessMark}</Mark>}<Meter min={0} max={100} value={lowest.derivedRemainingPercent} aria-label={`${profile.label}: ${lowest.derivedRemainingPercent}% remaining (derived)`}><MeterTrack><MeterIndicator /></MeterTrack></Meter></div> : rowLoading ? 'Loading…' : 'Not available'}</td>
            <td role="cell" data-label="Last updated">{rowQuota ? <time dateTime={rowQuota.observedAtUtc}>{dates.format(new Date(rowQuota.observedAtUtc))}</time> : 'Not available'}</td>
            <td role="cell"><div className="overview-row-actions">{refreshingProfileIds.includes(profile.id) && <span className="overview-meta" data-overview-row-refreshing>Refreshing…</span>}{signin ? <Button disabled={busy} onClick={() => act({ kind: 'signin', profileId: profile.id })}>{profile.connectionState === 'unauthorized' ? 'Reconnect' : 'Sign in'}</Button> : <Button variant="outline" disabled={busy} onClick={() => act({ kind: 'usage', profileId: profile.id })}>View usage</Button>}<Button variant="ghost" className="overview-more" disabled={busy} aria-label={`Manage ${profile.label}`} title="Manage" onClick={() => act({ kind: 'manage', profileId: profile.id })}><MoreIcon /></Button></div></td>
          </tr>;
        })}</tbody>
      </table></div> : <div className="overview-no-match"><p role="status">No accounts match.</p><Button variant="outline" onClick={() => { setQuery(''); setAccountFilter('all'); }}>Clear filters</Button></div>}
    </section>
    </>}
  </section>;
}
