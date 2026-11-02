import { Fragment, useState, type ReactNode } from 'react';
import type { ProfileRecord, SavedProfileQuota, SavedQuotaWindow } from './contracts';
import { DetailPanel, Mark } from './detail-panel';
import { ClockIcon, GaugeIcon, PulseIcon, UsersIcon } from './icons';
import { ResetCountdown } from './reset-countdown';
import { useDateFormat } from './time-display';

type Props = { profiles: ProfileRecord[]; quotas: Record<string, SavedProfileQuota | null | undefined>; plans: string[] };
type Entry = { profile: ProfileRecord; quota: SavedProfileQuota; window: SavedQuotaWindow; limitId: string };
const freshness = { fresh: 'Current', stale: 'Stale', unknown: 'Freshness unknown' };

function age(observedAtUtc: string) {
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

// Select one saved window; never sum percentages or infer identity across profiles.
export function OverviewGlance({ profiles, quotas, plans }: Props) {
  const dates = useDateFormat();
  const [opened, setOpened] = useState<string | null>(null);
  const connected = profiles.filter(profile => profile.connectionState === 'connected');
  const saved = connected.flatMap(profile => quotas[profile.id] ? [{ profile, quota: quotas[profile.id]! }] : []);
  const readings: Entry[] = saved.flatMap(({ profile, quota }) => quota.limits.flatMap(limit =>
    [limit.primary, limit.secondary].flatMap(window => window ? [{ profile, quota, window, limitId: limit.limitId }] : [])));
  const lowest = readings.reduce<Entry | null>((best, entry) => !best || entry.window.derivedRemainingPercent < best.window.derivedRemainingPercent ? entry : best, null);
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
  const missing = <span className="overview-glance-empty">Not available</span>;

  type Card = { id: string; name: string; icon: ReactNode; value: ReactNode; details: ReactNode; state?: string | null; attention?: string | undefined };
  const cards: Card[] = [
    { id: 'accounts', name: 'Connected accounts', icon: <UsersIcon />, value: connected.length, details: <>
      <dt>Source</dt><dd>Local profiles</dd>
      <dt>Active accounts</dt><dd>{profiles.length}</dd>
      <dt>Needs sign-in</dt><dd>{profiles.length - connected.length}</dd>
      <dt>Saved plans</dt><dd>{counts.size ? [...counts].map(([plan, count]) => <div key={plan}>{count} {plan}</div>) : 'Not available'}</dd>
    </> },
    { id: 'quota', name: 'Lowest quota left', icon: <GaugeIcon />, value: lowest ? `${lowest.window.derivedRemainingPercent}%` : missing, state: lowest ? mark : null, attention: lowest ? lowest.window.derivedRemainingPercent <= 10 ? 'low' : lowest.window.derivedRemainingPercent <= 25 ? 'warn' : undefined : undefined, details: <>
      {lowest ? <><WindowDetails entry={lowest} /><dt>Calculation</dt><dd>100% − {lowest.window.usedPercent}% used</dd></> : <><dt>Reason</dt><dd>No saved quota windows for connected accounts</dd></>}
      {coverage}
    </> },
    { id: 'reset', name: 'Next expected reset', icon: <ClockIcon />, value: reset ? <ResetCountdown detached resetsAtUnix={reset.window.resetsAtUnix} /> : missing, state: reset ? mark : null, details: <>
      {reset ? <><WindowDetails entry={reset} /><dt>Expected reset</dt><dd><time dateTime={new Date(reset.window.resetsAtUnix! * 1000).toISOString()}>{dates.format(new Date(reset.window.resetsAtUnix! * 1000))}</time></dd><dt>Confirmation</dt><dd>Time alone does not confirm a reset</dd></> : <><dt>Reason</dt><dd>No upcoming reset time reported for connected accounts</dd></>}
      {coverage}
    </> },
    { id: 'latest', name: 'Latest reading', icon: <PulseIcon />, value: latest ? <time dateTime={latest.quota.observedAtUtc}>{age(latest.quota.observedAtUtc)}</time> : missing, state: latest ? mark : null, details: <>
      {latest ? <><dt>Account</dt><dd><bdi>{latest.profile.label}</bdi></dd><ReadingDetails quota={latest.quota} /></> : <><dt>Reason</dt><dd>No saved quota for connected accounts</dd></>}
      {coverage}
    </> },
  ];
  const open = cards.find(card => card.id === opened);

  return <div className="overview-glance" role="group" aria-label="Summary across accounts" data-overview-glance>
    {cards.map(card => <div key={card.id} data-attention={card.attention} data-overview-summary={card.id}>
      <button type="button" className="overview-glance-trigger" aria-haspopup="dialog" onClick={() => setOpened(card.id)}>
        <span className="overview-glance-name">{card.icon}{card.name}</span>
        <span className="overview-glance-value">{card.value}</span>
        {card.state && <Mark>{card.state}</Mark>}
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
