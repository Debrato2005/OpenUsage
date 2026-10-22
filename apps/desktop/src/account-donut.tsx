import { useState } from 'react';
import { Pie, PieChart } from 'recharts';
import { Button } from './components/ui/button';
import type { ProfileRecord, SavedQuotaWindow } from './contracts';

// Quota percentages are already validated and derived in Rust. Token shares require a separate verified aggregate.
export function AccountDonut({ profiles, label, window }: { profiles: ProfileRecord[]; label: string; window: SavedQuotaWindow | null }) {
  const [mode, setMode] = useState<'quota' | 'share'>('quota');
  const connected = profiles.filter(profile => profile.connectionState === 'connected');
  return <section className="overview-panel overview-share-panel" aria-labelledby="overview-share-heading" data-overview-account-share data-aggregation="unavailable">
    <div className="overview-panel-heading"><h2 id="overview-share-heading">{mode === 'quota' ? 'Quota remaining' : 'Account usage share'}</h2></div>
    <div className="overview-chart-modes" role="group" aria-label="Donut view"><Button variant="ghost" aria-pressed={mode === 'quota'} onClick={() => setMode('quota')}>Selected account</Button><Button variant="ghost" aria-pressed={mode === 'share'} onClick={() => setMode('share')}>Account share</Button></div>
    {mode === 'share' ? <div className="overview-share-unavailable"><h3>Unavailable</h3><p>OpenAI’s current connection does not provide the identity checks needed for combined totals.</p></div> : <figure className="overview-donut" aria-label={mode === 'quota' && window ? `${label}, ${window.label}: ${window.derivedRemainingPercent}% remaining, ${window.usedPercent}% used` : 'Account share unavailable. Verified identities and compatible activity coverage are required.'}>
      {mode === 'quota' && window ? <div className="overview-donut-plot" aria-hidden="true" data-overview-quota-donut data-remaining={window.derivedRemainingPercent}>
        <PieChart width={230} height={230} accessibilityLayer={false}>
          <Pie data={[{ name: 'Remaining', value: window.derivedRemainingPercent, fill: 'var(--chart, var(--accent))' }, { name: 'Used', value: window.usedPercent, fill: 'var(--line)' }]} dataKey="value" nameKey="name" innerRadius={77} outerRadius={109} startAngle={90} endAngle={-270} stroke="none" rootTabIndex={-1} isAnimationActive={false} />
        </PieChart>
        <div className="overview-donut-center"><strong>{window.derivedRemainingPercent}%</strong><span>remaining</span></div>
      </div> : <div className="overview-donut-plot overview-donut-unavailable" aria-hidden="true"><div className="overview-donut-center"><strong>Unavailable</strong><span>{mode === 'quota' ? 'no saved quota' : 'account share'}</span></div></div>}
      <figcaption>{mode === 'quota' ? <><bdi>{label}</bdi><span>{window?.label ?? 'No quota window supplied'}</span></> : 'Compatible account activity is not available yet.'}</figcaption>
    </figure>}
    {mode === 'quota' && window ? <dl className="overview-donut-values"><div><dt><i />Remaining</dt><dd>{window.derivedRemainingPercent}%</dd></div><div><dt><i />Used</dt><dd>{window.usedPercent}%</dd></div></dl>
      : mode === 'share' && <ul className="overview-share-legend" aria-label="Account share availability">{connected.map(profile => <li key={profile.id}><bdi>{profile.label}</bdi><span aria-label="Share unavailable">—</span></li>)}</ul>}

  </section>;
}
