import openusageLogo from './assets/openusage-logo.png';
import { Button } from './components/ui/button';
import { Input } from './components/ui/input';
import { Checkbox } from './components/ui/checkbox';
import { Field, FieldLabel } from './components/ui/field';
import { Select, SelectTrigger, SelectValue, SelectPopup, SelectItem } from './components/ui/select';
import { StrictMode, useCallback, useEffect, useRef, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { DesktopUnavailable, invoke, type BackupReceipt, type Bootstrap, type Theme, type ProviderExecutableApproval, type ProfileRecord } from './contracts';
import { ProfileManager } from './profiles';
import { BackupInspection } from './backup-inspection';
import { DataExport } from './data-export';
import { CheckpointRecovery } from './checkpoint-recovery';
import { AlertHistory } from './alert-history';
import { ResetNotifications } from './reset-notifications';
import { ProviderSettings } from './provider';
import { Overview } from './overview';
import type { AccountAction, AccountActionRequest } from './account-actions';
import { Usage } from './usage';
import { useMonitoring } from './monitoring';
import { providerFailure } from './provider-errors';
import './styles.css';
import './workspace-layout.css';

type Startup = { kind: 'loading' } | { kind: 'desktop'; data: Bootstrap }
  | { kind: 'unavailable' } | { kind: 'failed' };

function App() {
  const [startup, setStartup] = useState<Startup>({ kind: 'loading' });
  const actionSequence = useRef(0);
  const [accountAction, setAccountAction] = useState<AccountActionRequest | null>(null);
  const handledAccountAction = useCallback((sequence: number) => setAccountAction(current => current?.sequence === sequence ? null : current), []);
  const [displayScale, setDisplayScale] = useState(100);
  const [scaleBusy, setScaleBusy] = useState(false);
  const [scaleError, setScaleError] = useState(false);
  useEffect(() => { let current = true; void invoke('get_display_scale').then(value => { if (current) setDisplayScale(value.percent); }).catch(() => {}); return () => { current = false; }; }, []);
  const [view, setView] = useState<WorkspaceView>('overview');
  const [accountPanelOpen, setAccountPanelOpen] = useState(false);
  const [accountPanelProfile, setAccountPanelProfile] = useState<string | null>(null);
  const [usageProfile, setUsageProfile] = useState<string | null>(null);
  const [refreshingProfiles, setRefreshingProfiles] = useState<string[]>([]);
  const [refreshWaitSeconds, setRefreshWaitSeconds] = useState<Record<string, number>>({});
  const manualRefreshes = useRef(new Set<string>());
  const [accountMessage, setAccountMessage] = useState<{ profileId: string; outcome: 'saved' | 'failed' } | null>(null);
  const accountPanel = useRef<HTMLDialogElement>(null);
  const accountReturnFocus = useRef<HTMLElement | null>(null);
  const refreshState = useCallback((id: string, working: boolean, waitSeconds: number) => {
    setRefreshingProfiles(current => working ? current.includes(id) ? current : [...current, id] : current.includes(id) ? current.filter(value => value !== id) : current);
    setRefreshWaitSeconds(current => (current[id] ?? 0) === waitSeconds ? current : { ...current, [id]: waitSeconds });
  }, []);
  const reportAccountOperation = useCallback((event: { profileId: string; outcome: 'saved' | 'failed' }) => {
    if (manualRefreshes.current.delete(event.profileId)) setAccountMessage(event);
  }, []);
  useEffect(() => {
    const dialog = accountPanel.current;
    if (!dialog) return;
    if (accountPanelOpen && !dialog.open) {
      accountReturnFocus.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
      dialog.showModal();
    } else if (!accountPanelOpen && dialog.open) {
      dialog.close();
      if (accountReturnFocus.current?.isConnected) accountReturnFocus.current.focus({ preventScroll: true });
    }
  }, [accountPanelOpen, startup]);
  const [profiles, setProfiles] = useState<ProfileRecord[]>([]);
  const [readingRevision, setReadingRevision] = useState(0);
  const updateProfiles = useCallback((value: ProfileRecord[]) => setProfiles(value), []);
  const readingsChanged = useCallback(() => setReadingRevision(n => n + 1), []);
  const [theme, setTheme] = useState<Theme>('system');
  const [attempt, setAttempt] = useState(0);
  const [privacy, setPrivacy] = useState(false);
  const [busy, setBusy] = useState(false);
  const [themeFocusPending, setThemeFocusPending] = useState(false);
  const themeControl = useRef<HTMLButtonElement>(null);
  const [operationError, setOperationError] = useState<string | null>(null);
  const [workspaceErrorField, setWorkspaceErrorField] = useState<'workspace-passphrase' | 'workspace-confirmation' | null>(null);
  const [passphrase, setPassphrase] = useState('');
  const [confirmation, setConfirmation] = useState('');
  const [backupAction, setBackupAction] = useState<'create' | 'inspect' | 'recover' | null>(null);
  const [backupRecords, setBackupRecords] = useState(false);
  const [backupPassword, setBackupPassword] = useState('');
  const [backupConfirmation, setBackupConfirmation] = useState('');
  const [backupError, setBackupError] = useState<string | null>(null);
  const [backupErrorField, setBackupErrorField] = useState<'backup-password' | 'backup-confirmation' | null>(null);
  const [backupReceipt, setBackupReceipt] = useState<BackupReceipt | null>(null);
  const backupToggle = useRef<HTMLButtonElement>(null);
  const [profileRevision, setProfileRevision] = useState(0);
  const [providerApproval, setProviderApproval] = useState<ProviderExecutableApproval | null>(null);
  const [connectionRequest, setConnectionRequest] = useState(0);
  const preparedRequest = useRef(0);
  const connectionGeneration = useRef(0);
  const [connectionPreparing, setConnectionPreparing] = useState(false);
  const [connectionError, setConnectionError] = useState<string | null>(null);
  useEffect(() => () => { connectionGeneration.current += 1; }, []);
  useEffect(() => {
    if (!connectionRequest || connectionRequest === preparedRequest.current || busy || providerApproval) return;
    preparedRequest.current = connectionRequest;
    const generation = connectionGeneration.current;
    setBusy(true); setConnectionPreparing(true); setConnectionError(null);
    void invoke('prepare_provider_connection').then(value => {
      if (generation === connectionGeneration.current) setProviderApproval(value);
    }).catch(error => {
      if (generation === connectionGeneration.current) setConnectionError(error === 'provider_source_unverified' || error === 'stale_provider_approval'
        ? 'OpenUsage could not verify its OpenAI companion. Check the application installation or open Connection details.'
        : providerFailure(error));
    }).finally(() => {
      if (generation === connectionGeneration.current) { setConnectionPreparing(false); setBusy(false); }
    });
  }, [connectionRequest, busy, providerApproval]);
  const updateProfileCount = useCallback((count: number) => {
    setStartup(current => current.kind === 'desktop' && current.data.storage === 'ready'
      ? { ...current, data: { ...current.data, activeProfiles: count } } : current);
  }, []);
  const refreshRestoredWorkspace = useCallback(async () => {
    connectionGeneration.current += 1;
    setConnectionPreparing(false); setConnectionError(null);
    const updated = await invoke('bootstrap');
    setStartup({ kind: 'desktop', data: updated });
    setTheme(updated.theme);
    setProfileRevision(value => value + 1);
    setProfiles([]);
    setAccountAction(null);
    setAccountPanelOpen(false); setAccountPanelProfile(null); setRefreshingProfiles([]); setRefreshWaitSeconds({}); manualRefreshes.current.clear(); setAccountMessage(null);
    setReadingRevision(value => value + 1);
  }, []);

  useEffect(() => {
    let cancelled = false;
    void invoke('bootstrap').then(data => {
      if (!cancelled) { setStartup({ kind: 'desktop', data }); setTheme(data.theme); }
    }).catch((error: unknown) => {
      if (!cancelled) setStartup({ kind: error instanceof DesktopUnavailable ? 'unavailable' : 'failed' });
    });
    return () => { cancelled = true; };
  }, [attempt]);

  useEffect(() => { document.documentElement.dataset.theme = theme; }, [theme]);
  useEffect(() => {
    if (busy || !themeFocusPending || !themeControl.current) return;
    const trigger = themeControl.current;
    const restoreWhenEnabled = () => {
      if (trigger.disabled) return;
      if (document.activeElement === document.body) trigger.focus({ preventScroll: true });
      setThemeFocusPending(false);
    };
    // The composite control can commit its enabled state after this effect.
    const observer = new MutationObserver(restoreWhenEnabled);
    observer.observe(trigger, { attributes: true, attributeFilter: ['disabled'] });
    restoreWhenEnabled();
    return () => observer.disconnect();
  }, [busy, themeFocusPending]);
  useEffect(() => { if (backupReceipt) backupToggle.current?.focus(); }, [backupReceipt]);
  useEffect(() => {
    if (!busy && operationError && workspaceErrorField) document.getElementById(workspaceErrorField)?.focus();
  }, [busy, operationError, workspaceErrorField]);
  useEffect(() => {
    if (!busy && backupError && backupErrorField) document.getElementById(backupErrorField)?.focus();
  }, [busy, backupError, backupErrorField]);

  const data = startup.kind === 'desktop' ? startup.data : null;
  const backupOpen = backupAction === 'create';
  const ready = data?.storage === 'ready';
  const monitoring = useMonitoring(ready, profileRevision, readingsChanged);
  const recovery = data?.storage === 'recovery_required';
  const needsPassphrase = data?.keyProtection === 'passphrase_required'
    || (data?.storage === 'locked' && data.keyProtection === 'passphrase');

  async function updateWorkspace(operation: () => Promise<Bootstrap>) {
    setBusy(true);
    setOperationError(null);
    setWorkspaceErrorField(null);
    try {
      const updated = await operation();
      setStartup({ kind: 'desktop', data: updated });
      setTheme(updated.theme);
    } catch (error: unknown) {
      const messages: Record<string, string> = {
        invalid_passphrase: 'That app password did not unlock your data. Try again.',
        passphrase_too_short: 'Use an app password with at least 12 characters.',
        passphrase_too_long: 'Use a shorter app password.',
        passphrase_required: 'Set an app password to protect your saved account data.',
        workspace_in_use: 'OpenUsage already uses this workspace in another window. Close that window, then try again.',
        workspace_locked: 'Unlock your workspace before you save changes.',
        already_initialized: 'This workspace already exists. Check its current state before you continue.',
        os_store_required: 'Use your system key store to open this workspace.',
      };
      setOperationError(typeof error === 'string' && messages[error]
        ? messages[error] : 'OpenUsage could not complete this change. Keep your workspace files, then try again.');
      if (typeof error === 'string' && ['invalid_passphrase', 'passphrase_too_short', 'passphrase_too_long', 'passphrase_required'].includes(error)) {
        setWorkspaceErrorField('workspace-passphrase');
      }
    } finally {
      setPassphrase('');
      setConfirmation('');
      setBusy(false);
    }
  }

  function changeTheme(next: Theme) {
    if (ready) {
      void updateWorkspace(() => invoke('set_theme', { theme: next })).then(() => {
        setThemeFocusPending(true);
      });
    } else {
      setTheme(next);
    }
  }

  async function saveBackup() {
    if (!backupRecords) return;
    if (backupPassword !== backupConfirmation) {
      setBackupError('The passwords do not match.');
      setBackupErrorField('backup-confirmation');
      return;
    }
    setBusy(true);
    setBackupError(null);
    setBackupErrorField(null);
    setBackupReceipt(null);
    try {
      setBackupReceipt(await invoke('create_backup', { password: backupPassword }));
      setBackupAction(null);
      setBackupRecords(false);
    } catch (error: unknown) {
      const messages: Record<string, string> = {
        passphrase_too_short: 'Use a backup password with at least 12 characters.',
        passphrase_too_long: 'Use a shorter backup password.',
        insufficient_space: 'There is not enough free space for a verified backup. Free space in your Downloads folder, then try again.',
        workspace_locked: 'Unlock your workspace before you save a backup.',
      };
      setBackupError(typeof error === 'string' && messages[error]
        ? messages[error] : 'OpenUsage could not save the backup. Keep your workspace files, then try again.');
      if (error === 'passphrase_too_short' || error === 'passphrase_too_long') setBackupErrorField('backup-password');
    } finally {
      setBackupPassword('');
      setBackupConfirmation('');
      setBusy(false);
    }
  }
  function toggleBackup(action: 'create' | 'inspect' | 'recover') {
    setBackupAction(current => current === action ? null : action);
    setBackupPassword('');
    setBackupConfirmation('');
    setBackupError(null);
  }
  const message = startup.kind === 'loading' ? 'Checking local storage…'
    : startup.kind === 'unavailable' ? 'OpenUsage needs its desktop application'
    : startup.kind === 'failed' ? 'OpenUsage could not read its local state'
    : recovery ? 'Your local data needs recovery'
    : ready ? 'Workspace ready' : data?.storage === 'locked' ? 'Welcome back' : 'Welcome to OpenUsage';
  const recoveryMessage = data?.storageIssue === 'workspace_in_use'
    ? 'Another OpenUsage window uses this workspace. Close that window, then check the workspace again.'
    : data?.storageIssue === 'missing_database'
    ? 'The protected key exists, but its database is missing. Keep your database and key files. Recover your original encrypted database before you try again.'
    : 'OpenUsage cannot open the existing database. Keep your database and key files. Recover the original database and its protected key before you try again.';

  const appearance = <div className="theme-control"><label id="theme-label" htmlFor="theme">Appearance</label><Select modal={false} value={theme} disabled={busy} items={{ system: 'System', light: 'Light', dark: 'Dark' }} onValueChange={next => { if (next === 'system' || next === 'light' || next === 'dark') changeTheme(next); }}><SelectTrigger ref={themeControl} id="theme" aria-labelledby="theme-label">{ready && <svg className="appearance-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" aria-hidden="true"><circle cx="12" cy="12" r="4"/><path d="M12 2v2m0 16v2M2 12h2m16 0h2M5 5l1.4 1.4m11.2 11.2L19 19M5 19l1.4-1.4M17.6 6.4 19 5"/></svg>}<SelectValue className={ready ? 'sr-only' : undefined} /></SelectTrigger><SelectPopup alignItemWithTrigger={false}><SelectItem value="system">System</SelectItem><SelectItem value="light">Light</SelectItem><SelectItem value="dark">Dark</SelectItem></SelectPopup></Select></div>;
  function navigate(next: WorkspaceView) {
    setAccountPanelOpen(false);
    setView(next);
    window.scrollTo({ top: 0, behavior: 'instant' });
  }
  function requestAccountAction(action: AccountAction) {
    if (action.kind === 'usage') { setUsageProfile(action.profileId); navigate('usage'); return; }
    if (action.kind === 'refresh') { manualRefreshes.current.add(action.profileId); setAccountMessage(null); setAccountAction({ ...action, sequence: ++actionSequence.current }); return; }
    setAccountPanelProfile(action.kind === 'add' ? null : action.profileId);
    setAccountPanelOpen(true);
    setAccountAction({ ...action, sequence: ++actionSequence.current });
  }
  async function changeScale(percent: number) {
    if (scaleBusy) return;
    setScaleBusy(true); setScaleError(false);
    try { setDisplayScale((await invoke('set_display_scale', { percent })).percent); }
    catch { setScaleError(true); }
    finally { setScaleBusy(false); }
  }
  const scaleControl = <div className="display-scale-control"><label htmlFor="display-scale" className="sr-only">Display scale</label><Select modal={false} value={displayScale} disabled={scaleBusy} items={Object.fromEntries([90, 100, 110, 125, 150, 175, 200].map(value => [value, `${value}%`]))} onValueChange={value => { if (value !== null) void changeScale(value); }}><SelectTrigger id="display-scale" aria-label="Display scale"><SelectValue /></SelectTrigger><SelectPopup alignItemWithTrigger={false}>{[90, 100, 110, 125, 150, 175, 200].map(value => <SelectItem key={value} value={value}>{value}%</SelectItem>)}</SelectPopup></Select></div>;
  const titles = { overview: 'Overview', usage: 'Usage', settings: 'Settings' };
  const descriptions = {
    overview: 'Monitor your OpenAI usage and available quotas.',
    usage: 'Token activity saved on this device.',
    settings: 'Appearance, Codex connection, and encrypted backups.',
  };
  return <div className={`app-shell${ready ? '' : ' onboarding-shell'}`}>
    <a className="skip-link" href="#main">Skip to dashboard</a>
    <header className="workspace-header" hidden={!ready}>
      <a className="brand" href="#main" onClick={() => navigate('overview')} aria-label="OpenUsage dashboard"><span className="brand-wordmark"><img src={openusageLogo} alt="OpenUsage" /></span></a>
      <nav className="workspace-nav" aria-label="Main">{(Object.keys(titles) as WorkspaceView[]).map(name => <a key={name} href={`#${name}`} data-workspace-nav={name} aria-current={view === name ? 'page' : undefined} onClick={event => { event.preventDefault(); navigate(name); }}><NavIcon view={name} /><span>{titles[name]}</span></a>)}</nav>
      {ready && <div className="workspace-appearance"><div className="header-appearance">{appearance}</div>{scaleControl}</div>}
    </header>
    <div className="workspace">
      {!ready && <header className="onboarding-header"><a className="brand" href="#main" aria-label="OpenUsage"><span className="brand-wordmark"><img src={openusageLogo} alt="OpenUsage" /></span></a>{appearance}</header>}
      <main id="main" tabIndex={-1}>
        {ready && <div className="page-heading"><div><h1>{titles[view]}</h1><p className="subtitle">{descriptions[view]}</p></div><div className="page-controls"><span className="status-pill" data-global-monitoring={ready && !monitoring.unavailable ? monitoring.status?.state : undefined}><span className="status-dot" aria-hidden="true" />{!ready ? 'Monitoring stopped' : monitoring.unavailable ? 'Monitoring status unavailable' : monitoring.status ? `Monitoring ${monitoring.status.state}` : 'Loading monitoring status…'}</span></div></div>}
        {scaleError && <p role="alert" className="operation-error">Display size could not be saved. Try again.</p>}
        <section hidden={ready && view !== 'settings'} inert={ready && view !== 'settings'} className="startup-panel" data-storage={data?.storage ?? startup.kind} aria-labelledby="startup-heading" aria-busy={startup.kind === 'loading' || busy}>
          <div className="startup-content">{ready ? <h2 id="startup-heading" role="status">{message}</h2> : <h1 id="startup-heading" role="status">{message}</h1>}
            {!ready && <p>{startup.kind === 'unavailable' ? 'This browser view cannot access encrypted storage or connect an account. Open the desktop application to continue.'
              : startup.kind === 'failed' ? 'Restart the desktop application, then try again. Your existing database stays unchanged.'
              : recovery ? recoveryMessage
              : needsPassphrase && data?.storage === 'setup_required' ? 'Your desktop key store is unavailable. An app password protects saved account data.'
              : data?.storage === 'locked' ? 'Unlock to return to your saved accounts and readings.'
              : 'Keep your accounts and usage together, on this device.'}</p>}
            {(startup.kind === 'failed' || startup.kind === 'unavailable') && <Button className="primary-button" onClick={() => { setStartup({ kind: 'loading' }); setAttempt(n => n + 1); }}>Check desktop connection</Button>}
            {startup.kind === 'desktop' && data?.storage === 'setup_required' && !needsPassphrase && <Button className="primary-button" disabled={busy} onClick={() => { void updateWorkspace(() => invoke('setup_workspace', { passphrase: null })); }}>{busy ? 'Creating workspace…' : 'Create private workspace'}</Button>}
            {startup.kind === 'desktop' && !recovery && needsPassphrase && <form className="unlock-form" onSubmit={event => {
              event.preventDefault();
              if (data?.storage === 'setup_required' && passphrase !== confirmation) {
                setOperationError('The passwords do not match.');
                setWorkspaceErrorField('workspace-confirmation');
                return;
              }
              void updateWorkspace(() => data?.storage === 'locked'
                ? invoke('unlock_workspace', { passphrase })
                : invoke('setup_workspace', { passphrase }));
            }}>
              <p id="passphrase-help">{data?.storage === 'locked'
                ? 'Enter the app password you chose for this device.'
                : 'Use at least 12 characters. You will need this password when you reopen OpenUsage.'}</p>
              <Field><FieldLabel htmlFor="workspace-passphrase">App password</FieldLabel>
              <Input nativeInput id="workspace-passphrase" name="workspace-passphrase" type="password" autoComplete={data?.storage === 'locked' ? 'current-password' : 'new-password'} required maxLength={1024} aria-invalid={!!operationError && workspaceErrorField === 'workspace-passphrase'} aria-describedby={`passphrase-help${operationError && workspaceErrorField === 'workspace-passphrase' ? ' workspace-error' : ''}`} disabled={busy} value={passphrase} onChange={event => { setPassphrase(event.target.value); setOperationError(null); }} /></Field>
              {data?.storage === 'setup_required' && <Field><FieldLabel htmlFor="workspace-confirmation">Confirm app password</FieldLabel><Input nativeInput id="workspace-confirmation" name="workspace-confirmation" type="password" autoComplete="new-password" required maxLength={1024} aria-invalid={!!operationError && workspaceErrorField === 'workspace-confirmation'} aria-describedby={operationError && workspaceErrorField === 'workspace-confirmation' ? 'workspace-error' : undefined} disabled={busy} value={confirmation} onChange={event => { setConfirmation(event.target.value); setOperationError(null); }} /></Field>}
              <Button className="primary-button" type="submit" disabled={busy}>{busy ? 'Opening workspace…' : data?.storage === 'locked' ? 'Unlock workspace' : 'Create private workspace'}</Button>
            </form>}
            {data?.storage === 'locked' && !needsPassphrase && <p className="development-note">Unlock your system key store, then check the workspace again.</p>}
            {(recovery || (data?.storage === 'locked' && !needsPassphrase)) && <Button className="primary-button" disabled={busy} onClick={() => { setOperationError(null); setStartup({ kind: 'loading' }); setAttempt(n => n + 1); }}>Check workspace</Button>}
            {ready && data?.keyProtection === 'os_store' && <p className="protection-note">Encrypted storage · Key protected by your system key store</p>}
            {ready && data?.keyProtection === 'passphrase' && <p className="protection-note">Encrypted storage · Key protected by your app password</p>}
            {operationError && <p id="workspace-error" className="operation-error" role="alert">{operationError}</p>}
          </div>
        </section>
        {(!ready || view === 'settings') && <Button variant="ghost" className={`text-button ${ready ? 'settings-privacy' : 'onboarding-privacy'}`} onClick={() => setPrivacy(!privacy)} aria-expanded={privacy} aria-controls="privacy-details">{privacy ? 'Hide privacy details' : 'About your privacy'}</Button>}
        {privacy && (!ready || view === 'settings') && <section className="privacy-details" id="privacy-details" aria-labelledby="privacy-heading"><h2 id="privacy-heading">Privacy and local storage</h2><p>OpenUsage has no hosted account or telemetry. Account connections require your approval. Provider credentials stay with Codex.</p><p>Encrypted storage protects application records. It does not encrypt provider credential files or operating-system swap.</p><p>Codex handles approved sign-in requests, connection checks, and readings. Monitoring runs only while OpenUsage is open and your workspace is unlocked.</p></section>}
        {ready && view === 'overview' && connectionError && !accountPanelOpen && <div className="account-refresh-feedback" role="alert" data-overview-connection-error><p>{connectionError}</p><Button variant="outline" onClick={() => navigate('settings')}>Connection details</Button><Button variant="ghost" onClick={() => setConnectionError(null)}>Dismiss</Button></div>}
        {ready && view === 'overview' && accountMessage && <div className="account-refresh-feedback" role={accountMessage.outcome === 'failed' ? 'alert' : 'status'} data-account-refresh-message><p>{profiles.find(profile => profile.id === accountMessage.profileId)?.label ?? 'Account'}: {accountMessage.outcome === 'saved' ? 'Readings updated.' : 'Could not finish refreshing. Saved readings are still available.'}</p>{accountMessage.outcome === 'failed' && <Button variant="outline" onClick={() => requestAccountAction({ kind: 'manage', profileId: accountMessage.profileId })}>View account details</Button>}<Button variant="ghost" aria-label="Dismiss account update" onClick={() => setAccountMessage(null)}>Dismiss</Button></div>}
        {ready && <section data-workspace-view="overview" hidden={view !== 'overview'} inert={view !== 'overview'} aria-label="Overview"><Overview key={profileRevision} active={view === 'overview'} profiles={profiles} revision={readingRevision} onAccounts={() => { setAccountPanelProfile(null); setAccountPanelOpen(true); }} onAccountAction={requestAccountAction} busy={busy} refreshingProfileIds={refreshingProfiles} refreshWaitSeconds={refreshWaitSeconds} /></section>}
        {ready && <div hidden={view !== 'settings'} inert={view !== 'settings'}><ProviderSettings busy={busy} setBusy={setBusy} approval={providerApproval} onApproved={setProviderApproval} /></div>}
        {ready && <dialog ref={accountPanel} data-account-panel aria-modal="true" data-workspace-view="accounts" hidden={!accountPanelOpen} aria-labelledby="account-panel-heading" onCancel={event => { event.preventDefault(); setAccountPanelOpen(false); }} onClose={() => setAccountPanelOpen(false)}>
          <div className="account-panel-heading"><h2 id="account-panel-heading">{profiles.find(profile => profile.id === accountPanelProfile)?.label ?? 'Manage accounts'}</h2><Button data-account-panel-close variant="ghost" aria-label="Close account details" onClick={() => setAccountPanelOpen(false)}>Close</Button></div>
          {connectionPreparing && <p role="status">Preparing OpenAI sign-in…</p>}
          {connectionError && <div role="alert"><p>{connectionError}</p><Button type="button" variant="outline" disabled={busy} onClick={() => setConnectionRequest(value => value + 1)}>Retry connection</Button><Button type="button" variant="ghost" onClick={() => navigate('settings')}>Connection details</Button></div>}
          <ProfileManager key={profileRevision} embedded selectedProfileId={accountPanelProfile} onSelectProfile={setAccountPanelProfile} onAccountOperation={reportAccountOperation} onRefreshState={refreshState} busy={busy} setBusy={setBusy} onCount={updateProfileCount} requestedAction={accountAction} onActionHandled={handledAccountAction} providerApproval={providerApproval} onProfilesChange={updateProfiles} onReadingsChanged={readingsChanged} monitoring={monitoring.status} monitoringUnavailable={monitoring.unavailable} onMonitoringAction={monitoring.action} onConnectAccount={() => setConnectionRequest(value => value + 1)} />
        </dialog>}
        {ready && <section data-workspace-view="usage" hidden={view !== 'usage'} inert={view !== 'usage'} aria-label="Saved usage"><Usage requestedProfileId={usageProfile} active={view === 'usage'} profiles={profiles} revision={readingRevision} onAccounts={() => { setAccountPanelProfile(null); setAccountPanelOpen(true); }} /></section>}
        <section data-workspace-view="settings" hidden={!ready || view !== 'settings'} inert={!ready || view !== 'settings'} aria-label="Workspace settings">

        {ready && <section className="account-settings"><h2>Accounts</h2><p>Rename accounts or restore an archived profile.</p><Button data-account-manager-open variant="outline" onClick={() => { setAccountPanelProfile(null); setAccountPanelOpen(true); }}>Manage accounts</Button></section>}
        {ready && <section className="backup-panel" aria-labelledby="backup-heading">
          <div className="section-heading"><div><h2 id="backup-heading">Encrypted backups</h2><p>Save a password-protected copy of your application records.</p></div><div className="backup-actions"><Button ref={backupToggle} variant="outline" className="secondary-button" disabled={busy} onClick={() => toggleBackup('create')} aria-expanded={backupOpen} aria-controls="backup-form">{backupOpen ? 'Hide backup form' : 'Create a backup'}</Button><Button variant="outline" className="secondary-button" disabled={busy} onClick={() => toggleBackup('inspect')} aria-expanded={backupAction === 'inspect'} aria-controls="backup-inspection">{backupAction === 'inspect' ? 'Hide backup inspection' : 'Inspect a backup'}</Button><Button variant="outline" className="secondary-button" disabled={busy} onClick={() => toggleBackup('recover')} aria-expanded={backupAction === 'recover'} aria-controls="checkpoint-recovery">{backupAction === 'recover' ? 'Hide saved copies' : 'Undo a restore'}</Button></div></div>
          {backupOpen && <form id="backup-form" className="unlock-form backup-form" onSubmit={event => { event.preventDefault(); void saveBackup(); }}>
            <p id="backup-records-help">Application records can include account names, email addresses, and activity. Provider credentials and local evidence are excluded.</p>
            <Field disabled={busy}><FieldLabel className="backup-selection"><Checkbox nativeButton render={<button type="button" />} id="backup-records" name="backup-records" required checked={backupRecords} disabled={busy} aria-describedby="backup-records-help" onCheckedChange={checked => setBackupRecords(checked === true)} />Application records</FieldLabel></Field>
            <Field><FieldLabel htmlFor="backup-password">Backup password</FieldLabel><Input nativeInput id="backup-password" name="backup-password" type="password" autoComplete="new-password" required minLength={12} maxLength={1024} disabled={busy} aria-invalid={!!backupError && backupErrorField === 'backup-password'} aria-describedby={`backup-password-help${backupError && backupErrorField === 'backup-password' ? ' backup-error' : ''}`} value={backupPassword} onChange={event => { setBackupPassword(event.target.value); setBackupError(null); }} /></Field>
            <Field><FieldLabel htmlFor="backup-confirmation">Confirm backup password</FieldLabel><Input nativeInput id="backup-confirmation" name="backup-confirmation" type="password" autoComplete="new-password" required minLength={12} maxLength={1024} disabled={busy} aria-invalid={!!backupError && backupErrorField === 'backup-confirmation'} aria-describedby={backupError && backupErrorField === 'backup-confirmation' ? 'backup-error' : undefined} value={backupConfirmation} onChange={event => { setBackupConfirmation(event.target.value); setBackupError(null); }} /></Field>
            <p id="backup-password-help">Use at least 12 characters. Keep this password. OpenUsage cannot recover it.</p>
            <p>The encrypted file will save in Downloads/OpenUsage.</p>
            <Button className="primary-button" type="submit" disabled={busy || !backupRecords}>{busy ? 'Encrypting backup…' : 'Save encrypted backup'}</Button>
            {backupError && <p id="backup-error" className="operation-error" role="alert">{backupError}</p>}
          </form>}
          {backupReceipt && <div className="backup-receipt" role="status"><p>Backup saved and authenticated.</p><p><strong>{backupReceipt.fileName}</strong><br />{backupReceipt.directory}</p></div>}
          {backupAction === 'inspect' && <section id="backup-inspection" className="backup-inspection" aria-labelledby="backup-inspection-heading"><h3 id="backup-inspection-heading">Inspect a backup</h3><BackupInspection busy={busy} setBusy={setBusy} onRestored={refreshRestoredWorkspace} /></section>}
          {backupAction === 'recover' && <CheckpointRecovery busy={busy} setBusy={setBusy} onRestored={refreshRestoredWorkspace} />}
          <p className="backup-note">{backupAction === 'recover'
            ? 'Undoing a restore requires a preview and your confirmation.'
            : 'Inspect a backup before you prepare a restore. Replacing records requires confirmation.'}</p>
        </section>}
        {ready && view === 'settings' && <ResetNotifications profiles={profiles} revision={String(profileRevision)} busy={busy} setBusy={setBusy} />}
        {ready && <AlertHistory revision={`${profileRevision}:${readingRevision}`} onViewUsage={profileId => requestAccountAction({ kind: 'usage', profileId })} onViewAccount={profileId => requestAccountAction({ kind: 'manage', profileId })} />}
        {ready && <DataExport key={profileRevision} profiles={profiles} busy={busy} setBusy={setBusy} />}
        </section>
      </main>
    </div>
  </div>;
}

type WorkspaceView = 'overview' | 'usage' | 'settings';
function NavIcon({ view }: { view: WorkspaceView }) {
  const paths = {
    overview: <><path d="m3 10 9-7 9 7v10a1 1 0 0 1-1 1h-5v-7H9v7H4a1 1 0 0 1-1-1Z" /></>,
    usage: <path d="M4 20v-6m5 6V9m6 11V4m5 16V11" />,
    settings: <><path d="m9 3-1 3-3 1-2 4 2 2v4l4 3 3-1 3 1 4-3v-4l2-2-2-4-3-1-1-3Z" /><circle cx="12" cy="12" r="3" /></>,
  };
  return <svg className="nav-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">{paths[view]}</svg>;
}

createRoot(document.getElementById('root')!).render(<StrictMode><App /></StrictMode>);
