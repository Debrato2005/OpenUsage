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
  const summary = page.getByRole('region', { name: 'Workspace summary' });
  await expect(summary.getByText('Not available', { exact: true })).toHaveCount(2);
  await page.getByRole('button', { name: 'Check desktop connection' }).click();
  await expect(page.getByRole('status')).toHaveText('OpenUsage needs its desktop application');
});

test('locked storage never invents a profile count or provider reading', async ({ page }) => {
  await bootstrap(page, locked);
  await page.goto('/');
  await expect(page.getByRole('status')).toHaveText('Unlock your local workspace');
  await expect(page.getByText(/Monitoring stopped/)).toBeVisible();
  const summary = page.getByRole('region', { name: 'Workspace summary' });
  await expect(summary.getByText('Locked', { exact: true })).toBeVisible();
  await expect(summary.getByText('Not available', { exact: true })).toHaveCount(2);
});

test('malformed bootstrap data shows a recoverable connection error', async ({ page }) => {
  await bootstrap(page, { ...locked, storage: ['ready'] });
  await page.goto('/');
  await expect(page.getByRole('status')).toHaveText('OpenUsage could not read its local state');
  await expect(page.getByRole('button', { name: 'Check desktop connection' })).toBeVisible();
  await expect(page.getByText('Your workspace is ready', { exact: true })).toHaveCount(0);
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
    await page.getByRole('combobox', { name: 'Appearance' }).selectOption(theme);
    await expect(page.locator('html')).toHaveAttribute('data-theme', theme);
    const result = await new AxeBuilder({ page }).withTags(['wcag2a', 'wcag2aa', 'wcag21aa']).analyze();
    expect(result.violations).toEqual([]);
  }
});
