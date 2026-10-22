import { test, expect, type Page } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';
import type { ReadingExportReceipt, QuotaHistoryEntry } from '../../apps/desktop/src/contracts';
import { navigate } from './navigation';

// Synthetic renderer IPC. Native SQLCipher pagination and restart are verified separately.
const first = '11111111-1111-4111-8111-111111111111';
const second = '22222222-2222-4222-8222-222222222222';
const uuid = (value: number) => `${value.toString(16).padStart(8, '0')}-aaaa-4aaa-8aaa-aaaaaaaaaaaa`;
function entry(profileId: string, ordinal: number): QuotaHistoryEntry {
  return { profileId, bindingId: uuid(profileId === first ? 1 : 2), collectionRunId: uuid(ordinal), observedAtUtc: '2026-10-21T00:00:00.000Z', sourceKind: 'test_fixture', providerVersion: '0.163.0', limits: [{ limitId: 'codex', limitName: null, primary: { usedPercent: ordinal === 100 ? 0 : 100, derivedRemainingPercent: ordinal === 100 ? 100 : 0, windowDurationMins: 300, resetsAtUnix: null, label: 'Five-hour' }, secondary: null }] };
}
type HistoryFixture = { calls: { command: string; args: Record<string, unknown> }[]; deferFirst: boolean; pending: ((value: unknown) => void)[]; failSecond: boolean; wrongSecond: boolean; exportResolve?: (value: unknown) => void; exportReject?: (value: unknown) => void };
async function open(page: Page, options: { deferFirst?: boolean; failSecond?: boolean } = {}) {
  const entries = Array.from({ length: 22 }, (_, index) => entry(first, 100 - index));
  entries[1]!.limits[0]!.primary = null;
  await page.addInitScript(({ first, second, entries, other, options }) => {
    const fixture: HistoryFixture = { calls: [], deferFirst: !!options.deferFirst, pending: [], failSecond: !!options.failSecond, wrongSecond: false };
    Object.assign(window, { historyFixture: fixture });
    let theme = 'light';
    const profiles = [first, second].map((id, index) => ({ id, label: `History account ${index + 1}`, environment: 'native', archived: false, connectionState: 'authentication_required' }));
    const bootstrap = () => ({ storage: 'ready', theme, schemaVersion: 4, activeProfiles: 2, monitoring: 'stopped', provider: 'not_configured', keyProtection: 'os_store', storageIssue: null });
    window.__TAURI__ = { core: { invoke: async <T>(command: string, args: Record<string, unknown> = {}) => {
      if (command === 'load_time_zone') return { timeZone: 'UTC' } as T;
      fixture.calls.push({ command, args });
      if (command === 'bootstrap') return bootstrap() as T;
      if (command === 'set_theme') { theme = String(args.theme); return bootstrap() as T; }
      if (command === 'get_display_scale') return { percent: 100 } as T;
      if (command === 'list_profiles') return { activeProfiles: 2, profiles } as T;
      if (command === 'load_monitoring') return { state: 'stopped', readingRevision: first, profiles: [] } as T;
      if (command === 'load_profile_refresh_status') return { profileId: args.profileId, retryAfterSeconds: 0 } as T;
      if (command === 'load_profile_quota' || command === 'load_profile_activity') return null as T;
      if (command === 'export_quota_history') return await new Promise<unknown>((resolve, reject) => { fixture.exportResolve = resolve; fixture.exportReject = reject; }) as T;
      if (command === 'load_profile_quota_history') {
        if (args.profileId === first && fixture.deferFirst) return await new Promise<unknown>(resolve => fixture.pending.push(resolve)) as T;
        if (args.profileId === second) {
          if (fixture.failSecond) throw 'provider_quota_load_failed';
          return { profileId: fixture.wrongSecond ? first : second, entries: fixture.wrongSecond ? entries.slice(0, 1) : [other], nextBeforeRunId: null } as T;
        }
        const index = args.beforeRunId === null ? 0 : entries.findIndex(value => value.collectionRunId === args.beforeRunId) + 1;
        const selected = entries.slice(index, index + 20);
        return { profileId: first, entries: selected, nextBeforeRunId: index + 20 < entries.length ? selected.at(-1)!.collectionRunId : null } as T;
      }
      throw new Error(`Unexpected synthetic command ${command}`);
    } } };
  }, { first, second, entries, other: entry(second, 200), options });
  await page.goto('/');
  await navigate(page, 'usage');
}
const history = (page: Page) => page.locator('[data-quota-history]');
const fixture = (page: Page) => page.evaluate(() => (window as unknown as { historyFixture: HistoryFixture }).historyFixture.calls);
async function choose(page: Page, name: string) { await page.getByRole('combobox', { name: 'Usage profile' }).click(); await page.getByRole('option', { name, exact: true }).click(); }

test('history loads only on request, pages saved snapshots, and preserves zero, missing windows and provenance', async ({ page }) => {
  await open(page);
  expect((await fixture(page)).some(value => value.command === 'load_profile_quota_history')).toBe(false);
  await history(page).getByRole('button', { name: 'View quota history' }).click();
  await expect(history(page).locator('[data-quota-history-run]')).toHaveCount(20);
  const firstRow = history(page).locator('[data-quota-history-run]').first();
  await firstRow.getByRole('button').first().focus(); await page.keyboard.press('Enter');
  await expect(firstRow.getByRole('cell', { name: '0%', exact: true })).toBeVisible();
  await expect(firstRow.getByRole('cell', { name: '100%', exact: true })).toBeVisible();
  await expect(firstRow.getByRole('cell', { name: 'Not reported', exact: true })).toBeVisible();
  await expect(firstRow).toContainText('Synthetic test data');
  await expect(firstRow).not.toContainText('Fresh');
  const missing = history(page).locator('[data-quota-history-run]').nth(1);
  await expect(missing.getByRole('button').first()).toContainText('No quota windows');
  await missing.getByRole('button').first().click();
  await expect(missing).toContainText('This accepted reading supplied no quota windows.');
  await expect(missing.getByRole('row')).toHaveCount(1);
  await history(page).getByRole('button', { name: 'Older readings' }).click();
  await expect(history(page).locator('[data-quota-history-run]')).toHaveCount(2);
  await expect(history(page).locator('[data-quota-history-run]').first()).toHaveAttribute('data-quota-history-run', uuid(80));
  await expect(history(page).getByRole('button', { name: 'Older readings' })).toBeDisabled();
  await history(page).getByRole('button', { name: 'Newer readings' }).click();
  await expect(history(page).locator('[data-quota-history-run]').first()).toHaveAttribute('data-quota-history-run', uuid(100));
  expect((await fixture(page)).filter(value => value.command === 'load_profile_quota_history').map(value => value.args)).toEqual([{ profileId: first, beforeRunId: null }, { profileId: first, beforeRunId: uuid(81) }, { profileId: first, beforeRunId: null }]);
  expect((await fixture(page)).some(value => /^(begin_|complete_|check_profile)/.test(value.command))).toBe(false);
});

test('history ignores a late previous-profile reply and recovers through the selected profile only', async ({ page }) => {
  await open(page, { deferFirst: true, failSecond: true });
  await history(page).getByRole('button', { name: 'View quota history' }).click();
  await expect(history(page).getByRole('status')).toHaveText('Loading saved history…');
  await choose(page, 'History account 2');
  await history(page).getByRole('button', { name: 'View quota history' }).click();
  await expect(history(page).getByRole('alert')).toContainText('could not be loaded');
  await page.evaluate(({ first, value }) => { const state = (window as unknown as { historyFixture: HistoryFixture }).historyFixture; for (const resolve of state.pending.splice(0)) resolve({ profileId: first, entries: [value], nextBeforeRunId: null }); state.failSecond = false; state.wrongSecond = true; }, { first, value: entry(first, 100) });
  await history(page).getByRole('button', { name: 'Retry history' }).click();
  await expect(history(page).getByRole('alert')).toContainText('could not be loaded');
  await expect(history(page).locator('[data-quota-history-run]')).toHaveCount(0);
  await page.evaluate(() => { (window as unknown as { historyFixture: HistoryFixture }).historyFixture.wrongSecond = false; });
  await history(page).getByRole('button', { name: 'Retry history' }).click();
  await expect(history(page).locator('[data-quota-history-run]')).toHaveCount(1);
  await expect(history(page).locator('[data-quota-history-run]')).toHaveAttribute('data-quota-history-run', uuid(200));
  await expect(history(page)).toHaveAttribute('data-quota-history', second);
});

test('history details stay accessible in both themes at a narrow viewport with enlarged text', async ({ page }) => {
  await open(page); await history(page).getByRole('button', { name: 'View quota history' }).click();
  await history(page).locator('[data-quota-history-run]').first().getByRole('button').first().click();
  await page.setViewportSize({ width: 360, height: 800 });
  for (const theme of ['light', 'dark']) {
    await page.getByRole('combobox', { name: 'Appearance', exact: true }).click();
    await page.getByRole('option', { name: theme === 'light' ? 'Light' : 'Dark', exact: true }).click();
    await expect(page.locator('html')).toHaveAttribute('data-theme', theme);
    for (const scale of [1, 2]) {
      await page.evaluate(scale => { for (const node of document.querySelectorAll<HTMLElement>('body *')) { const baseline = node.dataset.historyTestSize ?? getComputedStyle(node).fontSize; node.dataset.historyTestSize = baseline; node.style.fontSize = `${parseFloat(baseline) * scale}px`; } }, scale);
      expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth + 1)).toBe(true);
      const table = history(page).getByRole('region', { name: 'Quota snapshot 2026-10-21T00:00:00.000Z', exact: true });
      await table.focus(); await page.keyboard.press('ArrowRight');
      await expect.poll(() => table.evaluate(node => node.scrollLeft)).toBeGreaterThan(0);
      expect((await new AxeBuilder({ page }).withTags(['wcag2a', 'wcag2aa', 'wcag21aa']).analyze()).violations).toEqual([]);
    }
  }
});


test('history export requires consent, omits identifiers by default, and keeps failures and late replies scoped', async ({ page }) => {
  await open(page);
  await history(page).getByRole('button', { name: 'View quota history' }).click();
  await history(page).getByRole('button', { name: 'Export quota history', exact: true }).click();
  const form = history(page).getByRole('form', { name: 'Quota history export' });
  const save = form.locator('[data-export-save]');
  const consent = form.getByRole('checkbox', { name: 'I understand this file contains unencrypted activity.', exact: true });
  await expect(form).toContainText('all history pages');
  await expect(form).toContainText('Email and credentials are never included.');
  await expect(save).toBeDisabled();
  await expect(form.getByRole('checkbox', { name: 'Include profile name and local references' })).not.toBeChecked();
  await consent.focus(); await page.keyboard.press('Space'); await save.click();
  await expect(save).toBeDisabled();
  await expect.poll(async () => (await fixture(page)).filter(value => value.command === 'export_quota_history').length).toBe(1);
  expect((await fixture(page)).find(value => value.command === 'export_quota_history')?.args).toEqual({ profileId: first, format: 'csv', includeIdentifiers: false, confirmed: true });
  const receipt: ReadingExportReceipt = { fileName: 'openusage-readings-0123456789abcdef.csv', directory: '/synthetic/Downloads/OpenUsage', format: 'csv', profileCount: 1, rowCount: 220, identifiersIncluded: false, exportedAt: '2026-10-21T00:00:00.000Z' };
  await page.evaluate(value => (window as unknown as { historyFixture: HistoryFixture }).historyFixture.exportResolve!(value), receipt);
  await expect(history(page).locator('[data-export-result]')).toBeFocused();
  await expect(history(page).locator('[data-export-result]')).toContainText('220 rows');
  await expect(consent).not.toBeChecked();
  await form.getByRole('combobox', { name: 'File format', exact: true }).click();
  await page.getByRole('option', { name: 'JSON', exact: true }).click();
  await form.getByRole('checkbox', { name: 'Include profile name and local references' }).check();
  await expect(save).toBeDisabled();
  await consent.check(); await save.click();
  await page.evaluate(() => (window as unknown as { historyFixture: HistoryFixture }).historyFixture.exportReject!('export_too_large'));
  await expect(history(page).locator('[data-export-error]')).toBeFocused();
  await expect(history(page).locator('[data-export-error]')).toContainText('No partial file was saved.');
  await expect(consent).not.toBeChecked();
  await consent.check(); await save.click();
  await expect.poll(async () => (await fixture(page)).filter(value => value.command === 'export_quota_history').length).toBe(3);
  expect((await fixture(page)).filter(value => value.command === 'export_quota_history').at(-1)?.args).toEqual({ profileId: first, format: 'json', includeIdentifiers: true, confirmed: true });
  await choose(page, 'History account 2');
  await page.evaluate(value => (window as unknown as { historyFixture: HistoryFixture }).historyFixture.exportResolve!(value), { ...receipt, format: 'json', fileName: 'openusage-readings-0123456789abcdef.json', identifiersIncluded: true });
  await history(page).getByRole('button', { name: 'View quota history' }).click();
  await expect(history(page).locator('[data-export-result]')).toHaveCount(0);
  await expect(history(page)).toHaveAttribute('data-quota-history', second);
  expect((await fixture(page)).some(value => /^(begin_|complete_|check_profile)/.test(value.command))).toBe(false);
});
