import { useState } from 'react';
import { Button } from './components/ui/button';
import { Select, SelectTrigger, SelectValue, SelectPopup, SelectItem } from './components/ui/select';
import type { ProfileRecord } from './contracts';
import { ProfileActivity } from './activity';

export function Usage({ profiles, revision, onAccounts, active }: { active: boolean; profiles: ProfileRecord[]; revision: number; onAccounts: () => void }) {
  const [selected, setSelected] = useState<string | null>(null);
  const profile = profiles.find(value => value.id === selected) ?? profiles.find(value => !value.archived) ?? profiles[0];
  if (!active) return null;
  return <section className="usage-content">
    <div className="section-heading"><div><h2>Saved token activity</h2><p>Provider-reported activity for one profile. Combined totals are not available.</p></div>
      {profile && <Select modal={false} value={profile.id} items={Object.fromEntries(profiles.map(value => [value.id, value.label]))} onValueChange={setSelected}>
        <SelectTrigger aria-label="Usage profile"><SelectValue /></SelectTrigger><SelectPopup>{profiles.map(value => <SelectItem key={value.id} value={value.id}>{value.label}{value.archived ? ' (archived)' : ''}</SelectItem>)}</SelectPopup>
      </Select>}
    </div>
    {profile ? <ProfileActivity key={profile.id} readingRevision={String(revision)} profileId={profile.id} profileLabel={profile.label} emptyContent={<div className="empty-state"><h3>No saved activity for this profile</h3><p>Read its activity from Accounts to save a reading.</p><Button variant="outline" onClick={onAccounts}>View accounts</Button></div>}>{() => null}</ProfileActivity>
      : <div className="empty-state"><h3>No saved activity yet</h3><p>Add an account profile, then read its activity.</p><Button variant="outline" onClick={onAccounts}>View accounts</Button></div>}
  </section>;
}
