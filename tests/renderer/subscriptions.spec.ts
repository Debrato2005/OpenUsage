import { expect, test, type Page } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';
import type { Bootstrap, ProfileRecord, SubscriptionInput, SubscriptionRecord, SubscriptionsView } from '../../apps/desktop/src/contracts';
import { navigate } from './navigation';

// Synthetic renderer IPC only. Rust and native journeys own storage and exact currency validation.
const ids = ['11111111-1111-4111-8111-111111111111', '22222222-2222-4222-8222-222222222222'];
const recordId = '33333333-3333-4333-8333-333333333333';
const revision = '44444444-4444-4444-8444-444444444444';
const profiles: ProfileRecord[] = ids.map((id, index) => ({ id, label: `Synthetic account ${index + 1}`, environment: 'native', archived: false, connectionState: 'authentication_required' }));
const initial: SubscriptionRecord = {
  id: recordId, profileId: ids[0]!, provider: 'openai', plan: 'Synthetic Plus', tier: null, currency: 'USD', price: null,
  amountMinor: null, currencyScale: 2, intervalUnit: null, intervalCount: null, startDate: null, renewalDate: null,
  endDate: null, cancellationDate: null, autoRenew: null, archived: false, revision, source: 'user_confirmed',
  createdAtUtc: '2026-10-22T00:00:00.000Z', confirmedAtUtc: '2026-10-22T00:00:00.000Z', updatedAtUtc: '2026-10-22T00:00:00.000Z',
};
type Call = { command: string; args: Record<string, unknown> };
type Deferred = { resolve: (value: unknown) => void; reject: (error: unknown) => void; profileId: string };
type Fixture = {
  calls: Call[]; records: SubscriptionRecord[]; holdLists: boolean; holdWrites: boolean;
  lists: Deferred[]; writes: Deferred[]; error: string | null; nextCursor: string | null;
};
type FixtureWindow = Window & { subscriptionFixture: Fixture };

async function setup(page: Page, records: SubscriptionRecord[] = [initial], theme: 'light' | 'dark' = 'light') {
  await page.addInitScript(({ profiles, records, theme, revision }) => {
    const state: Fixture = { calls: [], records, holdLists: false, holdWrites: false, lists: [], writes: [], error: null, nextCursor: null };
    Object.assign(window, { subscriptionFixture: state });
    const bootstrap: Bootstrap = { storage: 'ready', theme, schemaVersion: 9, activeProfiles: profiles.length, monitoring: 'stopped', provider: 'not_configured', keyProtection: 'os_store', storageIssue: null };
    window.__TAURI__ = { core: { invoke: async <T>(command: string, args: Record<string, unknown> = {}) => {
      state.calls.push({ command, args });
      if (command === 'bootstrap') return bootstrap as T;
      if (command === 'get_display_scale') return { percent: 100 } as T;
      if (command === 'list_profiles') return { profiles, activeProfiles: profiles.length } as T;
      if (command === 'load_monitoring') return { state: 'stopped', readingRevision: revision, profiles: [] } as T;
      if (command === 'load_profile_refresh_status') return { profileId: args.profileId, retryAfterSeconds: 0 } as T;
      if (['load_profile_quota', 'load_profile_activity', 'load_profile_heatmap'].includes(command)) return null as T;
      if (command === 'list_subscriptions') {
        if (state.holdLists) return await new Promise<unknown>((resolve, reject) => state.lists.push({ resolve, reject, profileId: args.profileId as string })) as T;
        return { profileId: args.profileId, records: state.records.filter(record => record.profileId === args.profileId), nextCursor: state.nextCursor, currencies: [{ code: 'USD', scale: 2 }, { code: 'BHD', scale: 3 }, { code: 'JPY', scale: 0 }] } as T;
      }
      if (['create_subscription', 'update_subscription', 'set_subscription_archived', 'delete_subscription'].includes(command)) {
        if (state.holdWrites) return await new Promise<unknown>((resolve, reject) => state.writes.push({ resolve, reject, profileId: args.profileId as string })) as T;
        if (state.error) throw state.error;
        const previous = state.records.find(record => record.id === args.subscriptionId);
        if (command === 'delete_subscription') {
          state.records = state.records.filter(record => record.id !== args.subscriptionId);
          return { deleted: true, subscriptionId: args.subscriptionId, profileId: args.profileId } as T;
        }
        const input = args.input as SubscriptionInput;
        const saved: SubscriptionRecord = command === 'set_subscription_archived'
          ? { ...previous!, archived: args.archived as boolean, revision: '55555555-5555-4555-8555-555555555555' }
          : { ...records[0]!, id: previous?.id ?? '66666666-6666-4666-8666-666666666666', profileId: args.profileId as string, ...input,
            amountMinor: input.price === null ? null : input.price.replace('.', ''), currencyScale: input.currency === 'BHD' ? 3 : input.currency === 'JPY' ? 0 : 2,
            archived: previous?.archived ?? false, revision: '55555555-5555-4555-8555-555555555555' };
        state.records = [...state.records.filter(record => record.id !== saved.id), saved]; return saved as T;
      }
      throw new Error(`Unexpected synthetic command ${command}`);
    } } };
  }, { profiles, records: records.length ? records : [initial], theme, revision });
  if (!records.length) await page.addInitScript(() => {
    window.addEventListener('DOMContentLoaded', () => { (window as unknown as FixtureWindow).subscriptionFixture.records = []; });
  });
  await page.goto('/'); await navigate(page, 'settings');
}
const panel = (page: Page) => page.locator('[data-subscriptions]');
const toggle = (page: Page) => page.locator('[data-subscriptions-toggle]');
const form = (page: Page) => panel(page).getByRole('form');
const row = (page: Page, id = recordId) => page.locator(`[data-subscription-record="${id}"]`);
const result = (page: Page) => panel(page).locator('[data-subscriptions-result]');
async function calls(page: Page, command?: string) {
  return page.evaluate(command => (window as unknown as FixtureWindow).subscriptionFixture.calls.filter(call => command ? call.command === command : /^(create|update|set_subscription_archived|delete)_subscription/.test(call.command)), command);
}
async function choose(page: Page, name: string, option: string) {
  await panel(page).locator(`[data-subscription-select="${name}"]`).click();
  await page.getByRole('option', { name: option, exact: true }).click();
}
async function expand(page: Page) {
  await toggle(page).focus(); await page.keyboard.press('Enter');
  await expect(panel(page).locator('[data-subscription-add]')).toBeEnabled();
}
async function publishList(page: Page, index: number, profileId: string, records: SubscriptionRecord[]) {
  await expect.poll(() => page.evaluate(() => (window as unknown as FixtureWindow).subscriptionFixture.lists.length)).toBeGreaterThan(index);
  const value: SubscriptionsView = { profileId, records, nextCursor: null, currencies: [{ code: 'USD', scale: 2 }] };
  await page.evaluate(({ index, value }) => (window as unknown as FixtureWindow).subscriptionFixture.lists[index]!.resolve(value), { index, value });
}

test('collapsed panel loads on demand and saves exact text, optional dates and explicit interval for its account', async ({ page }) => {
  await setup(page); expect(await calls(page, 'list_subscriptions')).toEqual([]);
  await expand(page); await expect(row(page)).toContainText('Price not entered');
  await panel(page).locator('[data-subscription-add]').focus(); await page.keyboard.press('Enter');
  await expect(form(page).getByLabel('Plan', { exact: true })).toBeFocused();
  await form(page).getByLabel('Plan', { exact: true }).fill('Large exact plan');
  await form(page).getByLabel('Price', { exact: true }).fill('9007199254740993.01');
  await choose(page, 'interval', 'Month'); await form(page).getByLabel('Interval count').fill('3');
  await form(page).getByLabel('Renewal date').fill('2026-12-01');
  await panel(page).getByText('Dates and details', { exact: true }).click();
  await form(page).getByLabel('Tier', { exact: true }).fill('Individual');
  await form(page).getByLabel('Start date').fill('2026-10-22');
  await form(page).getByLabel('End date').fill('2027-10-22');
  await form(page).getByLabel('Cancellation deadline').fill('2026-11-25');
  await choose(page, 'autoRenew', 'No');
  await form(page).locator('[data-subscription-save]').focus(); await page.keyboard.press('Enter');
  await expect(result(page)).toHaveText('Subscription record saved.'); await expect(result(page)).toBeFocused();
  const saved = await calls(page, 'create_subscription');
  expect(saved).toHaveLength(1); expect(saved[0]!.args).toEqual({ profileId: ids[0], input: {
    plan: 'Large exact plan', tier: 'Individual', currency: 'USD', price: '9007199254740993.01', intervalUnit: 'month', intervalCount: 3,
    startDate: '2026-10-22', renewalDate: '2026-12-01', endDate: '2027-10-22', cancellationDate: '2026-11-25', autoRenew: false,
  } });
  await expect(panel(page)).toContainText('USD 9007199254740993.01');
  await expect(panel(page)).toContainText('Renews 2026-12-01');
});

test('edit retains account and revision while null price stays distinct from exact zero and one-time intervals clear count', async ({ page }) => {
  await setup(page, [{ ...initial, price: '0.00', amountMinor: '0', intervalUnit: 'month', intervalCount: 1 }]); await expand(page);
  await expect(row(page)).toContainText('USD 0.00'); await row(page).locator('[data-subscription-edit]').click();
  await expect(form(page).getByLabel('Price', { exact: true })).toHaveValue('0.00');
  await form(page).getByLabel('Price', { exact: true }).fill(''); await choose(page, 'interval', 'Once');
  await expect(form(page).getByLabel('Interval count')).toHaveCount(0); await choose(page, 'currency', 'JPY');
  await form(page).locator('[data-subscription-save]').click(); await expect(result(page)).toContainText('saved');
  expect((await calls(page, 'update_subscription'))[0]!.args).toMatchObject({ profileId: ids[0], subscriptionId: recordId, expectedRevision: revision, input: { price: null, currency: 'JPY', intervalUnit: 'once', intervalCount: null } });
  await expect(row(page)).toContainText('Price not entered');
  await choose(page, 'account', 'Synthetic account 2'); await expect(row(page)).toHaveCount(0);
  await expect(panel(page)).toContainText('No subscription records for this account.');
});

test('archive and unarchive preserve records and deletion requires keyboard confirmation', async ({ page }) => {
  await setup(page); await expand(page);
  await row(page).locator('[data-subscription-archive]').click(); await expect(row(page)).toContainText('Archived');
  await row(page).getByRole('button', { name: 'Unarchive', exact: true }).click();
  await expect(row(page)).not.toContainText('Archived');
  expect((await calls(page, 'set_subscription_archived')).map(call => call.args.archived)).toEqual([true, false]);
  await row(page).locator('[data-subscription-delete]').focus(); await page.keyboard.press('Enter');
  const dialog = panel(page).getByRole('alertdialog');
  await expect(dialog.locator('[data-subscription-delete-cancel]')).toBeFocused();
  await page.keyboard.press('Escape'); await expect(dialog).toBeHidden();
  await expect(row(page).locator('[data-subscription-delete]')).toBeFocused(); expect(await calls(page, 'delete_subscription')).toEqual([]);
  await page.keyboard.press('Enter'); await page.keyboard.press('Tab');
  await expect(dialog.locator('[data-subscription-delete-confirm]')).toBeFocused(); await page.keyboard.press('Enter');
  await expect(dialog).toBeHidden(); await expect(row(page)).toHaveCount(0); await expect(result(page)).toBeFocused();
  expect((await calls(page, 'delete_subscription')).map(call => call.args)).toEqual([{ profileId: ids[0], subscriptionId: recordId, expectedRevision: '55555555-5555-4555-8555-555555555555', confirmed: true }]);
});

test('failed edits retain form values, show a safe focused error and reload clears stale revision', async ({ page }) => {
  await setup(page); await expand(page); await row(page).locator('[data-subscription-edit]').click();
  await form(page).getByLabel('Price', { exact: true }).fill('123.45');
  await page.evaluate(() => { (window as unknown as FixtureWindow).subscriptionFixture.error = 'subscription_changed'; });
  await form(page).locator('[data-subscription-save]').click();
  await expect(result(page)).toContainText('This record changed. Reload records'); await expect(result(page)).toBeFocused();
  await expect(form(page).getByLabel('Price', { exact: true })).toHaveValue('123.45');
  await page.evaluate(() => { (window as unknown as FixtureWindow).subscriptionFixture.error = 'SYNTHETIC-PRIVATE-MARKER'; });
  await form(page).locator('[data-subscription-save]').click();
  await expect(result(page)).toContainText('could not load or save'); await expect(panel(page)).not.toContainText('SYNTHETIC-PRIVATE-MARKER');
  await panel(page).locator('[data-subscriptions-reload]').click(); await expect(form(page)).toHaveCount(0);
  await expect(row(page)).toContainText('Price not entered');
});

test('late profile list responses cannot publish into another selected account or a reopened panel', async ({ page }) => {
  await setup(page); await page.evaluate(() => { (window as unknown as FixtureWindow).subscriptionFixture.holdLists = true; });
  const length = () => page.evaluate(() => (window as unknown as FixtureWindow).subscriptionFixture.lists.length);
  async function publish(start: number, profileId: string, plan: string) {
    await expect.poll(length).toBeGreaterThan(start);
    const value: SubscriptionsView = { profileId, records: [{ ...initial, profileId, plan }], nextCursor:null, currencies:[{code:'USD',scale:2}] };
    await page.evaluate(({start,profileId,value}) => {
      (window as unknown as FixtureWindow).subscriptionFixture.lists.forEach((item,index) => { if(index>=start && item.profileId===profileId) item.resolve(value); });
    },{start,profileId,value});
  }
  await toggle(page).click(); await expect.poll(length).toBeGreaterThan(0);
  await choose(page, 'account', 'Synthetic account 2');
  await publish(0, ids[1]!, 'Second account record'); await expect(panel(page)).toContainText('Second account record');
  await publish(0, ids[0]!, 'Stale first account record'); await expect(panel(page)).not.toContainText('Stale first account record');
  const closedIndex = await length();
  await panel(page).locator('[data-subscriptions-reload]').click(); await expect.poll(length).toBeGreaterThan(closedIndex);
  const reopenIndex = await length();
  await toggle(page).click(); await toggle(page).click();
  await publish(reopenIndex, ids[1]!, 'Reopened record'); await expect(panel(page)).toContainText('Reopened record');
  await publishList(page, closedIndex, ids[1]!, [{ ...initial, profileId:ids[1]!, plan:'Stale closed record' }]);
  await expect(panel(page)).not.toContainText('Stale closed record');
});

test('closing a pending save releases only its own busy state and ignores the late reply', async ({ page }) => {
  await setup(page); await expand(page); await page.evaluate(() => { (window as unknown as FixtureWindow).subscriptionFixture.holdWrites = true; });
  await row(page).locator('[data-subscription-edit]').click(); await form(page).getByLabel('Plan', { exact: true }).fill('Stale saved plan');
  await form(page).locator('[data-subscription-save]').click();
  await expect.poll(() => page.evaluate(() => (window as unknown as FixtureWindow).subscriptionFixture.writes.length)).toBe(1);
  await form(page).evaluate(node => node.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true })));
  expect(await calls(page, 'update_subscription')).toHaveLength(1);
  await toggle(page).click(); await expand(page); await choose(page, 'account', 'Synthetic account 2');
  await page.evaluate(initial => (window as unknown as FixtureWindow).subscriptionFixture.writes[0]!.resolve({ ...initial, plan: 'Stale saved plan' }), initial);
  await expect(panel(page).locator('[data-subscription-add]')).toBeEnabled(); await expect(panel(page)).not.toContainText('Stale saved plan');
  await expect(panel(page).locator('[data-subscription-select="account"]')).toBeEnabled();
});

test('pagination sends its opaque cursor with the account and preserves exact canonical BHD prices', async ({ page }) => {
  const cursor = '77777777-7777-4777-8777-777777777777';
  const firstPage = Array.from({length:50}, (_, index) => ({ ...initial, id: index === 0 ? recordId : index === 49 ? cursor : `00000000-0000-4000-8000-${String(index).padStart(12,'0')}`, currency: 'BHD', currencyScale: 3, price: '1.001', amountMinor: '1001' }));
  await setup(page, firstPage);
  await page.evaluate(cursor => { (window as unknown as FixtureWindow).subscriptionFixture.nextCursor = cursor; }, cursor);
  await expand(page); await expect(row(page)).toContainText('BHD 1.001');
  await page.evaluate(() => { const fixture = (window as unknown as FixtureWindow).subscriptionFixture; fixture.records = [{ ...fixture.records[0]!, id: '88888888-8888-4888-8888-888888888888', plan: 'Next page record' }]; fixture.nextCursor = null; });
  await panel(page).locator('[data-subscriptions-more]').click(); await expect(panel(page)).toContainText('Next page record');
  await expect(row(page)).toContainText('BHD 1.001'); await expect(panel(page).locator('[data-subscriptions-more]')).toHaveCount(0);
  expect((await calls(page, 'list_subscriptions')).filter(call => call.args.afterId !== null).map(call => call.args)).toEqual([{ profileId: ids[0], afterId: cursor }]);
});

for (const theme of ['light', 'dark'] as const) test(`form labels and keyboard focus survive ${theme} theme and 200% text`, async ({ page }) => {
  await setup(page, [initial], theme); await expand(page);
  await page.setViewportSize({ width: 430, height: 900 });
  await page.addStyleTag({ content: 'html { font-size: 200% !important; }' });
  await row(page).locator('[data-subscription-edit]').click();
  await expect(form(page).getByLabel('Plan', { exact: true })).toBeFocused();
  await page.keyboard.press('Tab'); await expect(form(page).getByLabel('Price', { exact: true })).toBeFocused();
  await page.keyboard.press('Tab'); await expect(form(page).getByRole('combobox', { name: 'Currency', exact: true })).toBeFocused();
  await page.keyboard.press('Enter'); await page.keyboard.press('Escape');
  await expect(form(page).getByRole('combobox', { name: 'Currency', exact: true })).toBeFocused();
  await expect(panel(page)).toContainText('Local records you confirm.');
  const accessibility = await new AxeBuilder({ page }).include('[data-subscriptions]').analyze();
  expect(accessibility.violations).toEqual([]);
  const overflow = await panel(page).evaluate(node => node.scrollWidth > node.clientWidth + 1);
  expect(overflow).toBe(false);
});
