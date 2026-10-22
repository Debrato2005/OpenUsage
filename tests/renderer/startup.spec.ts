import { expect, test, type Page } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';

// These fixtures exercise the renderer, not native IPC, SQLCipher, or provider access.
async function bootstrap(page: Page, value: unknown) {
  await page.addInitScript(response => {
    window.__TAURI__ = { core: { invoke: async <T>() => response as T } };
  }, value);
}

const locked = {
  storage: 'locked', theme: 'system', schemaVersion: null, activeProfiles: null,
  monitoring: 'stopped', provider: 'not_configured',
  keyProtection: 'os_store', storageIssue: null,
};

test('browser mode explains that native access is unavailable', async ({ page }) => {
  await page.goto('/');
  await expect(page.getByRole('status')).toHaveText('OpenUsage needs its desktop application');
  await expect(page.getByRole('navigation', { name: 'Main', exact: true })).toBeHidden();
  await expect(page.getByRole('region', { name: 'Workspace summary' })).toHaveCount(0);
  await page.getByRole('button', { name: 'Check desktop connection' }).click();
  await expect(page.getByRole('status')).toHaveText('OpenUsage needs its desktop application');
});

test('locked storage never invents a profile count or provider reading', async ({ page }) => {
  await bootstrap(page, locked);
  await page.goto('/');
  await expect(page.getByRole('status')).toHaveText('Welcome back');
  await expect(page.getByRole('navigation', { name: 'Main', exact: true })).toBeHidden();
  await expect(page.getByRole('region', { name: 'Workspace summary' })).toHaveCount(0);
  await expect(page.locator('[data-global-monitoring]')).toHaveCount(0);
  await expect(page.getByRole('heading', { name: 'Overview', exact: true })).toHaveCount(0);
  await expect(page.getByRole('button', { name: 'Check workspace', exact: true })).toBeVisible();
  await expect(page.locator('#workspace-passphrase')).toHaveCount(0);
});

test('app password setup keeps consent local, rejects mismatched confirmation, and opens the real workspace after setup', async ({ page }) => {
  await page.addInitScript(() => {
    const calls: { command: string; args: Record<string, unknown> }[] = [];
    Object.assign(window, { startupCalls: calls });
    window.__TAURI__ = { core: { invoke: async <T>(command: string, args: Record<string, unknown> = {}) => {
      if (command === 'load_time_zone') return { timeZone: 'UTC' } as T;
      calls.push({ command, args });
      const state = { theme: 'light', monitoring: 'stopped', provider: 'not_configured', storageIssue: null };
      let value: unknown;
      if (command === 'bootstrap') value = { ...state, storage: 'setup_required', schemaVersion: null, activeProfiles: null, keyProtection: 'passphrase_required' };
      else if (command === 'setup_workspace') value = { ...state, storage: 'ready', schemaVersion: 4, activeProfiles: 0, keyProtection: 'passphrase' };
      else if (command === 'list_profiles') value = { profiles: [], activeProfiles: 0 };
      else if (command === 'load_monitoring') value = { state: 'stopped', readingRevision: '11111111-1111-4111-8111-111111111111', profiles: [] };
      else throw new Error(`Unexpected startup command ${command}`);
      return value as T;
    } } };
  });
  await page.goto('/');
  await expect(page.getByRole('status')).toHaveText('Welcome to OpenUsage');
  await expect(page.getByText('Your desktop key store is unavailable. An app password protects saved account data.', { exact: true })).toBeVisible();
  await expect(page.getByRole('navigation', { name: 'Main', exact: true })).toBeHidden();
  await page.getByLabel('App password', { exact: true }).fill('synthetic app password');
  await page.getByLabel('Confirm app password', { exact: true }).fill('different app password');
  await page.getByRole('button', { name: 'Create private workspace', exact: true }).click();
  await expect(page.getByRole('alert')).toHaveText('The passwords do not match.');
  await expect(page.locator('#workspace-confirmation')).toBeFocused();
  expect(await page.evaluate(() => (window as unknown as { startupCalls: { command: string }[] }).startupCalls.filter(call => call.command === 'setup_workspace'))).toEqual([]);
  await page.getByLabel('Confirm app password', { exact: true }).fill('synthetic app password');
  await page.getByRole('button', { name: 'Create private workspace', exact: true }).click();
  await expect(page.getByRole('navigation', { name: 'Main', exact: true })).toBeVisible();
  await expect(page.locator('.onboarding-shell')).toHaveCount(0);
  await expect(page.getByRole('heading', { name: 'Overview', exact: true })).toBeVisible();
  expect(await page.evaluate(() => (window as unknown as { startupCalls: { command: string; args: unknown }[] }).startupCalls.filter(call => call.command === 'setup_workspace'))).toEqual([{ command: 'setup_workspace', args: { passphrase: 'synthetic app password' } }]);
});

test('desktop key-store setup offers one primary action without an app password form', async ({ page }) => {
  await bootstrap(page, { ...locked, storage: 'setup_required', keyProtection: 'os_store' });
  await page.goto('/');
  await expect(page.getByRole('status')).toHaveText('Welcome to OpenUsage');
  await expect(page.getByRole('button', { name: 'Create private workspace', exact: true })).toBeVisible();
  await expect(page.locator('.unlock-form')).toHaveCount(0);
  await expect(page.locator('[data-global-monitoring]')).toHaveCount(0);
  await page.getByRole('button', { name: 'About your privacy', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Privacy and local storage', exact: true })).toBeVisible();
});

test('malformed bootstrap data shows a recoverable connection error', async ({ page }) => {
  await bootstrap(page, { ...locked, storage: ['ready'] });
  await page.goto('/');
  await expect(page.getByRole('status')).toHaveText('OpenUsage could not read its local state');
  await expect(page.getByRole('button', { name: 'Check desktop connection' })).toBeVisible();
  await expect(page.getByText('Workspace ready', { exact: true })).toHaveCount(0);
});

test('database recovery guidance does not offer an automatic reset', async ({ page }) => {
  await bootstrap(page, { ...locked, storage: 'recovery_required' });
  await page.goto('/');
  await expect(page.getByRole('status')).toHaveText('Your local data needs recovery');
  await expect(page.getByText(/Keep your database and key files/)).toBeVisible();
  await expect(page.getByRole('button', { name: /reset|delete|connect/i })).toHaveCount(0);
});

test('skip navigation works with a keyboard', async ({ page }) => {
  await page.goto('/');
  await page.keyboard.press('Tab');
  await expect(page.getByRole('link', { name: 'Skip to dashboard' })).toBeFocused();
  await page.keyboard.press('Enter');
  await expect(page.getByRole('main')).toBeFocused();
});

test('light and dark startup views have no automated accessibility violations', async ({ page }) => {
  await page.goto('/');
  await expect(page.getByRole('status')).toHaveText('OpenUsage needs its desktop application');
  for (const theme of ['light', 'dark']) {
    await page.getByRole('combobox', { name: 'Appearance' }).click();
    await page.getByRole('option', { name: theme === 'light' ? 'Light' : 'Dark', exact: true }).click();
    await expect(page.locator('html')).toHaveAttribute('data-theme', theme);
    const result = await new AxeBuilder({ page }).withTags(['wcag2a', 'wcag2aa', 'wcag21aa']).analyze();
    expect(result.violations).toEqual([]);
  }
});
