import { useEffect, useRef, useState, type ReactNode } from 'react';
import { createPortal } from 'react-dom';
import { useAccountSlot } from './account-slots';
import { Button } from './components/ui/button';
import { Collapsible, CollapsiblePanel, CollapsibleTrigger } from './components/ui/collapsible';
import { AssociationReview } from './association-review';
import { invoke, type ProfileActivityAssociationReview, type ProfileCollectionOutcome, type ProviderExecutableApproval } from './contracts';
import { providerFailure } from './provider-errors';

type Props = {
  automaticRequest?: number | null;
  actionContainerId?: string;
  connected?: boolean;
  profileId: string; approval: ProviderExecutableApproval; busy: boolean; retryAfterSeconds: number;
  onActivity: (active: boolean) => void; onStarted: () => void;
  onSaved: (value: ProfileCollectionOutcome) => Promise<void>; onFailed: () => Promise<void>;
  children: (action: ReactNode, cancelAction: ReactNode) => ReactNode;
};
type Outcome = { kind: 'error' | 'cancelled' | 'saved' | 'unavailable'; message: string; reassociationAvailable?: boolean };
function focusVisible(element: HTMLElement | null) {
  if (element?.getClientRects().length && !element.closest('[hidden]')) element.focus();
}

export function ProfileCollectionRead({ actionContainerId, connected = false, automaticRequest = null, profileId, approval, busy, retryAfterSeconds, onActivity, onStarted, onSaved, onFailed, children }: Props) {
  const actionContainer = useAccountSlot(actionContainerId);
  const [working, setWorking] = useState(false);
  const [stopping, setStopping] = useState(false);
  const [requestId, setRequestId] = useState<string | null>(null);
  const [outcome, setOutcome] = useState<Outcome | null>(null);
  const [review, setReview] = useState<ProfileActivityAssociationReview | null>(null);
  const [reviewOpen, setReviewOpen] = useState(false);
  const [reviewing, setReviewing] = useState(false);
  const [reviewError, setReviewError] = useState<string | null>(null);
  const [approved, setApproved] = useState(false);
  const [focus, setFocus] = useState(false);
  const result = useRef<HTMLDivElement>(null);
  const cancelButton = useRef<HTMLButtonElement>(null);
  const consent = useRef<HTMLButtonElement>(null);
  const reviewTrigger = useRef<HTMLButtonElement>(null);
  const alive = useRef(true);
  const pending = useRef(false);
  const attemptedAutomaticRequest = useRef<number | null>(null);
  const activeRequest = useRef<string | null>(null);
  const generation = useRef(0);
  const cancellationFailure = useRef<unknown>(null);
  const current = (operation: number) => alive.current && generation.current === operation;
  useEffect(() => {
    alive.current = true;
    return () => {
      alive.current = false; generation.current += 1;
      const request = activeRequest.current;
      if (request) void invoke('cancel_profile_collection_read', { profileId, requestId: request }).catch(() => {});
    };
  }, [profileId, approval.approvalId]);
  useEffect(() => { if (working && requestId) focusVisible(cancelButton.current); }, [working, requestId]);
  useEffect(() => { if (reviewOpen && review && !working) focusVisible(consent.current); }, [reviewOpen, review, working]);
  useEffect(() => { if (focus && !working) { focusVisible(result.current); setFocus(false); } }, [focus, working, outcome]);

  function closeReview() {
    generation.current += 1; setReviewOpen(false); setReview(null); setApproved(false); setReviewError(null); setReviewing(false);
  }
  async function reviewAssociation() {
    if (busy || retryAfterSeconds > 0 || pending.current || reviewing) return;
    const operation = ++generation.current;
    setReviewOpen(true); setReview(null); setApproved(false); setReviewError(null); setReviewing(true); onActivity(true);
    try {
      const value = await invoke('review_profile_collection_association', { profileId, approvalId: approval.approvalId });
      if (value.profileId !== profileId || value.approvalId !== approval.approvalId) throw new Error('Incompatible collection account review.');
      if (current(operation)) setReview(value);
    } catch (failure) {
      if (current(operation)) setReviewError(providerFailure(failure));
    } finally { if (current(operation)) { setReviewing(false); onActivity(false); } }
  }
  async function read(reassociate = false) {
    if (busy || retryAfterSeconds > 0 || pending.current || reviewing || (reassociate && (!outcome?.reassociationAvailable || !reviewOpen || !approved || !review))) return;
    const reviewId = review?.reviewId;
    closeReview(); const operation = ++generation.current;
    pending.current = true; cancellationFailure.current = null;
    setWorking(true); setOutcome(previous => previous?.kind === 'error' ? previous : null); onActivity(true); onStarted();
    try {
      const begun = reassociate
        ? await invoke('begin_profile_collection_reassociation', { profileId, approvalId: approval.approvalId, reviewId: reviewId!, confirmed: true })
        : await invoke('begin_profile_collection_read', { profileId, approvalId: approval.approvalId });
      if (begun.profileId !== profileId || begun.approvalId !== approval.approvalId || begun.sourceKind !== approval.sourceKind) throw new Error('Incompatible collection binding.');
      if (!current(operation)) { await invoke('cancel_profile_collection_read', { profileId, requestId: begun.requestId }); return; }
      activeRequest.current = begun.requestId; setRequestId(begun.requestId);
      const value = await invoke('complete_profile_collection_read', { profileId, requestId: begun.requestId });
      if (!current(operation)) return;
      if (value.profileId !== profileId || value.requestId !== begun.requestId || value.approvalId !== approval.approvalId || value.sourceKind !== begun.sourceKind) throw new Error('Incompatible collection result.');
      await onSaved(value);
      if (!current(operation)) return;
      setOutcome(cancellationFailure.current ? { kind: 'error', message: providerFailure(cancellationFailure.current) } : {
        kind: value.accepted ? 'saved' : 'unavailable',
        message: !value.accepted ? 'Codex does not support quota or activity readings. Your saved readings are still available.'
          : !value.quota ? 'Activity saved. Codex does not support quota readings. Previous quotas remain available unless the sign-in changed.'
          : !value.activity ? 'Quotas saved. Codex does not support activity readings. Previous activity remains available unless the sign-in changed.'
          : 'Quota and activity readings saved. Identity has not been verified.',
      });
    } catch (failure) {
      if (current(operation)) {
        setOutcome(failure === 'provider_collection_cancelled' ? { kind: 'cancelled', message: providerFailure(failure) }
          : { kind: 'error', message: providerFailure(cancellationFailure.current ?? failure), reassociationAvailable: !cancellationFailure.current && failure === 'provider_collection_association_changed' });
        await onFailed();
      }
    } finally {
      if (current(operation)) { pending.current = false; activeRequest.current = null; setWorking(false); setStopping(false); setRequestId(null); setFocus(true); onActivity(false); }
    }
  }
  useEffect(() => {
    if (automaticRequest === null || attemptedAutomaticRequest.current === automaticRequest || busy || retryAfterSeconds > 0 || working || pending.current || reviewing) return;
    attemptedAutomaticRequest.current = automaticRequest;
    void read();
  }, [automaticRequest, busy, retryAfterSeconds, working, reviewing]);

  async function cancel() {
    if (!requestId || !pending.current || stopping) return;
    const operation = generation.current; setStopping(true);
    try {
      const value = await invoke('cancel_profile_collection_read', { profileId, requestId });
      if (!current(operation) || !pending.current || activeRequest.current !== requestId || !value.cancelled) return;
      generation.current += 1; pending.current = false; activeRequest.current = null;
      setWorking(false); setRequestId(null); setOutcome({ kind: 'cancelled', message: 'Reading refresh stopped.' }); setFocus(true); onActivity(false);
      await onFailed();
    } catch (failure) {
      if (current(operation) && pending.current && activeRequest.current === requestId) { cancellationFailure.current = failure; setOutcome({ kind: 'error', message: providerFailure(failure) }); }
    } finally { if (alive.current && (generation.current === operation || generation.current === operation + 1)) setStopping(false); }
  }
  const action = <Button type="button" className="primary-button" data-profile-connect={profileId} data-collection-read={profileId} aria-describedby={`profile-name-${profileId}`} disabled={busy || working || reviewing || retryAfterSeconds > 0} onClick={() => { void read(); }}>{working ? 'Refreshing readings…' : 'Refresh readings'}</Button>;
  const cancelAction = working && requestId ? <Button ref={cancelButton} type="button" variant="ghost" data-collection-cancel={profileId} disabled={stopping} onClick={() => { void cancel(); }}>{stopping ? 'Stopping refresh…' : 'Cancel refresh'}</Button> : null;
  const status = <>
    {working && <div className="profile-actions"><span role="status">Refreshing quota and activity readings…</span></div>}
    {outcome && <div ref={result} className="quota-result" data-collection-profile={profileId} role={outcome.kind === 'error' ? 'alert' : 'status'} tabIndex={-1}>
      {approval.sourceKind === 'test_fixture' && <p className="provider-fixture-note" data-provider-fixture>Synthetic test data</p>}
      <p>{outcome.message}</p>
      {outcome.reassociationAvailable && <Collapsible open={reviewOpen} onOpenChange={open => { if (open) void reviewAssociation(); else { closeReview(); onActivity(false); } }}>
        <CollapsibleTrigger ref={reviewTrigger} render={<Button variant="ghost" className="text-button h-auto max-w-full whitespace-normal" />} type="button" data-collection-reassociate={profileId} disabled={busy || working || reviewing || retryAfterSeconds > 0}>Use current sign-in</CollapsibleTrigger>
        <CollapsiblePanel><AssociationReview profileId={profileId} purpose="collection" review={review} reviewing={reviewing} error={reviewError} approved={approved} busy={busy || working || reviewing} consentRef={consent} onApproved={setApproved} onRetry={() => { void reviewAssociation(); }} onConfirm={() => { void read(true); }} onKeep={() => { closeReview(); onActivity(false); focusVisible(reviewTrigger.current); }} /></CollapsiblePanel>
      </Collapsible>}
    </div>}
  </>;
  return <div className="profile-collection-read">
    {children(actionContainerId ? null : action, actionContainerId ? null : cancelAction)}
    {actionContainer ? createPortal(<>{(connected || working || outcome) && <div className="profile-actions">{action}{cancelAction}</div>}{status}</>, actionContainer) : !actionContainerId && status}
  </div>;
}
