import { expect, test, type Page } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';
import type { SavedAlertHistory } from '../../apps/desktop/src/contracts';
import { navigate } from './navigation';

// Synthetic renderer IPC only. Native tests own persistence and reset evidence.
const first = '11111111-1111-4111-8111-111111111111';
const second = '22222222-2222-4222-8222-222222222222';
const saved = { events: [
  { id: 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa', profileId: second, profileLabel: 'Second account', kind: 'quota_reset', thresholdPercent: null, evidenceRunId: 'cccccccc-cccc-4ccc-8ccc-cccccccccccc', limitId: 'codex', windowRole: 'primary', windowDurationMins: 300, evidenceAtUtc: '2026-10-22T03:04:05.123Z', sourceKind: 'test_fixture', deliveryState: 'recorded' },
  { id: 'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb', profileId: first, profileLabel: 'First account', kind: 'quota_reset', thresholdPercent: null, evidenceRunId: 'cccccccc-cccc-4ccc-8ccc-cccccccccccc', limitId: 'codex', windowRole: 'secondary', windowDurationMins: 10080, evidenceAtUtc: '2026-10-21T23:59:59.000Z', sourceKind: 'official_artifact', deliveryState: 'recorded' },
], truncated: false } satisfies SavedAlertHistory;
const operational: SavedAlertHistory = { events: [
  { id: 'eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee', profileId: second, profileLabel: 'Second account', kind: 'authentication_required', evidenceAtUtc: '2026-10-22T03:04:05.123Z', sourceKind: 'test_fixture', deliveryState: 'submitted' },
  { id: 'ffffffff-ffff-4fff-8fff-ffffffffffff', profileId: first, profileLabel: 'First account', kind: 'collection_failure', evidenceAtUtc: '2026-10-22T03:05:06.123Z', sourceKind: 'test_fixture', deliveryState: 'queued' },
], truncated: false };
type Fixture = { calls: string[]; value: unknown; fail: boolean; defer: boolean; pending: ((value: unknown) => void)[] };
type FixtureWindow = Window & { alertFixture: Fixture };

async function open(page: Page, options: { empty?: boolean; fail?: boolean; defer?: boolean } = {}) {
  await page.addInitScript(({ first, second, value, options }) => {
    const fixture: Fixture = { calls: [], value: options.empty ? { events: [], truncated: false } : value, fail: !!options.fail, defer: !!options.defer, pending: [] };
    Object.assign(window, { alertFixture: fixture });
    let theme = 'light';
    const profiles = [first, second].map((id, index) => ({ id, label: index === 0 ? 'First account' : 'Second account', environment: 'native', archived: false, connectionState: 'authentication_required' }));
    const bootstrap = () => ({ storage: 'ready', theme, schemaVersion: 8, activeProfiles: 2, monitoring: 'stopped', provider: 'not_configured', keyProtection: 'os_store', storageIssue: null });
    window.__TAURI__ = { core: { invoke: async <T>(command: string, args: Record<string, unknown> = {}) => {
      if (command === 'load_time_zone') return { timeZone: 'UTC' } as T;
      fixture.calls.push(command);
      if (command === 'bootstrap') return bootstrap() as T;
      if (command === 'set_theme') { theme = String(args.theme); return bootstrap() as T; }
      if (command === 'get_display_scale') return { percent: 100 } as T;
      if (command === 'list_profiles') return { activeProfiles: 2, profiles } as T;
      if (command === 'load_monitoring') return { state: 'stopped', readingRevision: first, profiles: [] } as T;
      if (command === 'load_profile_refresh_status') return { profileId: args.profileId, retryAfterSeconds: 0 } as T;
      if (['load_profile_quota', 'load_profile_activity', 'load_profile_heatmap'].includes(command)) return null as T;
      if (command === 'load_alert_history') {
        if (fixture.fail) throw 'alert_history_unavailable';
        if (fixture.defer) return await new Promise<unknown>(resolve => fixture.pending.push(resolve)) as T;
        return fixture.value as T;
      }
      throw new Error(`Unexpected synthetic command ${command}`);
    } } };
  }, { first, second, value: saved, options });
  await page.goto('/');
  await navigate(page, 'settings');
}
const history = (page: Page) => page.locator('[data-alert-history]');
const events = (page: Page) => history(page).locator('[data-alert-history-event]');
const calls = (page: Page) => page.evaluate(() => (window as unknown as FixtureWindow).alertFixture.calls);

test('alert history loads on expansion, preserves exact evidence and opens the event account', async ({ page }) => {
  await open(page);
  expect((await calls(page)).filter(command => command === 'load_alert_history')).toEqual([]);
  const trigger = history(page).getByRole('button', { name: 'Alert history', exact: true });
  await trigger.focus(); await page.keyboard.press('Enter');
  await expect(trigger).toHaveAttribute('aria-expanded', 'true');
  await expect(events(page)).toHaveCount(2);
  const event = events(page).first();
  await expect(event).toContainText('Reset confirmed');
  await expect(event).toContainText('Second account');
  await expect(event).toContainText('5-hour window');
  await expect(event.locator('time').first()).toContainText('Oct 22, 2026');
  await expect(event.locator('time').first()).toContainText('3:04 AM UTC');
  await expect(event.locator('time').first()).toHaveAttribute('datetime', '2026-10-22T03:04:05.123Z');
  await expect(event.locator('time').first()).toHaveAttribute('title', '2026-10-22T03:04:05.123Z');
  await expect(history(page)).toContainText('Alerts saved on this device.');
  await event.getByRole('button', { name: 'Source details', exact: true }).click();
  await expect(event).toContainText('Synthetic test data');
  await expect(event).toContainText('Primary · 300 minutes');
  await expect(event.locator('.alert-history-source time')).toHaveText('2026-10-22T03:04:05.123Z');
  await expect(event).toContainText('No desktop notification sent.');
  await expect(events(page).nth(1)).toContainText('7-day window');
  await event.getByRole('button', { name: 'View usage for Second account', exact: true }).click();
  await expect(page.locator('[data-workspace-nav="usage"]')).toHaveAttribute('aria-current', 'page');
  await expect(page.getByRole('combobox', { name: 'Usage profile', exact: true })).toContainText('Second account');
  expect((await calls(page)).some(command => /^(begin_|complete_|check_profile|start_profile)/.test(command))).toBe(false);
});

test('threshold history distinguishes exact levels and covered lower alerts from desktop submission', async ({ page }) => {
  await open(page);
  const thresholds: SavedAlertHistory = { events: [
    { ...saved.events[0]!, kind: 'quota_threshold', thresholdPercent: 100, deliveryState: 'submitted' },
    { ...saved.events[0]!, id: saved.events[1]!.id, kind: 'quota_threshold', thresholdPercent: 90, deliveryState: 'covered' },
    { ...saved.events[0]!, id: 'dddddddd-dddd-4ddd-8ddd-dddddddddddd', kind: 'quota_threshold', thresholdPercent: 80, deliveryState: 'covered' },
  ], truncated: false };
  await page.evaluate(value => { (window as unknown as FixtureWindow).alertFixture.value = value; }, thresholds);
  await history(page).getByRole('button', { name: 'Alert history', exact: true }).click();
  await expect(events(page)).toHaveCount(3);
  for (const [index, threshold] of [100, 90, 80].entries()) {
    const event = events(page).nth(index);
    await expect(event.locator('strong')).toHaveText(`${threshold}% quota used`);
    await event.getByRole('button', { name: 'Source details', exact: true }).click();
    if (index === 0) await expect(event).toContainText('Accepted by the desktop notification service.');
    else {
      await expect(event).toContainText('A higher threshold was recorded. No separate desktop notification sent.');
      await expect(event).not.toContainText('Accepted by the desktop notification service.');
    }
  }
  await events(page).nth(1).getByRole('button', { name: 'View usage for Second account', exact: true }).click();
  await expect(page.getByRole('combobox', { name: 'Usage profile', exact: true })).toContainText('Second account');
});

test('sign-in and reading alerts show only supported evidence and open the exact account controls', async ({ page }) => {
  await open(page);
  await page.evaluate(value => { (window as unknown as FixtureWindow).alertFixture.value = value; }, operational);
  await history(page).getByRole('button', { name: 'Alert history', exact: true }).click();
  await expect(events(page)).toHaveCount(2);
  for (const [index, expected] of [
    { title: 'Sign-in required', account: 'Second account', profileId: second, delivery: 'Accepted by the desktop notification service.' },
    { title: 'Three readings failed', account: 'First account', profileId: first, delivery: 'Waiting for quiet hours to end.' },
  ].entries()) {
    const event = events(page).nth(index);
    await expect(event.locator('strong')).toHaveText(expected.title);
    await event.getByRole('button', { name: 'Source details', exact: true }).click();
    await expect(event.locator('dt')).toHaveText(['Source', 'Evidence time (UTC)', 'Delivery']);
    await expect(event).toContainText(expected.delivery);
    await expect(event).not.toContainText(/quota|window|invalid credentials/i);
    await expect(event.getByRole('button', { name: /View usage/ })).toHaveCount(0);
    const action = event.getByRole('button', { name: `View account for ${expected.account}`, exact: true });
    await action.focus(); await page.keyboard.press('Enter');
    const account = page.locator('[data-account-panel]');
    await expect(account).toHaveAttribute('open');
    await expect(account.getByRole('heading', { name: expected.account, exact: true }).first()).toBeVisible();
    await expect(account.locator(`[data-profile-id="${expected.profileId}"]`)).toBeVisible();
    await account.locator('[data-account-panel-close]').click();
    await expect(action).toBeFocused();
  }
  expect((await calls(page)).some(command => /^(begin_|complete_|check_profile|start_profile)/.test(command))).toBe(false);
});

test('alert history reports an empty list and refreshes only when requested', async ({ page }) => {
  await open(page, { empty: true });
  await history(page).getByRole('button', { name: 'Alert history', exact: true }).click();
  await expect(history(page)).toContainText('No alerts are saved yet.');
  await page.evaluate(value => { (window as unknown as FixtureWindow).alertFixture.value = { events: Array.from({ length: 100 }, (_, index) => ({ ...value.events[index % 2]!, id: `${index.toString(16).padStart(8, '0')}-cccc-4ccc-8ccc-cccccccccccc` })), truncated: true }; }, saved);
  await history(page).getByRole('button', { name: 'Refresh list', exact: true }).click();
  await expect(events(page)).toHaveCount(100);
  await expect(history(page)).toContainText('Showing the latest 100 saved events.');
  expect((await calls(page)).filter(command => command === 'load_alert_history')).toHaveLength(2);
});

test('alert history fails closed on errors and malformed saved events, then retries', async ({ page }) => {
  await open(page, { fail: true });
  await history(page).getByRole('button', { name: 'Alert history', exact: true }).click();
  await expect(history(page).getByRole('alert')).toContainText('Alert history could not be loaded.');
  for (const invalid of [
    { ...saved.events[0], windowDurationMins: 0 },
    { ...saved.events[0], evidenceAtUtc: '2026-02-30T03:04:05.123Z' },
    { ...saved.events[0], deliveryState: 'delivered' },
    { ...saved.events[0], sourceKind: 'unverified' },
    { ...saved.events[0], kind: 'quota_threshold', thresholdPercent: null },
    { ...saved.events[0], kind: 'quota_threshold', thresholdPercent: 85 },
    { ...saved.events[0], thresholdPercent: 80 },
    { ...saved.events[0], deliveryState: 'covered' },
    { ...saved.events[0], evidenceRunId: 'not-a-run-id' },
    { ...operational.events[0], limitId: 'codex' },
    { ...operational.events[0], deliveryState: 'covered' },
    { ...operational.events[1], rawError: 'private provider response' },
  ]) {
    await page.evaluate(value => { const fixture = (window as unknown as FixtureWindow).alertFixture; fixture.fail = false; fixture.value = { events: [value], truncated: false }; }, invalid);
    await history(page).getByRole('button', { name: 'Retry alert history', exact: true }).click();
    await expect(history(page).getByRole('alert')).toContainText('could not be loaded');
    await expect(events(page)).toHaveCount(0);
  }
  await page.evaluate(value => { (window as unknown as FixtureWindow).alertFixture.value = value; }, saved);
  await history(page).getByRole('button', { name: 'Retry alert history', exact: true }).click();
  await expect(events(page)).toHaveCount(2);
  await expect(history(page).getByRole('alert')).toHaveCount(0);
});

test('closing alert history ignores a late reply before a new load', async ({ page }) => {
  await open(page, { defer: true });
  const trigger = history(page).getByRole('button', { name: 'Alert history', exact: true });
  await trigger.click();
  await expect(history(page).getByRole('status')).toHaveText('Loading alert history…');
  await trigger.click();
  await page.evaluate(() => { const fixture = (window as unknown as FixtureWindow).alertFixture; fixture.defer = false; fixture.value = { events: [], truncated: false }; });
  await trigger.click();
  await expect(history(page)).toContainText('No alerts are saved yet.');
  await page.evaluate(value => { for (const resolve of (window as unknown as FixtureWindow).alertFixture.pending.splice(0)) resolve(value); }, saved);
  await expect(history(page)).toContainText('No alerts are saved yet.');
  await expect(events(page)).toHaveCount(0);
  expect((await calls(page)).filter(command => command === 'load_alert_history')).toHaveLength(2);
});

test('alert history remains readable and keyboard operable in both themes at 200% text', async ({ page }) => {
  await open(page);
  await page.evaluate(value => { (window as unknown as FixtureWindow).alertFixture.value = value; }, { events: [...saved.events, ...operational.events], truncated: false });
  await history(page).getByRole('button', { name: 'Alert history', exact: true }).click();
  await expect(events(page)).toHaveCount(4);
  const details = events(page).first().getByRole('button', { name: 'Source details', exact: true });
  await details.focus(); await page.keyboard.press('Enter');
  await page.setViewportSize({ width: 360, height: 800 });
  for (const theme of ['light', 'dark']) {
    await page.getByRole('combobox', { name: 'Appearance', exact: true }).click();
    await page.getByRole('option', { name: theme === 'light' ? 'Light' : 'Dark', exact: true }).click();
    await expect(page.locator('html')).toHaveAttribute('data-theme', theme);
    for (const scale of [1, 2]) {
      await page.evaluate(scale => { for (const node of document.querySelectorAll<HTMLElement>('body *')) { const baseline = node.dataset.alertTestSize ?? getComputedStyle(node).fontSize; node.dataset.alertTestSize = baseline; node.style.fontSize = `${parseFloat(baseline) * scale}px`; } }, scale);
      expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth + 1)).toBe(true);
      const buttons = await history(page).getByRole('button').all();
      for (const button of buttons) if (await button.isVisible()) expect(await button.evaluate(node => node.getBoundingClientRect().right)).toBeLessThanOrEqual(361);
      expect((await new AxeBuilder({ page }).include('[data-alert-history]').withTags(['wcag2a', 'wcag2aa', 'wcag21aa']).analyze()).violations).toEqual([]);
    }
  }
});
