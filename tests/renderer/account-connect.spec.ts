import { expect, test } from '@playwright/test';
import { navigate } from './navigation';

// Renderer-only fixtures. Real authentication and native isolation require separate evidence.
for (const verified of [true, false]) test(`Add OpenAI account prepares the companion and ${verified ? 'opens sign-in without technical setup' : 'refuses an unverified installation'}`, async ({ page }) => {
  await page.addInitScript(({ verified }) => {
    const id = '11111111-1111-4111-8111-111111111111';
    const requests: { command: string; args: Record<string, unknown> }[] = [];
    Object.assign(window, { connectRequests: requests });
    let created = false;
    const profiles = () => ({ activeProfiles: created ? 1 : 0, profiles: created ? [{ id, label: 'Account A', environment: 'native', archived: false, connectionState: 'authentication_required' }] : [] });
    const metadata = { path: '/synthetic/reviewed-codex', fileBytes: '100', sha256: 'a'.repeat(64), testedVersion: '0.163.0', versionChecked: false, sourceVerified: true, sourceKind: 'official_artifact', sourceUrl: 'https://github.com/openai/codex/releases/tag/rust-v0.163.0' };
    window.__TAURI__ = { core: { invoke: async <T>(command: string, args: Record<string, unknown> = {}) => {
      requests.push({ command, args });
      if (command === 'bootstrap') return { storage: 'ready', theme: 'light', schemaVersion: 4, activeProfiles: 0, monitoring: 'stopped', provider: 'not_configured', keyProtection: 'os_store', storageIssue: null } as T;
      if (command === 'load_monitoring') return { state: 'stopped', readingRevision: 'cccccccc-cccc-4ccc-8ccc-cccccccccccc', profiles: [] } as T;
      if (command === 'list_profiles') return profiles() as T;
      if (command === 'create_profile') { if (args.label !== 'Account A') throw new Error('Unexpected default account name.'); created = true; return profiles() as T; }
      if (command === 'load_profile_activity' || command === 'load_profile_quota' || command === 'load_profile_heatmap') return null as T;
      if (command === 'load_profile_refresh_status') return { profileId: id, retryAfterSeconds: 0 } as T;
      if (command === 'prepare_provider_connection') { if (!verified) throw 'provider_source_unverified'; return { ...metadata, approvalId: '33333333-3333-4333-8333-333333333333' } as T; }
      if (command === 'discover_provider_executables') return { candidates: [{ path: metadata.path, location: 'path' }], truncated: false } as T;
      if (command === 'inspect_provider_executable') return { ...metadata, candidateId: '22222222-2222-4222-8222-222222222222' } as T;
      if (command === 'approve_provider_executable') return { ...metadata, approvalId: '33333333-3333-4333-8333-333333333333' } as T;
      throw new Error(`Unexpected fixture command: ${command}`);
    } } };
  }, { verified });
  await page.goto('/');
  await navigate(page, 'overview');
  await page.locator('[data-overview-add-account]').click();
  await expect(page.locator('[data-profile-id] .profile-label')).toHaveText('Account A');
  await expect(page.locator('#profile-label')).toBeHidden();
  await expect(page.locator('[data-workspace-nav="accounts"]')).toHaveCount(0);
  await expect(page.locator('[data-account-panel]')).toBeVisible();
  await expect(page.locator('.provider-panel')).toBeHidden();
  await expect(page.locator('#provider-executable-approved')).toHaveCount(0);
  if (verified) {
    const signIn = page.locator('[data-account-panel]').getByRole('button', { name: 'Sign in', exact: true });
    await expect(signIn).toBeFocused();
    await expect(signIn).toBeVisible();
    await expect(page.locator('[data-profile-action=readings]')).toBeHidden();
  } else {
    await expect(page.locator('[data-account-panel]').getByRole('alert')).toContainText('OpenUsage could not verify its OpenAI companion.');
    await expect(page.getByRole('button', { name: 'Retry connection', exact: true })).toBeEnabled();
    await expect(page.locator('[data-login-start]')).toHaveCount(0);
    expect(await page.evaluate(() => (window as unknown as { connectRequests: { command: string }[] }).connectRequests.some(row => row.command === 'begin_profile_login'))).toBe(false);
  }
  const requests = await page.evaluate(() => (window as unknown as { connectRequests: { command: string }[] }).connectRequests);
  expect(requests.filter(value => value.command === 'prepare_provider_connection')).toHaveLength(1);
  expect(requests.filter(value => value.command === 'discover_provider_executables')).toHaveLength(0);
  expect(requests.filter(value => value.command === 'create_profile')).toHaveLength(1);
  expect(requests.some(value => value.command === 'begin_profile_login')).toBe(false);
  expect(requests.filter(value => value.command === 'approve_provider_executable')).toHaveLength(0);
});
