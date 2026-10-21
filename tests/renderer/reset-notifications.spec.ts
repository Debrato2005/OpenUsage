import { expect, test, type Page } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';
import type { NotificationPreferences } from '../../apps/desktop/src/contracts';
import { navigate } from './navigation';

// Synthetic renderer IPC. Rust/native tests own persistence, permission and OS delivery.
const first = '11111111-1111-4111-8111-111111111111';
const second = '22222222-2222-4222-8222-222222222222';
const archived = '33333333-3333-4333-8333-333333333333';
const defaults: NotificationPreferences = { reset: { enabled: false, timeZone: 'Asia/Kolkata', quietEnabled: true, quietStart: '22:00', quietEnd: '08:00', profileOverrides: [] }, usage: { enabled: false, profileOverrides: [] } };
type Pending = { resolve: (value: unknown) => void; reject: (error: unknown) => void };
type Fixture = { calls: { command: string; args: unknown }[]; value: NotificationPreferences; loadError: boolean; saveError: string | null; deferLoad: boolean; deferSave: boolean; loads: Pending[]; saves: Pending[] };
type FixtureWindow = Window & { resetNotificationFixture: Fixture };

async function open(page: Page, options: { loadError?: boolean; deferLoad?: boolean; deferSave?: boolean } = {}) {
  await page.addInitScript(({ first, second, archived, defaults, options }) => {
    let theme = 'light';
    const fixture: Fixture = { calls: [], value: defaults, loadError: !!options.loadError, saveError: null, deferLoad: !!options.deferLoad, deferSave: !!options.deferSave, loads: [], saves: [] };
    Object.assign(window, { resetNotificationFixture: fixture });
    const profiles = [first, second].map((id, index) => ({ id, label: index === 0 ? 'First account' : 'Second account', environment: 'native', archived: false, connectionState: 'authentication_required' }));
    profiles.push({ id: archived, label: 'Archived account', environment: 'native', archived: true, connectionState: 'authentication_required' });
    const bootstrap = () => ({ storage: 'ready', theme, schemaVersion: 7, activeProfiles: 2, monitoring: 'stopped', provider: 'not_configured', keyProtection: 'os_store', storageIssue: null });
    window.__TAURI__ = { core: { invoke: async <T>(command: string, args: Record<string, unknown> = {}) => {
      fixture.calls.push({ command, args });
      if (command === 'bootstrap') return bootstrap() as T;
      if (command === 'set_theme') { theme = String(args.theme); return bootstrap() as T; }
      if (command === 'get_display_scale') return { percent: 100 } as T;
      if (command === 'list_profiles') return { activeProfiles: 2, profiles } as T;
      if (command === 'load_monitoring') return { state: 'stopped', readingRevision: first, profiles: [] } as T;
      if (command === 'load_profile_refresh_status') return { profileId: args.profileId, retryAfterSeconds: 0 } as T;
      if (['load_profile_quota', 'load_profile_activity', 'load_profile_heatmap'].includes(command)) return null as T;
      if (command === 'load_notification_preferences') {
        if (fixture.loadError) throw 'storage_unavailable';
        if (fixture.deferLoad) return await new Promise<unknown>((resolve, reject) => fixture.loads.push({ resolve, reject })) as T;
        return fixture.value as T;
      }
      if (command === 'save_notification_preferences') {
        if (fixture.saveError) throw fixture.saveError;
        if (fixture.deferSave) return await new Promise<unknown>((resolve, reject) => fixture.saves.push({ resolve, reject })) as T;
        fixture.value = args.preferences as NotificationPreferences;
        return fixture.value as T;
      }
      throw new Error(`Unexpected synthetic command ${command}`);
    } } };
  }, { first, second, archived, defaults, options });
  await page.goto('/');
  await navigate(page, 'settings');
}
const panel = (page: Page) => page.locator('[data-reset-notifications]');
const toggle = (page: Page) => panel(page).getByRole('button', { name: 'Notifications', exact: true });
const enabled = (page: Page) => panel(page).getByRole('checkbox', { name: 'Notify me when a quota reset is confirmed', exact: true });
const usageEnabled = (page: Page) => panel(page).getByRole('checkbox', { name: 'Notify me at 80%, 90% and 100% quota used', exact: true });
const calls = (page: Page, command: string) => page.evaluate(command => (window as unknown as FixtureWindow).resetNotificationFixture.calls.filter(call => call.command === command), command);

test('notification preferences save both alert types atomically and preserve independent account overrides', async ({ page }) => {
  await open(page);
  expect(await calls(page, 'load_notification_preferences')).toHaveLength(0);
  await page.evaluate(archived => { const value = (window as unknown as FixtureWindow).resetNotificationFixture.value; value.reset.profileOverrides = [{ profileId: archived, enabled: true }]; value.usage.profileOverrides = [{ profileId: archived, enabled: false }]; }, archived);
  await toggle(page).focus(); await page.keyboard.press('Enter');
  await expect(enabled(page)).not.toBeChecked();
  await expect(usageEnabled(page)).not.toBeChecked();
  await expect(panel(page)).toContainText('Saved events are not replayed.');
  await enabled(page).check(); await usageEnabled(page).check();
  expect(await calls(page, 'save_notification_preferences')).toHaveLength(0);
  await panel(page).getByRole('button', { name: 'Quiet hours', exact: true }).click();
  await expect(panel(page).getByLabel('Start time', { exact: true })).toHaveValue('22:00');
  await expect(panel(page).getByLabel('End time', { exact: true })).toHaveValue('08:00');
  await panel(page).getByLabel('Start time', { exact: true }).fill('23:30');
  await panel(page).getByLabel('Time zone', { exact: true }).fill('Europe/London');
  await panel(page).getByRole('button', { name: 'Account overrides', exact: true }).click();
  await panel(page).getByRole('combobox', { name: 'Reset alerts for Second account', exact: true }).click();
  await page.getByRole('option', { name: 'Off', exact: true }).click();
  await panel(page).getByRole('combobox', { name: 'Usage alerts for First account', exact: true }).click();
  await page.getByRole('option', { name: 'On', exact: true }).click();
  await panel(page).getByRole('button', { name: 'Save preferences', exact: true }).click();
  await expect(panel(page).getByRole('status')).toHaveText('Notification preferences saved.');
  const expected = { reset: { ...defaults.reset, enabled: true, quietStart: '23:30', timeZone: 'Europe/London', profileOverrides: [{ profileId: archived, enabled: true }, { profileId: second, enabled: false }] }, usage: { enabled: true, profileOverrides: [{ profileId: archived, enabled: false }, { profileId: first, enabled: true }] } };
  expect(await calls(page, 'save_notification_preferences')).toEqual([{ command: 'save_notification_preferences', args: { preferences: expected } }]);
  await toggle(page).click(); await toggle(page).click();
  await expect(enabled(page)).toBeChecked();
  await expect(usageEnabled(page)).toBeChecked();
  await panel(page).getByRole('button', { name: 'Account overrides', exact: true }).click();
  await expect(panel(page).getByRole('combobox', { name: 'Reset alerts for First account', exact: true })).toHaveText('Default');
  await expect(panel(page).getByRole('combobox', { name: 'Reset alerts for Second account', exact: true })).toHaveText('Off');
  await expect(panel(page).getByRole('combobox', { name: 'Usage alerts for First account', exact: true })).toHaveText('On');
  await expect(panel(page).getByRole('combobox', { name: 'Usage alerts for Second account', exact: true })).toHaveText('Default');
  expect((await page.evaluate(() => (window as unknown as FixtureWindow).resetNotificationFixture.calls)).some(call => /^(begin_|start_profile|complete_profile|check_profile)/.test(call.command))).toBe(false);
});

test('notification preference failures retain the draft and give an actionable retry', async ({ page }) => {
  await open(page, { loadError: true });
  await toggle(page).click();
  await expect(panel(page).getByRole('alert')).toContainText('Notification preferences could not be loaded.');
  await page.evaluate(() => { (window as unknown as FixtureWindow).resetNotificationFixture.loadError = false; });
  await panel(page).getByRole('button', { name: 'Retry notification preferences', exact: true }).click();
  await enabled(page).check(); await usageEnabled(page).check();
  await page.evaluate(() => { (window as unknown as FixtureWindow).resetNotificationFixture.saveError = 'notification_permission_denied'; });
  await panel(page).getByRole('button', { name: 'Save preferences', exact: true }).click();
  await expect(panel(page).getByRole('alert')).toHaveText('Allow OpenUsage notifications in your system settings, then try again.');
  await expect(enabled(page)).toBeChecked();
  await expect(usageEnabled(page)).toBeChecked();
  expect(await page.evaluate(() => (window as unknown as FixtureWindow).resetNotificationFixture.value)).toEqual(defaults);
  await expect(panel(page).getByRole('status')).toHaveCount(0);
  await page.evaluate(() => { (window as unknown as FixtureWindow).resetNotificationFixture.saveError = null; });
  await panel(page).getByRole('button', { name: 'Save preferences', exact: true }).click();
  await expect(panel(page).getByRole('status')).toHaveText('Notification preferences saved.');
});

test('closing notification preferences discards a late load before reopening', async ({ page }) => {
  await open(page, { deferLoad: true });
  await toggle(page).click();
  await expect(panel(page).getByRole('status')).toHaveText('Loading notification preferences…');
  await toggle(page).click(); await toggle(page).click();
  await expect.poll(() => page.evaluate(() => (window as unknown as FixtureWindow).resetNotificationFixture.loads.length)).toBe(2);
  await page.evaluate(value => { (window as unknown as FixtureWindow).resetNotificationFixture.loads[0]!.resolve({ ...value, reset: { ...value.reset, enabled: true }, usage: { ...value.usage, enabled: true } }); }, defaults);
  await expect(enabled(page)).toHaveCount(0);
  await expect(usageEnabled(page)).toHaveCount(0);
  await page.evaluate(value => { (window as unknown as FixtureWindow).resetNotificationFixture.loads[1]!.resolve(value); }, defaults);
  await expect(enabled(page)).not.toBeChecked();
  await expect(usageEnabled(page)).not.toBeChecked();
  expect(await calls(page, 'save_notification_preferences')).toHaveLength(0);
});

test('a save reply from a previous Settings visit cannot replace a new draft or clear its busy state', async ({ page }) => {
  await open(page, { deferSave: true });
  await toggle(page).click(); await enabled(page).check();
  await panel(page).getByRole('button', { name: 'Save preferences', exact: true }).click();
  await expect(panel(page).getByRole('button', { name: 'Saving preferences…', exact: true })).toBeDisabled();
  await navigate(page, 'overview'); await navigate(page, 'settings');
  await toggle(page).click();
  await expect(enabled(page)).not.toBeChecked();
  await panel(page).getByRole('button', { name: 'Save preferences', exact: true }).click();
  await expect.poll(() => page.evaluate(() => (window as unknown as FixtureWindow).resetNotificationFixture.saves.length)).toBe(2);
  await page.evaluate(value => { (window as unknown as FixtureWindow).resetNotificationFixture.saves[0]!.resolve({ ...value, reset: { ...value.reset, enabled: true }, usage: { ...value.usage, enabled: true } }); }, defaults);
  await expect(panel(page).getByRole('button', { name: 'Saving preferences…', exact: true })).toBeDisabled();
  await expect(enabled(page)).not.toBeChecked();
  await expect(usageEnabled(page)).not.toBeChecked();
  await page.evaluate(value => { (window as unknown as FixtureWindow).resetNotificationFixture.saves[1]!.resolve(value); }, defaults);
  await expect(panel(page).getByRole('status')).toHaveText('Notification preferences saved.');
  await expect(panel(page).getByRole('button', { name: 'Save preferences', exact: true })).toBeEnabled();
});

test('notification settings remain readable and keyboard accessible at 360 pixels with enlarged text', async ({ page }) => {
  await open(page); await toggle(page).click();
  await panel(page).getByRole('button', { name: 'Quiet hours', exact: true }).click();
  await panel(page).getByRole('button', { name: 'Account overrides', exact: true }).click();
  await page.setViewportSize({ width: 360, height: 800 });
  await page.evaluate(() => { const sizes = [...document.querySelectorAll<HTMLElement>('body *')].map(node => ({ node, size: getComputedStyle(node).fontSize })); for (const { node, size } of sizes) node.style.fontSize = `${parseFloat(size) * 2}px`; });
  await enabled(page).focus(); await page.keyboard.press('Space'); await expect(enabled(page)).toBeChecked();
  await usageEnabled(page).focus(); await page.keyboard.press('Space'); await expect(usageEnabled(page)).toBeChecked();
  for (const theme of ['light', 'dark']) {
    await page.getByRole('combobox', { name: 'Appearance', exact: true }).click();
    await page.getByRole('option', { name: theme === 'light' ? 'Light' : 'Dark', exact: true }).click();
    await expect(page.locator('html')).toHaveAttribute('data-theme', theme);
    await enabled(page).focus(); await expect(enabled(page)).toBeFocused();
    expect(await page.evaluate(() => document.scrollingElement!.scrollWidth <= innerWidth + 1)).toBe(true);
    const audit = await new AxeBuilder({ page }).withTags(['wcag2a', 'wcag2aa', 'wcag21aa']).analyze();
    expect(audit.violations, `${theme}, 360px and200% text`).toEqual([]);
  }
});
