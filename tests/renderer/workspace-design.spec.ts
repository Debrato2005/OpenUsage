import { expect, test, type Page } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';
import type { ProfileRecord, SavedProfileActivity, SavedProfileHeatmap, SavedProfileQuota, SavedQuotaWindow } from '../../apps/desktop/src/contracts';
import { closeDetail, openAccount, openReadingDetails, selectedCard } from './navigation';

// Synthetic renderer fixtures. Native rendering and provider sessions are separate acceptance checks.
const ids = ['11111111-1111-4111-8111-111111111111', '22222222-2222-4222-8222-222222222222', '33333333-3333-4333-8333-333333333333', '44444444-4444-4444-8444-444444444444'];
const observedAtUtc = '2026-10-20T00:00:00.000Z';
const bindingId = '55555555-5555-4555-8555-555555555555', collectionRunId = '66666666-6666-4666-8666-666666666666';
const windowOf = (usedPercent: number, windowDurationMins: number, label: string, resetsAtUnix: number | null = null): SavedQuotaWindow => ({ usedPercent, derivedRemainingPercent: 100 - usedPercent, windowDurationMins, resetsAtUnix, label, resetState: 'expected' });
function quota(profileId: string, primary: SavedQuotaWindow | null, secondary: SavedQuotaWindow | null = null, overrides: Partial<SavedProfileQuota> = {}): SavedProfileQuota {
  return { profileId, bindingId, collectionRunId, providerVersion: '0.163.0', sourceKind: 'test_fixture', observedAtUtc, identityConfidence: 'provisional', verifiedIdentityKey: null, monitoring: 'stopped', accepted: true, coverage: 'unknown', scope: 'account_quota_with_unknown_application_scope', freshness: 'fresh', availableResetCredits: null, resetCreditDetails: null, limits: [{ limitId: 'codex', limitName: 'Codex', reachedType: null, monthlyCreditLimit: null, spendControlReached: null, primary, secondary }], ...overrides };
}
function activity(profileId: string, summary: Partial<SavedProfileActivity['summary']> = {}): SavedProfileActivity {
  return { profileId, bindingId, collectionRunId, providerVersion: '0.163.0', sourceKind: 'test_fixture', observedAtUtc, identityConfidence: 'provisional', verifiedIdentityKey: null, monitoring: 'stopped', accepted: true, coverage: 'unknown', scope: 'account_activity_with_unknown_application_scope', freshness: 'fresh', summary: { lifetimeTokens: '1000', peakDailyTokens: '500', currentStreakDays: '1', longestStreakDays: '2', longestRunningTurnSec: null, ...summary }, dailyState: 'unavailable', dailyUsageBuckets: [], dailyTruncated: false, dailyChart: { maxTokens: null, bars: [] } };
}
const label = (index: number) => ['Alpha', 'Bravo', 'Charlie', 'Delta'][index]!;
const profile = (index: number, connectionState: ProfileRecord['connectionState'] = 'connected'): ProfileRecord => ({ id: ids[index]!, label: label(index), environment: 'native', archived: false, connectionState });

type Scenario = { profiles?: ProfileRecord[]; quotas?: Record<string, SavedProfileQuota | null>; activity?: SavedProfileActivity | null; heatmap?: SavedProfileHeatmap | null; rejectQuota?: string[]; mismatchQuota?: string[] };
const standard: Scenario = {
  profiles: [profile(0), profile(1), profile(2, 'authentication_required'), profile(3, 'unauthorized')],
  quotas: {
    [ids[0]!]: quota(ids[0]!, windowOf(40, 300, 'Five-hour'), windowOf(20, 10080, 'Seven-day')),
    [ids[1]!]: quota(ids[1]!, windowOf(92, 300, 'Five-hour'), windowOf(30, 10080, 'Seven-day')),
    // A signed-out account keeps a saved reading, which must not lead the live summary.
    [ids[2]!]: quota(ids[2]!, windowOf(99, 300, 'Five-hour')),
    [ids[3]!]: null,
  },
};

async function open(page: Page, scenario: Scenario = standard) {
  await page.addInitScript(({ scenario, ids }) => {
    const profiles = scenario.profiles ?? [];
    const rejected = scenario.rejectQuota ?? [], mismatched = scenario.mismatchQuota ?? [];
    const calls: string[] = [];
    Object.assign(window, { designCalls: calls });
    const boot = () => ({ storage: 'ready', theme: 'light', schemaVersion: 9, activeProfiles: profiles.length, monitoring: 'stopped', provider: 'not_configured', keyProtection: 'passphrase', storageIssue: null });
    window.__TAURI__ = { core: { invoke: async <T>(command: string, args: Record<string, unknown> = {}) => {
      calls.push(command);
      const id = String(args.profileId);
      const answers: Record<string, unknown> = {
        bootstrap: boot(), get_display_scale: { percent: 100 }, load_time_zone: { timeZone: 'UTC' },
        list_profiles: { activeProfiles: profiles.length, profiles },
        load_monitoring: { state: 'stopped', readingRevision: 'cccccccc-cccc-4ccc-8ccc-cccccccccccc', profiles: [] },
        load_profile_refresh_status: { profileId: id, retryAfterSeconds: 0 },
        load_profile_plan: { profileId: id, bindingId: args.bindingId, plan: 'plus' },
        list_subscriptions: { profileId: id, records: [], nextCursor: null, currencies: [{ code: 'USD', scale: 2 }] },
        load_alert_history: { events: [], truncated: false },
        load_notification_preferences: { reset: { enabled: false, timeZone: 'UTC', quietEnabled: true, quietStart: '22:00', quietEnd: '08:00', profileOverrides: [] }, usage: { enabled: false, profileOverrides: [] }, authentication: { enabled: false, profileOverrides: [] }, collectionFailure: { enabled: false, profileOverrides: [] } },
      };
      if (command === 'load_profile_quota') {
        if (rejected.includes(id)) throw 'provider_quota_load_failed';
        const reading = scenario.quotas?.[mismatched.includes(id) ? ids[0]! : id] ?? null;
        return reading as T;
      }
      if (command === 'load_profile_heatmap') return (scenario.heatmap && scenario.heatmap.profileId === id && scenario.heatmap.mode === args.mode ? scenario.heatmap : null) as T;
      if (command === 'load_profile_activity') return (scenario.activity && scenario.activity.profileId === id ? scenario.activity : null) as T;
      if (command === 'load_activity_dashboard') return { view: args.view, status: 'unavailable', mode: args.mode, periodStart: args.startDate, periodEnd: args.endDate, sourceKind: 'unverified', observedAtUtc: null, scopeVerified: false, totalTokens: null, knownContributions: 0, expectedContributions: 0, scaleMaximum: null, members: [], exclusions: (args.profileIds as string[]).map(profileId => ({ profileId, reason: 'activity_unavailable' })), cells: [] } as T;
      if (command in answers) return answers[command] as T;
      throw new Error(`Unexpected design fixture command: ${command}`);
    } } };
  }, { scenario, ids });
  await page.goto('/');
  await expect(page.getByRole('region', { name: 'Usage overview', exact: true })).toBeVisible();
}
const glance = (page: Page) => page.locator('[data-overview-glance]');
const rows = (page: Page) => page.locator('[data-overview-account]');
const detail = (page: Page) => page.locator('[data-detail-panel][open]');
async function chooseFilter(page: Page, name: string) {
  await page.getByRole('combobox', { name: 'Filter accounts', exact: true }).click();
  await page.getByRole('option', { name, exact: true }).click();
}

test('tabular figures stay on digit runs so spaces and punctuation keep their normal width', async ({ page }) => {
  await open(page);
  const measured = await page.evaluate(async () => {
    await document.fonts.ready;
    const probe = document.createElement('span');
    probe.style.cssText = 'position:absolute;visibility:hidden;white-space:pre;font-size:100px';
    document.querySelector('main')!.append(probe);
    const width = (text: string) => { probe.textContent = text; return probe.getBoundingClientRect().width; };
    const space = width('a b') - width('ab');
    probe.remove();
    return { space, root: getComputedStyle(document.documentElement).fontVariantNumeric, value: getComputedStyle(document.querySelector('.overview-glance-value')!).fontVariantNumeric, row: getComputedStyle(document.querySelector('.overview-window-left')!).fontVariantNumeric };
  });
  expect(measured.root).toBe('normal');
  expect(measured.value).toBe('normal');
  expect(measured.row).toBe('normal');
  // DM Sans uses about 0.19em for a space. Its tnum feature widens that to about 0.6em.
  expect(measured.space).toBeLessThan(35);
});

test('summary keeps only names and values visible and opens one keyboard detail panel', async ({ page }) => {
  await open(page);
  const card = (id: string) => glance(page).locator(`[data-overview-summary="${id}"]`);
  const account = card('accounts').getByRole('button');
  await expect(account).toContainText('Connected accounts2 / 4');
  await expect(card('quota').getByRole('button')).toContainText('Runs out first8%Bravo, five-hour');
  await expect(card('quota')).toHaveAttribute('data-attention', 'low');
  await expect(detail(page)).toHaveCount(0);
  await account.focus();
  await page.keyboard.press('Enter');
  const accounts = page.getByRole('dialog', { name: 'Connected accounts details' });
  await expect(accounts).toContainText('Active accounts4');
  await expect(accounts).toContainText('Needs sign-in2');
  await expect(accounts).toContainText('3 ChatGPT Plus');
  await page.keyboard.press('Escape');
  await expect(detail(page)).toHaveCount(0);
  await expect(account).toBeFocused();
  await page.keyboard.press('Tab');
  await expect(card('streak').getByRole('button')).toBeFocused();
  const quota = card('quota').getByRole('button');
  await quota.focus();
  expect(await quota.evaluate(node => getComputedStyle(node).outlineStyle)).not.toBe('none');
  await page.keyboard.press('Space');
  const details = page.getByRole('dialog', { name: 'Runs out first details' });
  await expect(detail(page)).toHaveCount(1);
  await expect(details).toContainText('AccountBravo');
  await expect(details).toContainText('WindowFive-hour');
  await expect(details).toContainText('Calculation100% − 92% used');
  await expect(details).toContainText('Synthetic test data');
  await expect(details.locator('time')).toHaveAttribute('datetime', observedAtUtc);
  await expect(details).toContainText('ScopeAccount quota; application scope unknown');
  await expect(details).toContainText('CoverageUnknown');
  await expect(details).toContainText('IdentityProvisional');
  await page.keyboard.press('Escape');
  await expect(detail(page)).toHaveCount(0);
  await expect(quota).toBeFocused();
  expect(await page.evaluate(() => (window as unknown as { designCalls: string[] }).designCalls.filter(command => /^(begin_|collect_|start_)/.test(command)))).toEqual([]);
});

test('summary shows Not available instead of inventing a value without saved readings', async ({ page }) => {
  await open(page, { profiles: [profile(0), profile(1)], quotas: { [ids[0]!]: null, [ids[1]!]: null } });
  const card = (id: string) => glance(page).locator(`[data-overview-summary="${id}"]`).getByRole('button');
  await expect(card('quota')).toHaveText('Runs out firstNot available');
  await expect(card('best')).toHaveText('Most left nowNot available');
  await expect(card('reset')).toHaveText('Next resetNot available');
  await expect(glance(page)).not.toContainText('0%');
  await card('quota').click();
  await expect(detail(page)).toContainText('No saved quota windows for connected accounts');
});

for (const freshness of ['stale', 'unknown'] as const) test(`summary retains zero and a single ${freshness} mark with its evidence`, async ({ page }) => {
  await open(page, { profiles: [profile(0)], quotas: { [ids[0]!]: quota(ids[0]!, windowOf(100, 300, 'Five-hour'), null, { freshness }) } });
  const tile = glance(page).locator('[data-overview-summary="quota"]');
  await expect(tile.locator('.overview-glance-value')).toHaveText('0%');
  await expect(tile.locator('.mark')).toHaveText(freshness === 'stale' ? 'Stale' : 'Freshness unknown');
  await tile.getByRole('button').click();
  await expect(detail(page)).toContainText(`Freshness${freshness === 'stale' ? 'Stale' : 'Freshness unknown'}`);
});

test('summary marks partial account coverage without hiding stale reading evidence', async ({ page }) => {
  await open(page, { profiles: [profile(0), profile(1)], quotas: { [ids[0]!]: quota(ids[0]!, windowOf(40, 300, 'Five-hour'), null, { freshness: 'stale' }), [ids[1]!]: null } });
  const tile = glance(page).locator('[data-overview-summary="quota"]');
  await expect(tile.locator('.mark')).toHaveText('Incomplete');
  await tile.getByRole('button').click();
  await expect(detail(page)).toContainText('Accounts with saved quota1 of 2 connected');
  await expect(detail(page)).toContainText('FreshnessStale');
  await expect(detail(page)).toContainText('BravoNot available');
  await closeDetail(page);
  await glance(page).locator('[data-overview-summary="accounts"]').getByRole('button').click();
  await expect(detail(page)).toContainText('Latest readingAlpha');
  await expect(detail(page)).toContainText('FreshnessStale');
});

test('next expected reset exposes its account and never confirms a reset on elapsed time', async ({ page }) => {
  await page.clock.install({ time: new Date(observedAtUtc) });
  const soon = Date.parse(observedAtUtc) / 1000 + 30, later = soon + 86400;
  await open(page, { profiles: [profile(0), profile(1)], quotas: { [ids[0]!]: quota(ids[0]!, windowOf(10, 10080, 'Seven-day', later)), [ids[1]!]: quota(ids[1]!, windowOf(10, 300, 'Five-hour', soon)) } });
  const tile = glance(page).locator('[data-overview-summary="reset"]');
  await expect(tile.locator('.overview-glance-value')).toHaveText('In 1m');
  await tile.getByRole('button').click();
  await expect(detail(page)).toContainText('AccountBravo');
  await expect(detail(page)).toContainText('WindowFive-hour');
  await expect(detail(page)).toContainText('Time alone does not confirm a reset');
  await expect(tile.locator('[data-reset-countdown]')).toHaveCount(0);
  await page.clock.runFor(30_000);
  await expect(tile.locator('.overview-glance-value')).toHaveText('In 1d');
  await expect(detail(page)).toContainText('AccountAlpha');
  await expect(page.locator(`[data-overview-account="${ids[1]}"]`)).toContainText('90%');
  await openAccount(page, ids[1]!);
  await expect(page.locator('[data-overview-window] [data-reset-countdown]').first()).toBeVisible();
  await expect(page.locator('[data-overview-window] [data-reset-countdown]').first()).not.toHaveText('Reset confirmed');
});

for (const theme of ['light', 'dark'] as const) test(`summary detail reflows and stays accessible at 360 pixels in ${theme}`, async ({ page }) => {
  await open(page);
  await page.evaluate(theme => { document.documentElement.dataset.theme = theme; }, theme);
  await page.setViewportSize({ width: 360, height: 800 });
  await glance(page).locator('[data-overview-summary="quota"]').getByRole('button').click();
  await page.evaluate(() => {
    const sizes = [...document.querySelectorAll<HTMLElement>('body *')].map(node => ({ node, size: getComputedStyle(node).fontSize }));
    for (const { node, size } of sizes) node.style.fontSize = `${parseFloat(size) * 2}px`;
  });
  const layout = await page.evaluate(() => ({ width: innerWidth, scrollWidth: document.scrollingElement!.scrollWidth,
    overflow: [...document.querySelectorAll('body *')].filter(node => { const r = node.getBoundingClientRect(); return r.width > 0 && r.right > innerWidth + 1 && !node.closest('.overview-table-scroll'); }).map(node => ({ tag: node.tagName, className: node.className, width: node.getBoundingClientRect().width })).slice(0, 15) }));
  expect(layout.scrollWidth, JSON.stringify(layout)).toBeLessThanOrEqual(layout.width + 1);
  const bounds = await detail(page).boundingBox();
  expect(bounds!.x).toBeGreaterThanOrEqual(0);
  expect(bounds!.x + bounds!.width).toBeLessThanOrEqual(361);
  const audit = await new AxeBuilder({ page }).include('[data-detail-panel][open]').withTags(['wcag2a', 'wcag2aa', 'wcag21aa']).analyze();
  expect(audit.violations).toEqual([]);
});

test('search and filters narrow the account list and can be cleared', async ({ page }) => {
  await open(page);
  await expect(rows(page)).toHaveCount(4);
  const search = page.getByRole('searchbox', { name: 'Search accounts', exact: true });
  await search.fill('BRA');
  await expect(rows(page)).toHaveCount(1);
  await expect(rows(page).first()).toHaveAttribute('data-overview-account', ids[1]!);
  await search.fill('no such account');
  await expect(rows(page)).toHaveCount(0);
  await expect(page.getByRole('status').filter({ hasText: 'No accounts match.' })).toBeVisible();
  await page.getByRole('button', { name: 'Clear filters', exact: true }).click();
  await expect(rows(page)).toHaveCount(4);
  await expect(search).toHaveValue('');
  await chooseFilter(page, 'Needs sign-in');
  await expect(rows(page)).toHaveCount(2);
  await chooseFilter(page, 'Connected');
  await expect(rows(page)).toHaveCount(2);
  await chooseFilter(page, 'Low quota');
  await expect(rows(page)).toHaveCount(2);
  await expect(page.locator(`[data-overview-account="${ids[3]}"]`)).toHaveCount(0);
});

test('short account lists skip search until it would help', async ({ page }) => {
  await open(page, { profiles: [profile(0), profile(1)], quotas: {} });
  await expect(page.getByRole('searchbox', { name: 'Search accounts', exact: true })).toHaveCount(0);
});

test('choosing an account in the Overview chips selects its card without hijacking card controls', async ({ page }) => {
  await open(page);
  const overview = page.locator('[data-overview]');
  await expect(overview).toHaveAttribute('data-overview-profile', ids[0]!);
  await expect(detail(page)).toHaveCount(0);
  await page.locator(`[data-overview-hero] [data-activity-account="${ids[1]}"]`).click();
  await expect(overview).toHaveAttribute('data-overview-profile', ids[1]!);
  await expect(page.locator(`[data-overview-account="${ids[1]}"]`)).toHaveAttribute('data-selected', 'true');
  await expect(page.locator(`[data-overview-account-card="${ids[1]}"]`)).toContainText('Bravo');
  await expect(detail(page)).toHaveCount(0);
  await page.locator(`[data-overview-account="${ids[0]}"]`).getByRole('button', { name: 'Manage Alpha', exact: true }).click();
  await expect(page.locator('[data-account-panel]')).toBeVisible();
  await expect(detail(page)).toHaveCount(0);
  await expect(overview).toHaveAttribute('data-overview-profile', ids[1]!);
});

test('selected account shows longest task and reset credits, and keeps missing values distinct from zero', async ({ page }) => {
  const reading = quota(ids[0]!, windowOf(40, 300, 'Five-hour'), null, { availableResetCredits: '0' });
  await open(page, { profiles: [profile(0)], quotas: { [ids[0]!]: reading }, activity: activity(ids[0]!, { longestRunningTurnSec: '3725' }) });
  await openAccount(page, ids[0]!);
  await expect(selectedCard(page)).toContainText('Longest task1h 2m');
  await expect(selectedCard(page).locator('[data-overview-stat="credits"]')).toHaveCount(0);
  await expect(await openReadingDetails(page)).toContainText('Reset credits0');
});

test('a reset credit count that the provider did not report stays unknown', async ({ page }) => {
  await open(page, { profiles: [profile(0)], quotas: { [ids[0]!]: quota(ids[0]!, windowOf(40, 300, 'Five-hour')) }, activity: activity(ids[0]!) });
  await openAccount(page, ids[0]!);
  await expect(selectedCard(page)).toContainText('Longest taskNot available');
  await expect(await openReadingDetails(page)).toContainText('Reset creditsNot available');
});

test('an empty workspace offers one action instead of unavailable tiles', async ({ page }) => {
  await open(page, { profiles: [], quotas: {} });
  const welcome = page.locator('[data-overview-welcome]');
  await expect(welcome.getByRole('heading', { name: 'No accounts', exact: true })).toBeVisible();
  await expect(welcome.getByRole('button', { name: 'Add OpenAI account', exact: true })).toBeVisible();
  await expect(glance(page)).toHaveCount(0);
  await expect(page.locator('.overview-panels')).toHaveCount(0);
});

test('Settings groups every topic under a section index that moves keyboard focus', async ({ page }) => {
  await open(page);
  await page.locator('[data-workspace-nav="settings"]').click();
  const index = page.getByRole('navigation', { name: 'Settings sections', exact: true });
  await expect(index.getByRole('link')).toHaveText(['General', 'Accounts and plans', 'Alerts', 'Backups and exports', 'Connection', 'Privacy and data']);
  await index.getByRole('link', { name: 'Backups and exports', exact: true }).click();
  await expect(page.locator('#settings-data')).toBeFocused();
  await expect(page.locator('#settings-data')).toContainText('Encrypted backups');
  await expect(page.locator('#settings-privacy')).toContainText('Delete all local data');
  await expect(page.locator('#settings-connection [data-provider-settings]')).toBeVisible();
  await expect(page.locator('#settings-accounts [data-subscriptions]')).toBeVisible();
});

test('a control brought into view stays clear of the sticky sidebar', async ({ page }) => {
  await open(page);
  await page.locator('[data-workspace-nav="settings"]').click();
  await page.setViewportSize({ width: 1280, height: 600 });
  const covered = await page.evaluate(() => {
    const control = document.querySelector<HTMLElement>('[data-account-manager-open]')!;
    window.scrollTo({ top: document.documentElement.scrollHeight, behavior: 'instant' });
    control.scrollIntoView({ behavior: 'instant' });
    const bounds = control.getBoundingClientRect();
    return { left: bounds.left, top: bounds.top, header: document.querySelector('.workspace-header')!.getBoundingClientRect().right, hit: document.elementFromPoint(bounds.left + bounds.width / 2, bounds.top + bounds.height / 2)?.closest('button')?.textContent };
  });
  expect(covered.left).toBeGreaterThanOrEqual(covered.header);
  expect(covered.top).toBeGreaterThanOrEqual(-1);
  expect(covered.hit).toBe('Manage accounts');
});

for (const theme of ['light', 'dark'] as const) test(`Settings reflows at 360 pixels with no accessibility violations in ${theme} theme`, async ({ page }) => {
  await open(page);
  await page.locator('[data-workspace-nav="settings"]').click();
  await page.getByRole('combobox', { name: 'Appearance', exact: true }).click();
  await page.getByRole('option', { name: theme === 'light' ? 'Light' : 'Dark', exact: true }).click();
  await page.setViewportSize({ width: 360, height: 800 });
  expect(await page.evaluate(() => document.scrollingElement!.scrollWidth <= innerWidth + 1)).toBe(true);
  const audit = await new AxeBuilder({ page }).withTags(['wcag2a', 'wcag2aa', 'wcag21aa']).analyze();
  expect(audit.violations).toEqual([]);
});

test.describe('quota comparison', () => {
  const compare = (page: Page) => page.locator('[data-quota-compare]');
  const openCompare = async (page: Page, scenario: Scenario) => { await open(page, scenario); await page.locator('[data-workspace-nav="usage"]').click(); await expect(compare(page)).toBeVisible(); };

  test('aligns windows with the same limit and duration, and marks gaps as Not available', async ({ page }) => {
    await openCompare(page, standard);
    await expect(compare(page).getByRole('columnheader')).toHaveText(['Account', 'Codex · Five-hour', 'Codex · Seven-day']);
    await expect(page.locator(`[data-quota-compare-account="${ids[0]}"]`).getByRole('cell')).toHaveText([/60% left/, /80% left/]);
    await expect(page.locator(`[data-quota-compare-account="${ids[2]}"]`).getByRole('cell')).toHaveText([/1% left/, 'Not available']);
    await expect(page.locator(`[data-quota-compare-account="${ids[3]}"]`).getByRole('cell')).toHaveText(['Not available', 'Not available']);
    await expect(page.locator(`[data-quota-compare-account="${ids[1]}"]`).getByRole('meter').first()).toHaveAttribute('aria-valuenow', '8');
  });

  test('keeps different durations in separate columns even under one limit label', async ({ page }) => {
    await openCompare(page, { profiles: [profile(0), profile(1)], quotas: { [ids[0]!]: quota(ids[0]!, windowOf(40, 300, 'Five-hour')), [ids[1]!]: quota(ids[1]!, windowOf(10, 600, 'Five-hour')) } });
    await expect(compare(page).getByRole('columnheader')).toHaveCount(3);
    await expect(page.locator(`[data-quota-compare-account="${ids[0]}"]`).getByRole('cell')).toHaveText([/60% left/, 'Not available']);
    await expect(page.locator(`[data-quota-compare-account="${ids[1]}"]`).getByRole('cell')).toHaveText(['Not available', /90% left/]);
  });

  test('never shows a reading attributed to another profile', async ({ page }) => {
    await openCompare(page, { ...standard, mismatchQuota: [ids[1]!] });
    await expect(page.locator(`[data-quota-compare-account="${ids[1]}"]`)).toContainText('Could not load saved quota');
    await expect(page.locator(`[data-quota-compare-account="${ids[1]}"]`).getByRole('meter')).toHaveCount(0);
    await expect(page.locator(`[data-quota-compare-account="${ids[0]}"]`).getByRole('meter')).toHaveCount(2);
  });

  test('a failed load stays on its own row and is not reported as zero', async ({ page }) => {
    await openCompare(page, { ...standard, rejectQuota: [ids[0]!] });
    await expect(page.locator(`[data-quota-compare-account="${ids[0]}"]`)).toContainText('Could not load saved quota');
    await expect(page.locator(`[data-quota-compare-account="${ids[0]}"]`)).not.toContainText('0%');
    await expect(page.locator(`[data-quota-compare-account="${ids[1]}"]`).getByRole('meter')).toHaveCount(2);
  });

  test('stays hidden for a single account', async ({ page }) => {
    await open(page, { profiles: [profile(0)], quotas: { [ids[0]!]: quota(ids[0]!, windowOf(40, 300, 'Five-hour')) } });
    await page.locator('[data-workspace-nav="usage"]').click();
    await expect(page.locator('[data-workspace-view="usage"]')).toBeVisible();
    await expect(compare(page)).toHaveCount(0);
  });

  test('stays reachable by keyboard and has no accessibility violations', async ({ page }) => {
    await openCompare(page, standard);
    await page.setViewportSize({ width: 360, height: 800 });
    expect(await page.evaluate(() => document.scrollingElement!.scrollWidth <= innerWidth + 1)).toBe(true);
    await compare(page).getByRole('region', { name: 'Quota comparison', exact: true }).focus();
    await expect(compare(page).getByRole('region', { name: 'Quota comparison', exact: true })).toBeFocused();
    const audit = await new AxeBuilder({ page }).include('[data-quota-compare]').withTags(['wcag2a', 'wcag2aa', 'wcag21aa']).analyze();
    expect(audit.violations).toEqual([]);
  });
});
