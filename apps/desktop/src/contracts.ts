export type Theme = 'system' | 'light' | 'dark';
export type StorageState = 'setup_required' | 'locked' | 'ready' | 'recovery_required';
export type KeyProtection = 'os_store' | 'passphrase' | 'passphrase_required' | 'unavailable';
export type StorageIssue = 'key_store_unavailable' | 'missing_key' | 'missing_database' | 'invalid_key' | 'storage_unavailable' | 'workspace_in_use';
export interface Bootstrap {
  storage: StorageState;
  theme: Theme;
  schemaVersion: number | null;
  activeProfiles: number | null;
  monitoring: 'stopped';
  provider: 'not_configured';
  keyProtection: KeyProtection;
  storageIssue: StorageIssue | null;
}

export interface BackupReceipt {
  fileName: string;
  directory: string;
}

export interface ProfileRecord {
  id: string;
  label: string;
  environment: 'native' | 'wsl';
  connectionState: 'connected' | 'authentication_required' | 'unauthorized' | 'offline' | 'unsupported' | 'error';
  archived: boolean;
}

export interface ProfileView {
  profiles: ProfileRecord[];
  activeProfiles: number;
}

interface Commands {
  list_profiles: { args: undefined; result: ProfileView };
  create_profile: { args: { label: string }; result: ProfileView };
  rename_profile: { args: { id: string; label: string }; result: ProfileView };
  set_profile_archived: { args: { id: string; archived: boolean }; result: ProfileView };
  bootstrap: { args: undefined; result: Bootstrap };
  setup_workspace: { args: { passphrase: string | null }; result: Bootstrap };
  unlock_workspace: { args: { passphrase: string }; result: Bootstrap };
  set_theme: { args: { theme: Theme }; result: Bootstrap };
  create_backup: { args: { password: string }; result: BackupReceipt };
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
  const valid = command === 'create_backup' ? isBackupReceipt(response)
    : ['list_profiles', 'create_profile', 'rename_profile', 'set_profile_archived'].includes(command)
    ? isProfileView(response) : isBootstrap(response);
  if (!valid) {
    throw new Error('The desktop response is incompatible. Restart OpenUsage after updating the application.');
  }
  return response as Commands[K]['result'];
}

function isBackupReceipt(value: unknown): value is BackupReceipt {
  if (typeof value !== 'object' || value === null) return false;
  const receipt = value as Record<string, unknown>;
  return typeof receipt.fileName === 'string' && receipt.fileName.length > 0
    && receipt.fileName.length <= 160 && !/[\\/\0]/.test(receipt.fileName)
    && typeof receipt.directory === 'string' && receipt.directory.length > 0
    && receipt.directory.length <= 32768 && !receipt.directory.includes('\0');
}

function isBootstrap(value: unknown): value is Bootstrap {
  if (typeof value !== 'object' || value === null) return false;
  const v = value as Record<string, unknown>;
  return typeof v.storage === 'string' && ['setup_required', 'locked', 'ready', 'recovery_required'].includes(v.storage)
    && typeof v.theme === 'string' && ['system', 'light', 'dark'].includes(v.theme)
    && (v.schemaVersion === null || (Number.isSafeInteger(v.schemaVersion) && Number(v.schemaVersion) > 0))
    && (v.activeProfiles === null || (Number.isSafeInteger(v.activeProfiles) && Number(v.activeProfiles) >= 0 && Number(v.activeProfiles) <= 10))
    && v.monitoring === 'stopped' && v.provider === 'not_configured'
    && typeof v.keyProtection === 'string'
    && ['os_store', 'passphrase', 'passphrase_required', 'unavailable'].includes(v.keyProtection)
    && (v.storageIssue === null || (typeof v.storageIssue === 'string'
      && ['key_store_unavailable', 'missing_key', 'missing_database', 'invalid_key', 'storage_unavailable', 'workspace_in_use'].includes(v.storageIssue)))
    && (v.storage !== 'ready' || (v.activeProfiles !== null && v.schemaVersion !== null));
}

function isProfileView(value: unknown): value is ProfileView {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return false;
  const v = value as Record<string, unknown>;
  if (Object.keys(v).length !== 2 || !Array.isArray(v.profiles)
    || !Number.isSafeInteger(v.activeProfiles) || Number(v.activeProfiles) < 0 || Number(v.activeProfiles) > 10) return false;
  const ids = new Set<string>();
  for (const value of v.profiles) {
    if (typeof value !== 'object' || value === null || Array.isArray(value)) return false;
    const p = value as Record<string, unknown>;
    if (Object.keys(p).some(key => !['id', 'label', 'environment', 'connectionState', 'archived'].includes(key)) || typeof p.id !== 'string'
      || !/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(p.id)
      || ids.has(p.id) || typeof p.label !== 'string' || p.label.length > 400
      || Array.from(p.label).length < 1 || Array.from(p.label).length > 100 || /[\u0000-\u001f\u007f-\u009f]/u.test(p.label)
      || typeof p.environment !== 'string' || !['native', 'wsl'].includes(p.environment)
      || typeof p.connectionState !== 'string'
      || !['connected', 'authentication_required', 'unauthorized', 'offline', 'unsupported', 'error'].includes(p.connectionState)
      || typeof p.archived !== 'boolean') return false;
    ids.add(p.id);
  }
  return v.profiles.filter(p => !(p as ProfileRecord).archived).length === v.activeProfiles;
}
