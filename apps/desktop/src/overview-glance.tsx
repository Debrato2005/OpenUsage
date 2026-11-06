import { Fragment, useState, type ReactNode } from 'react';
import type { ActivityDashboard, ProfileRecord, SavedProfileActivity, SavedProfileQuota, SavedQuotaWindow } from './contracts';
import { DetailPanel, Mark } from './detail-panel';
import { BigNumber } from './overview-charts';
import { ResetCountdown } from './reset-countdown';
import { useDateFormat } from './time-display';

type Props = {
  profiles: ProfileRecord[]; quotas: Record<string, SavedProfileQuota | null | undefined>; plans: string[];
  activity: Record<string, SavedProfileActivity | null | undefined>; period: ActivityDashboard | null; periodDays: number; scope: string | null;
};
type Entry = { profile: ProfileRecord; quota: SavedProfileQuota; window: SavedQuotaWindow; limitId: string };
const freshness = { fresh: 'Current', stale: 'Stale', unknown: 'Freshness unknown' };
const exact = new Intl.NumberFormat('en-US');
const attention = (remaining: number) => remaining <= 10 ? 'low' : remaining <= 25 ? 'warn' : undefined;

export function age(observedAtUtc: string) {
  const minutes = Math.floor((Date.now() - Date.parse(observedAtUtc)) / 60_000);
  if (minutes < 1) return 'Just now';
  if (minutes < 60) return `${minutes} min ago`;
  return minutes < 1440 ? `${Math.floor(minutes / 60)} hr ago` : `${Math.floor(minutes / 1440)} d ago`;
}

export function ReadingDetails({ quota }: { quota: SavedProfileQuota }) {
  const dates = useDateFormat();
  return <>
    <dt>Source</dt><dd>{quota.sourceKind === 'test_fixture' ? 'Synthetic test data' : 'Codex app-server'} · {quota.providerVersion}</dd>
    <dt>Observed</dt><dd><time dateTime={quota.observedAtUtc}>{dates.format(new Date(quota.observedAtUtc))}</time></dd>
    <dt>Freshness</dt><dd>{freshness[quota.freshness]}</dd>
    <dt>Scope</dt><dd>Account quota; application scope unknown</dd>
    <dt>Coverage</dt><dd>Unknown</dd>
    <dt>Identity</dt><dd>Provisional</dd>
  </>;
}

function WindowDetails({ entry }: { entry: Entry }) {
  return <>
    <dt>Account</dt><dd><bdi>{entry.profile.label}</bdi></dd>
    <dt>Limit</dt><dd><bdi>{entry.limitId}</bdi></dd>
    <dt>Window</dt><dd>{entry.window.label}</dd>
    <ReadingDetails quota={entry.quota} />
  </>;
}

const unit = (value: ReactNode, rest: string) => <>{value}<span className="overview-dim">{rest}</span></>;

// Eight answers across accounts. Select one saved window at a time; never sum percentages or infer identity across profiles.
export function OverviewGlance({ profiles, quotas, plans, activity, period, periodDays, scope }: Props) {
  const dates = useDateFormat();
  const [opened, setOpened] = useState<string | null>(null);
  const connected = profiles.filter(profile => profile.connectionState === 'connected');
  const saved = connected.flatMap(profile => quotas[profile.id] ? [{ profile, quota: quotas[profile.id]! }] : []);
  const readings: Entry[] = saved.flatMap(({ profile, quota }) => quota.limits.flatMap(limit =>
    [limit.primary, limit.secondary].flatMap(window => window ? [{ profile, quota, window, limitId: limit.limitId }] : [])));
  const lowest = readings.reduce<Entry | null>((best, entry) => !best || entry.window.derivedRemainingPercent < best.window.derivedRemainingPercent ? entry : best, null);
  // Each account is limited by its lowest window; the account whose limit is highest has the most left.
  const binding = saved.map(({ profile }) => readings.filter(entry => entry.profile.id === profile.id).reduce<Entry | null>((low, entry) => !low || entry.window.derivedRemainingPercent < low.window.derivedRemainingPercent ? entry : low, null)).filter((entry): entry is Entry => entry !== null);
  const most = binding.length > 1 ? binding.reduce((best, entry) => entry.window.derivedRemainingPercent > best.window.derivedRemainingPercent ? entry : best) : null;
  const now = Date.now();
  const upcoming = readings.filter(({ window }) => window.resetState === 'expected' && window.resetsAtUnix !== null && window.resetsAtUnix * 1000 > now);
  const reset = upcoming.reduce<Entry | null>((best, entry) => !best || entry.window.resetsAtUnix! < best.window.resetsAtUnix! ? entry : best, null);
  const latest = saved.reduce<(typeof saved)[number] | null>((best, entry) => !best || Date.parse(entry.quota.observedAtUtc) > Date.parse(best.quota.observedAtUtc) ? entry : best, null);
  const counts = new Map<string, number>();
  for (const plan of plans) counts.set(plan, (counts.get(plan) ?? 0) + 1);
  const mark = saved.length < connected.length ? 'Incomplete' : saved.some(({ quota }) => quota.freshness === 'stale') ? 'Stale' : saved.some(({ quota }) => quota.freshness === 'unknown') ? 'Freshness unknown' : null;
  const coverage = <>
    <dt>Accounts with saved quota</dt><dd>{saved.length} of {connected.length} connected</dd>
    {connected.map(profile => <Fragment key={profile.id}><dt><bdi>{profile.label}</bdi></dt><dd>{quotas[profile.id] ? freshness[quotas[profile.id]!.freshness] : 'Not available'}</dd></Fragment>)}
  </>;
  const where = (entry: Entry) => <><bdi>{entry.profile.label}</bdi>, {entry.window.label.toLowerCase()}</>;
  const missing = <span className="overview-glance-empty">Not available</span>;
  const periodLabel = `last ${periodDays === 365 ? 'year' : `${periodDays} days`}`;

  // Activity answers follow the accounts and dates chosen above.
  const known = (period?.cells ?? []).filter(cell => cell.tokens !== null);
  const peak = known.reduce<(typeof known)[number] | null>((best, cell) => !best || BigInt(cell.tokens!) > BigInt(best.tokens!) ? cell : best, null);
  const active = known.filter(cell => BigInt(cell.tokens!) > 0n).length;
  const scoped = scope ? profiles.filter(profile => profile.id === scope) : profiles;
  const streaks = scoped.flatMap(profile => activity[profile.id]?.summary.currentStreakDays ? [{ profile, current: BigInt(activity[profile.id]!.summary.currentStreakDays!), longest: activity[profile.id]!.summary.longestStreakDays }] : []);
  const streak = streaks.reduce<(typeof streaks)[number] | null>((best, entry) => !best || entry.current > best.current ? entry : best, null);
  const lifetimes = scoped.flatMap(profile => activity[profile.id]?.summary.lifetimeTokens ? [{ profile, tokens: BigInt(activity[profile.id]!.summary.lifetimeTokens!) }] : []);
  const lifetime = lifetimes.length ? lifetimes.reduce((sum, entry) => sum + entry.tokens, 0n) : null;
  const periodDetails = period ? <><dt>Period</dt><dd>{period.periodStart} – {period.periodEnd}</dd><dt>Profile-days reported</dt><dd>{period.knownContributions} of {period.expectedContributions}</dd></> : <><dt>Reason</dt><dd>No saved activity for these dates</dd></>;

  type Card = { id: string; name: string; value: ReactNode; context?: ReactNode; details: ReactNode; state?: string | null; attention?: string | undefined };
  const cards: Card[] = [
    { id: 'quota', name: 'Runs out first', value: lowest ? unit(lowest.window.derivedRemainingPercent, '%') : missing, context: lowest && where(lowest), state: lowest ? mark : null, attention: lowest ? attention(lowest.window.derivedRemainingPercent) : undefined, details: <>
      {lowest ? <><WindowDetails entry={lowest} /><dt>Calculation</dt><dd>100% − {lowest.window.usedPercent}% used</dd></> : <><dt>Reason</dt><dd>No saved quota windows for connected accounts</dd></>}
      {coverage}
    </> },
    { id: 'reset', name: 'Next reset', value: reset ? <ResetCountdown detached resetsAtUnix={reset.window.resetsAtUnix} /> : missing, context: reset && where(reset), state: reset ? mark : null, details: <>
      {reset ? <><WindowDetails entry={reset} /><dt>Expected reset</dt><dd><time dateTime={new Date(reset.window.resetsAtUnix! * 1000).toISOString()}>{dates.format(new Date(reset.window.resetsAtUnix! * 1000))}</time></dd><dt>Confirmation</dt><dd>Time alone does not confirm a reset</dd></> : <><dt>Reason</dt><dd>No upcoming reset time reported for connected accounts</dd></>}
      {coverage}
    </> },
    { id: 'best', name: 'Most left now', value: most ? unit(most.window.derivedRemainingPercent, '%') : missing, context: most && where(most), state: most ? mark : null, details: <>
      {most ? <><WindowDetails entry={most} /><dt>Calculation</dt><dd>The highest of each account's lowest window</dd></> : <><dt>Reason</dt><dd>Needs saved quota from two connected accounts</dd></>}
      {coverage}
    </> },
    { id: 'accounts', name: 'Connected accounts', value: unit(connected.length, ` / ${profiles.length}`), context: latest ? <>Updated <time dateTime={latest.quota.observedAtUtc} data-overview-latest>{age(latest.quota.observedAtUtc)}</time></> : profiles.length > connected.length ? `${profiles.length - connected.length} need sign-in` : null, details: <>
      <dt>Source</dt><dd>Local profiles</dd>
      <dt>Active accounts</dt><dd>{profiles.length}</dd>
      <dt>Needs sign-in</dt><dd>{profiles.length - connected.length}</dd>
      <dt>Saved plans</dt><dd>{counts.size ? [...counts].map(([plan, count]) => <div key={plan}>{count} {plan}</div>) : 'Not available'}</dd>
      {latest ? <><dt>Latest reading</dt><dd><bdi>{latest.profile.label}</bdi></dd><ReadingDetails quota={latest.quota} /></> : <><dt>Latest reading</dt><dd>Not available</dd></>}
    </> },
    { id: 'streak', name: 'Current streak', value: streak ? unit(String(streak.current), streak.current === 1n ? ' day' : ' days') : missing, context: streak && <>{!scope && <><bdi>{streak.profile.label}</bdi>, </>}longest {streak.longest ?? 'not available'}</>, details: <>
      {streak ? <><dt>Account</dt><dd><bdi>{streak.profile.label}</bdi></dd><dt>Current streak</dt><dd>{String(streak.current)} days</dd><dt>Longest streak</dt><dd>{streak.longest === null ? 'Not available' : `${streak.longest} days`}</dd><dt>Source</dt><dd>Provider activity summary</dd></> : <><dt>Reason</dt><dd>No current streak reported</dd></>}
    </> },
    { id: 'peak', name: 'Peak day', value: peak ? <BigNumber value={BigInt(peak.tokens!)} /> : missing, context: peak && <time dateTime={peak.startDate}>{new Date(`${peak.startDate}T00:00:00Z`).toLocaleDateString('en-US', { month: 'long', day: 'numeric', timeZone: 'UTC' })}</time>, details: <>
      {peak && <><dt>Date</dt><dd><time dateTime={peak.startDate}>{peak.startDate}</time></dd><dt>Tokens</dt><dd><bdi>{exact.format(BigInt(peak.tokens!))}</bdi></dd></>}{periodDetails}
    </> },
    { id: 'active', name: 'Active days', value: known.length ? unit(active, ` / ${known.length}`) : missing, context: known.length ? `with tokens, ${periodLabel}` : null, details: <>
      <dt>Days with tokens</dt><dd>{active}</dd><dt>Days with a reading</dt><dd>{known.length}</dd><dt>Days in period</dt><dd>{period?.cells.length ?? 'Not available'}</dd>{periodDetails}
    </> },
    { id: 'lifetime', name: 'Lifetime tokens', value: lifetime === null ? missing : <BigNumber value={lifetime} />, context: lifetime === null ? null : scope ? 'as reported' : `${lifetimes.length} of ${scoped.length} accounts, as reported`, state: !scope && lifetimes.length > 1 ? 'Unverified' : null, details: <>
      <dt>Exact value</dt><dd><bdi>{lifetime === null ? 'Not available' : exact.format(lifetime)}</bdi></dd>
      {lifetimes.map(entry => <Fragment key={entry.profile.id}><dt><bdi>{entry.profile.label}</bdi></dt><dd><bdi>{exact.format(entry.tokens)}</bdi></dd></Fragment>)}
      <dt>Identity</dt><dd>Accounts are not verified as distinct, so a sum may count one account twice</dd>
    </> },
  ];
  const open = cards.find(card => card.id === opened);

  return <div className="overview-glance" role="group" aria-label="Summary across accounts" data-overview-glance>
    {cards.map(card => <div key={card.id} data-attention={card.attention} data-overview-summary={card.id}>
      <button type="button" className="overview-glance-trigger" aria-haspopup="dialog" onClick={() => setOpened(card.id)}>
        <span className="overview-glance-name">{card.name}{card.state && <Mark>{card.state}</Mark>}</span>
        <span className="overview-glance-value">{card.value}</span>
        {card.context && <span className="overview-glance-context">{card.context}</span>}
      </button>
    </div>)}
    <DetailPanel open={!!open} title={open?.name} label={`${open?.name ?? 'Summary'} details`} onClose={() => setOpened(null)}>
      {open && <>
        <p className="detail-panel-value">{open.value}</p>
        {open.state && <Mark>{open.state}</Mark>}
        <dl className="detail-rows">{open.details}</dl>
      </>}
    </DetailPanel>
  </div>;
}
