import { expect, test, type Page } from '@playwright/test';
import { navigate } from './navigation';
import type { ReadingExportReceipt } from '../../apps/desktop/src/contracts';

// Synthetic renderer IPC only. Rust/native tests own snapshot contents and file publication.
const first = '11111111-1111-4111-8111-111111111111';
const second = '22222222-2222-4222-8222-222222222222';
const archived = '33333333-3333-4333-8333-333333333333';
type ExportFixture = {
  calls: Record<string, unknown>[];
  resolve?: (value: unknown) => void;
  reject?: (error: string) => void;
};
type FixtureWindow = Window & { exportFixture: ExportFixture };
const receipt: ReadingExportReceipt = {
  fileName: 'openusage-readings-0123456789abcdef.csv', directory: '/synthetic/Downloads/OpenUsage',
  format: 'csv', profileCount: 2, rowCount: 0, identifiersIncluded: false, exportedAt: '2026-10-20T00:00:00.000Z',
};
async function open(page: Page, empty = false) {
  await page.addInitScript(({ ids, empty }) => {
    const fixture: ExportFixture = { calls: [] }; Object.assign(window, { exportFixture: fixture });
    const profiles = empty ? [] : ids.map((id, index) => ({ id, label: ['Synthetic first', 'Synthetic second', 'Synthetic archived'][index], environment: 'native', connectionState: 'authentication_required', archived: index === 2 }));
    window.__TAURI__ = { core: { invoke: async <T>(command: string, args: Record<string, unknown> = {}) => {
      if (command === 'load_time_zone') return { timeZone: 'UTC' } as T;
      let value: unknown;
      switch (command) {
        case 'bootstrap': value = { storage: 'ready', theme: 'light', schemaVersion: 4, activeProfiles: empty ? 0 : 2, monitoring: 'stopped', provider: 'not_configured', keyProtection: 'passphrase', storageIssue: null }; break;
        case 'list_profiles': value = { profiles, activeProfiles: empty ? 0 : 2 }; break;
        case 'load_monitoring': value = { state: 'stopped', readingRevision: ids[0], profiles: [] }; break;
        case 'load_profile_refresh_status': value = { profileId: args.profileId, retryAfterSeconds: 0 }; break;
        case 'load_profile_quota': case 'load_profile_activity': value = null; break;
        case 'export_saved_readings':
          fixture.calls.push(structuredClone(args));
          value = await new Promise((resolve, reject) => { fixture.resolve = resolve; fixture.reject = reject; }); break;
        default: throw new Error(`Unexpected export renderer command: ${command}`);
      }
      return value as T;
    } } };
  }, { ids: [first, second, archived], empty });
  await page.goto('/'); await navigate(page, 'settings');
  const toggle = page.locator('[data-export-toggle]');
  await toggle.focus(); await page.keyboard.press('Enter');
  await expect(page.getByRole('form', { name: 'Saved readings export' })).toBeVisible();
  if (!empty) await expect(page.locator('[data-export-profile]')).toHaveCount(2);
}
const save = (page: Page) => page.locator('[data-export-save]');
const consent = (page: Page) => page.getByRole('checkbox', { name: 'I understand this file contains unencrypted activity.', exact: true });
async function publish(page: Page, value: unknown = receipt) {
  await expect.poll(() => page.evaluate(() => !!(window as unknown as FixtureWindow).exportFixture.resolve)).toBe(true);
  await page.evaluate(value => (window as unknown as FixtureWindow).exportFixture.resolve!(value), value);
}

test('export trigger wraps at 200% text in a narrow window and remains keyboard operable', async ({ page }) => {
  await open(page);
  const toggle = page.locator('[data-export-toggle]');
  await toggle.click();
  await page.setViewportSize({ width: 360, height: 800 });
  await page.evaluate(() => {
    const sizes = [...document.body.querySelectorAll<HTMLElement>('*')].map(element => [element, parseFloat(getComputedStyle(element).fontSize)] as const);
    for (const [element, size] of sizes) element.style.fontSize = `${size * 2}px`;
  });
  expect(await page.evaluate(() => document.scrollingElement!.scrollWidth)).toBeLessThanOrEqual(361);
  expect(await toggle.evaluate(element => element.getBoundingClientRect().right)).toBeLessThanOrEqual(361);
  await toggle.focus(); await page.keyboard.press('Enter');
  await expect(page.getByRole('form', { name: 'Saved readings export' })).toBeVisible();
});

test('keyboard consent gates one CSV save, defaults omit identifiers, and focused receipt reports zero rows honestly', async ({ page }) => {
  await open(page);
  await expect(page.locator('[data-export-profile]')).toHaveCount(2);
  for (const id of [first, second]) await expect(page.locator(`[data-export-profile="${id}"]`)).toBeChecked();
  await expect(page.getByRole('checkbox', { name: 'Synthetic archived', exact: true })).toHaveCount(0);
  await expect(page.getByRole('checkbox', { name: 'Include profile names and email addresses', exact: true })).not.toBeChecked();
  await expect(consent(page)).not.toBeChecked(); await expect(save(page)).toBeDisabled();
  await expect(page.getByText('Exports are not encrypted. Activity may identify an account even when names and email are omitted.', { exact: true })).toBeVisible();
  await consent(page).focus(); await page.keyboard.press('Space');
  await page.keyboard.press('Tab'); await expect(save(page)).toBeFocused(); await page.keyboard.press('Enter');
  await expect(save(page)).toBeDisabled(); await expect(page.locator('#reading-export-format')).toBeDisabled();
  await expect(page.locator(`[data-export-profile="${first}"]`)).toBeDisabled();
  await expect(page.locator('[data-export-toggle]')).toBeDisabled();
  await publish(page);
  await expect(page.locator('[data-export-result]')).toBeFocused();
  await expect(page.locator('[data-export-result]')).toContainText('CSV · 2 profiles · 0 rows');
  await expect(consent(page)).not.toBeChecked(); await expect(save(page)).toBeDisabled();
  expect(await page.evaluate(() => (window as unknown as FixtureWindow).exportFixture.calls)).toEqual([{ profileIds: [first, second], format: 'csv', includeIdentifiers: false, confirmed: true }]);
});

test('profile and JSON identifier choices require fresh consent and match the saved receipt', async ({ page }) => {
  await open(page); await consent(page).check();
  await page.locator(`[data-export-profile="${second}"]`).uncheck();
  await expect(consent(page)).not.toBeChecked();
  await page.getByRole('combobox', { name: 'File format', exact: true }).click();
  await page.getByRole('option', { name: 'JSON', exact: true }).click();
  await page.getByRole('checkbox', { name: 'Include profile names and email addresses', exact: true }).check();
  await expect(save(page)).toBeDisabled(); await consent(page).check(); await save(page).click();
  await publish(page, { ...receipt, fileName: 'openusage-readings-0123456789abcdef.json', format: 'json', profileCount: 1, identifiersIncluded: true, rowCount: 12 });
  await expect(page.locator('[data-export-result]')).toBeFocused();
  await expect(page.locator('[data-export-result]')).toContainText('Profile names and email addresses included.');
  expect(await page.evaluate(() => (window as unknown as FixtureWindow).exportFixture.calls)).toEqual([{ profileIds: [first], format: 'json', includeIdentifiers: true, confirmed: true }]);
});

test('late completion keeps navigation focus and the receipt available when Settings is revisited', async ({ page }) => {
  await open(page); await consent(page).check(); await save(page).click();
  await navigate(page, 'usage'); await publish(page);
  await expect(page.locator('[data-workspace-nav="usage"]')).toBeFocused();
  await navigate(page, 'settings');
  await expect(page.locator('[data-export-result]')).toContainText('Saved readings exported.');
  await expect(page.locator('[data-workspace-nav="settings"]')).toBeFocused();
});

test('invalid and mismatched native receipts cannot produce a success message and failures require renewed consent', async ({ page }) => {
  await open(page);
  for (const value of [{ ...receipt, payload: 'must not reach UI' }, { ...receipt, profileCount: 1 }]) {
    await consent(page).check(); await save(page).click(); await publish(page, value);
    await expect(page.locator('[data-export-error]')).toBeFocused();
    await expect(page.locator('[data-export-error]')).toContainText('Check Downloads/OpenUsage');
    await expect(page.locator('[data-export-result]')).toHaveCount(0);
    await expect(consent(page)).not.toBeChecked(); await expect(save(page)).toBeDisabled();
  }
  await consent(page).check(); await save(page).click();
  await page.evaluate(() => (window as unknown as FixtureWindow).exportFixture.reject!('export_too_large'));
  await expect(page.locator('[data-export-error]')).toContainText('Choose fewer profiles');
});

test('fully deselected profiles cannot authorize an export', async ({ page }) => {
  await open(page);
  for (const id of [first, second]) await page.locator(`[data-export-profile="${id}"]`).uncheck();
  await expect(consent(page)).toBeDisabled(); await expect(save(page)).toBeDisabled();
  expect(await page.evaluate(() => (window as unknown as FixtureWindow).exportFixture.calls)).toEqual([]);
});

test('an empty workspace explains the prerequisite without offering an enabled save', async ({ page }) => {
  await open(page, true);
  await expect(page.getByText('No active profiles are available. Add an account profile before you export readings.', { exact: true })).toBeVisible();
  await expect(page.locator('[data-export-profile]')).toHaveCount(0);
  await expect(save(page)).toBeDisabled(); await expect(consent(page)).toBeDisabled();
});
