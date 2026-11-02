import { useEffect, useState } from 'react';
import { Meter, MeterIndicator, MeterTrack } from './components/ui/meter';
import { invoke, type ProfileRecord, type SavedProfileQuota, type SavedQuotaWindow } from './contracts';
import { DetailLink } from './detail-panel';
import { ReadingDetails } from './overview-glance';
import { ResetCountdown } from './reset-countdown';
import { DisplayTime } from './time-display';
import './quota-compare.css';

type Column = { key: string; title: string };
type Loaded = { key: string; quotas: Record<string, SavedProfileQuota | null>; failed: string[] };

// Windows align only when limit ID, role and duration all match. Percentages are never summed.
export function comparableWindows(limits: SavedProfileQuota['limits']) {
  return limits.flatMap(limit => ([['primary', limit.primary], ['secondary', limit.secondary]] as const).flatMap(([role, window]) => window ? [{ key: `${limit.limitId}\u0000${role}\u0000${window.windowDurationMins ?? 'unknown'}`, title: `${limit.limitName ?? limit.limitId} · ${window.label}`, window }] : []));
}

function Reading({ label, value, quota }: { label: string; value: SavedQuotaWindow; quota: SavedProfileQuota }) {
  const level = value.derivedRemainingPercent <= 10 ? 'low' : value.derivedRemainingPercent <= 25 ? 'warn' : undefined;
  return <div className="quota-compare-reading" data-attention={level}>
    <DetailLink label={`${label} details`} details={<dl className="detail-rows"><dt>Window</dt><dd>{value.label}</dd><dt>Calculation</dt><dd>100% − {value.usedPercent}% used</dd><dt>Expected reset</dt><dd>{value.resetsAtUnix === null ? 'Not available' : <DisplayTime value={new Date(value.resetsAtUnix * 1000).toISOString()} />}</dd><ReadingDetails quota={quota} /></dl>}><strong>{value.derivedRemainingPercent}% left</strong></DetailLink>
    <Meter min={0} max={100} value={value.derivedRemainingPercent} aria-label={`${label}: ${value.derivedRemainingPercent}% remaining (derived)`}><MeterTrack><MeterIndicator /></MeterTrack></Meter>
    <ResetCountdown detached resetsAtUnix={value.resetsAtUnix} />
  </div>;
}

export function QuotaCompare({ profiles, revision }: { profiles: ProfileRecord[]; revision: number }) {
  const active = profiles.filter(profile => !profile.archived);
  const ids = active.map(profile => profile.id).join(',');
  const key = `${ids}:${revision}`;
  const [loaded, setLoaded] = useState<Loaded | null>(null);
  useEffect(() => {
    if (active.length < 2) return;
    let current = true;
    const list = ids.split(',');
    void Promise.allSettled(list.map(id => invoke('load_profile_quota', { profileId: id }).then(value => {
      if (value && value.profileId !== id) throw new Error('Incompatible saved quota profile.');
      return value;
    }))).then(results => {
      if (!current) return;
      setLoaded({ key, quotas: Object.fromEntries(list.map((id, index) => { const result = results[index]!; return [id, result.status === 'fulfilled' ? result.value : null]; })), failed: list.filter((_, index) => results[index]!.status === 'rejected') });
    });
    return () => { current = false; };
  }, [key]);
  if (active.length < 2) return null;
  const data = loaded?.key === key ? loaded : null;
  const columns = new Map<string, Column>();
  for (const profile of active) for (const { key: column, title } of comparableWindows(data?.quotas[profile.id]?.limits ?? [])) if (!columns.has(column)) columns.set(column, { key: column, title });
  return <section className="usage-card quota-compare" aria-labelledby="quota-compare-heading" aria-busy={!data} data-quota-compare>
    <h2 id="quota-compare-heading">Compare accounts</h2>
    {!data ? <p className="quota-compare-note" role="status">Loading saved quotas…</p>
      : columns.size === 0 ? <p className="quota-compare-note">No saved quota windows to compare yet.</p>
      : <>
        <div className="quota-compare-scroll" tabIndex={0} role="region" aria-label="Quota comparison"><table>
          <thead><tr><th scope="col">Account</th>{[...columns.values()].map(column => <th scope="col" key={column.key}>{column.title}</th>)}</tr></thead>
          <tbody>{active.map(profile => {
            const saved = new Map(comparableWindows(data.quotas[profile.id]?.limits ?? []).map(entry => [entry.key, entry.window]));
            return <tr key={profile.id} data-quota-compare-account={profile.id}><th scope="row"><bdi>{profile.label}</bdi>{data.quotas[profile.id] && data.quotas[profile.id]!.freshness !== 'fresh' && <small>{data.quotas[profile.id]!.freshness === 'stale' ? 'Stale' : 'Freshness unknown'}</small>}{data.failed.includes(profile.id) && <small className="quota-compare-error">Could not load saved quota</small>}</th>
              {[...columns.values()].map(column => { const value = saved.get(column.key); return <td key={column.key}>{value ? <Reading label={`${profile.label}, ${column.title}`} value={value} quota={data.quotas[profile.id]!} /> : <span className="quota-compare-missing">Not available</span>}</td>; })}
            </tr>;
          })}</tbody>
        </table></div>
      </>}
  </section>;
}
