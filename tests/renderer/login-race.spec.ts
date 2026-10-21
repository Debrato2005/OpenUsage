import { navigate, openReadings, openOptions } from './navigation';
import { expect, test, type Page } from '@playwright/test';
import type { ProfileQuotaSavedRead } from '../../apps/desktop/src/contracts';

// Renderer-only synthetic IPC. No native process, credential home, network login, or storage.
type Deferred = { resolve: (value: unknown) => void; reject: (error: string) => void };
type LoginFixture = {
  completions: Record<string, Deferred>;
  cancellations: Record<string, Deferred>;
  savedQuota: unknown;
  created: boolean;
  connectionState: 'authentication_required' | 'connected' | 'error';
  failProfiles: boolean; deferProfiles: boolean; profileLoads: Deferred[];
  requests: { command: string; args: Record<string, unknown> }[];
};
type FixtureWindow = Window & { loginFixture: LoginFixture };

const profileId = '11111111-1111-4111-8111-111111111111';
const approvalId = '22222222-2222-4222-8222-222222222222';
const attempts = [
  '33333333-3333-4333-8333-333333333333',
  '44444444-4444-4444-8444-444444444444',
];

async function openSyntheticProfile(page: Page, newAccount = false) {
  await page.addInitScript(({ profileId, approvalId, attempts, newAccount }) => {
    const fixture: LoginFixture = { completions: {}, cancellations: {}, requests: [], savedQuota: null, created: !newAccount, connectionState: 'authentication_required', failProfiles: false, deferProfiles: false, profileLoads: [] };
    Object.assign(window, { loginFixture: fixture });
    let started = 0;
    const deferred = (target: Record<string, Deferred>, id: string) => new Promise((resolve, reject) => {
      target[id] = { resolve, reject };
    });
    const metadata = {
      path: '/synthetic/reviewed-codex', fileBytes: '100', sha256: 'a'.repeat(64),
      testedVersion: '0.163.0', versionChecked: false, sourceVerified: true,
      sourceKind: 'test_fixture', sourceUrl: null,
    };
    window.__TAURI__ = { core: { invoke: async <T>(command: string, args: Record<string, unknown> = {}) => {
      fixture.requests.push({ command, args });
      let value: unknown;
      switch (command) {
        case 'load_monitoring': value = { state: 'stopped', readingRevision: 'cccccccc-cccc-4ccc-8ccc-cccccccccccc', profiles: [] }; break;
        case 'bootstrap':
          value = { storage: 'ready', theme: 'light', schemaVersion: 1, activeProfiles: newAccount ? 0 : 1,
            monitoring: 'stopped', provider: 'not_configured', keyProtection: 'passphrase', storageIssue: null };
          break;
        case 'load_profile_activity': value = null; break;
        case 'load_profile_refresh_status': value = { profileId, retryAfterSeconds: 0 }; break;
        case 'load_profile_quota': value = structuredClone(fixture.savedQuota); break;
        case 'create_profile':
          fixture.created = true;
          value = { activeProfiles: 1, profiles: [{ id: profileId, label: 'Synthetic login profile', environment: 'native', archived: false, connectionState: fixture.connectionState }] };
          break;
        case 'list_profiles':
          if (fixture.failProfiles) throw 'profile_unavailable';
          if (fixture.deferProfiles) { value = await new Promise((resolve, reject) => { fixture.profileLoads.push({ resolve, reject }); }); break; }
          value = { activeProfiles: fixture.created ? 1 : 0, profiles: fixture.created ? [{ id: profileId, label: 'Synthetic login profile',
            environment: 'native', archived: false, connectionState: fixture.connectionState }] : [] };
          break;
        case 'inspect_provider_executable':
          value = { ...metadata, candidateId: '55555555-5555-4555-8555-555555555555' };
          break;
        case 'approve_provider_executable':
          value = { ...metadata, approvalId };
          break;
        case 'begin_profile_login':
          value = { profileId, approvalId, attemptId: attempts[started++], sourceKind: 'test_fixture',
            monitoring: 'stopped', applicationWaitLimitSeconds: 900 };
          break;
        case 'await_profile_login_prompt':
          value = { profileId, attemptId: args.attemptId, sourceKind: 'test_fixture',
            verificationUrl: 'https://auth.openai.com/codex/device',
            userCode: args.attemptId === attempts[0] ? 'SYNTH-OLD' : 'SYNTH-NEW', applicationWaitLimitSeconds: 900 };
          break;
        case 'complete_profile_login':
          value = await deferred(fixture.completions, String(args.attemptId));
          if (typeof value === 'object' && value !== null && 'diagnostic' in value) {
            const diagnostic = (value as { diagnostic: { persisted?: boolean; identityConfidence?: string; authentication?: string } }).diagnostic;
            if (diagnostic.persisted && diagnostic.authentication === 'chatgpt') fixture.connectionState = diagnostic.identityConfidence === 'provisional' ? 'connected' : 'error';
          }
          break;
        case 'cancel_profile_login':
          value = await deferred(fixture.cancellations, String(args.attemptId));
          break;
        case 'check_profile_connection':
          value = await deferred(fixture.completions, `check:${profileId}`);
          break;
        case 'cancel_profile_check':
          value = await deferred(fixture.cancellations, `check:${profileId}`);
          break;
        case 'begin_profile_collection_read':
          value = { profileId, approvalId, requestId: attempts[0], sourceKind: 'test_fixture', monitoring: 'stopped', accepted: false };
          break;
        case 'complete_profile_collection_read':
          value = await deferred(fixture.completions, `collection:${String(args.requestId)}`);
          break;
        case 'cancel_profile_collection_read':
          value = { cancelled: true };
          break;
        case 'begin_profile_quota_read':
          value = { profileId, approvalId, requestId: attempts[0], sourceKind: 'test_fixture',
            monitoring: 'stopped', accepted: false };
          break;
        case 'cancel_profile_quota_read':
          value = await deferred(fixture.cancellations, `quota:${String(args.requestId)}`);
          break;
        case 'complete_profile_quota_read':
          value = await deferred(fixture.completions, `quota:${String(args.requestId)}`);
          if (typeof value === 'object' && value !== null && 'bindingId' in value) {
            const { requestId: _requestId, approvalId: _approvalId, ...saved } = value as ProfileQuotaSavedRead;
            fixture.savedQuota = { ...saved, limits: saved.limits.map(limit => ({ ...limit, primary: limit.primary && { ...limit.primary, resetState: 'expected' }, secondary: limit.secondary && { ...limit.secondary, resetState: 'expected' } })), freshness: 'unknown', coverage: 'unknown', scope: 'account_quota_with_unknown_application_scope' };
          }
          break;
        default: throw new Error(`Unexpected renderer fixture command: ${command}`);
      }
      return value as T;
    } } };
  }, { profileId, approvalId, attempts, newAccount });
  await page.goto('/');
  await navigate(page, 'accounts');
  if (!newAccount) await openReadings(page, profileId);
  await navigate(page, 'settings');
  await page.getByRole('heading', { name: 'Codex connection', exact: true }).click();
  await page.locator('[data-provider-manual]').click();
  await page.getByLabel('Codex executable', { exact: true }).fill('/synthetic/reviewed-codex');
  await page.getByRole('button', { name: 'Inspect executable', exact: true }).click();
  await page.getByLabel('Allow Codex to sign in and read usage.').check();
  await page.getByRole('button', { name: 'Continue', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Ready to sign in' })).toBeVisible();
  await navigate(page, 'accounts');
  if (newAccount) {
    await page.locator('[data-account-connect]').click();
    await expect(page.locator(`[data-login-start="${profileId}"]`)).toBeVisible();
  }
}

async function begin(page: Page, code: string) {
  await page.locator(`[data-login-start="${profileId}"]`).click();
  await expect(page.getByLabel('Your sign-in code', { exact: true })).toHaveValue(code);
}

async function reject(page: Page, kind: 'completions' | 'cancellations', attemptId: string, error: string) {
  await expect.poll(() => page.evaluate(({ kind, attemptId }) =>
    Boolean((window as unknown as FixtureWindow).loginFixture[kind][attemptId]), { kind, attemptId })).toBe(true);
  await page.evaluate(({ kind, attemptId, error }) => {
    (window as unknown as FixtureWindow).loginFixture[kind][attemptId]!.reject(error);
  }, { kind, attemptId, error });
}

test('renderer fixture keeps the new sign-in intact after an old cancel rejects late', async ({ page }) => {
  await openSyntheticProfile(page);
  expect(await page.evaluate(() => (window as unknown as FixtureWindow).loginFixture.requests
    .filter(request => request.command === 'begin_profile_login'))).toEqual([]);
  await begin(page, 'SYNTH-OLD');
  await page.getByRole('button', { name: 'Cancel sign-in', exact: true }).click();
  await reject(page, 'completions', attempts[0]!, 'provider_login_cancelled');
  await expect(page.getByRole('heading', { name: 'Sign-in stopped', exact: true })).toBeVisible();

  await begin(page, 'SYNTH-NEW');
  await reject(page, 'cancellations', attempts[0]!, 'provider_cleanup_failed');
  await expect(page.getByLabel('Your sign-in code', { exact: true })).toHaveValue('SYNTH-NEW');
  await expect(page.getByRole('button', { name: 'Cancel sign-in', exact: true })).toBeEnabled();
  await expect(page.getByRole('heading', { name: 'Sign-in could not finish', exact: true })).toHaveCount(0);

  await expect.poll(() => page.evaluate(attemptId =>
    Boolean((window as unknown as FixtureWindow).loginFixture.completions[attemptId]), attempts[1]!)).toBe(true);
  await page.evaluate(({ attemptId, profileId, approvalId }) => {
    (window as unknown as FixtureWindow).loginFixture.completions[attemptId]!.resolve({ attemptId,
      diagnostic: { profileId, approvalId, providerVersion: '0.163.0', authentication: 'chatgpt',
        identityConfidence: 'provisional', requiresOpenaiAuth: true, checkedAt: '2026-10-18T04:00:00.123Z',
        monitoring: 'stopped', persisted: true, sourceKind: 'test_fixture' } });
  }, { attemptId: attempts[1]!, profileId, approvalId });
  await expect(connectionRow(page)).toHaveText('Connected');
  await expect(page.locator(`[data-collection-cancel="${profileId}"]`)).toBeVisible();
  await expect(page.getByLabel('Your sign-in code', { exact: true })).toHaveCount(0);
});

test('renderer fixture retains a cleanup failure when requested cancellation also completes', async ({ page }) => {
  await openSyntheticProfile(page);
  await begin(page, 'SYNTH-OLD');
  await page.getByRole('button', { name: 'Cancel sign-in', exact: true }).click();
  await reject(page, 'cancellations', attempts[0]!, 'provider_cleanup_failed');
  await expect(page.getByRole('heading', { name: 'Sign-in could not finish', exact: true })).toBeVisible();
  await reject(page, 'completions', attempts[0]!, 'provider_login_cancelled');
  await expect(page.getByRole('button', { name: 'Retry sign-in', exact: true })).toBeVisible();
  await expect(page.getByRole('heading', { name: 'Sign-in could not finish', exact: true })).toBeVisible();
  await expect(page.getByText('OpenUsage could not confirm that the provider process stopped. Restart OpenUsage before another check.')).toBeVisible();
  await expect(page.getByRole('heading', { name: 'Sign-in stopped', exact: true })).toHaveCount(0);
});

test('renderer fixture preserves the saved quotas after an old connection cancel rejects', async ({ page }) => {
  await openSyntheticProfile(page);
  await openOptions(page, profileId);
  await page.getByRole('button', { name: 'Check connection', exact: true }).click();
  await page.getByRole('button', { name: 'Cancel check', exact: true }).click();
  await expect.poll(() => page.evaluate(profileId =>
    Boolean((window as unknown as FixtureWindow).loginFixture.cancellations[`check:${profileId}`]), profileId)).toBe(true);

  await page.evaluate(({ profileId, approvalId }) => {
    (window as unknown as FixtureWindow).loginFixture.completions[`check:${profileId}`]!.resolve({
      profileId, approvalId, providerVersion: '0.163.0', authentication: 'chatgpt',
      identityConfidence: 'provisional', requiresOpenaiAuth: true, checkedAt: null,
      monitoring: 'stopped', persisted: false, sourceKind: 'test_fixture',
    });
  }, { profileId, approvalId });
  await expect(page.getByText('Account response received', { exact: true })).toBeVisible();
  await openOptions(page, profileId);
  await page.getByRole('button', { name: 'Read quotas', exact: true }).click();
  await expect.poll(() => page.evaluate(requestId =>
    Boolean((window as unknown as FixtureWindow).loginFixture.completions[`quota:${requestId}`]), attempts[0]!)).toBe(true);
  await page.evaluate(({ profileId, approvalId, requestId }) => {
    (window as unknown as FixtureWindow).loginFixture.completions[`quota:${requestId}`]!.resolve({
      profileId, approvalId, requestId, providerVersion: '0.163.0', sourceKind: 'test_fixture',
      observedAtUtc: '2026-10-18T00:00:00.000Z', identityConfidence: 'provisional',
      verifiedIdentityKey: null, availableResetCredits: null, resetCreditDetails: null, monitoring: 'stopped', accepted: true, bindingId: '66666666-6666-4666-8666-666666666666', collectionRunId: '77777777-7777-4777-8777-777777777777',
      limits: [{ limitId: 'codex', limitName: null, reachedType: null, monthlyCreditLimit: null, spendControlReached: null, secondary: null,
        primary: { usedPercent: 25, derivedRemainingPercent: 75,
          windowDurationMins: 300, resetsAtUnix: null, label: 'Five-hour' } }],
    });
  }, { profileId, approvalId, requestId: attempts[0]! });
  const preview = page.getByRole('region', { name: 'Saved quotas for Synthetic login profile', exact: true });
  await expect(preview).toBeVisible();
  await expect(preview).toBeFocused();
  await expect(preview.getByText('75%', { exact: true })).toBeVisible();

  await reject(page, 'cancellations', `check:${profileId}`, 'provider_quota_in_progress');
  await page.evaluate(() => new Promise<void>(resolve => requestAnimationFrame(() => requestAnimationFrame(() => resolve()))));
  await expect(preview).toBeVisible();
  await expect(preview).toBeFocused();
  await expect(preview.getByText('75%', { exact: true })).toBeVisible();
  await expect(page.locator(`[data-provider-profile="${profileId}"]`)).toHaveCount(0);
  await expect(page.getByRole('alert')).toHaveCount(0);
});


async function startQuota(page: Page) {
  await page.evaluate(requestId => { delete (window as unknown as FixtureWindow).loginFixture.completions[`quota:${requestId}`]; }, attempts[0]!);
  await openOptions(page, profileId);
  await page.getByRole('button', { name: 'Read quotas', exact: true }).click();
  await expect(page.locator(`[data-quota-cancel="${profileId}"]`)).toBeFocused();
  await expect.poll(() => page.evaluate(requestId => Boolean((window as unknown as FixtureWindow).loginFixture.completions[`quota:${requestId}`]), attempts[0]!)).toBe(true);
}
async function publishQuota(page: Page, remaining: number, observedAtUtc: string) {
  await page.evaluate(({ profileId, approvalId, requestId, remaining, observedAtUtc }) => {
    (window as unknown as FixtureWindow).loginFixture.completions[`quota:${requestId}`]!.resolve({
      profileId, approvalId, requestId, providerVersion: '0.163.0', sourceKind: 'test_fixture', observedAtUtc,
      identityConfidence: 'provisional', verifiedIdentityKey: null, availableResetCredits: null, resetCreditDetails: null, monitoring: 'stopped', accepted: true, bindingId: '66666666-6666-4666-8666-666666666666', collectionRunId: '77777777-7777-4777-8777-777777777777',
      limits: [{ limitId: 'codex', limitName: null, reachedType: null, monthlyCreditLimit: null, spendControlReached: null, secondary: null,
        primary: { usedPercent: 100 - remaining, derivedRemainingPercent: remaining, windowDurationMins: 300, resetsAtUnix: null, label: 'Five-hour' } }],
    });
  }, { profileId, approvalId, requestId: attempts[0]!, remaining, observedAtUtc });
}

test('renderer retains the previous saved quotas through pending, invalid response and cancellation, then replaces it on success', async ({ page }) => {
  await openSyntheticProfile(page);
  const root = page.locator(`[data-quota-saved="${profileId}"]`);
  const outcome = page.locator(`[data-quota-profile="${profileId}"]`);
  await startQuota(page); await publishQuota(page, 75, '2026-10-18T00:00:00.000Z');
  await expect(root).toBeFocused(); await expect(root.getByText('75%', { exact: true })).toBeVisible();
  await expect(root.locator('[data-quota-previous]')).toHaveCount(0);
  await startQuota(page);
  await expect(root.getByText('75%', { exact: true })).toBeVisible();
  await expect(root.getByText('Unverified identity · Saved locally', { exact: true })).toBeVisible();
  await page.evaluate(requestId => { (window as unknown as FixtureWindow).loginFixture.completions[`quota:${requestId}`]!.resolve({ accepted: false }); }, attempts[0]!);
  await expect(outcome).toBeFocused(); await expect(outcome).toHaveAttribute('role', 'alert');
  await expect(outcome.getByText('OpenUsage could not complete this request. Review the executable and try again.', { exact: true })).toBeVisible();
  await expect(root.getByText('75%', { exact: true })).toBeVisible();
  await expect(root.locator('.quota-observed time')).toHaveAttribute('datetime', '2026-10-18T00:00:00.000Z');
  await startQuota(page);
  await expect(outcome.getByText('OpenUsage could not complete this request. Review the executable and try again.', { exact: true })).toBeVisible();
  await expect(page.locator(`[data-quota-cancel="${profileId}"]`)).toBeFocused();
  await page.locator(`[data-quota-cancel="${profileId}"]`).click();
  await expect.poll(() => page.evaluate(requestId => Boolean((window as unknown as FixtureWindow).loginFixture.cancellations[`quota:${requestId}`]), attempts[0]!)).toBe(true);
  await page.evaluate(requestId => { const fixture = (window as unknown as FixtureWindow).loginFixture; fixture.cancellations[`quota:${requestId}`]!.resolve({ cancelled: true }); fixture.completions[`quota:${requestId}`]!.reject('provider_quota_cancelled'); }, attempts[0]!);
  await expect(outcome).toBeFocused(); await expect(outcome).toHaveAttribute('role', 'status');
  await expect(outcome.getByText('Quota read stopped.', { exact: true })).toBeVisible();
  await expect(root.getByText('75%', { exact: true })).toBeVisible();
  await startQuota(page); await publishQuota(page, 60, '2026-10-18T00:01:00.000Z');
  await expect(root).toBeFocused(); await expect(root).toHaveAttribute('role', 'region');
  await expect(root.getByText('60%', { exact: true })).toBeVisible(); await expect(root.getByText('75%', { exact: true })).toHaveCount(0);
  await expect(root.locator('.quota-observed time')).toHaveAttribute('datetime', '2026-10-18T00:01:00.000Z');
  await expect(root.locator('[data-quota-previous]')).toHaveCount(0); await expect(outcome.getByText('Quota read stopped.', { exact: true })).toHaveCount(0);
});

test('renderer retains saved quotas when its provider operation component is replaced', async ({ page }) => {
  await openSyntheticProfile(page); await startQuota(page); await publishQuota(page, 75, '2026-10-18T00:00:00.000Z');
  await expect(page.locator(`[data-quota-saved="${profileId}"]`).getByText('75%', { exact: true })).toBeVisible();
  await openOptions(page, profileId);
  await page.getByRole('button', { name: 'Check connection', exact: true }).click();
  await expect(page.locator(`[data-quota-saved="${profileId}"]`).getByText('75%', { exact: true })).toBeVisible();
  await expect(page.locator(`[data-quota-profile="${profileId}"]`)).toHaveCount(0);
});


async function finishLogin(page: Page, index = 0, identity: 'provisional' | 'unknown' = 'provisional') {
  const attemptId = attempts[index]!;
  await expect.poll(() => page.evaluate(attemptId => Boolean((window as unknown as FixtureWindow).loginFixture.completions[attemptId]), attemptId)).toBe(true);
  await page.evaluate(({ attemptId, profileId, approvalId, identity }) => {
    (window as unknown as FixtureWindow).loginFixture.completions[attemptId]!.resolve({ attemptId,
      diagnostic: { profileId, approvalId, providerVersion: '0.163.0', authentication: 'chatgpt',
        identityConfidence: identity, requiresOpenaiAuth: true, checkedAt: '2026-10-18T04:00:00.123Z',
        monitoring: 'stopped', persisted: true, sourceKind: 'test_fixture' } });
  }, { attemptId, profileId, approvalId, identity });
}
const connectionRow = (page: Page) => page.locator(`[data-profile-id="${profileId}"] .profile-connection-state`);
const profileLoadCount = (page: Page) => page.evaluate(() => (window as unknown as FixtureWindow).loginFixture.requests.filter(request => request.command === 'list_profiles').length);

test('an existing account refreshes its Connected row and automatically reads after matched sign-in', async ({ page }) => {
  await openSyntheticProfile(page);
  const loads = await profileLoadCount(page);
  await begin(page, 'SYNTH-OLD');
  await expect(connectionRow(page)).toHaveText('Not connected');
  await finishLogin(page);
  await expect(connectionRow(page)).toHaveText('Connected');
  await expect(page.locator(`[data-collection-cancel="${profileId}"]`)).toBeFocused();
  expect(await profileLoadCount(page)).toBe(loads + 1);
  expect(await collectionStarts(page)).toBe(1);
  expect(await page.evaluate(() => (window as unknown as FixtureWindow).loginFixture.requests.filter(request => request.command === 'cancel_profile_login'))).toEqual([]);
  await navigate(page, 'overview');
  await expect(page.locator(`[data-overview-account="${profileId}"]`)).toContainText('Connected');
});

test('renderer shows durable Error for a completed login with unknown identity', async ({ page }) => {
  await openSyntheticProfile(page); await begin(page, 'SYNTH-OLD'); await finishLogin(page, 0, 'unknown');
  await expect(connectionRow(page)).toHaveText('Connection error');
  await expect(page.getByRole('heading', { name: 'Connection needs attention', exact: true })).toBeVisible();
  await expect(page.getByText('OpenAI completed sign-in, but account details are unavailable. Try reconnecting. Your saved readings are still available.', { exact: true })).toBeVisible();
  await expect(page.getByRole('heading', { name: 'Sign-in completed', exact: true })).toHaveCount(0);
});

test('renderer does not refresh Connected after a completion received during requested cancellation', async ({ page }) => {
  await openSyntheticProfile(page); const loads = await profileLoadCount(page); await begin(page, 'SYNTH-OLD');
  await page.getByRole('button', { name: 'Cancel sign-in', exact: true }).click();
  await finishLogin(page);
  await expect(page.getByRole('heading', { name: 'Sign-in stopped', exact: true })).toBeVisible();
  await expect(connectionRow(page)).toHaveText('Not connected');
  expect(await profileLoadCount(page)).toBe(loads);
});

test('renderer preserves saved sign-in success and retries only the connection status after a reload failure', async ({ page }) => {
  await openSyntheticProfile(page); await begin(page, 'SYNTH-OLD');
  await page.evaluate(() => { (window as unknown as FixtureWindow).loginFixture.failProfiles = true; });
  await finishLogin(page);
  await expect(page.getByRole('heading', { name: 'Sign-in completed', exact: true })).toBeVisible();
  await expect(page.locator('[data-login-connection-refresh-error]')).toHaveText('The sign-in result was saved, but the connection status could not be refreshed. Retry the status refresh to check it.');
  await expect(connectionRow(page)).toHaveText('Not connected');
  await expect(page.getByRole('heading', { name: 'Sign-in could not finish', exact: true })).toHaveCount(0);
  await page.getByRole('button', { name: 'Retry status refresh', exact: true }).click();
  await expect(page.locator('[data-login-connection-refresh-error]')).toBeVisible();
  await expect(page.getByRole('heading', { name: 'Sign-in completed', exact: true })).toBeVisible();
  await page.evaluate(() => { (window as unknown as FixtureWindow).loginFixture.failProfiles = false; });
  await page.getByRole('button', { name: 'Retry status refresh', exact: true }).click();
  await expect(connectionRow(page)).toHaveText('Connected');
  await expect(page.locator('[data-login-connection-refresh-error]')).toHaveCount(0);
  expect(await page.evaluate(() => (window as unknown as FixtureWindow).loginFixture.requests.filter(request => request.command === 'begin_profile_login').length)).toBe(1);
});

test('renderer ignores an old connection-refresh failure while the next sign-in is active', async ({ page }) => {
  await openSyntheticProfile(page); await begin(page, 'SYNTH-OLD');
  await page.evaluate(() => { (window as unknown as FixtureWindow).loginFixture.deferProfiles = true; });
  await finishLogin(page);
  await expect(page.getByRole('heading', { name: 'Sign-in completed', exact: true })).toBeVisible();
  await expect.poll(() => page.evaluate(() => (window as unknown as FixtureWindow).loginFixture.profileLoads.length)).toBe(1);
  await begin(page, 'SYNTH-NEW');
  await page.evaluate(() => { (window as unknown as FixtureWindow).loginFixture.profileLoads[0]!.reject('profile_unavailable'); });
  await expect(page.getByLabel('Your sign-in code', { exact: true })).toHaveValue('SYNTH-NEW');
  await expect(page.locator('[data-login-connection-refresh-error]')).toHaveCount(0);
});


const collectionStarts = (page: Page) => page.evaluate(() => (window as unknown as FixtureWindow).loginFixture.requests.filter(request => request.command === 'begin_profile_collection_read').length);

test('new account reads once after matched login and status reload, then uses the existing saved quota view', async ({ page }) => {
  await openSyntheticProfile(page, true);
  await begin(page, 'SYNTH-OLD');
  await page.evaluate(() => { (window as unknown as FixtureWindow).loginFixture.deferProfiles = true; });
  await finishLogin(page);
  await expect.poll(() => page.evaluate(() => (window as unknown as FixtureWindow).loginFixture.profileLoads.length)).toBe(1);
  expect(await collectionStarts(page)).toBe(0);
  await page.evaluate(profileId => {
    const f = (window as unknown as FixtureWindow).loginFixture;
    f.deferProfiles = false;
    f.profileLoads[0]!.resolve({ activeProfiles: 1, profiles: [{ id: profileId, label: 'Synthetic login profile', environment: 'native', archived: false, connectionState: 'connected' }] });
  }, profileId);
  await expect(page.locator(`[data-collection-cancel="${profileId}"]`)).toBeVisible();
  await expect(page.getByText('Refreshing quota and activity readings…', { exact: true })).toBeVisible();
  expect(await collectionStarts(page)).toBe(1);
  await page.evaluate(({ profileId, approvalId, requestId }) => {
    const f = (window as unknown as FixtureWindow).loginFixture;
    const bindingId = '66666666-6666-4666-8666-666666666666';
    const collectionRunId = '77777777-7777-4777-8777-777777777777';
    const observedAtUtc = '2026-10-18T00:00:00.000Z';
    f.savedQuota = { profileId, bindingId, collectionRunId, providerVersion: '0.163.0', sourceKind: 'test_fixture', observedAtUtc, identityConfidence: 'provisional', verifiedIdentityKey: null, availableResetCredits: null, resetCreditDetails: null, monitoring: 'stopped', accepted: true, freshness: 'fresh', coverage: 'unknown', scope: 'account_quota_with_unknown_application_scope', limits: [{ limitId: 'codex', limitName: null, reachedType: null, monthlyCreditLimit: null, spendControlReached: null, secondary: null, primary: { usedPercent: 25, derivedRemainingPercent: 75, windowDurationMins: 300, resetsAtUnix: null, label: 'Five-hour', resetState: 'unconfirmed' } }] };
    f.completions[`collection:${requestId}`]!.resolve({ profileId, approvalId, requestId, providerVersion: '0.163.0', sourceKind: 'test_fixture', identityConfidence: 'provisional', verifiedIdentityKey: null, monitoring: 'stopped', accepted: true, quota: { bindingId, collectionRunId, observedAtUtc }, activity: null });
  }, { profileId, approvalId, requestId: attempts[0]! });
  await expect(page.locator(`[data-account-quota="${profileId}"] [role=meter]`)).toHaveAttribute('aria-valuenow', '75');
  await openReadings(page, profileId);
  await expect(page.locator(`[data-quota-saved="${profileId}"]`).getByText('75%', { exact: true })).toBeVisible();
  await expect(page.locator(`[data-collection-profile="${profileId}"]`)).toContainText('Quotas saved.');
  await navigate(page, 'overview'); await navigate(page, 'accounts');
  await expect(page.locator(`[data-login-start="${profileId}"]`)).toHaveCount(0);
  expect(await collectionStarts(page)).toBe(1);
});

for (const outcome of ['cancelled', 'failed', 'unknown'] as const) {
  test(`new account does not read after ${outcome} login`, async ({ page }) => {
    await openSyntheticProfile(page, true); await begin(page, 'SYNTH-OLD');
    if (outcome === 'cancelled') {
      await page.locator(`[data-login-cancel="${profileId}"]`).click();
      await finishLogin(page);
      await expect(page.getByRole('heading', { name: 'Sign-in stopped', exact: true })).toBeVisible();
    } else if (outcome === 'failed') {
      await reject(page, 'completions', attempts[0]!, 'provider_login_failed');
      await expect(page.getByRole('heading', { name: 'Sign-in could not finish', exact: true })).toBeVisible();
    } else {
      await finishLogin(page, 0, 'unknown');
      await expect(connectionRow(page)).toHaveText('Connection error');
    }
    expect(await collectionStarts(page)).toBe(0);
  });
}

test('a failed initial read exposes manual retry without automatically repeating', async ({ page }) => {
  await openSyntheticProfile(page, true); await begin(page, 'SYNTH-OLD'); await finishLogin(page);
  await expect(page.locator(`[data-collection-cancel="${profileId}"]`)).toBeVisible();
  await reject(page, 'completions', `collection:${attempts[0]!}`, 'provider_protocol_failed');
  await expect(page.locator(`[data-collection-profile="${profileId}"]`)).toHaveAttribute('role', 'alert');
  await expect(page.locator(`[data-collection-read="${profileId}"]`)).toBeEnabled();
  expect(await collectionStarts(page)).toBe(1);
  await page.locator(`[data-collection-read="${profileId}"]`).click();
  await expect(page.locator(`[data-collection-cancel="${profileId}"]`)).toBeVisible();
  expect(await collectionStarts(page)).toBe(2);
});


test('new account waits for successful status retry before its first reading', async ({ page }) => {
  await openSyntheticProfile(page, true); await begin(page, 'SYNTH-OLD');
  await page.evaluate(() => { (window as unknown as FixtureWindow).loginFixture.failProfiles = true; });
  await finishLogin(page);
  await expect(page.locator('[data-login-connection-refresh-error]')).toBeVisible();
  expect(await collectionStarts(page)).toBe(0);
  await page.evaluate(() => { (window as unknown as FixtureWindow).loginFixture.failProfiles = false; });
  await page.getByRole('button', { name: 'Retry status refresh', exact: true }).click();
  await expect(page.locator(`[data-collection-cancel="${profileId}"]`)).toBeVisible();
  expect(await collectionStarts(page)).toBe(1);
});

test('a superseded login status response cannot start a new account reading', async ({ page }) => {
  await openSyntheticProfile(page, true); await begin(page, 'SYNTH-OLD');
  await page.evaluate(() => { (window as unknown as FixtureWindow).loginFixture.deferProfiles = true; });
  await finishLogin(page);
  await expect.poll(() => page.evaluate(() => (window as unknown as FixtureWindow).loginFixture.profileLoads.length)).toBe(1);
  await begin(page, 'SYNTH-NEW');
  await page.evaluate(profileId => (window as unknown as FixtureWindow).loginFixture.profileLoads[0]!.resolve({ activeProfiles: 1, profiles: [{ id: profileId, label: 'Synthetic login profile', environment: 'native', archived: false, connectionState: 'connected' }] }), profileId);
  await expect(page.getByLabel('Your sign-in code', { exact: true })).toHaveValue('SYNTH-NEW');
  expect(await collectionStarts(page)).toBe(0);
});
