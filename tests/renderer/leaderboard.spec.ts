import { expect, test, type Page } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';
import type { LeaderboardRank, LeaderboardView } from '../../apps/desktop/src/contracts';
import { navigate } from './navigation';

type Call = { command: string; args: Record<string, unknown> };
type Fixture = { calls: Call[]; view: LeaderboardView; releaseBegin: (() => void) | null; rankError: string | null; holdRank: boolean; rankReading: boolean; rankReplies: Array<() => void> };
const operationId = '11111111-1111-4111-8111-111111111111';
async function open(page: Page, configured = false, deferBegin = false, joined = false) {
  await page.addInitScript(({ configured, operationId, deferBegin, joined }) => {
    const state: Fixture = { calls: [], releaseBegin: null, rankError: null, holdRank: false, rankReading: false, rankReplies: [], view: { configured, enrollment: joined ? 'joined' : configured ? 'not_joined' : 'not_checked', login: joined ? 'synthetic-user' : null, busy: false, operation: null } };
    Object.assign(window, { leaderboardFixture: state });
    window.__TAURI__ = { core: { invoke: async <T>(command: string, args: Record<string, unknown> = {}) => {
      state.calls.push({ command, args });
      if (command === 'bootstrap') return { storage: 'ready', theme: 'dark', schemaVersion: 13, activeProfiles: 0, monitoring: 'stopped', provider: 'not_configured', keyProtection: 'os_store', storageIssue: null } as T;
      if (command === 'get_display_scale') return { percent: 100 } as T;
      if (command === 'load_time_zone') return { timeZone: 'UTC' } as T;
      if (command === 'list_profiles') return { profiles: [], activeProfiles: 0 } as T;
      if (command === 'load_monitoring') return { state: 'stopped', readingRevision: '11111111-1111-4111-8111-111111111111', profiles: [] } as T;
      if (command === 'load_leaderboard' || command === 'refresh_leaderboard') return state.view as T;
      if (command === 'load_leaderboard_rank') {
        if (state.rankReading) throw 'busy';
        state.rankReading = true;
        try {
          if (state.rankError) throw state.rankError;
          if (!configured) throw 'unconfigured';
          if (state.view.enrollment !== 'joined') throw 'not_joined';
          const value: LeaderboardRank = { title: 'Self-reported', period: args.period as LeaderboardRank['period'], metric: args.metric as LeaderboardRank['metric'], snapshotAt: '2026-11-16T00:00:00.000Z', totalParticipants: '105', entry: { rank: '105', login: 'synthetic-user', totalTokens: '9007199254740991', estimatedValueMicros: null } };
          if (state.holdRank) return await new Promise<T>(resolve => { state.rankReplies.push(() => resolve(value as T)); });
          return value as T;
        } finally { state.rankReading = false; }
      }
      if (command === 'begin_leaderboard') {
        if (args.kind === 'sync') {
          const body = JSON.stringify({ deviceId: operationId, rows: [{ date: '2026-11-15', tool: 'claude', model: 'claude-sonnet', inputTokens: 1, outputTokens: 0, cacheReadTokens: 0, cacheWriteTokens: 0, totalTokens: 1, estimatedValueMicros: null }] });
          state.view = { configured: true, enrollment: 'joined', login: 'synthetic-user', busy: true, operation: { id: operationId, kind: 'sync', phase: 'preview', body } };
          return state.view as T;
        }
        if (args.kind === 'leave' || args.kind === 'forget') {
          state.view = { configured: true, enrollment: 'not_joined', login: null, busy: false, operation: { id: operationId, kind: args.kind, phase: 'finished', outcome: args.kind === 'leave' ? { state: 'left', login: 'synthetic-user', credentials: 'removed' } : { state: 'forgotten' } } };
          return state.view as T;
        }
        state.view = { configured: true, enrollment: 'pending', login: null, busy: true, operation: { id: operationId, kind: args.kind as 'join', phase: 'authorizing', userCode: 'ABCD-EFGH', verificationUri: 'https://github.com/login/device', expiresInSeconds: 600 } };
        if (deferBegin) return await new Promise<T>(resolve => { state.releaseBegin = () => resolve(state.view as T); });
        return state.view as T;
      }
      if (command === 'cancel_leaderboard') {
        state.view = { ...state.view, busy: false, operation: { id: operationId, kind: state.view.operation?.kind ?? 'join', phase: 'finished', outcome: { state: 'cancelled' } } };
        return state.view as T;
      }
      if (command === 'confirm_leaderboard_sync') {
        state.view = { ...state.view, busy: false, operation: { id: operationId, kind: 'sync', phase: 'finished', outcome: { state: 'synced', accepted: '1' } } };
        return state.view as T;
      }
      if (command === 'open_leaderboard_login_page') return null as T;
      throw new Error(`Unexpected synthetic command ${command}`);
    } } };
  }, { configured, operationId, deferBegin, joined });
  await page.goto('/');
  await navigate(page, 'settings');
}
const calls = (page: Page) => page.evaluate(() => (window as unknown as { leaderboardFixture: Fixture }).leaderboardFixture.calls);
const releaseRank = (page: Page, index: number) => page.evaluate(index => {
  const reply = (window as unknown as { leaderboardFixture: Fixture }).leaderboardFixture.rankReplies[index];
  if (!reply) throw new Error(`Rank reply ${index} is not ready`);
  reply();
}, index);

test('Overview opens own rank explicitly with exact scope and no enrollment mutation', async ({ page }) => {
  await open(page, true, false, true);
  await navigate(page, 'overview');
  expect((await calls(page)).filter(call => call.command === 'load_leaderboard_rank')).toEqual([]);
  const access = page.getByRole('button', { name: 'Self-reported rank', exact: true });
  await access.click();
  const dialog = page.getByRole('dialog', { name: 'Self-reported rank', exact: true });
  await expect(dialog.locator('[data-own-rank-value]')).toHaveText('105');
  await expect(dialog).toContainText('9,007,199,254,740,991');
  await expect(dialog).toContainText('Not available');
  await expect(dialog.locator('time')).toHaveAttribute('dateTime', '2026-11-16T00:00:00.000Z');
  expect((await calls(page)).filter(call => call.command === 'load_leaderboard_rank').map(call => call.args)).toEqual([{ period: '30d', metric: 'tokens' }]);
  expect((await calls(page)).filter(call => ['begin_leaderboard', 'confirm_leaderboard_sync'].includes(call.command))).toEqual([]);
  expect((await new AxeBuilder({ page }).include('[data-detail-panel][open]').withTags(['wcag2a', 'wcag2aa', 'wcag21aa']).analyze()).violations).toEqual([]);
  await page.keyboard.press('Escape');
  await expect(dialog).not.toBeVisible();
  await expect(access).toBeFocused();
});

test('rank selection serializes requests and discards the old scope before the next reply', async ({ page }) => {
  await open(page, true, false, true);
  await navigate(page, 'overview');
  await page.evaluate(() => { (window as unknown as { leaderboardFixture: Fixture }).leaderboardFixture.holdRank = true; });
  await page.getByRole('button', { name: 'Self-reported rank', exact: true }).click();
  const dialog = page.getByRole('dialog', { name: 'Self-reported rank', exact: true });
  await expect.poll(() => page.evaluate(() => (window as unknown as { leaderboardFixture: Fixture }).leaderboardFixture.rankReplies.length)).toBe(1);
  await dialog.getByRole('combobox', { name: 'Period', exact: true }).click();
  await dialog.getByRole('option', { name: '7 days', exact: true }).click();
  expect((await calls(page)).filter(call => call.command === 'load_leaderboard_rank')).toHaveLength(1);
  await releaseRank(page, 0);
  await expect.poll(() => page.evaluate(() => (window as unknown as { leaderboardFixture: Fixture }).leaderboardFixture.rankReplies.length)).toBe(2);
  await expect(dialog.locator('[data-own-rank-value]')).toHaveText('Loading…');
  await releaseRank(page, 1);
  await expect(dialog.locator('[data-own-rank-value]')).toHaveText('105');
  expect((await calls(page)).filter(call => call.command === 'load_leaderboard_rank').map(call => call.args)).toEqual([{ period: '30d', metric: 'tokens' }, { period: '7d', metric: 'tokens' }]);
});

test('rank selectors keep mouse and keyboard interaction inside the detail panel', async ({ page }) => {
  await open(page, true, false, true);
  await navigate(page, 'overview');
  await page.getByRole('button', { name: 'Self-reported rank', exact: true }).click();
  const dialog = page.getByRole('dialog', { name: 'Self-reported rank', exact: true });
  const period = dialog.getByRole('combobox', { name: 'Period', exact: true });
  await period.click();
  await expect(dialog.getByRole('option', { name: '7 days', exact: true })).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(dialog.getByRole('listbox')).toHaveCount(0);
  await expect(dialog).toBeVisible();
  await expect(period).toBeFocused();
  const metric = dialog.getByRole('combobox', { name: 'Rank by', exact: true });
  await metric.click();
  await dialog.getByRole('option', { name: 'Estimated value', exact: true }).click();
  await expect(metric).toHaveText('Estimated value');
  await expect(dialog.locator('[data-own-rank-value]')).toHaveText('105');
  await metric.press('Enter');
  await expect(dialog.getByRole('option', { name: 'Tokens', exact: true })).toBeVisible();
  await page.keyboard.press('Home');
  await page.keyboard.press('Enter');
  await expect(metric).toHaveText('Tokens');
  await expect(page.getByRole('dialog')).toHaveCount(1);
  expect((await calls(page)).filter(call => call.command === 'load_leaderboard_rank').map(call => call.args)).toEqual([{ period: '30d', metric: 'tokens' }, { period: '30d', metric: 'value' }, { period: '30d', metric: 'tokens' }]);
});

test('immediate rank reopen waits for the active read and discards closed selections', async ({ page }) => {
  await open(page, true, false, true);
  await navigate(page, 'overview');
  await page.evaluate(() => { (window as unknown as { leaderboardFixture: Fixture }).leaderboardFixture.holdRank = true; });
  const access = page.getByRole('button', { name: 'Self-reported rank', exact: true });
  await access.click();
  let dialog = page.getByRole('dialog', { name: 'Self-reported rank', exact: true });
  await expect.poll(() => page.evaluate(() => (window as unknown as { leaderboardFixture: Fixture }).leaderboardFixture.rankReplies.length)).toBe(1);
  await dialog.getByRole('combobox', { name: 'Period', exact: true }).click();
  await dialog.getByRole('option', { name: 'All time', exact: true }).click();
  await page.keyboard.press('Escape');
  await expect(dialog).not.toBeVisible();
  await access.click();
  dialog = page.getByRole('dialog', { name: 'Self-reported rank', exact: true });
  await expect(dialog).toBeVisible();
  expect((await calls(page)).filter(call => call.command === 'load_leaderboard_rank')).toHaveLength(1);
  await expect(dialog.locator('[data-own-rank-value]')).toHaveText('Loading…');
  await expect(dialog.getByRole('alert')).toHaveCount(0);
  await releaseRank(page, 0);
  await expect.poll(() => page.evaluate(() => (window as unknown as { leaderboardFixture: Fixture }).leaderboardFixture.rankReplies.length)).toBe(2);
  await expect(dialog.locator('[data-own-rank-value]')).toHaveText('Loading…');
  await releaseRank(page, 1);
  await expect(dialog.locator('[data-own-rank-value]')).toHaveText('105');
  await expect(dialog.getByRole('alert')).toHaveCount(0);
  expect((await calls(page)).filter(call => call.command === 'load_leaderboard_rank').map(call => call.args)).toEqual([{ period: '30d', metric: 'tokens' }, { period: '30d', metric: 'tokens' }]);
});

test('leaderboard status stays local and unavailable configuration never means absent enrollment', async ({ page }) => {
  await open(page);
  expect((await calls(page)).map(call => call.command).filter(command => command.includes('leaderboard'))).toEqual([]);
  await page.locator('[data-leaderboard-toggle]').click();
  const panel = page.locator('[data-leaderboard]');
  await expect(panel.getByText('Unavailable', { exact: true })).toBeVisible();
  await expect(panel.getByRole('button', { name: 'Join with GitHub', exact: true })).toBeDisabled();
  await expect(panel.getByText('Not joined', { exact: true })).toHaveCount(0);
  expect((await calls(page)).map(call => call.command).filter(command => command.includes('leaderboard'))).toEqual(['load_leaderboard']);
});

test('joining shows the public code and opens GitHub only after an explicit action', async ({ page }) => {
  await open(page, true);
  const panel = page.locator('[data-leaderboard]');
  await panel.locator('[data-leaderboard-toggle]').click();
  await panel.getByRole('button', { name: 'Join with GitHub', exact: true }).click();
  const code = panel.getByRole('textbox', { name: 'GitHub code', exact: true });
  await expect(code).toHaveValue('ABCD-EFGH');
  await expect(code).toHaveAttribute('readonly', '');
  await expect(code).toBeFocused();
  expect((await new AxeBuilder({ page }).include('[data-leaderboard]').withTags(['wcag2a', 'wcag2aa', 'wcag21aa']).analyze()).violations).toEqual([]);
  expect((await calls(page)).filter(call => call.command === 'begin_leaderboard').map(call => call.args)).toEqual([{ kind: 'join', confirmed: true }]);
  expect((await calls(page)).filter(call => call.command === 'open_leaderboard_login_page')).toEqual([]);
  await panel.getByRole('button', { name: 'Open GitHub', exact: true }).click();
  expect((await calls(page)).filter(call => call.command === 'open_leaderboard_login_page').map(call => call.args)).toEqual([{ operationId }]);
  await panel.getByRole('button', { name: 'Cancel', exact: true }).focus();
  await page.keyboard.press('Enter');
  await expect(panel.getByText('Sign-in cancelled.', { exact: true })).toBeVisible();
  await expect(code).toHaveCount(0);
  await expect(panel.getByRole('button', { name: 'Continue with GitHub', exact: true })).toBeFocused();
});

test('refreshed enrollment takes precedence over a retained join receipt', async ({ page }) => {
  await open(page, true);
  const panel = page.locator('[data-leaderboard]');
  await panel.locator('[data-leaderboard-toggle]').click();
  await panel.getByRole('button', { name: 'Join with GitHub', exact: true }).click();
  await page.evaluate(operationId => {
    const fixture = (window as unknown as { leaderboardFixture: Fixture }).leaderboardFixture;
    fixture.view = { configured: true, enrollment: 'joined', login: 'synthetic-user', busy: false, operation: { id: operationId, kind: 'join', phase: 'finished', outcome: { state: 'joined', login: 'synthetic-user' } } };
  }, operationId);
  await expect(panel.getByText('Joined as @synthetic-user.', { exact: true })).toBeVisible();
  await expect(panel.getByRole('button', { name: 'Reconnect with GitHub', exact: true })).toBeFocused();
  await page.evaluate(() => {
    const fixture = (window as unknown as { leaderboardFixture: Fixture }).leaderboardFixture;
    fixture.view = { ...fixture.view, enrollment: 'not_joined', login: null };
  });
  await panel.locator('.provider-advanced > summary').click();
  await panel.getByRole('button', { name: 'Refresh status', exact: true }).click();
  await expect(panel.getByText('Not joined', { exact: true })).toBeVisible();
  await expect(panel.getByText('Joined as @synthetic-user.', { exact: true })).toHaveCount(0);
  await expect(panel.getByRole('button', { name: 'Join with GitHub', exact: true })).toBeEnabled();
});

test('closing the section before a start reply cancels the late operation', async ({ page }) => {
  await open(page, true, true);
  const panel = page.locator('[data-leaderboard]');
  await panel.locator('[data-leaderboard-toggle]').click();
  await panel.getByRole('button', { name: 'Join with GitHub', exact: true }).click();
  await expect.poll(() => page.evaluate(() => !!(window as unknown as { leaderboardFixture: Fixture }).leaderboardFixture.releaseBegin)).toBe(true);
  await panel.locator('[data-leaderboard-toggle]').click();
  await page.evaluate(() => (window as unknown as { leaderboardFixture: Fixture }).leaderboardFixture.releaseBegin!());
  await expect.poll(async () => (await calls(page)).filter(call => call.command === 'cancel_leaderboard').map(call => call.args)).toEqual([{ operationId }]);
  await expect(panel.locator('[data-leaderboard-toggle]')).toBeFocused();
  await panel.locator('[data-leaderboard-toggle]').click();
  await expect(panel.getByText('Sign-in cancelled.', { exact: true })).toBeVisible();
  await expect(panel.getByRole('textbox', { name: 'GitHub code', exact: true })).toHaveCount(0);
});

test('sync reviews the exact upload and confirms only its retained operation', async ({ page }) => {
  await open(page, true, false, true);
  const panel = page.locator('[data-leaderboard]');
  await panel.locator('[data-leaderboard-toggle]').click();
  await panel.getByRole('button', { name: 'Preview upload', exact: true }).click({ timeout: 3000 });
  const dialog = page.getByRole('dialog', { name: 'Upload local usage', exact: true });
  await expect(dialog).toBeVisible();
  await expect(dialog.getByRole('button', { name: 'Cancel', exact: true })).toBeFocused();
  expect((await new AxeBuilder({ page }).include('[data-leaderboard] dialog').withTags(['wcag2a', 'wcag2aa', 'wcag21aa']).analyze()).violations).toEqual([]);
  await expect(dialog.locator('[data-leaderboard-preview]')).toHaveText('{"deviceId":"11111111-1111-4111-8111-111111111111","rows":[{"date":"2026-11-15","tool":"claude","model":"claude-sonnet","inputTokens":1,"outputTokens":0,"cacheReadTokens":0,"cacheWriteTokens":0,"totalTokens":1,"estimatedValueMicros":null}]}');
  expect((await calls(page)).filter(call => call.command === 'confirm_leaderboard_sync')).toEqual([]);
  await page.keyboard.press('Tab');
  await expect(dialog.getByRole('button', { name: 'Upload', exact: true })).toBeFocused();
  await page.keyboard.press('Enter');
  await expect(dialog).toHaveCount(0);
  await expect(panel.getByText('Upload confirmed.', { exact: true })).toBeVisible();
  await expect(panel.getByRole('button', { name: 'Preview upload', exact: true })).toBeFocused();
  await page.keyboard.press('Tab');
  await expect(panel.getByRole('button', { name: 'Leave the leaderboard', exact: true })).toBeFocused();
  expect((await calls(page)).filter(call => call.command === 'confirm_leaderboard_sync').map(call => call.args)).toEqual([{ operationId, confirmed: true }]);
});

test('cancelling an upload preview returns keyboard focus to the upload control', async ({ page }) => {
  await open(page, true, false, true);
  const panel = page.locator('[data-leaderboard]');
  await panel.locator('[data-leaderboard-toggle]').click();
  const preview = panel.getByRole('button', { name: 'Preview upload', exact: true });
  await preview.focus();
  await page.keyboard.press('Enter');
  const dialog = page.getByRole('dialog', { name: 'Upload local usage', exact: true });
  await expect(dialog.getByRole('button', { name: 'Cancel', exact: true })).toBeFocused();
  await page.keyboard.press('Escape');
  await expect(dialog).toHaveCount(0);
  await expect(preview).toBeFocused();
  expect((await calls(page)).filter(call => call.command === 'confirm_leaderboard_sync')).toEqual([]);
});

test('public departure and local erasure require separate renewed consent', async ({ page }) => {
  await open(page, true, false, true);
  const panel = page.locator('[data-leaderboard]');
  await panel.locator('[data-leaderboard-toggle]').click();
  const leave = panel.getByRole('button', { name: 'Leave the leaderboard', exact: true });
  await leave.click();
  let dialog = page.getByRole('alertdialog', { name: 'Leave the leaderboard', exact: true });
  await expect(dialog.getByRole('button', { name: 'Leave', exact: true })).toBeDisabled();
  await expect(dialog.getByRole('button', { name: 'Cancel', exact: true })).toBeFocused();
  expect((await new AxeBuilder({ page }).include('[data-leaderboard] dialog').withTags(['wcag2a', 'wcag2aa', 'wcag21aa']).analyze()).violations).toEqual([]);
  await expect(dialog).toContainText('all its devices, and all its public data');
  await page.keyboard.press('Escape');
  await expect(dialog).toHaveCount(0);
  await expect(leave).toBeFocused();
  expect((await calls(page)).filter(call => call.command === 'begin_leaderboard')).toEqual([]);
  await leave.click();
  await dialog.getByRole('checkbox').check();
  await dialog.getByRole('button', { name: 'Leave', exact: true }).click();
  await expect(panel.getByText('Public departure confirmed.', { exact: true })).toBeVisible();
  await panel.locator('.provider-advanced > summary').click();
  await panel.getByRole('button', { name: 'Clear local enrollment', exact: true }).click();
  dialog = page.getByRole('alertdialog', { name: 'Clear local enrollment', exact: true });
  await expect(dialog).toContainText('Public leaderboard data remains.');
  await expect(dialog.getByRole('button', { name: 'Clear enrollment', exact: true })).toBeDisabled();
  await dialog.getByRole('checkbox').check();
  await dialog.getByRole('button', { name: 'Clear enrollment', exact: true }).click();
  await expect(panel.getByText('Local enrollment cleared. Public leaderboard data remains.', { exact: true })).toBeVisible();
  expect((await calls(page)).filter(call => call.command === 'begin_leaderboard').map(call => call.args)).toEqual([{ kind: 'leave', confirmed: true }, { kind: 'forget', confirmed: true }]);
});
