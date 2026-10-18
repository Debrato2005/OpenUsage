import { useEffect, useRef, useState } from 'react';
import { invoke, type ProviderExecutableApproval, type ProviderLoginAttempt, type ProviderLoginPrompt } from './contracts';
import { providerFailure } from './provider-errors';

type Props = {
  profileId: string;
  profileLabel: string;
  approval: ProviderExecutableApproval;
  busy: boolean;
  onActivity: (active: boolean) => void;
  onStarted: () => void;
};

type Outcome = { kind: 'success' | 'cancelled' | 'error'; message: string };

export function ProfileDeviceLogin({ profileId, profileLabel, approval, busy, onActivity, onStarted }: Props) {
  const [consent, setConsent] = useState(false);
  const [working, setWorking] = useState(false);
  const [cancelling, setCancelling] = useState(false);
  const [attempt, setAttempt] = useState<ProviderLoginAttempt | null>(null);
  const [prompt, setPrompt] = useState<ProviderLoginPrompt | null>(null);
  const [outcome, setOutcome] = useState<Outcome | null>(null);
  const [focus, setFocus] = useState<'consent' | 'prompt' | 'result' | 'launch' | null>(null);
  const launch = useRef<HTMLButtonElement>(null);
  const submit = useRef<HTMLButtonElement>(null);
  const code = useRef<HTMLInputElement>(null);
  const result = useRef<HTMLDivElement>(null);
  const pending = useRef(false);
  const cancelled = useRef(false);
  const cancellationFailure = useRef<unknown>(null);
  const activeAttempt = useRef<string | null>(null);
  const generation = useRef(0);
  const alive = useRef(true);
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
    const target = focus === 'consent' ? submit.current : focus === 'prompt' ? code.current : focus === 'result' ? result.current : launch.current;
    if (target) { target.focus(); setFocus(null); }
  }, [focus, consent, prompt, outcome]);

  function explain() {
    if (busy || pending.current) return;
    setOutcome(null);
    setConsent(true);
    setFocus('consent');
  }

  async function start() {
    if (busy || pending.current || !consent) return;
    generation.current += 1;
    pending.current = true;
    cancelled.current = false;
    cancellationFailure.current = null;
    setConsent(false);
    setWorking(true);
    setOutcome(null);
    setPrompt(null);
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
      if (alive.current && !cancelled.current) { setPrompt(challenge); setFocus('prompt'); }
      challenge = null;
      const completed = await invoke('complete_profile_login', args);
      if (completed.attemptId !== begun.attemptId || completed.diagnostic.profileId !== profileId || completed.diagnostic.approvalId !== approval.approvalId || completed.diagnostic.sourceKind !== begun.sourceKind) throw new Error('Incompatible sign-in result.');
      if (alive.current) {
        setOutcome(cancellationFailure.current
          ? { kind: 'error', message: providerFailure(cancellationFailure.current) }
          : cancelled.current
          ? { kind: 'cancelled', message: 'Credentials already saved by Codex may remain in this profile.' }
          : { kind: 'success', message: 'Codex confirmed a ChatGPT account for this profile. Identity has not been verified. Usage collection remains stopped.' });
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

  return <div className="profile-device-login">
    {!working && !consent && <button ref={launch} className="secondary-button" type="button" data-login-start={profileId} aria-describedby={`profile-name-${profileId}`} disabled={busy} onClick={explain}>Sign in</button>}
    {consent && <form className="login-panel" data-login-consent={profileId} aria-labelledby={`login-heading-${profileId}`} onSubmit={event => { event.preventDefault(); void start(); }}>
      <h3 id={`login-heading-${profileId}`}>Sign in to <bdi>{profileLabel}</bdi></h3>
      <p>Codex will contact OpenAI to request a sign-in code. If you finish, Codex saves sign-in information for this profile on this device.</p>
      <p className="provider-help">Cancelling stops this attempt. Credentials already saved by Codex may remain.</p>
      {approval.sourceKind === 'test_fixture' && <p className="provider-fixture-note" data-provider-fixture>Synthetic provider fixture. This flow uses controlled test data.</p>}
      <div className="profile-actions"><button ref={submit} className="primary-button" type="submit" disabled={busy}>Get sign-in code</button><button className="text-button" type="button" onClick={() => { setConsent(false); setFocus('launch'); }}>Back</button></div>
    </form>}
    {working && <div className="login-panel" aria-labelledby={`login-pending-${profileId}`}>
      <h3 id={`login-pending-${profileId}`}>Sign in to <bdi>{profileLabel}</bdi></h3>
      {approval.sourceKind === 'test_fixture' && <p className="provider-fixture-note" data-provider-fixture>Synthetic provider fixture. Do not use this code for a real sign-in.</p>}
      {prompt ? <div className="login-prompt" data-login-profile={profileId}>
        <p>Open this address in your browser, then enter the code.</p>
        <label htmlFor={`login-address-${profileId}`}>Sign-in page</label>
        <input id={`login-address-${profileId}`} type="text" readOnly value={prompt.verificationUrl} onFocus={event => event.currentTarget.select()} />
        <label htmlFor={`login-code-${profileId}`}>Your sign-in code</label>
        <input ref={code} id={`login-code-${profileId}`} className="login-code" type="text" autoComplete="off" readOnly value={prompt.userCode} aria-describedby={`login-wait-${profileId}`} onFocus={event => event.currentTarget.select()} />
        <p id={`login-wait-${profileId}`} className="provider-help">OpenUsage waits up to 15 minutes. This is an application time limit, not the code’s expiry time.</p>
      </div> : <p role="status">{cancelling || cancelled.current ? 'Stopping sign-in…' : 'Preparing your sign-in code…'}</p>}
      {attempt && <button className="secondary-button" type="button" data-login-cancel={profileId} disabled={cancelling} onClick={() => { void cancel(); }}>{cancelling ? 'Stopping sign-in…' : 'Cancel sign-in'}</button>}
    </div>}
    {outcome && <div ref={result} className="login-panel login-result" data-login-profile={profileId} role={outcome.kind === 'error' ? 'alert' : 'status'} tabIndex={-1}>
      {approval.sourceKind === 'test_fixture' && <p className="provider-fixture-note" data-provider-fixture>Synthetic provider fixture</p>}
      <h3>{outcome.kind === 'success' ? 'Sign-in completed' : outcome.kind === 'cancelled' ? 'Sign-in stopped' : 'Sign-in could not finish'}</h3>
      <p>{outcome.message}</p>
    </div>}
  </div>;
}
