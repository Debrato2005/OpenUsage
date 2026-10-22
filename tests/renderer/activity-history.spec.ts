import { expect, test, type Page } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';
import type { ActivityHistoryEntry, ActivityHistoryPage } from '../../apps/desktop/src/contracts';
import { navigate } from './navigation';

// Synthetic renderer IPC only. These checks do not establish native persistence or provider support.
const first = '11111111-1111-4111-8111-111111111111';
const second = '22222222-2222-4222-8222-222222222222';
const uuid = (value: number) => `${value.toString(16).padStart(8, '0')}-aaaa-4aaa-8aaa-aaaaaaaaaaaa`;
function entry(profileId: string, ordinal: number): ActivityHistoryEntry {
  return { profileId, bindingId: uuid(profileId === first ? ordinal === 99 ? 9 : 1 : 2), collectionRunId: uuid(ordinal), observedAtUtc: new Date(Date.UTC(2026, 9, 22, 0, 0, ordinal)).toISOString(), sourceKind: ordinal === 99 ? 'official_artifact' : 'test_fixture', providerVersion: '0.163.0', sourceScope: 'account_activity_with_unknown_application_scope', coverage: 'unknown', summary: ordinal === 98 ? { lifetimeTokens: '0', peakDailyTokens: '0', longestRunningTurnSec: '0', currentStreakDays: '0', longestStreakDays: '0' } : { lifetimeTokens: '9007199254740993', peakDailyTokens: '9223372036854775807', longestRunningTurnSec: null, currentStreakDays: '0', longestStreakDays: null }, dailyBucketsState: ordinal === 99 ? 'unavailable' : ordinal === 98 ? 'empty' : 'returned' };
}
type Pending = { profileId: string; beforeRunId: string | null; resolve: (value: unknown) => void };
type Fixture = { calls: { command: string; args: Record<string, unknown> }[]; deferFirst: boolean; pending: Pending[]; failOlder: boolean; override: unknown; readingRevision: string };
type FixtureWindow = Window & { activityHistoryFixture: Fixture };

async function open(page: Page, options: { deferFirst?: boolean; failOlder?: boolean } = {}) {
  const entries = Array.from({ length: 22 }, (_, index) => entry(first, 100 - index));
  await page.addInitScript(({ first, second, entries, other, options }) => {
    const state: Fixture = { calls: [], deferFirst: !!options.deferFirst, pending: [], failOlder: !!options.failOlder, override: null, readingRevision: first };
    Object.assign(window, { activityHistoryFixture: state });
    let theme = 'light';
    const profiles = [first, second].map((id, index) => ({ id, label: `History account ${index + 1}`, environment: 'native', archived: false, connectionState: 'authentication_required' }));
    const bootstrap = () => ({ storage: 'ready', theme, schemaVersion: 8, activeProfiles: 2, monitoring: 'stopped', provider: 'not_configured', keyProtection: 'os_store', storageIssue: null });
    window.__TAURI__ = { core: { invoke: async <T>(command: string, args: Record<string, unknown> = {}) => {
      state.calls.push({ command, args });
      if (command === 'bootstrap') return bootstrap() as T;
      if (command === 'set_theme') { theme = String(args.theme); return bootstrap() as T; }
      if (command === 'get_display_scale') return { percent: 100 } as T;
      if (command === 'list_profiles') return { activeProfiles: 2, profiles } as T;
      if (command === 'load_monitoring') return { state: 'stopped', readingRevision: state.readingRevision, profiles: [] } as T;
      if (command === 'load_profile_refresh_status') return { profileId: args.profileId, retryAfterSeconds: 0 } as T;
      if (['load_profile_quota', 'load_profile_activity', 'load_profile_heatmap'].includes(command)) return null as T;
      if (command === 'load_profile_activity_history') {
        if (args.profileId === first && state.deferFirst) return await new Promise<unknown>(resolve => state.pending.push({ profileId: first, beforeRunId: args.beforeRunId as string | null, resolve })) as T;
        if (state.override !== null) return state.override as T;
        if (state.failOlder && args.beforeRunId !== null) throw 'invalid_arguments';
        if (args.profileId === second) return { profileId: second, entries: [other], nextBeforeRunId: null } as T;
        return { profileId: first, entries: args.beforeRunId === null ? entries.slice(0, 20) : entries.slice(20), nextBeforeRunId: args.beforeRunId === null ? entries[19]!.collectionRunId : null } as T;
      }
      throw new Error(`Unexpected synthetic command ${command}`);
    } } };
  }, { first, second, entries, other: entry(second, 200), options });
  await page.goto('/'); await navigate(page, 'usage');
}
const history = (page: Page) => page.locator('[data-activity-history]');
const rows = (page: Page) => history(page).locator('[data-activity-history-run]');
const trigger = (page: Page) => history(page).locator('[data-activity-history-open]');
const calls = (page: Page) => page.evaluate(() => (window as unknown as FixtureWindow).activityHistoryFixture.calls.filter(call => call.command === 'load_profile_activity_history'));
async function choose(page: Page, name: string) { await page.getByRole('combobox', { name: 'Usage profile', exact: true }).click(); await page.getByRole('option', { name, exact: true }).click(); }

test('saved summary history loads on request, preserves exact values and pages without combining snapshots', async ({ page }) => {
  await open(page); expect(await calls(page)).toEqual([]);
  await trigger(page).focus(); await page.keyboard.press('Enter'); await expect(rows(page)).toHaveCount(20);
  await expect(history(page)).toContainText('not individual usage events');
  const latest = rows(page).first(); await latest.getByRole('button').first().click();
  for (const [name, value] of Object.entries({ lifetimeTokens: '9007199254740993', peakDailyTokens: '9223372036854775807', longestRunningTurnSec: 'Not available', currentStreakDays: '0', longestStreakDays: 'Not available' })) await expect(latest.locator(`[data-activity-history-metric="${name}"]`)).toHaveText(value);
  await expect(latest.locator('[data-activity-history-daily-state]')).toHaveAttribute('data-activity-history-daily-state', 'returned');
  await latest.getByRole('button', { name: 'Source details', exact: true }).click();
  await expect(latest).toContainText('Synthetic provider'); await expect(latest).toContainText('0.163.0'); await expect(latest).toContainText('Application scope'); await expect(latest).toContainText('Coverage');
  await expect(latest.locator('.activity-history-source dd').filter({ hasText: /^Unknown$/ })).toHaveCount(2);
  await expect(latest.locator('.activity-history-source time')).toHaveText(entry(first, 100).observedAtUtc);
  await expect(latest.locator('.activity-history-source')).toContainText(uuid(1)); await expect(latest.locator('.activity-history-source')).toContainText(uuid(100));
  await rows(page).nth(1).getByRole('button').first().click();
  await expect(rows(page).nth(1).locator('[data-activity-history-daily-state]')).toHaveAttribute('data-activity-history-daily-state', 'unavailable');
  await rows(page).nth(1).getByRole('button', { name: 'Source details', exact: true }).click();
  await expect(rows(page).nth(1)).toContainText('Official OpenAI companion'); await expect(rows(page).nth(1)).toContainText(uuid(9));
  await rows(page).nth(2).getByRole('button').first().click();
  await expect(rows(page).nth(2).locator('[data-activity-history-metric]')).toHaveText(['0', '0', '0', '0', '0']);
  await expect(rows(page).nth(2).locator('[data-activity-history-daily-state]')).toHaveAttribute('data-activity-history-daily-state', 'empty');
  await expect(history(page).locator('[data-activity-day]')).toHaveCount(0);
  await history(page).getByRole('button', { name: 'Older readings', exact: true }).click(); await expect(rows(page)).toHaveCount(2);
  await expect(rows(page).first()).toHaveAttribute('data-activity-history-run', uuid(80)); await expect(history(page)).toContainText('Page 2');
  await expect(history(page).getByRole('button', { name: 'Older readings', exact: true })).toBeDisabled();
  await history(page).getByRole('button', { name: 'Newer readings', exact: true }).click(); await expect(rows(page)).toHaveCount(20);
  expect((await calls(page)).map(call => call.args)).toEqual([{ profileId: first, beforeRunId: null }, { profileId: first, beforeRunId: uuid(81) }, { profileId: first, beforeRunId: null }]);
  expect(await page.evaluate(() => (window as unknown as FixtureWindow).activityHistoryFixture.calls.some(call => /^(begin_|complete_|check_profile|start_profile)/.test(call.command)))).toBe(false);
});

test('invalid older cursors give retry and latest recovery without showing a stale page', async ({ page }) => {
  await open(page, { failOlder: true }); await trigger(page).click(); await expect(rows(page)).toHaveCount(20);
  await history(page).getByRole('button', { name: 'Older readings', exact: true }).click();
  await expect(history(page).getByRole('alert')).toContainText('This activity history page could not be loaded.'); await expect(rows(page)).toHaveCount(0);
  await history(page).getByRole('button', { name: 'Retry activity history', exact: true }).click(); await expect(history(page).getByRole('alert')).toBeVisible();
  await history(page).getByRole('button', { name: 'Back to latest readings', exact: true }).click(); await expect(rows(page)).toHaveCount(20); await expect(history(page)).toContainText('Page 1');
  await page.evaluate(() => { (window as unknown as FixtureWindow).activityHistoryFixture.failOlder = false; });
  await history(page).getByRole('button', { name: 'Older readings', exact: true }).click(); await expect(rows(page)).toHaveCount(2);
  await trigger(page).click(); await trigger(page).click(); await expect(rows(page)).toHaveCount(20); await expect(history(page)).toContainText('Page 1');
});

test('closing and switching profiles fence late history replies', async ({ page }) => {
  await open(page, { deferFirst: true }); await trigger(page).click(); await expect(history(page).getByRole('status')).toHaveText('Loading saved activity history…');
  await trigger(page).click(); await choose(page, 'History account 2'); await expect(trigger(page)).toHaveAttribute('aria-expanded', 'false');
  await trigger(page).click(); await expect(rows(page)).toHaveCount(1); await expect(rows(page).first()).toHaveAttribute('data-activity-history-run', uuid(200));
  await page.evaluate(value => { for (const pending of (window as unknown as FixtureWindow).activityHistoryFixture.pending.splice(0)) pending.resolve(value); }, { profileId: first, entries: [entry(first, 100)], nextBeforeRunId: null });
  await expect(history(page)).toHaveAttribute('data-activity-history', second); await expect(rows(page).first()).toHaveAttribute('data-activity-history-run', uuid(200));
});

test('a reading revision invalidates pending pages and starts again at the latest summaries', async ({ page }) => {
  await open(page, { deferFirst: true }); await trigger(page).click();
  await expect.poll(() => page.evaluate(() => (window as unknown as FixtureWindow).activityHistoryFixture.pending.length)).toBe(1);
  // A changed Rust reading revision is the renderer invalidation boundary, not a simulated archive restore.
  await page.evaluate(value => { (window as unknown as FixtureWindow).activityHistoryFixture.readingRevision = value; }, uuid(300));
  await expect.poll(() => page.evaluate(() => (window as unknown as FixtureWindow).activityHistoryFixture.pending.length)).toBe(2);
  await page.evaluate(value => { (window as unknown as FixtureWindow).activityHistoryFixture.pending[0]!.resolve(value); }, { profileId: first, entries: [entry(first, 100)], nextBeforeRunId: null });
  await expect(rows(page)).toHaveCount(0);
  await page.evaluate(value => { (window as unknown as FixtureWindow).activityHistoryFixture.pending[1]!.resolve(value); }, { profileId: first, entries: [entry(first, 201)], nextBeforeRunId: null });
  await expect(rows(page).first()).toHaveAttribute('data-activity-history-run', uuid(201)); await expect(history(page)).toContainText('Page 1');
  expect((await calls(page)).every(call => call.args.beforeRunId === null)).toBe(true);
});

test('history rejects malformed and cross-profile responses and renders a real empty state', async ({ page }) => {
  await open(page); const valid = entry(first, 100);
  const malformed: unknown[] = [
    { profileId: second, entries: [entry(second, 200)], nextBeforeRunId: null },
    { profileId: first, entries: [{ ...valid, profileId: second }], nextBeforeRunId: null },
    { profileId: first, entries: [{ ...valid, summary: { ...valid.summary, lifetimeTokens: 9007199254740993 } }], nextBeforeRunId: null },
    { profileId: first, entries: [{ ...valid, dailyUsageBuckets: [] }], nextBeforeRunId: null },
    { profileId: first, entries: [{ ...valid, dailyBucketsState: 'zero' }], nextBeforeRunId: null },
  ];
  await page.evaluate(value => { (window as unknown as FixtureWindow).activityHistoryFixture.override = value; }, malformed[0]); await trigger(page).click();
  for (const value of malformed) {
    await page.evaluate(value => { (window as unknown as FixtureWindow).activityHistoryFixture.override = value; }, value);
    await history(page).getByRole('button', { name: 'Retry activity history', exact: true }).click(); await expect(history(page).getByRole('alert')).toContainText('Saved activity history could not be loaded.'); await expect(rows(page)).toHaveCount(0);
  }
  const empty: ActivityHistoryPage = { profileId: first, entries: [], nextBeforeRunId: null };
  await page.evaluate(value => { (window as unknown as FixtureWindow).activityHistoryFixture.override = value; }, empty);
  await history(page).getByRole('button', { name: 'Retry activity history', exact: true }).click(); await expect(history(page)).toContainText('No accepted activity summaries are saved for this account.'); await expect(history(page).getByRole('alert')).toHaveCount(0);
});

test('saved summaries remain readable and keyboard accessible in both themes at 360 pixels and 200% text', async ({ page }) => {
  await open(page); await trigger(page).focus(); await page.keyboard.press('Enter'); await expect(rows(page)).toHaveCount(20);
  await rows(page).first().getByRole('button').first().focus(); await page.keyboard.press('Enter');
  await rows(page).first().getByRole('button', { name: 'Source details', exact: true }).focus(); await page.keyboard.press('Enter');
  await page.setViewportSize({ width: 360, height: 800 });
  for (const theme of ['light', 'dark']) {
    await page.getByRole('combobox', { name: 'Appearance', exact: true }).click(); await page.getByRole('option', { name: theme === 'light' ? 'Light' : 'Dark', exact: true }).click();
    for (const scale of [1, 2]) {
      await page.evaluate(scale => { for (const node of document.querySelectorAll<HTMLElement>('body *')) { const base = node.dataset.activityHistoryTestSize ?? getComputedStyle(node).fontSize; node.dataset.activityHistoryTestSize = base; node.style.fontSize = `${parseFloat(base) * scale}px`; } }, scale);
      expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth + 1)).toBe(true);
      await expect(rows(page).first().locator('[data-activity-history-metric="peakDailyTokens"]')).toHaveText('9223372036854775807');
      expect((await new AxeBuilder({ page }).include('[data-activity-history]').withTags(['wcag2a', 'wcag2aa', 'wcag21aa']).analyze()).violations).toEqual([]);
    }
  }
});
