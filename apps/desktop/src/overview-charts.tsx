import { useEffect, useRef, useState, type ReactNode, type RefObject } from 'react';
import type { ActivityDashboard, ProfileRecord } from './contracts';
import { DetailLink, Mark } from './detail-panel';

const exact = new Intl.NumberFormat('en-US');
const compact = new Intl.NumberFormat('en-US', { notation: 'compact', maximumFractionDigits: 1 });
const big = (value: string | null | undefined) => value === null || value === undefined ? null : BigInt(value);
// Geometry only: a 0–1 share of the scale, from exact integers.
const ratio = (value: bigint, max: bigint) => max > 0n ? Number(value * 100_000n / max) / 100_000 : 0;
export const compactCount = (value: bigint) => compact.format(value);
const weekdays = ['Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday', 'Saturday', 'Sunday'];
const weekday = (date: string) => (new Date(`${date}T00:00:00Z`).getUTCDay() + 6) % 7;
const monthDay = (date: string) => new Date(`${date}T00:00:00Z`).toLocaleDateString('en-US', { month: 'short', day: 'numeric', timeZone: 'UTC' });

// A round scale top and four gridlines for an axis of token counts.
function scale(max: bigint) {
  if (max <= 0n) return { top: 4n, ticks: [0n, 1n, 2n, 3n, 4n] };
  const rough = max / 4n + 1n;
  let unit = 1n;
  while (unit * 10n <= rough) unit *= 10n;
  const step = [1n, 2n, 5n, 10n].map(m => m * unit).find(s => s >= rough) ?? unit * 10n;
  return { top: step * 4n, ticks: [0n, step, step * 2n, step * 3n, step * 4n] };
}

type Account = { profile: ProfileRecord; slot: number };

// Charts draw at their real width so text stays the same size in every window.
function useWidth(fallback: number) {
  const ref = useRef<HTMLElement>(null);
  const [width, setWidth] = useState(fallback);
  useEffect(() => {
    const node = ref.current;
    if (!node) return;
    const observer = new ResizeObserver(entries => { const next = Math.round(entries[0]!.contentRect.width); if (next > 0) setWidth(next); });
    observer.observe(node);
    return () => observer.disconnect();
  }, []);
  return [ref, width] as const;
}

// Tokens per day, one stacked segment per account. A day with no reading from any account is drawn as an outline.
export function TokensPerDay({ accounts, series, period }: { accounts: Account[]; series: Record<string, ActivityDashboard | null> | null; period: ActivityDashboard | null }) {
  const [frame, measured] = useWidth(960);
  const dates = period?.cells.map(cell => cell.startDate) ?? [];
  if (!period || !dates.length) return <figure className="overview-chart" ref={frame}><p className="overview-chart-empty">No saved daily activity for these dates.</p></figure>;
  const shown = accounts.filter(account => series?.[account.profile.id]?.cells.length);
  const byAccount = new Map(shown.map(account => [account.profile.id, new Map(series![account.profile.id]!.cells.map(cell => [cell.startDate, big(cell.tokens)]))]));
  const days = dates.map(date => {
    const parts = shown.map(account => ({ account, tokens: byAccount.get(account.profile.id)!.get(date) ?? null }));
    const known = parts.filter(part => part.tokens !== null);
    const total = known.reduce((sum, part) => sum + part.tokens!, 0n);
    return { date, parts, known: known.length, total, missing: !known.length };
  });
  const { top, ticks } = scale(days.reduce((max, day) => day.total > max ? day.total : max, 0n));
  const width = Math.max(280, measured), height = width < 520 ? 220 : 280, left = 44, bottom = 26, plot = height - bottom - 12;
  const step = (width - left - 4) / days.length, bar = Math.max(1.5, Math.min(22, step - (step > 6 ? 3 : 1)));
  const y = (value: bigint) => height - bottom - ratio(value, top) * plot;
  const firstOfMonth = days.filter((day, index) => index === 0 || day.date.endsWith('-01'));
  const labelEvery = Math.max(1, Math.ceil(64 / step));
  return <figure className="overview-chart" data-overview-days ref={frame}>
    <ul className="overview-key" aria-label="Accounts">{shown.map(({ profile, slot }) => <li key={profile.id}><i data-tint={slot} aria-hidden="true" /><bdi>{profile.label}</bdi></li>)}<li><i data-missing aria-hidden="true" />No reading</li></ul>
    <svg viewBox={`0 0 ${width} ${height}`} role="img" aria-label={`Tokens per day by account, ${period.periodStart} to ${period.periodEnd}`}>
      {ticks.map(tick => <g key={String(tick)}><line x1={left} x2={width - 4} y1={y(tick)} y2={y(tick)} className="overview-grid" /><text x={left - 10} y={y(tick) + 4} textAnchor="end" className="overview-axis">{tick === 0n ? '0' : compactCount(tick)}</text></g>)}
      {days.map((day, index) => {
        const x = left + index * step + (step - bar) / 2;
        if (day.missing) return <rect key={day.date} x={x} y={height - bottom - 6} width={bar} height={6} rx={2} className="overview-missing"><title>{`${day.date}: no reading`}</title></rect>;
        let base = 0n;
        return <g key={day.date}><title>{`${day.date}: ${exact.format(day.total)} tokens${day.known < shown.length ? ', incomplete' : ''}`}</title>
          {day.parts.map(({ account, tokens }) => {
            if (!tokens) return null;
            const y0 = y(base), y1 = y(base + tokens);
            base += tokens;
            return <rect key={account.profile.id} x={x} y={y1} width={bar} height={Math.max(0, y0 - y1 - (y0 - y1 > 3 ? 1.5 : 0))} rx={Math.min(3, bar / 2)} data-tint={account.slot} className="overview-segment" />;
          })}
        </g>;
      })}
      {firstOfMonth.filter((day, index) => index === 0 || days.indexOf(day) >= labelEvery).map(day => <text key={day.date} x={left + days.indexOf(day) * step} y={height - 6} className="overview-axis">{monthDay(day.date)}</text>)}
    </svg>
  </figure>;
}

// Share of the chosen accounts' tokens, from Rust's shares. Parts are ranked; colour still follows the account.
export function ShareDonut({ data, slots, view }: { data: ActivityDashboard | null; slots: Record<string, number>; view: string }) {
  if (view === 'individual' || !data?.members.length) return null;
  const parts = data.members.filter(member => member.shareBasisPoints !== null).sort((a, b) => b.shareBasisPoints! - a.shareBasisPoints!);
  const radius = 62, circumference = 2 * Math.PI * radius;
  let offset = 0;
  const total = big(data.totalTokens);
  const percent = (basisPoints: number) => `${(basisPoints / 100).toFixed(basisPoints % 100 ? 1 : 0)}%`;
  return <section className="overview-share" aria-labelledby="overview-share-heading" data-overview-account-share data-aggregation={data.status}>
    <div className="overview-share-heading"><h3 id="overview-share-heading">Share</h3>{view === 'profile_sum' && <Mark>Unverified</Mark>}</div>
    {parts.some(part => part.shareBasisPoints! > 0) ? <div className="overview-donut-ring" data-activity-share-chart>
      <svg viewBox="0 0 160 160" role="img" aria-label={`Share of tokens: ${parts.map(part => `${part.label} ${percent(part.shareBasisPoints!)}`).join(', ')}`}>
        {parts.map(part => {
          const length = part.shareBasisPoints! / 10_000 * circumference;
          const arc = <circle key={part.profileId} cx="80" cy="80" r={radius} data-tint={slots[part.profileId] ?? 0} className="overview-arc" strokeDasharray={`${Math.max(0, length - (parts.length > 1 ? 3 : 0))} ${circumference}`} strokeDashoffset={-offset} transform="rotate(-90 80 80)"><title>{`${part.label}: ${percent(part.shareBasisPoints!)}`}</title></circle>;
          offset += length;
          return arc;
        })}
      </svg>
      <span className="overview-donut-total">{total === null ? 'Not available' : <BigNumber value={total} />}</span>
    </div> : <p className="overview-chart-empty">{data.totalTokens === '0' ? 'No shares for a zero-token total.' : 'No eligible reported activity.'}</p>}
    <ol className="overview-share-legend" aria-label="Activity contribution breakdown">{parts.map(part => <li key={part.profileId} data-share-profile={part.profileId}><i data-tint={slots[part.profileId] ?? 0} aria-hidden="true" /><bdi>{part.label}</bdi><span>{percent(part.shareBasisPoints!)}</span><span className="overview-share-tokens">{part.tokens === null ? 'Not available' : compactCount(BigInt(part.tokens))}</span></li>)}
      {data.members.filter(member => member.shareBasisPoints === null).map(member => <li key={member.profileId} data-share-profile={member.profileId}><i data-missing aria-hidden="true" /><bdi>{member.label}</bdi><span>Not available</span></li>)}</ol>
    <DetailLink className="text-button" label="Values and coverage" details={<><p>{data.knownContributions} of {data.expectedContributions} profile-days reported. Shares use observed subtotals for the same selected dates. Missing values remain unknown.</p><table className="activity-share-values"><thead><tr><th>Account</th><th>Observed tokens</th><th>Days reported</th></tr></thead><tbody>{data.members.map(m => <tr key={m.profileId}><th><bdi>{m.label}</bdi></th><td>{m.tokens === null ? 'Unavailable' : exact.format(BigInt(m.tokens))}</td><td>{m.knownDays}</td></tr>)}</tbody></table><ul>{data.members.map(m => <li key={m.profileId}><bdi>{m.label}</bdi>: {m.sourceKind === 'test_fixture' ? 'Synthetic test fixture' : 'Official OpenAI provider'}. {data.scopeVerified ? m.scope : 'Application scope and day definition unverified'}. Last read: <time dateTime={m.observedAtUtc}>{m.observedAtUtc}</time>.</li>)}</ul></>}>Values and coverage</DetailLink>
  </section>;
}

// The big number: whole part in full ink, the decimals and unit dimmed.
export function BigNumber({ value }: { value: bigint }) {
  const text = compactCount(value), match = /^([\d,]+)(.*)$/.exec(text);
  return <bdi title={exact.format(value)}>{match ? <>{match[1]}<span className="overview-dim">{match[2]}</span></> : text}</bdi>;
}

// Average tokens per weekday over the chosen dates, counting only days with a reading.
export function WeekRhythm({ period }: { period: ActivityDashboard | null }) {
  const sums = weekdays.map(() => ({ tokens: 0n, days: 0 }));
  for (const cell of period?.cells ?? []) if (cell.tokens !== null) { const day = sums[weekday(cell.startDate)]!; day.tokens += BigInt(cell.tokens); day.days++; }
  const averages = sums.map(day => day.days ? day.tokens / BigInt(day.days) : null);
  const known = averages.filter((value): value is bigint => value !== null);
  const [frame, measured] = useWidth(520);
  if (known.length < 7) return <Panel title="Your week" frame={frame}><p className="overview-chart-empty">A week of readings is needed for this.</p></Panel>;
  const peak = known.reduce((max, value) => value > max ? value : max, 0n), peakDay = averages.indexOf(peak);
  const weekend = (averages[5]! + averages[6]!) / 2n;
  const width = Math.max(260, measured), height = 200, bottom = 26, plot = height - bottom - 30, slot = width / 7, bar = Math.max(12, slot - Math.min(22, slot * .3));
  return <Panel title="Your week" frame={frame} context={`${weekdays[peakDay]}s are your busiest day. Weekends run at ${peak > 0n ? Math.round(ratio(weekend, peak) * 100) : 0}% of that.`}>
    <svg viewBox={`0 0 ${width} ${height}`} role="img" aria-label={`Average tokens per weekday: ${weekdays.map((day, index) => `${day} ${compactCount(averages[index]!)}`).join(', ')}`} data-overview-week>
      {averages.map((value, index) => {
        const h = Math.max(3, ratio(value!, peak) * plot), x = index * slot + (slot - bar) / 2, top = height - bottom - h;
        return <g key={weekdays[index]}><rect x={x} y={top} width={bar} height={h} rx={6} className={index === peakDay ? 'overview-week-peak' : 'overview-week-bar'}><title>{`${weekdays[index]}: ${exact.format(value!)} tokens on average`}</title></rect>
          {index === peakDay && <text x={x + bar / 2} y={top - 9} textAnchor="middle" className="overview-axis overview-axis-strong">{compactCount(value!)}</text>}
          <text x={x + bar / 2} y={height - 6} textAnchor="middle" className={index === peakDay ? 'overview-axis overview-axis-strong' : 'overview-axis'}>{weekdays[index]!.slice(0, 3)}</text></g>;
      })}
    </svg>
  </Panel>;
}

// Running totals of known days for the chosen period and the one before it, on one axis.
export function PeriodCompare({ period, previous, days }: { period: ActivityDashboard | null; previous: ActivityDashboard | null; days: number }) {
  const run = (data: ActivityDashboard | null) => { let sum = 0n; return (data?.cells ?? []).map(cell => (sum += cell.tokens === null ? 0n : BigInt(cell.tokens))); };
  const now = run(period), before = run(previous);
  const nowTotal = big(period?.totalTokens), beforeTotal = big(previous?.totalTokens);
  const title = `Last ${days} days and the ${days} before`;
  const [frame, measured] = useWidth(520);
  if (now.length < 2 || before.length < 2 || nowTotal === null || beforeTotal === null) return <Panel title={title} frame={frame}><p className="overview-chart-empty">Not available until both periods have readings.</p></Panel>;
  const top = scale(now.at(-1)! > before.at(-1)! ? now.at(-1)! : before.at(-1)!).top;
  const width = Math.max(260, measured), height = 200, left = 6, right = 118, bottom = 24, plot = height - bottom - 14;
  const x = (index: number, length: number) => left + index / (length - 1) * (width - left - right);
  const y = (value: bigint) => height - bottom - ratio(value, top) * plot;
  const path = (values: bigint[]) => values.map((value, index) => `${index ? 'L' : 'M'}${x(index, values.length).toFixed(1)},${y(value).toFixed(1)}`).join(' ');
  const ahead = nowTotal >= beforeTotal, gap = ahead ? nowTotal - beforeTotal : beforeTotal - nowTotal;
  const yNow = y(now.at(-1)!), yBefore = y(before.at(-1)!), apart = Math.abs(yNow - yBefore) < 16 ? (ahead ? -8 : 8) : 0;
  return <Panel title={title} frame={frame} context={`${ahead ? 'Ahead' : 'Behind'} by ${compactCount(gap)}. Days without a reading add nothing.`}>
    <svg viewBox={`0 0 ${width} ${height}`} role="img" aria-label={`Running total: ${compactCount(nowTotal)} in the last ${days} days, ${compactCount(beforeTotal)} in the ${days} before`} data-overview-compare-periods>
      {[1, 2, 3].map(step => <line key={step} x1={left} x2={width - right + 8} y1={y(top / 4n * BigInt(step))} y2={y(top / 4n * BigInt(step))} className="overview-grid" />)}
      <path d={`${path(now)} L${x(now.length - 1, now.length)},${y(0n)} L${x(0, now.length)},${y(0n)} Z`} className="overview-area" />
      <path d={path(before)} className="overview-line-before" /><path d={path(now)} className="overview-line-now" />
      <circle cx={x(now.length - 1, now.length)} cy={yNow} r={4.5} className="overview-dot-now" /><circle cx={x(before.length - 1, before.length)} cy={yBefore} r={4} className="overview-dot-before" />
      <text x={width - right + 14} y={yNow + 4 + apart} className="overview-axis overview-axis-strong">{compactCount(nowTotal)} now</text>
      <text x={width - right + 14} y={yBefore + 4 - apart} className="overview-axis">{compactCount(beforeTotal)} before</text>
    </svg>
  </Panel>;
}

function Panel({ title, context, frame, children }: { title: string; context?: string; frame: RefObject<HTMLElement | null>; children: ReactNode }) {
  return <section className="overview-chart-panel" aria-label={title}><h3>{title}</h3>{context && <p className="overview-chart-context">{context}</p>}<div className="overview-chart-frame" ref={frame as RefObject<HTMLDivElement>}>{children}</div></section>;
}
