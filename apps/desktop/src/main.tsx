import { StrictMode, useEffect, useRef, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { DesktopUnavailable, invoke, type BackupReceipt, type Bootstrap, type Theme } from './contracts';
import './styles.css';

type Startup = { kind: 'loading' } | { kind: 'desktop'; data: Bootstrap }
  | { kind: 'unavailable' } | { kind: 'failed' };

function App() {
  const [startup, setStartup] = useState<Startup>({ kind: 'loading' });
  const [theme, setTheme] = useState<Theme>('system');
  const [attempt, setAttempt] = useState(0);
  const [privacy, setPrivacy] = useState(false);
  const [busy, setBusy] = useState(false);
  const [operationError, setOperationError] = useState<string | null>(null);
  const [workspaceErrorField, setWorkspaceErrorField] = useState<'workspace-passphrase' | 'workspace-confirmation' | null>(null);
  const [passphrase, setPassphrase] = useState('');
  const [confirmation, setConfirmation] = useState('');
  const [backupOpen, setBackupOpen] = useState(false);
  const [backupRecords, setBackupRecords] = useState(false);
  const [backupPassword, setBackupPassword] = useState('');
  const [backupConfirmation, setBackupConfirmation] = useState('');
  const [backupError, setBackupError] = useState<string | null>(null);
  const [backupErrorField, setBackupErrorField] = useState<'backup-password' | 'backup-confirmation' | null>(null);
  const [backupReceipt, setBackupReceipt] = useState<BackupReceipt | null>(null);
  const backupToggle = useRef<HTMLButtonElement>(null);

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
  useEffect(() => { if (backupReceipt) backupToggle.current?.focus(); }, [backupReceipt]);
  useEffect(() => {
    if (!busy && operationError && workspaceErrorField) document.getElementById(workspaceErrorField)?.focus();
  }, [busy, operationError, workspaceErrorField]);
  useEffect(() => {
    if (!busy && backupError && backupErrorField) document.getElementById(backupErrorField)?.focus();
  }, [busy, backupError, backupErrorField]);

  const data = startup.kind === 'desktop' ? startup.data : null;
  const ready = data?.storage === 'ready';
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
        invalid_passphrase: 'The passphrase could not unlock this workspace. Try again. Your database stays unchanged.',
        passphrase_too_short: 'Use a passphrase with at least 12 characters.',
        passphrase_too_long: 'Use a shorter passphrase.',
        passphrase_required: 'Set a passphrase to protect your workspace.',
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
      void updateWorkspace(() => invoke('set_theme', { theme: next }));
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
      setBackupOpen(false);
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
  const message = startup.kind === 'loading' ? 'Checking local storage…'
    : startup.kind === 'unavailable' ? 'OpenUsage needs its desktop application'
    : startup.kind === 'failed' ? 'OpenUsage could not read its local state'
    : recovery ? 'Your local data needs recovery'
    : ready ? 'Your workspace is ready' : data?.storage === 'locked' ? 'Unlock your local workspace' : 'Set up your private workspace';
  const recoveryMessage = data?.storageIssue === 'workspace_in_use'
    ? 'Another OpenUsage window uses this workspace. Close that window, then check the workspace again.'
    : data?.storageIssue === 'missing_database'
    ? 'The protected key exists, but its database is missing. Keep your database and key files. Recover your original encrypted database before you try again.'
    : 'OpenUsage cannot open the existing database. Keep your database and key files. Recover the original database and its protected key before you try again.';

  return <div className="app-shell">
    <a className="skip-link" href="#main">Skip to dashboard</a>
    <aside className="sidebar" aria-label="Workspace">
      <a className="brand" href="#main" aria-label="OpenUsage dashboard"><span className="brand-mark" aria-hidden="true">ou</span><span>OpenUsage</span></a>
      <nav aria-label="Main"><a className="nav-active" href="#main" aria-current="page">Overview</a></nav>
      <div className="sidebar-note"><button className="text-button" onClick={() => setPrivacy(!privacy)} aria-expanded={privacy} aria-controls="privacy-details">{privacy ? 'Hide privacy details' : 'About your privacy'}</button></div>
      <div className="sidebar-footer"><span>OpenUsage</span><span>0.1.0 · In development</span></div>
    </aside>
    <div className="workspace">
      <header className="topbar"><span>Workspace <span className="breadcrumb" aria-hidden="true">/</span> <strong>Overview</strong></span><div className="theme-control"><label htmlFor="theme">Appearance</label><select id="theme" value={theme} disabled={busy} onChange={e => changeTheme(e.target.value as Theme)}><option value="system">System</option><option value="light">Light</option><option value="dark">Dark</option></select></div></header>
      <main id="main" tabIndex={-1}>
        <div className="page-heading"><div><h1>Overview</h1><p className="subtitle">Account usage and activity on this device.</p></div><span className="status-pill"><span className="status-dot" aria-hidden="true" />Monitoring stopped</span></div>
        <section className="startup-panel" data-storage={data?.storage ?? startup.kind} aria-labelledby="startup-heading" aria-busy={startup.kind === 'loading' || busy}>
          <div className="startup-content"><h2 id="startup-heading" role="status">{message}</h2>
            <p>{startup.kind === 'unavailable' ? 'This browser view cannot access encrypted storage or connect an account. Open the desktop application to continue.'
              : startup.kind === 'failed' ? 'Restart the desktop application, then try again. Your existing database stays unchanged.'
              : recovery ? recoveryMessage
              : ready ? 'No accounts are connected. Usage readings require an authorized account connection.'
              : 'Create or unlock encrypted local storage to manage your accounts.'}</p>
            {(startup.kind === 'failed' || startup.kind === 'unavailable') && <button className="primary-button" onClick={() => { setStartup({ kind: 'loading' }); setAttempt(n => n + 1); }}>Check desktop connection</button>}
            {startup.kind === 'desktop' && data?.storage === 'setup_required' && !needsPassphrase && <button className="primary-button" disabled={busy} onClick={() => { void updateWorkspace(() => invoke('setup_workspace', { passphrase: null })); }}>{busy ? 'Creating workspace…' : 'Create private workspace'}</button>}
            {startup.kind === 'desktop' && !recovery && needsPassphrase && <form className="unlock-form" onSubmit={event => {
              event.preventDefault();
              if (data?.storage === 'setup_required' && passphrase !== confirmation) {
                setOperationError('The passphrases do not match.');
                setWorkspaceErrorField('workspace-confirmation');
                return;
              }
              void updateWorkspace(() => data?.storage === 'locked'
                ? invoke('unlock_workspace', { passphrase })
                : invoke('setup_workspace', { passphrase }));
            }}>
              <p id="passphrase-help">{data?.storage === 'locked'
                ? 'Enter your workspace passphrase. It stays on this device.'
                : 'Your system key store is unavailable. Choose a passphrase with at least 12 characters. You will need it each time you open OpenUsage.'}</p>
              <label htmlFor="workspace-passphrase">Workspace passphrase</label>
              <input id="workspace-passphrase" name="workspace-passphrase" type="password" autoComplete={data?.storage === 'locked' ? 'current-password' : 'new-password'} required maxLength={1024} aria-invalid={!!operationError && workspaceErrorField === 'workspace-passphrase'} aria-describedby={`passphrase-help${operationError && workspaceErrorField === 'workspace-passphrase' ? ' workspace-error' : ''}`} disabled={busy} value={passphrase} onChange={event => { setPassphrase(event.target.value); setOperationError(null); }} />
              {data?.storage === 'setup_required' && <><label htmlFor="workspace-confirmation">Confirm passphrase</label><input id="workspace-confirmation" name="workspace-confirmation" type="password" autoComplete="new-password" required maxLength={1024} aria-invalid={!!operationError && workspaceErrorField === 'workspace-confirmation'} aria-describedby={operationError && workspaceErrorField === 'workspace-confirmation' ? 'workspace-error' : undefined} disabled={busy} value={confirmation} onChange={event => { setConfirmation(event.target.value); setOperationError(null); }} /></>}
              <button className="primary-button" type="submit" disabled={busy}>{busy ? 'Opening workspace…' : data?.storage === 'locked' ? 'Unlock workspace' : 'Create private workspace'}</button>
            </form>}
            {data?.storage === 'locked' && !needsPassphrase && <p className="development-note">Unlock your system key store, then check the workspace again.</p>}
            {(recovery || (data?.storage === 'locked' && !needsPassphrase)) && <button className="primary-button" disabled={busy} onClick={() => { setOperationError(null); setStartup({ kind: 'loading' }); setAttempt(n => n + 1); }}>Check workspace</button>}
            {ready && data?.keyProtection === 'os_store' && <p className="protection-note">Encrypted storage · Key protected by your system key store</p>}
            {ready && data?.keyProtection === 'passphrase' && <p className="protection-note">Encrypted storage · Key protected by your workspace passphrase</p>}
            {operationError && <p id="workspace-error" className="operation-error" role="alert">{operationError}</p>}
          </div>
        </section>
        {privacy && <section className="privacy-details" id="privacy-details" aria-labelledby="privacy-heading"><h2 id="privacy-heading">Privacy and local storage</h2><p>OpenUsage has no hosted account or telemetry. Account connections require your approval. Provider credentials stay with Codex.</p><p>Encrypted storage protects application records. It does not encrypt provider credential files or operating-system swap.</p><p>This development build does not collect provider data.</p></section>}
        <section className="metric-grid" aria-label="Workspace summary">
          <Metric label="Active profiles" value={ready ? String(data.activeProfiles) : 'Locked'} detail={ready ? 'Of 10 available profile slots' : 'Unlock to view your profiles'} />
          <Metric label="Quota remaining" value="Not available" detail="No accepted provider reading" />
          <Metric label="Token activity" value="Not available" detail="No accepted provider reading" />
        </section>
        <section className="accounts-section" aria-labelledby="accounts-heading"><div className="section-heading"><div><h2 id="accounts-heading">Accounts</h2><p>Each profile uses a separate Codex environment.</p></div><span className="muted-tag">{ready ? `${data.activeProfiles} active` : 'Workspace locked'}</span></div>
          <div className="empty-state"><h3>{ready ? 'No accounts connected' : 'Unlock to view your accounts'}</h3><p>{ready ? 'Account connection is not available in this development build. No provider data has been collected.' : 'Open your local workspace to manage account profiles and view saved activity.'}</p></div>
        </section>
        {ready && <section className="backup-panel" aria-labelledby="backup-heading">
          <div className="section-heading"><div><h2 id="backup-heading">Encrypted backups</h2><p>Save a password-protected copy of your application records.</p></div><button ref={backupToggle} className="secondary-button" disabled={busy} onClick={() => { setBackupOpen(!backupOpen); setBackupPassword(''); setBackupConfirmation(''); setBackupError(null); }} aria-expanded={backupOpen} aria-controls="backup-form">{backupOpen ? 'Hide backup form' : 'Create a backup'}</button></div>
          {backupOpen && <form id="backup-form" className="unlock-form backup-form" onSubmit={event => { event.preventDefault(); void saveBackup(); }}>
            <p id="backup-records-help">Application records can include account names, email addresses, and activity. Provider credentials and local evidence are excluded.</p>
            <label className="backup-selection"><input id="backup-records" name="backup-records" type="checkbox" required checked={backupRecords} disabled={busy} aria-describedby="backup-records-help" onChange={event => setBackupRecords(event.target.checked)} />Application records</label>
            <label htmlFor="backup-password">Backup password</label><input id="backup-password" name="backup-password" type="password" autoComplete="new-password" required minLength={12} maxLength={1024} disabled={busy} aria-invalid={!!backupError && backupErrorField === 'backup-password'} aria-describedby={`backup-password-help${backupError && backupErrorField === 'backup-password' ? ' backup-error' : ''}`} value={backupPassword} onChange={event => { setBackupPassword(event.target.value); setBackupError(null); }} />
            <label htmlFor="backup-confirmation">Confirm backup password</label><input id="backup-confirmation" name="backup-confirmation" type="password" autoComplete="new-password" required minLength={12} maxLength={1024} disabled={busy} aria-invalid={!!backupError && backupErrorField === 'backup-confirmation'} aria-describedby={backupError && backupErrorField === 'backup-confirmation' ? 'backup-error' : undefined} value={backupConfirmation} onChange={event => { setBackupConfirmation(event.target.value); setBackupError(null); }} />
            <p id="backup-password-help">Use at least 12 characters. Keep this password. OpenUsage cannot recover it.</p>
            <p>The encrypted file will save in Downloads/OpenUsage.</p>
            <button className="primary-button" type="submit" disabled={busy || !backupRecords}>{busy ? 'Encrypting backup…' : 'Save encrypted backup'}</button>
            {backupError && <p id="backup-error" className="operation-error" role="alert">{backupError}</p>}
          </form>}
          {backupReceipt && <div className="backup-receipt" role="status"><p>Backup saved and authenticated.</p><p><strong>{backupReceipt.fileName}</strong><br />{backupReceipt.directory}</p></div>}
          <p className="backup-note">Restore is not available in this build.</p>
        </section>}
        <footer className="main-footer"><span>Provider data: Not available</span><span>Last observation: Not available</span></footer>
      </main>
    </div>
  </div>;
}

function Metric({ label, value, detail }: { label: string; value: string; detail: string }) {
  return <article className="metric"><div className="metric-label">{label}</div><p className="metric-value">{value}</p><p className="metric-detail">{detail}</p></article>;
}

createRoot(document.getElementById('root')!).render(<StrictMode><App /></StrictMode>);
