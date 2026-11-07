import { expect, test, type Page } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';
import { localUsage } from './local-usage-fixture';

// Synthetic renderer IPC only. Rust builds the real images. Native tests must establish saved files.
const ready = { storage: 'ready', theme: 'light', schemaVersion: 10, activeProfiles: 0, monitoring: 'stopped', provider: 'not_configured', keyProtection: 'os_store', storageIssue: null };
const sizes = { wrapped: [1200, 630], heatmap: [1000, 230], badge: [234, 20] } as const;
type Call = { command: string; args: Record<string, unknown>; png?: { signature: boolean; width: number; height: number } };
type FixtureWindow = Window & { shareCalls: Call[] };

async function open(page: Page, svg?: string) {
  await page.addInitScript(({ boot, usage, sizes, svg }) => {
    const calls: Call[] = []; Object.assign(window, { shareCalls: calls });
    window.__TAURI__ = { core: { invoke: async <T>(command: string, args: Record<string, unknown> = {}) => {
      if (command === 'load_time_zone') return { timeZone: 'UTC' } as T;
      if (command === 'bootstrap') return boot as T;
      if (command === 'get_display_scale') return { percent: 100 } as T;
      if (command === 'list_profiles') return { activeProfiles: 0, profiles: [] } as T;
      if (command === 'load_monitoring') return { state: 'stopped', readingRevision: '11111111-1111-4111-8111-111111111111', profiles: [] } as T;
      if (command === 'load_local_logs') return { consentRequired: false, tools: [{ tool: 'claude', detected: true, granted: true }], scan: { state: 'idle', lastScanAtUtc: null } } as T;
      if (command === 'load_local_usage') return usage as T;
      const kind = args.kind as keyof typeof sizes;
      if (command === 'load_share_image') {
        calls.push({ command, args });
        const [width, height] = sizes[kind];
        return { svg: svg ?? `<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}" viewBox="0 0 ${width} ${height}"><rect width="${width}" height="${height}" rx="24" fill="#101216"/><text x="64" y="160" font-size="96" fill="#f3f4f6">55.0M</text></svg>`, width, height, summary: `Synthetic ${kind} image` } as T;
      }
      if (command === 'save_share_image') {
        const call: Call = { command, args };
        if (typeof args.png === 'string') {
          const bytes = Uint8Array.from(atob(args.png), c => c.charCodeAt(0));
          const view = new DataView(bytes.buffer);
          call.png = { signature: [137, 80, 78, 71, 13, 10, 26, 10].every((b, i) => bytes[i] === b), width: view.getUint32(16), height: view.getUint32(20) };
        }
        calls.push(call);
        return { fileName: `openusage-${kind}-AbCdEfGhIjKlMnOp.${args.format}`, directory: '/home/synthetic/Downloads/OpenUsage', format: args.format, byteCount: '1234' } as T;
      }
      throw new Error(`Unexpected synthetic command ${command}`);
    } } };
  }, { boot: ready, usage: localUsage(), sizes, svg });
  await page.goto('/');
}
const calls = (page: Page) => page.evaluate(() => (window as unknown as FixtureWindow).shareCalls);
const dialog = (page: Page) => page.getByRole('dialog', { name: 'Share', exact: true });

test('Share previews each image, removes the branding on request, and saves SVG and PNG only on an explicit action', async ({ page }) => {
  await open(page);
  await page.locator('[data-local-overview] [data-share-open]').click();
  await expect(dialog(page).getByRole('img', { name: 'Synthetic wrapped image', exact: true })).toBeVisible();
  // StrictMode runs effects twice in development, so every load, not the count, is checked.
  expect(new Set((await calls(page)).map(c => JSON.stringify(c)))).toEqual(new Set([JSON.stringify({ command: 'load_share_image', args: { kind: 'wrapped', branding: true } })]));
  expect((await new AxeBuilder({ page }).include('[data-share-dialog]').withTags(['wcag2a', 'wcag2aa', 'wcag21aa']).analyze()).violations).toEqual([]);
  await dialog(page).getByRole('checkbox', { name: 'Show the OpenUsage name and address', exact: true }).click();
  await dialog(page).getByRole('button', { name: 'Heatmap', exact: true }).click();
  await expect(dialog(page).getByRole('img', { name: 'Synthetic heatmap image', exact: true })).toBeVisible();
  expect((await calls(page)).filter(c => c.command === 'save_share_image')).toEqual([]);
  await dialog(page).getByRole('button', { name: 'Save SVG', exact: true }).click();
  await expect(dialog(page).getByRole('status')).toHaveText('Saved openusage-heatmap-AbCdEfGhIjKlMnOp.svg in /home/synthetic/Downloads/OpenUsage');
  await dialog(page).getByRole('button', { name: 'Save PNG', exact: true }).click();
  await expect(dialog(page).getByRole('status')).toHaveText('Saved openusage-heatmap-AbCdEfGhIjKlMnOp.png in /home/synthetic/Downloads/OpenUsage');
  const saves = (await calls(page)).filter(c => c.command === 'save_share_image');
  expect(saves[0]).toEqual({ command: 'save_share_image', args: { kind: 'heatmap', branding: false, format: 'svg', png: null } });
  // The PNG is the preview drawn at twice its size.
  expect(saves[1]!.args).toMatchObject({ kind: 'heatmap', branding: false, format: 'png' });
  expect(saves[1]!.png).toEqual({ signature: true, width: 2000, height: 460 });
  await page.keyboard.press('Escape');
  await expect(dialog(page)).toHaveCount(0);
});

test('an image with script or links is refused and shows an error, not a preview', async ({ page }) => {
  await open(page, '<svg xmlns="http://www.w3.org/2000/svg" width="1200" height="630"><script>alert(1)</script></svg>');
  await page.locator('[data-local-overview] [data-share-open]').click();
  await expect(dialog(page).getByRole('alert')).toHaveText('OpenUsage could not make this image. Try again.');
  await expect(dialog(page).getByRole('img')).toHaveCount(0);
  await expect(dialog(page).getByRole('button', { name: 'Save PNG', exact: true })).toBeDisabled();
});

test('the README badge is SVG only and saves with its own name', async ({ page }) => {
  await open(page);
  await page.locator('[data-local-overview] [data-share-open]').click();
  await dialog(page).getByRole('button', { name: 'Badge', exact: true }).click();
  await expect(dialog(page).getByRole('img', { name: 'Synthetic badge image', exact: true })).toBeVisible();
  await expect(dialog(page).getByRole('button', { name: 'Save PNG', exact: true })).toHaveCount(0);
  await dialog(page).getByRole('button', { name: 'Save SVG', exact: true }).click();
  await expect(dialog(page).getByRole('status')).toHaveText('Saved openusage-badge-AbCdEfGhIjKlMnOp.svg in /home/synthetic/Downloads/OpenUsage');
  expect((await calls(page)).filter(c => c.command === 'save_share_image')).toEqual([{ command: 'save_share_image', args: { kind: 'badge', branding: true, format: 'svg', png: null } }]);
});
