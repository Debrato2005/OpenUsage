import { expect, test, type Page } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';
import { navigate } from './navigation';
import { localUsage } from './local-usage-fixture';

// Synthetic renderer IPC only. Native tests must establish saved totals and scans.
type Usage = ReturnType<typeof localUsage>;
type FixtureWindow = Window & { usageCalls: Record<string, unknown>[] };
const ready = { storage: 'ready', theme: 'light', schemaVersion: 10, activeProfiles: 0, monitoring: 'stopped', provider: 'not_configured', keyProtection: 'os_store', storageIssue: null };

async function open(page: Page, usage: Usage | Record<string, unknown>, scopes: Record<string, unknown> = {}) {
  await page.addInitScript(({ boot, usage, scopes }) => {
    const calls: Record<string, unknown>[] = []; Object.assign(window, { usageCalls: calls });
    window.__TAURI__ = { core: { invoke: async <T>(command: string, args: Record<string, unknown> = {}) => {
      if (command === 'load_time_zone') return { timeZone: 'UTC' } as T;
      if (command === 'bootstrap') return boot as T;
      if (command === 'get_display_scale') return { percent: 100 } as T;
      if (command === 'list_profiles') return { activeProfiles: 0, profiles: [] } as T;
      if (command === 'load_monitoring') return { state: 'stopped', readingRevision: '11111111-1111-4111-8111-111111111111', profiles: [] } as T;
      if (command === 'load_local_logs') return { consentRequired: false, tools: [{ tool: 'claude', detected: true, granted: true }, { tool: 'codex', detected: true, granted: true }], scan: { state: 'idle', lastScanAtUtc: null } } as T;
      if (command === 'load_local_usage') { calls.push(args); return (scopes[String(args.scope)] ?? usage) as T; }
      throw new Error(`Unexpected synthetic command ${command}`);
    } } };
  }, { boot: ready, usage, scopes });
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

test('the hourly view covers 7 days, shows local hours, and marks a tool with no hours as Not available', async ({ page }) => {
  const usage = localUsage(7);
  usage.startUtc = '2026-11-01T18:30:00.000Z'; usage.endUtc = '2026-11-08T18:30:00.000Z';
  usage.hours = [{ startUtc: '2026-11-07T04:00:00.000Z', tool: 'claude', totalTokens: '125000' }];
  Object.assign(usage, { activeDays: 6, longestStreakDays: 6, busiestDay: { date: '2026-11-07', totalTokens: '6912345' } });
  await open(page, usage);
  await navigate(page, 'usage');
  const grouping = device(page).getByRole('group', { name: 'Grouping', exact: true });
  // 30 days is too long for hours.
  await expect(grouping.getByRole('button', { name: 'Hourly', exact: true })).toBeDisabled();
  await device(page).getByRole('group', { name: 'Dates', exact: true }).getByRole('button', { name: '7 days', exact: true }).click();
  await grouping.getByRole('button', { name: 'Hourly', exact: true }).click();
  await expect.poll(async () => (await usageCalls(page)).at(-1)).toEqual({ days: 7, period: 'hour' });
  const chart = device(page).locator('[data-local-bars="hour"]');
  await expect(chart.getByRole('img')).toHaveAttribute('aria-label', 'Tokens per hour by tool, 2026-11-02 to 2026-11-08');
  // 168 hours that start at :30 local time in Asia/Kolkata, and one with saved usage.
  await expect(chart.locator('svg > g')).toHaveCount(5 + 168);
  await expect(chart.locator('table tbody tr')).toHaveText(['Nov 7, 9:30 AM125,000']);
  await expect(chart.locator('[data-local-no-hours="codex"]')).toHaveText('CodexNot available');
  expect((await new AxeBuilder({ page }).withTags(['wcag2a', 'wcag2aa', 'wcag21aa']).analyze()).violations).toEqual([]);
  // A longer range falls back to days.
  await device(page).getByRole('group', { name: 'Dates', exact: true }).getByRole('button', { name: '90 days', exact: true }).click();
  await expect.poll(async () => (await usageCalls(page)).at(-1)).toEqual({ days: 90, period: 'day' });
  await expect(grouping.getByRole('button', { name: 'Daily', exact: true })).toHaveAttribute('aria-pressed', 'true');
});

test('a custom range asks Rust for exact dates and an invalid range asks nothing', async ({ page }) => {
  await open(page, localUsage());
  await navigate(page, 'usage');
  const dates = device(page).getByRole('group', { name: 'Dates', exact: true });
  await dates.getByRole('button', { name: 'Custom', exact: true }).click();
  await expect(dates.getByRole('button', { name: 'Custom', exact: true })).toHaveAttribute('aria-pressed', 'true');
  const custom = device(page).getByRole('group', { name: 'Custom dates', exact: true });
  // Custom starts from the shown dates, then follows each change.
  await expect(custom.getByLabel('From')).toHaveValue('2026-10-10');
  await expect(custom.getByLabel('To')).toHaveValue('2026-11-08');
  await custom.getByLabel('From').fill('2026-01-05');
  await expect.poll(async () => (await usageCalls(page)).at(-1)).toEqual({ since: '2026-01-05', until: '2026-11-08', period: 'day' });
  const before = (await usageCalls(page)).length;
  await custom.getByLabel('From').fill('2025-11-01');
  await expect(custom.getByRole('alert')).toHaveText('Choose a start and an end at most 366 days apart.');
  await custom.getByLabel('To').fill('2025-10-01');
  await page.waitForTimeout(300);
  expect((await usageCalls(page)).slice(before).filter(call => 'since' in call && call.since === '2025-11-01')).toEqual([]);
  expect((await new AxeBuilder({ page }).withTags(['wcag2a', 'wcag2aa', 'wcag21aa']).analyze()).violations).toEqual([]);
  await dates.getByRole('button', { name: '90 days', exact: true }).click();
  await expect.poll(async () => (await usageCalls(page)).at(-1)).toEqual({ days: 90, period: 'day' });
  await expect(custom).toHaveCount(0);
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

test('projects and sessions show exact counts, and a count that a tool does not report stays missing', async ({ page }) => {
  await open(page, localUsage());
  await navigate(page, 'usage');
  const projects = device(page).getByRole('region', { name: 'Projects', exact: true });
  await expect(projects.getByRole('row')).toHaveCount(3);
  await expect(projects.getByRole('row', { name: /-home-dev-alpha/ })).toHaveText('-home-dev-alphaClaude Code13,400,000');
  const sessions = device(page).getByRole('region', { name: 'Sessions', exact: true });
  await expect(sessions.getByRole('row')).toHaveCount(4);
  // Times show in the pinned zone. Codex has no project or start, and Claude Code has no reasoning count.
  await expect(sessions.locator('[data-local-session="codex"]')).toHaveText('Nov 8, 9:30 AMCodexNot availableNot available1,250,00048,000');
  await expect(sessions.locator('[data-local-session="claude"]').first()).toHaveText('Nov 7, 5:15 PMClaude Code-home-dev-alphaNov 7, 3:00 PM3,400,000Not available');
  expect((await new AxeBuilder({ page }).withTags(['wcag2a', 'wcag2aa', 'wcag21aa']).analyze()).violations).toEqual([]);
});

test('a session whose reasoning exceeds its total is refused', async ({ page }) => {
  const usage = localUsage();
  usage.sessions[0]!.reasoningTokens = '1250001';
  await open(page, usage);
  await navigate(page, 'usage');
  await expect(device(page).getByRole('alert')).toHaveText('OpenUsage could not read saved local usage. Try again.');
  await expect(device(page).getByRole('region', { name: 'Sessions', exact: true })).toHaveCount(0);
});

test('no saved local usage hides the Overview section and Usage shows one empty state', async ({ page }) => {
  await open(page, { ...localUsage(), tools: [], models: [], buckets: [], differences: [], sessions: [], projects: [], activeDays: 0, longestStreakDays: 0, busiestDay: null });
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

test('undated totals and missing details stay outside date charts and share images', async ({ page }) => {
  const duplicateKeys: string[] = [];
  page.on('console', message => { if (message.text().includes('Encountered two children with the same key')) duplicateKeys.push(message.text()); });
  const base = localUsage();
  const scopes = [...base.scopes, { id: 'unplaced:opencode', kind: 'unplaced', tool: 'opencode', timeZone: null, folder: null, totalTokens: '173' }];
  const undated = { ...base, scope: 'unplaced:opencode', scopes, dated: false, timeZone: 'UTC',
    tools: [{ tool: 'opencode', inputTokens: null, outputTokens: null, cacheWriteTokens: null, cacheReadTokens: null, reasoningTokens: null, totalTokens: '173', estimatedValueUsdMicros: null, estimateComplete: false, logsGone: false }],
    models: [{ tool: 'opencode', model: 'm', totalTokens: null, estimatedValueUsdMicros: null }],
    buckets: [], differences: [100, 80].map(saved => ({ date: null, model: null, tool: 'opencode', savedTotalTokens: String(saved), newTotalTokens: '0', scanAtUtc: '2026-11-08T12:00:00.000Z' })), activeDays: null, longestStreakDays: null, busiestDay: null, projects: [], startUtc: null, endUtc: null, hours: [], hourTools: [],
    sessions: [{ key: 'synthetic', tool: 'opencode', project: null, startUtc: null, endUtc: null, totalTokens: '999', reasoningTokens: null, estimatedValueUsdMicros: null }],
  };
  await open(page, { ...base, scopes }, { 'unplaced:opencode': undated });
  await navigate(page, 'usage');
  await device(page).getByRole('combobox', { name: 'Saved history' }).click();
  await page.getByRole('option', { name: 'OpenCode · Undated', exact: true }).click();
  await expect(device(page).locator('[data-local-tool-total="173"]')).toBeVisible();
  await expect(device(page).locator('[data-local-bars], [data-local-records], [data-share-open]')).toHaveCount(0);
  await expect(device(page).getByRole('group', { name: 'Dates', exact: true })).toHaveCount(0);
  await expect(device(page).getByRole('group', { name: 'Grouping', exact: true })).toHaveCount(0);
  await expect(device(page).locator('[data-local-sessions] time')).toHaveCount(0);
  await expect(device(page).locator('[data-local-session]')).toContainText('Not available');
  await expect(device(page).getByRole('row', { name: /^m / })).toContainText('Not availableNot available');
  await device(page).locator('[data-local-tool-total]').click();
  await expect(page.locator('[data-detail-panel][open]')).toContainText('InputNot available');
  await expect(page.locator('[data-detail-panel][open]')).toContainText('Total173');
  await expect(page.locator('[data-detail-panel][open] .local-differences li')).toHaveCount(2);
  expect(duplicateKeys).toEqual([]);
  expect((await new AxeBuilder({ page }).withTags(['wcag2a', 'wcag2aa', 'wcag21aa']).analyze()).violations).toEqual([]);
});

test('ambiguous history exposes each folder in the detail panel without a combined number', async ({ page }) => {
  const base = localUsage();
  const ids = ['aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa', 'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb'];
  const scopes = [...base.scopes, ...ids.map((id, index) => ({ id: `legacy:${id}`, kind: 'legacy', tool: 'claude', timeZone: index ? 'Asia/Kolkata' : 'UTC', folder: `/synthetic/folder-${index + 1}`, totalTokens: '100' }))];
  const unavailable = { ...base, scopes, available: false, tools: [], models: [], buckets: [], sessions: [], projects: [], differences: [], hours: [], activeDays: null, longestStreakDays: null, busiestDay: null };
  const selected = { ...base, scopes, scope: scopes[1]!.id, tools: [{ ...base.tools[0]!, inputTokens: '100', outputTokens: '0', cacheWriteTokens: '0', cacheReadTokens: '0', totalTokens: '100' }], models: [], buckets: [], differences: [] };
  await open(page, unavailable, { [selected.scope]: selected });
  await navigate(page, 'usage');
  await device(page).getByRole('button', { name: 'Combined history: Not available', exact: true }).click();
  const panel = page.locator('[data-detail-panel][open]');
  await expect(panel).toContainText('/synthetic/folder-1'); await expect(panel).toContainText('/synthetic/folder-2');
  await expect(panel).toContainText('2026-10-10 to 2026-11-08 · Asia/Kolkata');
  await page.keyboard.press('Escape');
  await expect(device(page).locator('[data-share-open], [data-local-bars]')).toHaveCount(0);
  await device(page).getByRole('combobox', { name: 'Saved history' }).click();
  await page.getByRole('option', { name: 'Claude Code · Folder 1', exact: true }).click();
  await expect(device(page).locator('[data-local-tool-total="100"]')).toBeVisible();
  expect((await usageCalls(page)).at(-1)).toEqual({ days: 30, period: 'day', scope: selected.scope });
});

test('a saved zero day has a value while an absent tool-day remains unavailable', async ({ page }) => {
  const base = localUsage();
  await open(page, { ...base, buckets: [{ startDate: '2026-11-08', tool: 'claude', totalTokens: '0' }] });
  await navigate(page, 'usage');
  const chart = device(page).locator('[data-local-bars]');
  await expect(chart.locator('title').filter({ hasText: /^2026-11-08:/ })).toHaveText('2026-11-08: 0 tokens');
  await expect(chart.locator('table tbody tr')).toHaveText('2026-11-080Not available');
});
