import { useEffect, useRef, useState } from 'react';
import { Button } from './components/ui/button';
import { Checkbox } from './components/ui/checkbox';
import { invoke, type LocalLogTool, type LocalLogsView } from './contracts';
import './local-logs.css';

export const localLogToolNames: Record<LocalLogTool, string> = { claude: 'Claude Code', codex: 'Codex' };
const order: LocalLogTool[] = ['claude', 'codex'];
export const orderedTools = (tools: LocalLogTool[]) => order.filter(tool => tools.includes(tool));

/** Loads the local log view. A failure gives null, so no consent list appears. */
export function useLocalLogs(enabled: boolean) {
  const [view, setView] = useState<LocalLogsView | null>(null);
  useEffect(() => {
    if (!enabled) { setView(null); return; }
    let current = true;
    void invoke('load_local_logs').then(value => { if (current) setView(value); }).catch(() => { if (current) setView(null); });
    return () => { current = false; };
  }, [enabled]);
  return [view, setView] as const;
}

/** One checkbox for each tool (OU-LOG-001, OU-LOG-002). */
export function LocalLogChoice({ tools, selected, onChange, disabled }: {
  tools: LocalLogTool[]; selected: LocalLogTool[]; onChange: (next: LocalLogTool[]) => void; disabled: boolean;
}) {
  return <fieldset className="local-log-choice" disabled={disabled}>
    <legend>Read token counts from local logs</legend>
    {tools.map(tool => <label key={tool} className="local-log-tool">
      <Checkbox nativeButton render={<button type="button" />} id={`local-log-${tool}`} name={`local-log-${tool}`}
        checked={selected.includes(tool)} disabled={disabled}
        onCheckedChange={checked => onChange(orderedTools(checked === true ? [...selected, tool] : selected.filter(t => t !== tool)))} />
      <span>{localLogToolNames[tool]}</span>
    </label>)}
  </fieldset>;
}

/** The one consent step for a workspace that has no saved choice yet. */
export function LocalLogConsent({ view, busy, onSaved }: { view: LocalLogsView; busy: boolean; onSaved: (view: LocalLogsView) => void }) {
  const detected = orderedTools(view.tools.filter(t => t.detected).map(t => t.tool));
  const [selected, setSelected] = useState<LocalLogTool[]>(detected);
  const [saving, setSaving] = useState(false);
  const [failed, setFailed] = useState(false);
  return <section className="startup-panel local-log-consent" aria-labelledby="local-log-consent-heading">
    <div className="startup-content">
      <h1 id="local-log-consent-heading">Local logs</h1>
      <LocalLogChoice tools={detected} selected={selected} onChange={setSelected} disabled={busy || saving} />
      <Button className="primary-button" disabled={busy || saving} onClick={() => {
        setSaving(true); setFailed(false);
        void invoke('save_local_log_consent', { tools: selected }).then(onSaved).catch(() => setFailed(true)).finally(() => setSaving(false));
      }}>{saving ? 'Saving…' : 'Continue'}</Button>
      {failed && <p className="operation-error" role="alert">OpenUsage could not save this choice. Try again.</p>}
    </div>
  </section>;
}

/** The consent step for an existing workspace with no saved choice. Escape does not skip it. */
export function LocalLogConsentDialog(props: { view: LocalLogsView; busy: boolean; onSaved: (view: LocalLogsView) => void }) {
  const dialog = useRef<HTMLDialogElement>(null);
  useEffect(() => { const current = dialog.current; if (current && !current.open) current.showModal(); }, []);
  return <dialog ref={dialog} className="local-log-dialog" aria-labelledby="local-log-consent-heading" onCancel={event => event.preventDefault()}>
    <LocalLogConsent {...props} />
  </dialog>;
}

/** The Settings control that changes the choice (OU-LOG-002). */
export function LocalLogSettings({ busy, setBusy }: { busy: boolean; setBusy: (busy: boolean) => void }) {
  const [view, setView] = useLocalLogs(true);
  const granted = view ? orderedTools(view.tools.filter(t => t.granted).map(t => t.tool)) : [];
  const [selected, setSelected] = useState<LocalLogTool[] | null>(null);
  const [failed, setFailed] = useState(false);
  if (!view) return null;
  const choice = selected ?? granted;
  const changed = choice.join() !== granted.join();
  return <section className="local-log-settings" aria-labelledby="local-log-settings-heading">
    <h2 id="local-log-settings-heading">Local logs</h2>
    {view.tools.length === 0 ? <p>No Claude Code or Codex logs on this device</p> : <>
      <LocalLogChoice tools={view.tools.map(t => t.tool)} selected={choice} onChange={setSelected} disabled={busy} />
      <Button variant="outline" disabled={busy || !changed} onClick={() => {
        setBusy(true); setFailed(false);
        void invoke('save_local_log_consent', { tools: choice }).then(next => { setView(next); setSelected(null); })
          .catch(() => setFailed(true)).finally(() => setBusy(false));
      }}>Save</Button>
    </>}
    {view.scan.state === 'failed' && <p className="operation-error" role="alert">The last local log scan failed. Saved totals stay unchanged.</p>}
    {failed && <p className="operation-error" role="alert">OpenUsage could not save this choice. Try again.</p>}
  </section>;
}
