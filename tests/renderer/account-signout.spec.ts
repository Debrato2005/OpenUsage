import { expect, test, type Page } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';
import type { ProfileRecord, ProfileView } from '../../apps/desktop/src/contracts';
import { navigate, openOptions } from './navigation';

// Synthetic renderer IPC only. Native tests own logout, credential isolation and retained storage.
const ids = ['11111111-1111-4111-8111-111111111111', '22222222-2222-4222-8222-222222222222'];
const approvalId = 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa';
const initial: ProfileRecord[] = ids.map((id, index) => ({ id, label: `Synthetic account ${index + 1}`, environment: 'native', archived: false, connectionState: 'connected' }));
type Deferred = { resolve: (value: unknown) => void; reject: (error: unknown) => void };
type Fixture = { profiles: ProfileRecord[]; calls: { command: string; args: Record<string, unknown> }[]; signouts: Deferred[]; checks: Deferred[] };
type FixtureWindow = Window & { accountSignoutFixture: Fixture };

async function open(page: Page, approved = true) {
  await page.addInitScript(({ initial, approvalId }) => {
    const state: Fixture = { profiles: initial, calls: [], signouts: [], checks: [] }; Object.assign(window, { accountSignoutFixture: state });
    const metadata = { path: '/synthetic/reviewed-codex', fileBytes: '100', sha256: 'a'.repeat(64), testedVersion: '0.163.0', versionChecked: false, sourceVerified: true, sourceKind: 'test_fixture', sourceUrl: null };
    window.__TAURI__ = { core: { invoke: async <T>(command: string, args: Record<string, unknown> = {}) => {
      state.calls.push({ command, args });
      if (command === 'bootstrap') return { storage: 'ready', theme: 'light', schemaVersion: 8, activeProfiles: 2, monitoring: 'stopped', provider: 'not_configured', keyProtection: 'os_store', storageIssue: null } as T;
      if (command === 'get_display_scale') return { percent: 100 } as T;
      if (command === 'set_theme') return { storage: 'ready', theme: args.theme, schemaVersion: 8, activeProfiles: 2, monitoring: 'stopped', provider: 'not_configured', keyProtection: 'os_store', storageIssue: null } as T;
      if (command === 'load_monitoring') return { state: 'stopped', readingRevision: 'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb', profiles: [] } as T;
      if (command === 'list_profiles') return { activeProfiles: state.profiles.filter(profile => !profile.archived).length, profiles: structuredClone(state.profiles) } as T;
      if (['load_profile_quota', 'load_profile_activity', 'load_profile_heatmap'].includes(command)) return null as T;
      if (command === 'load_profile_refresh_status') return { profileId: args.profileId, retryAfterSeconds: 0 } as T;
      if (command === 'inspect_provider_executable') return { ...metadata, candidateId: 'cccccccc-cccc-4ccc-8ccc-cccccccccccc' } as T;
      if (command === 'approve_provider_executable') return { ...metadata, approvalId } as T;
      if (command === 'sign_out_profile') return await new Promise<unknown>((resolve, reject) => state.signouts.push({ resolve, reject })) as T;
      if (command === 'check_profile_connection') return await new Promise<unknown>((resolve, reject) => state.checks.push({ resolve, reject })) as T;
      throw new Error(`Unexpected synthetic command ${command}`);
    } } };
  }, { initial, approvalId });
  await page.goto('/'); await navigate(page, 'settings');
  if (approved) {
    await page.getByRole('heading', { name: 'Codex connection', exact: true }).click(); await page.locator('[data-provider-manual]').click();
    await page.getByLabel('Codex executable', { exact: true }).fill('/synthetic/reviewed-codex');
    await page.getByRole('button', { name: 'Inspect executable', exact: true }).click();
    await page.getByLabel('Allow Codex to sign in and read usage.').check(); await page.getByRole('button', { name: 'Continue', exact: true }).click();
    await expect(page.getByRole('heading', { name: 'Ready to sign in', exact: true })).toBeVisible();
  }
  await navigate(page, 'accounts'); await openOptions(page, ids[0]!);
}
const row = (page: Page, id = ids[0]!) => page.locator(`[data-profile-id="${id}"]`);
const trigger = (page: Page, id = ids[0]!) => page.locator(`[data-account-signout-open="${id}"]`);
const dialog = (page: Page, id = ids[0]!) => page.locator(`[data-account-signout-dialog="${id}"]`);
const submit = (page: Page) => page.locator('[data-account-signout-submit]');
const signoutCalls = (page: Page) => page.evaluate(() => (window as unknown as FixtureWindow).accountSignoutFixture.calls.filter(call => call.command === 'sign_out_profile'));
async function publish(page: Page, index: number, id = ids[0]!, value?: ProfileView) {
  await expect.poll(() => page.evaluate(() => (window as unknown as FixtureWindow).accountSignoutFixture.signouts.length)).toBeGreaterThan(index);
  await page.evaluate(({ index, id, value }) => {
    const state = (window as unknown as FixtureWindow).accountSignoutFixture;
    const next = value ?? { activeProfiles: 2, profiles: state.profiles.map(profile => profile.id === id ? { ...profile, connectionState: 'authentication_required' as const } : profile) };
    state.profiles = next.profiles; state.signouts[index]!.resolve(next);
  }, { index, id, value });
}

test('sign-out stays in secondary actions and Cancel or Escape never invokes logout', async ({ page }) => {
  await open(page); expect(await signoutCalls(page)).toEqual([]);
  await trigger(page).focus(); await page.keyboard.press('Enter'); await expect(dialog(page)).toBeVisible();
  await expect(dialog(page).locator('[data-account-signout-cancel]')).toBeFocused();
  await expect(dialog(page)).toContainText('OpenAI may revoke this app session. Saved readings and history will remain');
  await expect(dialog(page).getByRole('textbox')).toHaveCount(0);
  await page.keyboard.press('Escape'); await expect(dialog(page)).toBeHidden(); await expect(trigger(page)).toBeFocused();
  await trigger(page).click(); await dialog(page).locator('[data-account-signout-cancel]').click(); await expect(dialog(page)).toBeHidden();
  expect(await signoutCalls(page)).toEqual([]);
  await navigate(page, 'overview'); await expect(page.locator('[data-overview] [data-account-signout-open]')).toHaveCount(0);
});

test('explicit selected-account logout submits once and exposes Sign in without changing another account', async ({ page }) => {
  await open(page); await trigger(page).click(); await submit(page).click();
  await expect(submit(page)).toBeDisabled(); await expect(dialog(page).locator('[data-account-signout-cancel]')).toBeDisabled();
  await page.keyboard.press('Escape'); await expect(dialog(page)).toBeVisible();
  await submit(page).evaluate(node => node.dispatchEvent(new MouseEvent('click', { bubbles: true })));
  expect((await signoutCalls(page)).map(call => call.args)).toEqual([{ profileId: ids[0], approvalId, confirmed: true }]);
  await publish(page, 0); await expect(row(page).getByRole('button', { name: 'Sign in', exact: true })).toBeVisible();
  await expect(row(page, ids[1]).locator('.profile-connection-state')).toHaveText('Connected');
  await expect(trigger(page)).toHaveCount(0);
  const calls = await page.evaluate(() => (window as unknown as FixtureWindow).accountSignoutFixture.calls.map(call => call.command));
  expect(calls.some(command => /delete|archive|create_profile|begin_profile_login/.test(command))).toBe(false);
});

test('failures show safe recovery text and a fresh confirmation is required after Cancel', async ({ page }) => {
  await open(page); await trigger(page).click(); await submit(page).click();
  await expect.poll(() => page.evaluate(() => (window as unknown as FixtureWindow).accountSignoutFixture.signouts.length)).toBe(1);
  await page.evaluate(() => (window as unknown as FixtureWindow).accountSignoutFixture.signouts[0]!.reject('provider_check_in_progress'));
  await expect(dialog(page).getByRole('alert')).toContainText('A provider operation is already running for this profile.'); await expect(dialog(page).getByRole('alert')).toBeFocused();
  await expect(row(page).locator('.profile-connection-state')).toHaveText('Connected');
  await dialog(page).locator('[data-account-signout-cancel]').click(); await trigger(page).click(); await expect(dialog(page).getByRole('alert')).toHaveCount(0);
  await submit(page).click(); await expect.poll(() => page.evaluate(() => (window as unknown as FixtureWindow).accountSignoutFixture.signouts.length)).toBe(2);
  await page.evaluate(() => (window as unknown as FixtureWindow).accountSignoutFixture.signouts[1]!.reject('SYNTHETIC-SECRET-ERROR'));
  await expect(dialog(page).getByRole('alert')).toContainText('could not complete this request'); await expect(dialog(page)).not.toContainText('SYNTHETIC-SECRET-ERROR');
  await submit(page).click(); await expect.poll(() => page.evaluate(() => (window as unknown as FixtureWindow).accountSignoutFixture.signouts.length)).toBe(3);
  const before = await page.evaluate(() => (window as unknown as FixtureWindow).accountSignoutFixture.calls.filter(call => call.command === 'list_profiles').length);
  await page.evaluate(() => (window as unknown as FixtureWindow).accountSignoutFixture.signouts[2]!.reject('provider_sign_out_save_failed'));
  await expect(dialog(page).getByRole('alert')).toContainText('Signed out, but OpenUsage could not save the account status.');
  expect(await page.evaluate(() => (window as unknown as FixtureWindow).accountSignoutFixture.calls.filter(call => call.command === 'list_profiles').length)).toBe(before + 1);
  await submit(page).click(); await publish(page, 3, ids[0]!, { activeProfiles: 2, profiles: initial });
  await expect(dialog(page).getByRole('alert')).toBeVisible();
  await expect(row(page).locator('.profile-connection-state')).toHaveText('Connected');
  await expect(row(page).getByRole('button', { name: 'Sign in', exact: true })).toHaveCount(0);
});

test('missing approval and in-flight provider work prevent sign-out', async ({ page }) => {
  await open(page, false); await expect(trigger(page)).toHaveCount(0);
  await navigate(page, 'settings');
  await page.getByRole('heading', { name: 'Codex connection', exact: true }).click(); await page.locator('[data-provider-manual]').click();
  await page.getByLabel('Codex executable', { exact: true }).fill('/synthetic/reviewed-codex'); await page.getByRole('button', { name: 'Inspect executable', exact: true }).click();
  await page.getByLabel('Allow Codex to sign in and read usage.').check(); await page.getByRole('button', { name: 'Continue', exact: true }).click();
  await navigate(page, 'accounts'); await openOptions(page, ids[0]!);
  await row(page).locator(`[data-provider-check="${ids[0]}"]`).click(); await expect(trigger(page)).toBeDisabled();
  expect(await signoutCalls(page)).toEqual([]);
});

test('a late old-account reply cannot alter a new pending account or release its busy state', async ({ page }) => {
  await open(page); await trigger(page).click(); await submit(page).click();
  await expect.poll(() => page.evaluate(() => (window as unknown as FixtureWindow).accountSignoutFixture.signouts.length)).toBe(1);
  // Replace the owning component deliberately; this does not cancel the native request.
  await page.evaluate(id => document.querySelector<HTMLButtonElement>(`[data-profile-id="${id}"] [data-profile-action="readings"]`)!.click(), ids[0]!);
  await openOptions(page, ids[1]!); await trigger(page, ids[1]).click(); await page.locator('[data-account-signout-submit]').click();
  await expect.poll(() => page.evaluate(() => (window as unknown as FixtureWindow).accountSignoutFixture.signouts.length)).toBe(2);
  await publish(page, 0); await expect(dialog(page, ids[1]).locator('[data-account-signout-submit]')).toBeDisabled();
  await expect(row(page, ids[1]).locator('.profile-connection-state')).toHaveText('Connected');
  await publish(page, 1, ids[1]); await expect(row(page, ids[1]).getByRole('button', { name: 'Sign in', exact: true })).toBeVisible();
});

test('confirmation is keyboard accessible and readable at narrow sizes in both themes', async ({ page }) => {
  await open(page); await page.setViewportSize({ width: 360, height: 800 });
  for (const theme of ['light', 'dark']) {
    await navigate(page, 'settings'); await page.getByRole('combobox', { name: 'Appearance', exact: true }).click(); await page.getByRole('option', { name: theme === 'light' ? 'Light' : 'Dark', exact: true }).click();
    await navigate(page, 'accounts'); await openOptions(page, ids[0]!);
    for (const scale of [1, 2]) {
      await page.evaluate(scale => { for (const node of document.querySelectorAll<HTMLElement>('body *')) { const base = node.dataset.signoutTestSize ?? getComputedStyle(node).fontSize; node.dataset.signoutTestSize = base; node.style.fontSize = `${parseFloat(base) * scale}px`; } }, scale);
      await trigger(page).click(); await expect(dialog(page).locator('[data-account-signout-cancel]')).toBeFocused();
      expect(await dialog(page).evaluate(node => node.scrollWidth <= node.clientWidth + 1)).toBe(true);
      expect((await new AxeBuilder({ page }).include('[data-account-signout-dialog]').withTags(['wcag2a', 'wcag2aa', 'wcag21aa']).analyze()).violations).toEqual([]);
      await page.keyboard.press('Escape'); await expect(trigger(page)).toBeFocused();
    }
  }
  expect(await signoutCalls(page)).toEqual([]);
});
