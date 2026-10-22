import { useDateFormat } from './time-display';
import { cloneElement, useEffect, useId, useRef, useState, type KeyboardEvent, type ReactElement, type SVGProps } from 'react';
import { ActivityCalendar } from 'react-activity-calendar';
import { Button } from './components/ui/button';
import { Collapsible, CollapsiblePanel, CollapsibleTrigger } from './components/ui/collapsible';
import { Select, SelectItem, SelectPopup, SelectTrigger, SelectValue } from './components/ui/select';
import { invoke, type SavedProfileHeatmap } from './contracts';
import './token-heatmap.css';

type Props = { profileId: string | null; profileLabel: string; revision: number; compact?: boolean };
type Mode = 'daily' | 'weekly' | 'cumulative';
type Period = 30 | 90 | 180 | 365;
type Cell = SavedProfileHeatmap['cells'][number];
const modes: Record<Mode, string> = { daily: 'Daily', weekly: 'Weekly', cumulative: 'Cumulative' };
const freshness = { fresh: 'Fresh', stale: 'Stale', unknown: 'Freshness unknown' };

function description(cell: Cell, mode: Mode): string {
  const period = cell.startDate === cell.endDate ? cell.startDate : `${cell.startDate} to ${cell.endDate}`;
  if (cell.tokens === null) return `${period}: no saved reading`;
  return `${period}: ${cell.tokens} ${mode === 'daily' ? 'tokens' : mode === 'weekly' ? 'observed token subtotal' : 'cumulative observed tokens'}; ${cell.knownDays} of ${cell.expectedDays} days known${cell.knownDays < cell.expectedDays ? '; Incomplete' : ''}; ${freshness[cell.freshness]}`;
}

export function TokenHeatmap({ profileId, profileLabel, revision, compact = false }: Props) {
  const observedDates = useDateFormat();
  const calendar = useRef<HTMLDivElement>(null);
  const [calendarWidth, setCalendarWidth] = useState(300);
  useEffect(() => {
    const node = calendar.current;
    if (!node) return;
    const observer = new ResizeObserver(entries => setCalendarWidth(entries[0]!.contentRect.width));
    observer.observe(node);
    return () => observer.disconnect();
  }, []);
  const headingId = useId();
  const detailId = useId();
  const [mode, setMode] = useState<Mode>('daily');
  const [periodDays, setPeriodDays] = useState<Period>(compact ? 90 : 365);
  const [retry, setRetry] = useState(0);
  const [result, setResult] = useState<{ key: string; value: SavedProfileHeatmap | null; failed: boolean } | null>(null);
  const [focused, setFocused] = useState(0);
  const [preview, setPreview] = useState(0);
  const buttons = useRef<(SVGRectElement | null)[]>([]);
  const requestKey = `${profileId}:${mode}:${periodDays}:${revision}:${retry}`;
  useEffect(() => {
    let cancelled = false;
    setFocused(0);
    setPreview(0);
    if (!profileId) return;
    void invoke('load_profile_heatmap', { profileId, mode, periodDays }).then(value => {
      if (cancelled) return;
      if (value && (value.profileId !== profileId || value.mode !== mode)) throw new Error('Mismatched saved activity');
      const lastKnown = value?.cells.reduce((last, cell, index) => cell.tokens !== null ? index : last, 0) ?? 0;
      setFocused(lastKnown);
      setPreview(lastKnown);
      setResult({ key: requestKey, value, failed: false });
    }).catch(() => {
      if (!cancelled) setResult({ key: requestKey, value: null, failed: true });
    });
    return () => { cancelled = true; };
  }, [profileId, mode, periodDays, revision, retry, requestKey]);
  const current = result?.key === requestKey ? result : null;
  const value = current?.value ?? null;
  const cells = value?.cells ?? [];
  const loading = profileId !== null && current === null;
  const selected = cells[preview] ?? cells[0];
  const weekly = mode === 'weekly';
  const byDate = new Map(cells.map((cell, index) => [cell.startDate, { cell, index }]));
  // The library count is unused: hide both footers and supply all exact text from Rust.
  const calendarData = cells.map(cell => ({ date: cell.startDate, count: 0, level: cell.level }));
  const firstWeekday = cells.length ? (new Date(`${cells[0]!.startDate}T00:00:00Z`).getUTCDay() + 6) % 7 : 0;
  const calendarDays = cells.length ? Math.round((Date.parse(cells.at(-1)!.startDate) - Date.parse(cells[0]!.startDate)) / 86400000) + 1 : 1;
  if (!weekly && cells.length) {
    const first = Date.parse(cells[0]!.startDate);
    const last = Date.parse(cells.at(-1)!.startDate);
    const padding = (offset: number, anchor: number) => ({ date: new Date(anchor + offset * 86400000).toISOString().slice(0, 10), count: 0, level: 0 });
    // Layout-only dates complete Monday–Sunday columns. They never enter saved values.
    calendarData.unshift(...Array.from({ length: firstWeekday }, (_, index) => padding(index - firstWeekday, first)));
    calendarData.push(...Array.from({ length: (7 - (calendarDays + firstWeekday) % 7) % 7 }, (_, index) => padding(index + 1, last)));
  }
  const columns = Math.ceil((calendarDays + firstWeekday) / 7);
  const blockSize = Math.max(11, Math.min(20, Math.floor((calendarWidth - 36 - (columns - 1) * 3) / columns)));
  const knownDays = mode === 'daily' ? cells.filter(cell => cell.tokens !== null).length : mode === 'weekly' ? cells.reduce((sum, cell) => sum + cell.knownDays, 0) : cells.at(-1)?.knownDays ?? 0;
  const calendarColors = ['var(--heatmap-zero)', ...[1, 2, 3, 4].map(level => `var(--heatmap-level-${level})`)];
  function move(event: KeyboardEvent<SVGRectElement>, index: number) {
    if (event.key === 'Enter' || event.key === ' ') { event.preventDefault(); setFocused(index); setPreview(index); return; }
    const step = weekly ? 1 : 7;
    const next = event.key === 'ArrowRight' ? index + step : event.key === 'ArrowLeft' ? index - step
      : event.key === 'ArrowDown' ? index + 1 : event.key === 'ArrowUp' ? index - 1
      : event.key === 'Home' ? 0 : event.key === 'End' ? cells.length - 1 : null;
    if (next === null) return;
    event.preventDefault();
    const target = Math.min(cells.length - 1, Math.max(0, next));
    setFocused(target);
    setPreview(target);
    buttons.current[target]?.focus();
  }
  return <section ref={calendar} className="token-heatmap" data-token-heatmap data-profile-id={profileId ?? ''} data-mode={mode} data-period-days={periodDays} data-compact={compact || undefined} aria-labelledby={headingId} aria-busy={loading}>
    <header className="token-heatmap-header">
      <div><h2 id={headingId}>Token activity</h2><p><bdi>{profileLabel || 'No account selected'}</bdi>{value && <span> · {cells.some(cell => cell.knownDays < cell.expectedDays) ? 'Partial history' : 'Saved dates'}</span>}</p></div>
      <div className="token-heatmap-controls">
        <div className="token-heatmap-modes" role="group" aria-label="Activity view">{(Object.keys(modes) as Mode[]).map(item => <Button key={item} variant="ghost" type="button" data-heatmap-mode={item} aria-pressed={mode === item} onClick={() => setMode(item)}>{modes[item]}</Button>)}</div>
        <Select modal={false} value={String(periodDays)} items={{ '30': '30 days', '90': '90 days', '180': '180 days', '365': '365 days' }} onValueChange={next => { if (next === '30' || next === '90' || next === '180' || next === '365') setPeriodDays(Number(next) as Period); }}>
          <SelectTrigger aria-label="Activity history range"><SelectValue /></SelectTrigger>
          <SelectPopup alignItemWithTrigger={false}>{([30, 90, 180, 365] as const).map(days => <SelectItem key={days} value={String(days)}>{days} days</SelectItem>)}</SelectPopup>
        </Select>
      </div>
    </header>
    {!profileId ? <p className="token-heatmap-empty">Connect an account to view activity.</p>
      : loading ? <p className="token-heatmap-empty" role="status">Loading saved activity…</p>
      : current?.failed ? <div className="token-heatmap-empty" role="alert"><p>Saved activity could not be loaded.</p><Button variant="outline" onClick={() => setRetry(value => value + 1)}>Retry</Button></div>
      : !value || cells.length === 0 ? <p className="token-heatmap-empty">No saved daily activity yet.</p>
      : <>
        {value.sourceKind === 'test_fixture' && <p className="token-heatmap-fixture">Synthetic test data</p>}
        <p className="token-heatmap-range" data-heatmap-coverage><time dateTime={value.periodStart}>{value.periodStart}</time> – <time dateTime={value.periodEnd}>{value.periodEnd}</time><span>{knownDays} of {periodDays} days reported</span></p>
        <div className="token-heatmap-calendar-scroll" data-heatmap-grid role="group" aria-label={`${modes[mode]} token activity. Use arrow keys to explore dates.`}>
          <ActivityCalendar data={calendarData} weekStart={1} blockSize={blockSize} blockMargin={3} blockRadius={3} fontSize={13}
            showTotalCount={false} showColorLegend={false} showWeekdayLabels={weekly ? false : ['mon', 'tue', 'wed', 'thu', 'fri', 'sat', 'sun']}
            theme={{ light: calendarColors, dark: calendarColors }}
            renderBlock={(block, activity) => {
              const saved = byDate.get(activity.date);
              // Outside-period cells complete the grid, but are not readings or zeroes.
              if (!saved) return weekly ? <g aria-hidden="true" /> : cloneElement(block as ReactElement<SVGProps<SVGRectElement>>, { ...{ 'data-heatmap-padding': true }, 'aria-hidden': true, style: { fill: 'var(--heatmap-unknown-surface)', stroke: 'none', pointerEvents: 'none' } });
              const { cell, index } = saved;
              const attributes = {
                'data-heatmap-cell': cell.startDate, 'data-start-date': cell.startDate, 'data-end-date': cell.endDate,
                'data-level': cell.level, 'data-value': cell.tokens === null ? 'missing' : cell.tokens === '0' ? 'zero' : 'positive',
                'data-exact-value': cell.tokens ?? '',
              };
              return cloneElement(block as ReactElement<SVGProps<SVGRectElement>>, {
                ...attributes, ref: element => { buttons.current[index] = element; }, role: 'button',
                className: 'token-heatmap-cell', tabIndex: focused === index ? 0 : -1,
                'aria-label': description(cell, mode), 'aria-describedby': detailId,
                children: <title>{description(cell, mode)}</title>,
                onFocus: () => { setFocused(index); setPreview(index); }, onPointerEnter: () => setPreview(index),
                onClick: () => { setFocused(index); setPreview(index); }, onKeyDown: event => move(event, index),
                style: { ...block.props.style, stroke: 'none', strokeWidth: 0,
                  fill: cell.tokens === null ? 'var(--heatmap-unknown-surface)' : calendarColors[cell.level] },
              });
            }} />
        </div>
        <div className="token-heatmap-legend" aria-label="Heatmap legend"><span><i data-value="missing" />Missing</span><span><i data-value="zero" />Zero</span><span className="token-heatmap-scale">Less{[1, 2, 3, 4].map(level => <i key={level} data-level={level} />)}More</span></div>
        {selected && <div className="token-heatmap-selected" id={detailId} data-heatmap-selected>
          <span><time dateTime={selected.startDate}>{selected.startDate}</time>{selected.startDate !== selected.endDate && <> – <time dateTime={selected.endDate}>{selected.endDate}</time></>}</span>
          <strong>{selected.tokens === null ? 'No saved reading' : <><bdi>{selected.tokens}</bdi> <span>{mode === 'daily' ? 'tokens' : mode === 'weekly' ? 'observed tokens' : 'cumulative observed tokens'}</span></>}</strong>
          {selected.tokens !== null && <span>{selected.knownDays} of {selected.expectedDays} days known{selected.knownDays < selected.expectedDays && ' · Incomplete'} · {freshness[selected.freshness]}</span>}
        </div>}
        <Collapsible className="token-heatmap-details">
          <CollapsibleTrigger render={<Button variant="ghost" />} type="button" data-heatmap-values-toggle>Values</CollapsibleTrigger>
          <CollapsiblePanel>
            <p>{mode === 'cumulative' ? 'Cumulative values sum known days within this selected period. Missing days remain unknown.' : mode === 'weekly' ? 'Weekly subtotals include known days only. Missing days remain unknown.' : 'Each cell shows the saved provider-reported count for that date. Missing dates are not zero.'} Dates follow provider-reported day buckets; changing the display time zone does not shift or split them. Provider activity scope and coverage are unknown.</p>
            <p>Source: {value.sourceKind === 'official_artifact' ? 'Official OpenAI provider' : value.sourceKind === 'test_fixture' ? 'Synthetic test fixture' : 'Unverified'}. Last read: <time dateTime={value.observedAtUtc}>{observedDates.format(new Date(value.observedAtUtc))}</time>. Scale maximum: <bdi>{value.scaleMaximum ?? 'Not available'}</bdi>.</p>
            <div className="token-heatmap-values" data-heatmap-values role="region" aria-label="Exact activity values" tabIndex={0}>
              <table><thead><tr><th scope="col">Date</th><th scope="col">{mode === 'daily' ? 'Tokens' : mode === 'weekly' ? 'Observed subtotal' : 'Cumulative observed tokens'}</th><th scope="col">Known days</th><th scope="col">Freshness</th><th scope="col">Last read</th></tr></thead>
                <tbody>{cells.map(cell => <tr key={cell.startDate}><th scope="row"><time dateTime={cell.startDate}>{cell.startDate}</time>{cell.startDate !== cell.endDate && <> – <time dateTime={cell.endDate}>{cell.endDate}</time></>}</th><td><bdi>{cell.tokens ?? 'Not available'}</bdi></td><td>{cell.knownDays}/{cell.expectedDays}</td><td>{freshness[cell.freshness]}</td><td>{cell.observedAtUtc ? <time dateTime={cell.observedAtUtc}>{observedDates.format(new Date(cell.observedAtUtc))}</time> : 'Not available'}</td></tr>)}</tbody>
              </table>
            </div>
          </CollapsiblePanel>
        </Collapsible>
      </>}
  </section>;
}
