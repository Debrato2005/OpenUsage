import { expect, test, type Page } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';
import type { Bootstrap } from '../../apps/desktop/src/contracts';
import { navigate } from './navigation';

// Synthetic renderer IPC only. Native tests must establish filesystem/key-store deletion and recovery.
const profileId = '11111111-1111-4111-8111-111111111111';
const ready: Bootstrap = { storage: 'ready', theme: 'light', schemaVersion: 8, activeProfiles: 1, monitoring: 'stopped', provider: 'not_configured', keyProtection: 'os_store', storageIssue: null };
const fresh: Bootstrap = { ...ready, storage: 'setup_required', schemaVersion: null, activeProfiles: null };
const incomplete: Bootstrap = { ...ready, storage: 'recovery_required', schemaVersion: null, activeProfiles: null, keyProtection: 'unavailable', storageIssue: 'local_deletion_incomplete' };
type Deferred = { resolve: (value: unknown) => void; reject: (error: unknown) => void };
type Fixture = { calls: { command: string; args: Record<string, unknown> }[]; deletions: Deferred[]; value: Bootstrap };
type FixtureWindow = Window & { deletionFixture: Fixture };

async function open(page: Page, pending = false) {
  await page.addInitScript(({ profileId, initial }) => {
    const state: Fixture = { calls: [], deletions: [], value: initial }; Object.assign(window, { deletionFixture: state });
    window.__TAURI__ = { core: { invoke: async <T>(command: string, args: Record<string, unknown> = {}) => {
      state.calls.push({ command, args });
      if (command === 'bootstrap') return state.value as T;
      if (command === 'set_theme') { state.value = { ...state.value, theme: args.theme as Bootstrap['theme'] }; return state.value as T; }
      if (command === 'get_display_scale') return { percent: 100 } as T;
      if (command === 'list_profiles') return { activeProfiles: 1, profiles: [{ id: profileId, label: 'Synthetic saved account', environment: 'native', archived: false, connectionState: 'authentication_required' }] } as T;
      if (command === 'load_monitoring') return { state: 'stopped', readingRevision: profileId, profiles: [] } as T;
      if (command === 'load_profile_refresh_status') return { profileId: args.profileId, retryAfterSeconds: 0 } as T;
      if (['load_profile_quota', 'load_profile_activity', 'load_profile_heatmap'].includes(command)) return null as T;
      if (command === 'delete_all_local_data') return await new Promise<unknown>((resolve, reject) => state.deletions.push({ resolve, reject })) as T;
      throw new Error(`Unexpected synthetic command ${command}`);
    } } };
  }, { profileId, initial: pending ? incomplete : ready });
  await page.goto('/'); if (!pending) await navigate(page, 'settings');
}
const section = (page: Page) => page.locator('[data-local-data-deletion]');
const trigger = (page: Page) => section(page).locator('[data-local-data-deletion-open]');
const dialog = (page: Page) => page.getByRole('alertdialog');
const consent = (page: Page) => dialog(page).getByRole('checkbox', { name: 'Delete my local OpenUsage data and account sessions.', exact: true });
const submit = (page: Page) => dialog(page).locator('[data-local-data-deletion-submit]');
const calls = (page: Page) => page.evaluate(() => (window as unknown as FixtureWindow).deletionFixture.calls);
const deletionCalls = async (page: Page) => (await calls(page)).filter(call => call.command === 'delete_all_local_data');
async function confirm(page: Page) { await trigger(page).click(); await expect(dialog(page)).toBeVisible(); await consent(page).check(); await submit(page).click(); }
async function publish(page: Page, index: number, value: Bootstrap = fresh, changeBootstrap = true) {
  await expect.poll(() => page.evaluate(() => (window as unknown as FixtureWindow).deletionFixture.deletions.length)).toBeGreaterThan(index);
  await page.evaluate(({ index, value, changeBootstrap }) => { const state = (window as unknown as FixtureWindow).deletionFixture; if (changeBootstrap) state.value = value; state.deletions[index]!.resolve(value); }, { index, value, changeBootstrap });
}

test('opening, cancelling and Escape never delete data or preserve confirmation', async ({ page }) => {
  await open(page); expect(await deletionCalls(page)).toEqual([]);
  await trigger(page).focus(); await page.keyboard.press('Enter'); await expect(dialog(page)).toBeVisible();
  await expect(dialog(page).getByRole('button', { name: 'Cancel', exact: true })).toBeFocused();
  await expect(dialog(page)).toContainText('account sessions, saved readings, alerts, account settings and managed recovery data');
  await expect(dialog(page)).toContainText('Display size, exported files and external backups remain.');
  await expect(dialog(page)).toContainText('This does not delete your OpenAI records or data in other Codex installations.');
  await expect(dialog(page).getByRole('textbox')).toHaveCount(0); await expect(submit(page)).toBeDisabled();
  await page.keyboard.press('Escape'); await expect(dialog(page)).toHaveCount(0); await expect(trigger(page)).toBeFocused();
  await trigger(page).click(); await consent(page).check(); await dialog(page).getByRole('button', { name: 'Cancel', exact: true }).click();
  await expect(dialog(page)).toHaveCount(0); await trigger(page).click(); await expect(consent(page)).not.toBeChecked(); await expect(submit(page)).toBeDisabled();
  expect(await deletionCalls(page)).toEqual([]);
});

test('renewed explicit consent submits once and blocks cancellation while deletion runs', async ({ page }) => {
  await open(page); await trigger(page).click(); await consent(page).focus(); await page.keyboard.press('Space');
  await submit(page).focus(); await page.keyboard.press('Enter');
  await expect(submit(page)).toBeDisabled(); await expect(dialog(page).getByRole('button', { name: 'Cancel', exact: true })).toBeDisabled();
  await expect(dialog(page).getByRole('form')).toHaveAttribute('aria-busy', 'true');
  await page.keyboard.press('Escape'); await expect(dialog(page)).toBeVisible();
  // A duplicate submit event must not bypass the same pending-operation guard.
  await dialog(page).getByRole('form').evaluate(node => node.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true })));
  expect((await deletionCalls(page)).map(call => call.args)).toEqual([{ confirmed: true }]);
  await publish(page, 0);
  await expect(page.getByRole('status')).toHaveText('Welcome to OpenUsage'); await expect(page.getByRole('navigation', { name: 'Main', exact: true })).toBeHidden();
  expect((await calls(page)).some(call => call.command === 'setup_workspace')).toBe(false);
});

test('failed deletion rechecks bootstrap, keeps errors actionable and clears cached account UI when cleanup becomes pending', async ({ page }) => {
  await open(page);
  for (const [index, expected] of [
    { error: 'workspace_locked', message: 'Unlock your workspace' },
    { error: 'local_deletion_unsupported', message: 'Local deletion is not available on this operating system yet.' },
    { error: 'key_store_unavailable', message: 'system credential store is unavailable' },
    { error: 'local_deletion_unsafe', message: 'An unexpected file or path prevents deletion.' },
  ].entries()) {
    if (index === 0) await trigger(page).click();
    await consent(page).check(); await submit(page).click();
    await expect.poll(() => page.evaluate(() => (window as unknown as FixtureWindow).deletionFixture.deletions.length)).toBe(index + 1);
    await page.evaluate(({ index, error }) => (window as unknown as FixtureWindow).deletionFixture.deletions[index]!.reject(error), { index, error: expected.error });
    await expect(dialog(page).getByRole('alert')).toContainText(expected.message); await expect(dialog(page).getByRole('alert')).toBeFocused();
    await expect(consent(page)).not.toBeChecked(); await expect(submit(page)).toBeDisabled();
    const commands = (await calls(page)).map(call => call.command), failedAt = commands.lastIndexOf('delete_all_local_data');
    expect(commands.slice(failedAt + 1)).toContain('bootstrap');
  }
  await consent(page).check(); await submit(page).click();
  await expect.poll(() => page.evaluate(() => (window as unknown as FixtureWindow).deletionFixture.deletions.length)).toBe(5);
  await page.evaluate(value => { const state = (window as unknown as FixtureWindow).deletionFixture; state.value = value; state.deletions[4]!.reject('local_deletion_failed'); }, incomplete);
  await expect(trigger(page)).toHaveText('Finish deleting local data'); await expect(section(page)).toContainText('Some local data may remain.');
  await expect(page.getByRole('navigation', { name: 'Main', exact: true })).toBeHidden(); await expect(page.getByText('Synthetic saved account', { exact: true })).not.toBeVisible();
  expect(await deletionCalls(page)).toHaveLength(5);
});

test('pending cleanup requires another explicit confirmation and never deletes automatically on restart', async ({ page }) => {
  await open(page, true); await expect(trigger(page)).toHaveText('Finish deleting local data'); expect(await deletionCalls(page)).toEqual([]);
  await expect(section(page)).toContainText('before creating another workspace'); await expect(page.getByRole('button', { name: 'Create private workspace', exact: true })).toHaveCount(0);
  await trigger(page).click(); await expect(consent(page)).not.toBeChecked(); await expect(submit(page)).toBeDisabled();
  await expect(dialog(page)).toContainText('Some local data may remain from the previous deletion.');
  await dialog(page).getByRole('button', { name: 'Cancel', exact: true }).click(); expect(await deletionCalls(page)).toEqual([]);
  await confirm(page); await publish(page, 0, ready, false);
  await expect(trigger(page)).toHaveText('Finish deleting local data');
  await expect(dialog(page).getByRole('alert')).toBeVisible(); await expect(consent(page)).not.toBeChecked();
  await expect(page.getByText('Welcome to OpenUsage', { exact: true })).toHaveCount(0);
  await consent(page).check(); await submit(page).click(); await publish(page, 1);
  await expect(page.getByRole('status')).toHaveText('Welcome to OpenUsage');
  expect((await deletionCalls(page)).map(call => call.args)).toEqual([{ confirmed: true }, { confirmed: true }]);
});

test('a reply from a replaced Settings component cannot change the new view or release its deletion busy state', async ({ page }) => {
  await open(page); await confirm(page);
  await expect.poll(() => page.evaluate(() => (window as unknown as FixtureWindow).deletionFixture.deletions.length)).toBe(1);
  // Deliberate renderer lifecycle replacement, not user cancellation of an irreversible native operation.
  await page.evaluate(() => document.querySelector<HTMLAnchorElement>('[data-workspace-nav="usage"]')!.click());
  await expect(section(page)).toHaveCount(0); await navigate(page, 'settings'); await confirm(page);
  await expect.poll(() => page.evaluate(() => (window as unknown as FixtureWindow).deletionFixture.deletions.length)).toBe(2);
  await publish(page, 0, fresh, false);
  await expect(dialog(page).getByRole('form')).toHaveAttribute('aria-busy', 'true'); await expect(submit(page)).toBeDisabled();
  await expect(page.getByText('Welcome to OpenUsage', { exact: true })).toHaveCount(0);
  await publish(page, 1); await expect(page.getByRole('status')).toHaveText('Welcome to OpenUsage');
});

test('the confirmation dialog stays readable and keyboard accessible in narrow light and dark views', async ({ page }) => {
  await open(page); await page.setViewportSize({ width: 360, height: 800 });
  for (const theme of ['light', 'dark']) {
    await page.getByRole('combobox', { name: 'Appearance', exact: true }).click(); await page.getByRole('option', { name: theme === 'light' ? 'Light' : 'Dark', exact: true }).click();
    for (const scale of [1, 2]) {
      await page.evaluate(scale => { for (const node of document.querySelectorAll<HTMLElement>('body *')) { const base = node.dataset.deletionTestSize ?? getComputedStyle(node).fontSize; node.dataset.deletionTestSize = base; node.style.fontSize = `${parseFloat(base) * scale}px`; } }, scale);
      await trigger(page).click(); await expect(dialog(page).getByRole('button', { name: 'Cancel', exact: true })).toBeFocused();
      await consent(page).focus(); await page.keyboard.press('Space'); await expect(consent(page)).toBeChecked();
      expect(await dialog(page).evaluate(node => node.scrollWidth <= node.clientWidth + 1)).toBe(true);
      expect((await new AxeBuilder({ page }).include('[data-local-data-deletion-dialog]').withTags(['wcag2a', 'wcag2aa', 'wcag21aa']).analyze()).violations).toEqual([]);
      await page.keyboard.press('Escape'); await expect(dialog(page)).toHaveCount(0); await expect(trigger(page)).toBeFocused();
    }
  }
  expect(await deletionCalls(page)).toEqual([]);
});
