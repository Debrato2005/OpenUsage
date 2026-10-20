import { Button } from './components/ui/button';
import { Collapsible, CollapsibleTrigger, CollapsiblePanel } from './components/ui/collapsible';
import './quota-disclosure.css';
import { Meter, MeterIndicator, MeterLabel, MeterTrack, MeterValue } from './components/ui/meter';
import { useEffect, useRef, useState, type ReactNode } from 'react';
import { invoke, type ProviderExecutableApproval, type ProfileReadingReference, type SavedProfileQuota as SavedQuota, type ProfileActivityAssociationReview, type SavedQuotaWindow } from './contracts';
import { AssociationReview } from './association-review';
import { providerFailure } from './provider-errors';

type Props = {
  profileId: string;
  profileLabel: string;
  approval: ProviderExecutableApproval;
  busy: boolean;
  onActivity: (active: boolean) => void;
  onStarted: () => void;
  actionSiblings?: ReactNode;
  renderActions?: (action: ReactNode, cancelAction: ReactNode) => ReactNode;
  retryAfterSeconds?: number;
  onSaved: (value: ProfileReadingReference) => Promise<void>;
  onFailed: () => Promise<void>;
};
type Outcome = { kind: 'error' | 'cancelled'; message: string; reassociationAvailable?: boolean };
const dateFormat = new Intl.DateTimeFormat('en-US', { timeZone: 'UTC', year: 'numeric', month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit', timeZoneName: 'short' });
function date(value: string) { return dateFormat.format(new Date(value)); }
function QuotaDisclosure({ label, className, defaultOpen = false, children }: { label: ReactNode; className: string; defaultOpen?: boolean; children: ReactNode }) {
  return <Collapsible className={className} defaultOpen={defaultOpen}>
    <CollapsibleTrigger render={<Button variant="ghost" className="quota-disclosure-trigger" />} type="button">
      <svg className="quota-disclosure-caret" aria-hidden="true" focusable="false" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5"><path d="m6 4 4 4-4 4" /></svg>
      <span className="quota-disclosure-label">{label}</span>
    </CollapsibleTrigger>
    <CollapsiblePanel>{children}</CollapsiblePanel>
  </Collapsible>;
}
function CreditTime({ seconds }: { seconds: number }) {
  const value = new Date(seconds * 1000).toISOString();
  return <time dateTime={value}>{date(value)}</time>;
}
function CreditDetails({ rows }: { rows: SavedQuota['resetCreditDetails'] }) {
  if (rows === null) return <p data-credit-detail-state="unavailable">Credit details were not supplied.</p>;
  if (rows.length === 0) return <p data-credit-detail-state="empty">No credit rows were returned.</p>;
  const statuses = { available: 'Available', redeeming: 'Being redeemed', redeemed: 'Redeemed', unknown: 'Unknown' };
  return <>
    <ul className="quota-credit-details" role="list" aria-label="Credit details">{rows.map((row, index) => <li key={index} data-credit-detail>
      <p className="quota-credit-title"><bdi>{row.title ?? 'Title not supplied'}</bdi></p>
      <dl className="backup-inspection-metadata"><dt>Status</dt><dd>{statuses[row.status]}</dd><dt>Granted</dt><dd><CreditTime seconds={row.grantedAtUnix} /></dd><dt>Expires</dt><dd>{row.expiresAtUnix === null ? 'No expiry' : <CreditTime seconds={row.expiresAtUnix} />}</dd></dl>
    </li>)}</ul>
    <p>These rows may not include every credit. The count above is separate.</p>
  </>;
}
function WindowValues({ value }: { value: SavedQuotaWindow }) {
  const reset = value.resetsAtUnix === null ? null : new Date(value.resetsAtUnix * 1000).toISOString();
  const resetStatus = { expected: null, due: 'Reset due; awaiting confirmation', unconfirmed: 'Reset status not confirmed', confirmed: 'Reset confirmed' }[value.resetState];
  return <div className="quota-window">
    <h4>{value.label}</h4>
    <Meter min={0} max={100} value={value.derivedRemainingPercent} aria-label={`${value.label}: ${value.derivedRemainingPercent}% remaining (derived)`} aria-valuetext={`${value.derivedRemainingPercent}% remaining (derived)`}>
      <div className="quota-meter-heading"><MeterLabel><span className="sr-only">{value.label}: </span>Remaining (derived)</MeterLabel><MeterValue>{(_formatted, remaining) => `${remaining}%`}</MeterValue></div>
      <MeterTrack><MeterIndicator /></MeterTrack>
    </Meter>
    <dl className="quota-values"><div><dt>Used</dt><dd>{value.usedPercent}%</dd></div><div><dt>Expected reset</dt><dd>{reset ? <time dateTime={reset}>{date(reset)}</time> : 'Not available'}{resetStatus && <p className="provider-help" data-quota-reset-state={value.resetState} data-quota-reset-due={value.resetState === 'due' ? '' : undefined}>{resetStatus}</p>}</dd></div></dl>
  </div>;
}

function QuotaValues({ value }: { value: SavedQuota }) {
  return <>
        {value.limits.length === 0 ? <p>Codex returned no quota limits for this profile. Missing readings do not mean zero usage.</p> : value.limits.map((limit, index) => <QuotaDisclosure className="quota-limit" defaultOpen={index === 0} key={`${value.collectionRunId}:${limit.limitId}`} label={<bdi>{limit.limitName ?? limit.limitId}</bdi>}>
          <div className="quota-windows">{limit.primary && <WindowValues value={limit.primary} />}{limit.secondary && <WindowValues value={limit.secondary} />}</div>
          {!limit.primary && !limit.secondary && <p>No quota windows were supplied for this limit.</p>}
          {(limit.primary === null || limit.secondary === null) && (limit.primary || limit.secondary) && <p className="provider-help">{limit.primary === null ? 'Primary window' : 'Secondary window'}: Not available</p>}
          <QuotaDisclosure className="quota-source-details" label="Reading details"><dl className="backup-inspection-metadata"><dt>Provider limit ID</dt><dd><bdi>{limit.limitId}</bdi></dd><dt>Provider status</dt><dd><bdi>{limit.reachedType ?? 'Not supplied'}</bdi></dd><dt>Monthly credit limit</dt><dd className="quota-credit-count" data-monthly-credit-limit={limit.limitId} tabIndex={0} aria-label={`Monthly credit limit for ${limit.limitName ?? limit.limitId}`}><bdi className="whitespace-pre">{limit.monthlyCreditLimit ?? 'Not supplied'}</bdi></dd><dt>Spend control</dt><dd data-spend-control={limit.limitId}>{limit.spendControlReached === null ? 'Not supplied' : limit.spendControlReached ? 'Reached' : 'Not reached'}</dd></dl></QuotaDisclosure>
        </QuotaDisclosure>)}
        <QuotaDisclosure className="quota-preview-notes" label="About these readings"><p>These readings are saved locally and excluded from combined totals.</p><p>Remaining quota is calculated from reported usage. Expected reset times do not confirm that a reset occurred.</p><dl className="backup-inspection-metadata"><dt>Earned reset credits</dt><dd className="quota-credit-count" data-earned-reset-credits tabIndex={0} aria-label="Earned reset credits"><bdi>{value.availableResetCredits ?? 'Not supplied'}</bdi></dd></dl><p>Earned reset credits are separate from subscription payments and OpenAI Platform API billing.</p><CreditDetails rows={value.resetCreditDetails} /><p>Account quota. Application scope and coverage are unknown.</p></QuotaDisclosure>
  </>;
}

// Cache reads do not require executable approval or execute the provider.
export function SavedProfileQuota({ profileId, profileLabel, children, readingRevision }: { profileId: string; profileLabel: string; readingRevision?: string | undefined; children: (onSaved: (value: ProfileReadingReference, focusResult?: boolean) => Promise<void>, onFailed: () => Promise<void>) => ReactNode }) {
  const [value, setValue] = useState<SavedQuota | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [focus, setFocus] = useState(false);
  const result = useRef<HTMLDivElement>(null);
  const alive = useRef(true);
  const loadGeneration = useRef(0);
  const manualLoad = useRef(false);
  const latestReadingRevision = useRef(readingRevision);
  useEffect(() => {
    const previous = latestReadingRevision.current;
    latestReadingRevision.current = readingRevision;
    if (previous && readingRevision && previous !== readingRevision) void load();
  }, [readingRevision]);
  useEffect(() => {
    alive.current = true; void load();
    const interval = window.setInterval(() => { void load(); }, 60_000);
    return () => { alive.current = false; loadGeneration.current += 1; window.clearInterval(interval); };
  }, [profileId]);
  useEffect(() => { if (focus && result.current) { if (!result.current.closest('[hidden]') && result.current.getClientRects().length) result.current.focus(); setFocus(false); } }, [focus, value, error]);
  async function load(committed?: ProfileReadingReference, focusResult = false) {
    if (!committed && !focusResult && manualLoad.current) return;
    if (committed) manualLoad.current = true;
    const operation = ++loadGeneration.current; setLoading(true);
    try {
      const next = await invoke('load_profile_quota', { profileId });
      if (next && next.profileId !== profileId) throw new Error('Incompatible saved quota profile.');
      if (committed && (!next || next.bindingId !== committed.bindingId || next.collectionRunId !== committed.collectionRunId)) throw new Error('Incompatible saved quota observation.');
      if (alive.current && operation === loadGeneration.current) { setValue(next); setError(null); }
    } catch {
      if (alive.current && operation === loadGeneration.current) {
        setValue(current => current && { ...current, freshness: 'unknown' });
        setError(providerFailure('provider_quota_load_failed'));
      }
    } finally {
      if (committed) manualLoad.current = false;
      if (alive.current && operation === loadGeneration.current) { setLoading(false); if (focusResult) setFocus(true); }
    }
  }
  const freshnessLabels = { unknown: 'Freshness unknown', fresh: 'Current', stale: 'Stale' };
  return <>
    {children((receipt, focusResult = true) => load(receipt, focusResult), () => load())}
    {(value || error) && <div ref={result} className="quota-result" data-quota-saved={profileId} role="region" aria-label={`Saved quotas for ${profileLabel}`} tabIndex={-1}>
      <div className="quota-preview-heading"><h3>Saved quotas</h3>{value?.sourceKind === 'test_fixture' && <span className="quota-fixture-label" data-provider-fixture>Synthetic test data</span>}</div>
      {error && <div><p role="alert" data-quota-cache-error>{error}</p><Button type="button" variant="outline" disabled={loading} onClick={() => { void load(undefined, true); }}>Reload saved quotas</Button></div>}
      {value && <>
        <div className="quota-preview-meta"><p>Unverified identity · Saved locally</p><p className="quota-observed"><span data-quota-freshness={value.freshness}>{freshnessLabels[value.freshness]}</span> · Last read <time dateTime={value.observedAtUtc}>{date(value.observedAtUtc)}</time></p></div>
        <QuotaValues key={`${value.bindingId}:${value.collectionRunId}`} value={value} />
      </>}
    </div>}
  </>;
}

export function ProfileQuotaRead({ profileId, approval, busy, onActivity, onStarted, actionSiblings, renderActions, onSaved, onFailed, retryAfterSeconds = 0 }: Props) {
  const [associationReview, setAssociationReview] = useState<ProfileActivityAssociationReview | null>(null);
  const [reassociationOpen, setReassociationOpen] = useState(false);
  const [reassociationApproved, setReassociationApproved] = useState(false);
  const [reviewing, setReviewing] = useState(false);
  const [reviewError, setReviewError] = useState<string | null>(null);
  const reassociationConsent = useRef<HTMLButtonElement>(null);
  const reassociationTrigger = useRef<HTMLButtonElement>(null);
  const [working, setWorking] = useState(false);
  const [stopping, setStopping] = useState(false);
  const [requestId, setRequestId] = useState<string | null>(null);
  const [outcome, setOutcome] = useState<Outcome | null>(null);
  const [focus, setFocus] = useState(false);
  const result = useRef<HTMLDivElement>(null);
  const cancelButton = useRef<HTMLButtonElement>(null);
  const alive = useRef(true);
  const pending = useRef(false);
  const stopped = useRef(false);
  const cancellationFailure = useRef<unknown>(null);
  const activeRequest = useRef<string | null>(null);
  const generation = useRef(0);
  useEffect(() => {
    alive.current = true;
    return () => {
      alive.current = false;
      const requestId = activeRequest.current;
      if (requestId) void invoke('cancel_profile_quota_read', { profileId, requestId }).catch(() => {});
    };
  }, [profileId]);
  useEffect(() => { if (working && requestId) cancelButton.current?.focus(); }, [working, requestId]);
  useEffect(() => { if (outcome && !working) setFocus(true); }, [outcome, working]);
  useEffect(() => { if (focus && !working && result.current) { if (!result.current.closest('[hidden]') && result.current.getClientRects().length) result.current.focus(); setFocus(false); } }, [focus, working, outcome]);

  useEffect(() => { if (reassociationOpen && associationReview && !working) reassociationConsent.current?.focus(); }, [reassociationOpen, associationReview, working]);
  function closeReview() {
    generation.current += 1; setReassociationOpen(false); setReassociationApproved(false); setAssociationReview(null); setReviewError(null); setReviewing(false);
  }
  async function reviewAssociation() {
    if (busy || retryAfterSeconds > 0 || pending.current || reviewing) return;
    const operation = ++generation.current;
    setAssociationReview(null); setReassociationApproved(false); setReviewError(null); setReviewing(true); setReassociationOpen(true);
    try {
      const review = await invoke('review_profile_quota_association', { profileId, approvalId: approval.approvalId });
      if (review.profileId !== profileId || review.approvalId !== approval.approvalId) throw new Error('Incompatible association review.');
      if (alive.current && operation === generation.current) setAssociationReview(review);
    } catch (failure) {
      if (alive.current && operation === generation.current) setReviewError(failure === 'provider_quota_review_unavailable' ? 'Read quotas again to review the current sign-in.' : 'The account review could not be loaded. Try again.');
    } finally { if (alive.current && operation === generation.current) setReviewing(false); }
  }
  async function read(reassociate = false) {
    if (busy || retryAfterSeconds > 0 || pending.current || reviewing || (reassociate && (!outcome?.reassociationAvailable || !reassociationOpen || !reassociationApproved || !associationReview))) return;
    const reviewId = associationReview?.reviewId;
    closeReview();
    generation.current += 1;
    pending.current = true;
    stopped.current = false;
    cancellationFailure.current = null;
    setWorking(true); setOutcome(current => current?.kind === 'error' ? current : null); onActivity(true); onStarted();
    try {
      const begun = reassociate
        ? await invoke('begin_profile_quota_reassociation', { profileId, approvalId: approval.approvalId, reviewId: reviewId!, confirmed: true })
        : await invoke('begin_profile_quota_read', { profileId, approvalId: approval.approvalId });
      if (begun.profileId !== profileId || begun.approvalId !== approval.approvalId || begun.sourceKind !== approval.sourceKind) throw new Error('Incompatible quota binding.');
      activeRequest.current = begun.requestId;
      if (!alive.current) { await invoke('cancel_profile_quota_read', { profileId, requestId: begun.requestId }); return; }
      setRequestId(begun.requestId);
      const value = await invoke('complete_profile_quota_read', { profileId, requestId: begun.requestId });
      if (value.profileId !== profileId || value.requestId !== begun.requestId || value.approvalId !== approval.approvalId || value.sourceKind !== begun.sourceKind) throw new Error('Incompatible quota result.');
      if (alive.current) { setOutcome(null); await onSaved(value); if (alive.current && cancellationFailure.current) setOutcome({ kind: 'error', message: providerFailure(cancellationFailure.current) }); }
    } catch (failure) {
      const cancelled = failure === 'provider_quota_cancelled' || (stopped.current && failure === 'provider_quota_not_found');
      if (alive.current) {
        setOutcome(!cancellationFailure.current && cancelled ? { kind: 'cancelled', message: 'Quota read stopped.' } : { kind: 'error', message: providerFailure(cancellationFailure.current ?? failure), reassociationAvailable: !cancellationFailure.current && failure === 'provider_quota_association_changed' });
        void onFailed();
      }
    } finally {
      pending.current = false; activeRequest.current = null;
      if (alive.current) { setWorking(false); setStopping(false); setRequestId(null); onActivity(false); }
    }
  }

  async function cancel() {
    if (!requestId || !pending.current || stopping) return;
    const operation = generation.current;
    stopped.current = true; setStopping(true);
    try { await invoke('cancel_profile_quota_read', { profileId, requestId }); }
    catch (failure) {
      if (alive.current && generation.current === operation) { cancellationFailure.current = failure; setOutcome({ kind: 'error', message: providerFailure(failure) }); setFocus(true); }
    } finally { if (alive.current && generation.current === operation) setStopping(false); }
  }

  const action = <Button type="button" variant="outline" className="secondary-button" data-quota-read={profileId} aria-describedby={`profile-name-${profileId}`} disabled={busy || working || retryAfterSeconds > 0} onClick={() => { void read(); }}>{working ? 'Reading quotas…' : 'Read quotas'}</Button>;
  const cancelAction = working && requestId ? <Button ref={cancelButton} type="button" variant="ghost" className="text-button" data-quota-cancel={profileId} aria-describedby={`profile-name-${profileId}`} disabled={stopping} onClick={() => { void cancel(); }}>{stopping ? 'Stopping read…' : 'Cancel read'}</Button> : null;
  return <div className="profile-quota-read">
    {renderActions ? renderActions(action, cancelAction) : <div className="profile-actions">{action}{cancelAction}{actionSiblings}</div>}
    {outcome && <div ref={result} className="quota-result" data-quota-profile={profileId} role={outcome.kind === 'error' ? 'alert' : 'status'} tabIndex={-1}>
      {approval.sourceKind === 'test_fixture' && <p className="quota-fixture-label" data-provider-fixture>Synthetic test data</p>}
      <p>{outcome.message}</p>
      {outcome.reassociationAvailable && <Collapsible open={reassociationOpen} onOpenChange={open => { if (open) void reviewAssociation(); else closeReview(); }}>
        <CollapsibleTrigger ref={reassociationTrigger} render={<Button variant="ghost" className="text-button h-auto max-w-full whitespace-normal" />} type="button" data-quota-reassociate={profileId} disabled={busy || working || retryAfterSeconds > 0}>Use current sign-in</CollapsibleTrigger>
        <CollapsiblePanel><AssociationReview profileId={profileId} purpose="quota" review={associationReview} reviewing={reviewing} error={reviewError} approved={reassociationApproved} busy={busy || working} consentRef={reassociationConsent} onApproved={setReassociationApproved} onRetry={() => { void reviewAssociation(); }} onConfirm={() => { void read(true); }} onKeep={() => { closeReview(); reassociationTrigger.current?.focus(); }} /></CollapsiblePanel>
      </Collapsible>}
    </div>}
  </div>;
}
