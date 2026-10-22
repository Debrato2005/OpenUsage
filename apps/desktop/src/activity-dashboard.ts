import { useEffect, useState } from 'react';
import { invoke, type ActivityDashboard, type ActivityView, type HeatmapMode, type ProfileRecord } from './contracts';

function priorDate(end: string, days: number) {
  return new Date(Date.parse(`${end}T00:00:00Z`) - (days - 1) * 86400000).toISOString().slice(0,10);
}

export function useActivityDashboard(profiles: ProfileRecord[], profileId: string | null, revision: number, active: boolean) {
  const [view, setView] = useState<ActivityView>('individual');
  const [mode, setMode] = useState<HeatmapMode>('daily');
  const [selection, setSelection] = useState<string[] | null>(null);
  const [range, setRange] = useState(() => { const end = new Date().toISOString().slice(0,10); return { start: priorDate(end,90), end }; });
  const [attempt, setAttempt] = useState(0);
  const [result, setResult] = useState<{ key: string; data: ActivityDashboard | null; failed: boolean } | null>(null);
  const periodDays = Math.round((Date.parse(range.end) - Date.parse(range.start)) / 86400000) + 1;
  const validRange = /^[0-9]{4}-[0-9]{2}-[0-9]{2}$/.test(range.start) && /^[0-9]{4}-[0-9]{2}-[0-9]{2}$/.test(range.end) && periodDays >= 1 && periodDays <= 365;
  const ids = view === 'individual' ? profileId ? [profileId] : [] : (selection ?? profiles.filter(p => p.connectionState === 'connected').map(p => p.id)).filter(id => profiles.some(p => p.id === id && !p.archived));
  const key = JSON.stringify([view, mode, ids, range, revision, attempt, profiles.map(p => [p.id,p.connectionState])]);
  useEffect(() => {
    if (!active || !ids.length || !validRange) return;
    let cancelled = false;
    void invoke('load_activity_dashboard', { profileIds: ids, view, mode, startDate: range.start, endDate: range.end }).then(data => {
      const returned = [...data.members, ...data.exclusions].map(m => m.profileId);
      if (data.view !== view || data.mode !== mode || data.periodStart !== range.start || data.periodEnd !== range.end
        || returned.length !== ids.length || returned.some(id => !ids.includes(id))) throw new Error('Mismatched activity selection');
      if (!cancelled) setResult({ key, data, failed: false });
    }).catch(() => { if (!cancelled) setResult({ key, data: null, failed: true }); });
    return () => { cancelled = true; };
  }, [key, active]);
  const current = result?.key === key ? result : null;
  const data = ids.length ? current?.data ?? null : null;
  return { view, setView, mode, setMode, ids, setSelection, range, setRange, periodDays, validRange, data,
    loading: active && validRange && ids.length > 0 && current === null, failed: current?.failed ?? false,
    retry: () => setAttempt(n => n + 1),
    setPeriodDays: (days: number) => { if (Number.isFinite(Date.parse(range.end))) setRange({ start: priorDate(range.end, days), end: range.end }); },
  };
}
export type ActivityDashboardState = ReturnType<typeof useActivityDashboard>;
