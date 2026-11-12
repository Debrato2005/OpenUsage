import { useEffect, useRef, useState } from 'react';
import { Button } from './components/ui/button';
import { Checkbox } from './components/ui/checkbox';
import { invoke, type ShareComponent, type ShareDetails, type ShareImage, type ShareKind, type ShareRange, type ShareReceipt } from './contracts';
import { CloseIcon } from './icons';
import './share.css';

// Draws the Rust-made SVG at twice its size. Rust checks the PNG before it saves it.
async function png(url: string, width: number, height: number) {
  const image = new Image();
  image.src = url;
  await image.decode();
  const canvas = document.createElement('canvas');
  canvas.width = width * 2;
  canvas.height = height * 2;
  canvas.getContext('2d')!.drawImage(image, 0, 0, canvas.width, canvas.height);
  const blob = await new Promise<Blob | null>(done => canvas.toBlob(done, 'image/png'));
  if (!blob) throw new Error('png');
  const bytes = new Uint8Array(await blob.arrayBuffer());
  let binary = '';
  for (let index = 0; index < bytes.length; index += 0x8000) binary += String.fromCharCode(...bytes.subarray(index, index + 0x8000));
  return btoa(binary);
}

const componentNames: Record<ShareComponent, string> = { inputTokens: 'Input tokens', outputTokens: 'Output tokens', cacheReadTokens: 'Cache read tokens', cacheWriteTokens: 'Cache write tokens', reasoningTokens: 'Reasoning tokens' };
const grouped = (value: string | null) => value === null ? 'Not available' : value.replace(/\B(?=(\d{3})+(?!\d))/g, ',');
function ReceiptDetails({ details }: { details: ShareDetails }) {
  return <details className="share-details" data-share-details>
    <summary>Details</summary>
    <dl>
      <dt>Period</dt><dd>{details.startDate} to {details.endDate} · {details.timeZone}</dd>
      <dt>Tokens</dt><dd>{grouped(details.totalTokens)}</dd>
      <dt>Estimated API value</dt><dd>{details.estimatedValueDisplay ?? 'Not available'}{details.estimatedValueDisplay !== null && !details.estimateComplete ? ' · Incomplete' : ''}</dd>
      <dt>Currency</dt><dd>USD</dd>
      <dt>Active days</dt><dd>{details.activeDays ?? 'Not available'}</dd>
      <dt>Pricing basis</dt><dd>{details.pricingBasis}</dd>
      <dt>Source scope</dt><dd>{details.sourceScope}</dd>
      <dt>Coverage</dt><dd>{details.savedDays} saved {details.savedDays === 1 ? 'day' : 'days'} · {details.missingDays} {details.missingDays === 1 ? 'day' : 'days'} without saved usage{details.logsGone ? ' · A later scan reported less' : ''}</dd>
      <dt>Unavailable components</dt><dd>{details.totalTokens === null ? 'Tokens, active days, and estimated API value' : details.missingComponents.length ? details.missingComponents.map(key => componentNames[key]).join(', ') : 'None'}</dd>
    </dl>
  </details>;
}

/** A preview of each image and an explicit save, made on this device (OU-SHARE-001, OU-SHARE-003). */
function ShareDialog({ onClose, scope }: { onClose: () => void; scope: string }) {
  const dialog = useRef<HTMLDialogElement>(null);
  const [kind, setKind] = useState<ShareKind>('wrapped');
  const [range, setRange] = useState<ShareRange>('year');
  const [branding, setBranding] = useState(true);
  const [image, setImage] = useState<ShareImage | null>(null);
  const [url, setUrl] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [receipt, setReceipt] = useState<ShareReceipt | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => { const current = dialog.current; if (current && !current.open) current.showModal(); }, []);
  useEffect(() => {
    let current = true;
    let objectUrl: string | null = null;
    setImage(null); setUrl(null); setReceipt(null); setError(null);
    void invoke('load_share_image', { kind, branding, ...(kind === 'wrapped' && range !== 'year' ? { range } : {}), ...(scope === 'combined' ? {} : { scope }) }).then(value => {
      if (!current) return;
      objectUrl = URL.createObjectURL(new Blob([value.svg], { type: 'image/svg+xml' }));
      setImage(value); setUrl(objectUrl);
    }).catch(() => { if (current) setError('OpenUsage could not make this image. Try again.'); });
    return () => { current = false; if (objectUrl) URL.revokeObjectURL(objectUrl); };
  }, [kind, branding, scope, range]);
  async function save(format: 'svg' | 'png') {
    if (!image || !url) return;
    setBusy(true); setReceipt(null); setError(null);
    try {
      const bytes = format === 'png' ? await png(url, image.width, image.height) : null;
      setReceipt(await invoke('save_share_image', { kind, branding, format, png: bytes, ...(kind === 'wrapped' && range !== 'year' ? { range } : {}), ...(scope === 'combined' ? {} : { scope }) }));
    } catch {
      setError(format === 'png' ? 'OpenUsage could not save the PNG. Try the SVG.' : 'OpenUsage could not save the image. Try again.');
    } finally { setBusy(false); }
  }
  return <dialog ref={dialog} className="share-dialog" aria-labelledby="share-heading" onClose={onClose} data-share-dialog>
    <div className="share-head"><h2 id="share-heading">Share</h2><Button variant="ghost" size="icon" aria-label="Close" onClick={() => dialog.current?.close()}><CloseIcon /></Button></div>
    <div className="overview-chips" role="group" aria-label="Image">
      <Button variant="ghost" disabled={busy} aria-pressed={kind === 'wrapped'} onClick={() => setKind('wrapped')}>Wrapped</Button>
      <Button variant="ghost" disabled={busy} aria-pressed={kind === 'heatmap'} onClick={() => setKind('heatmap')}>Heatmap</Button>
      <Button variant="ghost" disabled={busy} aria-pressed={kind === 'badge'} onClick={() => setKind('badge')}>Badge</Button>
    </div>
    {kind === 'wrapped' && <label className="share-period">Period<select data-share-period value={range} disabled={busy} onChange={event => setRange(event.target.value as ShareRange)}>
      <option value="year">Last 365 days</option><option value="7d">Last 7 days</option><option value="30d">Last 30 days</option><option value="month">This month</option>
    </select></label>}
    <figure className="share-preview" data-share-preview={kind} data-badge={kind === 'badge' || undefined}>{url && image ? <img src={url} alt={image.summary} width={image.width} height={image.height} /> : !error && <p className="overview-dim" role="status">Loading…</p>}</figure>
    {image?.details && <ReceiptDetails details={image.details} />}
    <label className="share-branding"><Checkbox nativeButton render={<button type="button" />} id="share-branding" checked={branding} disabled={busy} onCheckedChange={checked => setBranding(checked === true)} />Show the OpenUsage name and address</label>
    <div className="share-actions">
      {kind !== 'badge' && <Button className="primary-button" disabled={busy || !image} onClick={() => { void save('png'); }}>Save PNG</Button>}
      <Button variant={kind === 'badge' ? 'default' : 'outline'} className={kind === 'badge' ? 'primary-button' : undefined} disabled={busy || !image} onClick={() => { void save('svg'); }}>Save SVG</Button>
    </div>
    {receipt && <p className="share-receipt" role="status">Saved {receipt.fileName} in {receipt.directory}</p>}
    {error && <p className="operation-error" role="alert">{error}</p>}
  </dialog>;
}

export function ShareButton({ scope = 'combined' }: { scope?: string }) {
  const [open, setOpen] = useState(false);
  return <>
    <Button variant="outline" onClick={() => setOpen(true)} data-share-open>Share</Button>
    {open && <ShareDialog scope={scope} onClose={() => setOpen(false)} />}
  </>;
}
