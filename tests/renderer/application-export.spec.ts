import { expect, test, type Page } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';
import type { ApplicationExportReceipt, Bootstrap } from '../../apps/desktop/src/contracts';
import { navigate } from './navigation';

// Synthetic renderer IPC only. Native tests own snapshot completeness, exclusions and file publication.
const profileId = '11111111-1111-4111-8111-111111111111';
const ready: Bootstrap = { storage: 'ready', theme: 'light', schemaVersion: 8, activeProfiles: 0, monitoring: 'stopped', provider: 'not_configured', keyProtection: 'os_store', storageIssue: null };
const receipt: ApplicationExportReceipt = { fileName: 'openusage-application-0123456789abcdef.json', directory: '/synthetic/Downloads/OpenUsage', format: 'json', schemaVersion: 8, tableCount: 30, recordCount: '0', byteCount: '1024', exportedAt: '2026-10-22T00:00:00.000Z' };
type Deferred = { resolve: (value: unknown) => void; reject: (error: unknown) => void };
type Fixture = { calls: { command: string; args: Record<string, unknown> }[]; exports: Deferred[]; value: Bootstrap };
type FixtureWindow = Window & { applicationExportFixture: Fixture };

async function open(page: Page, archived = false) {
  await page.addInitScript(({ profileId, initial, archived }) => {
    const state: Fixture = { calls: [], exports: [], value: initial }; Object.assign(window, { applicationExportFixture: state });
    window.__TAURI__ = { core: { invoke: async <T>(command: string, args: Record<string, unknown> = {}) => {
      if (command === 'load_time_zone') return { timeZone: 'UTC' } as T;
      state.calls.push({ command, args });
      if (command === 'bootstrap') return state.value as T;
      if (command === 'set_theme') { state.value = { ...state.value, theme: args.theme as Bootstrap['theme'] }; return state.value as T; }
      if (command === 'get_display_scale') return { percent: 100 } as T;
      if (command === 'list_profiles') return { activeProfiles: 0, profiles: archived ? [{ id: profileId, label: 'Synthetic archived account', environment: 'native', archived: true, connectionState: 'authentication_required' }] : [] } as T;
      if (command === 'load_monitoring') return { state: 'stopped', readingRevision: profileId, profiles: [] } as T;
      if (command === 'load_profile_refresh_status') return { profileId: args.profileId, retryAfterSeconds: 0 } as T;
      if (['load_profile_quota', 'load_profile_activity', 'load_profile_heatmap'].includes(command)) return null as T;
      if (command === 'export_application_data') return await new Promise<unknown>((resolve, reject) => state.exports.push({ resolve, reject })) as T;
      throw new Error(`Unexpected synthetic command ${command}`);
    } } };
  }, { profileId, initial: ready, archived });
  await page.goto('/'); await navigate(page, 'settings');
  await toggle(page).focus(); await page.keyboard.press('Enter');
  await expect(form(page)).toBeVisible();
}
const section = (page: Page) => page.locator('[data-application-export]');
const toggle = (page: Page) => section(page).locator('[data-application-export-toggle]');
const form = (page: Page) => section(page).getByRole('form', { name: 'Application data export', exact: true });
const consent = (page: Page) => section(page).locator('[data-application-export-consent]');
const save = (page: Page) => section(page).locator('[data-application-export-save]');
const result = (page: Page) => section(page).locator('[data-application-export-result]');
const error = (page: Page) => section(page).locator('[data-application-export-error]');
const calls = (page: Page) => page.evaluate(() => (window as unknown as FixtureWindow).applicationExportFixture.calls.filter(call => call.command === 'export_application_data'));
async function publish(page: Page, index: number, value: unknown = receipt) {
  await expect.poll(() => page.evaluate(() => (window as unknown as FixtureWindow).applicationExportFixture.exports.length)).toBeGreaterThan(index);
  await page.evaluate(({ index, value }) => (window as unknown as FixtureWindow).applicationExportFixture.exports[index]!.resolve(value), { index, value });
}

test('zero-account application export requires explicit renewed consent and submits only once', async ({ page }) => {
  await open(page); expect(await calls(page)).toEqual([]);
  await expect(consent(page)).not.toBeChecked(); await expect(save(page)).toBeDisabled();
  await form(page).evaluate(node => node.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true })));
  expect(await calls(page)).toEqual([]);
  await expect(form(page).getByRole('combobox')).toHaveCount(0); await expect(form(page).getByRole('checkbox')).toHaveCount(1);
  await consent(page).check(); await toggle(page).click(); await toggle(page).click();
  await expect(consent(page)).not.toBeChecked(); await expect(save(page)).toBeDisabled();
  await consent(page).focus(); await page.keyboard.press('Space'); await save(page).focus(); await page.keyboard.press('Enter');
  await expect(form(page)).toHaveAttribute('aria-busy', 'true'); await expect(toggle(page)).toBeDisabled();
  await form(page).evaluate(node => node.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true })));
  expect((await calls(page)).map(call => call.args)).toEqual([{ confirmed: true }]);
  await publish(page, 0); await expect(result(page)).toBeFocused();
  await expect(result(page)).toContainText('0 records · JSON'); await expect(consent(page)).not.toBeChecked(); await expect(save(page)).toBeDisabled();
});

test('archived-only export explains sensitive plaintext contents and preserves exact large record counts', async ({ page }) => {
  await open(page, true);
  await expect(form(page)).toContainText('including archived accounts, names, email addresses, saved readings and history, settings, alerts and local references');
  await expect(form(page)).toContainText('Credentials, encryption keys and provider home files are excluded.');
  await expect(form(page)).toContainText('Use an encrypted backup to recover your workspace.');
  await expect(form(page).getByRole('textbox')).toHaveCount(0);
  await consent(page).check(); await save(page).click();
  await publish(page, 0, { ...receipt, recordCount: '9007199254740993' });
  await expect(result(page)).toContainText('9007199254740993 records · JSON');
  await expect(result(page)).toContainText('Application data exported. This file is not encrypted.');
  await expect(result(page)).toContainText(receipt.fileName); await expect(result(page)).toContainText(receipt.directory);
  await expect(result(page).getByText('Schema version 8 · 30 tables · 1024 bytes', { exact: true })).toBeHidden();
  await result(page).getByText('Export details', { exact: true }).click();
  await expect(result(page).getByText('Schema version 8 · 30 tables · 1024 bytes', { exact: true })).toBeVisible();
  expect((await calls(page)).map(call => call.args)).toEqual([{ confirmed: true }]);
});

test('actionable failures require renewed consent without retries or partial-file promises', async ({ page }) => {
  await open(page);
  for (const [index, failure] of [
    { code: 'workspace_locked', text: 'Unlock your workspace' },
    { code: 'export_unavailable', text: 'Check your Downloads folder and free space' },
    { code: 'insufficient_space', text: 'Free space in your Downloads folder' },
    { code: 'export_too_large', text: '5 GiB size limit or two-minute time limit. No partial file was saved.' },
  ].entries()) {
    await consent(page).check(); await save(page).click();
    await expect.poll(() => page.evaluate(() => (window as unknown as FixtureWindow).applicationExportFixture.exports.length)).toBe(index + 1);
    await page.evaluate(({ index, code }) => (window as unknown as FixtureWindow).applicationExportFixture.exports[index]!.reject(code), { index, code: failure.code });
    await expect(error(page)).toContainText(failure.text); await expect(error(page)).toBeFocused();
    await expect(consent(page)).not.toBeChecked(); await expect(save(page)).toBeDisabled(); await expect(result(page)).toHaveCount(0);
    expect(await calls(page)).toHaveLength(index + 1);
  }
  await consent(page).check(); await save(page).click(); await publish(page, 4);
  await expect(result(page)).toBeVisible(); await expect(error(page)).toHaveCount(0);
});

test('invalid receipts and unexpected native fields never render success or raw response data', async ({ page }) => {
  await open(page);
  for (const [index, invalid] of [
    { ...receipt, recordCount: 9007199254740992 },
    { ...receipt, credentials: 'SYNTHETIC-PRIVATE-MARKER' },
    { ...receipt, fileName: '../outside.json' },
  ].entries()) {
    await consent(page).check(); await save(page).click(); await publish(page, index, invalid);
    await expect(error(page)).toContainText('could not confirm the saved export'); await expect(result(page)).toHaveCount(0);
    await expect(section(page)).not.toContainText('SYNTHETIC-PRIVATE-MARKER'); await expect(consent(page)).not.toBeChecked();
  }
});

test('an unmounted request cannot release a new export owner or render its stale reply', async ({ page }) => {
  await open(page); await consent(page).check(); await save(page).click();
  await expect.poll(() => page.evaluate(() => (window as unknown as FixtureWindow).applicationExportFixture.exports.length)).toBe(1);
  await navigate(page, 'usage'); await expect(section(page)).toHaveCount(0);
  await navigate(page, 'settings'); await toggle(page).click(); await expect(consent(page)).not.toBeChecked();
  await consent(page).check(); await save(page).click();
  await expect.poll(() => page.evaluate(() => (window as unknown as FixtureWindow).applicationExportFixture.exports.length)).toBe(2);
  await publish(page, 0, { ...receipt, recordCount: '111' });
  await expect(form(page)).toHaveAttribute('aria-busy', 'true'); await expect(save(page)).toBeDisabled(); await expect(result(page)).toHaveCount(0);
  await publish(page, 1, { ...receipt, recordCount: '222' });
  await expect(result(page)).toContainText('222 records'); await expect(result(page)).not.toContainText('111 records');
});

test('application export remains readable and keyboard accessible in narrow light and dark views', async ({ page }) => {
  await open(page); await page.setViewportSize({ width: 360, height: 800 });
  for (const theme of ['light', 'dark']) {
    await page.getByRole('combobox', { name: 'Appearance', exact: true }).click(); await page.getByRole('option', { name: theme === 'light' ? 'Light' : 'Dark', exact: true }).click();
    for (const scale of [1, 2]) {
      await page.evaluate(scale => { for (const node of document.querySelectorAll<HTMLElement>('body *')) { const base = node.dataset.applicationExportTestSize ?? getComputedStyle(node).fontSize; node.dataset.applicationExportTestSize = base; node.style.fontSize = `${parseFloat(base) * scale}px`; } }, scale);
      await consent(page).focus(); await page.keyboard.press('Space'); await expect(consent(page)).toBeChecked();
      const geometry = await section(page).evaluate(node => ({ width: node.clientWidth, scrollWidth: node.scrollWidth, overflow: [...node.querySelectorAll<HTMLElement>('*')].filter(child => child.scrollWidth > child.clientWidth + 1).map(child => ({ tag: child.tagName, text: child.textContent?.slice(0, 90), width: child.clientWidth, scrollWidth: child.scrollWidth })) }));
      expect(geometry.scrollWidth, JSON.stringify({ theme, scale, ...geometry })).toBeLessThanOrEqual(geometry.width + 1);
      expect(await page.evaluate(() => document.scrollingElement!.scrollWidth)).toBeLessThanOrEqual(361);
      expect((await new AxeBuilder({ page }).include('[data-application-export]').withTags(['wcag2a', 'wcag2aa', 'wcag21aa']).analyze()).violations).toEqual([]);
      await page.keyboard.press('Space'); await expect(consent(page)).not.toBeChecked();
    }
  }
  expect(await calls(page)).toEqual([]);
});
