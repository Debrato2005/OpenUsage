import type { RefObject } from 'react';
import { Button } from './components/ui/button';
import { Checkbox } from './components/ui/checkbox';
import { Field, FieldLabel } from './components/ui/field';
import type { ProfileActivityAssociationReview } from './contracts';

type Props = { profileId: string; purpose: 'activity' | 'quota'; review: ProfileActivityAssociationReview | null; reviewing: boolean; error: string | null; approved: boolean; busy: boolean; consentRef: RefObject<HTMLButtonElement | null>; onApproved: (approved: boolean) => void; onRetry: () => void; onConfirm: () => void; onKeep: () => void };
export function AssociationReview({ profileId, purpose, review, reviewing, error, approved, busy, consentRef, onApproved, onRetry, onConfirm, onKeep }: Props) {
  const helpId = `${purpose}-reassociation-help-${profileId}`;
  return <>
    {reviewing && <p role="status">Loading account review…</p>}
    {error && <div><p role="alert">{error}</p><Button type="button" variant="outline" disabled={busy || reviewing} onClick={onRetry}>Retry account review</Button></div>}
    {review && <form onSubmit={event => { event.preventDefault(); onConfirm(); }}>
      <dl className="activity-summary">{(['previous', 'current'] as const).map(kind => <div key={kind} data-activity-association={purpose === 'activity' ? kind : undefined} data-quota-association={purpose === 'quota' ? kind : undefined}><dt>{kind === 'previous' ? 'Previous account' : 'Current account'}</dt><dd className="activity-count" tabIndex={0}><bdi>{review[kind].email}</bdi></dd><dd className="provider-help [overflow-wrap:anywhere]">Plan: <bdi>{review[kind].plan ?? 'Not available'}</bdi></dd></div>)}</dl>
      {review.homeChanged && <p className="provider-help">Profile environment changed.</p>}
      <p className="provider-help">Account details do not verify identity.</p>
      <p id={helpId} className="provider-help">New readings will use the current sign-in. Previous readings stay in encrypted history.</p>
      <Field disabled={busy}><FieldLabel className="backup-selection"><Checkbox ref={consentRef} nativeButton render={<button type="button" />} id={`${purpose}-reassociation-consent-${profileId}`} required checked={approved} disabled={busy} aria-describedby={`profile-name-${profileId} ${helpId}`} onCheckedChange={checked => onApproved(checked === true)} />I approve using the current sign-in for this profile.</FieldLabel></Field>
      <div className="profile-actions"><Button type="submit" className="h-auto max-w-full whitespace-normal" disabled={busy || !approved}>Confirm and read</Button></div>
    </form>}
    <Button type="button" variant="ghost" className="text-button h-auto max-w-full whitespace-normal" disabled={busy} onClick={onKeep}>Keep saved readings</Button>
  </>;
}
