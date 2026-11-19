import { instanceAdmin } from '../playwright.config';
import { test, expect, openUserMenu, openWorkspacePage, signInPage } from './fixtures';

test('an Owner adds a Domain and sees it pending, an Instance Admin approves it, and the Owner creates a Link on it', async ({
  page,
  browser,
  user,
}) => {
  const custom = `go-${Date.now()}.example.com`;
  await signInPage(page, user.email, user.password);
  await openWorkspacePage(page, 'Domains');
  await page.getByLabel('Domain').fill(custom);
  await page.getByRole('button', { name: 'Add Domain' }).click();
  const domain = page.getByRole('row').filter({ hasText: custom });
  await expect(domain).toContainText('Pending');
  // Where to point its DNS: at the Default Domain.
  await expect(domain).toContainText('CNAME localhost');

  const adminPage = await (await browser.newContext()).newPage();
  await signInPage(adminPage, instanceAdmin.email, instanceAdmin.password);
  await openUserMenu(adminPage);
  await adminPage.getByRole('link', { name: 'Instance Admin' }).click();
  const managed = adminPage.getByRole('row').filter({ hasText: custom });
  await managed.getByRole('button', { name: 'Approve' }).click();
  await expect(managed).toContainText('Approved');

  await page.reload();
  await expect(domain).toContainText('Approved');
  await page.getByRole('link', { name: 'Links' }).click();
  await page.getByRole('button', { name: 'New Link' }).click();
  await page.getByLabel('Target').fill('https://example.com/branded');
  await page.getByText('More options').click();
  await page.getByLabel('Code').fill('sale');
  await page.getByLabel('Domain').selectOption(custom);
  await page.getByRole('button', { name: 'Create' }).click();
  const link = page.getByRole('row').filter({ hasText: 'https://example.com/branded' });
  await expect(link.getByRole('cell').first()).toHaveText(`http://${custom}/sale`);
});
