import { Pie, PieChart } from 'recharts';
import type { SavedQuotaWindow } from './contracts';
import { DetailLink } from './detail-panel';


// One account's quota window on Usage.
export function AccountDonut({ label, window }: { mode: 'quota'; label: string; window: SavedQuotaWindow | null }) {
  const plot = window ? [{ name:'Remaining',value:window.derivedRemainingPercent,fill:'var(--accent)' },{ name:'Used',value:window.usedPercent,fill:'var(--line)' }] : [];
  return <section className="overview-panel overview-share-panel" aria-labelledby="overview-share-heading" data-overview-account-share>
    <div className="overview-panel-heading"><h2 id="overview-share-heading">Quota remaining</h2></div>
    {window ? <figure className="overview-donut" aria-label={`${label}, ${window.label}: ${window.derivedRemainingPercent}% remaining, ${window.usedPercent}% used`}>
      <div className="overview-donut-plot" aria-hidden="true" data-overview-quota-donut data-remaining={window.derivedRemainingPercent}>
        <PieChart width={190} height={190} accessibilityLayer={false}><Pie data={plot} dataKey="value" nameKey="name" innerRadius={68} outerRadius={90} startAngle={90} endAngle={-270} stroke="none" rootTabIndex={-1} isAnimationActive={false} /></PieChart>
        <div className="overview-donut-center"><strong>{window.derivedRemainingPercent}%</strong><span>remaining</span></div>
      </div>
      <figcaption><bdi>{label}</bdi><span>{window.label}</span></figcaption>
    </figure> : <div className="overview-share-unavailable"><h3>No saved quota</h3></div>}
    {window && <dl className="overview-donut-values"><div><dt><i />Remaining</dt><dd><DetailLink label={`${label}, ${window.label} details`} details={<dl className="detail-rows"><dt>Window</dt><dd>{window.label}</dd><dt>Calculation</dt><dd>100% − {window.usedPercent}% used</dd></dl>}>{window.derivedRemainingPercent}%</DetailLink></dd></div><div><dt><i />Used</dt><dd>{window.usedPercent}%</dd></div></dl>}
  </section>;
}
