import { expect, test, type Page } from '@playwright/test';
import { navigate, openReadings } from './navigation';
import type { MonitoringStatus, ProfileCollectionOutcome, SavedProfileActivity, SavedProfileQuota } from '../../apps/desktop/src/contracts';

// Synthetic renderer IPC only. Atomic storage and owned provider execution require native acceptance.
const profileId = '11111111-1111-4111-8111-111111111111';
const approvalId = '22222222-2222-4222-8222-222222222222';
const requests = ['33333333-3333-4333-8333-333333333333', '44444444-4444-4444-8444-444444444444'];
const bindingId = '55555555-5555-4555-8555-555555555555';
const quotaRun = '66666666-6666-4666-8666-666666666666';
const activityRun = '77777777-7777-4777-8777-777777777777';
const nextQuotaRun = '88888888-8888-4888-8888-888888888888';
const nextActivityRun = '99999999-9999-4999-8999-999999999999';
const reviewId = 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa';
const observedAtUtc = '2026-10-20T00:00:00.000Z';
type Deferred = { resolve: (value: unknown) => void; reject: (error: string) => void };
type Fixture = {
  calls: { command: string; args: Record<string, unknown> }[];
  completions: Record<string, Deferred>; cancellations: Record<string, Deferred>;
  quota: SavedProfileQuota; activity: SavedProfileActivity; failLoads: boolean;
  approvalId: string; refreshSeconds: number; monitoring: MonitoringStatus; deferMonitoring: boolean; pendingMonitoring: Deferred[]; failMonitoring: boolean;
  connectionState?: 'connected' | 'authentication_required';
};
type FixtureWindow = Window & { collectionFixture: Fixture };
function cachedQuota(): SavedProfileQuota {
  return { profileId, bindingId, collectionRunId: quotaRun, providerVersion: '0.163.0', sourceKind: 'test_fixture', observedAtUtc, identityConfidence: 'provisional', verifiedIdentityKey: null, monitoring: 'stopped', accepted: true, coverage: 'unknown', scope: 'account_quota_with_unknown_application_scope', freshness: 'fresh', availableResetCredits: null, resetCreditDetails: null,
    limits: [{ limitId: 'codex', limitName: 'Synthetic quota', primary: { usedPercent: 25, derivedRemainingPercent: 75, windowDurationMins: 300, resetsAtUnix: null, label: 'Five-hour', resetState: 'unconfirmed' }, secondary: null, reachedType: null, monthlyCreditLimit: null, spendControlReached: null }] };
}
function cachedActivity(): SavedProfileActivity {
  return { profileId, bindingId, collectionRunId: activityRun, providerVersion: '0.163.0', sourceKind: 'test_fixture', observedAtUtc, identityConfidence: 'provisional', verifiedIdentityKey: null, monitoring: 'stopped', accepted: true, coverage: 'unknown', scope: 'account_activity_with_unknown_application_scope', freshness: 'fresh', summary: { lifetimeTokens: '9007199254740993', peakDailyTokens: null, currentStreakDays: '0', longestStreakDays: null, longestRunningTurnSec: null }, dailyState: 'empty', dailyUsageBuckets: [], dailyTruncated: false, dailyChart: { maxTokens: null, bars: [] } };
}
function committed(index = 0): ProfileCollectionOutcome {
  return { profileId, approvalId, requestId: requests[index]!, providerVersion: '0.163.0', sourceKind: 'test_fixture', identityConfidence: 'provisional', verifiedIdentityKey: null, monitoring: 'stopped', accepted: true,
    quota: { bindingId, collectionRunId: nextQuotaRun, observedAtUtc }, activity: { bindingId, collectionRunId: nextActivityRun, observedAtUtc } };
}
async function open(page: Page, reconnect = false) {
  await page.addInitScript(({ profileId, approvalId, requests, reviewId, observedAtUtc, quota, activity, reconnect }) => {
    const fixture: Fixture = { calls: [], completions: {}, cancellations: {}, quota, activity, failLoads: false, approvalId, refreshSeconds: 0, monitoring: { state: 'stopped', readingRevision: 'cccccccc-cccc-4ccc-8ccc-cccccccccccc', profiles: [{ profileId, state: 'stopped', eligible: false, issue: 'sign_in_required', nextReadInSeconds: null, readingActive: false }] }, deferMonitoring: false, pendingMonitoring: [], failMonitoring: false };
    Object.assign(window, { collectionFixture: fixture }); let started = 0;
    const metadata = { path: '/synthetic/reviewed-codex', fileBytes: '100', sha256: 'a'.repeat(64), testedVersion: '0.163.0', versionChecked: false, sourceVerified: true, sourceKind: reconnect ? 'official_artifact' : 'test_fixture', sourceUrl: reconnect ? 'https://github.com/openai/codex/releases/tag/rust-v0.163.0' : null };
    window.__TAURI__ = { core: { invoke: async <T>(command: string, args: Record<string, unknown> = {}) => {
      fixture.calls.push({ command, args: { ...args } }); let result: unknown;
      switch (command) {
        case 'list_backups': result = { backups: [{ fileName: 'openusage-backup-synthetic0000000.oubackup', fileBytes: '100' }], truncated: false }; break;
        case 'inspect_backup': result = { fileName: 'openusage-backup-synthetic0000000.oubackup', archiveVersion: 1, manifestSchemaVersion: 4, entryCount: 1, totalBytes: '100', entries: [{ name: 'records.sqlite', bytes: '100' }], databaseValidation: 'valid', contentSummary: { activeProfiles: 1, archivedProfiles: '0', accounts: '0' }, restoreAvailable: false }; break;
        case 'prepare_restore_preview': result = { previewId: reviewId, fileName: 'openusage-backup-synthetic0000000.oubackup', current: { activeProfiles: 1, archivedProfiles: '0', accounts: '0' }, incoming: { activeProfiles: 1, archivedProfiles: '0', accounts: '0' }, checkpoint: { id: reviewId, fileBytes: '100' }, restoreAvailable: true }; break;
        case 'apply_restore':
          fixture.deferMonitoring = false; fixture.monitoring = { state: 'stopped', readingRevision: 'ffffffff-ffff-4fff-8fff-ffffffffffff', profiles: [{ profileId, state: 'stopped', eligible: false, issue: 'sign_in_required', nextReadInSeconds: null, readingActive: false }] };
          result = { applied: true, restored: { activeProfiles: 1, archivedProfiles: '0', accounts: '0' }, checkpoint: { id: reviewId, fileBytes: '100' }, monitoring: 'stopped', authenticationRequired: true }; break;
        case 'cancel_restore_preview': result = { cancelled: true }; break;
        case 'load_monitoring':
          if (fixture.failMonitoring) throw 'provider_unavailable';
          result = fixture.deferMonitoring ? await new Promise((resolve, reject) => fixture.pendingMonitoring.push({ resolve, reject })) : structuredClone(fixture.monitoring); break;
        case 'start_profile_monitoring':
          if (!fixture.monitoring.profiles[0]!.eligible) throw 'monitoring_not_eligible';
          fixture.monitoring.state = 'running'; Object.assign(fixture.monitoring.profiles[0]!, { state: 'running', nextReadInSeconds: 300 }); result = structuredClone(fixture.monitoring); break;
        case 'pause_profile_monitoring':
          Object.assign(fixture.monitoring.profiles[0]!, { state: 'paused', readingActive: false, nextReadInSeconds: null });
          fixture.monitoring.state = fixture.monitoring.profiles.some(value => value.state === 'running') ? 'running' : 'paused'; result = structuredClone(fixture.monitoring); break;
        case 'bootstrap': result = { storage: 'ready', theme: 'light', schemaVersion: 4, activeProfiles: 1, monitoring: 'stopped', provider: 'not_configured', keyProtection: 'passphrase', storageIssue: null }; break;
        case 'list_profiles': result = { activeProfiles: 1, profiles: [{ id: profileId, label: 'Synthetic collection profile', environment: 'native', archived: false, connectionState: fixture.connectionState ?? 'connected' }] }; break;
        case 'load_profile_refresh_status': result = { profileId, retryAfterSeconds: fixture.refreshSeconds }; break;
        case 'load_profile_quota': if (fixture.failLoads) throw 'provider_quota_load_failed'; result = structuredClone(fixture.quota); break;
        case 'load_profile_activity': if (fixture.failLoads) throw 'provider_activity_load_failed'; result = structuredClone(fixture.activity); break;
        case 'prepare_provider_connection': result = { ...metadata, approvalId: fixture.approvalId }; break;
        case 'inspect_provider_executable': result = { ...metadata, candidateId: reviewId }; break;
        case 'approve_provider_executable': result = { ...metadata, approvalId: fixture.approvalId }; break;
        case 'begin_profile_activity_read':
        case 'begin_profile_quota_read':
        case 'begin_profile_collection_read':
        case 'begin_profile_collection_reassociation': result = { profileId, approvalId: fixture.approvalId, requestId: requests[started++], sourceKind: metadata.sourceKind, monitoring: 'stopped', accepted: false }; break;
        case 'complete_profile_activity_read':
        case 'complete_profile_quota_read':
        case 'complete_profile_collection_read': result = await new Promise((resolve, reject) => { fixture.completions[String(args.requestId)] = { resolve, reject }; }); break;
        case 'cancel_profile_activity_read':
        case 'cancel_profile_quota_read':
        case 'cancel_profile_collection_read': result = await new Promise((resolve, reject) => { fixture.cancellations[String(args.requestId)] = { resolve, reject }; }); break;
        case 'review_profile_collection_association': result = { reviewId, profileId, approvalId: fixture.approvalId, observedAtUtc, previous: { email: 'previous@synthetic.invalid', plan: null }, current: { email: 'current@synthetic.invalid', plan: 'plus' }, homeChanged: false, identityConfidence: 'provisional' }; break;
        default: throw new Error(`Unexpected collection renderer command: ${command}`);
      }
      return result as T;
    } } };
  }, { profileId, approvalId, requests, reviewId, observedAtUtc, quota: { ...cachedQuota(), ...(reconnect ? { sourceKind: 'official_artifact' as const } : {}) }, activity: { ...cachedActivity(), ...(reconnect ? { sourceKind: 'official_artifact' as const } : {}) }, reconnect });
  await page.goto('/'); await navigate(page, 'accounts'); await openReadings(page, profileId); if (!reconnect) await approve(page); await navigate(page, 'accounts');
}
async function approve(page: Page) {
  await navigate(page, 'settings');
  const section = page.locator('.provider-panel');
  if (!(await section.locator('[data-provider-settings]').evaluate(node => node.closest('details')!.open))) await section.locator('[data-provider-settings]').click();
  if (!(await section.locator('[data-provider-manual]').evaluate(node => node.closest('details')!.open))) await section.locator('[data-provider-manual]').click();
  await page.getByLabel('Codex executable', { exact: true }).fill('/synthetic/reviewed-codex');
  await page.getByRole('button', { name: 'Inspect executable', exact: true }).click();
  await page.getByLabel('Allow Codex to sign in and read usage.').check();
  await page.getByRole('button', { name: 'Continue', exact: true }).click();
}
const status = (page: Page) => page.locator(`[data-collection-profile="${profileId}"]`);
const cancel = (page: Page) => page.getByRole('button', { name: 'Cancel refresh', exact: true });
async function begin(page: Page, index = 0) {
  await page.getByRole('button', { name: 'Refresh readings', exact: true }).click();
  await expect(cancel(page)).toBeFocused();
  await expect.poll(() => page.evaluate(id => Boolean((window as unknown as FixtureWindow).collectionFixture.completions[id]), requests[index]!)).toBe(true);
}
async function publish(page: Page, value = committed(), updateCache = true) {
  await page.evaluate(({ value, updateCache }) => {
    const f = (window as unknown as FixtureWindow).collectionFixture;
    if (updateCache) {
      if (value.quota) { f.quota = { ...f.quota, ...value.quota }; f.quota.limits[0]!.primary!.usedPercent = 50; f.quota.limits[0]!.primary!.derivedRemainingPercent = 50; }
      if (value.activity) f.activity = { ...f.activity, ...value.activity, summary: { ...f.activity.summary, lifetimeTokens: '9223372036854775807' } };
    }
    f.completions[value.requestId]!.resolve(value);
  }, { value, updateCache });
}
async function reject(page: Page, error: string, index = 0) {
  await page.evaluate(({ id, error }) => (window as unknown as FixtureWindow).collectionFixture.completions[id]!.reject(error), { id: requests[index]!, error });
}

test('combined primary action preserves pending work across Manage and navigation, then reloads both saved views', async ({ page }) => {
  await open(page);
  await expect(page.getByRole('button', { name: 'Read quotas', exact: true })).toBeHidden();
  await expect(page.getByRole('button', { name: 'Read activity', exact: true })).toBeHidden();
  await begin(page);
  await page.getByRole('button', { name: 'Manage profile', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Read quotas', exact: true })).toBeDisabled();
  await expect(page.getByRole('button', { name: 'Read activity', exact: true })).toBeDisabled();
  await page.getByRole('button', { name: 'Manage profile', exact: true }).click();
  await expect(cancel(page)).toBeVisible();
  await navigate(page, 'usage'); await publish(page);
  await expect(page.locator('[data-workspace-nav="usage"]')).toBeFocused();
  await navigate(page, 'accounts');
  await expect(status(page)).toContainText('Quota and activity readings saved.');
  await expect(page.locator('[data-quota-saved] [role=meter]')).toHaveAttribute('aria-valuenow', '50');
  await expect(page.locator('[data-activity-saved] [data-activity-metric="lifetimeTokens"]')).toHaveText('9223372036854775807');
});

test('successful cancellation fences a late completion from a newer refresh', async ({ page }) => {
  await open(page); await begin(page); await cancel(page).click();
  await expect.poll(() => page.evaluate(id => Boolean((window as unknown as FixtureWindow).collectionFixture.cancellations[id]), requests[0]!)).toBe(true);
  await page.evaluate(id => (window as unknown as FixtureWindow).collectionFixture.cancellations[id]!.resolve({ cancelled: true }), requests[0]!);
  await expect(status(page)).toContainText('Reading refresh stopped.'); await begin(page, 1);
  await publish(page, committed(), false);
  await expect(cancel(page)).toBeVisible(); await expect(status(page)).toHaveCount(0);
  await publish(page, committed(1)); await expect(status(page)).toContainText('Quota and activity readings saved.');
});

test('unknown completion fields retain both cached measurements and reload both freshness states', async ({ page }) => {
  await open(page); await begin(page);
  await page.evaluate(({ id, value }) => { const f = (window as unknown as FixtureWindow).collectionFixture; f.quota.freshness = 'stale'; f.activity.freshness = 'stale'; f.completions[id]!.resolve({ ...value, unknownField: true }); }, { id: requests[0]!, value: committed() });
  await expect(status(page)).toHaveAttribute('role', 'alert');
  await expect(page.locator('[data-quota-saved] [role=meter]')).toHaveAttribute('aria-valuenow', '75');
  await expect(page.locator('[data-activity-saved] [data-activity-metric="lifetimeTokens"]')).toHaveText('9007199254740993');
  await expect(page.locator('[data-quota-freshness]')).toHaveAttribute('data-quota-freshness', 'stale');
  await expect(page.locator('[data-activity-freshness]')).toHaveAttribute('data-activity-freshness', 'stale');
});

for (const missing of ['quota', 'activity', 'both'] as const) test(`unsupported ${missing} retains previous values and explains capability absence`, async ({ page }) => {
  await open(page); await begin(page);
  const value = committed(); if (missing !== 'activity') value.quota = null; if (missing !== 'quota') value.activity = null; value.accepted = missing !== 'both';
  await publish(page, value);
  await expect(status(page)).toContainText('does not support');
  if (missing !== 'activity') await expect(page.locator('[data-quota-saved] [role=meter]')).toHaveAttribute('aria-valuenow', '75');
  if (missing !== 'quota') await expect(page.locator('[data-activity-saved] [data-activity-metric="lifetimeTokens"]')).toHaveText('9007199254740993');
  if (missing === 'both') await expect(status(page)).not.toContainText('saved.');
});

test('a signed-out provider response guides sign-in and retains both saved readings', async ({ page }) => {
  await open(page); await begin(page); await reject(page, 'provider_authentication_required');
  await expect(status(page)).toHaveAttribute('role', 'alert');
  await expect(status(page)).toContainText('Sign in again before reading or starting monitoring.');
  await expect(page.locator('[data-quota-saved] [role=meter]')).toHaveAttribute('aria-valuenow', '75');
  await expect(page.locator('[data-activity-saved] [data-activity-metric="lifetimeTokens"]')).toHaveText('9007199254740993');
});

test('committed cache mismatches keep previous values with freshness unknown and explicit reload errors', async ({ page }) => {
  await open(page); await begin(page); await publish(page, committed(), false);
  await expect(status(page)).toContainText('Quota and activity readings saved.');
  await expect(page.locator('[data-quota-cache-error]')).toBeVisible(); await expect(page.locator('[data-activity-cache-error]')).toBeVisible();
  await expect(page.locator('[data-quota-freshness]')).toHaveAttribute('data-quota-freshness', 'unknown');
  await expect(page.locator('[data-activity-freshness]')).toHaveAttribute('data-activity-freshness', 'unknown');
  await expect(page.locator('[data-quota-saved] [role=meter]')).toHaveAttribute('aria-valuenow', '75');
  await expect(page.locator('[data-activity-saved] [data-activity-metric="lifetimeTokens"]')).toHaveText('9007199254740993');
});

test('changed association requires visible review and explicit consent before another combined start', async ({ page }) => {
  await open(page); await begin(page); await reject(page, 'provider_collection_association_changed');
  await status(page).getByRole('button', { name: 'Use current sign-in', exact: true }).click();
  await expect(status(page).locator('[data-collection-association="previous"]')).toContainText('previous@synthetic.invalid');
  await expect(status(page).locator('[data-collection-association="current"]')).toContainText('current@synthetic.invalid');
  await expect(status(page).getByRole('button', { name: 'Confirm and read', exact: true })).toBeDisabled();
  await page.getByLabel('I approve using the current sign-in for this profile.').check();
  await status(page).getByRole('button', { name: 'Confirm and read', exact: true }).click();
  await expect.poll(() => page.evaluate(id => Boolean((window as unknown as FixtureWindow).collectionFixture.completions[id]), requests[1]!)).toBe(true);
  const calls = await page.evaluate(() => (window as unknown as FixtureWindow).collectionFixture.calls);
  expect(calls.filter(call => call.command === 'begin_profile_collection_reassociation')).toEqual([{ command: 'begin_profile_collection_reassociation', args: { profileId, approvalId, reviewId, confirmed: true } }]);
  await publish(page, committed(1)); await expect(status(page)).toContainText('Quota and activity readings saved.');
});

test('approval replacement cancels the unmounted request and ignores its late outcome', async ({ page }) => {
  await open(page); await begin(page);
  await page.evaluate(() => { (window as unknown as FixtureWindow).collectionFixture.approvalId = 'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb'; });
  await approve(page);
  await expect.poll(() => page.evaluate(id => Boolean((window as unknown as FixtureWindow).collectionFixture.cancellations[id]), requests[0]!)).toBe(true);
  await publish(page, committed(), false); await navigate(page, 'accounts');
  await expect(status(page)).toHaveCount(0); await expect(page.getByRole('button', { name: 'Refresh readings', exact: true })).toBeEnabled();
  await expect(page.locator('[data-quota-saved] [role=meter]')).toHaveAttribute('aria-valuenow', '75');
});


test('a delayed cancel failure cannot replace a completed committed outcome', async ({ page }) => {
  await open(page); await begin(page); await cancel(page).click();
  await expect.poll(() => page.evaluate(id => Boolean((window as unknown as FixtureWindow).collectionFixture.cancellations[id]), requests[0]!)).toBe(true);
  await publish(page); await expect(status(page)).toContainText('Quota and activity readings saved.');
  await page.evaluate(id => (window as unknown as FixtureWindow).collectionFixture.cancellations[id]!.reject('provider_cleanup_failed'), requests[0]!);
  await expect(status(page)).toContainText('Quota and activity readings saved.');
  await expect(status(page)).toHaveAttribute('role', 'status');
});


for (const kind of ['quota', 'collection'] as const) test(`closed Manage reverse Tab reaches active ${kind} Cancel through native DOM order`, async ({ page }) => {
  await open(page);
  const manage = page.getByRole('button', { name: 'Manage profile', exact: true });
  if (kind === 'quota') {
    await manage.click();
    await page.getByRole('button', { name: 'Read quotas', exact: true }).click();
    await expect(page.getByRole('button', { name: 'Cancel read', exact: true })).toBeFocused();
    await manage.click();
  } else {
    await begin(page);
    await manage.click();
    await manage.click();
  }
  await expect(manage).toHaveAttribute('aria-expanded', 'false');
  await expect(manage).toBeFocused();
  await page.keyboard.press('Shift+Tab');
  await expect(page.getByRole('button', { name: kind === 'quota' ? 'Cancel read' : 'Cancel refresh', exact: true })).toBeFocused();
});

test('combined completion keeps focus on its result after both real saved-cache wrappers reload', async ({ page }) => {
  await open(page); await begin(page); await publish(page);
  await expect(status(page)).toContainText('Quota and activity readings saved.');
  await expect(page.locator('[data-quota-saved] [role=meter]')).toHaveAttribute('aria-valuenow', '50');
  await expect(page.locator('[data-activity-saved] [data-activity-metric="lifetimeTokens"]')).toHaveText('9223372036854775807');
  await expect(status(page)).toBeFocused();
});


for (const kind of ['quota', 'activity'] as const) test(`individual ${kind} refresh still focuses its validated saved cache`, async ({ page }) => {
  await open(page); await page.getByRole('button', { name: 'Manage profile', exact: true }).click();
  await page.getByRole('button', { name: kind === 'quota' ? 'Read quotas' : 'Read activity', exact: true }).click();
  await expect.poll(() => page.evaluate(id => Boolean((window as unknown as FixtureWindow).collectionFixture.completions[id]), requests[0]!)).toBe(true);
  if (kind === 'quota') {
    const { freshness: _freshness, coverage: _coverage, scope: _scope, ...value } = cachedQuota();
    const receipt = { ...value, requestId: requests[0]!, approvalId, collectionRunId: nextQuotaRun, limits: value.limits.map(limit => ({ ...limit, primary: limit.primary && { usedPercent: 50, derivedRemainingPercent: 50, windowDurationMins: 300, resetsAtUnix: null, label: 'Five-hour' } })) };
    await page.evaluate(({ receipt, run }) => { const f = (window as unknown as FixtureWindow).collectionFixture; f.quota.collectionRunId = run; f.quota.limits[0]!.primary!.usedPercent = 50; f.quota.limits[0]!.primary!.derivedRemainingPercent = 50; f.completions[receipt.requestId]!.resolve(receipt); }, { receipt, run: nextQuotaRun });
  } else {
    const { freshness: _freshness, dailyState: _dailyState, dailyTruncated: _dailyTruncated, dailyChart: _dailyChart, ...value } = cachedActivity();
    const receipt = { ...value, requestId: requests[0]!, approvalId, collectionRunId: nextActivityRun, dailyUsageBuckets: [] };
    await page.evaluate(({ receipt, run }) => { const f = (window as unknown as FixtureWindow).collectionFixture; f.activity.collectionRunId = run; f.completions[receipt.requestId]!.resolve(receipt); }, { receipt, run: nextActivityRun });
  }
  await expect(page.locator(`[data-${kind}-saved="${profileId}"]`)).toBeFocused();
});

const monitoringPanel = (page: Page) => page.locator(`[data-monitoring-profile="${profileId}"]`);
async function eligibleMonitoring(page: Page) {
  await page.evaluate(() => { const f = (window as unknown as FixtureWindow).collectionFixture; Object.assign(f.monitoring.profiles[0]!, { eligible: true, issue: null }); });
  await expect(page.locator(`[data-monitoring-start="${profileId}"]`)).toBeEnabled();
}

test('persisted Connected does not grant monitoring eligibility or cause automatic provider work', async ({ page }) => {
  await open(page); await page.getByRole('button', { name: 'Manage profile', exact: true }).click();
  await expect(page.locator('.profile-connection-state')).toHaveText('Connected');
  await expect(page.locator(`[data-monitoring-start="${profileId}"]`)).toBeDisabled();
  await expect(monitoringPanel(page)).toContainText('Sign in again');
  await expect(page.locator('[data-global-monitoring]')).toHaveText('Monitoring stopped');
  await eligibleMonitoring(page);
  const before = await page.evaluate(() => (window as unknown as FixtureWindow).collectionFixture.calls.length);
  await page.locator(`[data-monitoring-start="${profileId}"]`).click();
  await expect(page.locator(`[data-monitoring-pause="${profileId}"]`)).toBeFocused();
  await expect(page.locator('[data-global-monitoring]')).toHaveText('Monitoring running');
  const calls = await page.evaluate(n => (window as unknown as FixtureWindow).collectionFixture.calls.slice(n), before);
  expect(calls.filter(call => call.command === 'start_profile_monitoring')).toEqual([{ command: 'start_profile_monitoring', args: { profileId, approvalId } }]);
  expect(calls.some(call => /^(begin_|complete_|check_profile|discover_|inspect_|approve_)/.test(call.command))).toBe(false);
});

test('one global status poll reloads caches only on changed reading revision and preserves Pause focus', async ({ page }) => {
  await open(page); await page.getByRole('button', { name: 'Manage profile', exact: true }).click(); await eligibleMonitoring(page);
  await page.locator(`[data-monitoring-start="${profileId}"]`).click();
  const pause = page.locator(`[data-monitoring-pause="${profileId}"]`); await expect(pause).toBeFocused();
  const baseline = await page.evaluate(() => {
    const f = (window as unknown as FixtureWindow).collectionFixture;
    return { polls: f.calls.filter(call => call.command === 'load_monitoring').length, loads: f.calls.filter(call => call.command === 'load_profile_quota' || call.command === 'load_profile_activity').length };
  });
  await expect.poll(() => page.evaluate(() => (window as unknown as FixtureWindow).collectionFixture.calls.filter(call => call.command === 'load_monitoring').length)).toBeGreaterThan(baseline.polls);
  expect(await page.evaluate(() => (window as unknown as FixtureWindow).collectionFixture.calls.filter(call => call.command === 'load_profile_quota' || call.command === 'load_profile_activity').length)).toBe(baseline.loads);
  await page.evaluate(({ quotaRun, activityRun }) => {
    const f = (window as unknown as FixtureWindow).collectionFixture;
    f.monitoring.readingRevision = 'dddddddd-dddd-4ddd-8ddd-dddddddddddd'; f.monitoring.profiles[0]!.readingActive = true;
    f.quota.collectionRunId = quotaRun; f.quota.limits[0]!.primary!.derivedRemainingPercent = 50; f.quota.limits[0]!.primary!.usedPercent = 50;
    f.activity.collectionRunId = activityRun; f.activity.summary.lifetimeTokens = '9223372036854775807';
  }, { quotaRun: nextQuotaRun, activityRun: nextActivityRun });
  await expect(page.locator('[data-quota-saved] [role=meter]')).toHaveAttribute('aria-valuenow', '50');
  await expect(page.locator('[data-activity-saved] [data-activity-metric="lifetimeTokens"]')).toHaveText('9223372036854775807');
  await expect(pause).toBeFocused(); await expect(pause).toBeEnabled();
  await expect(page.getByRole('button', { name: 'Refresh readings', exact: true })).toBeDisabled();
  await navigate(page, 'overview'); await expect(page.locator('[data-overview-lifetime] bdi')).toHaveAttribute('title', '9223372036854775807');
  const calls = await page.evaluate(() => (window as unknown as FixtureWindow).collectionFixture.calls.filter(call => call.command === 'load_monitoring'));
  expect(calls.every(call => Object.keys(call.args).length === 0)).toBe(true);
});

test('a delayed running poll cannot overwrite a completed Pause or steal its focus', async ({ page }) => {
  await open(page); await page.getByRole('button', { name: 'Manage profile', exact: true }).click(); await eligibleMonitoring(page);
  await page.locator(`[data-monitoring-start="${profileId}"]`).click();
  await page.evaluate(() => { (window as unknown as FixtureWindow).collectionFixture.deferMonitoring = true; });
  await expect.poll(() => page.evaluate(() => (window as unknown as FixtureWindow).collectionFixture.pendingMonitoring.length)).toBe(1);
  await page.locator(`[data-monitoring-pause="${profileId}"]`).click();
  await expect(page.locator('[data-global-monitoring]')).toHaveText('Monitoring paused');
  await page.evaluate(() => { const f = (window as unknown as FixtureWindow).collectionFixture; f.pendingMonitoring[0]!.resolve({ ...f.monitoring, state: 'running', profiles: f.monitoring.profiles.map(p => ({ ...p, state: 'running' })) }); });
  await expect(page.locator('[data-global-monitoring]')).toHaveText('Monitoring paused');
  await expect(page.locator(`[data-monitoring-start="${profileId}"]`)).toBeFocused();
});

test('one paused profile does not make the global status stopped while another profile runs', async ({ page }) => {
  await open(page); await page.getByRole('button', { name: 'Manage profile', exact: true }).click(); await eligibleMonitoring(page);
  await page.locator(`[data-monitoring-start="${profileId}"]`).click();
  await page.evaluate(() => { const f = (window as unknown as FixtureWindow).collectionFixture; f.monitoring.profiles.push({ ...f.monitoring.profiles[0]!, profileId: 'eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee' }); });
  await page.locator(`[data-monitoring-pause="${profileId}"]`).click();
  await expect(monitoringPanel(page)).toHaveAttribute('data-monitoring-state', 'paused');
  await expect(page.locator('[data-global-monitoring]')).toHaveText('Monitoring running');
});

test('status failure retains saved readings without claiming monitoring stopped', async ({ page }) => {
  await open(page); await page.getByRole('button', { name: 'Manage profile', exact: true }).click(); await eligibleMonitoring(page);
  await page.locator(`[data-monitoring-start="${profileId}"]`).click();
  await page.evaluate(() => { (window as unknown as FixtureWindow).collectionFixture.failMonitoring = true; });
  await expect(page.locator('.status-pill')).toHaveText('Monitoring status unavailable');
  await expect(monitoringPanel(page)).toContainText('Monitoring status unavailable');
  const pause = page.locator(`[data-monitoring-pause="${profileId}"]`);
  await expect(pause).toBeEnabled(); await pause.click();
  await expect(page.locator('[data-global-monitoring]')).toHaveText('Monitoring paused');
  await expect(page.locator(`[data-monitoring-start="${profileId}"]`)).toBeFocused();
  await expect(page.locator('[data-quota-saved] [role=meter]')).toHaveAttribute('aria-valuenow', '75');
  await expect(page.locator('[data-activity-saved] [data-activity-metric="lifetimeTokens"]')).toHaveText('9007199254740993');
});


test('a pre-restore status reply cannot restart or repaint the new workspace monitoring epoch', async ({ page }) => {
  await open(page);
  await navigate(page, 'overview');
  const account = page.locator(`[data-overview-account="${profileId}"]`);
  await expect(account.locator('[data-overview-account-quota]')).toContainText('75% left');
  await expect(account.locator('[data-overview-account-observed] time')).toHaveAttribute('datetime', observedAtUtc);
  await expect(page.locator('[data-overview-lifetime] bdi')).toHaveAttribute('title', '9007199254740993');
  await navigate(page, 'accounts');
  await page.getByRole('button', { name: 'Manage profile', exact: true }).click(); await eligibleMonitoring(page);
  await page.locator(`[data-monitoring-start="${profileId}"]`).click();
  await page.evaluate(() => { (window as unknown as FixtureWindow).collectionFixture.deferMonitoring = true; });
  await expect.poll(() => page.evaluate(() => (window as unknown as FixtureWindow).collectionFixture.pendingMonitoring.length)).toBe(1);
  await navigate(page, 'settings'); await page.getByRole('button', { name: 'Inspect a backup', exact: true }).click();
  await page.getByLabel('Backup password', { exact: true }).fill('Synthetic backup password');
  await page.getByRole('button', { name: 'Inspect backup', exact: true }).click();
  await page.getByRole('button', { name: 'Preview restore', exact: true }).click();
  await page.locator('#restore-preview-password').fill('Synthetic backup password');
  await page.locator('.restore-preview-form button[type=submit]').click();
  await page.getByLabel('I approve replacing my current application records.').check();
  await page.evaluate(() => { (window as unknown as FixtureWindow).collectionFixture.failLoads = true; });
  await page.getByRole('button', { name: 'Restore records', exact: true }).click();
  await expect(page.locator('.restore-result')).toBeVisible();
  await expect(page.locator('[data-global-monitoring]')).toHaveText('Monitoring stopped');
  await page.evaluate(() => { const f = (window as unknown as FixtureWindow).collectionFixture; f.pendingMonitoring[0]!.resolve({ state: 'running', readingRevision: 'eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee', profiles: [{ ...f.monitoring.profiles[0]!, state: 'running', eligible: true, issue: null }] }); });
  await expect(page.locator('[data-global-monitoring]')).toHaveText('Monitoring stopped');
  await navigate(page, 'accounts'); await openReadings(page, profileId); await page.getByRole('button', { name: 'Manage profile', exact: true }).click();
  await expect(page.locator(`[data-monitoring-start="${profileId}"]`)).toBeDisabled();
  await navigate(page, 'overview');
  await expect(page.locator('[data-overview-quota] .overview-error')).toContainText('Saved quotas could not be loaded.');
  await expect(account.locator('[data-overview-account-quota]')).toHaveText('Not available');
  await expect(account.locator('[data-overview-account-observed]')).toContainText('—');
  await expect(account.locator('[data-overview-account-observed] time')).toHaveCount(0);
  await expect(page.locator('[data-overview-lifetime]')).toHaveText('Not available');
  await expect(page.locator('[data-global-monitoring]')).toHaveText('Monitoring stopped');
});


// Renderer-only restart state: no runtime provider approval or monitoring grant survives.
test('existing account Connect prepares its official source and reads the same profile once without login', async ({ page }) => {
  await open(page, true);
  await page.locator(`[data-profile-connect="${profileId}"]`).click();
  await expect.poll(() => page.evaluate(id => Boolean((window as unknown as FixtureWindow).collectionFixture.completions[id]), requests[0]!)).toBe(true);
  await expect(cancel(page)).toBeVisible();
  await publish(page, { ...committed(), sourceKind: 'official_artifact' });
  await expect(status(page)).toContainText('Quota and activity readings saved.');
  await expect(page.locator(`[data-activity-saved="${profileId}"] [data-activity-metric=lifetimeTokens]`)).toHaveText('9223372036854775807');
  await expect(page.locator(`[data-quota-saved="${profileId}"] [role=meter]`)).toHaveAttribute('aria-valuenow', '50');
  const calls = await page.evaluate(() => (window as unknown as FixtureWindow).collectionFixture.calls);
  expect(calls.filter(call => call.command === 'prepare_provider_connection')).toHaveLength(1);
  expect(calls.filter(call => call.command === 'begin_profile_collection_read').map(call => call.args)).toEqual([{ profileId, approvalId }]);
  expect(calls.some(call => call.command === 'create_profile' || call.command === 'begin_profile_login' || call.command === 'start_profile_monitoring')).toBe(false);
  await page.getByRole('button', { name: 'Manage profile', exact: true }).click();
  await expect(page.locator(`[data-monitoring-start="${profileId}"]`)).toBeDisabled();
});

test('reconnect authentication failure preserves saved readings and exposes sign-in without granting monitoring', async ({ page }) => {
  await open(page, true);
  await page.locator(`[data-profile-connect="${profileId}"]`).click();
  await expect.poll(() => page.evaluate(id => Boolean((window as unknown as FixtureWindow).collectionFixture.completions[id]), requests[0]!)).toBe(true);
  await page.evaluate(() => { (window as unknown as FixtureWindow).collectionFixture.connectionState = 'authentication_required'; });
  await reject(page, 'provider_authentication_required');
  await expect(status(page)).toContainText('Sign in again before reading or starting monitoring.');
  await expect(page.locator(`[data-login-start="${profileId}"]`)).toBeVisible();
  await expect(page.locator(`[data-profile-id="${profileId}"] .profile-connection-state`)).toHaveText('Not connected');
  await expect(page.locator(`[data-activity-saved="${profileId}"] [data-activity-metric=lifetimeTokens]`)).toHaveText('9007199254740993');
  await expect(page.locator(`[data-quota-saved="${profileId}"] [role=meter]`)).toHaveAttribute('aria-valuenow', '75');
  await page.getByRole('button', { name: 'Manage profile', exact: true }).click();
  await expect(page.locator(`[data-monitoring-start="${profileId}"]`)).toBeDisabled();
  const calls = await page.evaluate(() => (window as unknown as FixtureWindow).collectionFixture.calls);
  expect(calls.filter(call => call.command === 'begin_profile_collection_read')).toHaveLength(1);
  expect(calls.some(call => call.command === 'begin_profile_login' || call.command === 'start_profile_monitoring')).toBe(false);
});


test('row Refresh during cooldown cannot queue a provider read after the wait expires', async ({ page }) => {
  await page.clock.install();
  await open(page, true);
  await page.locator(`[data-profile-connect="${profileId}"]`).click();
  await expect.poll(() => page.evaluate(id => Boolean((window as unknown as FixtureWindow).collectionFixture.completions[id]), requests[0]!)).toBe(true);
  await page.evaluate(() => { (window as unknown as FixtureWindow).collectionFixture.refreshSeconds = 30; });
  await publish(page, { ...committed(), sourceKind: 'official_artifact' });
  await expect(status(page)).toContainText('Quota and activity readings saved.');
  await expect(page.locator(`[data-collection-read="${profileId}"]`)).toBeDisabled();
  await expect(page.locator(`[data-profile-refresh-status="${profileId}"]`)).toContainText('Next reading in');
  await expect(page.locator(`[data-profile-connect="${profileId}"]`)).toBeDisabled();
  await page.locator(`[data-profile-connect="${profileId}"]`).evaluate(node => (node as HTMLButtonElement).click());
  await page.evaluate(() => { (window as unknown as FixtureWindow).collectionFixture.refreshSeconds = 0; });
  await page.clock.runFor(31_000);
  await expect(page.locator(`[data-collection-read="${profileId}"]`)).toBeEnabled();
  const calls = await page.evaluate(() => (window as unknown as FixtureWindow).collectionFixture.calls);
  expect(calls.filter(call => call.command === 'begin_profile_collection_read')).toHaveLength(1);
  expect(calls.some(call => call.command === 'begin_profile_login' || call.command === 'start_profile_monitoring')).toBe(false);
  await expect(page.locator(`[data-activity-saved="${profileId}"] [data-activity-metric=lifetimeTokens]`)).toHaveText('9223372036854775807');
});
