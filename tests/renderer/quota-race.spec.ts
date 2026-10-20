import { navigate, openReadings } from './navigation';
import { expect, test, type Page } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';
import type { ProfileQuotaSavedRead, SavedProfileQuota } from '../../apps/desktop/src/contracts';

// Renderer-only synthetic IPC; SQLCipher, provider ownership and COMMIT are native acceptance scope.
type Deferred = { resolve: (value: unknown) => void; reject: (error: string) => void };
type Fixture = { completions: Record<string, Deferred>; cancellations: Record<string, Deferred>; requests: string[]; requestCalls: { command: string; args: Record<string, unknown> }[]; saved: SavedProfileQuota | null; failLoads: boolean; deferLoads: boolean; pendingLoads: Deferred[]; review: unknown; failReviews: boolean | string; deferReviews: boolean; pendingReviews: Deferred[]; refreshStatus: unknown; deferRefresh: boolean; pendingRefresh: Deferred[]; startError?: string };
type FixtureWindow = Window & { quotaFixture: Fixture };
const profileId = '11111111-1111-4111-8111-111111111111';
const approvalId = '22222222-2222-4222-8222-222222222222';
const requestIds = ['33333333-3333-4333-8333-333333333333', '44444444-4444-4444-8444-444444444444'];
const bindingId = '66666666-6666-4666-8666-666666666666';
const runIds = ['77777777-7777-4777-8777-777777777777', '88888888-8888-4888-8888-888888888888'];
const reviewId = 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa';
const observedAtUtc = '2026-10-18T00:00:00.000Z';
function committed(index = 0): ProfileQuotaSavedRead {
  return { profileId, approvalId, requestId: requestIds[index]!, collectionRunId: runIds[index]!, bindingId, sourceKind: 'test_fixture', monitoring: 'stopped', accepted: true,
    providerVersion: '0.163.0', observedAtUtc, identityConfidence: 'provisional', verifiedIdentityKey: null,
    limits: [{ limitId: 'codex', limitName: 'Synthetic limit', primary: { usedPercent: 25, derivedRemainingPercent: 75, windowDurationMins: 300, resetsAtUnix: null, label: 'Five-hour' }, secondary: null, reachedType: null, monthlyCreditLimit: '9007199254740993.012345678900', spendControlReached: false }], availableResetCredits: '9223372036854775807', resetCreditDetails: null };
}
function saved(index = 0): SavedProfileQuota {
  const { requestId: _requestId, approvalId: _approvalId, ...value } = committed(index);
  return { ...value, freshness: 'unknown', coverage: 'unknown', scope: 'account_quota_with_unknown_application_scope', limits: value.limits.map(limit => ({ ...limit, primary: limit.primary && { ...limit.primary, resetState: 'expected' }, secondary: limit.secondary && { ...limit.secondary, resetState: 'expected' } })) };
}
async function openProfile(page: Page, options: { approve?: boolean; cached?: SavedProfileQuota | null; archived?: boolean; refreshSeconds?: number; deferRefresh?: boolean } = {}) {
  await page.addInitScript(({ profileId, approvalId, requestIds, reviewId, observedAtUtc, options }) => {
    const fixture: Fixture = { completions: {}, cancellations: {}, requests: [], requestCalls: [], saved: options.cached ?? null, failLoads: false, deferLoads: false, pendingLoads: [], review: { reviewId, profileId, approvalId, observedAtUtc, previous: { email: 'previous@synthetic.invalid', plan: null }, current: { email: 'current@synthetic.invalid', plan: 'Synthetic plan' }, homeChanged: true, identityConfidence: 'provisional' }, failReviews: false, deferReviews: false, pendingReviews: [], refreshStatus: { profileId, retryAfterSeconds: options.refreshSeconds ?? 0 }, deferRefresh: options.deferRefresh ?? false, pendingRefresh: [] };
    Object.assign(window, { quotaFixture: fixture }); let started = 0;
    const metadata = { path: '/synthetic/reviewed-codex', fileBytes: '100', sha256: 'a'.repeat(64), testedVersion: '0.163.0', versionChecked: false, sourceVerified: true, sourceKind: 'test_fixture', sourceUrl: null };
    window.__TAURI__ = { core: { invoke: async <T>(command: string, args: Record<string, unknown> = {}) => {
      fixture.requests.push(command); fixture.requestCalls.push({ command, args: { ...args } }); let value: unknown;
      switch (command) {
        case 'bootstrap': value = { storage: 'ready', theme: 'light', schemaVersion: 1, activeProfiles: options.archived ? 0 : 1, monitoring: 'stopped', provider: 'not_configured', keyProtection: 'passphrase', storageIssue: null }; break;
        case 'list_profiles': value = { activeProfiles: options.archived ? 0 : 1, profiles: [{ id: profileId, label: 'Synthetic quota profile', environment: 'native', archived: options.archived ?? false, connectionState: 'authentication_required' }] }; break;
        case 'load_profile_activity': value = null; break;
        case 'load_profile_refresh_status': value = fixture.deferRefresh ? await new Promise((resolve, reject) => { fixture.pendingRefresh.push({ resolve, reject }); }) : structuredClone(fixture.refreshStatus); break;
        case 'load_profile_quota':
          if (fixture.failLoads) throw 'provider_quota_load_failed';
          value = fixture.deferLoads ? await new Promise((resolve, reject) => { fixture.pendingLoads.push({ resolve, reject }); }) : structuredClone(fixture.saved); break;
        case 'inspect_provider_executable': value = { ...metadata, candidateId: '55555555-5555-4555-8555-555555555555' }; break;
        case 'approve_provider_executable': value = { ...metadata, approvalId }; break;
        case 'review_profile_quota_association':
          if (fixture.failReviews) throw typeof fixture.failReviews === 'string' ? fixture.failReviews : 'provider_quota_association_changed';
          value = fixture.deferReviews ? await new Promise((resolve, reject) => { fixture.pendingReviews.push({ resolve, reject }); }) : structuredClone(fixture.review); break;
        case 'begin_profile_quota_reassociation':
        case 'begin_profile_quota_read':
          if (fixture.startError) throw fixture.startError;
          value = { profileId, approvalId, requestId: requestIds[started++], sourceKind: 'test_fixture', monitoring: 'stopped', accepted: false }; break;
        case 'complete_profile_quota_read': value = await new Promise((resolve, reject) => { fixture.completions[String(args.requestId)] = { resolve, reject }; }); break;
        case 'cancel_profile_quota_read': value = await new Promise((resolve, reject) => { fixture.cancellations[String(args.requestId)] = { resolve, reject }; }); break;
        default: throw new Error(`Unexpected renderer fixture command: ${command}`);
      }
      return value as T;
    } } };
  }, { profileId, approvalId, requestIds, reviewId, observedAtUtc, options });
  await page.goto('/');
  await navigate(page, 'accounts');
  if (options.archived) await page.getByText('Archived profiles (1)', { exact: true }).click();
  await openReadings(page, profileId);
  if (options.approve === false) return;
  await navigate(page, 'settings');
  await page.getByRole('heading', { name: 'Codex connection', exact: true }).click();
  await page.locator('[data-provider-manual]').click();
  await page.getByLabel('Codex executable', { exact: true }).fill('/synthetic/reviewed-codex');
  await page.getByRole('button', { name: 'Inspect executable', exact: true }).click();
  await page.getByLabel('I trust this executable for account checks and previews.').check();
  await page.getByRole('button', { name: 'Approve executable', exact: true }).click();
  await navigate(page, 'accounts');

}
async function begin(page: Page, index: number) {
  await page.getByRole('button', { name: 'Read quotas', exact: true }).click();
  await expect(page.locator(`[data-quota-cancel="${profileId}"]`)).toBeFocused();
  await expect.poll(() => page.evaluate(id => Boolean((window as unknown as FixtureWindow).quotaFixture.completions[id]), requestIds[index]!)).toBe(true);
}
async function publish(page: Page, index = 0, cached = saved(index), receipt = committed(index)) {
  await page.evaluate(({ id, value, cached }) => { const fixture = (window as unknown as FixtureWindow).quotaFixture; fixture.saved = cached; fixture.completions[id]!.resolve(value); }, { id: requestIds[index]!, value: receipt, cached });
}
const region = (page: Page) => page.getByRole('region', { name: 'Saved quotas for Synthetic quota profile' });
const remaining = (page: Page) => region(page).getByRole('meter', { name: 'Five-hour: Remaining (derived)' });
async function fail(page: Page, index = 0, error = 'provider_quota_failed') {
  await page.evaluate(({ id, error }) => { (window as unknown as FixtureWindow).quotaFixture.completions[id]!.reject(error); }, { id: requestIds[index]!, error });
}

test('renderer navigation retains a pending read and publishes its result without another provider request', async ({ page }) => {
  await openProfile(page);
  await begin(page, 0);
  for (const view of ['overview', 'usage', 'settings'] as const) {
    await navigate(page, view);
    await expect(page.locator(`[data-quota-cancel="${profileId}"]`)).toBeHidden();
  }
  await publish(page);
  await expect(region(page)).toBeHidden();
  await navigate(page, 'accounts');
  await expect(region(page)).toBeVisible();
  await expect(remaining(page)).toBeVisible();
  await expect(page.locator(`[data-profile-id="${profileId}"] [data-profile-action="readings"]`)).toHaveAttribute('aria-expanded', 'true');
  const calls = await page.evaluate(() => (window as unknown as FixtureWindow).quotaFixture.requestCalls);
  expect(calls.filter(call => call.command === 'begin_profile_quota_read')).toHaveLength(1);
  expect(calls.filter(call => call.command === 'complete_profile_quota_read')).toHaveLength(1);
  expect(calls.filter(call => call.command === 'cancel_profile_quota_read')).toHaveLength(0);
});

for (const archived of [false, true]) test(`renderer loads ${archived ? 'archived' : 'active'} saved quotas without executable approval or focus changes`, async ({ page }) => {
  await openProfile(page, { approve: false, archived, cached: saved() });
  await expect(region(page)).toBeVisible(); await expect(remaining(page)).toBeVisible();
  await expect(region(page)).not.toBeFocused();
  await expect(page.locator('[data-quota-freshness]')).toHaveAttribute('data-quota-freshness', 'unknown');
  await region(page).getByRole('button', { name: 'About these readings', exact: true }).click();
  await expect(region(page).locator('[data-earned-reset-credits]')).toHaveText('9223372036854775807');
  await expect(region(page).locator('[data-credit-detail-state]')).toHaveText('Credit details were not supplied.');
  await region(page).getByRole('button', { name: 'Reading details', exact: true }).click();
  await expect(region(page).locator('[data-monthly-credit-limit]')).toHaveText('9007199254740993.012345678900');
  const calls = await page.evaluate(() => (window as unknown as FixtureWindow).quotaFixture.requestCalls);
  const loads = calls.filter(call => call.command === 'load_profile_quota');
  expect(loads.length).toBeGreaterThan(0);
  for (const load of loads) expect(load.args).toEqual({ profileId });
  expect(calls.filter(call => /begin_|complete_|approve_|inspect_/.test(call.command))).toEqual([]);
});

test('renderer loads null quota as absent and never invents zeros', async ({ page }) => {
  await openProfile(page, { approve: false });
  await expect(region(page)).toHaveCount(0);
  await expect(page.getByRole('meter')).toHaveCount(0);
});

test('renderer focuses committed saved quotas outside closed Manage and survives provider component replacement', async ({ page }) => {
  await openProfile(page); await begin(page, 0); await publish(page);
  await expect(region(page)).toBeFocused(); await expect(remaining(page)).toBeVisible();
  await expect(page.getByRole('button', { name: 'Read activity', exact: true })).toBeHidden();
  await page.getByRole('button', { name: 'Sign in', exact: true }).click();
  await expect(remaining(page)).toBeVisible();
  await page.getByRole('button', { name: 'Back', exact: true }).click();
  await expect(remaining(page)).toBeVisible();
});

test('renderer retains quota values and source time after failed reads while using Rust freshness', async ({ page }) => {
  await openProfile(page, { cached: saved() }); await begin(page, 0);
  await page.evaluate(() => { const fixture = (window as unknown as FixtureWindow).quotaFixture; fixture.saved!.freshness = 'stale'; });
  await fail(page);
  await expect(page.locator('[data-quota-profile]')).toBeFocused();
  await expect(remaining(page)).toBeVisible();
  await expect(page.locator('[data-quota-freshness]')).toHaveText('Stale');
  await expect(region(page).locator('time').first()).toHaveAttribute('datetime', observedAtUtc);
  await begin(page, 1); await expect(remaining(page)).toBeVisible();
  await expect(page.getByText('Codex could not read quotas. Check the connection or sign in to this profile.', { exact: true })).toBeVisible();
});

for (const mismatch of ['profileId', 'bindingId', 'collectionRunId'] as const) test(`renderer rejects saved completion loads with a different ${mismatch} and keeps previous quota values`, async ({ page }) => {
  const old = saved(); await openProfile(page, { cached: old }); await begin(page, 0);
  const next = saved(); next[mismatch] = 'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb';
  await publish(page, 0, next);
  await expect(region(page)).toBeFocused();
  await expect(page.locator('[data-quota-cache-error]')).toHaveText('Saved quotas could not be loaded. Try again.');
  await expect(remaining(page)).toBeVisible();
  await page.evaluate(old => { (window as unknown as FixtureWindow).quotaFixture.saved = old; }, old);
  await page.getByRole('button', { name: 'Reload saved quotas', exact: true }).click();
  await expect(page.locator('[data-quota-cache-error]')).toHaveCount(0);
});

test('renderer discards a late periodic quota load after the explicit committed load', async ({ page }) => {
  await page.clock.install(); await openProfile(page, { cached: saved() });
  await page.evaluate(() => { (window as unknown as FixtureWindow).quotaFixture.deferLoads = true; });
  await page.clock.fastForward(60_000);
  await expect.poll(() => page.evaluate(() => (window as unknown as FixtureWindow).quotaFixture.pendingLoads.length)).toBe(1);
  await begin(page, 0);
  await page.evaluate(() => { (window as unknown as FixtureWindow).quotaFixture.deferLoads = false; });
  const next = saved(); next.limits = []; await publish(page, 0, next);
  await expect(region(page)).toBeFocused(); await expect(remaining(page)).toHaveCount(0);
  await page.evaluate(old => { (window as unknown as FixtureWindow).quotaFixture.pendingLoads[0]!.resolve(old); }, saved());
  await expect(remaining(page)).toHaveCount(0); await expect(region(page)).toBeFocused();
});

test('renderer preserves committed quota focus when a periodic tick occurs during its load', async ({ page }) => {
  await page.clock.install(); await openProfile(page, { cached: saved() }); await begin(page, 0);
  await page.evaluate(() => { (window as unknown as FixtureWindow).quotaFixture.deferLoads = true; });
  await publish(page);
  await expect.poll(() => page.evaluate(() => (window as unknown as FixtureWindow).quotaFixture.pendingLoads.length)).toBe(1);
  await page.clock.fastForward(60_000);
  expect(await page.evaluate(() => (window as unknown as FixtureWindow).quotaFixture.pendingLoads.length)).toBe(1);
  await page.evaluate(value => { (window as unknown as FixtureWindow).quotaFixture.pendingLoads[0]!.resolve(value); }, saved());
  await expect(region(page)).toBeFocused();
});

test('renderer requires displayed quota account review and fresh consent for quota-first reassociation', async ({ page }) => {
  await openProfile(page, { cached: saved() }); await begin(page, 0); await fail(page, 0, 'provider_quota_association_changed');
  const trigger = page.locator('[data-quota-reassociate]'); await trigger.click();
  await expect(page.locator('[data-quota-association=previous]')).toContainText('previous@synthetic.invalid');
  await expect(page.locator('[data-quota-association=current]')).toContainText('current@synthetic.invalid');
  const consent = page.getByRole('checkbox', { name: 'I approve using the current sign-in for this profile.', exact: true });
  await expect(consent).toBeFocused(); await expect(consent).not.toBeChecked();
  await expect(page.getByRole('button', { name: 'Confirm and read', exact: true })).toBeDisabled();
  await consent.check();
  for (const theme of ['light', 'dark']) {
    await page.evaluate(theme => { document.documentElement.dataset.theme = theme; }, theme);
    const audit = await new AxeBuilder({ page }).include('.quota-result button[type="submit"]').withRules(['color-contrast']).analyze();
    expect(audit.violations, `Enabled confirmation label contrast in ${theme} theme`).toEqual([]);
  }
  await page.evaluate(() => { document.documentElement.dataset.theme = 'light'; });
  await page.getByRole('button', { name: 'Keep saved readings', exact: true }).click();
  await expect(trigger).toBeFocused(); await trigger.click(); await expect(consent).not.toBeChecked();
  await consent.check(); await page.getByRole('button', { name: 'Confirm and read', exact: true }).click();
  await expect.poll(() => page.evaluate(() => (window as unknown as FixtureWindow).quotaFixture.requestCalls.filter(call => call.command === 'begin_profile_quota_reassociation'))).toEqual([{ command: 'begin_profile_quota_reassociation', args: { profileId, approvalId, reviewId, confirmed: true } }]);
  const next = saved(1); next.bindingId = 'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb'; next.limits = [];
  await publish(page, 1, next, { ...committed(1), bindingId: next.bindingId });
  await expect(region(page)).toBeFocused(); await expect(remaining(page)).toHaveCount(0);
});

test('renderer offers no blind quota consent when review fails and ignores a late closed review', async ({ page }) => {
  await openProfile(page, { cached: saved() }); await begin(page, 0); await fail(page, 0, 'provider_quota_association_changed');
  await page.evaluate(() => { (window as unknown as FixtureWindow).quotaFixture.failReviews = true; });
  await page.locator('[data-quota-reassociate]').click();
  await expect(page.getByText('The account review could not be loaded. Try again.', { exact: true })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Confirm and read', exact: true })).toHaveCount(0);
  await page.evaluate(() => { const fixture = (window as unknown as FixtureWindow).quotaFixture; fixture.failReviews = false; fixture.deferReviews = true; });
  await page.getByRole('button', { name: 'Retry account review', exact: true }).click();
  await page.getByRole('button', { name: 'Keep saved readings', exact: true }).click();
  await page.evaluate(() => { const fixture = (window as unknown as FixtureWindow).quotaFixture; fixture.pendingReviews[0]!.resolve(fixture.review); });
  await expect(page.locator('[data-quota-association]')).toHaveCount(0); await expect(remaining(page)).toBeVisible();
});

test('renderer preserves Rust due state after a future timestamp, cache failure and recovery', async ({ page }) => {
  const cached = saved(); cached.freshness = 'fresh';
  const limit = cached.limits[0]!, nearReset = '2026-10-18T00:00:30.000Z', futureReset = '2030-01-01T00:00:00.000Z';
  limit.primary!.resetsAtUnix = Date.parse(nearReset) / 1000;
  limit.secondary = { ...limit.primary!, label: 'Seven-day', usedPercent: 100, derivedRemainingPercent: 0, resetsAtUnix: null, resetState: 'expected' };
  cached.limits.push({ ...limit, limitId: 'future', limitName: 'Future limit', primary: { ...limit.primary!, label: 'Primary window', usedPercent: 0, derivedRemainingPercent: 100, resetsAtUnix: Date.parse(futureReset) / 1000 }, secondary: null });
  await page.clock.install({ time: new Date(observedAtUtc) });
  await openProfile(page, { approve: false, cached });
  const root = region(page), windows = root.locator('.quota-window');
  await root.getByRole('button', { name: 'Future limit', exact: true }).click();
  await root.getByRole('button', { name: 'About these readings', exact: true }).click();
  await expect(windows).toHaveCount(3);
  await expect(root.locator('[data-quota-freshness]')).toHaveText('Current');
  await expect(root.locator('[data-quota-reset-due]')).toHaveCount(0);
  await expect(windows.nth(0).locator('time')).toHaveAttribute('datetime', nearReset);
  await expect(windows.nth(1)).toContainText('Not available');
  await expect(windows.nth(2).locator('time')).toHaveAttribute('datetime', futureReset);
  let expectedReset = nearReset;
  const unchanged = async () => {
    for (const [index, value] of ['75', '0', '100'].entries()) await expect(windows.nth(index).getByRole('meter')).toHaveAttribute('aria-valuenow', value);
    await expect(root.locator('[data-earned-reset-credits]')).toHaveText('9223372036854775807');
    await expect(root.locator('.quota-observed time')).toHaveAttribute('datetime', observedAtUtc);
    await expect(windows.nth(0).locator('time')).toHaveAttribute('datetime', expectedReset);
  };
  await unchanged();
  await page.evaluate(() => { (window as unknown as FixtureWindow).quotaFixture.saved!.limits[0]!.primary!.resetState = 'due'; });
  await page.clock.fastForward(60_000);
  await expect(windows.nth(0).locator('[data-quota-reset-due]')).toHaveText('Reset due; awaiting confirmation');
  await expect(windows.nth(1).locator('[data-quota-reset-due]')).toHaveCount(0);
  await expect(windows.nth(2).locator('[data-quota-reset-due]')).toHaveCount(0);
  await unchanged();
  await page.evaluate(futureReset => {
    (window as unknown as FixtureWindow).quotaFixture.saved!.limits[0]!.primary!.resetsAtUnix = Date.parse(futureReset) / 1000;
  }, futureReset);
  expectedReset = futureReset;
  await page.clock.fastForward(60_000);
  await expect(windows.nth(0).locator('time')).toHaveAttribute('datetime', futureReset);
  await expect(windows.nth(0).locator('[data-quota-reset-due]')).toHaveText('Reset due; awaiting confirmation');
  await expect(root.getByText('Reset confirmed', { exact: true })).toHaveCount(0);
  await unchanged();
  await page.evaluate(() => { (window as unknown as FixtureWindow).quotaFixture.failLoads = true; });
  await page.clock.fastForward(60_000);
  await expect(root.locator('[data-quota-cache-error]')).toHaveText('Saved quotas could not be loaded. Try again.');
  await expect(root.locator('[data-quota-freshness]')).toHaveText('Freshness unknown');
  await expect(windows.nth(0).locator('[data-quota-reset-due]')).toBeVisible();
  await unchanged();
  await page.evaluate(() => { const fixture = (window as unknown as FixtureWindow).quotaFixture; fixture.failLoads = false; fixture.saved!.freshness = 'stale'; });
  await root.getByRole('button', { name: 'Reload saved quotas', exact: true }).click();
  await expect(root).toBeFocused();
  await expect(root.locator('[data-quota-cache-error]')).toHaveCount(0);
  await expect(root.locator('[data-quota-freshness]')).toHaveText('Stale');
  await expect(windows.nth(0).locator('[data-quota-reset-due]')).toHaveText('Reset due; awaiting confirmation');
  await unchanged();
  expect(await page.evaluate(() => (window as unknown as FixtureWindow).quotaFixture.requests.filter(command => /begin_|complete_/.test(command)))).toEqual([]);
});

test('renderer shows uncertain reset evidence until Rust accepts confirmation', async ({ page }) => {
  const cached = saved(), futureReset = '2030-01-01T00:00:00.000Z';
  cached.limits[0]!.primary = { ...cached.limits[0]!.primary!, resetsAtUnix: Date.parse(futureReset) / 1000, resetState: 'unconfirmed' };
  await page.clock.install({ time: new Date(observedAtUtc) });
  await openProfile(page, { cached });
  const root = region(page), window = root.locator('.quota-window').first();
  await expect(window.locator('[data-quota-reset-state]')).toHaveText('Reset status not confirmed');
  await page.clock.fastForward(60_000);
  await expect(window.locator('[data-quota-reset-state]')).toHaveText('Reset status not confirmed');
  await expect(window.locator('[data-quota-reset-due]')).toHaveCount(0);
  await expect(remaining(page)).toHaveAttribute('aria-valuenow', '75');
  await begin(page, 0);
  const confirmed = structuredClone(cached); confirmed.limits[0]!.primary!.resetState = 'confirmed';
  await publish(page, 0, confirmed);
  await expect(root).toBeFocused();
  await expect(window.locator('[data-quota-reset-state]')).toHaveText('Reset confirmed');
  await expect(window.locator('[data-quota-reset-state]')).toHaveCount(1);
  await expect(window.locator('[data-quota-reset-due]')).toHaveCount(0);
  await expect(root.getByText('Reset status not confirmed', { exact: true })).toHaveCount(0);
  await expect(root.locator('[data-quota-freshness]')).toHaveText('Freshness unknown');
  await expect(window.locator('time')).toHaveAttribute('datetime', futureReset);
  await expect(remaining(page)).toHaveAttribute('aria-valuenow', '75');
});

// Synthetic renderer clock only; native cooldown acceptance uses real elapsed time.
test('renderer shares the Rust cooldown across quota and activity without polling or blocking sign-in checks', async ({ page }) => {
  await page.clock.install({ time: new Date('2026-10-20T00:00:00Z') });
  await openProfile(page, { refreshSeconds: 3, deferRefresh: true });
  await page.getByRole('button', { name: 'Manage profile', exact: true }).click();
  await page.clock.pauseAt(new Date('2026-10-20T01:00:00Z'));
  await page.evaluate(() => {
    const fixture = (window as unknown as FixtureWindow).quotaFixture;
    fixture.deferRefresh = false;
    for (const pending of fixture.pendingRefresh.splice(0)) pending.resolve(structuredClone(fixture.refreshStatus));
  });
  const notice = page.locator(`[data-profile-refresh-status="${profileId}"]`);
  await expect(notice).toHaveText('Next reading in 3s');
  const initialStatusCalls = await page.evaluate(() => (window as unknown as FixtureWindow).quotaFixture.requests.filter(c => c === 'load_profile_refresh_status').length);
  await expect(notice).not.toHaveAttribute('role', 'status');
  await expect(notice).not.toHaveAttribute('aria-live');
  await expect(page.getByRole('button', { name: 'Read quotas', exact: true })).toBeDisabled();
  await expect(page.getByRole('button', { name: 'Read activity', exact: true })).toBeDisabled();
  await expect(page.getByRole('button', { name: 'Check connection', exact: true })).toBeEnabled();
  await expect(page.getByRole('button', { name: 'Sign in', exact: true })).toBeEnabled();
  await page.clock.setSystemTime(new Date('2020-01-01T00:00:00Z'));
  await page.clock.fastForward(1000);
  await expect(notice).toHaveText('Next reading in 2s');
  await page.clock.fastForward(2000);
  await expect(notice).toHaveCount(0);
  await expect(page.getByRole('button', { name: 'Read quotas', exact: true })).toBeEnabled();
  await expect(page.getByRole('button', { name: 'Read activity', exact: true })).toBeEnabled();
  expect(await page.evaluate(() => (window as unknown as FixtureWindow).quotaFixture.requests.filter(c => c === 'load_profile_refresh_status').length)).toBe(initialStatusCalls);
});

test('renderer reloads cooldown after a failed quota read and retains its accepted values', async ({ page }) => {
  await page.clock.install();
  await openProfile(page, { cached: saved() });
  const initialStatusCalls = await page.evaluate(() => (window as unknown as FixtureWindow).quotaFixture.requests.filter(c => c === 'load_profile_refresh_status').length);
  await begin(page, 0);
  await page.evaluate(profileId => { (window as unknown as FixtureWindow).quotaFixture.refreshStatus = { profileId, retryAfterSeconds: 30 }; }, profileId);
  await fail(page);
  await expect(page.locator(`[data-profile-refresh-status="${profileId}"]`)).toHaveText('Next reading in 30s');
  await expect(remaining(page)).toBeVisible();
  await expect(page.getByRole('button', { name: 'Read quotas', exact: true })).toBeDisabled();
  expect(await page.evaluate(() => (window as unknown as FixtureWindow).quotaFixture.requests.filter(c => c === 'load_profile_refresh_status').length)).toBe(initialStatusCalls + 1);
});

test('renderer validates the exact bounded refresh status contract', async ({ page }) => {
  await openProfile(page);
  const results = await page.evaluate(async profileId => {
    const modulePath = '/src/contracts.ts';
    const { invoke } = await import(modulePath);
    const values: unknown[] = [
      { profileId, retryAfterSeconds: 0 }, { profileId, retryAfterSeconds: 30 }, { profileId, retryAfterSeconds: 4320 },
      { profileId, retryAfterSeconds: -1 }, { profileId, retryAfterSeconds: 4321 },
      { profileId, retryAfterSeconds: 1.5 }, { profileId, retryAfterSeconds: '3' },
      { profileId, retryAfterSeconds: null }, { profileId },
      { profileId, retryAfterSeconds: 0, extra: true }, { profileId: 'email@example.invalid', retryAfterSeconds: 0 },
    ];
    const results = [];
    for (const value of values) {
      (window as unknown as FixtureWindow).quotaFixture.refreshStatus = value;
      try { await invoke('load_profile_refresh_status', { profileId }); results.push(true); }
      catch { results.push(false); }
    }
    return results;
  }, profileId);
  expect(results).toEqual([true, true, true, false, false, false, false, false, false, false, false]);
});


test('renderer refreshes the wait after a cancelled quota read and ignores older status replies', async ({ page }) => {
  await page.clock.install();
  await openProfile(page, { deferRefresh: true });
  await begin(page, 0);
  await page.evaluate(profileId => {
    const fixture = (window as unknown as FixtureWindow).quotaFixture;
    fixture.deferRefresh = false; fixture.refreshStatus = { profileId, retryAfterSeconds: 4320 };
  }, profileId);
  await page.getByRole('button', { name: 'Cancel read', exact: true }).click();
  await page.evaluate(id => {
    const fixture = (window as unknown as FixtureWindow).quotaFixture;
    fixture.cancellations[id]!.resolve({ cancelled: true });
    fixture.completions[id]!.reject('provider_quota_cancelled');
  }, requestIds[0]!);
  const notice = page.locator(`[data-profile-refresh-status="${profileId}"]`);
  await expect(notice).toHaveText('Next reading in 72m 0s');
  await page.evaluate(profileId => {
    for (const pending of (window as unknown as FixtureWindow).quotaFixture.pendingRefresh) pending.resolve({ profileId, retryAfterSeconds: 0 });
  }, profileId);
  await expect(notice).toHaveText('Next reading in 72m 0s');
  await expect(page.getByText('Quota read stopped.', { exact: true })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Read quotas', exact: true })).toBeDisabled();
});

test('renderer explains an authoritative cooldown refusal and loads the remaining wait', async ({ page }) => {
  await page.clock.install();
  await openProfile(page);
  await page.evaluate(profileId => {
    const fixture = (window as unknown as FixtureWindow).quotaFixture;
    fixture.startError = 'provider_refresh_cooldown'; fixture.refreshStatus = { profileId, retryAfterSeconds: 3 };
  }, profileId);
  await page.getByRole('button', { name: 'Read quotas', exact: true }).click();
  await expect(page.getByText('Wait a moment before reading this profile again. Your saved readings are still available.', { exact: true })).toBeVisible();
  await expect(page.locator(`[data-profile-refresh-status="${profileId}"]`)).toHaveText('Next reading in 3s');
  expect(await page.evaluate(() => Object.keys((window as unknown as FixtureWindow).quotaFixture.completions))).toEqual([]);
});


for (const [seconds, initial, next] of [[60, '1m 0s', '59s'], [4320, '72m 0s', '71m 59s']] as const) test(`renderer formats a ${seconds}s transport wait without idle IPC or blocking sign-in`, async ({ page }) => {
  await page.clock.install();
  await openProfile(page, { refreshSeconds: seconds });
  const notice = page.locator(`[data-profile-refresh-status="${profileId}"]`);
  await expect(notice).toHaveText(`Next reading in ${initial}`);
  const initialCalls = await page.evaluate(() => (window as unknown as FixtureWindow).quotaFixture.requests.filter(c => c === 'load_profile_refresh_status').length);
  await expect(notice).not.toHaveAttribute('aria-live');
  await expect(page.getByRole('button', { name: 'Read quotas', exact: true })).toBeDisabled();
  await page.getByRole('button', { name: 'Manage profile', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Read activity', exact: true })).toBeDisabled();
  await expect(page.getByRole('button', { name: 'Check connection', exact: true })).toBeEnabled();
  await expect(page.getByRole('button', { name: 'Sign in', exact: true })).toBeEnabled();
  await page.clock.fastForward(1000);
  await expect(notice).toHaveText(`Next reading in ${next}`);
  expect(await page.evaluate(() => (window as unknown as FixtureWindow).quotaFixture.requests.filter(c => c === 'load_profile_refresh_status').length)).toBe(initialCalls);
});

test('renderer explains an authoritative transport backoff refusal without guessing its cause', async ({ page }) => {
  await page.clock.install(); await openProfile(page);
  await page.evaluate(profileId => {
    const fixture = (window as unknown as FixtureWindow).quotaFixture;
    fixture.startError = 'provider_refresh_backoff'; fixture.refreshStatus = { profileId, retryAfterSeconds: 65 };
  }, profileId);
  await page.getByRole('button', { name: 'Read quotas', exact: true }).click();
  await expect(page.getByText('Codex could not finish the connection. Wait before reading again.', { exact: true })).toBeVisible();
  await expect(page.locator(`[data-profile-refresh-status="${profileId}"]`)).toHaveText('Next reading in 1m 5s');
  expect(await page.evaluate(() => Object.keys((window as unknown as FixtureWindow).quotaFixture.completions))).toEqual([]);
});
