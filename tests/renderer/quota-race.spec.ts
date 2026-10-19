import { expect, test, type Page } from '@playwright/test';
import type { ProfileQuotaSavedRead, SavedProfileQuota } from '../../apps/desktop/src/contracts';

// Renderer-only synthetic IPC; SQLCipher, provider ownership and COMMIT are native acceptance scope.
type Deferred = { resolve: (value: unknown) => void; reject: (error: string) => void };
type Fixture = { completions: Record<string, Deferred>; cancellations: Record<string, Deferred>; requests: string[]; requestCalls: { command: string; args: Record<string, unknown> }[]; saved: SavedProfileQuota | null; failLoads: boolean; deferLoads: boolean; pendingLoads: Deferred[]; review: unknown; failReviews: boolean | string; deferReviews: boolean; pendingReviews: Deferred[] };
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
  return { ...value, freshness: 'unknown', coverage: 'unknown', scope: 'account_quota_with_unknown_application_scope' };
}
async function openProfile(page: Page, options: { approve?: boolean; cached?: SavedProfileQuota | null; archived?: boolean } = {}) {
  await page.addInitScript(({ profileId, approvalId, requestIds, reviewId, observedAtUtc, options }) => {
    const fixture: Fixture = { completions: {}, cancellations: {}, requests: [], requestCalls: [], saved: options.cached ?? null, failLoads: false, deferLoads: false, pendingLoads: [], review: { reviewId, profileId, approvalId, observedAtUtc, previous: { email: 'previous@synthetic.invalid', plan: null }, current: { email: 'current@synthetic.invalid', plan: 'Synthetic plan' }, homeChanged: true, identityConfidence: 'provisional' }, failReviews: false, deferReviews: false, pendingReviews: [] };
    Object.assign(window, { quotaFixture: fixture }); let started = 0;
    const metadata = { path: '/synthetic/reviewed-codex', fileBytes: '100', sha256: 'a'.repeat(64), testedVersion: '0.163.0', versionChecked: false, sourceVerified: true, sourceKind: 'test_fixture', sourceUrl: null };
    window.__TAURI__ = { core: { invoke: async <T>(command: string, args: Record<string, unknown> = {}) => {
      fixture.requests.push(command); fixture.requestCalls.push({ command, args: { ...args } }); let value: unknown;
      switch (command) {
        case 'bootstrap': value = { storage: 'ready', theme: 'light', schemaVersion: 1, activeProfiles: options.archived ? 0 : 1, monitoring: 'stopped', provider: 'not_configured', keyProtection: 'passphrase', storageIssue: null }; break;
        case 'list_profiles': value = { activeProfiles: options.archived ? 0 : 1, profiles: [{ id: profileId, label: 'Synthetic quota profile', environment: 'native', archived: options.archived ?? false, connectionState: 'authentication_required' }] }; break;
        case 'load_profile_activity': value = null; break;
        case 'load_profile_quota':
          if (fixture.failLoads) throw 'provider_quota_load_failed';
          value = fixture.deferLoads ? await new Promise((resolve, reject) => { fixture.pendingLoads.push({ resolve, reject }); }) : structuredClone(fixture.saved); break;
        case 'inspect_provider_executable': value = { ...metadata, candidateId: '55555555-5555-4555-8555-555555555555' }; break;
        case 'approve_provider_executable': value = { ...metadata, approvalId }; break;
        case 'review_profile_quota_association':
          if (fixture.failReviews) throw typeof fixture.failReviews === 'string' ? fixture.failReviews : 'provider_quota_association_changed';
          value = fixture.deferReviews ? await new Promise((resolve, reject) => { fixture.pendingReviews.push({ resolve, reject }); }) : structuredClone(fixture.review); break;
        case 'begin_profile_quota_reassociation':
        case 'begin_profile_quota_read': value = { profileId, approvalId, requestId: requestIds[started++], sourceKind: 'test_fixture', monitoring: 'stopped', accepted: false }; break;
        case 'complete_profile_quota_read': value = await new Promise((resolve, reject) => { fixture.completions[String(args.requestId)] = { resolve, reject }; }); break;
        case 'cancel_profile_quota_read': value = await new Promise((resolve, reject) => { fixture.cancellations[String(args.requestId)] = { resolve, reject }; }); break;
        default: throw new Error(`Unexpected renderer fixture command: ${command}`);
      }
      return value as T;
    } } };
  }, { profileId, approvalId, requestIds, reviewId, observedAtUtc, options });
  await page.goto('/');
  if (options.archived) await page.getByText('Archived profiles (1)', { exact: true }).click();
  if (options.approve === false) return;
  await page.getByRole('heading', { name: 'Codex connection', exact: true }).click();
  await page.locator('[data-provider-manual]').click();
  await page.getByLabel('Codex executable', { exact: true }).fill('/synthetic/reviewed-codex');
  await page.getByRole('button', { name: 'Inspect executable', exact: true }).click();
  await page.getByLabel('I trust this executable for account checks and previews.').check();
  await page.getByRole('button', { name: 'Approve executable', exact: true }).click();

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
  await consent.check(); await page.getByRole('button', { name: 'Keep saved readings', exact: true }).click();
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
