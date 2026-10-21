import { useEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { useAccountSlot } from './account-slots';
import { Button } from './components/ui/button';
import { Input } from './components/ui/input';
import { Field, FieldLabel } from './components/ui/field';
import { Spinner } from './components/ui/spinner';
import { invoke, type ProviderExecutableApproval, type ProviderLoginAttempt, type ProviderLoginPrompt } from './contracts';
import { providerFailure } from './provider-errors';

type Props = {
  profileId: string;
  profileLabel: string;
  approval: ProviderExecutableApproval;
  busy: boolean;
  onActivity: (active: boolean) => void;
  onStarted: () => void;
  onConnectionChanged: () => Promise<void>;
  onAuthenticated?: () => void;
  actionContainerId?: string;
  connected?: boolean;
  actionLabel?: string;
};

type Outcome = { kind: 'success' | 'cancelled' | 'error' | 'incomplete'; message: string };

function selectField(field: HTMLInputElement | HTMLTextAreaElement) {
  field.select();
  requestAnimationFrame(() => { if (document.activeElement === field) field.select(); });
}

export function ProfileDeviceLogin({ profileId, profileLabel, approval, busy, onActivity, onStarted, onConnectionChanged, onAuthenticated, actionContainerId, connected = false, actionLabel = 'Sign in' }: Props) {
  const actionContainer = useAccountSlot(actionContainerId);
  const [working, setWorking] = useState(false);
  const [cancelling, setCancelling] = useState(false);
  const [openingBrowser, setOpeningBrowser] = useState(false);
  const [browserError, setBrowserError] = useState<string | null>(null);
  const [attempt, setAttempt] = useState<ProviderLoginAttempt | null>(null);
  const [prompt, setPrompt] = useState<ProviderLoginPrompt | null>(null);
  const [outcome, setOutcome] = useState<Outcome | null>(null);
  const [connectionRefreshError, setConnectionRefreshError] = useState<string | null>(null);
  const [refreshingConnection, setRefreshingConnection] = useState(false);
  const [focus, setFocus] = useState<'prompt' | 'result' | 'launch' | null>(null);
  const launch = useRef<HTMLButtonElement>(null);
  const code = useRef<HTMLInputElement>(null);
  const result = useRef<HTMLDivElement>(null);
  const pending = useRef(false);
  const cancelled = useRef(false);
  const cancellationFailure = useRef<unknown>(null);
  const activeAttempt = useRef<string | null>(null);
  const generation = useRef(0);
  const authenticatedOperation = useRef<number | null>(null);
  const alive = useRef(true);
  const browserRequest = useRef<string | null>(null);
  useEffect(() => {
    alive.current = true;
    return () => {
      alive.current = false;
      const attemptId = activeAttempt.current;
      if (attemptId) void invoke('cancel_profile_login', { profileId, attemptId }).catch(() => {});
    };
  }, [profileId]);
  useEffect(() => {
    if (!focus) return;
    const target = focus === 'prompt' ? code.current : focus === 'result' ? result.current : launch.current;
    if (target) { target.focus(); setFocus(null); }
  }, [focus, prompt, outcome]);

  async function refreshConnection(operation: number) {
    const current = () => alive.current && generation.current === operation && !cancelled.current && !cancellationFailure.current;
    if (!current()) return;
    setRefreshingConnection(true);
    try {
      await onConnectionChanged();
      if (current()) {
        setConnectionRefreshError(null);
        if (authenticatedOperation.current === operation) {
          authenticatedOperation.current = null;
          onAuthenticated?.();
        }
      }
    } catch {
      if (current()) setConnectionRefreshError('The sign-in result was saved, but the connection status could not be refreshed. Retry the status refresh to check it.');
    } finally {
      if (current()) setRefreshingConnection(false);
    }
  }

  async function start() {
    if (busy || pending.current) return;
    const operation = ++generation.current;
    authenticatedOperation.current = null;
    pending.current = true;
    cancelled.current = false;
    cancellationFailure.current = null;
    setWorking(true);
    setOutcome(null);
    setConnectionRefreshError(null);
    setRefreshingConnection(false);
    setPrompt(null);
    browserRequest.current = null;
    setOpeningBrowser(false);
    setBrowserError(null);
    onActivity(true);
    onStarted();
    try {
      const begun = await invoke('begin_profile_login', { profileId, approvalId: approval.approvalId, flow: 'device_code', confirmed: true });
      if (begun.profileId !== profileId || begun.approvalId !== approval.approvalId || begun.sourceKind !== approval.sourceKind) throw new Error('Incompatible sign-in binding.');
      activeAttempt.current = begun.attemptId;
      if (!alive.current) { await invoke('cancel_profile_login', { profileId, attemptId: begun.attemptId }); return; }
      setAttempt(begun);
      const args = { profileId, attemptId: begun.attemptId };
      let challenge: ProviderLoginPrompt | null = await invoke('await_profile_login_prompt', args);
      if (challenge.profileId !== profileId || challenge.attemptId !== begun.attemptId || challenge.sourceKind !== begun.sourceKind) throw new Error('Incompatible sign-in challenge.');
      if (alive.current && !cancelled.current) {
        setPrompt(challenge); setFocus('prompt');
        if (approval.sourceKind === 'official_artifact') {
          setOpeningBrowser(true);
          void invoke('open_profile_login_page', args).catch(failure => {
            if (alive.current && generation.current === operation && !cancelled.current) setBrowserError(providerFailure(failure));
          }).finally(() => { if (alive.current && generation.current === operation) setOpeningBrowser(false); });
        }
      }
      challenge = null;
      const completed = await invoke('complete_profile_login', args);
      if (completed.attemptId !== begun.attemptId || completed.diagnostic.profileId !== profileId || completed.diagnostic.approvalId !== approval.approvalId || completed.diagnostic.sourceKind !== begun.sourceKind) throw new Error('Incompatible sign-in result.');
      if (alive.current) {
        setOutcome(cancellationFailure.current
          ? { kind: 'error', message: providerFailure(cancellationFailure.current) }
          : cancelled.current
          ? { kind: 'cancelled', message: 'Credentials already saved by Codex may remain in this profile.' }
          : completed.diagnostic.identityConfidence === 'unknown'
          ? { kind: 'incomplete', message: 'OpenAI completed sign-in, but account details are unavailable. Try reconnecting. Your saved readings are still available.' }
          : { kind: 'success', message: 'Connected. Retrieving available usage…' });
        if (!cancelled.current && !cancellationFailure.current) {
          if (completed.diagnostic.persisted && completed.diagnostic.authentication === 'chatgpt' && completed.diagnostic.identityConfidence === 'provisional') authenticatedOperation.current = operation;
          void refreshConnection(operation);
        }
      }
    } catch (failure) {
      const problem = cancellationFailure.current ?? failure;
      const cancellationResult = failure === 'provider_login_cancelled'
        || (cancelled.current && (failure === 'provider_login_not_found' || failure === 'provider_check_cancelled'));
      if (alive.current) setOutcome(!cancellationFailure.current && cancellationResult
        ? { kind: 'cancelled', message: 'Credentials already saved by Codex may remain in this profile.' }
        : { kind: 'error', message: providerFailure(problem) });
    } finally {
      activeAttempt.current = null;
      pending.current = false;
      if (alive.current) {
        setPrompt(null);
        browserRequest.current = null;
        setOpeningBrowser(false);
        setBrowserError(null);
        setAttempt(null);
        setWorking(false);
        setCancelling(false);
        setFocus('result');
        onActivity(false);
      }
    }
  }

  async function cancel() {
    if (!attempt || !pending.current || cancelling) return;
    const operation = generation.current;
    cancelled.current = true;
    setPrompt(null);
    setBrowserError(null);
    setCancelling(true);
    try { await invoke('cancel_profile_login', { profileId, attemptId: attempt.attemptId }); }
    catch (failure) {
      if (alive.current && generation.current === operation) {
        cancellationFailure.current = failure;
        setOutcome({ kind: 'error', message: providerFailure(failure) });
        setFocus('result');
      }
    } finally { if (alive.current && generation.current === operation) setCancelling(false); }
  }

  async function openBrowser() {
    if (!attempt || !prompt || cancelled.current || cancelling || browserRequest.current) return;
    const attemptId = attempt.attemptId;
    const operation = generation.current;
    browserRequest.current = attemptId;
    setOpeningBrowser(true);
    setBrowserError(null);
    try {
      await invoke('open_profile_login_page', { profileId, attemptId });
    } catch (failure) {
      if (alive.current && generation.current === operation && activeAttempt.current === attemptId && !cancelled.current) setBrowserError(providerFailure(failure));
    } finally {
      if (browserRequest.current === attemptId) {
        browserRequest.current = null;
        if (alive.current && generation.current === operation) setOpeningBrowser(false);
      }
    }
  }

  const content = <div className="profile-device-login">
    {!working && (!connected || outcome?.kind === 'error' || outcome?.kind === 'incomplete') && <Button ref={launch} className="primary-button" type="button" data-login-start={profileId} aria-describedby={`profile-name-${profileId}`} disabled={busy} onClick={() => { void start(); }}>{outcome?.kind === 'error' || outcome?.kind === 'incomplete' ? 'Retry sign-in' : actionLabel}</Button>}
    {working && <div className="login-panel" role="dialog" aria-labelledby={`login-pending-${profileId}`} onKeyDown={event => { if (event.key === 'Escape') { event.preventDefault(); void cancel(); } }}>
      <h3 id={`login-pending-${profileId}`}>Sign in to <bdi>{profileLabel}</bdi></h3>
      {approval.sourceKind === 'test_fixture' && <p className="provider-fixture-note" data-provider-fixture>Synthetic provider fixture. Do not use this code for a real sign-in.</p>}
      {prompt ? <div className="login-prompt" data-login-profile={profileId}>
        <p>Enter this code on OpenAI’s sign-in page.</p>
        <Field><FieldLabel htmlFor={`login-code-${profileId}`}>Your sign-in code</FieldLabel>
        <Input ref={code} id={`login-code-${profileId}`} inputClassName="login-code" type="text" autoComplete="off" readOnly value={prompt.userCode} onFocus={event => selectField(event.currentTarget)} />
        </Field>
        <Button variant="outline" type="button" data-login-browser={profileId} disabled={openingBrowser || cancelling} aria-describedby={browserError ? `login-browser-error-${profileId}` : undefined} onClick={() => { void openBrowser(); }}>{openingBrowser ? 'Opening…' : 'Open sign-in page'}</Button>
        {browserError && <p id={`login-browser-error-${profileId}`} role="alert" className="operation-error">{browserError}</p>}
      </div> : <p role="status"><Spinner aria-hidden="true" /> {cancelling || cancelled.current ? 'Stopping sign-in…' : 'Preparing sign-in…'}</p>}
      {attempt && <Button className="secondary-button" variant="outline" type="button" data-login-cancel={profileId} aria-describedby={`profile-name-${profileId}`} disabled={cancelling} onClick={() => { void cancel(); }}>{cancelling ? 'Stopping sign-in…' : 'Cancel sign-in'}</Button>}
    </div>}
    {outcome && !(connected && outcome.kind === 'success') && <div ref={result} className="login-panel login-result" data-login-profile={profileId} role={outcome.kind === 'error' || outcome.kind === 'incomplete' ? 'alert' : 'status'} tabIndex={-1}>
      {approval.sourceKind === 'test_fixture' && <p className="provider-fixture-note" data-provider-fixture>Synthetic provider fixture</p>}
      <h3>{outcome.kind === 'success' ? 'Sign-in completed' : outcome.kind === 'cancelled' ? 'Sign-in stopped' : outcome.kind === 'incomplete' ? 'Connection needs attention' : 'Sign-in could not finish'}</h3>
      <p>{outcome.message}</p>
      {connectionRefreshError && <div>
        <p className="operation-error" role="alert" data-login-connection-refresh-error>{connectionRefreshError}</p>
        <Button variant="outline" type="button" disabled={busy || working || refreshingConnection} onClick={() => { void refreshConnection(generation.current); }}>Retry status refresh</Button>
      </div>}
    </div>}
  </div>;
  return actionContainer ? createPortal(content, actionContainer) : actionContainerId ? null : content;
}
