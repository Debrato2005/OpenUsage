import { useEffect, useState } from 'react';
import { invoke, type ActivityDashboard, type ActivityView } from './contracts';

const dayMs = 86_400_000;
export const shiftDate = (date: string, days: number) => new Date(Date.parse(`${date}T00:00:00Z`) + days * dayMs).toISOString().slice(0, 10);
const daysBetween = (start: string, end: string) => Math.round((Date.parse(end) - Date.parse(start)) / dayMs) + 1;

// The same saved activity for the dates just before the chosen period, so the change can be shown.
export function usePreviousPeriod(view: ActivityView, ids: string[], start: string, end: string, revision: number, active: boolean) {
  const days = daysBetween(start, end);
  const previousEnd = shiftDate(start, -1), previousStart = shiftDate(start, -days);
  const valid = days >= 1 && days <= 365 && ids.length > 0;
  const key = JSON.stringify([view, ids, previousStart, previousEnd, revision]);
  const [result, setResult] = useState<{ key: string; data: ActivityDashboard | null } | null>(null);
  useEffect(() => {
    if (!active || !valid) return;
    let cancelled = false;
    void invoke('load_activity_dashboard', { profileIds: ids, view, mode: 'daily', startDate: previousStart, endDate: previousEnd })
      .then(data => { if (!cancelled) setResult({ key, data }); })
      .catch(() => { if (!cancelled) setResult({ key, data: null }); });
    return () => { cancelled = true; };
  }, [key, active, valid]);
  return valid && result?.key === key ? result.data : null;
}

// Each account's own saved daily tokens for the chosen dates. Missing days stay missing.
export function useAccountSeries(ids: string[], start: string, end: string, revision: number, active: boolean) {
  const valid = daysBetween(start, end) >= 1 && daysBetween(start, end) <= 365;
  const key = JSON.stringify([ids, start, end, revision]);
  const [result, setResult] = useState<{ key: string; series: Record<string, ActivityDashboard | null> } | null>(null);
  useEffect(() => {
    if (!active || !valid || !ids.length) return;
    let cancelled = false;
    void Promise.allSettled(ids.map(profileId => invoke('load_activity_dashboard', { profileIds: [profileId], view: 'individual', mode: 'daily', startDate: start, endDate: end })))
      .then(results => {
        if (cancelled) return;
        setResult({ key, series: Object.fromEntries(ids.map((id, index) => {
          const value = results[index]!;
          return [id, value.status === 'fulfilled' && value.value.status === 'available' ? value.value : null];
        })) });
      });
    return () => { cancelled = true; };
  }, [key, active, valid]);
  return result?.key === key ? result.series : null;
}
