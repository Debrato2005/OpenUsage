export type Theme = 'system' | 'light' | 'dark';
export type StorageState = 'setup_required' | 'locked' | 'ready' | 'recovery_required';
export interface Bootstrap {
  storage: StorageState;
  theme: Theme;
  schemaVersion: number | null;
  activeProfiles: number | null;
  monitoring: 'stopped';
  provider: 'not_configured';
}

interface Commands {
  bootstrap: { args: undefined; result: Bootstrap };
}

declare global {
  interface Window {
    __TAURI__?: {
      core: { invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> };
    };
  }
}

export class DesktopUnavailable extends Error {}

export async function invoke<K extends keyof Commands>(
  command: K,
  ...args: Commands[K]['args'] extends undefined ? [] : [Commands[K]['args']]
): Promise<Commands[K]['result']> {
  const core = window.__TAURI__?.core;
  if (!core) throw new DesktopUnavailable('Open the installed OpenUsage desktop application.');
  const response = await core.invoke<unknown>(command, args[0]);
  if (command === 'bootstrap' && !isBootstrap(response)) {
    throw new Error('The desktop response is incompatible. Restart OpenUsage after updating the application.');
  }
  return response as Commands[K]['result'];
}

function isBootstrap(value: unknown): value is Bootstrap {
  if (typeof value !== 'object' || value === null) return false;
  const v = value as Record<string, unknown>;
  return typeof v.storage === 'string' && ['setup_required', 'locked', 'ready', 'recovery_required'].includes(v.storage)
    && typeof v.theme === 'string' && ['system', 'light', 'dark'].includes(v.theme)
    && (v.schemaVersion === null || (Number.isSafeInteger(v.schemaVersion) && Number(v.schemaVersion) > 0))
    && (v.activeProfiles === null || (Number.isSafeInteger(v.activeProfiles) && Number(v.activeProfiles) >= 0 && Number(v.activeProfiles) <= 10))
    && v.monitoring === 'stopped' && v.provider === 'not_configured'
    && (v.storage !== 'ready' || (v.activeProfiles !== null && v.schemaVersion !== null));
}
