import type { AccountAction } from './account-actions';
import { Fragment, useEffect, useId, useRef, useState, type ReactNode } from 'react';
import { Button } from './components/ui/button';
import { Meter, MeterIndicator, MeterLabel, MeterTrack } from './components/ui/meter';
import { Select, SelectItem, SelectPopup, SelectTrigger, SelectValue } from './components/ui/select';
import { invoke, type ActivityView, type ProfileRecord, type SavedProfileActivity, type SavedProfileQuota, type SavedQuotaWindow } from './contracts';
import { ActivityPanel } from './activity-panel';
import { useActivityDashboard } from './activity-dashboard';
import { useAccountSeries, usePreviousPeriod } from './activity-series';
import './activity.css';
import './overview.css';
import { Mark, useOpenDetail } from './detail-panel';
import { BigNumber, PeriodCompare, ShareDonut, TokensPerDay, WeekRhythm, compactCount } from './overview-charts';
import { OverviewGlance, ReadingDetails, age } from './overview-glance';
import { ResetCountdown } from './reset-countdown';
import { useDateFormat } from './time-display';
import { ArrowIcon, ClockIcon, GaugeIcon, MoreIcon, PlusIcon, RefreshIcon, SearchIcon } from './icons';

type Props = { profiles: ProfileRecord[]; revision: number; onAccounts: () => void; onAccountAction?: (action: AccountAction) => void; busy?: boolean; active?: boolean; refreshingProfileIds?: string[]; refreshWaitSeconds?: Record<string, number>; expectedProfiles?: number; notices?: ReactNode };
type Reading = { activity: SavedProfileActivity | null; quota: SavedProfileQuota | null; activityFailed: boolean; quotaFailed: boolean; loading: boolean };
const exactNumbers = new Intl.NumberFormat('en-US');
function ExactCount({ value }: { value: string | null }) {
  if (value === null) return <bdi>Not available</bdi>;
  // DM Sans tnum widens commas, so only the digit groups use tabular figures.
  return <bdi>{exactNumbers.format(BigInt(value)).split(',').map((group, index) => <Fragment key={index}>{index > 0 && ','}<span className="tabular-digits">{group}</span></Fragment>)}</bdi>;
}
// Compact in place; the exact count is in the title and the detail panel.
function CompactCount({ value }: { value: string | null }) {
  if (value === null) return <bdi>Not available</bdi>;
  return <bdi title={exactNumbers.format(BigInt(value))}>{compactCount(BigInt(value))}</bdi>;
}
const connectionLabels = { connected: 'Connected', authentication_required: 'Not connected', unauthorized: 'Session expired', offline: 'Offline', unsupported: 'Unavailable', error: 'Needs attention' };
const freshness = { fresh: 'Current', stale: 'Stale', unknown: 'Freshness unknown' };
const planNames: Record<string, string> = { plus: 'ChatGPT Plus', pro: 'ChatGPT Pro', free: 'ChatGPT Free', team: 'ChatGPT Team', business: 'ChatGPT Business', enterprise: 'ChatGPT Enterprise', edu: 'ChatGPT Edu' };
const periods = [[7, '7 days'], [30, '30 days'], [90, '90 days'], [365, '1 year']] as const;

function Avatar({ label, slot }: { label: string; slot: number }) {
  const initial = /^Account ([A-J])$/.exec(label)?.[1] ?? Array.from(label)[0]?.toUpperCase() ?? 'A';
  return <span className="overview-avatar" data-tint={slot} aria-hidden="true">{initial}</span>;
}

// Compares used percent with the window time elapsed when the reading was taken.
// It describes that reading; it never predicts a reset or an exhaustion time.
function pace(value: SavedQuotaWindow, observedAtUtc: string) {
  if (value.resetState !== 'expected' || value.resetsAtUnix === null || !value.windowDurationMins) return null;
  const minutesLeft = (value.resetsAtUnix * 1000 - Date.parse(observedAtUtc)) / 60_000;
  if (!(minutesLeft > 0) || minutesLeft > value.windowDurationMins) return null;
  const elapsed = Math.round((1 - minutesLeft / value.windowDurationMins) * 100);
  const difference = value.usedPercent - elapsed;
  return { elapsed, kind: difference > 10 ? 'fast' : difference < -10 ? 'slow' : 'even', word: difference > 10 ? 'Faster than even pace' : difference < -10 ? 'Slower than even pace' : 'On even pace' };
}

function QuotaWindow({ value, role, observedAtUtc }: { value: SavedQuotaWindow | null; role: string; observedAtUtc: string }) {
  if (!value) return null;
  const reset = value.resetsAtUnix === null ? null : new Date(value.resetsAtUnix * 1000);
  const resetState = { expected: null, due: 'Reset due; awaiting confirmation', unconfirmed: 'Reset status not confirmed', confirmed: 'Reset confirmed' }[value.resetState];
  const steady = pace(value, observedAtUtc);
  const left = value.derivedRemainingPercent;
  return <div className="overview-quota-window" data-overview-window={role} data-attention={attention(left)}>
    <Meter min={0} max={100} value={left} aria-label={`${value.label}: ${left}% remaining (derived)`} aria-valuetext={`${left}% remaining (derived)${steady ? `; ${steady.word.toLowerCase()}` : ''}`}>
      <div className="overview-meter-heading">
        <strong className="overview-window-left">{left}<span className="overview-dim">%</span></strong>
        <MeterLabel>{value.label} left</MeterLabel>
      </div>
      <MeterTrack><MeterIndicator />{steady && <span className="overview-pace-tick" style={{ left: `${100 - steady.elapsed}%` }} aria-hidden="true" />}</MeterTrack>
    </Meter>
    <p className="overview-window-note"><span data-pace={steady?.kind}>{steady?.word ?? `${value.usedPercent}% used`}</span><span className="overview-window-reset" title={reset?.toISOString()}><ClockIcon />{reset ? <ResetCountdown phrase resetsAtUnix={value.resetsAtUnix} /> : 'Reset time not available'}</span></p>
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

// Saved readings for every active account. A failed reload keeps the last reading with unknown freshness,
// and a reading whose activity and quota come from different bindings is dropped.
function useAccountReadings(ids: string, revision: number, refresh: number, active: boolean, focus: string | null) {
  const [readings, setReadings] = useState<Record<string, Reading>>({});
  const [plans, setPlans] = useState<Record<string, { bindingId: string; plan: string | null }>>({});
  const focused = useRef(focus);
  // Every account loads with the page, and an account chosen at the top loads again so its card is current.
  const reload = (list: string[], isCurrent: () => boolean) => {
    setReadings(previous => ({ ...(list.length > 1 ? Object.fromEntries(Object.entries(previous).filter(([id]) => ids.split(',').includes(id))) : previous),
      ...Object.fromEntries(list.map(id => [id, { ...(previous[id] ?? { activity: null, quota: null, activityFailed: false, quotaFailed: false }), loading: true }])) }));
    for (const profileId of list) {
      const activity = invoke('load_profile_activity', { profileId }).then(value => {
        if (value && value.profileId !== profileId) throw new Error('Incompatible saved activity profile.');
        return value;
      });
      const quota = invoke('load_profile_quota', { profileId }).then(value => {
        if (value && value.profileId !== profileId) throw new Error('Incompatible saved quota profile.');
        return value;
      });
      void Promise.allSettled([activity, quota]).then(([activity, quota]) => {
        if (!isCurrent()) return;
        setReadings(previous => {
          const retained = previous[profileId];
          let nextActivity = activity.status === 'fulfilled' ? activity.value : retained?.activity ? { ...retained.activity, freshness: 'unknown' as const } : null;
          let nextQuota = quota.status === 'fulfilled' ? quota.value : retained?.quota ? { ...retained.quota, freshness: 'unknown' as const } : null;
          if (activity.status === 'rejected' && quota.status === 'fulfilled' && (!quota.value || nextActivity?.bindingId !== quota.value.bindingId)) nextActivity = null;
          if (quota.status === 'rejected' && activity.status === 'fulfilled' && (!activity.value || nextQuota?.bindingId !== activity.value.bindingId)) nextQuota = null;
          const mixed = !!nextActivity && !!nextQuota && nextActivity.bindingId !== nextQuota.bindingId;
          return { ...previous, [profileId]: {
            activity: mixed ? null : nextActivity, quota: mixed ? null : nextQuota,
            activityFailed: mixed || activity.status === 'rejected', quotaFailed: mixed || quota.status === 'rejected', loading: false,
          } };
        });
      });
      void quota.then(async value => {
        if (!value) return;
        const bindingId = value.bindingId;
        try {
          const plan = await invoke('load_profile_plan', { profileId, bindingId });
          if (isCurrent() && plan.profileId === profileId && plan.bindingId === bindingId) setPlans(previous => ({ ...previous, [profileId]: { bindingId, plan: plan.plan } }));
        } catch { if (isCurrent()) setPlans(previous => ({ ...previous, [profileId]: { bindingId, plan: null } })); }
      }).catch(() => {});
    }
  };
  useEffect(() => {
    if (!active) { setReadings(previous => Object.fromEntries(Object.entries(previous).map(([id, value]) => [id, { ...value, loading: false }]))); return; }
    let current = true;
    reload(ids ? ids.split(',') : [], () => current);
    return () => { current = false; };
  }, [ids, revision, refresh, active]);
  useEffect(() => {
    if (focused.current === focus) return;
    focused.current = focus;
    if (!active || !focus || !ids.split(',').includes(focus)) return;
    let current = true;
    reload([focus], () => current);
    return () => { current = false; };
  }, [focus]);
  return { readings, plans };
}

type CardProps = { profile: ProfileRecord; slot: number; reading: Reading | undefined; plan: string; busy: boolean; active: boolean; refreshing: boolean; wait: number; selected: boolean; act: (action: AccountAction) => void };
// One pastel card per account: its windows first, then what it has done.
function AccountCard({ profile, slot, reading, plan, busy, active, refreshing, wait, selected, act }: CardProps) {
  const openDetail = useOpenDetail();
  const dates = useDateFormat({ year: undefined, timeZoneName: undefined });
  const headingId = useId();
  const [limitId, setLimitId] = useState<string | null>(null);
  const quota = reading?.quota ?? null, activity = reading?.activity ?? null;
  const pending = !!reading?.loading || (active && !reading);
  const limit = quota?.limits.find(limit => limit.limitId === limitId) ?? quota?.limits[0];
  const lowest = quotaWindows(quota).reduce<SavedQuotaWindow | null>((best, window) => !best || window.derivedRemainingPercent < best.derivedRemainingPercent ? window : best, null);
  const needsSignIn = profile.connectionState === 'authentication_required' || profile.connectionState === 'unauthorized';
  const observed = quota?.observedAtUtc ?? activity?.observedAtUtc ?? null;
  const freshnessMark = quota && quota.freshness !== 'fresh' ? freshness[quota.freshness] : null;
  const summary = activity?.summary;
  const provenance = quota ? <ReadingDetails quota={quota} /> : activity ? <><dt>Observed</dt><dd><time dateTime={activity.observedAtUtc}>{dates.format(new Date(activity.observedAtUtc))}</time></dd></> : null;
  const countDetails = (value: string | null | undefined) => <><dt>Exact value</dt><dd><ExactCount value={value ?? null} /></dd>{provenance}<dt>Activity coverage</dt><dd>Unknown</dd></>;
  const stat = (id: string, name: string, value: ReactNode, details: ReactNode, attributes: Record<string, string | undefined> = {}) => <button key={id} type="button" className="overview-stat" aria-haspopup="dialog" data-overview-stat={id} onClick={() => openDetail({ label: `${name} details`, title: name, content: <dl className="detail-rows">{details}</dl> })}>
    <span className="overview-stat-name">{name}</span><span className="overview-stat-value" {...attributes}>{value}</span>
  </button>;
  const streak = (days: string | null | undefined) => days === null || days === undefined ? <bdi>Not available</bdi> : `${days} ${days === '1' ? 'day' : 'days'}`;
  const readingDetails = <>
    <dt>Plan</dt><dd>{plan}</dd>
    <dt>Connection</dt><dd>{connectionLabels[profile.connectionState]}</dd>
    <dt>Reset credits</dt><dd>{quota?.availableResetCredits ?? 'Not available'}</dd>
    <dt>Combined totals</dt><dd>Not available</dd>
    {quota ? <ReadingDetails quota={quota} /> : <><dt>Observed</dt><dd>Not available</dd></>}
  </>;
  const state = needsSignIn || profile.connectionState !== 'connected' ? 'away' : lowest && lowQuota(lowest) ? 'low' : undefined;
  return <article className="overview-card" data-overview-account={profile.id} data-overview-account-card={profile.id} data-tint={slot} data-state={state} data-selected={selected || undefined} data-attention={lowest ? attention(lowest.derivedRemainingPercent) : undefined} aria-labelledby={headingId} aria-busy={pending}>
    <header className="overview-card-head">
      <Avatar label={profile.label} slot={slot} />
      <div className="overview-card-name">
        <span className="overview-card-plan">{plan !== 'Not available' && <span data-overview-plan>{plan}</span>}{quota?.availableResetCredits && quota.availableResetCredits !== '0' && <button type="button" className="overview-credits" aria-haspopup="dialog" data-overview-stat="credits" onClick={() => openDetail({ label: 'Reset credits details', title: 'Reset credits', content: <dl className="detail-rows"><dt>Reset credits</dt><dd>{quota.availableResetCredits}</dd><ReadingDetails quota={quota} /></dl> })}>{quota.availableResetCredits} reset {quota.availableResetCredits === '1' ? 'credit' : 'credits'}</button>}</span>
        <h3 id={headingId}><bdi>{profile.label}</bdi></h3>
        <p className="overview-card-status" data-overview-account-status>
          {state === 'low' && <span className="overview-pill">Almost empty</span>}
          <span className="overview-connection" data-connection={profile.connectionState}><span className="status-dot" aria-hidden="true" />{connectionLabels[profile.connectionState]}</span>
          {observed && <span>Updated <time dateTime={observed}>{age(observed)}</time></span>}
          {freshnessMark && <Mark>{freshnessMark}</Mark>}
          {refreshing && <span data-overview-row-refreshing>Refreshing…</span>}
        </p>
      </div>
      <Button variant="ghost" className="overview-more overview-card-action" aria-label={`Manage ${profile.label}`} title="Manage" disabled={busy} onClick={() => act({ kind: 'manage', profileId: profile.id })}><MoreIcon /></Button>
    </header>
    {pending && !quota && !activity && <p className="overview-meta" role="status" data-overview-loading>Loading saved readings…</p>}
    {limit ? <div className="overview-quota-block" data-overview-quota data-overview-account-quota data-window={lowest?.label}>
      <h4 className="sr-only">Quota remaining</h4>
      {quota && quota.limits.length > 1 ? <div className="overview-limit-picker"><label id={`${headingId}-limit-label`} htmlFor={`${headingId}-limit`} className="sr-only">Quota limit</label>
        <Select modal={false} value={limit.limitId} items={Object.fromEntries(quota.limits.map(limit => [limit.limitId, limit.limitName ?? limit.limitId]))} onValueChange={value => { if (value && quota.limits.some(limit => limit.limitId === value)) setLimitId(value); }}>
          <SelectTrigger id={`${headingId}-limit`} aria-labelledby={`${headingId}-limit-label`}><SelectValue /></SelectTrigger><SelectPopup alignItemWithTrigger={false}>{quota.limits.map(limit => <SelectItem key={limit.limitId} value={limit.limitId}>{limit.limitName ?? limit.limitId}</SelectItem>)}</SelectPopup>
        </Select>
      </div> : <span className="sr-only"><bdi>{limit.limitName ?? limit.limitId}</bdi></span>}
      <QuotaWindow value={limit.primary} role="Primary window" observedAtUtc={quota!.observedAtUtc} /><QuotaWindow value={limit.secondary} role="Secondary window" observedAtUtc={quota!.observedAtUtc} />
    </div> : !(pending && !quota) && <div className="overview-quota-empty"><h4>{needsSignIn ? 'Connect to view usage' : quota ? 'No quota windows reported' : 'No saved quota yet'}</h4></div>}
    {reading?.quotaFailed && <p className="overview-error">Saved quotas could not be loaded. Try Refresh.</p>}
    <div className="overview-stats" role="group" aria-label={`${profile.label} activity summary`}>
      {stat('lifetime', 'Lifetime', pending && !activity ? 'Loading…' : <CompactCount value={summary?.lifetimeTokens ?? null} />, countDetails(summary?.lifetimeTokens), { 'data-overview-lifetime': '', 'data-exact-value': summary?.lifetimeTokens ?? undefined })}
      {stat('peak', 'Peak day', <CompactCount value={summary?.peakDailyTokens ?? null} />, countDetails(summary?.peakDailyTokens), { 'data-exact-value': summary?.peakDailyTokens ?? undefined })}
      {stat('streak', 'Streak', streak(summary?.currentStreakDays), <><dt>Current streak</dt><dd>{streak(summary?.currentStreakDays)}</dd><dt>Longest streak</dt><dd>{streak(summary?.longestStreakDays)}</dd><dt>Activity coverage</dt><dd>Unknown</dd></>)}
      {stat('task', 'Longest task', taskDuration(summary?.longestRunningTurnSec), <><dt>Longest task</dt><dd>{taskDuration(summary?.longestRunningTurnSec)}</dd></>)}
    </div>
    {reading?.activityFailed && <p className="overview-error" role="alert">Saved activity could not be loaded. Try Refresh.</p>}
    <footer className="overview-card-foot">
      <Button variant={needsSignIn ? 'default' : 'ghost'} className={needsSignIn ? 'overview-card-signin' : 'overview-card-action'} disabled={busy || refreshing || (!needsSignIn && wait > 0)} aria-busy={refreshing} data-overview-refresh={needsSignIn ? undefined : profile.id} onClick={() => act({ kind: needsSignIn ? 'signin' : 'refresh', profileId: profile.id })}>{!needsSignIn && <RefreshIcon />}{refreshing ? 'Refreshing…' : needsSignIn ? profile.connectionState === 'unauthorized' ? 'Reconnect' : 'Sign in' : wait > 0 ? `Refresh in ${wait}s` : 'Refresh'}</Button>
      <Button variant="ghost" className="overview-card-action" aria-haspopup="dialog" data-overview-reading-details onClick={() => openDetail({ label: 'Reading details', title: <><Avatar label={profile.label} slot={slot} /><bdi>{profile.label}</bdi></>, content: <dl className="detail-rows" aria-label="Reading details">{readingDetails}</dl> })}>Reading details</Button>
      <Button variant="ghost" className="overview-card-action" data-overview-view-usage onClick={() => act({ kind: 'usage', profileId: profile.id })}>View usage<ArrowIcon /></Button>
    </footer>
  </article>;
}

export function Overview({ profiles, revision, onAccounts, onAccountAction, busy = false, active = true, refreshingProfileIds = [], refreshWaitSeconds = {}, expectedProfiles = 0, notices }: Props) {
  const activeProfiles = profiles.filter(profile => !profile.archived);
  const ids = activeProfiles.map(profile => profile.id).join(',');
  // A colour follows the account, so archiving another account never repaints it.
  const slots = Object.fromEntries(profiles.map((profile, index) => [profile.id, index % 6]));
  const connectedCount = activeProfiles.filter(profile => profile.connectionState === 'connected').length;
  const [selection, setSelection] = useState<string | null>(null);
  const [query, setQuery] = useState('');
  const [accountFilter, setAccountFilter] = useState<AccountFilter>('all');
  const [refresh, setRefresh] = useState(0);
  const selected = activeProfiles.find(profile => profile.id === selection) ?? activeProfiles[0];
  const profileId = selected?.id ?? null;
  // The owner chose the all-accounts sum as the opening view (SC-015); it keeps its Unverified mark.
  const initialView: ActivityView = connectedCount > 1 ? 'profile_sum' : 'individual';
  const period = useActivityDashboard(activeProfiles, profileId, revision, active, { view: initialView, periodDays: 30 });
  const year = useActivityDashboard(activeProfiles, profileId, revision, active, { view: initialView });
  const previous = usePreviousPeriod(period.view, period.ids, period.range.start, period.range.end, revision, active);
  const seriesIds = period.view === 'individual' ? period.ids : period.ids.filter(id => activeProfiles.some(profile => profile.id === id && profile.connectionState === 'connected'));
  const series = useAccountSeries(seriesIds, period.range.start, period.range.end, revision, active);
  const { readings, plans } = useAccountReadings(ids, revision, refresh, active, period.view === 'individual' ? profileId : null);
  useEffect(() => {
    if (!active || !ids) return;
    const timer = window.setInterval(() => setRefresh(value => value + 1), 60_000);
    return () => window.clearInterval(timer);
  }, [active, ids]);
  const act = (action: AccountAction) => onAccountAction ? onAccountAction(action) : onAccounts();
  const planFor = (id: string) => {
    const plan = plans[id], bindingId = readings[id]?.quota?.bindingId;
    if (!plan || plan.bindingId !== bindingId || !plan.plan) return 'Not available';
    return planNames[plan.plan] ?? plan.plan;
  };
  const chosen = useRef(false);
  const choose = (view: ActivityView, id?: string, byPerson = true) => {
    if (byPerson) chosen.current = true;
    if (id) setSelection(id);
    period.setView(view); year.setView(view);
  };
  useEffect(() => { if (!chosen.current && connectedCount > 1 && period.view === 'individual') choose('profile_sum', undefined, false); }, [connectedCount]);
  const scope = period.view === 'individual' ? profileId : null;
  const loadedQuotas = Object.fromEntries(activeProfiles.map(profile => [profile.id, readings[profile.id]?.quota]));
  const loadedActivity = Object.fromEntries(activeProfiles.map(profile => [profile.id, readings[profile.id]?.activity]));
  const search = query.trim().toLocaleLowerCase();
  const shownProfiles = activeProfiles.filter(profile => {
    if (search && !profile.label.toLocaleLowerCase().includes(search)) return false;
    if (accountFilter === 'connected') return profile.connectionState === 'connected';
    if (accountFilter === 'attention') return profile.connectionState !== 'connected';
    return accountFilter !== 'low' || quotaWindows(loadedQuotas[profile.id]).some(lowQuota);
  });
  const synthetic = Object.values(readings).some(reading => reading.activity?.sourceKind === 'test_fixture' || reading.quota?.sourceKind === 'test_fixture');
  const pending = activeProfiles.some(profile => readings[profile.id]?.loading ?? active);
  const openDetail = useOpenDetail();
  const total = period.data?.totalTokens ?? null, before = previous?.totalTokens ?? null;
  const change = total !== null && before !== null && BigInt(before) > 0n ? (BigInt(total) - BigInt(before)) * 100n / BigInt(before) : null;
  const periodName = period.periodDays === 365 ? 'year' : `${period.periodDays} days`;
  const yearDashboard: typeof year = { ...year, setSelection: next => { period.setSelection(next); year.setSelection(next); } };
  return <section className="overview-dashboard" aria-label="Usage overview" data-overview data-overview-profile={profileId ?? undefined} aria-busy={pending}>
    {notices}
    {synthetic && <p className="overview-fixture-label">Synthetic test data</p>}
    {activeProfiles.length === 0 ? expectedProfiles > 0 ? <p className="overview-meta overview-loading" role="status">Loading accounts…</p> : <section className="overview-welcome" aria-labelledby="overview-welcome-heading" data-overview-welcome>
      <span className="overview-welcome-mark" aria-hidden="true"><GaugeIcon /></span>
      <div className="overview-quota-empty"><h2 id="overview-welcome-heading">No accounts</h2><Button disabled={busy} onClick={() => act({ kind: 'add' })} data-overview-add-account><PlusIcon />Add OpenAI account</Button></div>
    </section> : <>
    <section className="overview-hero" aria-labelledby="overview-hero-heading" data-overview-hero>
      <div className="overview-hero-controls">
        <div className="overview-chips" role="group" aria-label="Accounts shown" data-activity-scope={period.view}>
          {activeProfiles.length > 1 && <Button variant="ghost" aria-pressed={period.view === 'profile_sum'} onClick={() => choose('profile_sum')}>All accounts</Button>}
          {activeProfiles.map(profile => <Button key={profile.id} variant="ghost" data-activity-account={profile.id} aria-pressed={period.view === 'individual' && profile.id === profileId} onClick={() => choose('individual', profile.id)}><i data-tint={slots[profile.id] ?? 0} aria-hidden="true" /><bdi>{profile.label}</bdi></Button>)}
          {activeProfiles.length > 1 && <Button variant="ghost" aria-pressed={period.view === 'verified'} onClick={() => choose('verified')}>Verified only</Button>}
        </div>
        <div className="overview-chips overview-periods" role="group" aria-label="Period">
          {periods.map(([days, label]) => <Button key={days} variant="ghost" aria-pressed={period.periodDays === days} onClick={() => period.setPeriodDays(days)}>{label}</Button>)}
        </div>
      </div>
      <h2 id="overview-hero-heading" className="overview-hero-label">Tokens in the last {periodName}{period.view === 'profile_sum' && <Mark>Unverified</Mark>}</h2>
      <div className="overview-hero-row">
        <button type="button" className="overview-hero-value" aria-haspopup="dialog" data-overview-hero-total data-exact-value={total ?? undefined} onClick={() => openDetail({ label: 'Tokens details', title: `Tokens in the last ${periodName}`, content: <dl className="detail-rows"><dt>Exact value</dt><dd><ExactCount value={total} /></dd><dt>Period</dt><dd>{period.range.start} – {period.range.end}</dd><dt>Profile-days reported</dt><dd>{period.data ? `${period.data.knownContributions} of ${period.data.expectedContributions}` : 'Not available'}</dd><dt>Previous period</dt><dd><ExactCount value={before} /></dd><dt>Accounts</dt><dd>{period.view === 'profile_sum' ? 'Sum of chosen accounts, not verified as distinct' : period.view === 'verified' ? 'Verified distinct accounts' : selected?.label ?? 'Not available'}</dd></dl> })}>
          {period.loading && !period.data ? <span className="overview-dim">Loading…</span> : total === null ? <span className="overview-hero-empty">Not available</span> : <BigNumber value={BigInt(total)} />}
        </button>
        {change !== null && <span className="overview-change" data-direction={change > 0n ? 'up' : change < 0n ? 'down' : 'flat'}>{change > 0n ? '↑' : change < 0n ? '↓' : '→'} {String(change < 0n ? -change : change)}% {change < 0n ? 'less' : 'more'} than the {periodName} before</span>}
      </div>
    </section>
    <OverviewGlance profiles={activeProfiles} quotas={loadedQuotas} activity={loadedActivity} period={period.data} periodDays={period.periodDays} scope={scope} plans={activeProfiles.map(profile => planFor(profile.id)).filter(plan => plan !== 'Not available')} />
    <section className="overview-accounts" aria-labelledby="overview-accounts-heading" data-overview-accounts-table>
      <div className="overview-section-heading"><h2 id="overview-accounts-heading">Limits</h2>
        <span className="overview-legend" aria-hidden="true"><span><i className="overview-legend-fill" />Left</span><span><i className="overview-legend-tick" />Even pace</span></span>
        {activeProfiles.length > 2 && <div className="overview-accounts-tools" role="search" aria-label="Find accounts">
          <label className="overview-search"><SearchIcon /><span className="sr-only">Search accounts</span><input type="search" placeholder="Search accounts" autoComplete="off" value={query} onChange={event => setQuery(event.target.value)} /></label>
          <Select modal={false} value={accountFilter} items={accountFilters} onValueChange={value => { if (value) setAccountFilter(value); }}><SelectTrigger aria-label="Filter accounts"><SelectValue /></SelectTrigger><SelectPopup alignItemWithTrigger={false}>{(Object.keys(accountFilters) as AccountFilter[]).map(value => <SelectItem key={value} value={value}>{accountFilters[value]}</SelectItem>)}</SelectPopup></Select>
        </div>}
        <Button type="button" variant="outline" disabled={busy || activeProfiles.length >= 10} onClick={() => act({ kind: 'add' })} data-overview-add-account><PlusIcon />Add account</Button></div>
      {shownProfiles.length > 0 ? <div className="overview-account-grid" role="list" aria-label="Account readings">{shownProfiles.map(profile => <div role="listitem" key={profile.id} className="overview-account-item">
        <AccountCard profile={profile} slot={slots[profile.id] ?? 0} reading={readings[profile.id]} plan={planFor(profile.id)} busy={busy} active={active} refreshing={refreshingProfileIds.includes(profile.id)} wait={refreshWaitSeconds[profile.id] ?? 0} selected={scope === profile.id} act={act} />
      </div>)}</div> : <div className="overview-no-match"><p role="status">No accounts match.</p><Button variant="outline" onClick={() => { setQuery(''); setAccountFilter('all'); }}>Clear filters</Button></div>}
    </section>
    <section className="overview-days" aria-labelledby="overview-days-heading" data-share={period.view !== 'individual' && !!period.data?.members.length || undefined}>
      <div className="overview-section-heading"><h2 id="overview-days-heading">Tokens per day</h2></div>
      <div className="overview-days-body">
        <TokensPerDay accounts={activeProfiles.filter(profile => seriesIds.includes(profile.id)).map(profile => ({ profile, slot: slots[profile.id] ?? 0 }))} series={series} period={period.data} />
        <ShareDonut data={period.data} slots={slots} view={period.view} />
      </div>
    </section>
    <div className="overview-duo"><WeekRhythm period={period.data} /><PeriodCompare period={period.data} previous={previous} days={period.periodDays} /></div>
    <section className="overview-activity-panel" aria-label="Token activity" data-overview-activity>
      <ActivityPanel profiles={activeProfiles} profileId={active ? profileId : null} revision={revision} dashboard={yearDashboard} />
    </section>
    </>}
  </section>;
}
