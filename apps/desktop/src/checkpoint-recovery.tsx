import { useEffect, useRef, useState } from 'react';
import { Button } from './components/ui/button';
import { Field, FieldLabel, FieldDescription } from './components/ui/field';
import { Select, SelectTrigger, SelectValue, SelectPopup, SelectItem } from './components/ui/select';
import { invoke, type RestoreCheckpointList } from './contracts';
import { CheckpointRestorePreviewPanel } from './backup-restore';

type Props = { busy: boolean; setBusy: (busy: boolean) => void; onRestored: () => Promise<void> };

function checkpointDate(value: string | null): string {
  return value === null ? 'Date unavailable' : new Intl.DateTimeFormat('en-US', {
    dateStyle: 'medium', timeStyle: 'short', timeZone: 'UTC',
  }).format(new Date(value)) + ' UTC';
}

export function CheckpointRecovery({ busy, setBusy, onRestored }: Props) {
  const [list, setList] = useState<RestoreCheckpointList | null>(null);
  const [selected, setSelected] = useState('');
  const [loading, setLoading] = useState(true);
  const [attempt, setAttempt] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const [focus, setFocus] = useState<'select' | 'error' | null>(null);
  const select = useRef<HTMLButtonElement>(null);
  const refresh = useRef<HTMLButtonElement>(null);
  const errorPanel = useRef<HTMLParagraphElement>(null);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);
    void invoke('list_restore_checkpoints').then(next => {
      if (cancelled) return;
      setList(next);
      setSelected(current => next.checkpoints.some(item => item.id === current)
        ? current : next.checkpoints[0]?.id ?? '');
      if (attempt > 0) setFocus('select');
    }).catch(() => {
      if (cancelled) return;
      setList(null);
      setSelected('');
      setError('OpenUsage could not read the checkpoint list. Keep your workspace files and refresh the list.');
      setFocus('error');
    }).finally(() => { if (!cancelled) setLoading(false); });
    return () => { cancelled = true; };
  }, [attempt]);

  useEffect(() => {
    if (busy || loading || !focus) return;
    const target = focus === 'error' ? errorPanel.current : select.current ?? refresh.current;
    target?.focus();
    setFocus(null);
  }, [busy, loading, focus]);

  const checkpoint = list?.checkpoints.find(item => item.id === selected);
  return <section id="checkpoint-recovery" className="backup-inspection" aria-labelledby="checkpoint-recovery-title" aria-busy={loading}>
    <h3 id="checkpoint-recovery-title">Restore a checkpoint</h3>
    <p className="restore-help">Checkpoints contain application records saved before a restore preview. They use this workspace key. A preview checks their records and original profile homes before you can approve replacement.</p>
    <div className="checkpoint-catalog">
      <Button ref={refresh} type="button" variant="outline" className="secondary-button" disabled={busy || loading} onClick={() => setAttempt(value => value + 1)}>{loading ? 'Reading checkpoints…' : 'Refresh checkpoints'}</Button>
      {loading && <p role="status">Reading local checkpoint files.</p>}
      {!loading && list?.checkpoints.length === 0 && <p role="status">No recovery checkpoints are available in this workspace.</p>}
      {!loading && checkpoint && <>
        <Field className="w-full" disabled={busy}>
          <FieldLabel nativeLabel={false} render={<div />}>Checkpoint file</FieldLabel>
          <Select value={selected} disabled={busy} items={list?.checkpoints.map(item => ({ value: item.id, label: `${checkpointDate(item.modifiedAt)} · ${item.id}` }))} onValueChange={next => {
            if (typeof next === 'string' && list?.checkpoints.some(item => item.id === next)) setSelected(next);
          }}>
            <SelectTrigger ref={select} id="restore-checkpoint" style={{ width: '100%' }} aria-describedby="checkpoint-file-help"><SelectValue /></SelectTrigger>
            <SelectPopup alignItemWithTrigger={false}>
              {list?.checkpoints.map(item => <SelectItem key={item.id} value={item.id} data-option-value={item.id}>{checkpointDate(item.modifiedAt)} · {item.id}</SelectItem>)}
            </SelectPopup>
          </Select>
          <FieldDescription id="checkpoint-file-help" className="restore-help">Dates come from file metadata. The list does not verify records or identify the last applied restore.</FieldDescription>
        </Field>
        <dl className="backup-inspection-metadata"><dt>Checkpoint ID</dt><dd><bdi>{checkpoint.id}</bdi></dd><dt>File size</dt><dd>{checkpoint.fileBytes} bytes</dd></dl>
      </>}
      {list?.truncated && <p role="status">This list is limited. Some checkpoint files are not shown.</p>}
      {error && <p ref={errorPanel} className="operation-error" role="alert" tabIndex={-1}>{error}</p>}
    </div>
    {!loading && checkpoint && <CheckpointRestorePreviewPanel key={`${selected}:${attempt}`} checkpointId={selected} busy={busy} setBusy={setBusy} onRestored={onRestored} />}
  </section>;
}
