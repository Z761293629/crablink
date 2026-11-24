import { baseURL, instanceAdmin } from '../playwright.config';
import { test, expect, confirmDialog, openUserMenu, signInPage } from './fixtures';

test('an Instance Admin blocks a Link, and its Workspace sees it blocked but cannot change it', async ({
  page,
  browser,
  user,
}) => {
  const code = `phish-${Date.now()}`;
  const shortUrl = `${baseURL}/${code}`;
  await user.api('POST', `/workspaces/${user.slug}/links`, { code, target: 'https://evil.example/login' });

  await signInPage(page, instanceAdmin.email, instanceAdmin.password);
  await openUserMenu(page);
  await page.getByRole('link', { name: 'Instance Admin' }).click();
  await page.getByLabel('Short URL').fill(shortUrl);
  await page.getByRole('button', { name: 'Block Link' }).click();
  await expect(page.getByRole('dialog')).toContainText(`Block ${shortUrl}?`);
  await confirmDialog(page);
  const blocked = page.getByRole('table', { name: 'Blocked Links' }).getByRole('row').filter({ hasText: code });
  await expect(blocked).toContainText('https://evil.example/login');
  await expect(blocked).toContainText(user.slug);

  // Visitors see why instead of being redirected.
  const visit = await page.request.get(shortUrl, { maxRedirects: 0 });
  expect(visit.status()).toBe(403);
  expect(await visit.text()).toContain('blocked for abuse');

  const owner = await (await browser.newContext()).newPage();
  await signInPage(owner, user.email, user.password);
  const row = owner.getByRole('row').filter({ hasText: code });
  await expect(row).toContainText('Blocked');
  await expect(row.getByRole('link', { name: `Download QR of ${code} (PNG)` })).toHaveCount(0);
  await row.getByRole('button', { name: `More: ${code}` }).click();
  await expect(owner.getByRole('menuitem', { name: 'Delete' })).toBeVisible();
  await expect(owner.getByRole('menuitem', { name: 'Edit' })).toHaveCount(0);
  await owner.keyboard.press('Escape');
  await row.getByRole('button').first().click();
  const pass = owner.getByRole('region', { name: 'Selected Link' });
  await expect(pass).toContainText('An Instance Admin blocked this Link for abuse');
  await expect(pass.getByRole('button', { name: 'Change Target' })).toHaveCount(0);
  await expect(pass).toContainText('A Blocked Link has no QR code.');

  // Unblocked, it redirects again.
  await blocked.getByRole('button', { name: 'Unblock' }).click();
  await expect(blocked).toHaveCount(0);
  const again = await page.request.get(shortUrl, { maxRedirects: 0 });
  expect(again.status()).toBe(302);
});
