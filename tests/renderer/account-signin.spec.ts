import { expect, test } from '@playwright/test';
import { navigate } from './navigation';

// Renderer-only fixtures: this tests account routing, not OAuth or native persistence.
test('collapsed account rows expose independent sign-in and keep another account pending after cancellation', async ({ page }) => {
  const ids = ['11111111-1111-4111-8111-111111111111', '22222222-2222-4222-8222-222222222222'];
  await page.addInitScript(({ ids }) => {
    const approvalId = 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa';
    const attempts = ['33333333-3333-4333-8333-333333333333', '44444444-4444-4444-8444-444444444444'];
    const pending: Record<string, (error: string) => void> = {};
    const calls: { command: string; args: Record<string, unknown> }[] = [];
    Object.assign(window, { accountSignInCalls: calls });
    const profiles = ids.map((id, i) => ({ id, label: `Fixture account ${i + 1}`, environment: 'native', archived: false, connectionState: 'authentication_required' }));
    window.__TAURI__ = { core: { invoke: async <T>(command: string, args: Record<string, unknown> = {}) => {
      if (command === 'load_time_zone') return { timeZone: 'UTC' } as T;
      calls.push({ command, args });
      const index = ids.indexOf(String(args.profileId));
      let value: unknown;
      switch (command) {
        case 'bootstrap': value = { storage: 'ready', theme: 'light', schemaVersion: 4, activeProfiles: 2, monitoring: 'stopped', provider: 'not_configured', keyProtection: 'os_store', storageIssue: null }; break;
        case 'load_monitoring': value = { state: 'stopped', readingRevision: 'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb', profiles: [] }; break;
        case 'list_profiles': value = { activeProfiles: 2, profiles }; break;
        case 'load_profile_activity': case 'load_profile_quota': value = null; break;
        case 'load_profile_refresh_status': value = { profileId: args.profileId, retryAfterSeconds: 0 }; break;
        case 'prepare_provider_connection': value = { approvalId, path: '/renderer-fixture/codex', fileBytes: '1', sha256: 'a'.repeat(64), testedVersion: '0.163.0', versionChecked: false, sourceVerified: true, sourceKind: 'official_artifact', sourceUrl: 'https://github.com/openai/codex/releases/tag/rust-v0.163.0' }; break;
        case 'open_profile_login_page': value = null; break;
        case 'begin_profile_login':
          if (index < 0 || args.confirmed !== true || args.flow !== 'device_code') throw 'invalid_arguments';
          value = { profileId: args.profileId, approvalId, attemptId: attempts[index], sourceKind: 'official_artifact', monitoring: 'stopped', applicationWaitLimitSeconds: 900 }; break;
        case 'await_profile_login_prompt': value = { profileId: args.profileId, attemptId: args.attemptId, sourceKind: 'official_artifact', verificationUrl: 'https://auth.openai.com/codex/device', userCode: `FIXTURE-${index + 1}`, applicationWaitLimitSeconds: 900 }; break;
        case 'complete_profile_login': value = await new Promise((_resolve, reject) => { pending[String(args.attemptId)] = reject; }); break;
        case 'cancel_profile_login': pending[String(args.attemptId)]?.('provider_login_cancelled'); value = { cancelled: true }; break;
        default: throw new Error(`Unexpected fixture command: ${command}`);
      }
      return value as T;
    } } };
  }, { ids });
  await page.goto('/'); await navigate(page, 'accounts');
  for (const id of ids) {
    const row = page.locator(`[data-profile-id="${id}"]`);
    await expect(row.getByRole('button', { name: 'Sign in', exact: true })).toBeVisible();
    await expect(row.locator('[data-profile-action=readings]')).toHaveAttribute('aria-expanded', 'false');
  }
  const first = page.locator(`[data-profile-id="${ids[0]}"]`);
  const second = page.locator(`[data-profile-id="${ids[1]}"]`);
  await first.getByRole('button', { name: 'Sign in', exact: true }).click();
  await expect(first.getByLabel('Your sign-in code', { exact: true })).toHaveValue('FIXTURE-1');
  await expect(second.getByRole('button', { name: 'Sign in', exact: true })).toBeEnabled();
  await second.getByRole('button', { name: 'Sign in', exact: true }).click();
  await expect(second.getByLabel('Your sign-in code', { exact: true })).toHaveValue('FIXTURE-2');
  await first.getByRole('button', { name: 'Cancel sign-in', exact: true }).click();
  await expect(first.getByRole('heading', { name: 'Sign-in stopped', exact: true })).toBeVisible();
  await expect(second.getByLabel('Your sign-in code', { exact: true })).toHaveValue('FIXTURE-2');
  await expect(second.getByRole('button', { name: 'Cancel sign-in', exact: true })).toBeEnabled();
  const calls = await page.evaluate(() => (window as unknown as { accountSignInCalls: { command: string; args: Record<string, unknown> }[] }).accountSignInCalls);
  expect(calls.filter(row => row.command === 'begin_profile_login').map(row => row.args.profileId)).toEqual(ids);
  expect(calls.filter(row => row.command === 'open_profile_login_page').map(row => row.args.profileId)).toEqual(ids);
  expect(calls.filter(row => row.command === 'cancel_profile_login').map(row => row.args.profileId)).toEqual([ids[0]]);
  expect(calls.some(row => row.command === 'begin_profile_collection_read')).toBe(false);
});
