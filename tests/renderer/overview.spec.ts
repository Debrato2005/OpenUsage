import { expect, test, type Page } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';
import type { ProfileRecord, SavedProfileActivity, SavedProfileQuota, SavedProfileHeatmap } from '../../apps/desktop/src/contracts';
import { navigate, openOptions, openReadings } from './navigation';

// Synthetic renderer fixtures. Native provider processes and encrypted storage are separate acceptance checks.
const first = '11111111-1111-4111-8111-111111111111';
const second = '22222222-2222-4222-8222-222222222222';
const observedAtUtc = '2026-10-20T00:00:00.000Z';
function activity(profileId: string, lifetimeTokens: string): SavedProfileActivity {
  return { profileId, bindingId: '33333333-3333-4333-8333-333333333333', collectionRunId: '44444444-4444-4444-8444-444444444444',
    providerVersion: '0.163.0', sourceKind: 'test_fixture', observedAtUtc, identityConfidence: 'provisional', verifiedIdentityKey: null,
    monitoring: 'stopped', accepted: true, coverage: 'unknown', scope: 'account_activity_with_unknown_application_scope', freshness: 'unknown',
    summary: { lifetimeTokens, peakDailyTokens: '9223372036854775807', currentStreakDays: '0', longestStreakDays: null, longestRunningTurnSec: null },
    dailyState: 'unavailable', dailyUsageBuckets: [], dailyTruncated: false, dailyChart: { maxTokens: null, bars: [] } };
}
function quota(profileId: string, remaining = 0): SavedProfileQuota {
  return { profileId, bindingId: '33333333-3333-4333-8333-333333333333', collectionRunId: '55555555-5555-4555-8555-555555555555',
    providerVersion: '0.163.0', sourceKind: 'test_fixture', observedAtUtc, identityConfidence: 'provisional', verifiedIdentityKey: null,
    monitoring: 'stopped', accepted: true, coverage: 'unknown', scope: 'account_quota_with_unknown_application_scope', freshness: 'unknown',
    availableResetCredits: '0', resetCreditDetails: null, limits: [{ limitId: 'codex', limitName: 'Synthetic account limit', reachedType: null, monthlyCreditLimit: null, spendControlReached: null,
      primary: { usedPercent: 100 - remaining, derivedRemainingPercent: remaining, windowDurationMins: 300, resetsAtUnix: null, label: 'Five-hour', resetState: 'expected' }, secondary: null }] };
}
type Pending = { profileId: string; kind: string; resolve: (value: unknown) => void };
type Fixture = { calls: string[]; deferFirst: boolean; mismatchFirst: boolean; pending: Pending[]; profiles: ProfileRecord[]; quotas: Record<string, SavedProfileQuota | null>; rejectQuota: string[]; mismatchQuota: string[]; deferQuota: string[] };
type FixtureWindow = Window & { overviewFixture: Fixture };

async function open(page: Page, options: { empty?: boolean; deferFirst?: boolean; chart?: boolean; count?: number; freshQuotas?: boolean; deferSecond?: boolean } = {}) {
  const chartActivity = activity(first, '0');
  if (options.chart) {
    chartActivity.dailyState = 'returned';
    chartActivity.dailyUsageBuckets = [
      { date: '2026-10-01', tokens: '0', observedAtUtc, freshness: 'stale' },
      { date: '2026-10-03', tokens: '9007199254740993', observedAtUtc, freshness: 'fresh' },
    ];
    chartActivity.dailyChart = { maxTokens: '9007199254740993', bars: [{ date: '2026-10-01', heightBasisPoints: 0 }, { date: '2026-10-03', heightBasisPoints: 10000 }] };
  }
  const ids = [first, second, ...Array.from({ length: Math.max(0, (options.count ?? 2) - 2) }, (_, index) => `${String(index + 3).padStart(8, '0')}-6666-4666-8666-666666666666`)];
  const readings = Object.fromEntries(ids.map(id => [id, id === first ? chartActivity : activity(id, '9223372036854775807')]));
  const quotas = Object.fromEntries(ids.map((id, index) => {
    const value = quota(id, index === 0 ? 0 : index === 1 ? 75 : index * 10);
    if (options.freshQuotas) value.freshness = 'fresh';
    return [id, value];
  }));
  const heatmap: SavedProfileHeatmap = { profileId: first, bindingId: chartActivity.bindingId, collectionRunId: chartActivity.collectionRunId, sourceKind: 'test_fixture', scope: chartActivity.scope, coverage: 'unknown', observedAtUtc, mode: 'daily', periodStart: '2026-10-01', periodEnd: '2026-10-03', scaleMaximum: '9007199254740993', cells: [
    { startDate: '2026-10-01', endDate: '2026-10-01', tokens: '0', level: 0, knownDays: 1, expectedDays: 1, observedAtUtc, freshness: 'stale' },
    { startDate: '2026-10-02', endDate: '2026-10-02', tokens: null, level: 0, knownDays: 0, expectedDays: 1, observedAtUtc: null, freshness: 'unknown' },
    { startDate: '2026-10-03', endDate: '2026-10-03', tokens: '9007199254740993', level: 4, knownDays: 1, expectedDays: 1, observedAtUtc, freshness: 'fresh' },
  ] };
  await page.addInitScript(({ first, second, ids, readings, quotas, options, heatmap }) => {
    const fixture: Fixture = { calls: [], deferFirst: options.deferFirst ?? false, mismatchFirst: false, pending: [], profiles: ids.map((id, index) => ({ id, label: index === 0 ? 'Zero fixture' : index === 1 ? 'Large fixture' : `Fixture ${index + 1}`, environment: 'native', archived: false, connectionState: 'authentication_required' })), quotas, rejectQuota: [], mismatchQuota: [], deferQuota: options.deferSecond ? [second] : [] };
    Object.assign(window, { overviewFixture: fixture });
    let theme = 'light';
    window.__TAURI__ = { core: { invoke: async <T>(command: string, args: Record<string, unknown> = {}) => {
      fixture.calls.push(command);
      if (command === 'load_combined_heatmap') return { status: 'unavailable', includedProfileIds: [], exclusions: (args.profileIds as string[]).map(profileId => ({ profileId, reason: 'not_connected' })) } as T;
      if (command === 'load_profile_heatmap') {
        if (!options.chart || args.profileId !== first) return null as T;
        if (args.mode === 'weekly') return { ...heatmap, mode: 'weekly', cells: [{ ...heatmap.cells[2], startDate: '2026-09-28', endDate: '2026-10-04', knownDays: 2, expectedDays: 7 }] } as T;
        if (args.mode === 'cumulative') return { ...heatmap, mode: 'cumulative', cells: heatmap.cells.map((cell, index) => ({ ...cell, tokens: index < 2 ? '0' : cell.tokens, knownDays: index < 2 ? 1 : 2, expectedDays: index + 1, observedAtUtc, freshness: 'stale' })) } as T;
        return heatmap as T;
      }
      if (command === 'load_monitoring') return { state: 'stopped', readingRevision: 'cccccccc-cccc-4ccc-8ccc-cccccccccccc', profiles: [] } as T;
      if (command === 'bootstrap') return { storage: 'ready', theme, schemaVersion: 4, activeProfiles: fixture.profiles.filter(profile => !profile.archived).length, monitoring: 'stopped', provider: 'not_configured', keyProtection: 'passphrase', storageIssue: null } as T;
      if (command === 'list_profiles') return { activeProfiles: fixture.profiles.filter(profile => !profile.archived).length, profiles: fixture.profiles } as T;
      if (command === 'set_profile_archived') {
        fixture.profiles.find(profile => profile.id === args.id)!.archived = args.archived === true;
        return { activeProfiles: fixture.profiles.filter(profile => !profile.archived).length, profiles: fixture.profiles } as T;
      }
      if (command === 'set_theme') { theme = String(args.theme); return { storage: 'ready', theme, schemaVersion: 4, activeProfiles: fixture.profiles.filter(profile => !profile.archived).length, monitoring: 'stopped', provider: 'not_configured', keyProtection: 'passphrase', storageIssue: null } as T; }
      if (command === 'load_profile_refresh_status') return { profileId: args.profileId, retryAfterSeconds: 0 } as T;
      if (command === 'load_profile_activity' || command === 'load_profile_quota') {
        const id = String(args.profileId), kind = command === 'load_profile_activity' ? 'activity' : 'quota';
        if (kind === 'quota' && fixture.rejectQuota.includes(id)) throw 'provider_quota_load_failed';
        if (fixture.deferFirst && id === first || kind === 'quota' && fixture.deferQuota.includes(id)) return await new Promise<unknown>(resolve => fixture.pending.push({ profileId: id, kind, resolve })) as T;
        if (options.empty) return null as T;
        const selected = fixture.mismatchFirst && id === first ? second : kind === 'quota' && fixture.mismatchQuota.includes(id) ? first : id;
        const values: Record<string, SavedProfileActivity | SavedProfileQuota | null> = kind === 'activity' ? readings : fixture.quotas;
        return structuredClone(values[selected]) as T;
      }
      throw new Error(`Unexpected overview fixture command: ${command}`);
    } } };
  }, { first, second, ids, readings, quotas, options, heatmap });
  await page.goto('/');
  await expect(page.getByRole('region', { name: 'Usage overview', exact: true })).toBeVisible();
}
const dashboard = (page: Page) => page.getByRole('region', { name: 'Usage overview', exact: true });
async function choose(page: Page, label: string) {
  await dashboard(page).getByRole('combobox', { name: 'Profile', exact: true }).click();
  await page.getByRole('option', { name: label, exact: true }).click();
}

test('renderer Overview keeps missing values unavailable and navigates through real Accounts controls', async ({ page }) => {
  await open(page, { empty: true });
  await expect(dashboard(page).locator('[data-overview-lifetime]')).toHaveText('Not available');
  await expect(dashboard(page).locator('[data-overview-quota]').getByRole('meter')).toHaveCount(0);
  await expect(dashboard(page).locator('[data-overview-quota-chart]')).toHaveCount(0);
  await navigate(page, 'accounts');
  await expect(page.locator('[data-workspace-view="accounts"]')).toBeVisible();
  await expect(page.locator('#profile-label')).toBeHidden();
  await page.getByRole('button', { name: 'Choose a name', exact: true }).click();
  await expect(page.getByLabel('Profile label', { exact: true })).toBeVisible();
  await navigate(page, 'usage');
  await navigate(page, 'overview');
  const calls = await page.evaluate(() => (window as unknown as FixtureWindow).overviewFixture.calls);
  expect(calls.some(command => /^(begin_|complete_|check_profile|inspect_provider|approve_provider)/.test(command))).toBe(false);
});

test('renderer Overview preserves exact zero and large values and rejects stale or mismatched profile readings', async ({ page }) => {
  await open(page);
  await expect(dashboard(page).locator('[data-overview-account-share]')).toHaveCount(1);
  await expect(dashboard(page).locator('[data-overview-quota-donut]')).toHaveAttribute('data-remaining', '0');
  await dashboard(page).getByRole('button', { name: 'Account share', exact: true }).click();
  await expect(dashboard(page).locator('.overview-share-panel')).toContainText('Unavailable');
  await expect(dashboard(page).locator('[data-overview-lifetime]')).toHaveText('0');
  await expect(dashboard(page).locator('[data-overview-quota]').getByRole('meter')).toHaveAttribute('aria-valuenow', '0');
  await choose(page, 'Large fixture');
  await expect(dashboard(page).locator('[data-overview-lifetime] bdi')).toHaveAttribute('title', '9223372036854775807');
  await page.evaluate(() => { const fixture = (window as unknown as FixtureWindow).overviewFixture; fixture.deferFirst = true; });
  await choose(page, 'Zero fixture');
  await expect(dashboard(page).locator('[data-overview-lifetime]')).toHaveText('Loading…');
  await expect(dashboard(page).locator('[data-overview-lifetime]')).not.toContainText('9223372036854775807');
  await expect.poll(() => page.evaluate(() => (window as unknown as FixtureWindow).overviewFixture.pending.length)).toBeGreaterThan(0);
  await choose(page, 'Large fixture');
  await expect(dashboard(page).locator('[data-overview-lifetime] bdi')).toHaveAttribute('title', '9223372036854775807');
  await page.evaluate(({ reading, savedQuota }) => { const fixture = (window as unknown as FixtureWindow).overviewFixture; fixture.deferFirst = false; fixture.mismatchFirst = true; for (const pending of fixture.pending.splice(0)) pending.resolve(pending.kind === 'activity' ? reading : savedQuota); }, { reading: activity(first, '0'), savedQuota: quota(first) });
  await expect(dashboard(page).locator('[data-overview-lifetime] bdi')).toHaveAttribute('title', '9223372036854775807');
  await choose(page, 'Zero fixture');
  await expect(dashboard(page).getByText('Saved activity could not be loaded. Try Refresh.', { exact: true })).toBeVisible();
  await expect(dashboard(page).locator('[data-overview-lifetime]')).toHaveText('Not available');
  await expect(dashboard(page).locator('[data-overview-quota]').getByRole('meter')).toHaveCount(0);
});

test('bundled DM Sans serves interface and exact readings with tabular digits', async ({ page }) => {
  const requestedFonts: string[] = [];
  page.on('request', request => { if (request.resourceType() === 'font') requestedFonts.push(request.url()); });
  await open(page); await choose(page, 'Large fixture');
  const rendered = await page.evaluate(async () => {
    const faces = await Promise.all([400, 500, 600].map(weight => document.fonts.load(`${weight} 14px "DM Sans"`)));
    await document.fonts.ready;
    const ui = getComputedStyle(document.body), reading = document.querySelector<HTMLElement>('[data-overview-lifetime]')!;
    const numeric = getComputedStyle(reading);
    const probe = document.createElement('span');
    probe.style.cssText = `position:absolute;visibility:hidden;font-family:${numeric.fontFamily};font-size:${numeric.fontSize};font-weight:${numeric.fontWeight};font-variant-numeric:tabular-nums;white-space:pre`;
    document.body.append(probe);
    const widths = ['0000000000', '1111111111', '9999999999'].map(text => { probe.textContent = text; return probe.getBoundingClientRect().width; });
    probe.remove();
    return { faces: faces.map(loaded => loaded.map(face => ({ family: face.family.replace(/["']/g, ''), status: face.status }))), ui: ui.fontFamily.split(',')[0]!.replace(/["']/g, ''), reading: numeric.fontFamily.split(',')[0]!.replace(/["']/g, ''), digits: numeric.fontVariantNumeric, widths };
  });
  expect(rendered.faces).toEqual(['DM Sans', 'DM Sans', 'DM Sans'].map(family => [{ family, status: 'loaded' }]));
  expect(rendered.ui).toBe('DM Sans'); expect(rendered.reading).toBe('DM Sans');
  expect(rendered.digits).toContain('tabular-nums');
  expect(rendered.widths[0]).toBeGreaterThan(0);
  expect(Math.max(...rendered.widths) - Math.min(...rendered.widths)).toBeLessThan(0.1);
  expect(requestedFonts.length).toBeGreaterThan(0);
  expect(requestedFonts).toContain(new URL('/fonts/DMSans-Variable.ttf', page.url()).href);
  expect(requestedFonts.every(url => url === new URL('/fonts/DMSans-Variable.ttf', page.url()).href)).toBe(true);
  await expect(dashboard(page).locator('[data-overview-lifetime] bdi')).toHaveAttribute('title', '9223372036854775807');
});

for (const scale of [1, 2]) test(`Overview compact count keeps exact values keyboard-accessible at 360px and ${scale * 100}% text`, async ({ page }) => {
  await open(page); await choose(page, 'Large fixture');
  await page.setViewportSize({ width: 360, height: 800 });
  if (scale === 2) await page.evaluate(() => {
    const sizes = [...document.querySelectorAll<HTMLElement>('body *')].map(node => ({ node, size: getComputedStyle(node).fontSize }));
    for (const { node, size } of sizes) node.style.fontSize = `${parseFloat(size) * 2}px`;
  });
  const total = dashboard(page).locator('[data-overview-lifetime]');
  await expect(total.locator('bdi')).toHaveAttribute('title', '9223372036854775807');
  await expect(total).not.toHaveText('9223372036854775807');
  expect(await page.evaluate(() => document.scrollingElement!.scrollWidth <= innerWidth + 1), JSON.stringify(await page.evaluate(() => [...document.querySelectorAll('.token-heatmap, .token-heatmap > *, .activity-scope-controls, .activity-scope-controls > *, .activity-scope-controls button, .overview-panel')].map(node => ({ tag: node.className, width: node.getBoundingClientRect().width, right: node.getBoundingClientRect().right, minWidth: getComputedStyle(node).minWidth }))))).toBe(true);
  const disclosure = dashboard(page).getByRole('button', { name: 'Reading details', exact: true });
  await dashboard(page).getByRole('button', { name: 'View usage', exact: true }).focus();
  await page.keyboard.press('Tab'); await expect(disclosure).toBeFocused();
  expect(await disclosure.evaluate(node => node.matches(':focus-visible') && getComputedStyle(node).outlineStyle !== 'none')).toBe(true);
  await page.keyboard.press('Enter');
  await expect(disclosure).toHaveAttribute('aria-expanded', 'true');
  const details = dashboard(page).locator('.overview-source-details');
  const exactValues = details.locator('bdi').filter({ hasText: /^9223372036854775807$/ });
  await expect(exactValues).toHaveCount(2);
  for (const value of await exactValues.all()) await expect(value).toBeVisible();
  await expect(details).toContainText('Account activity scope and coverage are unknown.');
  expect(await page.evaluate(() => document.scrollingElement!.scrollWidth <= innerWidth + 1), JSON.stringify(await page.evaluate(() => [...document.querySelectorAll('.token-heatmap, .token-heatmap > *, .activity-scope-controls, .activity-scope-controls > *, .activity-scope-controls button, .overview-panel')].map(node => ({ tag: node.className, width: node.getBoundingClientRect().width, right: node.getBoundingClientRect().right, minWidth: getComputedStyle(node).minWidth }))))).toBe(true);
});

test('renderer Overview remains accessible in both themes at 360 pixels and enlarged text', async ({ page }) => {
  await open(page);
  await choose(page, 'Large fixture');
  await page.setViewportSize({ width: 360, height: 800 });
  for (const theme of ['light', 'dark']) {
    await navigate(page, 'settings');
    await page.getByRole('combobox', { name: 'Appearance', exact: true }).click();
    await page.getByRole('option', { name: theme === 'light' ? 'Light' : 'Dark', exact: true }).click();
    await expect(page.locator('html')).toHaveAttribute('data-theme', theme);
    if (theme === 'dark') expect(await page.evaluate(() => ({ background: getComputedStyle(document.documentElement).backgroundColor, sidebar: getComputedStyle(document.querySelector('.sidebar')!).backgroundColor }))).toEqual({ background: 'rgb(0, 0, 0)', sidebar: 'rgb(0, 0, 0)' });
    await navigate(page, 'overview');
    for (const scale of [1, 2]) {
      await page.evaluate(scale => { for (const node of document.querySelectorAll<HTMLElement>('body *')) { const baseline = node.dataset.testFontSize ?? getComputedStyle(node).fontSize; node.dataset.testFontSize = baseline; node.style.fontSize = `${parseFloat(baseline) * scale}px`; } }, scale);
      expect(await page.evaluate(() => document.scrollingElement!.scrollWidth <= innerWidth + 1), JSON.stringify(await page.evaluate(() => [...document.querySelectorAll('.token-heatmap, .token-heatmap > *, .activity-scope-controls, .activity-scope-controls > *, .activity-scope-controls button, .overview-panel')].map(node => ({ tag: node.className, width: node.getBoundingClientRect().width, right: node.getBoundingClientRect().right, minWidth: getComputedStyle(node).minWidth }))))).toBe(true);
      await expect(dashboard(page).locator('[data-overview-lifetime] bdi')).toHaveAttribute('title', '9223372036854775807');
      const audit = await new AxeBuilder({ page }).withTags(['wcag2a', 'wcag2aa', 'wcag21aa']).analyze();
      expect(audit.violations, `${theme}, ${scale * 100}% test text`).toEqual([]);
      if (scale === 2) {
        await navigate(page, 'accounts');
        const status = page.locator('[data-workspace-view="accounts"] .profile-connection-state').first();
        await expect(status).toBeVisible();
        await status.scrollIntoViewIfNeeded();
        const word = await status.evaluate(node => {
          const walker = document.createTreeWalker(node, NodeFilter.SHOW_TEXT);
          while (walker.nextNode()) {
            const text = walker.currentNode, index = text.textContent?.indexOf('connected') ?? -1;
            if (index < 0) continue;
            const range = document.createRange();
            range.setStart(text, index); range.setEnd(text, index + 'connected'.length);
            const rectangles = [...range.getClientRects()].filter(rect => rect.width > 0 && rect.height > 0);
            return { text: range.toString(), rectangles: rectangles.length, lines: new Set(rectangles.map(rect => Math.round(rect.top))).size };
          }
          return null;
        });
        expect(word?.text).toBe('connected');
        expect(word?.rectangles).toBeGreaterThan(0);
        expect(word?.lines, `${theme}: Connection status must remain one rendered word at 360px and 200% text`).toBe(1);
        await navigate(page, 'overview');
      }
    }
  }
});


test('Overview heatmap distinguishes missing dates, zero and exact counts without bar geometry', async ({ page }) => {
  await open(page, { chart: true });
  const chart = dashboard(page).locator('[data-token-heatmap]');
  await expect(chart.locator('[data-heatmap-cell]')).toHaveCount(3);
  await expect(chart.locator('[data-heatmap-cell="2026-10-02"]')).toHaveAttribute('data-value', 'missing');
  await expect(chart.locator('[data-heatmap-cell="2026-10-01"]')).toHaveAttribute('data-value', 'zero');
  const cell = chart.locator('[data-heatmap-cell="2026-10-03"]');
  await cell.focus(); await expect(chart).toContainText('9007199254740993');
  await chart.locator('[data-heatmap-values-toggle]').click();
  await expect(chart.locator('table')).toContainText('9007199254740993');
  await choose(page, 'Large fixture');
  await expect(chart).toContainText('No saved daily activity');
});

for (const scale of [1, 2]) test(`Heatmap values stay exact and locally scrollable at 360px and ${scale * 100}% text`, async ({ page }) => {
  await open(page, { chart: true }); await page.setViewportSize({ width: 360, height: 800 });
  const chart = dashboard(page).locator('[data-token-heatmap]');
  await chart.locator('[data-heatmap-values-toggle]').click();
  if (scale === 2) await page.evaluate(() => { const sizes = [...document.querySelectorAll<HTMLElement>('body *')].map(node => ({ node, size: getComputedStyle(node).fontSize })); for (const { node, size } of sizes) node.style.fontSize = `${parseFloat(size) * 2}px`; });
  await expect(chart.locator('table')).toContainText('9007199254740993');
  expect(await page.evaluate(() => document.scrollingElement!.scrollWidth <= innerWidth + 1), JSON.stringify(await page.evaluate(() => [...document.querySelectorAll('.token-heatmap, .token-heatmap > *, .activity-scope-controls, .activity-scope-controls > *, .activity-scope-controls button, .overview-panel')].map(node => ({ tag: node.className, width: node.getBoundingClientRect().width, right: node.getBoundingClientRect().right, minWidth: getComputedStyle(node).minWidth }))))).toBe(true);
});


const accountRow = (page: Page, id: string) => dashboard(page).locator(`[data-overview-account="${id}"]`);

test('Overview shows saved quota and freshness for all ten accounts without selecting each or contacting a provider', async ({ page }) => {
  await open(page, { count: 10, freshQuotas: true });
  const rows = dashboard(page).locator('[data-overview-account]');
  await expect(rows).toHaveCount(10);
  const ids = await page.evaluate(() => (window as unknown as FixtureWindow).overviewFixture.profiles.map(profile => profile.id));
  for (const [index, id] of ids.entries()) {
    const row = accountRow(page, id);
    await expect(row.locator('[data-overview-account-quota]')).toContainText(`${index === 0 ? 0 : index === 1 ? 75 : index * 10}% left`);
    await expect(row.locator('[data-overview-account-quota]')).toContainText('Five-hour');
    await expect(row.getByRole('rowheader')).toContainText('Synthetic test data');
    await expect(row.locator('[data-overview-account-observed] time')).toHaveAttribute('datetime', observedAtUtc);
    await expect(row.locator('[data-overview-account-quota] [role=meter]')).toHaveAttribute('aria-valuenow', String(index === 0 ? 0 : index === 1 ? 75 : index * 10));
    await expect(row.locator('[data-overview-account-quota] [role=meter]')).toHaveAttribute('aria-label', /remaining \(derived\)/);
    await expect(row.locator('[data-overview-account-observed]')).not.toContainText('Freshness unknown');
  }
  await expect(accountRow(page, first).locator('button[aria-pressed]')).toHaveAttribute('aria-pressed', 'true');
  await expect(accountRow(page, second).locator('button[aria-pressed]')).toHaveAttribute('aria-pressed', 'false');
  expect(await page.evaluate(() => (window as unknown as FixtureWindow).overviewFixture.calls.some(command => /^(begin_|complete_|check_profile|inspect_provider|approve_provider)/.test(command)))).toBe(false);
});

for (const failure of ['rejected', 'mismatched'] as const) test(`Overview retains only the same account saved quota with unknown freshness after a ${failure} reload`, async ({ page }) => {
  await open(page, { freshQuotas: true });
  const row = accountRow(page, second);
  await expect(row.locator('[data-overview-account-quota]')).toContainText('75% left');
  await expect(row.locator('[data-overview-account-observed]')).not.toContainText('Freshness unknown');
  await navigate(page, 'accounts');
  await page.evaluate(({ second, failure }) => {
    const f = (window as unknown as FixtureWindow).overviewFixture;
    if (failure === 'rejected') f.rejectQuota = [second]; else f.mismatchQuota = [second];
  }, { second, failure });
  await navigate(page, 'overview');
  await expect(row.locator('[data-overview-account-quota]')).toContainText('75% left');
  await expect(row.locator('[data-overview-account-observed]')).toContainText('Freshness unknown');
  await expect(row.locator('[data-overview-account-observed]')).toContainText('Could not reload saved quota');
  await expect(row.locator('[data-overview-account-observed] time')).toHaveAttribute('datetime', observedAtUtc);
  await expect(accountRow(page, first).locator('[data-overview-account-quota]')).toContainText('0% left');
});

test('a successful missing-cache result clears the prior row instead of showing zero or an old quota', async ({ page }) => {
  await open(page);
  await expect(accountRow(page, second).locator('[data-overview-account-quota]')).toContainText('75% left');
  await navigate(page, 'accounts');
  await page.evaluate(second => { (window as unknown as FixtureWindow).overviewFixture.quotas[second] = null; }, second);
  await navigate(page, 'overview');
  await expect(accountRow(page, second).locator('[data-overview-account-quota]')).toHaveText('Not available');
  await expect(accountRow(page, second).locator('[data-overview-account-observed]')).toHaveText('Never');
  await expect(accountRow(page, first).locator('[data-overview-account-quota]')).toContainText('0% left');
});

test('archival removes the account and its late quota response cannot replace a restored row', async ({ page }) => {
  await open(page, { deferSecond: true });
  await expect(accountRow(page, second).locator('[data-overview-account-quota]')).toHaveText('Loading…');
  await expect.poll(() => page.evaluate(second => (window as unknown as FixtureWindow).overviewFixture.pending.filter(pending => pending.profileId === second).length, second)).toBeGreaterThan(0);
  await navigate(page, 'accounts'); await openOptions(page, second);
  await page.locator(`[data-profile-id="${second}"] [data-profile-action="archive"]`).click();
  await expect(page.getByText('Archived profiles (1)', { exact: true })).toBeVisible();
  await navigate(page, 'overview');
  await expect(accountRow(page, second)).toHaveCount(0);
  await expect(dashboard(page).locator('[data-overview-account]')).toHaveCount(1);
  await page.evaluate(({ second, value }) => {
    const f = (window as unknown as FixtureWindow).overviewFixture;
    f.deferQuota = []; f.quotas[second] = value;
  }, { second, value: quota(second, 30) });
  await navigate(page, 'accounts');
  await page.getByText('Archived profiles (1)', { exact: true }).click();
  await openReadings(page, second);
  await page.locator(`[data-profile-id="${second}"] [data-profile-action="restore"]`).click();
  await navigate(page, 'overview');
  await expect(accountRow(page, second).locator('[data-overview-account-quota]')).toContainText('30% left');
  await page.evaluate(async value => {
    const f = (window as unknown as FixtureWindow).overviewFixture;
    for (const pending of f.pending.splice(0)) pending.resolve(value);
    await new Promise<void>(resolve => requestAnimationFrame(() => requestAnimationFrame(() => resolve())));
  }, quota(second, 75));
  await expect(accountRow(page, first).locator('[data-overview-account-quota]')).toContainText('0% left');
  await expect(accountRow(page, second).locator('[data-overview-account-quota]')).toContainText('30% left');
  await expect(dashboard(page).locator('[data-overview-account]')).toHaveCount(2);
});


test('Overview offers individual and combined scopes without fabricating unsupported totals', async ({ page }) => {
  await open(page, { chart: true });
  await expect(dashboard(page).locator('[data-overview-account-share]')).toHaveCount(1);
  await expect(dashboard(page).locator('[data-overview-combined-usage]')).toHaveCount(0);
  await dashboard(page).getByRole('button', { name: 'Combined', exact: true }).click();
  const combined = dashboard(page).locator('[data-combined-heatmap]');
  await combined.getByRole('checkbox', { name: 'Zero fixture', exact: true }).check();
  await expect(combined).toContainText('These accounts cannot be combined yet.');
  await expect(combined).toContainText('Not connected');
  await expect(combined.locator('[data-heatmap-cell]')).toHaveCount(0);
  await dashboard(page).getByRole('button', { name: 'Individual', exact: true }).click();
  await expect(dashboard(page).locator('[data-token-heatmap]')).toContainText('Zero fixture');
  await dashboard(page).locator('[data-heatmap-mode=weekly]').click();
  await expect(dashboard(page).locator('[data-token-heatmap]')).toHaveAttribute('data-mode', 'weekly');
  const calls = await page.evaluate(() => (window as unknown as FixtureWindow).overviewFixture.calls);
  expect(calls.some(command => /^(begin_|complete_|check_profile)/.test(command))).toBe(false);
});

test('all provider-reported quota windows stay visible and missing windows are never inferred from plan names', async ({ page }) => {
  await open(page);
  await page.evaluate(first => {
    const fixture = (window as unknown as FixtureWindow).overviewFixture;
    fixture.quotas[first]!.limits[0]!.secondary = { usedPercent: 40, derivedRemainingPercent: 60, windowDurationMins: 10080, resetsAtUnix: null, label: 'Seven-day', resetState: 'expected' };
  }, first);
  await navigate(page, 'usage'); await navigate(page, 'overview');
  const selected = dashboard(page).locator('[data-overview-quota]');
  await expect(selected.getByRole('meter')).toHaveCount(2);
  await expect(selected.getByRole('meter').nth(0)).toHaveAttribute('aria-label', 'Five-hour: 0% remaining (derived)');
  await expect(selected.getByRole('meter').nth(1)).toHaveAttribute('aria-label', 'Seven-day: 60% remaining (derived)');
  await expect(selected.locator('[data-overview-window="Primary window"]')).toContainText('100% used');
  await expect(selected.locator('[data-overview-window="Secondary window"]')).toContainText('40% used');
  await expect(selected.getByRole('combobox', { name: 'Quota window', exact: true })).toHaveCount(0);
  await page.evaluate(first => {
    const fixture = (window as unknown as FixtureWindow).overviewFixture;
    fixture.quotas[first]!.limits[0]!.primary = null;
  }, first);
  await navigate(page, 'usage'); await navigate(page, 'overview');
  await expect(selected.getByRole('meter')).toHaveCount(1);
  await expect(selected.getByRole('meter')).toHaveAttribute('aria-label', 'Seven-day: 60% remaining (derived)');
  await expect(selected.locator('[data-overview-window="Primary window"]')).toHaveCount(0);
});

test('View usage returns to its account after choosing another account in Usage', async ({ page }) => {
  await open(page);
  const view = dashboard(page).getByRole('button', { name: 'View usage', exact: true }).first();
  await view.click();
  const picker = page.getByRole('combobox', { name: 'Usage profile', exact: true });
  await expect(picker).toContainText('Zero fixture');
  await picker.click();
  await page.getByRole('option', { name: 'Large fixture', exact: true }).click();
  await expect(picker).toContainText('Large fixture');
  await navigate(page, 'overview'); await view.click();
  await expect(picker).toContainText('Zero fixture');
});
