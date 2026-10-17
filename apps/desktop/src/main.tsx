import { StrictMode, useEffect, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { DesktopUnavailable, invoke, type Bootstrap, type Theme } from './contracts';
import './styles.css';

type Startup = { kind: 'loading' } | { kind: 'desktop'; data: Bootstrap }
  | { kind: 'unavailable' } | { kind: 'failed' };

function App() {
  const [startup, setStartup] = useState<Startup>({ kind: 'loading' });
  const [theme, setTheme] = useState<Theme>('system');
  const [attempt, setAttempt] = useState(0);
  const [privacy, setPrivacy] = useState(false);

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

  const data = startup.kind === 'desktop' ? startup.data : null;
  const ready = data?.storage === 'ready';
  const recovery = data?.storage === 'recovery_required';
  const message = startup.kind === 'loading' ? 'Checking local storage…'
    : startup.kind === 'unavailable' ? 'OpenUsage needs its desktop application'
    : startup.kind === 'failed' ? 'OpenUsage could not read its local state'
    : recovery ? 'Your local data needs recovery'
    : ready ? 'Your workspace is ready' : data?.storage === 'locked' ? 'Unlock your local workspace' : 'Set up your private workspace';

  return <div className="app-shell">
    <a className="skip-link" href="#main">Skip to dashboard</a>
    <aside className="sidebar" aria-label="Workspace">
      <a className="brand" href="#main" aria-label="OpenUsage dashboard"><span className="brand-mark" aria-hidden="true">o<span>u</span></span><span>OpenUsage<span className="brand-caption">YOUR LOCAL WORKSPACE</span></span></a>
      <div className="workspace-label">WORKSPACE</div>
      <nav aria-label="Main"><a className="nav-active" href="#main" aria-current="page"><span aria-hidden="true">◫</span> Overview</a></nav>
      <div className="sidebar-note"><span className="privacy-dot" aria-hidden="true" />Local by design<p>Your workspace stays on this device. You control what connects.</p><button className="text-button" onClick={() => setPrivacy(!privacy)} aria-expanded={privacy} aria-controls="privacy-details">{privacy ? 'Hide privacy details' : 'About your privacy'} <span aria-hidden="true">↗</span></button></div>
      <div className="sidebar-footer"><span>OpenUsage</span><span>0.1.0 · In development</span></div>
    </aside>
    <div className="workspace">
      <header className="topbar"><span>Workspace <span className="breadcrumb" aria-hidden="true">/</span> <strong>Overview</strong></span><div className="theme-control"><label htmlFor="theme">Appearance</label><select id="theme" value={theme} onChange={e => setTheme(e.target.value as Theme)}><option value="system">System</option><option value="light">Light</option><option value="dark">Dark</option></select></div></header>
      <main id="main" tabIndex={-1}>
        <div className="page-heading"><div><p className="eyebrow">A CLEARER VIEW OF YOUR USAGE</p><h1>Workspace overview<span className="heading-dot">.</span></h1><p className="subtitle">Your accounts, usage, and history. In one private place.</p></div><span className="status-pill"><span aria-hidden="true">○</span> Monitoring stopped</span></div>
        <section className="startup-panel" aria-labelledby="startup-heading" aria-busy={startup.kind === 'loading'}>
          <div className="panel-icon" aria-hidden="true">⌑</div>
          <div className="startup-content"><p className="eyebrow">{ready ? 'LOCAL WORKSPACE' : recovery ? 'RECOVERY REQUIRED' : 'GET STARTED'}</p><h2 id="startup-heading" role="status">{message}</h2>
            <p>{startup.kind === 'unavailable' ? 'This browser view cannot access encrypted storage or connect an account. Open the desktop application to continue.'
              : startup.kind === 'failed' ? 'Restart the desktop application, then try again. Your existing database stays unchanged.'
              : recovery ? 'OpenUsage cannot open the existing database. Keep your database and key files. Restore access to your key store or use your recovery passphrase.'
              : ready ? 'No accounts are connected. Provider readings appear only after an authorized account connection.'
              : 'Your usage history belongs in encrypted local storage. Monitoring stays stopped until the workspace is unlocked.'}</p>
            {(startup.kind === 'failed' || startup.kind === 'unavailable') && <button className="primary-button" onClick={() => { setStartup({ kind: 'loading' }); setAttempt(n => n + 1); }}>Check desktop connection <span aria-hidden="true">↗</span></button>}
            {startup.kind === 'desktop' && !ready && !recovery && <p className="development-note">Storage setup is not available in this development build.</p>}
          </div>
          <div className="panel-aside"><span className="step-number">01</span><span>PRIVATE BY DEFAULT</span><p>No telemetry.<br />No shared sessions.<br />No guessed readings.</p></div>
        </section>
        {privacy && <section className="privacy-details" id="privacy-details" aria-labelledby="privacy-heading"><h2 id="privacy-heading">Your data stays yours</h2><p>OpenUsage has no hosted account or telemetry. Account connections require your approval. Provider credentials stay with Codex.</p><p>Encrypted storage protects application records. It does not encrypt provider credential files or operating-system swap.</p><p>This development build does not collect provider data.</p></section>}
        <section className="metric-grid" aria-label="Workspace summary">
          <Metric label="Active profiles" value={ready ? String(data.activeProfiles) : 'Locked'} detail={ready ? 'Of 10 available profile slots' : 'Unlock to view your profiles'} icon="◉" />
          <Metric label="Quota remaining" value="Not available" detail="No accepted provider reading" icon="◴" />
          <Metric label="Token activity" value="Not available" detail="No accepted provider reading" icon="▥" />
        </section>
        <section className="accounts-section" aria-labelledby="accounts-heading"><div className="section-heading"><div><h2 id="accounts-heading">Your accounts</h2><p>Separate sessions. Clear attribution.</p></div><span className="muted-tag">{ready ? `${data.activeProfiles} active` : 'Workspace locked'}</span></div>
          <div className="empty-state"><div className="empty-art" aria-hidden="true"><span>◉</span><span>+</span><span>◉</span></div><h3>{ready ? 'A fresh start for your accounts' : 'Your workspace starts here'}</h3><p>{ready ? 'Account connection is not available in this development build.' : 'Unlock local storage before you connect an account. Each account will use its own approved Codex environment.'}</p><div className="empty-footnote"><span aria-hidden="true">⌁</span> No account data collected</div></div>
        </section>
        <footer className="main-footer"><span>Built around your privacy.</span><span>Provider data: Not available <span aria-hidden="true">·</span> Last observation: Not available</span></footer>
      </main>
    </div>
  </div>;
}

function Metric({ label, value, detail, icon }: { label: string; value: string; detail: string; icon: string }) {
  return <article className="metric"><div className="metric-label">{label}<span aria-hidden="true">{icon}</span></div><p className="metric-value">{value}</p><p className="metric-detail">{detail}</p></article>;
}

createRoot(document.getElementById('root')!).render(<StrictMode><App /></StrictMode>);
