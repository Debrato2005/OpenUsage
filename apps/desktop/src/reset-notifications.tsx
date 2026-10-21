import { useEffect, useRef, useState } from 'react';
import { Button } from './components/ui/button';
import { Checkbox } from './components/ui/checkbox';
import { Collapsible, CollapsiblePanel, CollapsibleTrigger } from './components/ui/collapsible';
import { Field, FieldLabel } from './components/ui/field';
import { Input } from './components/ui/input';
import { Select, SelectItem, SelectPopup, SelectTrigger, SelectValue } from './components/ui/select';
import { invoke, type ProfileRecord, type ResetNotificationPreferences } from './contracts';
import './reset-notifications.css';

type Props = { profiles: ProfileRecord[]; revision: string; busy: boolean; setBusy: (busy: boolean) => void };
type Loaded = { key: string; preferences: ResetNotificationPreferences | null; error: boolean };

function saveFailure(error: unknown) {
  if (error === 'notification_permission_denied') return 'Allow OpenUsage notifications in your system settings, then try again.';
  if (error === 'notifications_unavailable') return 'Desktop notifications are unavailable. Check your system notification service, then try again.';
  if (error === 'invalid_arguments' || error === 'invalid_notification_preferences') return 'Check the time zone, quiet hours and account selections, then try again.';
  if (error === 'workspace_locked') return 'Unlock your workspace before you save notification preferences.';
  return 'Notification preferences could not be saved. Try again.';
}

export function ResetNotifications({ profiles, revision, busy, setBusy }: Props) {
  const activeProfiles = profiles.filter(profile => !profile.archived);
  const profileKey = activeProfiles.map(profile => profile.id).join(',');
  const [open, setOpen] = useState(false);
  const [attempt, setAttempt] = useState(0);
  const [loaded, setLoaded] = useState<Loaded | null>(null);
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const generation = useRef(0);
  const pending = useRef(false);
  const ownsBusy = useRef(false);
  const key = `${revision}:${profileKey}:${attempt}`;
  const current = loaded?.key === key ? loaded : null;
  const preferences = current?.preferences;
  const loading = open && current === null;

  useEffect(() => {
    const epoch = ++generation.current;
    setSaved(false); setError(null); setSaving(false); pending.current = false;
    if (open) {
      void invoke('load_reset_notification_preferences').then(preferences => {
        if (generation.current === epoch) setLoaded({ key, preferences, error: false });
      }).catch(() => {
        if (generation.current === epoch) setLoaded({ key, preferences: null, error: true });
      });
    }
    return () => {
      generation.current += 1;
      if (ownsBusy.current) { ownsBusy.current = false; setBusy(false); }
    };
  }, [open, key, setBusy]);

  function edit(update: Partial<ResetNotificationPreferences>) {
    if (!preferences || busy || pending.current) return;
    setLoaded({ key, preferences: { ...preferences, ...update }, error: false });
    setSaved(false); setError(null);
  }
  async function save() {
    if (!preferences || busy || pending.current) return;
    const epoch = generation.current;
    pending.current = true; ownsBusy.current = true;
    setBusy(true); setSaving(true); setSaved(false); setError(null);
    try {
      const result = await invoke('save_reset_notification_preferences', { preferences });
      if (generation.current !== epoch) return;
      setLoaded({ key, preferences: result, error: false }); setSaved(true);
    } catch (error: unknown) {
      if (generation.current === epoch) setError(saveFailure(error));
    } finally {
      if (generation.current === epoch) {
        pending.current = false; setSaving(false);
        if (ownsBusy.current) { ownsBusy.current = false; setBusy(false); }
      }
    }
  }

  return <Collapsible className="reset-notifications" data-reset-notifications open={open} onOpenChange={next => { setLoaded(null); setOpen(next); }}>
    <CollapsibleTrigger render={<Button variant="outline" />} disabled={saving}>Reset notifications</CollapsibleTrigger>
    <CollapsiblePanel>
      {loading && <p className="reset-notifications-note" role="status">Loading notification preferences…</p>}
      {current?.error && <div className="reset-notifications-error" role="alert"><p>Notification preferences could not be loaded.</p><Button variant="outline" onClick={() => setAttempt(value => value + 1)}>Retry notification preferences</Button></div>}
      {preferences && <form className="reset-notifications-form" onSubmit={event => { event.preventDefault(); void save(); }} aria-busy={saving}>
        <Field disabled={busy}><FieldLabel className="reset-notifications-check"><Checkbox nativeButton render={<button type="button" />} id="reset-notifications-enabled" checked={preferences.enabled} disabled={busy} onCheckedChange={checked => edit({ enabled: checked === true })} />Notify me when a quota reset is confirmed</FieldLabel></Field>
        <p className="reset-notifications-note">Applies to new confirmations. Saved events are not replayed.</p>
        <Collapsible className="reset-notifications-details">
          <CollapsibleTrigger render={<Button variant="ghost" />}>Quiet hours</CollapsibleTrigger>
          <CollapsiblePanel>
            <Field disabled={busy}><FieldLabel className="reset-notifications-check"><Checkbox nativeButton render={<button type="button" />} id="reset-notifications-quiet" checked={preferences.quietEnabled} disabled={busy} onCheckedChange={checked => edit({ quietEnabled: checked === true })} />Use quiet hours</FieldLabel></Field>
            <p className="reset-notifications-note">Notifications wait until quiet hours end, if the reset is still relevant.</p>
            <div className="reset-notifications-times">
              <Field disabled={busy || !preferences.quietEnabled}><FieldLabel htmlFor="reset-notifications-start">Start time</FieldLabel><Input nativeInput id="reset-notifications-start" type="time" required step={60} value={preferences.quietStart} disabled={busy || !preferences.quietEnabled} onChange={event => edit({ quietStart: event.target.value })} /></Field>
              <Field disabled={busy || !preferences.quietEnabled}><FieldLabel htmlFor="reset-notifications-end">End time</FieldLabel><Input nativeInput id="reset-notifications-end" type="time" required step={60} value={preferences.quietEnd} disabled={busy || !preferences.quietEnabled} onChange={event => edit({ quietEnd: event.target.value })} /></Field>
            </div>
            <Field disabled={busy || !preferences.quietEnabled}><FieldLabel htmlFor="reset-notifications-zone">Time zone</FieldLabel><Input nativeInput id="reset-notifications-zone" required maxLength={128} value={preferences.timeZone} disabled={busy || !preferences.quietEnabled} autoCapitalize="none" spellCheck={false} aria-describedby="reset-notifications-zone-help" onChange={event => edit({ timeZone: event.target.value })} /></Field>
            <p className="reset-notifications-note" id="reset-notifications-zone-help">For example, Asia/Kolkata or Europe/London.</p>
          </CollapsiblePanel>
        </Collapsible>
        {activeProfiles.length > 0 && <Collapsible className="reset-notifications-details">
          <CollapsibleTrigger render={<Button variant="ghost" />}>Account overrides</CollapsibleTrigger>
          <CollapsiblePanel>
            <p className="reset-notifications-note">Default follows the setting above.</p>
            <ul className="reset-notifications-overrides">{activeProfiles.map(profile => {
              const override = preferences.profileOverrides.find(item => item.profileId === profile.id);
              const value = override ? override.enabled ? 'on' : 'off' : 'default';
              return <li key={profile.id}><label htmlFor={`reset-notifications-${profile.id}`}><bdi>{profile.label}</bdi></label><Select modal={false} value={value} disabled={busy} items={{ default: 'Default', on: 'On', off: 'Off' }} onValueChange={next => {
                if (!['default', 'on', 'off'].includes(String(next))) return;
                const profileOverrides = preferences.profileOverrides.filter(item => item.profileId !== profile.id);
                if (next !== 'default') profileOverrides.push({ profileId: profile.id, enabled: next === 'on' });
                edit({ profileOverrides });
              }}><SelectTrigger id={`reset-notifications-${profile.id}`} aria-label={`Reset notifications for ${profile.label}`}><SelectValue /></SelectTrigger><SelectPopup alignItemWithTrigger={false}><SelectItem value="default">Default</SelectItem><SelectItem value="on">On</SelectItem><SelectItem value="off">Off</SelectItem></SelectPopup></Select></li>;
            })}</ul>
          </CollapsiblePanel>
        </Collapsible>}
        <div className="reset-notifications-actions"><Button type="submit" disabled={busy}>{saving ? 'Saving preferences…' : 'Save preferences'}</Button>{saved && <p role="status">Notification preferences saved.</p>}</div>
        {error && <p className="operation-error" role="alert">{error}</p>}
      </form>}
    </CollapsiblePanel>
  </Collapsible>;
}
