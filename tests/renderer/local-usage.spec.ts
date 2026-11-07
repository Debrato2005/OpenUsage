import { expect, test, type Page } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';
import { navigate } from './navigation';
import { localUsage } from './local-usage-fixture';

// Synthetic renderer IPC only. Native tests must establish saved totals and scans.
type Usage = ReturnType<typeof localUsage>;
type FixtureWindow = Window & { usageCalls: Record<string, unknown>[] };
const ready = { storage: 'ready', theme: 'light', schemaVersion: 10, activeProfiles: 0, monitoring: 'stopped', provider: 'not_configured', keyProtection: 'os_store', storageIssue: null };

async function open(page: Page, usage: Usage | Record<string, unknown>) {
  await page.addInitScript(({ boot, usage }) => {
    const calls: Record<string, unknown>[] = []; Object.assign(window, { usageCalls: calls });
    window.__TAURI__ = { core: { invoke: async <T>(command: string, args: Record<string, unknown> = {}) => {
      if (command === 'load_time_zone') return { timeZone: 'UTC' } as T;
      if (command === 'bootstrap') return boot as T;
      if (command === 'get_display_scale') return { percent: 100 } as T;
      if (command === 'list_profiles') return { activeProfiles: 0, profiles: [] } as T;
      if (command === 'load_monitoring') return { state: 'stopped', readingRevision: '11111111-1111-4111-8111-111111111111', profiles: [] } as T;
      if (command === 'load_local_logs') return { consentRequired: false, tools: [{ tool: 'claude', detected: true, granted: true }, { tool: 'codex', detected: true, granted: true }], scan: { state: 'idle', lastScanAtUtc: null } } as T;
      if (command === 'load_local_usage') { calls.push(args); return usage as T; }
      throw new Error(`Unexpected synthetic command ${command}`);
    } } };
  }, { boot: ready, usage });
  await page.goto('/');
}
const usageCalls = (page: Page) => page.evaluate(() => (window as unknown as FixtureWindow).usageCalls);
const device = (page: Page) => page.locator('[data-local-usage]');

test('first launch with logs and no account shows This device on Overview and opens it in Usage', async ({ page }) => {
  const usage = localUsage();
  await open(page, usage);
  const section = page.getByRole('region', { name: 'This device', exact: true });
  await expect(section).toBeVisible();
  await expect(section.getByText('Last 30 days', { exact: true })).toBeVisible();
  const claude = section.locator('[data-local-tool="claude"]');
  await expect(claude.getByRole('button', { name: `Claude Code: ${Number(usage.tools[0]!.totalTokens).toLocaleString('en-US')} tokens`, exact: true })).toBeVisible();
  // An estimate is never spend, and an unpriced model makes it Incomplete (OU-LOG-013, OU-LOG-014).
  await expect(claude.locator('.local-tool-context')).toHaveText('Estimated $184.32Incomplete');
  await expect(section.locator('[data-local-tool="codex"] .local-tool-context')).toHaveText('Estimated $61.88');
  await expect(page.getByRole('heading', { name: 'No accounts', exact: true })).toBeVisible();
  expect((await new AxeBuilder({ page }).withTags(['wcag2a', 'wcag2aa', 'wcag21aa']).analyze()).violations).toEqual([]);
  await section.getByRole('button', { name: 'Open in Usage', exact: true }).click();
  await expect(page.getByRole('group', { name: 'Source', exact: true }).getByRole('button', { name: 'This device', exact: true })).toHaveAttribute('aria-pressed', 'true');
  await expect(device(page).locator('[data-local-tool="claude"]')).toBeVisible();
  expect(await usageCalls(page)).toContainEqual({ days: 30, period: 'day' });
});

test('Usage ranges and grouping ask Rust for new totals, and Provider returns to accounts', async ({ page }) => {
  await open(page, localUsage());
  await navigate(page, 'usage');
  const source = page.getByRole('group', { name: 'Source', exact: true });
  // With no account the scope starts at This device.
  await expect(source.getByRole('button', { name: 'This device', exact: true })).toHaveAttribute('aria-pressed', 'true');
  await device(page).getByRole('group', { name: 'Dates', exact: true }).getByRole('button', { name: '7 days', exact: true }).click();
  await device(page).getByRole('group', { name: 'Grouping', exact: true }).getByRole('button', { name: 'Weekly', exact: true }).click();
  await expect.poll(async () => (await usageCalls(page)).at(-1)).toEqual({ days: 7, period: 'week' });
  await expect(device(page).locator('[data-local-bars="week"]')).toBeVisible();
  await expect(device(page).getByRole('table').filter({ hasText: 'Model' }).getByRole('row')).toHaveCount(5);
  await expect(device(page).getByRole('row', { name: /totally-unknown-model-x/ })).toContainText('Not available');
  await source.getByRole('button', { name: 'Provider', exact: true }).click();
  await expect(device(page)).toHaveCount(0);
  await expect(page.getByRole('heading', { name: 'No saved activity yet', exact: true })).toBeVisible();
});

test('the detail panel shows exact counts, the estimate as not money, and saved differences', async ({ page }) => {
  const usage = localUsage();
  await open(page, usage);
  await navigate(page, 'usage');
  await device(page).locator('[data-local-tool="claude"] [data-local-tool-total]').click();
  const panel = page.locator('[data-detail-panel][open]');
  await expect(panel).toContainText(`Total${Number(usage.tools[0]!.totalTokens).toLocaleString('en-US')}`);
  await expect(panel).toContainText('Estimated value$184.32 at public API prices, not money paid. Some models have no price.');
  await expect(panel).toContainText('read by ccusage 20.0.48');
  await expect(panel).toContainText('days in Asia/Kolkata');
  await expect(panel).toContainText('2026-10-20, claude-sonnet-4-5-20250929: saved 5,120,000, a later scan read 0');
});

test('personal records open the detail panel with exact values', async ({ page }) => {
  await open(page, localUsage());
  await navigate(page, 'usage');
  const records = device(page).getByRole('region', { name: 'Records', exact: true });
  await expect(records.getByRole('button', { name: 'Active days: 26', exact: true })).toContainText('26');
  await expect(records.getByRole('button', { name: 'Longest streak: 6 days', exact: true })).toContainText('6 days');
  const busiest = records.getByRole('button', { name: 'Busiest day: 2026-10-10, 6,912,345 tokens', exact: true });
  await expect(busiest).toContainText('Oct 10');
  await busiest.click();
  await expect(page.locator('[data-detail-panel][open]')).toContainText('2026-10-10, 6,912,345 tokens');
});

test('no saved local usage hides the Overview section and Usage shows one empty state', async ({ page }) => {
  await open(page, { ...localUsage(), tools: [], models: [], buckets: [], differences: [], activeDays: 0, longestStreakDays: 0, busiestDay: null });
  await expect(page.getByRole('heading', { name: 'No accounts', exact: true })).toBeVisible();
  await expect(page.locator('[data-local-overview]')).toHaveCount(0);
  await navigate(page, 'usage');
  await expect(device(page).getByRole('heading', { name: 'No saved local usage for these dates', exact: true })).toBeVisible();
});

test('a response whose parts do not add up is refused and shows an error, not numbers', async ({ page }) => {
  const usage = localUsage();
  await open(page, { ...usage, tools: [{ ...usage.tools[0]!, totalTokens: '1' }] });
  await navigate(page, 'usage');
  await expect(device(page).getByRole('alert')).toHaveText('OpenUsage could not read saved local usage. Try again.');
  await expect(device(page).locator('[data-local-tool]')).toHaveCount(0);
  await expect(page.locator('[data-local-overview]')).toHaveCount(0);
});
