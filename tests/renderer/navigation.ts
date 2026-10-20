import { expect, type Page } from '@playwright/test';

export async function navigate(page: Page, view: 'overview' | 'accounts' | 'usage' | 'settings') {
  const link = page.getByRole('navigation', { name: 'Main', exact: true }).locator(`[data-workspace-nav="${view}"]`);
  await link.click();
  await expect(link).toHaveAttribute('aria-current', 'page');
  await expect(page.locator(`[data-workspace-view="${view}"]`)).toBeVisible();
  for (const other of ['overview', 'accounts', 'usage', 'settings'].filter(item => item !== view)) {
    await expect(page.locator(`[data-workspace-view="${other}"]`)).toBeHidden();
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
