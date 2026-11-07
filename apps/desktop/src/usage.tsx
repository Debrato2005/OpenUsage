import { useEffect, useState } from 'react';
import { Button } from './components/ui/button';
import { Select, SelectTrigger, SelectValue, SelectPopup, SelectItem } from './components/ui/select';
import { invoke, type ProfileRecord, type SavedProfileQuota } from './contracts';
import { AccountDonut } from './account-donut';
import { ProfileActivity } from './activity';
import { QuotaHistory } from './quota-history';
import { ActivityHistory } from './activity-history';
import { TokenHeatmap } from './token-heatmap';
import { QuotaCompare } from './quota-compare';
import { LocalUsagePanel } from './local-usage';

function UsageDonut({ profile, revision }: { profile: ProfileRecord; revision: number }) {
  const [quota, setQuota] = useState<SavedProfileQuota | null>(null);
  useEffect(() => {
    let current = true;
    setQuota(null);
    void invoke('load_profile_quota', { profileId: profile.id }).then(value => { if (current && value?.profileId === profile.id) setQuota(value); }).catch(() => {});
    return () => { current = false; };
  }, [profile.id, revision]);
  const limit = quota?.limits[0];
  return <AccountDonut mode="quota" label={profile.label} window={limit?.primary ?? limit?.secondary ?? null} />;
}

type Source = 'provider' | 'device';

// One source control sets the scope, so numbers need no source label (OU-LOG-016).
export function Usage({ profiles, revision, onAccounts, active, requestedProfileId, requestedDevice = 0 }: { requestedProfileId?: string | null; requestedDevice?: number; active: boolean; profiles: ProfileRecord[]; revision: number; onAccounts: () => void }) {
  const [selected, setSelected] = useState<string | null>(null);
  const [chosenSource, setSource] = useState<Source | null>(null);
  useEffect(() => { if (active && requestedProfileId) { setSelected(requestedProfileId); setSource('provider'); } }, [active, requestedProfileId]);
  useEffect(() => { if (active && requestedDevice) setSource('device'); }, [active, requestedDevice]);
  const profile = profiles.find(value => value.id === selected) ?? profiles.find(value => !value.archived) ?? profiles[0];
  const source = chosenSource ?? (profiles.length ? 'provider' : 'device');
  if (!active) return null;
  const sourceControl = <div className="overview-chips usage-source" role="group" aria-label="Source">
    <Button variant="ghost" aria-pressed={source === 'provider'} onClick={() => setSource('provider')}>Provider</Button>
    <Button variant="ghost" aria-pressed={source === 'device'} onClick={() => setSource('device')}>This device</Button>
  </div>;
  if (source === 'device') return <section className="usage-content">{sourceControl}<LocalUsagePanel active={active} /></section>;
  return <section className="usage-content">{sourceControl}
    <div className="usage-controls">
      {profile && <Select modal={false} value={profile.id} items={Object.fromEntries(profiles.map(value => [value.id, `${value.label}${value.archived ? ' (archived)' : ''}`]))} onValueChange={setSelected}>
        <SelectTrigger aria-label="Usage profile"><SelectValue /></SelectTrigger><SelectPopup>{profiles.map(value => <SelectItem key={value.id} value={value.id}>{value.label}{value.archived ? ' (archived)' : ''}</SelectItem>)}</SelectPopup>
      </Select>}
    </div>
    {profile && <div className="usage-activity"><div className="usage-card"><TokenHeatmap profileId={profile.id} profileLabel={profile.label} revision={revision} emptyAction={<Button variant="outline" onClick={onAccounts}>Manage accounts</Button>} /></div><UsageDonut profile={profile} revision={revision} /></div>}
    {profile ? <ProfileActivity key={profile.id} readingRevision={String(revision)} profileId={profile.id} profileLabel={profile.label} summaryOnly>{() => null}</ProfileActivity>
      : <div className="empty-state"><h3>No saved activity yet</h3><Button variant="outline" onClick={onAccounts}>Manage accounts</Button></div>}
    <QuotaCompare profiles={profiles} revision={revision} />
    {profile && <QuotaHistory key={`history:${profile.id}`} profile={profile} revision={revision} />}
    {profile && <ActivityHistory key={`activity-history:${profile.id}`} profile={profile} revision={revision} />}
  </section>;
}
