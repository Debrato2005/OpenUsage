import { useEffect, useRef, useState } from 'react';
import { Button } from './components/ui/button';
import { invoke, type LocalToolTotals, type LocalUsageDays, type LocalUsagePeriod, type LocalUsageRange, type LocalUsageView } from './contracts';
import { DetailLink, Mark, useOpenDetail } from './detail-panel';
import { BigNumber, compactCount } from './overview-charts';
import { localLogToolNames } from './local-logs';
import { ShareButton } from './share';
import './local-usage.css';
import { localLogToolTints as toolTint } from './local-tools';

const exact = new Intl.NumberFormat('en-US');
// Rounds millionths of a dollar to cents for display. The stored value keeps every digit.
const dollars = (micros: string) => {
  const cents = (BigInt(micros) + 5_000n) / 10_000n;
  return `$${exact.format(cents / 100n)}.${String(cents % 100n).padStart(2, '0')}`;
};
const day = (date: string) => new Date(`${date}T00:00:00Z`);
const iso = (value: Date) => value.toISOString().slice(0, 10);
const monthDay = (date: string) => day(date).toLocaleDateString('en-US', { month: 'short', day: 'numeric', timeZone: 'UTC' });
const monthName = (date: string) => day(date).toLocaleDateString('en-US', { month: 'short', year: 'numeric', timeZone: 'UTC' });

/** Saved local totals. A background scan can add days, so the view reloads while it is shown. */
export function useLocalUsage(active: boolean, range: LocalUsageRange | null, period: LocalUsagePeriod) {
  const [view, setView] = useState<LocalUsageView | null>(null);
  const [failed, setFailed] = useState(false);
  const key = range && JSON.stringify(range);
  useEffect(() => {
    if (!active || !key) return;
    let current = true;
    const load = () => void invoke('load_local_usage', { ...(JSON.parse(key) as LocalUsageRange), period })
      .then(value => { if (current) { setView(value); setFailed(false); } })
      .catch(() => { if (current) setFailed(true); });
    load();
    const timer = window.setInterval(load, 15_000);
    return () => { current = false; window.clearInterval(timer); };
  }, [active, key, period]);
  return { view, failed };
}

function Estimate({ totals }: { totals: LocalToolTotals }) {
  if (totals.estimatedValueUsdMicros === null) return <>Estimated value Not available</>;
  return <>Estimated {dollars(totals.estimatedValueUsdMicros)}{!totals.estimateComplete && <Mark>Incomplete</Mark>}</>;
}

function ToolDetails({ totals, view }: { totals: LocalToolTotals; view: LocalUsageView }) {
  const differences = view.differences.filter(d => d.tool === totals.tool);
  return <dl className="detail-rows">
    <dt>Input</dt><dd>{exact.format(BigInt(totals.inputTokens))}</dd>
    <dt>Output</dt><dd>{exact.format(BigInt(totals.outputTokens))}</dd>
    <dt>Cache write</dt><dd>{exact.format(BigInt(totals.cacheWriteTokens))}</dd>
    <dt>Cache read</dt><dd>{exact.format(BigInt(totals.cacheReadTokens))}</dd>
    <dt>Total</dt><dd>{exact.format(BigInt(totals.totalTokens))}</dd>
    <dt>Estimated value</dt><dd>{totals.estimatedValueUsdMicros === null ? 'Not available' : `${dollars(totals.estimatedValueUsdMicros)} at public API prices, not money paid`}{!totals.estimateComplete && totals.estimatedValueUsdMicros !== null && '. Some models have no price.'}</dd>
    <dt>Source</dt><dd>Local logs on this device, read by ccusage {view.scannerVersions.join(', ') || 'Not available'}</dd>
    <dt>Dates</dt><dd>{view.startDate} to {view.endDate}, days in {view.timeZone}</dd>
    {differences.length > 0 && <><dt>Logs gone</dt><dd><ul className="local-differences">{differences.map(d => <li key={`${d.date}:${d.model}:${d.newTotalTokens}`}>{d.date}, {d.model}: saved {exact.format(BigInt(d.savedTotalTokens))}, a later scan read {exact.format(BigInt(d.newTotalTokens))}</li>)}</ul></dd></>}
  </dl>;
}

function ToolCard({ totals, view }: { totals: LocalToolTotals; view: LocalUsageView }) {
  const open = useOpenDetail();
  const name = localLogToolNames[totals.tool];
  return <article className="local-tool-card" data-tint={toolTint[totals.tool]} data-local-tool={totals.tool} aria-labelledby={`local-tool-${totals.tool}`}>
    <h3 id={`local-tool-${totals.tool}`}><i aria-hidden="true" />{name}</h3>
    <button type="button" className="local-tool-total" aria-haspopup="dialog" aria-label={`${name}: ${exact.format(BigInt(totals.totalTokens))} tokens`} data-local-tool-total={totals.totalTokens}
      onClick={() => open({ label: `${name} details`, title: `${name} tokens`, content: <ToolDetails totals={totals} view={view} /> })}>
      <BigNumber value={BigInt(totals.totalTokens)} /><span className="local-tool-unit">tokens</span>
    </button>
    <p className="local-tool-context"><Estimate totals={totals} /></p>
  </article>;
}

// Every slot from start to end, so a day with no saved usage shows as a gap, not as zero.
function slots(view: LocalUsageView, period: LocalUsagePeriod) {
  const result: string[] = [];
  const end = day(view.endDate);
  const cursor = day(view.buckets.reduce((min, b) => b.startDate < min ? b.startDate : min, view.startDate));
  while (cursor <= end && result.length < 400) {
    result.push(iso(cursor));
    if (period === 'day') cursor.setUTCDate(cursor.getUTCDate() + 1);
    else if (period === 'week') cursor.setUTCDate(cursor.getUTCDate() + 7);
    else cursor.setUTCMonth(cursor.getUTCMonth() + 1, 1);
  }
  return result;
}

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

/** Tokens per day, week or month, one stacked segment per tool. */
// Every UTC hour that starts in the range. In a zone with a half-hour offset, hours start at :30 local (SC-021).
function hourSlots(view: LocalUsageView) {
  const result: string[] = [];
  if (!view.startUtc || !view.endUtc) return result;
  const end = Date.parse(view.endUtc);
  for (let t = Math.ceil(Date.parse(view.startUtc) / 3_600_000) * 3_600_000; t < end && result.length < 24 * 8; t += 3_600_000) result.push(new Date(t).toISOString());
  return result;
}

function LocalBars({ view, period }: { view: LocalUsageView; period: LocalUsagePeriod }) {
  const [frame, measured] = useWidth(960);
  const hourly = period === 'hour';
  const tools = view.tools.map(t => t.tool).filter(tool => !hourly || view.hourTools.includes(tool));
  const missing = hourly ? view.tools.map(t => t.tool).filter(tool => !view.hourTools.includes(tool)) : [];
  const totals = hourly
    ? new Map(view.hours.map(h => [`${h.startUtc}:${h.tool}`, BigInt(h.totalTokens)]))
    : new Map(view.buckets.map(b => [`${b.startDate}:${b.tool}`, BigInt(b.totalTokens)]));
  const local = (utc: string, options: Intl.DateTimeFormatOptions) => new Date(utc).toLocaleString('en-US', { timeZone: view.timeZone, ...options });
  const when = (start: string) => hourly ? local(start, { month: 'short', day: 'numeric', hour: 'numeric', minute: '2-digit' }) : start;
  const columns = (hourly ? hourSlots(view) : slots(view, period)).map(start => {
    const parts = tools.map(tool => ({ tool, tokens: totals.get(`${start}:${tool}`) ?? 0n }));
    return { start, parts, total: parts.reduce((sum, p) => sum + p.tokens, 0n) };
  });
  const max = columns.reduce((m, c) => c.total > m ? c.total : m, 0n);
  let unit = 1n; while (unit * 10n <= max / 4n + 1n) unit *= 10n;
  const step = [1n, 2n, 5n, 10n].map(m => m * unit).find(s => s * 4n >= max) ?? unit * 10n;
  const top = step * 4n, ticks = [0n, step, step * 2n, step * 3n, top];
  const width = Math.max(280, measured), height = width < 520 ? 200 : 240, left = 44, bottom = 26, plot = height - bottom - 12;
  const slot = (width - left - 4) / Math.max(1, columns.length), bar = Math.max(1.5, Math.min(26, slot - (slot > 6 ? 3 : 1)));
  const y = (value: bigint) => height - bottom - (top > 0n ? Number(value * 100_000n / top) / 100_000 : 0) * plot;
  const labelEvery = Math.max(1, Math.ceil(64 / slot));
  // A long hourly range labels the first hour of each local day with its date.
  const dayOf = (utc: string) => new Date(utc).toLocaleDateString('en-CA', { timeZone: view.timeZone });
  const dayLabels = hourly && columns.length > 48;
  const labelled = (index: number) => dayLabels ? index === 0 || dayOf(columns[index]!.start) !== dayOf(columns[index - 1]!.start) : index % labelEvery === 0;
  const label = (start: string) => hourly ? dayLabels ? local(start, { month: 'short', day: 'numeric' }) : local(start, { hour: 'numeric', minute: '2-digit' }) : period === 'month' ? monthName(start) : monthDay(start);
  const name = hourly ? 'hour' : period === 'day' ? 'day' : period === 'week' ? 'week' : 'month';
  return <figure className="overview-chart local-bars" ref={frame} data-local-bars={period}>
    <ul className="overview-key" aria-label="Tools">{tools.map(tool => <li key={tool}><i data-tint={toolTint[tool]} aria-hidden="true" />{localLogToolNames[tool]}</li>)}
      {missing.map(tool => <li key={tool} data-local-no-hours={tool}><i data-tint={toolTint[tool]} aria-hidden="true" />{localLogToolNames[tool]}<Mark>Not available</Mark></li>)}</ul>
    <svg viewBox={`0 0 ${width} ${height}`} role="img" aria-label={`Tokens per ${name} by tool, ${view.startDate} to ${view.endDate}`}>
      {ticks.map(tick => <g key={String(tick)}><line x1={left} x2={width - 4} y1={y(tick)} y2={y(tick)} className="overview-grid" /><text x={left - 10} y={y(tick) + 4} textAnchor="end" className="overview-axis">{tick === 0n ? '0' : compactCount(tick)}</text></g>)}
      {columns.map((column, index) => {
        const x = left + index * slot + (slot - bar) / 2;
        let base = 0n;
        return <g key={column.start}><title>{column.total === 0n ? `${when(column.start)}: no saved usage` : `${when(column.start)}: ${exact.format(column.total)} tokens`}</title>
          {column.parts.map(({ tool, tokens }) => {
            if (!tokens) return null;
            const y0 = y(base), y1 = y(base + tokens);
            base += tokens;
            return <rect key={tool} x={x} y={y1} width={bar} height={Math.max(1, y0 - y1 - (y0 - y1 > 3 ? 1.5 : 0))} rx={Math.min(3, bar / 2)} data-tint={toolTint[tool]} className="overview-segment" />;
          })}
        </g>;
      })}
      {columns.map((column, index) => labelled(index) && <text key={`label:${column.start}`} x={left + index * slot} y={height - 6} className="overview-axis">{label(column.start)}</text>)}
    </svg>
    <table className="sr-only"><caption>Tokens per {name}</caption><thead><tr><th scope="col">Start</th>{tools.map(tool => <th key={tool} scope="col">{localLogToolNames[tool]}</th>)}</tr></thead>
      <tbody>{columns.filter(c => c.total > 0n).map(c => <tr key={c.start}><th scope="row">{when(c.start)}</th>{c.parts.map(p => <td key={p.tool}>{exact.format(p.tokens)}</td>)}</tr>)}</tbody></table>
  </figure>;
}

/** Personal records for the chosen dates. Each number opens the detail panel (OU-DES-007). */
function Records({ view }: { view: LocalUsageView }) {
  const where = <><dt>Dates</dt><dd>{view.startDate} to {view.endDate}, days in {view.timeZone}</dd><dt>Source</dt><dd>Saved local log totals on this device</dd></>;
  const busiest = view.busiestDay;
  return <section className="local-records" aria-label="Records" data-local-records>
    <DetailLink className="local-record" label={`Active days: ${view.activeDays}`} details={<dl className="detail-rows"><dt>Active days</dt><dd>{view.activeDays}, the days with saved usage</dd>{where}</dl>}>
      <span>Active days</span><strong>{view.activeDays}</strong></DetailLink>
    <DetailLink className="local-record" label={`Longest streak: ${view.longestStreakDays} days`} details={<dl className="detail-rows"><dt>Longest streak</dt><dd>{view.longestStreakDays} days in a row with saved usage</dd>{where}</dl>}>
      <span>Longest streak</span><strong>{view.longestStreakDays} days</strong></DetailLink>
    <DetailLink className="local-record" label={busiest ? `Busiest day: ${busiest.date}, ${exact.format(BigInt(busiest.totalTokens))} tokens` : 'Busiest day: Not available'} details={<dl className="detail-rows"><dt>Busiest day</dt><dd>{busiest ? `${busiest.date}, ${exact.format(BigInt(busiest.totalTokens))} tokens` : 'Not available'}</dd>{where}</dl>}>
      <span>Busiest day</span><strong>{busiest ? monthDay(busiest.date) : 'Not available'}</strong>{busiest && <small>{compactCount(BigInt(busiest.totalTokens))} tokens</small>}</DetailLink>
  </section>;
}

function Models({ view }: { view: LocalUsageView }) {
  return <section className="local-models" aria-labelledby="local-models-heading">
    <h3 id="local-models-heading">Models</h3>
    <table><thead><tr><th scope="col">Model</th><th scope="col">Tool</th><th scope="col">Tokens</th><th scope="col">Estimated value</th></tr></thead>
      <tbody>{view.models.map(m => <tr key={`${m.tool}:${m.model}`}><th scope="row"><bdi>{m.model}</bdi></th><td>{localLogToolNames[m.tool]}</td><td>{exact.format(BigInt(m.totalTokens))}</td><td>{m.estimatedValueUsdMicros === null ? 'Not available' : dollars(m.estimatedValueUsdMicros)}</td></tr>)}</tbody></table>
  </section>;
}

function Projects({ view }: { view: LocalUsageView }) {
  if (view.projects.length === 0) return null;
  return <section className="local-models" aria-labelledby="local-projects-heading" data-local-projects>
    <h3 id="local-projects-heading">Projects</h3>
    <table><thead><tr><th scope="col">Project</th><th scope="col">Tool</th><th scope="col">Sessions</th><th scope="col">Tokens</th></tr></thead>
      <tbody>{view.projects.map(p => <tr key={`${p.tool}:${p.project}`}><th scope="row"><bdi>{p.project}</bdi></th><td>{localLogToolNames[p.tool]}</td><td>{p.sessions}</td><td>{exact.format(BigInt(p.totalTokens))}</td></tr>)}</tbody></table>
  </section>;
}

/** Sessions that ended in the range, newest first. A tool that reports no start or reasoning shows it as missing (SC-021). */
function Sessions({ view }: { view: LocalUsageView }) {
  if (view.sessions.length === 0) return null;
  const when = (utc: string) => new Date(utc).toLocaleString('en-US', { timeZone: view.timeZone, month: 'short', day: 'numeric', hour: 'numeric', minute: '2-digit' });
  return <section className="local-models" aria-labelledby="local-sessions-heading" data-local-sessions>
    <h3 id="local-sessions-heading">Sessions</h3>
    <table><thead><tr><th scope="col">Ended</th><th scope="col">Tool</th><th scope="col">Project</th><th scope="col">Started</th><th scope="col">Tokens</th><th scope="col">Reasoning</th></tr></thead>
      <tbody>{view.sessions.map((s, index) => <tr key={`${s.tool}:${s.key}:${index}`} data-local-session={s.tool}><th scope="row"><time dateTime={s.endUtc}>{when(s.endUtc)}</time></th><td>{localLogToolNames[s.tool]}</td>
        <td>{s.project === null ? 'Not available' : <bdi>{s.project}</bdi>}</td><td>{s.startUtc === null ? 'Not available' : <time dateTime={s.startUtc}>{when(s.startUtc)}</time>}</td>
        <td>{exact.format(BigInt(s.totalTokens))}</td><td>{s.reasoningTokens === null ? 'Not available' : exact.format(BigInt(s.reasoningTokens))}</td></tr>)}</tbody></table>
  </section>;
}

const ranges: [LocalUsageDays, string][] = [[7, '7 days'], [30, '30 days'], [90, '90 days'], [365, 'Year']];
const periods: [LocalUsagePeriod, string][] = [['hour', 'Hourly'], ['day', 'Daily'], ['week', 'Weekly'], ['month', 'Monthly']];

/** The This device scope in Usage (OU-LOG-016, OU-LOG-017). */
// A custom range is complete when both dates are set, in order, and at most 366 days apart.
function customRange(start: string, end: string): LocalUsageRange | null {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(start) || !/^\d{4}-\d{2}-\d{2}$/.test(end) || start > end) return null;
  return (day(end).getTime() - day(start).getTime()) / 86_400_000 < 366 ? { since: start, until: end } : null;
}

export function LocalUsagePanel({ active }: { active: boolean }) {
  const [days, setDays] = useState<LocalUsageDays | 'custom'>(30);
  const [custom, setCustom] = useState({ start: '', end: '' });
  const [period, setPeriod] = useState<LocalUsagePeriod>('day');
  const range = days === 'custom' ? customRange(custom.start, custom.end) : { days };
  // Hours cover at most 7 days. A longer range falls back to days (SC-021).
  const hourlyAllowed = !!range && ('days' in range ? range.days === 7 : (day(range.until).getTime() - day(range.since).getTime()) / 86_400_000 < 7);
  const shown = period === 'hour' && !hourlyAllowed ? 'day' : period;
  const { view, failed } = useLocalUsage(active, range, shown);
  const openCustom = () => { if (view && !custom.start) setCustom({ start: view.startDate, end: view.endDate }); setDays('custom'); };
  return <section className="local-usage" aria-label="This device" data-local-usage>
    <div className="overview-hero-controls">
      <div className="overview-chips" role="group" aria-label="Dates">{ranges.map(([value, label]) => <Button key={value} variant="ghost" aria-pressed={days === value} onClick={() => setDays(value)}>{label}</Button>)}
        <Button variant="ghost" aria-pressed={days === 'custom'} onClick={openCustom} data-local-custom>Custom</Button></div>
      <div className="overview-chips" role="group" aria-label="Grouping">{periods.map(([value, label]) => <Button key={value} variant="ghost" aria-pressed={shown === value} disabled={value === 'hour' && !hourlyAllowed} onClick={() => setPeriod(value)}>{label}</Button>)}</div>
      {view && view.tools.length > 0 && <ShareButton />}
    </div>
    {days === 'custom' && <div className="local-date-range" role="group" aria-label="Custom dates">
      <label>From<input type="date" value={custom.start} max={custom.end || undefined} onChange={e => setCustom({ ...custom, start: e.target.value })} /></label>
      <label>To<input type="date" value={custom.end} min={custom.start || undefined} onChange={e => setCustom({ ...custom, end: e.target.value })} /></label>
      {!range && <p className="operation-error" role="alert">Choose a start and an end at most 366 days apart.</p>}
    </div>}
    {failed && <p className="operation-error" role="alert">OpenUsage could not read saved local usage. Try again.</p>}
    {!view ? !failed && <p className="overview-dim" role="status">Loading…</p> : view.tools.length === 0 ? <div className="empty-state"><h3>No saved local usage for these dates</h3></div> : <>
      <div className="local-tool-grid">{view.tools.map(totals => <ToolCard key={totals.tool} totals={totals} view={view} />)}</div>
      <Records view={view} />
      <div className="overview-days"><LocalBars view={view} period={shown} /></div>
      <Models view={view} />
      <Projects view={view} />
      <Sessions view={view} />
    </>}
  </section>;
}

/** The last 30 days on Overview, so first launch reaches local history with no sign-in (OU-UI-025). */
export function LocalOverview({ active, onOpen }: { active: boolean; onOpen: () => void }) {
  const { view } = useLocalUsage(active, { days: 30 }, 'day');
  if (!view || view.tools.length === 0) return null;
  return <section className="local-overview" aria-labelledby="local-overview-heading" data-local-overview>
    <div className="overview-section-heading"><h2 id="local-overview-heading">This device</h2><span className="local-overview-range">Last 30 days</span><ShareButton /><Button variant="outline" onClick={onOpen} data-local-overview-open>Open in Usage</Button></div>
    <div className="local-tool-grid">{view.tools.map(totals => <ToolCard key={totals.tool} totals={totals} view={view} />)}</div>
    <div className="overview-days"><LocalBars view={view} period="day" /></div>
  </section>;
}
