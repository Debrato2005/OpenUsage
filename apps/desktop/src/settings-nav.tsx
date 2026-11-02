export const settingsSections = {
  general: { id: 'settings-general', title: 'General' },
  accounts: { id: 'settings-accounts', title: 'Accounts and plans' },
  notifications: { id: 'settings-notifications', title: 'Alerts' },
  data: { id: 'settings-data', title: 'Backups and exports' },
  connection: { id: 'settings-connection', title: 'Connection' },
  privacy: { id: 'settings-privacy', title: 'Privacy and data' },
};
const sections = Object.values(settingsSections);

// A link moves focus as well as scroll position, so keyboard users land in the section.
export function SettingsNav() {
  return <nav className="settings-nav" aria-label="Settings sections">{sections.map(section => <a key={section.id} href={`#${section.id}`} onClick={event => {
    event.preventDefault();
    const target = document.getElementById(section.id)!;
    target.scrollIntoView({ block: 'start', behavior: window.matchMedia('(prefers-reduced-motion: reduce)').matches ? 'auto' : 'smooth' });
    target.focus({ preventScroll: true });
  }}>{section.title}</a>)}</nav>;
}
