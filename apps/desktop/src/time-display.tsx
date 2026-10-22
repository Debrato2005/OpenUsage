import { createContext, useContext, useEffect, useMemo, useState, type ReactNode, type ComponentProps } from 'react';
import { invoke } from './contracts';
import { Button } from './components/ui/button';
import { Select, SelectItem, SelectPopup, SelectTrigger, SelectValue } from './components/ui/select';

const TimeContext = createContext({ zone: 'UTC', pending: true, failed: false, save: async (_zone: string) => {} });
export function TimeDisplay({ ready, revision, children }: { ready: boolean; revision: number; children: ReactNode }) {
  const [zone, setZone] = useState('UTC');
  const [pending, setPending] = useState(true);
  const [failed, setFailed] = useState(false);
  useEffect(() => {
    let current = true;
    if (!ready) { setZone('UTC'); setPending(true); return; }
    setPending(true);
    void invoke('load_time_zone').then(value => { if (current) { setZone(value.timeZone); setFailed(false); } })
      .catch(() => { if (current) setFailed(true); }).finally(() => { if (current) setPending(false); });
    return () => { current = false; };
  }, [ready, revision]);
  async function save(next: string) {
    setPending(true);
    try {
      const value = await invoke('set_time_zone', { timeZone: next });
      if (value.timeZone !== next) throw new Error('Time zone mismatch');
      setZone(value.timeZone); setFailed(false);
    } catch { setFailed(true); }
    finally { setPending(false); }
  }
  return <TimeContext.Provider value={{ zone, pending, failed, save }}>{children}</TimeContext.Provider>;
}
export function useDateFormat(options: Intl.DateTimeFormatOptions = {}) {
  const { zone } = useContext(TimeContext);
  const key = JSON.stringify(options);
  return useMemo(() => new Intl.DateTimeFormat('en-US', { year: 'numeric', month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit', timeZoneName: 'short', ...options, timeZone: zone }), [zone, key]);
}
export function DisplayTime({ value, ...props }: { value: string } & ComponentProps<'time'>) {
  const dates = useDateFormat();
  return <time {...props} dateTime={value}>{dates.format(new Date(value))}</time>;
}
export function LiveClock() {
  const dates = useDateFormat({ hourCycle: 'h23', second: '2-digit' });
  const [now, setNow] = useState(Date.now);
  useEffect(() => {
    let timer: number | undefined;
    const update = () => {
      window.clearTimeout(timer);
      const current = Date.now(); setNow(current);
      if (!document.hidden) timer = window.setTimeout(update, 1000 - current % 1000);
    };
    update();
    document.addEventListener('visibilitychange', update);
    window.addEventListener('focus', update);
    return () => { window.clearTimeout(timer); document.removeEventListener('visibilitychange', update); window.removeEventListener('focus', update); };
  }, []);
  return <time className="live-clock" data-live-clock dateTime={new Date(now).toISOString()}>{dates.format(new Date(now))}</time>;
}
const common: Record<string, string> = { UTC: 'UTC', 'Asia/Kolkata': 'India · Kolkata (UTC+05:30)', 'America/New_York': 'New York', 'Europe/London': 'London', 'Asia/Tokyo': 'Tokyo', 'Australia/Sydney': 'Sydney' };
export function TimeSettings() {
  const { zone, pending, failed, save } = useContext(TimeContext);
  const systemZone = Intl.DateTimeFormat().resolvedOptions().timeZone;
  const zones = useMemo(() => {
    const supported = (Intl as typeof Intl & { supportedValuesOf?: (key: string) => string[] }).supportedValuesOf?.('timeZone') ?? [];
    return { ...common, ...Object.fromEntries([...new Set([zone, systemZone, ...supported])].filter(value => !common[value]).map(value => [value, value.replaceAll('_', ' ')])) };
  }, [zone, systemZone]);
  return <section className="time-settings" aria-labelledby="time-settings-heading">
    <div><h2 id="time-settings-heading">Date and time</h2><p>Choose how clock times and reset dates appear. Activity dates stay as reported by OpenAI.</p></div>
    <div className="time-settings-controls"><label id="time-zone-label" htmlFor="time-zone">Time zone</label>
      <Select modal={false} value={zone} items={zones} disabled={pending} onValueChange={value => { if (value) void save(value); }}>
        <SelectTrigger id="time-zone" aria-labelledby="time-zone-label"><SelectValue /></SelectTrigger>
        <SelectPopup alignItemWithTrigger={false}>{Object.entries(zones).map(([value, label]) => <SelectItem key={value} value={value}>{label}</SelectItem>)}</SelectPopup>
      </Select><Button variant="outline" disabled={pending} onClick={() => { void save(systemZone); }}>Use system time</Button>
    </div>
    {failed && <p role="alert">Time zone could not be loaded or saved. Choose a time zone to try again.</p>}
  </section>;
}
