import { expect, type Page } from '@playwright/test';

export async function closeDetail(page: Page) {
  const detail = page.locator('[data-detail-panel][open]');
  if (await detail.count()) {
    await detail.locator('[data-detail-panel-close]').click();
    await expect(detail).toHaveCount(0);
  }
}

// Selects one account; its card above the accounts table holds that account's numbers.
export async function openAccount(page: Page, profileId: string) {
  await closeDetail(page);
  await page.locator(`[data-overview-account="${profileId}"] .overview-account-link`).click();
  await expect(page.locator(`[data-overview-account-card="${profileId}"]`)).toBeVisible();
}

// Source, time, connection and identity for the selected account live in its Reading details panel.
export async function openReadingDetails(page: Page) {
  await closeDetail(page);
  await page.locator('[data-overview-reading-details]').click();
  const panel = page.locator('[data-detail-panel][open]');
  await expect(panel).toHaveAttribute('aria-label', 'Reading details');
  return panel;
}

export async function navigate(page: Page, view: 'overview' | 'accounts' | 'usage' | 'settings') {
  await closeDetail(page);
  // Legacy test requests open account management from Settings; Accounts is not a route.
  const dialog = page.locator('[data-account-panel]');
  if (await dialog.isVisible()) {
    await dialog.locator('[data-account-panel-close]').click();
    await expect(dialog).not.toHaveAttribute('open');
    await expect(dialog).toBeHidden();
  }
  const destination = view === 'accounts' ? 'settings' : view;
  const navigation = page.getByRole('navigation', { name: 'Main', exact: true });
  await expect(navigation.locator('[data-workspace-nav="accounts"]')).toHaveCount(0);
  const link = navigation.locator(`[data-workspace-nav="${destination}"]`);
  await link.click();
  await expect(link).toHaveAttribute('aria-current', 'page');
  await expect(page.locator(`[data-workspace-view="${destination}"]`)).toBeVisible();
  for (const other of ['overview', 'usage', 'settings'].filter(item => item !== destination)) {
    await expect(page.locator(`[data-workspace-view="${other}"]`)).toBeHidden();
  }
  if (view === 'accounts') {
    await page.locator('[data-account-manager-open]').click();
    await expect(dialog).toHaveAttribute('open');
    await expect(dialog).toBeVisible();
    await expect(dialog.locator('.accounts-section')).toBeVisible();
    await expect(link).toHaveAttribute('aria-current', 'page');
  } else {
    await expect(dialog).toBeHidden();
  }
}

export async function openReadings(page: Page, profileId: string) {
  const trigger = page.locator(`[data-profile-id="${profileId}"] [data-profile-action="readings"]`);
  if (await trigger.getAttribute('aria-expanded') !== 'true') await trigger.click();
  await expect(trigger).toHaveAttribute('aria-expanded', 'true');
}


export async function openOptions(page: Page, profileId: string) {
  await openReadings(page, profileId);
  const trigger = page.locator(`[data-profile-id="${profileId}"] [data-profile-action="options"]`);
  if (await trigger.getAttribute('aria-expanded') !== 'true') await trigger.click();
  await expect(trigger).toHaveAttribute('aria-expanded', 'true');
}
