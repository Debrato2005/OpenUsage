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

interface Commands {
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
  if (command === 'create_backup' ? !isBackupReceipt(response) : !isBootstrap(response)) {
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
