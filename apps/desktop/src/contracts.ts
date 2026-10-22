export type MonitoringState = 'stopped' | 'running' | 'paused';
interface SavedAlertCommon {
  id: string; profileId: string; profileLabel: string;
  evidenceAtUtc: string; sourceKind: 'official_artifact' | 'test_fixture';
  deliveryState: 'recorded' | 'queued' | 'submitted' | 'failed' | 'suppressed' | 'uncertain' | 'covered';
}
export type SavedAlertEvent = (SavedAlertCommon & {
  kind: 'quota_reset' | 'quota_threshold'; thresholdPercent: 80 | 90 | 100 | null;
  evidenceRunId: string; limitId: string; windowRole: 'primary' | 'secondary'; windowDurationMins: number;
}) | (SavedAlertCommon & { kind: 'authentication_required' | 'collection_failure' });
export interface SavedAlertHistory { events: SavedAlertEvent[]; truncated: boolean }
export interface ResetNotificationPreferences {
  enabled: boolean; timeZone: string; quietEnabled: boolean; quietStart: string; quietEnd: string;
  profileOverrides: { profileId: string; enabled: boolean }[];
}
export interface ThresholdNotificationPreferences {
  enabled: boolean; profileOverrides: { profileId: string; enabled: boolean }[];
}
export interface NotificationPreferences {
  reset: ResetNotificationPreferences; usage: ThresholdNotificationPreferences;
  authentication: ThresholdNotificationPreferences; collectionFailure: ThresholdNotificationPreferences;
}
export interface ProfileMonitoringStatus {
  profileId: string; state: MonitoringState; eligible: boolean;
  issue: null | 'sign_in_required' | 'account_changed' | 'source_changed' | 'profile_unavailable' | 'unsupported' | 'storage_unavailable' | 'provider_error';
  nextReadInSeconds: number | null; readingActive: boolean;
}
export interface MonitoringStatus {
  state: MonitoringState; readingRevision: string; profiles: ProfileMonitoringStatus[];
}

export type Theme = 'system' | 'light' | 'dark';
export type StorageState = 'setup_required' | 'locked' | 'ready' | 'recovery_required';
export type KeyProtection = 'os_store' | 'passphrase' | 'passphrase_required' | 'unavailable';
export type StorageIssue = 'key_store_unavailable' | 'missing_key' | 'missing_database' | 'invalid_key' | 'storage_unavailable' | 'workspace_in_use' | 'local_deletion_incomplete';
export interface Bootstrap {
  storage: StorageState;
  theme: Theme;
  schemaVersion: number | null;
  activeProfiles: number | null;
  monitoring: MonitoringState;
  provider: 'not_configured';
  keyProtection: KeyProtection;
  storageIssue: StorageIssue | null;
}

export interface BackupReceipt {
  fileName: string;
  directory: string;
}

export type ReadingExportFormat = 'csv' | 'json';
export interface ReadingExportReceipt {
  fileName: string;
  directory: string;
  format: ReadingExportFormat;
  profileCount: number;
  rowCount: number;
  identifiersIncluded: boolean;
  exportedAt: string;
}

export interface ApplicationExportReceipt {
  fileName: string;
  directory: string;
  format: 'json';
  schemaVersion: number;
  tableCount: number;
  recordCount: string;
  byteCount: string;
  exportedAt: string;
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
  monitoring: MonitoringState;
  persisted: false;
  sourceKind: 'official_artifact' | 'test_fixture';
}

export interface ProviderLoginAttempt {
  profileId: string;
  attemptId: string;
  approvalId: string;
  sourceKind: ProviderConnectionDiagnostic['sourceKind'];
  monitoring: MonitoringState;
  applicationWaitLimitSeconds: 900;
}

export interface ProviderLoginPrompt {
  profileId: string;
  attemptId: string;
  verificationUrl: 'https://auth.openai.com/codex/device';
  userCode: string;
  sourceKind: ProviderConnectionDiagnostic['sourceKind'];
  applicationWaitLimitSeconds: 900;
}

export interface ProviderLoginDiagnostic extends Omit<ProviderConnectionDiagnostic, 'persisted' | 'authentication' | 'checkedAt'> {
  persisted: true;
  authentication: 'chatgpt';
  checkedAt: string;
}

export interface ProviderLoginCompletion {
  attemptId: string;
  diagnostic: ProviderLoginDiagnostic;
}

export interface ProviderExecutableDiscovery {
  candidates: { path: string; location: 'path' | 'standard' | 'npm_package' }[];
  truncated: boolean;
}

export interface ProfileRefreshStatus {
  profileId: string; retryAfterSeconds: number;
}

export interface ProfileReadingReference {
  profileId: string; bindingId: string; collectionRunId: string;
}
export interface CollectionReading {
  bindingId: string; collectionRunId: string; observedAtUtc: string;
}
export interface ProfileCollectionOutcome {
  requestId: string; profileId: string; approvalId: string;
  providerVersion: string; sourceKind: ProviderConnectionDiagnostic['sourceKind'];
  identityConfidence: 'provisional'; verifiedIdentityKey: null;
  monitoring: MonitoringState; accepted: boolean;
  quota: CollectionReading | null; activity: CollectionReading | null;
}

export interface ProfileQuotaRequest {
  profileId: string; requestId: string; approvalId: string;
  sourceKind: ProviderConnectionDiagnostic['sourceKind'];
  monitoring: MonitoringState; accepted: false;
}
export interface QuotaWindow {
  usedPercent: number; derivedRemainingPercent: number;
  windowDurationMins: number | null; resetsAtUnix: number | null; label: string;
}
export interface QuotaHistoryEntry {
  profileId: string; bindingId: string; collectionRunId: string; observedAtUtc: string;
  sourceKind: 'official_artifact' | 'test_fixture'; providerVersion: string;
  limits: { limitId: string; limitName: string | null; primary: QuotaWindow | null; secondary: QuotaWindow | null }[];
}
export interface QuotaHistoryPage { profileId: string; entries: QuotaHistoryEntry[]; nextBeforeRunId: string | null }
export interface SavedQuotaWindow extends QuotaWindow { resetState: 'expected' | 'due' | 'unconfirmed' | 'confirmed' }
export interface ResetCreditDetail {
  title: string | null; status: 'available' | 'redeeming' | 'redeemed' | 'unknown';
  grantedAtUnix: number; expiresAtUnix: number | null;
}
export interface ProfileQuotaSavedRead extends Omit<ProfileQuotaRequest, 'accepted'> {
  accepted: true; bindingId: string; collectionRunId: string;
  providerVersion: string; observedAtUtc: string;
  identityConfidence: 'provisional'; verifiedIdentityKey: null;
  availableResetCredits: string | null;
  resetCreditDetails: ResetCreditDetail[] | null;
  limits: { limitId: string; limitName: string | null; primary: QuotaWindow | null; secondary: QuotaWindow | null; reachedType: string | null; monthlyCreditLimit: string | null; spendControlReached: boolean | null }[];
}
export interface SavedProfileQuota extends Omit<ProfileQuotaSavedRead, 'requestId' | 'approvalId' | 'limits'> {
  coverage: 'unknown'; scope: 'account_quota_with_unknown_application_scope'; freshness: ActivityFreshness;
  limits: (Omit<ProfileQuotaSavedRead['limits'][number], 'primary' | 'secondary'> & { primary: SavedQuotaWindow | null; secondary: SavedQuotaWindow | null })[];
}



export interface ProfileActivityRequest {
  profileId: string; requestId: string; approvalId: string;
  sourceKind: ProviderConnectionDiagnostic['sourceKind'];
  monitoring: MonitoringState; accepted: false;
}
export interface ProfileActivitySavedRead extends Omit<ProfileActivityRequest, 'accepted'> {
  accepted: true; collectionRunId: string; bindingId: string;
  providerVersion: string; observedAtUtc: string;
  identityConfidence: 'provisional'; verifiedIdentityKey: null;
  coverage: 'unknown'; scope: 'account_activity_with_unknown_application_scope';
  summary: { lifetimeTokens: string | null; peakDailyTokens: string | null; longestRunningTurnSec: string | null; currentStreakDays: string | null; longestStreakDays: string | null };
  dailyUsageBuckets: { startDate: string; tokens: string }[] | null;
}


export interface ProfileActivityAssociationReview {
  reviewId: string; profileId: string; approvalId: string; observedAtUtc: string;
  previous: { email: string; plan: string | null }; current: { email: string; plan: string | null };
  homeChanged: boolean; identityConfidence: 'provisional';
}

export type ActivityFreshness = 'unknown' | 'fresh' | 'stale';
export interface CombinedHeatmapEligibility { status: 'unavailable'; includedProfileIds: string[]; exclusions: { profileId: string; reason: 'not_connected' | 'activity_unavailable' | 'identity_unverified' }[] }
export type HeatmapMode = 'daily' | 'weekly' | 'cumulative';
export interface SavedProfileHeatmap {
  profileId: string; bindingId: string; collectionRunId: string;
  sourceKind: ProviderConnectionDiagnostic['sourceKind']; scope: 'account_activity_with_unknown_application_scope';
  coverage: 'unknown'; observedAtUtc: string; mode: HeatmapMode;
  periodStart: string; periodEnd: string; scaleMaximum: string | null;
  cells: { startDate: string; endDate: string; tokens: string | null; level: number;
    knownDays: number; expectedDays: number; observedAtUtc: string | null; freshness: ActivityFreshness }[];
}
export interface SavedProfileActivity {
  profileId: string; bindingId: string; collectionRunId: string;
  providerVersion: string; sourceKind: ProviderConnectionDiagnostic['sourceKind']; observedAtUtc: string;
  identityConfidence: 'provisional'; verifiedIdentityKey: null; monitoring: MonitoringState; accepted: true;
  coverage: 'unknown'; scope: 'account_activity_with_unknown_application_scope'; freshness: ActivityFreshness;
  summary: ProfileActivitySavedRead['summary']; dailyState: 'unavailable' | 'empty' | 'returned';
  dailyUsageBuckets: { date: string; tokens: string; observedAtUtc: string; freshness: ActivityFreshness }[];
  dailyTruncated: boolean;
  dailyChart: { maxTokens: string | null; bars: { date: string; heightBasisPoints: number }[] };
}

export interface ActivityHistoryEntry {
  profileId: string; bindingId: string; collectionRunId: string; observedAtUtc: string;
  sourceKind: 'official_artifact' | 'test_fixture'; providerVersion: string;
  sourceScope: 'account_activity_with_unknown_application_scope'; coverage: 'unknown';
  summary: ProfileActivitySavedRead['summary']; dailyBucketsState: 'unavailable' | 'empty' | 'returned';
}
export interface ActivityHistoryPage { profileId: string; entries: ActivityHistoryEntry[]; nextBeforeRunId: string | null }

interface Commands {
  load_combined_heatmap: { args: { profileIds: string[] }; result: CombinedHeatmapEligibility };
  load_profile_heatmap: { args: { profileId: string; mode: HeatmapMode; periodDays: 30 | 90 | 180 | 365 }; result: SavedProfileHeatmap | null };
  load_profile_activity_history: { args: { profileId: string; beforeRunId: string | null }; result: ActivityHistoryPage };
  load_profile_quota_history: { args: { profileId: string; beforeRunId: string | null }; result: QuotaHistoryPage };
  load_alert_history: { args: undefined; result: SavedAlertHistory };
  load_notification_preferences: { args: undefined; result: NotificationPreferences };
  save_notification_preferences: { args: { preferences: NotificationPreferences }; result: NotificationPreferences };
  load_reset_notification_preferences: { args: undefined; result: ResetNotificationPreferences };
  save_reset_notification_preferences: { args: { preferences: ResetNotificationPreferences }; result: ResetNotificationPreferences };
  get_display_scale: { args: undefined; result: { percent: number } };
  set_display_scale: { args: { percent: number }; result: { percent: number } };
  load_profile_plan: { args: { profileId: string; bindingId: string }; result: { profileId: string; bindingId: string; plan: string | null } };
  load_monitoring: { args: undefined; result: MonitoringStatus };
  start_profile_monitoring: { args: { profileId: string; approvalId: string }; result: MonitoringStatus };
  pause_profile_monitoring: { args: { profileId: string }; result: MonitoringStatus };
  begin_profile_collection_read: { args: { profileId: string; approvalId: string }; result: ProfileQuotaRequest };
  complete_profile_collection_read: { args: { profileId: string; requestId: string }; result: ProfileCollectionOutcome };
  cancel_profile_collection_read: { args: { profileId: string; requestId: string }; result: { cancelled: boolean } };
  review_profile_collection_association: { args: { profileId: string; approvalId: string }; result: ProfileActivityAssociationReview };
  begin_profile_collection_reassociation: { args: { profileId: string; approvalId: string; reviewId: string; confirmed: true }; result: ProfileQuotaRequest };
  load_profile_refresh_status: { args: { profileId: string }; result: ProfileRefreshStatus };
  load_profile_activity: { args: { profileId: string }; result: SavedProfileActivity | null };
  begin_profile_activity_read: { args: { profileId: string; approvalId: string }; result: ProfileActivityRequest };
  review_profile_activity_association: { args: { profileId: string; approvalId: string }; result: ProfileActivityAssociationReview };
  begin_profile_activity_reassociation: { args: { profileId: string; approvalId: string; reviewId: string; confirmed: true }; result: ProfileActivityRequest };
  complete_profile_activity_read: { args: { profileId: string; requestId: string }; result: ProfileActivitySavedRead };
  cancel_profile_activity_read: { args: { profileId: string; requestId: string }; result: { cancelled: boolean } };
  begin_profile_quota_read: { args: { profileId: string; approvalId: string }; result: ProfileQuotaRequest };
  complete_profile_quota_read: { args: { profileId: string; requestId: string }; result: ProfileQuotaSavedRead };
  load_profile_quota: { args: { profileId: string }; result: SavedProfileQuota | null };
  review_profile_quota_association: { args: { profileId: string; approvalId: string }; result: ProfileActivityAssociationReview };
  begin_profile_quota_reassociation: { args: { profileId: string; approvalId: string; reviewId: string; confirmed: true }; result: ProfileQuotaRequest };
  cancel_profile_quota_read: { args: { profileId: string; requestId: string }; result: { cancelled: boolean } };
  discover_provider_executables: { args: undefined; result: ProviderExecutableDiscovery };
  begin_profile_login: { args: { profileId: string; approvalId: string; flow: 'device_code'; confirmed: true }; result: ProviderLoginAttempt };
  await_profile_login_prompt: { args: { profileId: string; attemptId: string }; result: ProviderLoginPrompt };
  open_profile_login_page: { args: { profileId: string; attemptId: string }; result: null };
  complete_profile_login: { args: { profileId: string; attemptId: string }; result: ProviderLoginCompletion };
  cancel_profile_login: { args: { profileId: string; attemptId: string }; result: { cancelled: boolean } };
  inspect_provider_executable: { args: { path: string }; result: ProviderExecutableCandidate };
  prepare_provider_connection: { args: undefined; result: ProviderExecutableApproval };
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
  delete_all_local_data: { args: { confirmed: true }; result: Bootstrap };
  bootstrap: { args: undefined; result: Bootstrap };
  setup_workspace: { args: { passphrase: string | null }; result: Bootstrap };
  unlock_workspace: { args: { passphrase: string }; result: Bootstrap };
  set_theme: { args: { theme: Theme }; result: Bootstrap };
  create_backup: { args: { password: string }; result: BackupReceipt };
  export_saved_readings: { args: { profileIds: string[]; format: ReadingExportFormat; includeIdentifiers: boolean; confirmed: true }; result: ReadingExportReceipt };
  export_quota_history: { args: { profileId: string; format: ReadingExportFormat; includeIdentifiers: boolean; confirmed: true }; result: ReadingExportReceipt };
  export_activity_history: { args: { profileId: string; format: ReadingExportFormat; includeIdentifiers: boolean; confirmed: true }; result: ReadingExportReceipt };
  export_application_data: { args: { confirmed: true }; result: ApplicationExportReceipt };
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
  const valid = command === 'delete_all_local_data' ? isDeletionResult(response)
    : command === 'load_notification_preferences' || command === 'save_notification_preferences' ? isNotificationPreferences(response)
    : command === 'load_reset_notification_preferences' || command === 'save_reset_notification_preferences' ? isResetNotificationPreferences(response)
    : command === 'load_alert_history' ? isSavedAlertHistory(response)
    : command === 'load_profile_activity_history' ? isActivityHistoryPage(response)
    : command === 'load_profile_quota_history' ? isQuotaHistoryPage(response)
    : (command === 'get_display_scale' || command === 'set_display_scale') ? isDisplayScale(response)
    : command === 'load_profile_plan' ? isProfilePlan(response)
    : ['load_monitoring', 'start_profile_monitoring', 'pause_profile_monitoring'].includes(command) ? isMonitoringStatus(response)
    : (command === 'begin_profile_collection_read' || command === 'begin_profile_collection_reassociation') ? isProfileQuotaRequest(response)
    : command === 'complete_profile_collection_read' ? isProfileCollectionOutcome(response)
    : command === 'cancel_profile_collection_read' ? isPreviewCancellation(response)
    : command === 'review_profile_collection_association' ? isActivityAssociationReview(response)
    : (command === 'begin_profile_activity_read' || command === 'begin_profile_activity_reassociation') ? isProfileQuotaRequest(response)
    : command === 'load_profile_refresh_status' ? isProfileRefreshStatus(response)
    : command === 'review_profile_activity_association' ? isActivityAssociationReview(response)
    : command === 'complete_profile_activity_read' ? isProfileActivitySavedRead(response)
    : command === 'load_profile_activity' ? isSavedProfileActivity(response)
    : command === 'load_profile_heatmap' ? isSavedProfileHeatmap(response)
    : command === 'load_combined_heatmap' ? isCombinedHeatmapEligibility(response)
    : command === 'cancel_profile_activity_read' ? isPreviewCancellation(response)
    : (command === 'begin_profile_quota_read' || command === 'begin_profile_quota_reassociation') ? isProfileQuotaRequest(response)
    : command === 'complete_profile_quota_read' ? isProfileQuotaSavedRead(response)
    : command === 'load_profile_quota' ? isSavedProfileQuota(response)
    : command === 'review_profile_quota_association' ? isActivityAssociationReview(response)
    : command === 'cancel_profile_quota_read' ? isPreviewCancellation(response)
    : command === 'discover_provider_executables' ? isProviderExecutableDiscovery(response)
    : command === 'begin_profile_login' ? isProviderLoginAttempt(response)
    : command === 'await_profile_login_prompt' ? isProviderLoginPrompt(response)
    : command === 'open_profile_login_page' ? response === null
    : command === 'complete_profile_login' ? isProviderLoginCompletion(response)
    : command === 'cancel_profile_login' ? isPreviewCancellation(response)
    : command === 'inspect_provider_executable' ? isProviderExecutableReview(response, 'candidateId')
    : command === 'prepare_provider_connection' ? isProviderExecutableReview(response, 'approvalId') && (response as ProviderExecutableApproval).sourceKind === 'official_artifact'
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
    : command === 'export_application_data' ? isApplicationExportReceipt(response)
    : command === 'export_saved_readings' || command === 'export_quota_history' || command === 'export_activity_history' ? isReadingExportReceipt(response) && (command === 'export_saved_readings' || response.profileCount === 1)
    : ['list_profiles', 'create_profile', 'rename_profile', 'set_profile_archived'].includes(command)
    ? isProfileView(response) : isBootstrap(response);
  if (!valid) {
    throw new Error('The desktop response is incompatible. Restart OpenUsage after updating the application.');
  }
  return response as Commands[K]['result'];
}

function isResetNotificationPreferences(value: unknown): value is ResetNotificationPreferences {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return false;
  const prefs = value as Record<string, unknown>;
  if (Object.keys(prefs).length !== 6 || typeof prefs.enabled !== 'boolean' || typeof prefs.quietEnabled !== 'boolean'
    || typeof prefs.timeZone !== 'string' || !/^[A-Za-z0-9_+./-]{1,128}$/.test(prefs.timeZone)
    || typeof prefs.quietStart !== 'string' || !/^(?:[01]\d|2[0-3]):[0-5]\d$/.test(prefs.quietStart)
    || typeof prefs.quietEnd !== 'string' || !/^(?:[01]\d|2[0-3]):[0-5]\d$/.test(prefs.quietEnd) || prefs.quietStart === prefs.quietEnd
    || !Array.isArray(prefs.profileOverrides) || prefs.profileOverrides.length > 100) return false;
  const ids = new Set<string>();
  for (const value of prefs.profileOverrides) {
    if (!value || typeof value !== 'object' || Array.isArray(value)) return false;
    const item = value as Record<string, unknown>;
    if (Object.keys(item).length !== 2 || !isUuid(item.profileId) || ids.has(item.profileId) || typeof item.enabled !== 'boolean') return false;
    ids.add(item.profileId);
  }
  return true;
}

function isNotificationPreferences(value: unknown): value is NotificationPreferences {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return false;
  const prefs = value as Record<string, unknown>;
  if (Object.keys(prefs).length !== 4 || !isResetNotificationPreferences(prefs.reset)) return false;
  return ['usage', 'authentication', 'collectionFailure'].every(key => {
    const value = prefs[key];
    if (!value || typeof value !== 'object' || Array.isArray(value)) return false;
    const preference = value as Record<string, unknown>;
    return Object.keys(preference).length === 2 && typeof preference.enabled === 'boolean'
      && isResetNotificationPreferences({ ...(prefs.reset as ResetNotificationPreferences), enabled: preference.enabled, profileOverrides: preference.profileOverrides });
  });
}

function isSavedAlertHistory(value: unknown): value is SavedAlertHistory {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return false;
  const history = value as Record<string, unknown>;
  if (Object.keys(history).length !== 2 || !Array.isArray(history.events) || history.events.length > 100 || typeof history.truncated !== 'boolean'
    || (history.truncated && history.events.length !== 100)) return false;
  const ids = new Set<string>();
  for (const item of history.events) {
    if (!item || typeof item !== 'object' || Array.isArray(item)) return false;
    const event = item as Record<string, unknown>;
    if (!isUuid(event.id) || ids.has(event.id) || !isUuid(event.profileId)
      || typeof event.profileLabel !== 'string' || Array.from(event.profileLabel).length < 1 || Array.from(event.profileLabel).length > 100
      || /[\u0000-\u001f\u007f-\u009f]/u.test(event.profileLabel)
      || !isUtcTimestamp(event.evidenceAtUtc) || (event.sourceKind !== 'official_artifact' && event.sourceKind !== 'test_fixture')
      || typeof event.deliveryState !== 'string' || !['recorded', 'queued', 'submitted', 'failed', 'suppressed', 'uncertain', 'covered'].includes(event.deliveryState)) return false;
    if (event.kind === 'authentication_required' || event.kind === 'collection_failure') {
      if (Object.keys(event).length !== 7 || event.deliveryState === 'covered') return false;
    } else if (event.kind === 'quota_reset' || event.kind === 'quota_threshold') {
      if (Object.keys(event).length !== 12 || !isUuid(event.evidenceRunId)
        || (event.kind === 'quota_reset' ? event.thresholdPercent !== null || event.deliveryState === 'covered' : ![80, 90, 100].includes(event.thresholdPercent as number) || (event.thresholdPercent === 100 && event.deliveryState === 'covered'))
        || !quotaText(event.limitId, 128) || (event.windowRole !== 'primary' && event.windowRole !== 'secondary')
        || !Number.isSafeInteger(event.windowDurationMins) || (event.windowDurationMins as number) < 1) return false;
    } else return false;
    ids.add(event.id);
  }
  return true;
}

function isActivityHistoryPage(value: unknown): value is ActivityHistoryPage {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return false;
  const page = value as Record<string, unknown>;
  if (Object.keys(page).length !== 3 || !isUuid(page.profileId) || !Array.isArray(page.entries) || page.entries.length > 20
    || !(page.nextBeforeRunId === null || isUuid(page.nextBeforeRunId))) return false;
  const ids = new Set<string>();
  let previous: { observedAtUtc: string; collectionRunId: string } | null = null;
  for (const value of page.entries) {
    if (!value || typeof value !== 'object' || Array.isArray(value)) return false;
    const row = value as Record<string, unknown>;
    if (Object.keys(row).length !== 10 || row.profileId !== page.profileId || !isUuid(row.bindingId) || !isUuid(row.collectionRunId)
      || ids.has(row.collectionRunId) || !isUtcTimestamp(row.observedAtUtc) || !quotaText(row.providerVersion, 80)
      || (row.sourceKind !== 'official_artifact' && row.sourceKind !== 'test_fixture')
      || row.sourceScope !== 'account_activity_with_unknown_application_scope' || row.coverage !== 'unknown'
      || !isActivitySummary(row.summary) || !['unavailable', 'empty', 'returned'].includes(row.dailyBucketsState as string)) return false;
    if (previous && (previous.observedAtUtc < row.observedAtUtc
      || previous.observedAtUtc === row.observedAtUtc && previous.collectionRunId <= row.collectionRunId)) return false;
    previous = { observedAtUtc: row.observedAtUtc as string, collectionRunId: row.collectionRunId as string };
    ids.add(row.collectionRunId);
  }
  return page.nextBeforeRunId === null || page.entries.length === 20 && page.nextBeforeRunId === (page.entries.at(-1) as ActivityHistoryEntry).collectionRunId;
}

function isQuotaHistoryPage(value: unknown): value is QuotaHistoryPage {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return false;
  const page = value as Record<string, unknown>;
  if (Object.keys(page).length !== 3 || !isUuid(page.profileId) || !Array.isArray(page.entries) || page.entries.length > 20
    || !(page.nextBeforeRunId === null || isUuid(page.nextBeforeRunId))) return false;
  const ids = new Set<string>();
  let previous: { observedAtUtc: string; collectionRunId: string } | null = null;
  for (const entry of page.entries) {
    if (!entry || typeof entry !== 'object' || Array.isArray(entry)) return false;
    const row = entry as Record<string, unknown>;
    if (Object.keys(row).length !== 7 || row.profileId !== page.profileId || !isUuid(row.bindingId) || !isUuid(row.collectionRunId)
      || ids.has(row.collectionRunId) || !isUtcTimestamp(row.observedAtUtc) || !quotaText(row.providerVersion, 80)
      || (row.sourceKind !== 'official_artifact' && row.sourceKind !== 'test_fixture') || !Array.isArray(row.limits) || row.limits.length > 64) return false;
    if (previous && (row.observedAtUtc > previous.observedAtUtc || (row.observedAtUtc === previous.observedAtUtc && row.collectionRunId >= previous.collectionRunId))) return false;
    previous = { observedAtUtc: row.observedAtUtc, collectionRunId: row.collectionRunId };
    ids.add(row.collectionRunId);
    const limits = new Set<string>();
    for (const limit of row.limits) {
      if (!limit || typeof limit !== 'object' || Array.isArray(limit)) return false;
      const item = limit as Record<string, unknown>;
      if (Object.keys(item).length !== 4 || !quotaText(item.limitId, 128) || limits.has(item.limitId)
        || !(item.limitName === null || quotaText(item.limitName, 256)) || !isQuotaWindow(item.primary) || !isQuotaWindow(item.secondary)) return false;
      limits.add(item.limitId);
    }
  }
  return page.nextBeforeRunId === null || (page.entries.length === 20 && page.nextBeforeRunId === page.entries.at(-1)?.collectionRunId);
}

function isDisplayScale(value: unknown): boolean {
  if (!value || typeof value !== 'object') return false;
  const record = value as Record<string, unknown>;
  return Object.keys(record).length === 1 && [90, 100, 110, 125, 150, 175, 200].includes(record.percent as number);
}
function isProfilePlan(value: unknown): boolean {
  if (!value || typeof value !== 'object') return false;
  const record = value as Record<string, unknown>;
  return Object.keys(record).length === 3 && isUuid(record.profileId) && isUuid(record.bindingId)
    && (record.plan === null || (typeof record.plan === 'string' && record.plan.length > 0 && record.plan.length <= 64 && !/\p{Cc}/u.test(record.plan)));
}

function isMonitoringState(value: unknown): value is MonitoringState {
  return value === 'stopped' || value === 'running' || value === 'paused';
}
function isMonitoringStatus(value: unknown): value is MonitoringStatus {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return false;
  const v = value as Record<string, unknown>;
  if (Object.keys(v).length !== 3 || !isMonitoringState(v.state) || !isUuid(v.readingRevision) || !Array.isArray(v.profiles) || v.profiles.length > 10) return false;
  const ids = new Set<string>();
  return v.profiles.every(value => {
    if (typeof value !== 'object' || value === null || Array.isArray(value)) return false;
    const p = value as Record<string, unknown>;
    if (Object.keys(p).length !== 6 || !isUuid(p.profileId) || ids.has(p.profileId) || !isMonitoringState(p.state)
      || typeof p.eligible !== 'boolean' || typeof p.readingActive !== 'boolean'
      || !(p.issue === null || ['sign_in_required', 'account_changed', 'source_changed', 'profile_unavailable', 'unsupported', 'storage_unavailable', 'provider_error'].includes(p.issue as string))
      || !(p.nextReadInSeconds === null || (Number.isInteger(p.nextReadInSeconds) && Number(p.nextReadInSeconds) >= 0 && Number(p.nextReadInSeconds) <= 4320))) return false;
    ids.add(p.profileId); return true;
  });
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

function isActivityAssociationReview(value: unknown): value is ProfileActivityAssociationReview {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return false;
  const v = value as Record<string, unknown>;
  const descriptor = (value: unknown): boolean => {
    if (typeof value !== 'object' || value === null || Array.isArray(value)) return false;
    const d = value as Record<string, unknown>;
    const text = (value: unknown, maximum: number) => quotaText(value, maximum) && new TextEncoder().encode(value as string).length <= maximum;
    return Object.keys(d).length === 2 && text(d.email, 254) && (d.plan === null || text(d.plan, 64));
  };
  return Object.keys(v).length === 8 && isUuid(v.reviewId) && isUuid(v.profileId) && isUuid(v.approvalId)
    && isUtcTimestamp(v.observedAtUtc) && descriptor(v.previous) && descriptor(v.current)
    && typeof v.homeChanged === 'boolean' && v.identityConfidence === 'provisional';
}
function isProfileRefreshStatus(value: unknown): value is ProfileRefreshStatus {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return false;
  const v = value as Record<string, unknown>;
  return Object.keys(v).length === 2 && isUuid(v.profileId)
    && Number.isInteger(v.retryAfterSeconds) && Number(v.retryAfterSeconds) >= 0 && Number(v.retryAfterSeconds) <= 4320;
}

function isProfileCollectionOutcome(value: unknown): value is ProfileCollectionOutcome {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return false;
  const v = value as Record<string, unknown>;
  const reading = (value: unknown): value is CollectionReading | null => {
    if (value === null) return true;
    if (typeof value !== 'object' || Array.isArray(value)) return false;
    const r = value as Record<string, unknown>;
    return Object.keys(r).length === 3 && isUuid(r.bindingId) && isUuid(r.collectionRunId) && isUtcTimestamp(r.observedAtUtc);
  };
  if (Object.keys(v).length !== 11 || !isQuotaBinding(v, v.accepted === true)
    || !quotaText(v.providerVersion, 80) || v.identityConfidence !== 'provisional' || v.verifiedIdentityKey !== null
    || !reading(v.quota) || !reading(v.activity) || v.accepted !== (v.quota !== null || v.activity !== null)) return false;
  return !(v.quota && v.activity) || (v.quota.bindingId === v.activity.bindingId && v.quota.collectionRunId !== v.activity.collectionRunId);
}

function isProfileQuotaRequest(value: unknown): value is ProfileQuotaRequest {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return false;
  const v = value as Record<string, unknown>;
  return Object.keys(v).length === 6 && isQuotaBinding(v);
}
function isQuotaBinding(v: Record<string, unknown>, accepted: false | true = false): boolean {
  return isUuid(v.profileId) && isUuid(v.requestId) && isUuid(v.approvalId)
    && (v.sourceKind === 'official_artifact' || v.sourceKind === 'test_fixture')
    && isMonitoringState(v.monitoring) && v.accepted === accepted;
}
function quotaText(value: unknown, maximum: number): value is string {
  return typeof value === 'string' && value.length <= maximum && value.trim().length > 0 && !/\p{Cc}/u.test(value);
}
function isQuotaWindow(value: unknown, saved = false): value is QuotaWindow | null {
  if (value === null) return true;
  if (typeof value !== 'object' || Array.isArray(value)) return false;
  const v = value as Record<string, unknown>;
  return Object.keys(v).length === (saved ? 6 : 5)
    && (!saved || v.resetState === 'expected' || v.resetState === 'due' || v.resetState === 'unconfirmed' || v.resetState === 'confirmed')
    && Number.isInteger(v.usedPercent) && Number(v.usedPercent) >= 0 && Number(v.usedPercent) <= 100
    && Number.isInteger(v.derivedRemainingPercent) && Number(v.derivedRemainingPercent) >= 0 && Number(v.derivedRemainingPercent) <= 100
    && (v.windowDurationMins === null || (Number.isSafeInteger(v.windowDurationMins) && Number(v.windowDurationMins) > 0))
    && (v.resetsAtUnix === null || (Number.isSafeInteger(v.resetsAtUnix) && Number(v.resetsAtUnix) >= -62135596800 && Number(v.resetsAtUnix) <= 253402300799))
    && quotaText(v.label, 128);
}
function isResetCreditCount(value: unknown): value is string | null {
  return value === null || (typeof value === 'string' && value.trim() === value && /^(0|[1-9][0-9]{0,18})$/.test(value)
    && (value.length < 19 || value <= '9223372036854775807'));
}
function isResetCreditDetails(value: unknown): value is ResetCreditDetail[] | null {
  if (value === null) return true;
  if (!Array.isArray(value) || value.length > 32) return false;
  return value.every(row => {
    if (typeof row !== 'object' || row === null || Array.isArray(row)) return false;
    const v = row as Record<string, unknown>;
    return Object.keys(v).length === 4
      && (v.title === null || (quotaText(v.title, 256) && new TextEncoder().encode(v.title).length <= 256))
      && typeof v.status === 'string' && ['available', 'redeeming', 'redeemed', 'unknown'].includes(v.status)
      && Number.isSafeInteger(v.grantedAtUnix) && Number(v.grantedAtUnix) >= -62135596800 && Number(v.grantedAtUnix) <= 253402300799
      && (v.expiresAtUnix === null || (Number.isSafeInteger(v.expiresAtUnix) && Number(v.expiresAtUnix) >= -62135596800 && Number(v.expiresAtUnix) <= 253402300799));
  });
}
function isProfileQuotaSavedRead(value: unknown): value is ProfileQuotaSavedRead {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return false;
  const v = value as Record<string, unknown>;
  return Object.keys(v).length === 15 && isQuotaBinding(v, true) && isUuid(v.bindingId) && isUuid(v.collectionRunId) && isQuotaValues(v);
}
function isSavedProfileQuota(value: unknown): value is SavedProfileQuota | null {
  if (value === null) return true;
  if (typeof value !== 'object' || Array.isArray(value)) return false;
  const v = value as Record<string, unknown>;
  return Object.keys(v).length === 16 && isUuid(v.profileId) && isUuid(v.bindingId) && isUuid(v.collectionRunId)
    && (v.sourceKind === 'official_artifact' || v.sourceKind === 'test_fixture') && v.accepted === true && isMonitoringState(v.monitoring)
    && v.coverage === 'unknown' && v.scope === 'account_quota_with_unknown_application_scope' && isActivityFreshness(v.freshness) && isQuotaValues(v, true);
}
function isQuotaValues(v: Record<string, unknown>, saved = false): boolean {
  if (!quotaText(v.providerVersion, 80) || !isUtcTimestamp(v.observedAtUtc)
    || v.identityConfidence !== 'provisional' || v.verifiedIdentityKey !== null || !isResetCreditCount(v.availableResetCredits) || !isResetCreditDetails(v.resetCreditDetails) || !Array.isArray(v.limits) || v.limits.length > 64) return false;
  const ids = new Set<string>();
  return v.limits.every(value => {
    if (typeof value !== 'object' || value === null || Array.isArray(value)) return false;
    const limit = value as Record<string, unknown>;
    if (Object.keys(limit).length !== 7 || !quotaText(limit.limitId, 128) || ids.has(limit.limitId)
      || !(limit.limitName === null || quotaText(limit.limitName, 256))
      || !(limit.reachedType === null || quotaText(limit.reachedType, 128))
      || !(limit.monthlyCreditLimit === null || (quotaText(limit.monthlyCreditLimit, 256) && new TextEncoder().encode(limit.monthlyCreditLimit).length <= 256))
      || !(limit.spendControlReached === null || typeof limit.spendControlReached === 'boolean')
      || !isQuotaWindow(limit.primary, saved) || !isQuotaWindow(limit.secondary, saved)) return false;
    ids.add(limit.limitId); return true;
  });
}


function isActivitySummary(value: unknown): boolean {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return false;
  const summary = value as Record<string, unknown>;
  return Object.keys(summary).length === 5 && ['lifetimeTokens', 'peakDailyTokens', 'longestRunningTurnSec', 'currentStreakDays', 'longestStreakDays'].every(key => isResetCreditCount(summary[key]));
}
function isActivitySource(v: Record<string, unknown>): boolean {
  return quotaText(v.providerVersion, 80) && isUtcTimestamp(v.observedAtUtc) && v.identityConfidence === 'provisional'
    && v.verifiedIdentityKey === null && v.coverage === 'unknown' && v.scope === 'account_activity_with_unknown_application_scope'
    && isActivitySummary(v.summary);
}
function isProviderDate(value: unknown): value is string {
  if (typeof value !== 'string' || !/^(?!0000)[0-9]{4}-[0-9]{2}-[0-9]{2}$/.test(value)) return false;
  const date = new Date(value + 'T00:00:00.000Z');
  return Number.isFinite(date.getTime()) && date.toISOString().slice(0, 10) === value;
}
function isActivityFreshness(value: unknown): value is ActivityFreshness { return value === 'unknown' || value === 'fresh' || value === 'stale'; }
function isProfileActivitySavedRead(value: unknown): value is ProfileActivitySavedRead {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return false;
  const v = value as Record<string, unknown>;
  if (Object.keys(v).length !== 16 || !isQuotaBinding(v, true) || !isActivitySource(v) || !isUuid(v.collectionRunId) || !isUuid(v.bindingId)) return false;
  if (v.dailyUsageBuckets === null) return true;
  if (!Array.isArray(v.dailyUsageBuckets) || v.dailyUsageBuckets.length > 4096) return false;
  const dates = new Set<string>();
  return v.dailyUsageBuckets.every(row => {
    if (typeof row !== 'object' || row === null || Array.isArray(row)) return false;
    const bucket = row as Record<string, unknown>;
    if (Object.keys(bucket).length !== 2 || !isProviderDate(bucket.startDate) || dates.has(bucket.startDate)
      || !isResetCreditCount(bucket.tokens) || bucket.tokens === null) return false;
    dates.add(bucket.startDate); return true;
  });
}
function isCombinedHeatmapEligibility(value: unknown): value is CombinedHeatmapEligibility {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return false;
  const v = value as Record<string, unknown>;
  if (Object.keys(v).length !== 3 || v.status !== 'unavailable' || !Array.isArray(v.includedProfileIds) || v.includedProfileIds.length !== 0 || !Array.isArray(v.exclusions) || v.exclusions.length > 10) return false;
  const seen = new Set();
  return v.exclusions.every(value => {
    if (typeof value !== 'object' || value === null || Array.isArray(value)) return false;
    const e = value as Record<string, unknown>;
    if (Object.keys(e).length !== 2 || !isUuid(e.profileId) || seen.has(e.profileId) || !['not_connected','activity_unavailable','identity_unverified'].includes(String(e.reason))) return false;
    seen.add(e.profileId); return true;
  });
}

function isSavedProfileHeatmap(value: unknown): value is SavedProfileHeatmap | null {
  if (value === null) return true;
  if (typeof value !== 'object' || Array.isArray(value)) return false;
  const v = value as Record<string, unknown>;
  const count = (value: unknown): value is string | null => value === null || typeof value === 'string' && /^(0|[1-9][0-9]{0,38})$/.test(value) && (value.length < 39 || value <= '340282366920938463463374607431768211455');
  if (Object.keys(v).length !== 12 || !isUuid(v.profileId) || !isUuid(v.bindingId) || !isUuid(v.collectionRunId)
    || !['official_artifact', 'test_fixture'].includes(String(v.sourceKind)) || v.coverage !== 'unknown'
    || v.scope !== 'account_activity_with_unknown_application_scope' || !isUtcTimestamp(v.observedAtUtc)
    || !['daily', 'weekly', 'cumulative'].includes(String(v.mode)) || !isProviderDate(v.periodStart) || !isProviderDate(v.periodEnd)
    || v.periodStart > v.periodEnd || !count(v.scaleMaximum) || !Array.isArray(v.cells) || v.cells.length > 365) return false;
  let last = '';
  return v.cells.every(value => {
    if (typeof value !== 'object' || value === null || Array.isArray(value)) return false;
    const c = value as Record<string, unknown>;
    if (Object.keys(c).length !== 8 || !isProviderDate(c.startDate) || !isProviderDate(c.endDate) || c.startDate > c.endDate || c.startDate <= last
      || !count(c.tokens) || !Number.isInteger(c.level) || Number(c.level) < 0 || Number(c.level) > 4
      || !Number.isInteger(c.knownDays) || !Number.isInteger(c.expectedDays) || Number(c.knownDays) < 0 || Number(c.expectedDays) < 1
      || Number(c.expectedDays) > 365 || Number(c.knownDays) > Number(c.expectedDays)
      || !(c.observedAtUtc === null || isUtcTimestamp(c.observedAtUtc)) || !isActivityFreshness(c.freshness)
      || (c.tokens === null ? c.level !== 0 || c.knownDays !== 0 : c.knownDays === 0 || c.observedAtUtc === null)
      || (c.tokens === '0' && c.level !== 0) || (c.tokens !== null && c.tokens !== '0' && c.level === 0)) return false;
    last = c.startDate;
    return true;
  });
}

function isSavedProfileActivity(value: unknown): value is SavedProfileActivity | null {
  if (value === null) return true;
  if (typeof value !== 'object' || Array.isArray(value)) return false;
  const v = value as Record<string, unknown>;
  if (Object.keys(v).length !== 18 || !isUuid(v.profileId) || !isUuid(v.bindingId) || !isUuid(v.collectionRunId)
    || !(v.sourceKind === 'official_artifact' || v.sourceKind === 'test_fixture') || !isMonitoringState(v.monitoring) || v.accepted !== true
    || !isActivitySource(v) || !isActivityFreshness(v.freshness) || !(v.dailyState === 'unavailable' || v.dailyState === 'empty' || v.dailyState === 'returned')
    || typeof v.dailyTruncated !== 'boolean' || !Array.isArray(v.dailyUsageBuckets) || v.dailyUsageBuckets.length > 4096) return false;
  const dates = new Set<string>();
  if (!v.dailyUsageBuckets.every(row => {
    if (typeof row !== 'object' || row === null || Array.isArray(row)) return false;
    const bucket = row as Record<string, unknown>;
    if (Object.keys(bucket).length !== 4 || !isProviderDate(bucket.date) || dates.has(bucket.date) || !isResetCreditCount(bucket.tokens)
      || bucket.tokens === null || !isUtcTimestamp(bucket.observedAtUtc) || !isActivityFreshness(bucket.freshness)) return false;
    dates.add(bucket.date); return true;
  })) return false;
  if (typeof v.dailyChart !== 'object' || v.dailyChart === null || Array.isArray(v.dailyChart)) return false;
  const chart = v.dailyChart as Record<string, unknown>;
  if (Object.keys(chart).length !== 2 || !isResetCreditCount(chart.maxTokens) || !Array.isArray(chart.bars)
    || chart.bars.length !== v.dailyUsageBuckets.length) return false;
  const buckets = v.dailyUsageBuckets as SavedProfileActivity['dailyUsageBuckets'];
  const maximum = chart.maxTokens;
  if (buckets.length === 0) return maximum === null;
  if (maximum === null || !buckets.some(bucket => bucket.tokens === maximum)) return false;
  return chart.bars.every((value, index) => {
    if (typeof value !== 'object' || value === null || Array.isArray(value)) return false;
    const bar = value as Record<string, unknown>, bucket = buckets[index]!;
    if (Object.keys(bar).length !== 2 || !isProviderDate(bar.date) || bar.date !== bucket.date
      || !Number.isInteger(bar.heightBasisPoints) || Number(bar.heightBasisPoints) < 0 || Number(bar.heightBasisPoints) > 10000
      || bucket.tokens.length > maximum.length || (bucket.tokens.length === maximum.length && bucket.tokens > maximum)) return false;
    if (bucket.tokens === '0') return bar.heightBasisPoints === 0;
    return bucket.tokens !== maximum || bar.heightBasisPoints === 10000;
  });
}

function isProviderExecutableDiscovery(value: unknown): value is ProviderExecutableDiscovery {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return false;
  const v = value as Record<string, unknown>;
  if (Object.keys(v).length !== 2 || typeof v.truncated !== 'boolean'
    || !Array.isArray(v.candidates) || v.candidates.length > 16) return false;
  const paths = new Set<string>();
  return v.candidates.every(item => {
    if (typeof item !== 'object' || item === null || Array.isArray(item)) return false;
    const c = item as Record<string, unknown>;
    if (Object.keys(c).length !== 2 || typeof c.path !== 'string' || !c.path.startsWith('/')
      || c.path.length > 4096 || /\p{Cc}/u.test(c.path) || paths.has(c.path)
      || typeof c.location !== 'string' || !['path', 'standard', 'npm_package'].includes(c.location)) return false;
    paths.add(c.path);
    return true;
  });
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
    && isMonitoringState(v.monitoring) && v.persisted === false
    && (v.sourceKind === 'official_artifact' || v.sourceKind === 'test_fixture');
}

function isProviderLoginAttempt(value: unknown): value is ProviderLoginAttempt {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return false;
  const v = value as Record<string, unknown>;
  return !Object.keys(v).some(key => !['profileId', 'attemptId', 'approvalId', 'sourceKind', 'monitoring', 'applicationWaitLimitSeconds'].includes(key))
    && isUuid(v.profileId) && isUuid(v.attemptId) && isUuid(v.approvalId)
    && (v.sourceKind === 'official_artifact' || v.sourceKind === 'test_fixture')
    && isMonitoringState(v.monitoring) && v.applicationWaitLimitSeconds === 900;
}

function isProviderLoginPrompt(value: unknown): value is ProviderLoginPrompt {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return false;
  const v = value as Record<string, unknown>;
  return !Object.keys(v).some(key => !['profileId', 'attemptId', 'verificationUrl', 'userCode', 'sourceKind', 'applicationWaitLimitSeconds'].includes(key))
    && isUuid(v.profileId) && isUuid(v.attemptId)
    && v.verificationUrl === 'https://auth.openai.com/codex/device'
    && typeof v.userCode === 'string' && v.userCode.length > 0 && v.userCode.length <= 64
    && /^[\x20-\x7e]+$/.test(v.userCode) && v.userCode.trim().length > 0
    && (v.sourceKind === 'official_artifact' || v.sourceKind === 'test_fixture')
    && v.applicationWaitLimitSeconds === 900;
}

function isProviderLoginDiagnostic(value: unknown): value is ProviderLoginDiagnostic {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return false;
  const v = value as Record<string, unknown>;
  return isProviderConnectionDiagnostic({ ...v, persisted: false }) && v.persisted === true
    && v.authentication === 'chatgpt' && isUtcTimestamp(v.checkedAt);
}

function isProviderLoginCompletion(value: unknown): value is ProviderLoginCompletion {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return false;
  const v = value as Record<string, unknown>;
  return Object.keys(v).length === 2 && isUuid(v.attemptId)
    && isProviderLoginDiagnostic(v.diagnostic);
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

function isReadingExportReceipt(value: unknown): value is ReadingExportReceipt {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return false;
  const v = value as Record<string, unknown>;
  return Object.keys(v).length === 7
    && Object.keys(v).every(key => ['fileName', 'directory', 'format', 'profileCount', 'rowCount', 'identifiersIncluded', 'exportedAt'].includes(key))
    && (v.format === 'csv' || v.format === 'json')
    && typeof v.fileName === 'string' && !/[\u0000-\u001f\u007f]/.test(v.fileName)
    && new RegExp(`^openusage-readings-[A-Za-z0-9_-]{16,64}\\.${v.format}$`).test(v.fileName)
    && typeof v.directory === 'string' && v.directory.length > 0 && v.directory.length <= 32768 && !/[\u0000-\u001f\u007f]/.test(v.directory)
    && Number.isInteger(v.profileCount) && Number(v.profileCount) >= 1 && Number(v.profileCount) <= 10
    && Number.isInteger(v.rowCount) && Number(v.rowCount) >= 0 && Number(v.rowCount) <= 50000
    && typeof v.identifiersIncluded === 'boolean' && isUtcTimestamp(v.exportedAt);
}

function isApplicationExportReceipt(value: unknown): value is ApplicationExportReceipt {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return false;
  const v = value as Record<string, unknown>;
  return Object.keys(v).length === 8
    && Object.keys(v).every(key => ['fileName', 'directory', 'format', 'schemaVersion', 'tableCount', 'recordCount', 'byteCount', 'exportedAt'].includes(key))
    && v.format === 'json'
    && typeof v.fileName === 'string' && /^openusage-application-[A-Za-z0-9]{16}\.json$/.test(v.fileName)
    && typeof v.directory === 'string' && v.directory.length > 0 && v.directory.length <= 32768 && !/[\u0000-\u001f\u007f]/.test(v.directory)
    && Number.isSafeInteger(v.schemaVersion) && Number(v.schemaVersion) > 0
    && Number.isSafeInteger(v.tableCount) && Number(v.tableCount) > 0 && Number(v.tableCount) <= 1000
    && isWideCount(v.recordCount) && BigInt(v.recordCount) <= 18446744073709551615n
    && isByteCount(v.byteCount) && BigInt(v.byteCount) > 0n && BigInt(v.byteCount) <= 5368709120n
    && isUtcTimestamp(v.exportedAt);
}

function isBackupReceipt(value: unknown): value is BackupReceipt {
  if (typeof value !== 'object' || value === null) return false;
  const receipt = value as Record<string, unknown>;
  return typeof receipt.fileName === 'string' && receipt.fileName.length > 0
    && receipt.fileName.length <= 160 && !/[\\/\0]/.test(receipt.fileName)
    && typeof receipt.directory === 'string' && receipt.directory.length > 0
    && receipt.directory.length <= 32768 && !receipt.directory.includes('\0');
}

function isDeletionResult(value: unknown): value is Bootstrap {
  return isBootstrap(value) && Object.keys(value).length === 8
    && value.storage === 'setup_required' && value.schemaVersion === null && value.activeProfiles === null
    && value.monitoring === 'stopped' && value.theme === 'system'
    && (value.storageIssue === null || value.storageIssue === 'key_store_unavailable');
}

function isBootstrap(value: unknown): value is Bootstrap {
  if (typeof value !== 'object' || value === null) return false;
  const v = value as Record<string, unknown>;
  return typeof v.storage === 'string' && ['setup_required', 'locked', 'ready', 'recovery_required'].includes(v.storage)
    && typeof v.theme === 'string' && ['system', 'light', 'dark'].includes(v.theme)
    && (v.schemaVersion === null || (Number.isSafeInteger(v.schemaVersion) && Number(v.schemaVersion) > 0))
    && (v.activeProfiles === null || (Number.isSafeInteger(v.activeProfiles) && Number(v.activeProfiles) >= 0 && Number(v.activeProfiles) <= 10))
    && isMonitoringState(v.monitoring) && v.provider === 'not_configured'
    && typeof v.keyProtection === 'string'
    && ['os_store', 'passphrase', 'passphrase_required', 'unavailable'].includes(v.keyProtection)
    && (v.storageIssue === null || (typeof v.storageIssue === 'string'
      && ['key_store_unavailable', 'missing_key', 'missing_database', 'invalid_key', 'storage_unavailable', 'workspace_in_use', 'local_deletion_incomplete'].includes(v.storageIssue)))
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
