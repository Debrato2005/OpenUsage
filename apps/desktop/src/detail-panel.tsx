import { createContext, useContext, useEffect, useRef, useState, type ReactNode } from 'react';
import { Button } from './components/ui/button';
import { CloseIcon } from './icons';
import './detail-panel.css';

// One short word on an abnormal number. Normal numbers carry no mark.
export function Mark({ children }: { children: ReactNode }) {
  return <span className="mark" data-mark>{children}</span>;
}

type Props = { open: boolean; title: ReactNode; label: string; onClose: () => void; children: ReactNode };

type Detail = { title: ReactNode; label: string; content: ReactNode };
const OpenDetail = createContext<(detail: Detail) => void>(() => {});
const InsideDetail = createContext(false);

// Reading links share a host and the existing panel. The content is the saved observation
// selected by the user; actions with live results keep their state inside that content.
export function DetailProvider({ children }: { children: ReactNode }) {
  const [detail, setDetail] = useState<Detail | null>(null);
  return <OpenDetail.Provider value={setDetail}>{children}<DetailPanel open={detail !== null} title={detail?.title} label={detail?.label ?? 'Reading details'} onClose={() => setDetail(null)}>{detail?.content}</DetailPanel></OpenDetail.Provider>;
}

export function useOpenDetail() { return useContext(OpenDetail); }

export function DetailLink({ label, children, details, className = '' }: { label: string; children: ReactNode; details: ReactNode; className?: string }) {
  const open = useOpenDetail();
  const inside = useContext(InsideDetail);
  return inside ? <span className={className}>{children}</span> : <button type="button" className={`detail-link ${className}`} aria-label={label} aria-haspopup="dialog" onClick={() => open({ label, title: label, content: details })}>{children}</button>;
}

// The one place for source, time, scope, coverage, derivation and reasons (OU-DES-007, OU-DES-010).
export function DetailPanel({ open, title, label, onClose, children }: Props) {
  const panel = useRef<HTMLDialogElement>(null);
  const returnFocus = useRef<HTMLElement | null>(null);
  useEffect(() => {
    const dialog = panel.current;
    if (!dialog) return;
    if (open && !dialog.open) {
      returnFocus.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
      dialog.showModal();
    } else if (!open && dialog.open) {
      dialog.close();
      if (returnFocus.current?.isConnected) returnFocus.current.focus({ preventScroll: true });
    }
  }, [open]);
  return <dialog ref={panel} className="detail-panel" aria-label={label} data-detail-panel onCancel={event => { event.preventDefault(); onClose(); }} onClick={event => { if (event.target === event.currentTarget) onClose(); }}>
    <div className="detail-panel-heading"><h2>{title}</h2><Button variant="ghost" className="icon-button" aria-label="Close" data-detail-panel-close onClick={onClose}><CloseIcon /></Button></div>
    {open && <InsideDetail.Provider value={true}><div className="detail-panel-body">{children}</div></InsideDetail.Provider>}
  </dialog>;
}
