import { useCallback, useEffect, useRef, useState } from 'react';
import { Button } from './components/ui/button';
import { invoke, type MonitoringStatus, type ProfileMonitoringStatus, type ProviderExecutableApproval } from './contracts';
import { providerFailure } from './provider-errors';

export type MonitoringAction = (profileId: string, action: 'start' | 'pause', approvalId?: string) => Promise<void>;

// One read-only poll for the unlocked workspace. Provider work remains in Rust.
export function useMonitoring(ready: boolean, workspaceRevision: number, onReadingsChanged: () => void) {
  const [status, setStatus] = useState<MonitoringStatus | null>(null);
  const [unavailable, setUnavailable] = useState(false);
  const epoch = useRef(0);
  const sequence = useRef(0);
  const actions = useRef(0);
  const readingRevision = useRef<string | null>(null);
  const changed = useRef(onReadingsChanged);
  changed.current = onReadingsChanged;
  const accept = useCallback((value: MonitoringStatus) => {
    const previous = readingRevision.current;
    readingRevision.current = value.readingRevision;
    setStatus(value); setUnavailable(false);
    if (previous && previous !== value.readingRevision) changed.current();
  }, []);
  useEffect(() => {
    const currentEpoch = ++epoch.current;
    let timer: number | undefined;
    setStatus(null); setUnavailable(false); readingRevision.current = null;
    if (!ready) return;
    async function poll() {
      if (epoch.current !== currentEpoch) return;
      if (actions.current === 0) {
        const request = ++sequence.current;
        try {
          const value = await invoke('load_monitoring');
          if (epoch.current === currentEpoch && sequence.current === request) accept(value);
        } catch {
          if (epoch.current === currentEpoch && sequence.current === request) setUnavailable(true);
        }
      }
      if (epoch.current === currentEpoch) timer = window.setTimeout(() => { void poll(); }, 2000);
    }
    // Deferring the initial call also avoids duplicate development StrictMode probes.
    timer = window.setTimeout(() => { void poll(); }, 0);
    return () => { epoch.current += 1; window.clearTimeout(timer); };
  }, [ready, workspaceRevision, accept]);
  const action = useCallback<MonitoringAction>(async (profileId, operation, approvalId) => {
    if (!ready) throw 'workspace_locked';
    const currentEpoch = epoch.current, request = ++sequence.current;
    actions.current += 1;
    try {
      if (operation === 'start' && !approvalId) throw 'monitoring_not_eligible';
      const value = operation === 'start'
        ? await invoke('start_profile_monitoring', { profileId, approvalId: approvalId! })
        : await invoke('pause_profile_monitoring', { profileId });
      if (epoch.current === currentEpoch && request === sequence.current) accept(value);
    } finally { actions.current -= 1; }
  }, [ready, accept]);
  return { status, unavailable, action };
}

export function ProfileMonitoringControls({ profileId, status, unavailable, approval, busy, onAction }: {
  profileId: string; status: ProfileMonitoringStatus | null; unavailable: boolean;
  approval: ProviderExecutableApproval | null; busy: boolean; onAction: MonitoringAction;
}) {
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const alive = useRef(true);
  const generation = useRef(0);
  useEffect(() => { alive.current = true; return () => { alive.current = false; generation.current += 1; }; }, [profileId, approval?.approvalId]);
  async function change(action: 'start' | 'pause') {
    if (pending || (action === 'start' && (unavailable || busy || !status?.eligible || !approval || status.readingActive))) return;
    const operation = ++generation.current; setPending(true); setError(null);
    try { await onAction(profileId, action, approval?.approvalId); }
    catch (failure) { if (alive.current && operation === generation.current) setError(providerFailure(failure)); }
    finally { if (alive.current && operation === generation.current) setPending(false); }
  }
  const guidance = unavailable ? 'Monitoring status is unavailable.'
    : !status ? 'Loading monitoring status…'
    : status.issue === 'account_changed' ? 'Review the current sign-in before starting monitoring.'
    : status.issue === 'source_changed' ? 'Review Codex and sign in again before starting monitoring.'
    : status.issue === 'profile_unavailable' ? 'Reload this profile before starting monitoring.'
    : status.issue === 'unsupported' ? 'Codex does not support readings for this profile.'
    : status.issue === 'storage_unavailable' ? 'Monitoring cannot save readings. Check the workspace before starting again.'
    : status.issue === 'provider_error' ? 'Codex could not finish the reading. Check the connection before starting again.'
    : !status.eligible ? 'Sign in again to enable monitoring for this session.'
    : !approval ? 'Review Codex before starting monitoring.' : null;
  return <div className="profile-monitoring" data-monitoring-profile={profileId} data-monitoring-state={unavailable ? undefined : status?.state}>
    <p className="provider-help">{unavailable ? 'Monitoring status unavailable' : !status ? 'Loading monitoring status…' : `Monitoring ${status.state}`}{!unavailable && status?.state === 'running' && status.nextReadInSeconds !== null ? ` · Next read in ${status.nextReadInSeconds}s` : ''}{!unavailable && status?.readingActive ? ' · Reading in progress' : ''}</p>
    {status?.state === 'running' ? <Button type="button" variant="ghost" data-monitoring-pause={profileId} aria-disabled={pending} onClick={() => { void change('pause'); }}>{pending ? 'Pausing monitoring…' : 'Pause monitoring'}</Button>
      : <Button type="button" variant="ghost" data-monitoring-start={profileId} aria-disabled={pending} disabled={busy || unavailable || !status?.eligible || !approval || status.readingActive} onClick={() => { void change('start'); }}>{pending ? 'Starting monitoring…' : 'Start monitoring'}</Button>}
    {guidance && <p className="provider-help">{guidance}</p>}
    {error && <p role="alert" className="operation-error">{error}</p>}
  </div>;
}
