import { admin } from '../playwright.config';
import { test, expect, signInPage } from './fixtures';

test('an Owner adds a Domain and sees it pending, an Instance Admin approves it, and the Owner creates a Link on it', async ({
  page,
  browser,
  user,
}) => {
  const host = `go-${Date.now()}.example.com`;
  await signInPage(page, user.email, user.password);
  await page.getByRole('link', { name: 'Domains' }).click();
  await page.getByLabel('Host').fill(host);
  await page.getByRole('button', { name: 'Add Domain' }).click();
  const domain = page.getByRole('row').filter({ hasText: host });
  await expect(domain).toContainText('Pending');
  // Where to point its DNS: at the Default Domain.
  await expect(domain).toContainText('CNAME localhost');

  const adminPage = await (await browser.newContext()).newPage();
  await signInPage(adminPage, admin.email, admin.password);
  await adminPage.getByRole('link', { name: 'Admin' }).click();
  const managed = adminPage.getByRole('row').filter({ hasText: host });
  await managed.getByRole('button', { name: 'Approve' }).click();
  await expect(managed).toContainText('Approved');

  await page.reload();
  await expect(domain).toContainText('Approved');
  await page.getByRole('link', { name: 'Links' }).click();
  await page.getByLabel('Target').fill('https://example.com/branded');
  await page.getByLabel('Code').fill('sale');
  await page.getByLabel('Domain').selectOption(host);
  await page.getByRole('button', { name: 'Create Link' }).click();
  const link = page.getByRole('row').filter({ hasText: 'https://example.com/branded' });
  await expect(link.getByRole('cell').first()).toHaveText(`http://${host}/sale`);
});
