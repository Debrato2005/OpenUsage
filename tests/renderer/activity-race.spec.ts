import { expect, test, type Page } from '@playwright/test';

// Renderer-only synthetic IPC; native ownership, storage and provider reads are tested separately.
type Deferred = { resolve: (value: unknown) => void; reject: (error: string) => void };
type Fixture = { completions: Record<string, Deferred>; cancellations: Record<string, Deferred>; requests: string[] };
type FixtureWindow = Window & { activityFixture: Fixture };
const profileId = '11111111-1111-4111-8111-111111111111';
const approvalId = '22222222-2222-4222-8222-222222222222';
const requestIds = ['33333333-3333-4333-8333-333333333333', '44444444-4444-4444-8444-444444444444'];
const receipt = (requestId: string) => ({ profileId, approvalId, requestId, sourceKind: 'test_fixture', monitoring: 'stopped', accepted: false });
function preview(requestId: string, dailyUsageBuckets: { startDate: string; tokens: string }[] | null) {
  return { ...receipt(requestId), providerVersion: '0.163.0', observedAtUtc: '2026-10-18T00:00:00.000Z', identityConfidence: 'provisional', verifiedIdentityKey: null, coverage: 'unknown', scope: 'account_activity_with_unknown_application_scope', summary: { lifetimeTokens: '9223372036854775807', peakDailyTokens: '9007199254740993', currentStreakDays: null, longestStreakDays: '0', longestRunningTurnSec: '7' }, dailyUsageBuckets };
}
async function openProfile(page: Page) {
  await page.addInitScript(({ profileId, approvalId, requestIds }) => {
    const fixture: Fixture = { completions: {}, cancellations: {}, requests: [] };
    Object.assign(window, { activityFixture: fixture }); let started = 0;
    const metadata = { path: '/synthetic/reviewed-codex', fileBytes: '100', sha256: 'a'.repeat(64), testedVersion: '0.163.0', versionChecked: false, sourceVerified: true, sourceKind: 'test_fixture', sourceUrl: null };
    window.__TAURI__ = { core: { invoke: async <T>(command: string, args: Record<string, unknown> = {}) => {
      fixture.requests.push(command); let value: unknown;
      switch (command) {
        case 'bootstrap': value = { storage: 'ready', theme: 'light', schemaVersion: 1, activeProfiles: 1, monitoring: 'stopped', provider: 'not_configured', keyProtection: 'passphrase', storageIssue: null }; break;
        case 'list_profiles': value = { activeProfiles: 1, profiles: [{ id: profileId, label: 'Synthetic activity profile', environment: 'native', archived: false, connectionState: 'authentication_required' }] }; break;
        case 'inspect_provider_executable': value = { ...metadata, candidateId: '55555555-5555-4555-8555-555555555555' }; break;
        case 'approve_provider_executable': value = { ...metadata, approvalId }; break;
        case 'begin_profile_activity_read': value = { profileId, approvalId, requestId: requestIds[started++], sourceKind: 'test_fixture', monitoring: 'stopped', accepted: false }; break;
        case 'complete_profile_activity_read': value = await new Promise((resolve, reject) => { fixture.completions[String(args.requestId)] = { resolve, reject }; }); break;
        case 'cancel_profile_activity_read': value = await new Promise((resolve, reject) => { fixture.cancellations[String(args.requestId)] = { resolve, reject }; }); break;
        default: throw new Error(`Unexpected renderer fixture command: ${command}`);
      }
      return value as T;
    } } };
  }, { profileId, approvalId, requestIds });
  await page.goto('/');
  await page.getByRole('heading', { name: 'Codex connection', exact: true }).click();
  await page.locator('[data-provider-manual]').click();
  await page.getByLabel('Codex executable', { exact: true }).fill('/synthetic/reviewed-codex');
  await page.getByRole('button', { name: 'Inspect executable', exact: true }).click();
  await page.getByLabel('I trust this executable for account checks and previews.').check();
  await page.getByRole('button', { name: 'Approve executable', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Read activity', exact: true })).toBeHidden();
  await page.getByRole('button', { name: 'Manage profile', exact: true }).click();
}
async function begin(page: Page, index: number) {
  await page.getByRole('button', { name: 'Read activity', exact: true }).click();
  await expect(page.locator(`[data-activity-cancel="${profileId}"]`)).toBeFocused();
  await expect.poll(() => page.evaluate(id => Boolean((window as unknown as FixtureWindow).activityFixture.completions[id]), requestIds[index]!)).toBe(true);
}
async function resolvePreview(page: Page, index: number, rows: { startDate: string; tokens: string }[] | null) {
  await page.evaluate(({ id, value }) => { (window as unknown as FixtureWindow).activityFixture.completions[id]!.resolve(value); }, { id: requestIds[index]!, value: preview(requestIds[index]!, rows) });
  await expect(page.getByRole('region', { name: 'Activity preview for Synthetic activity profile' })).toBeFocused();
}

test('renderer fixture keeps activity running outside Manage and pages exact returned rows', async ({ page }) => {
  await openProfile(page); await begin(page, 0);
  await page.getByRole('button', { name: 'Manage profile', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Read activity', exact: true })).toBeHidden();
  await expect(page.getByRole('button', { name: 'Cancel activity read', exact: true })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Sign in', exact: true })).toBeDisabled();
  await expect(page.getByRole('button', { name: 'Read quotas', exact: true })).toBeDisabled();
  const rows = Array.from({ length: 51 }, (_, index) => ({ startDate: index === 0 ? '2024-02-29' : '2023-12-31', tokens: index === 0 ? '9223372036854775807' : '0' }));
  await resolvePreview(page, 0, rows);
  await expect(page.locator('[data-activity-metric=lifetimeTokens]')).toHaveText('9223372036854775807');
  await expect(page.locator('[data-activity-metric=currentStreakDays]')).toHaveText('Not supplied');
  await expect(page.getByText('Account activity; application scope and coverage are unknown. Monitoring is stopped.')).toBeVisible();
  await expect(page.locator('[data-activity-day]')).toHaveCount(0);
  await page.getByRole('button', { name: 'Activity details', exact: true }).click();
  await expect(page.locator('[data-activity-day]')).toHaveCount(50);
  await expect(page.locator('[data-activity-day]').first()).toHaveText('2024-02-299223372036854775807');
  await expect(page.getByRole('button', { name: 'Previous', exact: true })).toBeDisabled();
  await page.getByRole('button', { name: 'Next', exact: true }).click();
  await expect(page.locator('[data-activity-day]')).toHaveCount(1);
  await expect(page.getByText('Rows 51–51 of 51 returned')).toBeVisible();
  await expect(page.getByRole('button', { name: 'Next', exact: true })).toBeDisabled();
  await page.getByRole('button', { name: 'Previous', exact: true }).click();
  await expect(page.locator('[data-activity-day]')).toHaveCount(50);
});

test('renderer fixture isolates an old cancel failure from the successor activity preview', async ({ page }) => {
  await openProfile(page); await begin(page, 0);
  await page.getByRole('button', { name: 'Cancel activity read', exact: true }).click();
  await page.evaluate(id => { (window as unknown as FixtureWindow).activityFixture.completions[id]!.reject('provider_activity_cancelled'); }, requestIds[0]!);
  await expect(page.getByText('Activity read stopped.', { exact: true })).toBeVisible();
  await begin(page, 1);
  await page.evaluate(id => { (window as unknown as FixtureWindow).activityFixture.cancellations[id]!.reject('provider_cleanup_failed'); }, requestIds[0]!);
  await expect(page.getByRole('button', { name: 'Cancel activity read', exact: true })).toBeEnabled();
  await resolvePreview(page, 1, []);
  await page.getByRole('button', { name: 'Activity details', exact: true }).click();
  await expect(page.locator('[data-activity-daily-state=empty]')).toHaveText('No daily rows were returned.');
  await expect(page.getByText('OpenUsage could not confirm that the provider process stopped.', { exact: false })).toHaveCount(0);
});

test('renderer fixture distinguishes missing daily activity and refuses incompatible completion data', async ({ page }) => {
  await openProfile(page); await begin(page, 0); await resolvePreview(page, 0, null);
  await page.getByRole('button', { name: 'Activity details', exact: true }).click();
  await expect(page.locator('[data-activity-daily-state=unavailable]')).toHaveText('Daily usage was not supplied.');
  await expect(page.getByRole('navigation', { name: 'Daily usage pages' })).toHaveCount(0);
  await begin(page, 1);
  await page.evaluate(({ id, value }) => { (window as unknown as FixtureWindow).activityFixture.completions[id]!.resolve({ ...value, coverage: 'complete', accountId: 'private' }); }, { id: requestIds[1]!, value: preview(requestIds[1]!, []) });
  await expect(page.locator(`.activity-result[data-activity-profile="${profileId}"]`)).toHaveAttribute('role', 'alert');
  await expect(page.getByRole('heading', { name: 'Activity preview', exact: true })).toHaveCount(0);
  await expect(page.locator('[data-activity-metric]')).toHaveCount(0);
});
