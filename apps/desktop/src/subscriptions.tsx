import { useEffect, useId, useRef, useState, type ReactNode } from 'react';
import { Button } from './components/ui/button';
import { Collapsible, CollapsiblePanel, CollapsibleTrigger } from './components/ui/collapsible';
import { Field, FieldLabel } from './components/ui/field';
import { Input } from './components/ui/input';
import { Select, SelectItem, SelectPopup, SelectTrigger, SelectValue } from './components/ui/select';
import { invoke, type ProfileRecord, type SubscriptionInput, type SubscriptionRecord, type SubscriptionsView } from './contracts';

type Props = { profiles: ProfileRecord[]; revision: string; busy: boolean; setBusy: (value: boolean) => void };
type Draft = Omit<SubscriptionInput, 'intervalCount'> & { intervalCount: string };
const intervals = { unknown: 'Not entered', day: 'Day', week: 'Week', month: 'Month', year: 'Year', once: 'Once' };

function failure(error: unknown): string {
  if (error === 'invalid_subscription') return 'Check the plan, price, currency, interval and dates, then save again.';
  if (error === 'subscription_changed') return 'This record changed. Reload records before you edit it again.';
  if (error === 'workspace_locked') return 'Unlock your workspace before you change subscription records.';
  return 'OpenUsage could not load or save these records. Reload records, then try again.';
}

function Choice({ label, value, items, disabled, onChange, name }: {
  label: string; value: string; items: Record<string, string>; disabled: boolean; onChange: (value: string) => void; name?: string;
}) {
  const id = useId();
  return <Field disabled={disabled}>
    <FieldLabel nativeLabel={false} render={<div id={`${id}-label`} />}>{label}</FieldLabel>
    <Select modal={false} value={value} items={items} disabled={disabled} onValueChange={next => { if (next !== null) onChange(next); }}>
      <SelectTrigger id={id} aria-labelledby={`${id}-label`} data-subscription-select={name}><SelectValue /></SelectTrigger>
      <SelectPopup alignItemWithTrigger={false}>{Object.entries(items).map(([key, text]) => <SelectItem key={key} value={key}>{text}</SelectItem>)}</SelectPopup>
    </Select>
  </Field>;
}

export function Subscriptions({ profiles, revision, busy, setBusy }: Props) {
  const [open, setOpen] = useState(false);
  const [selected, setSelected] = useState('');
  const profile = profiles.find(item => item.id === selected) ?? profiles.find(item => !item.archived) ?? profiles[0];
  return <section className="backup-panel subscriptions-panel" aria-label="Subscriptions" data-subscriptions>
    <Collapsible open={open} onOpenChange={setOpen}>
      <h2><CollapsibleTrigger render={<Button variant="outline" />} type="button" data-subscriptions-toggle>Subscriptions</CollapsibleTrigger></h2>
      <CollapsiblePanel>
        {open && <>
          <p>Local records you confirm. They do not prove payment or change a provider subscription.</p>
          {!profile ? <p>Add an account before you create a subscription record.</p> : <>
            <Choice label="Account" name="account" value={profile.id} items={Object.fromEntries(profiles.map(item => [item.id, `${item.label}${item.archived ? ' (archived)' : ''}`]))} disabled={busy} onChange={setSelected} />
            <AccountSubscriptions key={`${profile.id}:${revision}`} profile={profile} busy={busy} setBusy={setBusy} />
          </>}
        </>}
      </CollapsiblePanel>
    </Collapsible>
  </section>;
}

function AccountSubscriptions({ profile, busy, setBusy }: Pick<Props, 'busy' | 'setBusy'> & { profile: ProfileRecord }) {
  const id = useId();
  const [view, setView] = useState<SubscriptionsView | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [draft, setDraft] = useState<Draft | null>(null);
  const [editing, setEditing] = useState<SubscriptionRecord | null>(null);
  const [deleting, setDeleting] = useState<SubscriptionRecord | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [focusResult, setFocusResult] = useState(false);
  const mounted = useRef(false);
  const request = useRef(0);
  const pending = useRef(false);
  const ownsBusy = useRef(false);
  const add = useRef<HTMLButtonElement>(null);
  const plan = useRef<HTMLInputElement>(null);
  const result = useRef<HTMLParagraphElement>(null);
  const dialog = useRef<HTMLDialogElement>(null);
  const cancelDelete = useRef<HTMLButtonElement>(null);
  const deleteTrigger = useRef<HTMLButtonElement | null>(null);
  const disabled = busy || loading;

  useEffect(() => {
    mounted.current = true;
    void load(null);
    return () => {
      mounted.current = false; request.current += 1;
      if (ownsBusy.current) { ownsBusy.current = false; setBusy(false); }
    };
    // Each account and workspace revision owns a separate mounted instance.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  useEffect(() => { if (draft) plan.current?.focus(); }, [draft !== null]);
  useEffect(() => {
    const node = dialog.current;
    if (!node) return;
    if (deleting && !node.open) { node.showModal(); cancelDelete.current?.focus(); }
    else if (!deleting && node.open) { node.close(); deleteTrigger.current?.focus(); }
  }, [deleting]);
  useEffect(() => {
    if (!focusResult || busy || loading) return;
    result.current?.focus(); setFocusResult(false);
  }, [focusResult, busy, loading, error, notice, deleting]);

  async function load(afterId: string | null) {
    const epoch = ++request.current;
    setLoading(true); setError(null);
    try {
      const value = await invoke('list_subscriptions', { profileId: profile.id, afterId });
      if (!mounted.current || request.current !== epoch) return;
      if (value.profileId !== profile.id) throw new Error('Wrong account');
      setView(previous => afterId && previous ? { ...value, records: [...previous.records, ...value.records] } : value);
    } catch (error: unknown) {
      if (mounted.current && request.current === epoch) { setError(failure(error)); setFocusResult(true); }
    } finally {
      if (mounted.current && request.current === epoch) setLoading(false);
    }
  }

  async function change(action: () => Promise<unknown>, message: string) {
    if (disabled || pending.current) return;
    pending.current = true; ownsBusy.current = true;
    setBusy(true); setSaving(true); setError(null); setNotice(null);
    try {
      await action();
      if (!mounted.current) return;
      setDraft(null); setEditing(null); setDeleting(null);
      await load(null);
      if (!mounted.current) return;
      setNotice(message); setFocusResult(true);
    } catch (error: unknown) {
      if (mounted.current) { setError(failure(error)); setFocusResult(true); }
    } finally {
      if (mounted.current) {
        pending.current = false; setSaving(false);
        if (ownsBusy.current) { ownsBusy.current = false; setBusy(false); }
      }
    }
  }

  function edit(record: SubscriptionRecord | null) {
    setEditing(record); setError(null); setNotice(null);
    setDraft(record ? {
      plan: record.plan, tier: record.tier, currency: record.currency, price: record.price,
      intervalUnit: record.intervalUnit, intervalCount: record.intervalCount === null ? '' : String(record.intervalCount),
      startDate: record.startDate, renewalDate: record.renewalDate, endDate: record.endDate,
      cancellationDate: record.cancellationDate, autoRenew: record.autoRenew,
    } : {
      plan: '', tier: null, currency: view?.currencies.find(item => item.code === 'USD')?.code ?? view?.currencies[0]?.code ?? '',
      price: null, intervalUnit: null, intervalCount: '', startDate: null, renewalDate: null,
      endDate: null, cancellationDate: null, autoRenew: null,
    });
  }

  function save() {
    if (!draft) return;
    const input: SubscriptionInput = { ...draft, intervalCount: draft.intervalUnit && draft.intervalUnit !== 'once' ? Number(draft.intervalCount) : null };
    void change(() => editing
      ? invoke('update_subscription', { profileId: profile.id, subscriptionId: editing.id, expectedRevision: editing.revision, input })
      : invoke('create_subscription', { profileId: profile.id, input }), 'Subscription record saved.');
  }

  function dismissDelete() {
    if (pending.current) return;
    setDeleting(null); setError(null);
  }

  function textField(label: string, key: 'plan' | 'price' | 'tier' | 'startDate' | 'renewalDate' | 'endDate' | 'cancellationDate', type = 'text'): ReactNode {
    if (!draft) return null;
    return <Field disabled={disabled} key={key}><FieldLabel htmlFor={`${id}-${key}`}>{label}</FieldLabel>
      <Input nativeInput ref={key === 'plan' ? plan : undefined} id={`${id}-${key}`} name={key} type={type} required={key === 'plan'} maxLength={key === 'plan' || key === 'tier' ? 100 : key === 'price' ? 32 : undefined} inputMode={key === 'price' ? 'decimal' : undefined} disabled={disabled} value={draft[key] ?? ''} aria-describedby={key === 'price' ? `${id}-price-help` : undefined} onChange={event => setDraft({ ...draft, [key]: event.target.value || (key === 'plan' ? '' : null) })} />
    </Field>;
  }

  return <div data-subscription-account={profile.id} aria-busy={loading || saving}>
    <div className="profile-actions">
      <Button ref={add} variant="outline" disabled={disabled || !view || draft !== null} data-subscription-add onClick={() => edit(null)}>Add subscription</Button>
      <Button variant="ghost" disabled={disabled} data-subscriptions-reload onClick={() => { setDraft(null); setEditing(null); setNotice(null); void load(null); }}>Reload records</Button>
    </div>
    {loading && <p role="status">Loading subscription records…</p>}
    {view && view.records.length === 0 && !loading && <p>No subscription records for this account.</p>}
    <div className="subscription-records">{view?.records.map(record => <article key={record.id} data-subscription-record={record.id} aria-label={record.plan}>
      <p><strong>{record.plan}</strong>{record.archived ? ' · Archived' : ''}<br />
        {record.price === null ? 'Price not entered' : `${record.currency} ${record.price}`}
        {record.intervalUnit ? ` · ${record.intervalUnit === 'once' ? 'Once' : `Every ${record.intervalCount} ${record.intervalUnit}${record.intervalCount === 1 ? '' : 's'}`}` : ''}
        {record.renewalDate && <><br />Renews <time dateTime={record.renewalDate}>{record.renewalDate}</time></>}
      </p>
      <div className="profile-actions">
        <Button variant="ghost" disabled={disabled || draft !== null} data-subscription-edit onClick={() => edit(record)}>Edit</Button>
        <Button variant="ghost" disabled={disabled || draft !== null} data-subscription-archive onClick={() => { void change(() => invoke('set_subscription_archived', { profileId: profile.id, subscriptionId: record.id, expectedRevision: record.revision, archived: !record.archived }), record.archived ? 'Subscription record unarchived.' : 'Subscription record archived.'); }}>{record.archived ? 'Unarchive' : 'Archive'}</Button>
        <Button variant="ghost" disabled={disabled || draft !== null} data-subscription-delete onClick={event => { deleteTrigger.current = event.currentTarget; setError(null); setDeleting(record); }}>Delete</Button>
      </div>
    </article>)}</div>
    {view?.nextCursor && <Button variant="outline" disabled={disabled || draft !== null} data-subscriptions-more onClick={() => { void load(view.nextCursor); }}>More records</Button>}
    {draft && <form className="unlock-form subscription-form" aria-label={editing ? 'Edit subscription' : 'Add subscription'} aria-busy={saving} onSubmit={event => { event.preventDefault(); save(); }}>
      <p>{editing ? 'Edit' : 'Add'} a local record for <strong>{profile.label}</strong>.</p>
      <div className="subscription-fields">
        {textField('Plan', 'plan')}
        {textField('Price', 'price')}
        <Choice label="Currency" name="currency" value={draft.currency} items={Object.fromEntries(view?.currencies.map(item => [item.code, item.code]) ?? [])} disabled={disabled} onChange={currency => setDraft({ ...draft, currency })} />
        <Choice label="Interval" name="interval" value={draft.intervalUnit ?? 'unknown'} items={intervals} disabled={disabled} onChange={value => setDraft({ ...draft, intervalUnit: value === 'unknown' ? null : value as SubscriptionInput['intervalUnit'], intervalCount: value === 'unknown' || value === 'once' ? '' : draft.intervalCount || '1' })} />
        {draft.intervalUnit && draft.intervalUnit !== 'once' && <Field disabled={disabled}><FieldLabel htmlFor={`${id}-count`}>Interval count</FieldLabel><Input nativeInput id={`${id}-count`} name="intervalCount" type="number" min={1} max={120} step={1} required disabled={disabled} value={draft.intervalCount} onChange={event => setDraft({ ...draft, intervalCount: event.target.value })} /></Field>}
        {textField('Renewal date', 'renewalDate', 'date')}
      </div>
      <p id={`${id}-price-help`}>Leave price blank when it is unknown.</p>
      <details data-subscription-details><summary>Dates and details</summary><div className="subscription-fields">
        {textField('Tier', 'tier')}{textField('Start date', 'startDate', 'date')}{textField('End date', 'endDate', 'date')}{textField('Cancellation deadline', 'cancellationDate', 'date')}
        <Choice label="Auto-renew" name="autoRenew" value={draft.autoRenew === null ? 'unknown' : draft.autoRenew ? 'yes' : 'no'} items={{ unknown: 'Not entered', yes: 'Yes', no: 'No' }} disabled={disabled} onChange={value => setDraft({ ...draft, autoRenew: value === 'unknown' ? null : value === 'yes' })} />
      </div></details>
      <div className="profile-actions"><Button type="submit" disabled={disabled} data-subscription-save>{saving ? 'Saving…' : 'Save record'}</Button><Button type="button" variant="outline" disabled={disabled} data-subscription-cancel onClick={() => { setDraft(null); setEditing(null); setError(null); add.current?.focus(); }}>Cancel</Button></div>
    </form>}
    {!deleting && (error || notice) && <p ref={result} role={error ? 'alert' : 'status'} className={error ? 'operation-error' : undefined} tabIndex={-1} data-subscriptions-result>{error ?? notice}</p>}
    <dialog ref={dialog} role="alertdialog" aria-modal="true" aria-labelledby={`${id}-delete-title`} aria-describedby={`${id}-delete-scope`} data-subscription-delete-dialog onCancel={event => { event.preventDefault(); event.stopPropagation(); dismissDelete(); }} onClose={event => { event.stopPropagation(); dismissDelete(); }} style={{ position: 'fixed', inset: 0, width: 'min(30rem, calc(100vw - 2rem))', maxHeight: 'calc(100dvh - 2rem)', margin: 'auto', padding: 'var(--space-5)', border: '1px solid var(--line)', borderRadius: 'var(--radius-panel)', background: 'var(--surface)', color: 'var(--text)', font: 'inherit', overflow: 'auto', overflowWrap: 'anywhere' }}>
      <h2 id={`${id}-delete-title`}>Delete {deleting?.plan}?</h2><p id={`${id}-delete-scope`}>Delete this local record permanently. This does not cancel a provider subscription.</p>
      <div className="profile-actions"><Button ref={cancelDelete} variant="outline" disabled={saving} data-subscription-delete-cancel onClick={dismissDelete}>Cancel</Button><Button variant="destructive" disabled={disabled || !deleting} data-subscription-delete-confirm onClick={() => { if (deleting) void change(() => invoke('delete_subscription', { profileId: profile.id, subscriptionId: deleting.id, expectedRevision: deleting.revision, confirmed: true }), 'Subscription record deleted.'); }}>{saving ? 'Deleting…' : 'Delete record'}</Button></div>
      {deleting && error && <p ref={result} className="operation-error" role="alert" tabIndex={-1} data-subscriptions-result>{error}</p>}
    </dialog>
  </div>;
}
