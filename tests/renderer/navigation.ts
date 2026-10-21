import { expect, type Page } from '@playwright/test';

export async function navigate(page: Page, view: 'overview' | 'accounts' | 'usage' | 'settings') {
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
