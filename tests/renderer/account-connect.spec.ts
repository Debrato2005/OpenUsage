import { expect, test } from '@playwright/test';
import { navigate } from './navigation';

// Renderer-only fixtures. Real authentication and native isolation require separate evidence.
for (const verified of [true, false]) test(`Add OpenAI account discovers inline and ${verified ? 'requires consent before sign-in' : 'refuses an unverified installation'}`, async ({ page }) => {
  await page.addInitScript(({ verified }) => {
    const id = '11111111-1111-4111-8111-111111111111';
    const requests: { command: string; args: Record<string, unknown> }[] = [];
    Object.assign(window, { connectRequests: requests });
    let created = false;
    const profiles = () => ({ activeProfiles: created ? 1 : 0, profiles: created ? [{ id, label: 'Account A', environment: 'native', archived: false, connectionState: 'authentication_required' }] : [] });
    const metadata = { path: '/synthetic/reviewed-codex', fileBytes: '100', sha256: 'a'.repeat(64), testedVersion: '0.163.0', versionChecked: false, sourceVerified: verified, sourceKind: verified ? 'test_fixture' : 'unverified', sourceUrl: null };
    window.__TAURI__ = { core: { invoke: async <T>(command: string, args: Record<string, unknown> = {}) => {
      requests.push({ command, args });
      if (command === 'bootstrap') return { storage: 'ready', theme: 'light', schemaVersion: 4, activeProfiles: 0, monitoring: 'stopped', provider: 'not_configured', keyProtection: 'os_store', storageIssue: null } as T;
      if (command === 'load_monitoring') return { state: 'stopped', readingRevision: 'cccccccc-cccc-4ccc-8ccc-cccccccccccc', profiles: [] } as T;
      if (command === 'list_profiles') return profiles() as T;
      if (command === 'create_profile') { if (args.label !== 'Account A') throw new Error('Unexpected default account name.'); created = true; return profiles() as T; }
      if (command === 'load_profile_activity' || command === 'load_profile_quota') return null as T;
      if (command === 'load_profile_refresh_status') return { profileId: id, retryAfterSeconds: 0 } as T;
      if (command === 'discover_provider_executables') return { candidates: [{ path: metadata.path, location: 'path' }], truncated: false } as T;
      if (command === 'inspect_provider_executable') return { ...metadata, candidateId: '22222222-2222-4222-8222-222222222222' } as T;
      if (command === 'approve_provider_executable') return { ...metadata, approvalId: '33333333-3333-4333-8333-333333333333' } as T;
      throw new Error(`Unexpected fixture command: ${command}`);
    } } };
  }, { verified });
  await page.goto('/');
  await navigate(page, 'accounts');
  await page.getByRole('button', { name: 'Add OpenAI account', exact: true }).click();
  await expect(page.locator('[data-profile-id] .profile-label')).toHaveText('Account A');
  await expect(page.locator('#profile-label')).toBeHidden();
  await expect(page.getByRole('heading', { name: 'Codex found', exact: true })).toBeVisible();
  await expect(page.locator('[data-workspace-nav="accounts"]')).toHaveAttribute('aria-current', 'page');
  await expect(page.locator('.provider-executable-review .provider-checksum')).toBeHidden();
  const consent = page.getByRole('checkbox', { name: 'Allow Codex to sign in and read usage.', exact: true });
  const proceed = page.getByRole('button', { name: 'Continue', exact: true });
  await expect(proceed).toBeDisabled();
  if (verified) {
    await consent.check(); await proceed.click();
    const signIn = page.getByRole('button', { name: 'Sign in', exact: true });
    await expect(page.locator('.provider-panel')).toBeHidden();
    await expect(signIn).toBeFocused();
    await expect(signIn).toBeVisible(); await signIn.click();
    await expect(page.getByRole('heading', { name: 'Sign in to Account A', exact: true })).toBeVisible();
    await expect(page.getByRole('button', { name: 'Get sign-in code', exact: true })).toBeEnabled();
  } else {
    await expect(consent).toBeDisabled();
    await expect(page.getByText('This Codex installation could not be verified. Choose a supported installation under Advanced.', { exact: true })).toBeVisible();
  }
  const requests = await page.evaluate(() => (window as unknown as { connectRequests: { command: string }[] }).connectRequests);
  expect(requests.filter(value => value.command === 'discover_provider_executables')).toHaveLength(1);
  expect(requests.filter(value => value.command === 'create_profile')).toHaveLength(1);
  expect(requests.some(value => value.command === 'begin_profile_login')).toBe(false);
  expect(requests.filter(value => value.command === 'approve_provider_executable')).toHaveLength(verified ? 1 : 0);
});
