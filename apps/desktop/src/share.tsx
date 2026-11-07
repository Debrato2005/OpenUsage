import { useEffect, useRef, useState } from 'react';
import { Button } from './components/ui/button';
import { Checkbox } from './components/ui/checkbox';
import { invoke, type ShareImage, type ShareKind, type ShareReceipt } from './contracts';
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

/** A preview of each image and an explicit save, made on this device (OU-SHARE-001, OU-SHARE-003). */
function ShareDialog({ onClose }: { onClose: () => void }) {
  const dialog = useRef<HTMLDialogElement>(null);
  const [kind, setKind] = useState<ShareKind>('wrapped');
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
    void invoke('load_share_image', { kind, branding }).then(value => {
      if (!current) return;
      objectUrl = URL.createObjectURL(new Blob([value.svg], { type: 'image/svg+xml' }));
      setImage(value); setUrl(objectUrl);
    }).catch(() => { if (current) setError('OpenUsage could not make this image. Try again.'); });
    return () => { current = false; if (objectUrl) URL.revokeObjectURL(objectUrl); };
  }, [kind, branding]);
  async function save(format: 'svg' | 'png') {
    if (!image || !url) return;
    setBusy(true); setReceipt(null); setError(null);
    try {
      const bytes = format === 'png' ? await png(url, image.width, image.height) : null;
      setReceipt(await invoke('save_share_image', { kind, branding, format, png: bytes }));
    } catch {
      setError(format === 'png' ? 'OpenUsage could not save the PNG. Try the SVG.' : 'OpenUsage could not save the image. Try again.');
    } finally { setBusy(false); }
  }
  return <dialog ref={dialog} className="share-dialog" aria-labelledby="share-heading" onClose={onClose} data-share-dialog>
    <div className="share-head"><h2 id="share-heading">Share</h2><Button variant="ghost" size="icon" aria-label="Close" onClick={() => dialog.current?.close()}><CloseIcon /></Button></div>
    <div className="overview-chips" role="group" aria-label="Image">
      <Button variant="ghost" aria-pressed={kind === 'wrapped'} onClick={() => setKind('wrapped')}>Wrapped</Button>
      <Button variant="ghost" aria-pressed={kind === 'heatmap'} onClick={() => setKind('heatmap')}>Heatmap</Button>
    </div>
    <figure className="share-preview" data-share-preview={kind}>{url && image ? <img src={url} alt={image.summary} width={image.width} height={image.height} /> : !error && <p className="overview-dim" role="status">Loading…</p>}</figure>
    <label className="share-branding"><Checkbox nativeButton render={<button type="button" />} id="share-branding" checked={branding} disabled={busy} onCheckedChange={checked => setBranding(checked === true)} />Show the OpenUsage name and address</label>
    <div className="share-actions">
      <Button className="primary-button" disabled={busy || !image} onClick={() => { void save('png'); }}>Save PNG</Button>
      <Button variant="outline" disabled={busy || !image} onClick={() => { void save('svg'); }}>Save SVG</Button>
    </div>
    {receipt && <p className="share-receipt" role="status">Saved {receipt.fileName} in {receipt.directory}</p>}
    {error && <p className="operation-error" role="alert">{error}</p>}
  </dialog>;
}

export function ShareButton() {
  const [open, setOpen] = useState(false);
  return <>
    <Button variant="outline" onClick={() => setOpen(true)} data-share-open>Share</Button>
    {open && <ShareDialog onClose={() => setOpen(false)} />}
  </>;
}
