import { useEffect, useRef, useState } from 'react';
import { Button } from './components/ui/button';
import { Input } from './components/ui/input';
import { Checkbox } from './components/ui/checkbox';
import { Field, FieldLabel } from './components/ui/field';
import { invoke, type LeaderboardIssue, type LeaderboardKind, type LeaderboardMetric, type LeaderboardPeriod, type LeaderboardRank, type LeaderboardView } from './contracts';
import { Select, SelectItem, SelectPopup, SelectTrigger, SelectValue } from './components/ui/select';
import { useOpenDetail } from './detail-panel';
import { useDateFormat } from './time-display';

const issues: Record<LeaderboardIssue, string> = {
  unavailable: 'Could not complete the leaderboard action.',
  invalid_arguments: 'Could not complete the leaderboard action.',
  workspace_locked: 'Unlock your workspace, then try again.',
  busy: 'Another leaderboard operation is active.',
  unsupported: 'Leaderboard enrollment is unavailable on this system.',
  stopped: 'OpenUsage will close.',
  unconfigured: 'The leaderboard is unavailable in this build.',
  consent_required: 'Confirm this action before you continue.',
  stale_operation: 'This operation is no longer active. Refresh its status.',
  credential_unavailable: 'Unlock the system credential store, then try again.',
  invalid_credential: 'The saved enrollment is unavailable.',
  already_joined: 'This device is already joined.',
  not_joined: 'Join with GitHub before you continue.',
  save_failed: 'The service confirmed joining. Local saving failed. Continue with GitHub to recover this device.',
  authorization_denied: 'GitHub refused sign-in.',
  authorization_expired: 'GitHub sign-in expired. Start it again.',
  remote_unavailable: 'The service did not confirm this request.',
  remote_refused: 'The service refused this request.',
  rate_limited: 'This request reached the service limit. Try again later.',
  too_many_devices: 'This account reached its device limit.',
  incomplete_data: 'Some saved usage is incomplete or overlaps. Review it in Usage.',
  invalid_data: 'The selected data does not meet the upload rules.',
  too_large: 'Select fewer days to reduce the upload size.',
  preview_expired: 'The upload preview expired. Open it again.',
  local_cleanup_failed: 'The service confirmed public removal. Local credential cleanup failed.',
};
const failure = (value: unknown) => typeof value === 'string' && Object.hasOwn(issues, value)
  ? issues[value as LeaderboardIssue] : 'Could not complete the leaderboard action.';

const rankPeriods = { '7d': '7 days', '30d': '30 days', all: 'All time' } as const;
const rankMetrics = { tokens: 'Tokens', value: 'Estimated value' } as const;
let rankReadTail: Promise<void> = Promise.resolve();
const exactRankCount = (value: string) => new Intl.NumberFormat('en-US').format(BigInt(value));
const exactRankValue = (value: string | null) => {
  if (value === null) return 'Not available';
  const digits = value.padStart(7, '0');
  return `$${exactRankCount(digits.slice(0, -6))}.${digits.slice(-6)}`;
};

function OwnRankDetails() {
  const popupContainer = useRef<HTMLDivElement>(null);
  const [period, setPeriod] = useState<LeaderboardPeriod>('30d');
  const [metric, setMetric] = useState<LeaderboardMetric>('tokens');
  const [refresh, setRefresh] = useState(0);
  const [rank, setRank] = useState<LeaderboardRank | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const dates = useDateFormat();
  useEffect(() => {
    let current = true;
    setLoading(true); setRank(null); setError(null);
    const request = rankReadTail.then(async () => {
      if (!current) return;
      try {
        const result = await invoke('load_leaderboard_rank', { period, metric });
        if (current) setRank(result);
      } catch (cause) {
        if (current) setError(failure(cause));
      } finally { if (current) setLoading(false); }
    });
    rankReadTail = request;
    return () => { current = false; };
  }, [period, metric, refresh]);
  return <div ref={popupContainer} className="unlock-form" data-own-rank>
    <div className="leaderboard-actions">
      <Field><FieldLabel>Period</FieldLabel><Select value={period} onValueChange={value => { if (value && Object.hasOwn(rankPeriods, value)) setPeriod(value as LeaderboardPeriod); }}><SelectTrigger aria-label="Period"><SelectValue>{rankPeriods[period]}</SelectValue></SelectTrigger><SelectPopup portalProps={{ container: popupContainer }}>{Object.entries(rankPeriods).map(([value, label]) => <SelectItem key={value} value={value}>{label}</SelectItem>)}</SelectPopup></Select></Field>
      <Field><FieldLabel>Rank by</FieldLabel><Select value={metric} onValueChange={value => { if (value && Object.hasOwn(rankMetrics, value)) setMetric(value as LeaderboardMetric); }}><SelectTrigger aria-label="Rank by"><SelectValue>{rankMetrics[metric]}</SelectValue></SelectTrigger><SelectPopup portalProps={{ container: popupContainer }}>{Object.entries(rankMetrics).map(([value, label]) => <SelectItem key={value} value={value}>{label}</SelectItem>)}</SelectPopup></Select></Field>
      <Button variant="outline" disabled={loading} onClick={() => setRefresh(value => value + 1)}>Refresh</Button>
    </div>
    <dl className="detail-rows" aria-busy={loading}>
      <dt>Rank</dt><dd data-own-rank-value>{loading ? 'Loading…' : rank?.entry ? exactRankCount(rank.entry.rank) : 'Not available'}</dd>
      {rank && <>
        <dt>Period</dt><dd>{rankPeriods[rank.period]}</dd>
        <dt>Rank by</dt><dd>{rankMetrics[rank.metric]}</dd>
        <dt>Participants</dt><dd>{exactRankCount(rank.totalParticipants)}</dd>
        <dt>Snapshot</dt><dd><time dateTime={rank.snapshotAt} title={rank.snapshotAt}>{dates.format(new Date(rank.snapshotAt))}</time></dd>
        {rank.entry && <><dt>Account</dt><dd><bdi>@{rank.entry.login}</bdi></dd><dt>Total tokens</dt><dd>{exactRankCount(rank.entry.totalTokens)}</dd><dt>Estimated value</dt><dd>{exactRankValue(rank.entry.estimatedValueMicros)}</dd></>}
      </>}
    </dl>
    {error && <p className="operation-error" role="alert">{error}</p>}
  </div>;
}

export function LeaderboardRankAccess({ disabled }: { disabled: boolean }) {
  const open = useOpenDetail();
  return <div className="leaderboard-actions"><Button variant="outline" disabled={disabled} aria-haspopup="dialog" data-own-rank-access onClick={() => open({ label: 'Self-reported rank', title: 'Self-reported rank', content: <OwnRankDetails /> })}>Self-reported rank</Button></div>;
}

export function LeaderboardSettings({ busy }: { busy: boolean }) {
  const [view, setView] = useState<LeaderboardView | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [browserPending, setBrowserPending] = useState(false);
  const [expanded, setExpanded] = useState(false);
  const [days, setDays] = useState('30');
  const [confirming, setConfirming] = useState(false);
  const [confirmation, setConfirmation] = useState<'leave' | 'recover_leave' | 'forget' | null>(null);
  const [consent, setConsent] = useState(false);
  const pending = useRef<number | null>(null);
  const alive = useRef(true);
  const sequence = useRef(0);
  const browserSequence = useRef(0);
  const active = useRef<string | null>(null);
  const focused = useRef<string | null>(null);
  const returnFocus = useRef<LeaderboardKind | null>(null);
  const details = useRef<HTMLDetailsElement>(null);
  const summary = useRef<HTMLElement>(null);
  const joinButton = useRef<HTMLButtonElement>(null);
  const uploadButton = useRef<HTMLButtonElement>(null);
  const code = useRef<HTMLInputElement>(null);
  const previewDialog = useRef<HTMLDialogElement>(null);
  const consentDialog = useRef<HTMLDialogElement>(null);
  const previewCancel = useRef<HTMLButtonElement>(null);
  const consentCancel = useRef<HTMLButtonElement>(null);
  const trigger = useRef<HTMLButtonElement | null>(null);
  const operation = view?.operation;
  useEffect(() => {
    alive.current = true;
    return () => {
      alive.current = false;
      returnFocus.current = null;
      sequence.current++;
      const operationId = active.current;
      if (operationId) void invoke('cancel_leaderboard', { operationId }).catch(() => {});
    };
  }, []);

  function apply(result: LeaderboardView) {
    active.current = result.busy ? result.operation?.id ?? null : null;
    setView(result);
  }
  useEffect(() => {
    if (!expanded || !view?.busy || !operation) return;
    let current = true;
    const generation = sequence.current;
    const timer = setTimeout(async () => {
      if (!details.current?.open || generation !== sequence.current) return;
      try {
        const result = await invoke('load_leaderboard');
        if (current && alive.current && details.current?.open && generation === sequence.current) apply(result);
      } catch (cause) {
        if (current && alive.current && details.current?.open && generation === sequence.current) setError(failure(cause));
      }
    }, 400);
    return () => { current = false; clearTimeout(timer); };
  }, [view, expanded]);
  useEffect(() => {
    if (expanded && details.current?.open && operation?.phase === 'authorizing' && focused.current !== operation.id) {
      focused.current = operation.id;
      returnFocus.current = operation.kind;
      code.current?.focus();
      code.current?.select();
    }
  }, [operation, expanded]);

  useEffect(() => {
    if (!expanded) return;
    if (operation?.phase === 'preview' && previewDialog.current && !previewDialog.current.open) {
      returnFocus.current = operation.kind;
      previewDialog.current.showModal();
      previewCancel.current?.focus();
    }
    if (confirmation && consentDialog.current && !consentDialog.current.open) {
      consentDialog.current.showModal();
      consentCancel.current?.focus();
    }
  }, [operation?.id, operation?.phase, confirmation, expanded]);

  useEffect(() => {
    if (!alive.current || !expanded || !details.current?.open || loading || busy || view?.busy || !returnFocus.current) return;
    const button = returnFocus.current === 'sync' ? uploadButton.current : joinButton.current;
    returnFocus.current = null;
    if (document.activeElement !== document.body && !details.current.contains(document.activeElement)) return;
    (button && !button.disabled ? button : summary.current)?.focus();
  }, [view, loading, busy, expanded]);

  async function load(refresh = false) {
    if (pending.current !== null || !details.current?.open) return;
    const generation = ++sequence.current;
    pending.current = generation;
    setLoading(true);
    setError(null);
    try {
      const result = await (refresh ? invoke('refresh_leaderboard') : invoke('load_leaderboard'));
      if (alive.current && details.current?.open && generation === sequence.current) apply(result);
    } catch (cause) {
      if (alive.current && details.current?.open && generation === sequence.current) setError(failure(cause));
    } finally {
      if (pending.current === generation) pending.current = null;
      if (alive.current && generation === sequence.current) setLoading(false);
    }
  }
  async function begin(kind: LeaderboardKind, selectedDays?: number) {
    if (pending.current !== null || !details.current?.open || busy || view?.busy) return;
    const generation = ++sequence.current;
    pending.current = generation;
    returnFocus.current = kind;
    setLoading(true);
    setError(null);
    try {
      const result = await invoke('begin_leaderboard', { kind, ...(selectedDays === undefined ? {} : { days: selectedDays }), confirmed: true });
      if (!alive.current || !details.current?.open || generation !== sequence.current) {
        if (result.busy && result.operation) void invoke('cancel_leaderboard', { operationId: result.operation.id }).catch(() => {});
        return;
      }
      apply(result);
    } catch (cause) {
      if (alive.current && details.current?.open && generation === sequence.current) setError(failure(cause));
    } finally {
      if (pending.current === generation) pending.current = null;
      if (alive.current && generation === sequence.current) setLoading(false);
    }
  }
  async function cancel(operationId = operation?.id) {
    if (!operationId || pending.current !== null) return;
    const generation = ++sequence.current;
    pending.current = generation;
    setError(null);
    try {
      const result = await invoke('cancel_leaderboard', { operationId });
      if (alive.current && generation === sequence.current) apply(result);
    } catch (cause) {
      if (alive.current && generation === sequence.current) setError(failure(cause));
    } finally { if (pending.current === generation) pending.current = null; }
  }
  async function openGithub() {
    if (!operation || browserPending || !details.current?.open) return;
    const generation = sequence.current;
    const request = ++browserSequence.current;
    setBrowserPending(true);
    try { await invoke('open_leaderboard_login_page', { operationId: operation.id }); }
    catch (cause) { if (alive.current && details.current?.open && generation === sequence.current && active.current === operation.id) setError(failure(cause)); }
    finally { if (alive.current && request === browserSequence.current) setBrowserPending(false); }
  }

  async function confirmUpload() {
    if (operation?.phase !== 'preview' || pending.current !== null) return;
    const generation = ++sequence.current;
    pending.current = generation;
    setConfirming(true);
    setError(null);
    try {
      const result = await invoke('confirm_leaderboard_sync', { operationId: operation.id, confirmed: true });
      if (!alive.current || !details.current?.open || generation !== sequence.current) {
        if (result.busy && result.operation) void invoke('cancel_leaderboard', { operationId: result.operation.id }).catch(() => {});
        return;
      }
      apply(result);
    } catch (cause) {
      if (alive.current && details.current?.open && generation === sequence.current) setError(failure(cause));
    } finally {
      if (pending.current === generation) pending.current = null;
      if (alive.current && generation === sequence.current) setConfirming(false);
    }
  }
  function ask(kind: 'leave' | 'recover_leave' | 'forget', button: HTMLButtonElement) {
    if (busy || view?.busy) return;
    trigger.current = button;
    setError(null);
    setConsent(false);
    setConfirmation(kind);
  }
  function dismiss() {
    setConfirmation(null);
    setConsent(false);
    setError(null);
    requestAnimationFrame(() => { if (alive.current && details.current?.open) trigger.current?.focus(); });
  }
  function submitDestructive() {
    if (!confirmation || !consent || pending.current !== null || busy || view?.busy) return;
    const kind = confirmation;
    setConfirmation(null);
    setConsent(false);
    void begin(kind);
  }
  function previewUpload() {
    const selected = Number(days);
    if (!Number.isInteger(selected) || selected < 1 || selected > 366) { setError('Enter 1 to 366 days.'); return; }
    void begin('sync', selected);
  }

  const label = !view || !view.configured ? 'Unavailable'
    : view.enrollment === 'joined' ? `@${view.login}`
    : ({ not_checked: 'Unavailable', not_joined: 'Not joined', pending: 'Sign-in incomplete', invalid: 'Enrollment unavailable', unavailable: 'Credential store unavailable' } as const)[view.enrollment];
  const outcome = operation?.phase === 'finished' ? operation.outcome : null;
  const joinLabel = view?.enrollment === 'joined' ? 'Reconnect with GitHub' : view?.enrollment === 'pending' ? 'Continue with GitHub' : 'Join with GitHub';
  const canCancel = view?.busy && (operation?.phase === 'preparing' || operation?.phase === 'authorizing' || operation?.phase === 'preview');
  return <section className="provider-panel" data-leaderboard aria-labelledby="leaderboard-heading">
    <details ref={details} onToggle={event => {
      if (event.target !== event.currentTarget) return;
      const open = event.currentTarget.open;
      setExpanded(open);
      if (open) void load();
      else {
        sequence.current++;
        returnFocus.current = null;
        setConfirmation(null);
        setConsent(false);
        browserSequence.current++;
        pending.current = null;
        setLoading(false);
        setBrowserPending(false);
        if (active.current) void cancel(active.current);
      }
    }}>
      <summary ref={summary} className="settings-summary" data-leaderboard-toggle><h2 id="leaderboard-heading">Self-reported leaderboard</h2></summary>
      {loading && <p role="status">Please wait…</p>}
      {view && !loading && <>
        {!view.busy && <>
          <div className="section-heading">
            <span>{outcome?.state === 'joined' && view.configured && view.enrollment === 'joined' && view.login === outcome.login ? `Joined as @${outcome.login}.` : label}</span>
            <Button ref={joinButton} variant="outline" disabled={busy || !view.configured || !['not_joined','pending','joined'].includes(view.enrollment)} onClick={() => void begin(view.enrollment === 'joined' ? 'reconnect' : 'join')}>{joinLabel}</Button>
          </div>
          {view.enrollment === 'joined' && <div className="leaderboard-actions">
            <Field><FieldLabel htmlFor="leaderboard-days">Upload days</FieldLabel><Input nativeInput id="leaderboard-days" type="number" min={1} max={366} step={1} value={days} disabled={busy} onChange={event => setDays(event.target.value)} /></Field>
            <Button ref={uploadButton} variant="outline" disabled={busy || !view.configured} onClick={previewUpload}>Preview upload</Button>
            <Button variant="destructive-outline" disabled={busy || !view.configured} onClick={event => ask('leave', event.currentTarget)}>Leave the leaderboard</Button>
          </div>}
          <details className="provider-advanced"><summary>More actions</summary><div className="leaderboard-actions">
            <Button variant="outline" disabled={busy || !view.configured} onClick={event => ask('recover_leave', event.currentTarget)}>Leave with GitHub</Button>
            <Button variant="destructive-outline" disabled={busy} onClick={event => ask('forget', event.currentTarget)}>Clear local enrollment</Button>
            <Button variant="outline" disabled={busy} onClick={() => void load(true)}>Refresh status</Button>
          </div></details>
        </>}
        {operation?.phase === 'authorizing' && <div className="unlock-form">
          <Field><FieldLabel htmlFor={`github-code-${operation.id}`}>GitHub code</FieldLabel><Input nativeInput ref={code} id={`github-code-${operation.id}`} readOnly value={operation.userCode} /></Field>
          <div className="backup-actions"><Button variant="outline" disabled={browserPending} onClick={() => void openGithub()}>Open GitHub</Button></div>
          <p role="status">Complete sign-in on GitHub.</p>
        </div>}
        {view.busy && operation?.phase !== 'authorizing' && operation?.phase !== 'preview' && <p role="status">{operation?.phase === 'cancelling' ? 'This action will stop.' : 'This action is in progress.'}</p>}
        {view.busy && operation?.phase !== 'preview' && <Button variant="outline" disabled={!canCancel} onClick={() => void cancel()}>Cancel</Button>}
        {outcome?.state === 'cancelled' && <p role="status">{operation?.kind === 'join' || operation?.kind === 'reconnect' ? 'Sign-in cancelled.' : 'Cancelled.'}</p>}
        {outcome?.state === 'synced' && <p role="status">Upload confirmed.</p>}
        {outcome?.state === 'forgotten' && <p role="status">Local enrollment cleared. Public leaderboard data remains.</p>}
        {outcome?.state === 'left' && <p role="status">{outcome.credentials === 'retained' ? 'Public departure confirmed. Saved local enrollment remains.' : outcome.credentials === 'unavailable' ? 'Public departure confirmed. Local credential status is unavailable.' : 'Public departure confirmed.'}</p>}
        {outcome?.state === 'failed' && <div className="operation-error" role="alert"><p>{issues[outcome.issue]}</p>{outcome.remoteEffect === 'unknown' && <p>The public result is unknown. OpenUsage kept local enrollment.</p>}</div>}
        {view.statusIssue && <div className="operation-error" role="alert"><p>{issues[view.statusIssue]}</p><Button variant="outline" onClick={() => void load(true)}>Refresh status</Button></div>}
        {operation?.phase === 'preview' && <dialog className="leaderboard-dialog leaderboard-upload-dialog" ref={previewDialog} aria-labelledby="leaderboard-upload-heading" onCancel={event => { event.preventDefault(); if (!confirming) void cancel(); }}>
          <h3 id="leaderboard-upload-heading">Upload local usage</h3>
          <pre className="leaderboard-preview" data-leaderboard-preview>{operation.body}</pre>
          <div className="profile-actions"><Button ref={previewCancel} variant="outline" disabled={confirming} onClick={() => void cancel()}>Cancel</Button><Button disabled={confirming} onClick={() => void confirmUpload()}>Upload</Button></div>
          {error && <p className="operation-error" role="alert">{error}</p>}
        </dialog>}
      </>}
      {confirmation && <dialog className="leaderboard-dialog" ref={consentDialog} role="alertdialog" aria-modal="true" aria-labelledby="leaderboard-consent-heading" onCancel={event => { event.preventDefault(); dismiss(); }}>
        <h3 id="leaderboard-consent-heading">{confirmation === 'forget' ? 'Clear local enrollment' : confirmation === 'recover_leave' ? 'Leave with GitHub' : 'Leave the leaderboard'}</h3>
        <form className="unlock-form" onSubmit={event => { event.preventDefault(); submitDestructive(); }}>
          {confirmation === 'forget' && <p>Public leaderboard data remains.</p>}
          <Field><FieldLabel className="backup-selection"><Checkbox nativeButton render={<button type="button" />} checked={consent} onCheckedChange={checked => setConsent(checked === true)} />{confirmation === 'forget' ? "Clear this device's saved leaderboard enrollment." : confirmation === 'recover_leave' ? 'Remove the GitHub account I choose, all its devices, and all its public data.' : 'Remove this account, all its devices, and all its public data.'}</FieldLabel></Field>
          <div className="profile-actions"><Button ref={consentCancel} variant="outline" type="button" onClick={dismiss}>Cancel</Button><Button variant="destructive" type="submit" disabled={!consent || busy}>{confirmation === 'forget' ? 'Clear enrollment' : confirmation === 'recover_leave' ? 'Continue with GitHub' : 'Leave'}</Button></div>
        </form>
      </dialog>}
      {error && operation?.phase !== 'preview' && <div className="operation-error" role="alert"><p>{error}</p><Button variant="outline" onClick={() => void load(true)} disabled={loading}>Refresh status</Button></div>}
    </details>
  </section>;
}
