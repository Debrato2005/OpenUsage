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

export interface BackupList {
  backups: { fileName: string; fileBytes: string }[];
  truncated: boolean;
}

export interface BackupInspectionResult {
  fileName: string;
  archiveVersion: 1;
  manifestSchemaVersion: number;
  entryCount: number;
  totalBytes: string;
  entries: { name: string; bytes: string }[];
  databaseValidation: 'valid' | 'invalid' | 'unsupported';
  contentSummary: RestoreSummary | null;
  restoreAvailable: false;
}

export interface RestoreSummary {
  activeProfiles: number;
  archivedProfiles: string;
  accounts: string;
}

export interface RestorePreview {
  previewId: string;
  fileName: string;
  current: RestoreSummary;
  incoming: RestoreSummary;
  checkpoint: { id: string; fileBytes: string };
  restoreAvailable: true;
}

export interface RestoreReceipt {
  applied: true;
  restored: RestoreSummary;
  checkpoint: RestorePreview['checkpoint'];
  monitoring: 'stopped';
  authenticationRequired: true;
}

export interface RestoreCheckpointList {
  checkpoints: { id: string; fileBytes: string; modifiedAt: string | null }[];
  truncated: boolean;
}

export interface CheckpointRestorePreview extends Omit<RestorePreview, 'fileName'> {
  checkpointId: string;
}

interface ProviderExecutableMetadata {
  path: string;
  fileBytes: string;
  sha256: string;
  testedVersion: string;
  versionChecked: false;
  sourceVerified: boolean;
  sourceKind: 'official_artifact' | 'test_fixture' | 'unverified';
  sourceUrl: string | null;
}

export interface ProviderExecutableCandidate extends ProviderExecutableMetadata { candidateId: string }
export interface ProviderExecutableApproval extends ProviderExecutableMetadata { approvalId: string }
export interface ProviderConnectionDiagnostic {
  profileId: string;
  approvalId: string;
  providerVersion: string;
  authentication: 'authentication_required' | 'chatgpt' | 'unsupported';
  identityConfidence: 'unknown' | 'provisional';
  requiresOpenaiAuth: boolean;
  checkedAt: string | null;
  monitoring: 'stopped';
  persisted: false;
  sourceKind: 'official_artifact' | 'test_fixture';
}

interface Commands {
  inspect_provider_executable: { args: { path: string }; result: ProviderExecutableCandidate };
  approve_provider_executable: { args: { candidateId: string; confirmed: true }; result: ProviderExecutableApproval };
  check_profile_connection: { args: { profileId: string }; result: ProviderConnectionDiagnostic };
  cancel_profile_check: { args: { profileId: string }; result: { cancelled: boolean } };
  list_restore_checkpoints: { args: undefined; result: RestoreCheckpointList };
  prepare_checkpoint_restore_preview: { args: { checkpointId: string }; result: CheckpointRestorePreview };
  apply_restore: { args: { previewId: string; confirmed: true }; result: RestoreReceipt };
  prepare_restore_preview: { args: { fileName: string; password: string }; result: RestorePreview };
  cancel_restore_preview: { args: { previewId: string }; result: { cancelled: boolean } };
  list_backups: { args: undefined; result: BackupList };
  inspect_backup: { args: { fileName: string; password: string }; result: BackupInspectionResult };
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
  const valid = command === 'inspect_provider_executable' ? isProviderExecutableReview(response, 'candidateId')
    : command === 'approve_provider_executable' ? isProviderExecutableReview(response, 'approvalId')
    : command === 'check_profile_connection' ? isProviderConnectionDiagnostic(response)
    : command === 'cancel_profile_check' ? isPreviewCancellation(response)
    : command === 'list_restore_checkpoints' ? isRestoreCheckpointList(response)
    : command === 'prepare_checkpoint_restore_preview' ? isCheckpointRestorePreview(response)
    : command === 'apply_restore' ? isRestoreReceipt(response)
    : command === 'prepare_restore_preview' ? isRestorePreview(response)
    : command === 'cancel_restore_preview' ? isPreviewCancellation(response)
    : command === 'list_backups' ? isBackupList(response)
    : command === 'inspect_backup' ? isBackupInspection(response)
    : command === 'create_backup' ? isBackupReceipt(response)
    : ['list_profiles', 'create_profile', 'rename_profile', 'set_profile_archived'].includes(command)
    ? isProfileView(response) : isBootstrap(response);
  if (!valid) {
    throw new Error('The desktop response is incompatible. Restart OpenUsage after updating the application.');
  }
  return response as Commands[K]['result'];
}

function isUuid(value: unknown): value is string {
  return typeof value === 'string' && /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(value);
}

function isWideCount(value: unknown): value is string {
  return typeof value === 'string' && /^(0|[1-9]\d{0,19})$/.test(value);
}

function isUtcTimestamp(value: unknown): value is string {
  return typeof value === 'string' && /^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d\.\d{3}Z$/.test(value)
    && Number.isFinite(Date.parse(value)) && new Date(value).toISOString() === value;
}

function isProviderExecutableReview(value: unknown, id: 'candidateId' | 'approvalId'): boolean {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return false;
  const v = value as Record<string, unknown>;
  return !Object.keys(v).some(key => ![id, 'path', 'fileBytes', 'sha256', 'testedVersion', 'versionChecked', 'sourceVerified', 'sourceKind', 'sourceUrl'].includes(key))
    && isUuid(v[id]) && typeof v.path === 'string' && v.path.length > 0 && v.path.length <= 4096
    && !/\p{Cc}/u.test(v.path) && isWideCount(v.fileBytes) && v.fileBytes !== '0'
    && typeof v.sha256 === 'string' && /^[a-f0-9]{64}$/.test(v.sha256)
    && typeof v.testedVersion === 'string' && v.testedVersion.length <= 80 && /^\d+\.\d+\.\d+$/.test(v.testedVersion)
    && v.versionChecked === false && isProviderSource(v)
    && (id !== 'approvalId' || v.sourceVerified === true);
}

function isProviderSource(v: Record<string, unknown>): boolean {
  if (v.sourceKind === 'unverified') return v.sourceVerified === false && v.sourceUrl === null;
  if (v.sourceKind === 'test_fixture') return v.sourceVerified === true && v.sourceUrl === null;
  if (v.sourceKind !== 'official_artifact' || v.sourceVerified !== true || typeof v.sourceUrl !== 'string') return false;
  try {
    const url = new URL(v.sourceUrl);
    return url.protocol === 'https:' && !url.username && !url.password && !url.search && !url.hash
      && ((url.hostname === 'github.com' && url.pathname.startsWith('/openai/codex/releases/'))
        || (url.hostname === 'registry.npmjs.org' && url.pathname.startsWith('/@openai/')));
  } catch { return false; }
}

function isProviderConnectionDiagnostic(value: unknown): value is ProviderConnectionDiagnostic {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return false;
  const v = value as Record<string, unknown>;
  return !Object.keys(v).some(key => !['profileId', 'approvalId', 'providerVersion', 'authentication', 'identityConfidence', 'requiresOpenaiAuth', 'checkedAt', 'monitoring', 'persisted', 'sourceKind'].includes(key))
    && isUuid(v.profileId) && isUuid(v.approvalId)
    && typeof v.providerVersion === 'string' && v.providerVersion.length > 0 && v.providerVersion.length <= 80
    && !/\p{Cc}/u.test(v.providerVersion)
    && typeof v.authentication === 'string' && ['authentication_required', 'chatgpt', 'unsupported'].includes(v.authentication)
    && (v.identityConfidence === 'unknown' || (v.authentication === 'chatgpt' && v.identityConfidence === 'provisional'))
    && typeof v.requiresOpenaiAuth === 'boolean' && (v.checkedAt === null || isUtcTimestamp(v.checkedAt))
    && v.monitoring === 'stopped' && v.persisted === false
    && (v.sourceKind === 'official_artifact' || v.sourceKind === 'test_fixture');
}

function isRestoreCheckpointList(value: unknown): value is RestoreCheckpointList {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return false;
  const v = value as Record<string, unknown>;
  if (Object.keys(v).some(key => !['checkpoints', 'truncated'].includes(key))
    || typeof v.truncated !== 'boolean' || !Array.isArray(v.checkpoints) || v.checkpoints.length > 100) return false;
  const ids = new Set<string>();
  return v.checkpoints.every(item => {
    if (typeof item !== 'object' || item === null || Array.isArray(item)) return false;
    const c = item as Record<string, unknown>;
    if (Object.keys(c).some(key => !['id', 'fileBytes', 'modifiedAt'].includes(key))
      || !isUuid(c.id) || ids.has(c.id) || !isWideCount(c.fileBytes) || c.fileBytes === '0') return false;
    if (c.modifiedAt !== null && !isUtcTimestamp(c.modifiedAt)) return false;
    ids.add(c.id);
    return true;
  });
}

function isCheckpointRestorePreview(value: unknown): value is CheckpointRestorePreview {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return false;
  const v = value as Record<string, unknown>;
  if (Object.keys(v).some(key => !['checkpointId', 'previewId', 'current', 'incoming', 'checkpoint', 'restoreAvailable'].includes(key))
    || !isUuid(v.checkpointId)) return false;
  return isRestoreComparison(v);
}

function isRestoreSummary(value: unknown): value is RestoreSummary {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return false;
  const v = value as Record<string, unknown>;
  return !Object.keys(v).some(key => !['activeProfiles', 'archivedProfiles', 'accounts'].includes(key))
    && Number.isSafeInteger(v.activeProfiles) && Number(v.activeProfiles) >= 0 && Number(v.activeProfiles) <= 10
    && isWideCount(v.archivedProfiles) && isWideCount(v.accounts);
}

function isRestorePreview(value: unknown): value is RestorePreview {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return false;
  const v = value as Record<string, unknown>;
  return !Object.keys(v).some(key => !['previewId', 'fileName', 'current', 'incoming', 'checkpoint', 'restoreAvailable'].includes(key))
    && isArchiveFileName(v.fileName) && isRestoreComparison(v);
}

function isRestoreComparison(v: Record<string, unknown>): boolean {
  if (!isUuid(v.previewId) || v.restoreAvailable !== true
    || !isRestoreSummary(v.current) || !isRestoreSummary(v.incoming)
    || typeof v.checkpoint !== 'object' || v.checkpoint === null || Array.isArray(v.checkpoint)) return false;
  const checkpoint = v.checkpoint as Record<string, unknown>;
  return !Object.keys(checkpoint).some(key => !['id', 'fileBytes'].includes(key))
    && isUuid(checkpoint.id) && isWideCount(checkpoint.fileBytes) && checkpoint.fileBytes !== '0';
}

function isRestoreReceipt(value: unknown): value is RestoreReceipt {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return false;
  const v = value as Record<string, unknown>;
  if (Object.keys(v).some(key => !['applied', 'restored', 'checkpoint', 'monitoring', 'authenticationRequired'].includes(key))
    || v.applied !== true || v.monitoring !== 'stopped' || v.authenticationRequired !== true
    || !isRestoreSummary(v.restored) || typeof v.checkpoint !== 'object'
    || v.checkpoint === null || Array.isArray(v.checkpoint)) return false;
  const checkpoint = v.checkpoint as Record<string, unknown>;
  return !Object.keys(checkpoint).some(key => !['id', 'fileBytes'].includes(key))
    && isUuid(checkpoint.id) && isWideCount(checkpoint.fileBytes) && checkpoint.fileBytes !== '0';
}

function isPreviewCancellation(value: unknown): value is { cancelled: boolean } {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return false;
  const v = value as Record<string, unknown>;
  return Object.keys(v).length === 1 && typeof v.cancelled === 'boolean';
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

function isArchiveFileName(value: unknown): value is string {
  return typeof value === 'string' && /^openusage-backup-[A-Za-z0-9]{16}\.oubackup$/.test(value);
}

function isByteCount(value: unknown): value is string {
  return typeof value === 'string' && /^(0|[1-9]\d{0,9})$/.test(value);
}

function isBackupList(value: unknown): value is BackupList {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return false;
  const v = value as Record<string, unknown>;
  if (Object.keys(v).some(key => !['backups', 'truncated'].includes(key))
    || !Array.isArray(v.backups) || v.backups.length > 100 || typeof v.truncated !== 'boolean') return false;
  const names = new Set<string>();
  for (const value of v.backups) {
    if (typeof value !== 'object' || value === null || Array.isArray(value)) return false;
    const b = value as Record<string, unknown>;
    if (Object.keys(b).some(key => !['fileName', 'fileBytes'].includes(key))
      || !isArchiveFileName(b.fileName) || !isByteCount(b.fileBytes) || names.has(b.fileName)) return false;
    names.add(b.fileName);
  }
  return true;
}

function isBackupInspection(value: unknown): value is BackupInspectionResult {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return false;
  const v = value as Record<string, unknown>;
  if (Object.keys(v).some(key => !['fileName', 'archiveVersion', 'manifestSchemaVersion', 'entryCount', 'totalBytes', 'entries', 'databaseValidation', 'contentSummary', 'restoreAvailable'].includes(key))
    || !isArchiveFileName(v.fileName) || v.archiveVersion !== 1
    || !Number.isSafeInteger(v.manifestSchemaVersion) || Number(v.manifestSchemaVersion) < 1 || Number(v.manifestSchemaVersion) > 4294967295
    || !Number.isSafeInteger(v.entryCount) || Number(v.entryCount) < 1 || Number(v.entryCount) > 10000
    || !isByteCount(v.totalBytes) || !Array.isArray(v.entries) || v.entries.length !== v.entryCount
    || !['valid', 'invalid', 'unsupported'].some(state => state === v.databaseValidation)
    || v.restoreAvailable !== false) return false;
  if (v.databaseValidation === 'valid') {
    if (!isRestoreSummary(v.contentSummary)) return false;
  } else if (v.contentSummary !== null) return false;
  const names = new Set<string>();
  for (const value of v.entries) {
    if (typeof value !== 'object' || value === null || Array.isArray(value)) return false;
    const e = value as Record<string, unknown>;
    if (Object.keys(e).some(key => !['name', 'bytes'].includes(key))
      || typeof e.name !== 'string' || e.name.length < 1 || e.name.length > 240
      || !/^[a-z0-9._/-]+$/.test(e.name) || !isByteCount(e.bytes) || names.has(e.name)) return false;
    names.add(e.name);
  }
  return true;
}
