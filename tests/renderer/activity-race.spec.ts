import { expect, test, type Page } from '@playwright/test';
import type { ProfileActivitySavedRead, SavedProfileActivity } from '../../apps/desktop/src/contracts';

// Renderer-only synthetic IPC; SQLCipher, provider ownership and COMMIT are native acceptance scope.
type Deferred = { resolve: (value: unknown) => void; reject: (error: string) => void };
type Fixture = { completions: Record<string, Deferred>; cancellations: Record<string, Deferred>; requests: string[]; requestCalls: { command: string; args: Record<string, unknown> }[]; saved: SavedProfileActivity | null; failLoads: boolean; deferLoads: boolean; pendingLoads: Deferred[] };
type FixtureWindow = Window & { activityFixture: Fixture };
const profileId = '11111111-1111-4111-8111-111111111111';
const approvalId = '22222222-2222-4222-8222-222222222222';
const requestIds = ['33333333-3333-4333-8333-333333333333', '44444444-4444-4444-8444-444444444444'];
const bindingId = '66666666-6666-4666-8666-666666666666';
const runIds = ['77777777-7777-4777-8777-777777777777', '88888888-8888-4888-8888-888888888888'];
const observedAtUtc = '2026-10-18T00:00:00.000Z';
function committed(index: number, rows: { startDate: string; tokens: string }[] | null): ProfileActivitySavedRead {
  return { profileId, approvalId, requestId: requestIds[index]!, collectionRunId: runIds[index]!, bindingId, sourceKind: 'test_fixture', monitoring: 'stopped', accepted: true,
    providerVersion: '0.163.0', observedAtUtc, identityConfidence: 'provisional', verifiedIdentityKey: null, coverage: 'unknown', scope: 'account_activity_with_unknown_application_scope',
    summary: { lifetimeTokens: '9223372036854775807', peakDailyTokens: '9007199254740993', currentStreakDays: null, longestStreakDays: '0', longestRunningTurnSec: '7' }, dailyUsageBuckets: rows };
}
function saved(index = 0, rows: { startDate: string; tokens: string }[] | null = null): SavedProfileActivity {
  const { requestId: _requestId, approvalId: _approvalId, dailyUsageBuckets, ...value } = committed(index, rows);
  return { ...value, freshness: 'unknown', dailyState: rows === null ? 'unavailable' : rows.length === 0 ? 'empty' : 'returned', dailyTruncated: false,
    dailyUsageBuckets: (dailyUsageBuckets ?? []).map(row => ({ date: row.startDate, tokens: row.tokens, observedAtUtc, freshness: 'unknown' })) };
}
async function openProfile(page: Page, options: { approve?: boolean; cached?: SavedProfileActivity | null; archived?: boolean } = {}) {
  await page.addInitScript(({ profileId, approvalId, requestIds, options }) => {
    const fixture: Fixture = { completions: {}, cancellations: {}, requests: [], requestCalls: [], saved: options.cached ?? null, failLoads: false, deferLoads: false, pendingLoads: [] };
    Object.assign(window, { activityFixture: fixture }); let started = 0;
    const metadata = { path: '/synthetic/reviewed-codex', fileBytes: '100', sha256: 'a'.repeat(64), testedVersion: '0.163.0', versionChecked: false, sourceVerified: true, sourceKind: 'test_fixture', sourceUrl: null };
    window.__TAURI__ = { core: { invoke: async <T>(command: string, args: Record<string, unknown> = {}) => {
      fixture.requests.push(command); fixture.requestCalls.push({ command, args: { ...args } }); let value: unknown;
      switch (command) {
        case 'bootstrap': value = { storage: 'ready', theme: 'light', schemaVersion: 1, activeProfiles: options.archived ? 0 : 1, monitoring: 'stopped', provider: 'not_configured', keyProtection: 'passphrase', storageIssue: null }; break;
        case 'list_profiles': value = { activeProfiles: options.archived ? 0 : 1, profiles: [{ id: profileId, label: 'Synthetic activity profile', environment: 'native', archived: options.archived ?? false, connectionState: 'authentication_required' }] }; break;
        case 'load_profile_activity':
          if (fixture.failLoads) throw 'provider_activity_load_failed';
          value = fixture.deferLoads ? await new Promise((resolve, reject) => { fixture.pendingLoads.push({ resolve, reject }); }) : structuredClone(fixture.saved); break;
        case 'inspect_provider_executable': value = { ...metadata, candidateId: '55555555-5555-4555-8555-555555555555' }; break;
        case 'approve_provider_executable': value = { ...metadata, approvalId }; break;
        case 'begin_profile_activity_reassociation':
        case 'begin_profile_activity_read': value = { profileId, approvalId, requestId: requestIds[started++], sourceKind: 'test_fixture', monitoring: 'stopped', accepted: false }; break;
        case 'complete_profile_activity_read': value = await new Promise((resolve, reject) => { fixture.completions[String(args.requestId)] = { resolve, reject }; }); break;
        case 'cancel_profile_activity_read': value = await new Promise((resolve, reject) => { fixture.cancellations[String(args.requestId)] = { resolve, reject }; }); break;
        case 'begin_profile_quota_read': value = { profileId, approvalId, requestId: requestIds[1], sourceKind: 'test_fixture', monitoring: 'stopped', accepted: false }; break;
        case 'complete_profile_quota_read': value = await new Promise((resolve, reject) => { fixture.completions[`quota:${String(args.requestId)}`] = { resolve, reject }; }); break;
        default: throw new Error(`Unexpected renderer fixture command: ${command}`);
      }
      return value as T;
    } } };
  }, { profileId, approvalId, requestIds, options });
  await page.goto('/');
  if (options.archived) await page.getByText('Archived profiles (1)', { exact: true }).click();
  if (options.approve === false) return;
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
async function publish(page: Page, index: number, rows: { startDate: string; tokens: string }[] | null, cached = saved(index, rows), receipt = { ...committed(index, rows), summary: cached.summary, collectionRunId: cached.collectionRunId, bindingId: cached.bindingId }) {
  await page.evaluate(({ id, value, cached }) => { const fixture = (window as unknown as FixtureWindow).activityFixture; fixture.saved = cached; fixture.completions[id]!.resolve(value); }, { id: requestIds[index]!, value: receipt, cached });
  await expect(page.getByRole('region', { name: 'Saved activity for Synthetic activity profile' })).toBeFocused();
}
const metric = (page: Page, name: string) => page.locator(`[data-activity-saved] [data-activity-metric="${name}"]`);

test('renderer saves activity outside Manage and pages exact cached rows after committed completion', async ({ page }) => {
  await openProfile(page); await begin(page, 0);
  await page.getByRole('button', { name: 'Manage profile', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Read activity', exact: true })).toBeHidden();
  await expect(page.getByRole('button', { name: 'Cancel activity read', exact: true })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Sign in', exact: true })).toBeDisabled();
  await expect(page.getByRole('button', { name: 'Read quotas', exact: true })).toBeDisabled();
  const rows = Array.from({ length: 51 }, (_, index) => ({ startDate: index === 0 ? '2024-02-29' : new Date(Date.UTC(2023, 0, index)).toISOString().slice(0, 10), tokens: index === 0 ? '9223372036854775807' : '0' }));
  await publish(page, 0, rows);
  await expect(metric(page, 'lifetimeTokens')).toHaveText('9223372036854775807');
  await expect(metric(page, 'currentStreakDays')).toHaveText('Not available');
  await expect(page.getByText('Unverified identity · Saved locally')).toBeVisible();
  await expect(page.locator('[data-activity-day]')).toHaveCount(0);
  await page.getByRole('button', { name: 'Activity details', exact: true }).click();
  await expect(page.locator('[data-activity-day]')).toHaveCount(50);
  await expect(page.locator('[data-activity-day]').first().locator('time').first()).toHaveText('2024-02-29');
  await expect(page.locator('[data-activity-day]').first().locator('.activity-count')).toHaveText('9223372036854775807');
  await expect(page.getByRole('button', { name: 'Previous', exact: true })).toBeDisabled();
  await page.getByRole('button', { name: 'Next', exact: true }).click();
  await expect(page.locator('[data-activity-day]')).toHaveCount(1);
  await expect(page.getByText('Rows 51–51 of 51 shown')).toBeVisible();
  await expect(page.getByRole('button', { name: 'Next', exact: true })).toBeDisabled();
  await page.getByRole('button', { name: 'Previous', exact: true }).click();
  await expect(page.locator('[data-activity-day]')).toHaveCount(50);
});

test('renderer isolates an old cancel failure from the successor saved activity', async ({ page }) => {
  await openProfile(page); await begin(page, 0);
  await page.getByRole('button', { name: 'Cancel activity read', exact: true }).click();
  await page.evaluate(id => { (window as unknown as FixtureWindow).activityFixture.completions[id]!.reject('provider_activity_cancelled'); }, requestIds[0]!);
  await expect(page.getByText('Activity read stopped.', { exact: true })).toBeVisible();
  await begin(page, 1);
  await page.evaluate(id => { (window as unknown as FixtureWindow).activityFixture.cancellations[id]!.reject('provider_cleanup_failed'); }, requestIds[0]!);
  await expect(page.getByRole('button', { name: 'Cancel activity read', exact: true })).toBeEnabled();
  await publish(page, 1, []);
  await page.getByRole('button', { name: 'Activity details', exact: true }).click();
  await expect(page.locator('[data-activity-daily-state=empty]')).toHaveText('The latest read returned no daily rows.');
  await expect(page.getByText('OpenUsage could not confirm that the provider process stopped.', { exact: false })).toHaveCount(0);
});

test('renderer rejects incompatible completion while keeping the last accepted saved values', async ({ page }) => {
  await openProfile(page, { cached: saved() }); await begin(page, 0);
  await page.evaluate(({ id, value }) => { (window as unknown as FixtureWindow).activityFixture.completions[id]!.resolve({ ...value, coverage: 'complete', accountId: 'private' }); }, { id: requestIds[0]!, value: committed(0, []) });
  await expect(page.locator('.activity-result')).toHaveAttribute('role', 'alert');
  await expect(metric(page, 'lifetimeTokens')).toHaveText('9223372036854775807');
  await expect(page.getByRole('heading', { name: 'Saved activity', exact: true })).toBeVisible();
});

test('renderer keeps a new quota read focused when an old activity cancellation rejects', async ({ page }) => {
  await openProfile(page); await begin(page, 0);
  await page.getByRole('button', { name: 'Cancel activity read', exact: true }).click();
  await page.evaluate(id => { (window as unknown as FixtureWindow).activityFixture.completions[id]!.reject('provider_activity_cancelled'); }, requestIds[0]!);
  await expect(page.getByText('Activity read stopped.', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Read quotas', exact: true }).click();
  const cancelQuota = page.locator(`[data-quota-cancel="${profileId}"]`); await expect(cancelQuota).toBeFocused();
  await page.evaluate(id => { (window as unknown as FixtureWindow).activityFixture.cancellations[id]!.reject('provider_cleanup_failed'); }, requestIds[0]!);
  await expect(cancelQuota).toBeFocused(); await expect(cancelQuota).toBeEnabled(); await expect(page.locator('.activity-result')).toHaveCount(0);
  await page.evaluate(({ profileId, approvalId, requestId }) => {
    (window as unknown as FixtureWindow).activityFixture.completions[`quota:${requestId}`]!.resolve({ profileId, approvalId, requestId, sourceKind: 'test_fixture', monitoring: 'stopped', accepted: false,
      providerVersion: '0.163.0', observedAtUtc: '2026-10-18T00:00:00.000Z', identityConfidence: 'provisional', verifiedIdentityKey: null, availableResetCredits: null, resetCreditDetails: null, limits: [] });
  }, { profileId, approvalId, requestId: requestIds[1]! });
  await expect(page.getByRole('region', { name: 'Quota preview for Synthetic activity profile' })).toBeFocused();
  await expect(page.locator('.activity-result')).toHaveCount(0);
});

for (const archived of [false, true]) test(`renderer loads ${archived ? 'archived' : 'active'} saved activity without provider approval`, async ({ page }) => {
  const cached = saved(0, [{ startDate: '2024-02-29', tokens: '9007199254740993' }]);
  await openProfile(page, { approve: false, cached, archived });
  await expect(metric(page, 'lifetimeTokens')).toHaveText('9223372036854775807');
  await expect(page.locator('[data-activity-freshness]')).toHaveText('Freshness unknown');
  await expect(page.getByRole('button', { name: 'Read activity', exact: true })).toHaveCount(0);
  await page.getByRole('button', { name: 'Activity details', exact: true }).click();
  await expect(page.locator('[data-activity-day] time').first()).toHaveText('2024-02-29');
  const requests = await page.evaluate(() => (window as unknown as FixtureWindow).activityFixture.requests);
  expect(requests).toContain('load_profile_activity'); expect(requests.some(command => command.startsWith('begin_profile_'))).toBe(false);
});

test('renderer preserves saved daily rows and their ages when a saved replacement has null metrics and no daily array', async ({ page }) => {
  const cached = saved(0, [{ startDate: '2024-02-29', tokens: '9007199254740993' }]);
  cached.freshness = 'fresh'; cached.dailyUsageBuckets[0]!.freshness = 'fresh';
  await openProfile(page, { cached }); await begin(page, 0);
  await expect(page.locator('[data-activity-freshness]')).toHaveText('Current');
  await page.evaluate(id => { const fixture = (window as unknown as FixtureWindow).activityFixture; fixture.saved!.freshness = 'stale'; fixture.saved!.dailyUsageBuckets[0]!.freshness = 'stale'; fixture.completions[id]!.reject('provider_activity_save_failed'); }, requestIds[0]!);
  await expect(page.locator('[data-activity-freshness]')).toHaveText('Stale');
  await expect(page.locator('.activity-result')).toBeFocused();
  await expect(page.getByText('The reading could not be saved. Your previous activity is still available.')).toBeVisible();
  await expect(metric(page, 'lifetimeTokens')).toHaveText('9223372036854775807');
  await begin(page, 1);
  const next = saved(1); next.summary = { lifetimeTokens: null, peakDailyTokens: null, longestRunningTurnSec: null, currentStreakDays: null, longestStreakDays: null }; next.freshness = 'fresh'; next.dailyUsageBuckets = cached.dailyUsageBuckets.map(row => ({ ...row, freshness: 'stale' }));
  await publish(page, 1, null, next);
  await expect(metric(page, 'lifetimeTokens')).toHaveText('Not available');
  await expect(page.getByText('The reading could not be saved. Your previous activity is still available.')).toHaveCount(0);
  await page.getByRole('button', { name: 'Activity details', exact: true }).click();
  await expect(page.locator('[data-activity-daily-state=unavailable]')).toBeVisible();
  await expect(page.locator('[data-activity-day]')).toHaveCount(1);
  await expect(page.locator('[data-activity-day]')).toHaveAttribute('data-activity-day-freshness', 'stale');
  await expect(page.locator('[data-activity-observed]')).toHaveAttribute('datetime', observedAtUtc);
});

test('renderer refreshes cached state through Rust and retries load failures without hiding saved values', async ({ page }) => {
  await page.clock.install(); await openProfile(page, { approve: false, cached: saved() });
  await expect(metric(page, 'lifetimeTokens')).toBeVisible();
  await page.evaluate(() => { (window as unknown as FixtureWindow).activityFixture.failLoads = true; });
  await page.clock.fastForward(60_000);
  await expect(page.getByText('Saved activity could not be loaded. Try again.')).toBeVisible();
  await expect(metric(page, 'lifetimeTokens')).toHaveText('9223372036854775807');
  await page.evaluate(() => { const fixture = (window as unknown as FixtureWindow).activityFixture; fixture.failLoads = false; fixture.saved!.freshness = 'stale'; });
  await page.getByRole('button', { name: 'Reload saved activity', exact: true }).click();
  await expect(page.locator('[data-activity-freshness]')).toHaveText('Stale');
  await expect(page.getByText('Saved activity could not be loaded. Try again.')).toHaveCount(0);
});

test('renderer refuses an older background cache response after a newly committed reading loads', async ({ page }) => {
  await page.clock.install(); const old = saved(); old.summary.lifetimeTokens = '1'; await openProfile(page, { cached: old });
  await expect(metric(page, 'lifetimeTokens')).toHaveText('1');
  await page.evaluate(() => { (window as unknown as FixtureWindow).activityFixture.deferLoads = true; });
  await page.clock.fastForward(60_000);
  await expect.poll(() => page.evaluate(() => (window as unknown as FixtureWindow).activityFixture.pendingLoads.length)).toBe(1);
  await begin(page, 0);
  await page.evaluate(() => { (window as unknown as FixtureWindow).activityFixture.deferLoads = false; });
  await publish(page, 0, null);
  await page.evaluate(old => { (window as unknown as FixtureWindow).activityFixture.pendingLoads[0]!.resolve(old); }, old);
  await expect(metric(page, 'lifetimeTokens')).toHaveText('9223372036854775807');
  await expect(page.getByRole('region', { name: 'Saved activity for Synthetic activity profile' })).toBeFocused();
});


test('renderer preserves manual saved-result focus when the sixty-second cache tick occurs during its load', async ({ page }) => {
  await page.clock.install(); await openProfile(page, { cached: saved() }); await begin(page, 0);
  const cached = saved();
  await page.evaluate(({ id, value, cached }) => { const fixture = (window as unknown as FixtureWindow).activityFixture; fixture.deferLoads = true; fixture.saved = cached; fixture.completions[id]!.resolve(value); }, { id: requestIds[0]!, value: committed(0, null), cached });
  await expect.poll(() => page.evaluate(() => (window as unknown as FixtureWindow).activityFixture.pendingLoads.length)).toBe(1);
  await page.clock.fastForward(60_000);
  expect(await page.evaluate(() => (window as unknown as FixtureWindow).activityFixture.pendingLoads.length)).toBe(1);
  await page.evaluate(cached => { const fixture = (window as unknown as FixtureWindow).activityFixture; fixture.deferLoads = false; fixture.pendingLoads[0]!.resolve(cached); }, cached);
  await expect(page.getByRole('region', { name: 'Saved activity for Synthetic activity profile' })).toBeFocused();
  await expect(metric(page, 'lifetimeTokens')).toHaveText('9223372036854775807');
});


async function offerReassociation(page: Page) {
  await begin(page, 0);
  await page.evaluate(id => { (window as unknown as FixtureWindow).activityFixture.completions[id]!.reject('provider_activity_association_changed'); }, requestIds[0]!);
  await expect(page.getByRole('button', { name: 'Use current sign-in', exact: true })).toBeVisible();
}
async function confirmReassociation(page: Page) {
  await page.getByRole('button', { name: 'Use current sign-in', exact: true }).click();
  await page.getByRole('checkbox', { name: 'I approve using the current sign-in for this profile.', exact: true }).check();
  await page.getByRole('button', { name: 'Confirm and read', exact: true }).click();
  await expect(page.locator(`[data-activity-cancel="${profileId}"]`)).toBeFocused();
  await expect.poll(() => page.evaluate(id => Boolean((window as unknown as FixtureWindow).activityFixture.completions[id]), requestIds[1]!)).toBe(true);
}

test('renderer requires fresh explicit reassociation consent and Keep saved readings sends no request', async ({ page }) => {
  const cached = saved(); cached.summary.lifetimeTokens = '1';
  await openProfile(page, { cached }); await offerReassociation(page);
  const trigger = page.getByRole('button', { name: 'Use current sign-in', exact: true });
  await trigger.click();
  const consent = page.getByRole('checkbox', { name: 'I approve using the current sign-in for this profile.', exact: true });
  await expect(consent).not.toBeChecked(); await expect(consent).toBeFocused();
  await expect(page.getByRole('button', { name: 'Confirm and read', exact: true })).toBeDisabled();
  await expect(page.getByText('New readings will use the current sign-in. Previous readings stay in encrypted history.')).toBeVisible();
  await page.setViewportSize({ width: 360, height: 900 });
  await page.evaluate(() => { const root = document.querySelector('.activity-result')!; const nodes = [root, ...root.querySelectorAll('*')]; const sizes = nodes.map(node => ({ node: node as HTMLElement, size: getComputedStyle(node).fontSize })); for (const { node, size } of sizes) node.style.fontSize = `${parseFloat(size) * 2}px`; });
  expect(await page.evaluate(() => { const root = document.querySelector('.activity-result')!; return { pageWidth: document.documentElement.scrollWidth, viewport: innerWidth, buttons: [...root.querySelectorAll('button')].filter(node => node.getClientRects().length).map(node => ({ text: node.textContent?.trim(), width: node.getBoundingClientRect().width, container: root.getBoundingClientRect().width })) }; })).toMatchObject({ pageWidth: 360, viewport: 360 });

  await consent.check(); await expect(page.getByRole('button', { name: 'Confirm and read', exact: true })).toBeEnabled();
  await page.getByRole('button', { name: 'Keep saved readings', exact: true }).click();
  await expect(trigger).toBeFocused(); await expect(page.getByRole('button', { name: 'Confirm and read', exact: true })).toHaveCount(0);
  await trigger.click(); await expect(consent).not.toBeChecked();
  await expect(page.getByRole('button', { name: 'Confirm and read', exact: true })).toBeDisabled();
  await expect(metric(page, 'lifetimeTokens')).toHaveText('1');
  expect(await page.evaluate(() => (window as unknown as FixtureWindow).activityFixture.requests.filter(command => command === 'begin_profile_activity_reassociation'))).toEqual([]);
});

test('renderer never offers reassociation after an unsafe account change during the read', async ({ page }) => {
  await openProfile(page, { cached: saved() }); await begin(page, 0);
  await page.evaluate(id => { (window as unknown as FixtureWindow).activityFixture.completions[id]!.reject('provider_account_changed'); }, requestIds[0]!);
  await expect(page.getByText('The Codex account information changed during this read. Check the intended account, then read again.')).toBeVisible();
  await expect(page.getByRole('button', { name: 'Use current sign-in', exact: true })).toHaveCount(0);
  await expect(page.getByRole('checkbox', { name: 'I approve using the current sign-in for this profile.', exact: true })).toHaveCount(0);
  await expect(metric(page, 'lifetimeTokens')).toHaveText('9223372036854775807');
});

test('renderer uses the confirmed reassociation command and displays the newly committed binding', async ({ page }) => {
  const cached = saved(); cached.summary.lifetimeTokens = '1';
  await openProfile(page, { cached }); await offerReassociation(page); await confirmReassociation(page);
  const calls = await page.evaluate(() => (window as unknown as FixtureWindow).activityFixture.requestCalls.filter(call => call.command === 'begin_profile_activity_reassociation'));
  expect(calls).toEqual([{ command: 'begin_profile_activity_reassociation', args: { profileId, approvalId, confirmed: true } }]);
  await expect(metric(page, 'lifetimeTokens')).toHaveText('1');
  const next = saved(1); next.bindingId = '99999999-9999-4999-8999-999999999999'; next.summary.lifetimeTokens = '2';
  await publish(page, 1, null, next);
  await expect(metric(page, 'lifetimeTokens')).toHaveText('2');
  await expect(page.getByRole('button', { name: 'Use current sign-in', exact: true })).toHaveCount(0);
  await expect(page.getByRole('checkbox', { name: 'I approve using the current sign-in for this profile.', exact: true })).toHaveCount(0);
});

test('renderer keeps old saved values when confirmed reassociation fails before commit', async ({ page }) => {
  const cached = saved(); cached.summary.lifetimeTokens = '1';
  await openProfile(page, { cached }); await offerReassociation(page); await confirmReassociation(page);
  await page.evaluate(id => { (window as unknown as FixtureWindow).activityFixture.completions[id]!.reject('provider_activity_save_failed'); }, requestIds[1]!);
  await expect(page.getByText('The reading could not be saved. Your previous activity is still available.')).toBeVisible();
  await expect(metric(page, 'lifetimeTokens')).toHaveText('1');
  await expect(page.getByRole('checkbox', { name: 'I approve using the current sign-in for this profile.', exact: true })).toHaveCount(0);
});

test('renderer refuses a saved load with the wrong binding after reassociation completion', async ({ page }) => {
  const cached = saved(); cached.summary.lifetimeTokens = '1';
  await openProfile(page, { cached }); await offerReassociation(page); await confirmReassociation(page);
  await page.evaluate(({ id, receipt }) => { (window as unknown as FixtureWindow).activityFixture.completions[id]!.resolve(receipt); }, { id: requestIds[1]!, receipt: { ...committed(1, null), bindingId: '99999999-9999-4999-8999-999999999999' } });
  await expect(page.getByText('Saved activity could not be loaded. Try again.')).toBeVisible();
  await expect(metric(page, 'lifetimeTokens')).toHaveText('1');
  await expect(page.getByRole('button', { name: 'Reload saved activity', exact: true })).toBeVisible();
});
