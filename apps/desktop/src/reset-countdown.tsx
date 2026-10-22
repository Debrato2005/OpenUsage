import { useEffect, useState } from 'react';

// Display time only. A local clock never changes saved quotas or confirms a reset.
export function ResetCountdown({ resetsAtUnix }: { resetsAtUnix: number | null }) {
  const deadline = resetsAtUnix === null ? NaN : resetsAtUnix * 1000;
  const valid = Number.isSafeInteger(resetsAtUnix) && Number.isFinite(new Date(deadline).getTime());
  const [now, setNow] = useState(Date.now);
  useEffect(() => {
    if (!valid) return;
    let timer: number | undefined;
    const update = () => {
      window.clearTimeout(timer);
      const current = Date.now();
      setNow(current);
      if (!document.hidden) {
        const remaining = deadline - current;
        // Wake at the next displayed minute, including the expected reset itself.
        timer = window.setTimeout(update, remaining > 0 ? (remaining % 60_000 || 60_000) : 60_000);
      }
    };
    update();
    document.addEventListener('visibilitychange', update);
    window.addEventListener('focus', update);
    return () => {
      window.clearTimeout(timer);
      document.removeEventListener('visibilitychange', update);
      window.removeEventListener('focus', update);
    };
  }, [deadline, valid]);
  if (!valid) return null;
  const minutes = Math.ceil((deadline - now) / 60_000);
  const days = Math.floor(minutes / 1440);
  const hours = Math.floor(minutes % 1440 / 60);
  const remainder = minutes % 60;
  const duration = [days ? `${days}d` : '', hours ? `${hours}h` : '', remainder ? `${remainder}m` : ''].filter(Boolean).join(' ');
  return <span className="quota-reset-countdown" data-reset-countdown={minutes > 0 ? 'expected' : 'due'}>
    {minutes > 0 ? `In ${duration}` : 'Awaiting reset confirmation'}
  </span>;
}
