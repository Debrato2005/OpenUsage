import { expect, test, type Page } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';
import type { SavedProfileActivity, SavedProfileQuota } from '../../apps/desktop/src/contracts';
import { navigate } from './navigation';

// Synthetic renderer fixtures. Native provider processes and encrypted storage are separate acceptance checks.
const first = '11111111-1111-4111-8111-111111111111';
const second = '22222222-2222-4222-8222-222222222222';
const observedAtUtc = '2026-10-20T00:00:00.000Z';
function activity(profileId: string, lifetimeTokens: string): SavedProfileActivity {
  return { profileId, bindingId: '33333333-3333-4333-8333-333333333333', collectionRunId: '44444444-4444-4444-8444-444444444444',
    providerVersion: '0.163.0', sourceKind: 'test_fixture', observedAtUtc, identityConfidence: 'provisional', verifiedIdentityKey: null,
    monitoring: 'stopped', accepted: true, coverage: 'unknown', scope: 'account_activity_with_unknown_application_scope', freshness: 'unknown',
    summary: { lifetimeTokens, peakDailyTokens: '9223372036854775807', currentStreakDays: '0', longestStreakDays: null, longestRunningTurnSec: null },
    dailyState: 'unavailable', dailyUsageBuckets: [], dailyTruncated: false };
}
function quota(profileId: string): SavedProfileQuota {
  return { profileId, bindingId: '33333333-3333-4333-8333-333333333333', collectionRunId: '55555555-5555-4555-8555-555555555555',
    providerVersion: '0.163.0', sourceKind: 'test_fixture', observedAtUtc, identityConfidence: 'provisional', verifiedIdentityKey: null,
    monitoring: 'stopped', accepted: true, coverage: 'unknown', scope: 'account_quota_with_unknown_application_scope', freshness: 'unknown',
    availableResetCredits: '0', resetCreditDetails: null, limits: [{ limitId: 'codex', limitName: 'Synthetic account limit', reachedType: null, monthlyCreditLimit: null, spendControlReached: null,
      primary: { usedPercent: 100, derivedRemainingPercent: 0, windowDurationMins: 300, resetsAtUnix: null, label: 'Five-hour', resetState: 'expected' }, secondary: null }] };
}
type Pending = { profileId: string; kind: string; resolve: (value: unknown) => void };
type Fixture = { calls: string[]; deferFirst: boolean; mismatchFirst: boolean; pending: Pending[] };
type FixtureWindow = Window & { overviewFixture: Fixture };

async function open(page: Page, options: { empty?: boolean; deferFirst?: boolean } = {}) {
  await page.addInitScript(({ first, second, readings, quotas, options }) => {
    const fixture: Fixture = { calls: [], deferFirst: options.deferFirst ?? false, mismatchFirst: false, pending: [] };
    Object.assign(window, { overviewFixture: fixture });
    let theme = 'light';
    const profiles = [first, second].map((id, index) => ({ id, label: index === 0 ? 'Zero fixture' : 'Large fixture', environment: 'native', archived: false, connectionState: 'authentication_required' }));
    window.__TAURI__ = { core: { invoke: async <T>(command: string, args: Record<string, unknown> = {}) => {
      fixture.calls.push(command);
      if (command === 'bootstrap') return { storage: 'ready', theme, schemaVersion: 4, activeProfiles: 2, monitoring: 'stopped', provider: 'not_configured', keyProtection: 'passphrase', storageIssue: null } as T;
      if (command === 'list_profiles') return { activeProfiles: 2, profiles } as T;
      if (command === 'set_theme') { theme = String(args.theme); return { theme } as T; }
      if (command === 'load_profile_refresh_status') return { profileId: args.profileId, retryAfterSeconds: 0 } as T;
      if (command === 'load_profile_activity' || command === 'load_profile_quota') {
        const id = String(args.profileId), kind = command === 'load_profile_activity' ? 'activity' : 'quota';
        if (fixture.deferFirst && id === first) return await new Promise<unknown>(resolve => fixture.pending.push({ profileId: id, kind, resolve })) as T;
        if (options.empty) return null as T;
        const selected = fixture.mismatchFirst && id === first ? second : id;
        const values: Record<string, SavedProfileActivity | SavedProfileQuota> = kind === 'activity' ? readings : quotas;
        return structuredClone(values[selected]) as T;
      }
      throw new Error(`Unexpected overview fixture command: ${command}`);
    } } };
  }, { first, second, readings: { [first]: activity(first, '0'), [second]: activity(second, '9223372036854775807') }, quotas: { [first]: quota(first), [second]: quota(second) }, options });
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
  await expect(dashboard(page).getByRole('meter')).toHaveCount(0);
  await dashboard(page).getByRole('button', { name: 'View accounts', exact: true }).click();
  await expect(page.locator('[data-workspace-view="accounts"]')).toBeVisible();
  await expect(page.locator('#profile-label')).toBeHidden();
  await page.getByRole('button', { name: 'Add account', exact: true }).click();
  await expect(page.getByLabel('Profile label', { exact: true })).toBeVisible();
  await navigate(page, 'usage');
  await navigate(page, 'overview');
  const calls = await page.evaluate(() => (window as unknown as FixtureWindow).overviewFixture.calls);
  expect(calls.some(command => /^(begin_|complete_|check_profile|inspect_provider|approve_provider)/.test(command))).toBe(false);
});

test('renderer Overview preserves exact zero and large values and rejects stale or mismatched profile readings', async ({ page }) => {
  await open(page);
  await expect(dashboard(page).locator('[data-overview-lifetime]')).toHaveText('0');
  await expect(dashboard(page).getByRole('meter')).toHaveAttribute('aria-valuenow', '0');
  await choose(page, 'Large fixture');
  await expect(dashboard(page).locator('[data-overview-lifetime]')).toHaveText('9223372036854775807');
  await page.evaluate(() => { const fixture = (window as unknown as FixtureWindow).overviewFixture; fixture.deferFirst = true; });
  await choose(page, 'Zero fixture');
  await expect(dashboard(page).locator('[data-overview-lifetime]')).toHaveText('Loading…');
  await expect(dashboard(page).locator('[data-overview-lifetime]')).not.toContainText('9223372036854775807');
  await expect.poll(() => page.evaluate(() => (window as unknown as FixtureWindow).overviewFixture.pending.length)).toBeGreaterThan(0);
  await choose(page, 'Large fixture');
  await expect(dashboard(page).locator('[data-overview-lifetime]')).toHaveText('9223372036854775807');
  await page.evaluate(({ reading, savedQuota }) => { const fixture = (window as unknown as FixtureWindow).overviewFixture; fixture.deferFirst = false; fixture.mismatchFirst = true; for (const pending of fixture.pending.splice(0)) pending.resolve(pending.kind === 'activity' ? reading : savedQuota); }, { reading: activity(first, '0'), savedQuota: quota(first) });
  await expect(dashboard(page).locator('[data-overview-lifetime]')).toHaveText('9223372036854775807');
  await choose(page, 'Zero fixture');
  await expect(dashboard(page).getByText('Saved activity could not be loaded. Reload it from Accounts.', { exact: true })).toBeVisible();
  await expect(dashboard(page).locator('[data-overview-lifetime]')).toHaveText('Not available');
  await expect(dashboard(page).getByRole('meter')).toHaveCount(0);
});

test('renderer Overview remains accessible in both themes at 360 pixels and enlarged text', async ({ page }) => {
  await open(page);
  await choose(page, 'Large fixture');
  await page.setViewportSize({ width: 360, height: 800 });
  for (const theme of ['light', 'dark']) {
    await navigate(page, 'settings');
    await page.getByRole('combobox', { name: 'Appearance', exact: true }).click();
    await page.getByRole('option', { name: theme === 'light' ? 'Light' : 'Dark', exact: true }).click();
    await navigate(page, 'overview');
    for (const scale of [1, 2]) {
      await page.evaluate(scale => { for (const node of document.querySelectorAll<HTMLElement>('body *')) { const baseline = node.dataset.testFontSize ?? getComputedStyle(node).fontSize; node.dataset.testFontSize = baseline; node.style.fontSize = `${parseFloat(baseline) * scale}px`; } }, scale);
      expect(await page.evaluate(() => document.scrollingElement!.scrollWidth <= innerWidth + 1)).toBe(true);
      await expect(dashboard(page).locator('[data-overview-lifetime]')).toHaveText('9223372036854775807');
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
            const text = walker.currentNode, index = text.textContent?.indexOf('Sign-in') ?? -1;
            if (index < 0) continue;
            const range = document.createRange();
            range.setStart(text, index); range.setEnd(text, index + 'Sign-in'.length);
            const rectangles = [...range.getClientRects()].filter(rect => rect.width > 0 && rect.height > 0);
            return { text: range.toString(), rectangles: rectangles.length, lines: new Set(rectangles.map(rect => Math.round(rect.top))).size };
          }
          return null;
        });
        expect(word?.text).toBe('Sign-in');
        expect(word?.rectangles).toBeGreaterThan(0);
        expect(word?.lines, `${theme}: Sign-in must remain one rendered word at 360px and 200% text`).toBe(1);
        await navigate(page, 'overview');
      }
    }
  }
});
