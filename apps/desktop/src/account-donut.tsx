import { useState } from 'react';
import { Pie, PieChart } from 'recharts';
import { Button } from './components/ui/button';
import type { SavedQuotaWindow } from './contracts';
import type { ActivityDashboardState } from './activity-dashboard';

const compact = new Intl.NumberFormat('en-US', { notation:'compact',maximumFractionDigits:2 });
const exact = new Intl.NumberFormat('en-US');
export function AccountDonut({ label, window, dashboard: d }: { label: string; window: SavedQuotaWindow | null; dashboard: ActivityDashboardState }) {
  const [localMode,setMode] = useState<'quota'|'share'>('quota');
  const mode = d.view === 'individual' ? localMode : 'share';
  const data = d.data;
  const shares = data?.members.filter(m => m.shareBasisPoints !== null) ?? [];
  const hasShares = shares.some(m => m.shareBasisPoints! > 0);
  const colors = ['#0969da','#218bff','#54aeff','#80ccff','#0550ae','#579cff','#2d6bc8','#6db8f2','#134d91','#a5d6ff'];
  const plot = mode === 'quota' && window ? [{ name:'Remaining',value:window.derivedRemainingPercent,fill:'var(--accent)' },{ name:'Used',value:window.usedPercent,fill:'var(--line)' }]
    : shares.map((m,index) => ({ name:m.label,value:m.shareBasisPoints!,fill:colors[index] }));
  return <section className="overview-panel overview-share-panel" aria-labelledby="overview-share-heading" data-overview-account-share data-aggregation={data?.status ?? 'unavailable'}>
    <div className="overview-panel-heading"><h2 id="overview-share-heading">{mode === 'quota' ? 'Quota remaining' : 'Reported activity share'}</h2></div>
    <div className="overview-chart-modes" role="group" aria-label="Donut view"><Button variant="ghost" aria-pressed={mode === 'quota'} onClick={() => { d.setView('individual');setMode('quota'); }}>Selected account</Button><Button variant="ghost" aria-pressed={mode === 'share'} onClick={() => setMode('share')}>Account share</Button></div>
    {mode === 'quota' && window || mode === 'share' && hasShares ? <figure className="overview-donut" aria-label={mode === 'quota' && window ? `${label}, ${window.label}: ${window.derivedRemainingPercent}% remaining, ${window.usedPercent}% used` : `${d.view === 'profile_sum' ? 'Unverified selected-profile' : d.view === 'verified' ? 'Verified combined' : 'Individual'} share of observed tokens, ${data?.periodStart} to ${data?.periodEnd}`}>
      <div className="overview-donut-plot" aria-hidden="true" data-overview-quota-donut={mode === 'quota' || undefined} data-activity-share-chart={mode === 'share' || undefined} data-remaining={mode === 'quota' ? window?.derivedRemainingPercent : undefined}>
        <PieChart width={230} height={230} accessibilityLayer={false}><Pie data={plot} dataKey="value" nameKey="name" innerRadius={77} outerRadius={109} startAngle={90} endAngle={-270} stroke="none" rootTabIndex={-1} isAnimationActive={false} /></PieChart>
        <div className="overview-donut-center"><strong title={mode === 'share' && data?.totalTokens ? exact.format(BigInt(data.totalTokens)) : undefined}>{mode === 'quota' ? `${window?.derivedRemainingPercent}%` : data?.totalTokens === null || !data ? 'Unavailable' : compact.format(BigInt(data.totalTokens))}</strong><span>{mode === 'quota' ? 'remaining' : 'observed tokens'}</span></div>
      </div>
      <figcaption>{mode === 'quota' ? <><bdi>{label}</bdi><span>{window?.label}</span></> : <><span>{data?.periodStart} – {data?.periodEnd}</span><span>{d.view === 'profile_sum' ? 'Selected profiles · Unverified' : d.view === 'verified' ? 'Verified distinct accounts' : 'Individual account'}</span></>}</figcaption>
    </figure> : <div className="overview-share-unavailable"><h3>{d.loading && mode === 'share' ? 'Loading…' : 'Unavailable'}</h3><p>{mode === 'quota' ? 'No saved quota' : data?.totalTokens === '0' ? 'No shares for a zero-token total.' : 'No eligible reported activity in this selection.'}</p></div>}
    {mode === 'quota' && window ? <dl className="overview-donut-values"><div><dt><i />Remaining</dt><dd>{window.derivedRemainingPercent}%</dd></div><div><dt><i />Used</dt><dd>{window.usedPercent}%</dd></div></dl> : mode === 'share' && <>
      <ul className="overview-share-legend" aria-label="Activity contribution breakdown">{data?.members.map((m,index) => <li key={m.profileId} data-share-profile={m.profileId}><i style={{ background:colors[index] }} aria-hidden="true"/><bdi>{m.label}</bdi><span title={m.tokens === null ? 'No reported values' : `${exact.format(BigInt(m.tokens))} observed tokens`}>{m.shareBasisPoints === null ? '—' : `${m.shareBasisPoints / 100}%`}</span></li>)}</ul>
      {data && <details className="combined-explanation"><summary>Values and coverage</summary><p>{data.knownContributions} of {data.expectedContributions} profile-days reported. Shares use observed subtotals for the same selected dates. Missing values remain unknown.</p><table className="activity-share-values"><thead><tr><th>Account</th><th>Observed tokens</th><th>Days reported</th></tr></thead><tbody>{data.members.map(m => <tr key={m.profileId}><th><bdi>{m.label}</bdi></th><td>{m.tokens === null ? 'Unavailable' : exact.format(BigInt(m.tokens))}</td><td>{m.knownDays}</td></tr>)}</tbody></table><ul>{data.members.map(m => <li key={m.profileId}><bdi>{m.label}</bdi>: {m.sourceKind === 'test_fixture' ? 'Synthetic test fixture' : 'Official OpenAI provider'}. {data.scopeVerified ? m.scope : 'Application scope and day definition unverified'}. Last read: <time dateTime={m.observedAtUtc}>{m.observedAtUtc}</time>.</li>)}</ul></details>}
    </>}
  </section>;
}
