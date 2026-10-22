import { useEffect, useState } from 'react';
import { Button } from './components/ui/button';
import { Select, SelectTrigger, SelectValue, SelectPopup, SelectItem } from './components/ui/select';
import type { ProfileRecord } from './contracts';
import { ProfileActivity } from './activity';
import { QuotaHistory } from './quota-history';
import { ActivityHistory } from './activity-history';
import { TokenHeatmap } from './token-heatmap';

export function Usage({ profiles, revision, onAccounts, active, requestedProfileId }: { requestedProfileId?: string | null; active: boolean; profiles: ProfileRecord[]; revision: number; onAccounts: () => void }) {
  const [selected, setSelected] = useState<string | null>(null);
  useEffect(() => { if (active && requestedProfileId) setSelected(requestedProfileId); }, [active, requestedProfileId]);
  const profile = profiles.find(value => value.id === selected) ?? profiles.find(value => !value.archived) ?? profiles[0];
  if (!active) return null;
  return <section className="usage-content">
    <div className="section-heading"><div><h2>Saved usage</h2><p>Quota history and reported token activity for one account profile.</p></div>
      {profile && <Select modal={false} value={profile.id} items={Object.fromEntries(profiles.map(value => [value.id, `${value.label}${value.archived ? ' (archived)' : ''}`]))} onValueChange={setSelected}>
        <SelectTrigger aria-label="Usage profile"><SelectValue /></SelectTrigger><SelectPopup>{profiles.map(value => <SelectItem key={value.id} value={value.id}>{value.label}{value.archived ? ' (archived)' : ''}</SelectItem>)}</SelectPopup>
      </Select>}
    </div>
    {profile && <TokenHeatmap profileId={profile.id} profileLabel={profile.label} revision={revision} />}
    {profile && <QuotaHistory key={`history:${profile.id}`} profile={profile} revision={revision} />}
    {profile && <ActivityHistory key={`activity-history:${profile.id}`} profileId={profile.id} revision={revision} />}
    {profile ? <ProfileActivity key={profile.id} readingRevision={String(revision)} profileId={profile.id} profileLabel={profile.label} emptyContent={<div className="empty-state"><h3>No saved activity for this profile</h3><p>Refresh this account on Overview to save a reading.</p><Button variant="outline" onClick={onAccounts}>Manage accounts</Button></div>}>{() => null}</ProfileActivity>
      : <div className="empty-state"><h3>No saved activity yet</h3><p>Add an account profile, then read its activity.</p><Button variant="outline" onClick={onAccounts}>Manage accounts</Button></div>}
  </section>;
}
