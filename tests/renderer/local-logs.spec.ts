import { expect, test, type Page } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';
import type { Bootstrap, LocalLogsView } from '../../apps/desktop/src/contracts';
import { navigate } from './navigation';

// Synthetic renderer IPC only. Native tests must establish detection, consent storage and scans.
const ready: Bootstrap = { storage: 'ready', theme: 'light', schemaVersion: 10, activeProfiles: 0, monitoring: 'stopped', provider: 'not_configured', keyProtection: 'os_store', storageIssue: null };
const fresh: Bootstrap = { ...ready, storage: 'setup_required', schemaVersion: null, activeProfiles: null };
const idle: LocalLogsView['scan'] = { state: 'idle', lastScanAtUtc: null };
const detected = (granted: Partial<Record<'claude' | 'codex', boolean>> = {}): LocalLogsView['tools'] =>
  (['claude', 'codex'] as const).map(tool => ({ tool, detected: true, granted: granted[tool] ?? false }));
type Fixture = { calls: { command: string; args: Record<string, unknown> }[]; boot: Bootstrap; logs: LocalLogsView | null };
type FixtureWindow = Window & { logFixture: Fixture };

async function open(page: Page, boot: Bootstrap, logs: LocalLogsView | null) {
  await page.addInitScript(({ boot, logs, ready }) => {
    const state: Fixture = { calls: [], boot, logs }; Object.assign(window, { logFixture: state });
    window.__TAURI__ = { core: { invoke: async <T>(command: string, args: Record<string, unknown> = {}) => {
      if (command === 'load_time_zone') return { timeZone: 'UTC' } as T;
      state.calls.push({ command, args });
      if (command === 'bootstrap') return state.boot as T;
      if (command === 'setup_workspace') { state.boot = ready; return state.boot as T; }
      if (command === 'get_display_scale') return { percent: 100 } as T;
      if (command === 'list_profiles') return { activeProfiles: 0, profiles: [] } as T;
      if (command === 'load_monitoring') return { state: 'stopped', readingRevision: '11111111-1111-4111-8111-111111111111', profiles: [] } as T;
      if (command === 'load_local_logs') { if (!state.logs) throw 'storage_unavailable'; return state.logs as T; }
      if (command === 'save_local_log_consent') {
        const tools = args.tools as string[];
        state.logs = { consentRequired: false, tools: (state.logs?.tools ?? []).map(t => ({ ...t, granted: tools.includes(t.tool) })), scan: state.logs?.scan ?? { state: 'idle', lastScanAtUtc: null } };
        return state.logs as T;
      }
      throw new Error(`Unexpected synthetic command ${command}`);
    } } };
  }, { boot, logs, ready });
  await page.goto('/');
}
const calls = async (page: Page, command: string) =>
  (await page.evaluate(() => (window as unknown as FixtureWindow).logFixture.calls)).filter(call => call.command === command);
const choice = (page: Page) => page.getByRole('group', { name: 'Read token counts from local logs', exact: true });

test('first launch lists every detected tool, selected, and the one action saves the choice', async ({ page }) => {
  await open(page, fresh, { consentRequired: true, tools: detected(), scan: idle });
  await expect(page.getByRole('status')).toHaveText('Welcome to OpenUsage');
  await expect(choice(page).getByRole('checkbox', { name: 'Claude Code', exact: true })).toBeChecked();
  await expect(choice(page).getByRole('checkbox', { name: 'Codex', exact: true })).toBeChecked();
  await expect(choice(page).getByRole('checkbox')).toHaveCount(2);
  expect((await new AxeBuilder({ page }).withTags(['wcag2a', 'wcag2aa', 'wcag21aa']).analyze()).violations).toEqual([]);
  await choice(page).getByRole('checkbox', { name: 'Codex', exact: true }).click();
  await expect(choice(page).getByRole('checkbox', { name: 'Codex', exact: true })).not.toBeChecked();
  expect(await calls(page, 'save_local_log_consent')).toEqual([]);
  await page.getByRole('button', { name: 'Create private workspace', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Overview', exact: true })).toBeVisible();
  expect(await calls(page, 'save_local_log_consent')).toEqual([{ command: 'save_local_log_consent', args: { tools: ['claude'] } }]);
  // The saved choice ends first launch, so no second consent appears.
  await expect(page.getByRole('dialog')).toHaveCount(0);
});

test('first launch and Settings keep the Pi consent choice returned by Rust', async ({ page }) => {
  await open(page, fresh, { consentRequired: true, tools: [{ tool: 'pi', detected: true, granted: false }], scan: idle });
  const pi = choice(page).getByRole('checkbox', { name: 'Pi', exact: true });
  await expect(pi).toBeChecked();
  await expect(choice(page).getByRole('checkbox')).toHaveCount(1);
  await pi.click();
  await expect(pi).not.toBeChecked();
  await pi.click();
  await expect(pi).toBeChecked();
  expect(await calls(page, 'save_local_log_consent')).toEqual([]);
  await page.getByRole('button', { name: 'Create private workspace', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Overview', exact: true })).toBeVisible();
  expect(await calls(page, 'save_local_log_consent')).toEqual([{ command: 'save_local_log_consent', args: { tools: ['pi'] } }]);
  await expect(page.getByRole('dialog')).toHaveCount(0);

  await navigate(page, 'settings');
  const settings = page.getByRole('region', { name: 'Local logs', exact: true });
  const selectedPi = settings.getByRole('checkbox', { name: 'Pi', exact: true });
  const save = settings.getByRole('button', { name: 'Save', exact: true });
  await expect(selectedPi).toBeChecked();
  await expect(save).toBeDisabled();
  await selectedPi.click();
  await save.click();
  await expect(save).toBeDisabled();
  await expect(selectedPi).not.toBeChecked();
  await selectedPi.click();
  await save.click();
  await expect(save).toBeDisabled();
  await expect(selectedPi).toBeChecked();
  expect(await calls(page, 'save_local_log_consent')).toEqual([
    { command: 'save_local_log_consent', args: { tools: ['pi'] } },
    { command: 'save_local_log_consent', args: { tools: [] } },
    { command: 'save_local_log_consent', args: { tools: ['pi'] } },
  ]);
});

test('first launch with no detected tool, or a failed check, shows no list and saves no choice', async ({ page }) => {
  for (const logs of [{ consentRequired: false, tools: [], scan: idle }, null]) {
    await open(page, fresh, logs);
    await expect(page.getByRole('status')).toHaveText('Welcome to OpenUsage');
    await expect(page.getByRole('button', { name: 'Create private workspace', exact: true })).toBeVisible();
    await expect(choice(page)).toHaveCount(0);
    await page.getByRole('button', { name: 'Create private workspace', exact: true }).click();
    await expect(page.getByRole('heading', { name: 'Overview', exact: true })).toBeVisible();
    expect(await calls(page, 'save_local_log_consent')).toEqual([]);
  }
});

test('an existing workspace with no saved choice asks once and Escape does not skip it', async ({ page }) => {
  await open(page, ready, { consentRequired: true, tools: detected(), scan: idle });
  const dialog = page.getByRole('dialog', { name: 'Local logs', exact: true });
  await expect(dialog).toBeVisible();
  await expect(dialog.getByRole('checkbox', { name: 'Claude Code', exact: true })).toBeChecked();
  await expect(dialog.getByRole('checkbox', { name: 'Codex', exact: true })).toBeChecked();
  expect((await new AxeBuilder({ page }).include('dialog').withTags(['wcag2a', 'wcag2aa', 'wcag21aa']).analyze()).violations).toEqual([]);
  await page.keyboard.press('Escape');
  await expect(dialog).toBeVisible();
  await dialog.getByRole('button', { name: 'Continue', exact: true }).click();
  await expect(dialog).toHaveCount(0);
  expect(await calls(page, 'save_local_log_consent')).toEqual([{ command: 'save_local_log_consent', args: { tools: ['claude', 'codex'] } }]);
});

test('an existing workspace keeps the Pi consent choice returned by Rust', async ({ page }) => {
  await open(page, ready, { consentRequired: true, tools: [{ tool: 'pi', detected: true, granted: false }], scan: idle });
  const dialog = page.getByRole('dialog', { name: 'Local logs', exact: true });
  await expect(dialog.getByRole('checkbox', { name: 'Pi', exact: true })).toBeChecked();
  await expect(dialog.getByRole('checkbox')).toHaveCount(1);
  expect(await calls(page, 'save_local_log_consent')).toEqual([]);
  await dialog.getByRole('button', { name: 'Continue', exact: true }).click();
  await expect(dialog).toHaveCount(0);
  expect(await calls(page, 'save_local_log_consent')).toEqual([{ command: 'save_local_log_consent', args: { tools: ['pi'] } }]);
});

test('Settings changes the choice and shows a failed scan', async ({ page }) => {
  await open(page, ready, { consentRequired: false, tools: detected({ claude: true }), scan: { state: 'failed', lastScanAtUtc: null } });
  await navigate(page, 'settings');
  const settings = page.getByRole('region', { name: 'Local logs', exact: true });
  await expect(settings.getByRole('checkbox', { name: 'Claude Code', exact: true })).toBeChecked();
  await expect(settings.getByRole('checkbox', { name: 'Codex', exact: true })).not.toBeChecked();
  await expect(settings.getByRole('alert')).toHaveText('The last local log scan failed. Saved totals stay unchanged.');
  const save = settings.getByRole('button', { name: 'Save', exact: true });
  await expect(save).toBeDisabled();
  await settings.getByRole('checkbox', { name: 'Codex', exact: true }).click();
  await expect(save).toBeEnabled();
  await save.click();
  await expect(save).toBeDisabled();
  await expect(settings.getByRole('checkbox', { name: 'Codex', exact: true })).toBeChecked();
  expect(await calls(page, 'save_local_log_consent')).toEqual([{ command: 'save_local_log_consent', args: { tools: ['claude', 'codex'] } }]);
});
