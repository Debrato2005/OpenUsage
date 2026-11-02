import type { ReactNode, SVGProps } from 'react';

// Interface icons stay inline so the strict CSP never needs a remote asset.
function Icon({ children, ...props }: SVGProps<SVGSVGElement> & { children: ReactNode }) {
  return <svg className="ui-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true" {...props}>{children}</svg>;
}

export const RefreshIcon = () => <Icon><path d="M20 11a8 8 0 0 0-14.3-4.3L4 8.5" /><path d="M4 4v4.5h4.5" /><path d="M4 13a8 8 0 0 0 14.3 4.3l1.7-1.8" /><path d="M20 20v-4.5h-4.5" /></Icon>;
export const MoreIcon = () => <Icon strokeWidth="3"><path d="M5 12h.01M12 12h.01M19 12h.01" /></Icon>;
export const PlusIcon = () => <Icon><path d="M12 5v14M5 12h14" /></Icon>;
export const SearchIcon = () => <Icon><circle cx="11" cy="11" r="6.5" /><path d="m20 20-4.4-4.4" /></Icon>;
export const CloseIcon = () => <Icon><path d="M6 6l12 12M18 6 6 18" /></Icon>;
export const ArrowIcon = () => <Icon><path d="M7 17 17 7M9 7h8v8" /></Icon>;
export const UsersIcon = () => <Icon><circle cx="9" cy="8.5" r="3.5" /><path d="M3 20c0-3.3 2.7-5.5 6-5.5s6 2.2 6 5.5" /><path d="M16 5.3a3.5 3.5 0 0 1 0 6.4M18 14.8c1.9.7 3 2.5 3 5.2" /></Icon>;
export const GaugeIcon = () => <Icon><path d="M4.5 18a9 9 0 1 1 15 0" /><path d="m12 14 3.5-4.5" /><circle cx="12" cy="14" r="1.2" /></Icon>;
export const ClockIcon = () => <Icon><circle cx="12" cy="12" r="8.5" /><path d="M12 7.5V12l3 2" /></Icon>;
export const PulseIcon = () => <Icon><path d="M3 12h4l2.5-6 4 12 2.5-6h5" /></Icon>;
export const ShieldIcon = () => <Icon><path d="M12 3 5 6v5.5c0 4.4 3 7.6 7 9.5 4-1.9 7-5.1 7-9.5V6Z" /><path d="m9 12 2.2 2.2L15.5 10" /></Icon>;
export const LockIcon = () => <Icon><rect x="5" y="11" width="14" height="9" rx="2" /><path d="M8 11V8a4 4 0 0 1 8 0v3" /></Icon>;
export const EyeOffIcon = () => <Icon><path d="M4 4l16 16" /><path d="M10.6 6.2A9.6 9.6 0 0 1 12 6c5 0 8.5 4 9.5 6a13 13 0 0 1-2.6 3.5M6.5 7.6A13 13 0 0 0 2.5 12c1 2 4.5 6 9.5 6 1.5 0 2.9-.4 4.1-1" /></Icon>;
