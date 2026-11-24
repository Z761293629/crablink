import { baseURL, instanceAdmin } from '../playwright.config';
import { test, expect, confirmDialog, openUserMenu, signInPage } from './fixtures';

test('a visitor reports a Short URL, and an Instance Admin disables its Owner and blocks it', async ({
  page,
  browser,
  user,
}) => {
  const code = `phish-${Date.now()}`;
  const shortUrl = `${baseURL}/${code}`;
  await user.api('POST', `/workspaces/${user.slug}/links`, { code, target: 'https://evil.example/login' });

  const visitor = await (await browser.newContext()).newPage();
  await visitor.goto('/report');
  await visitor.getByLabel('Short URL').fill(shortUrl);
  await visitor.getByLabel('Reason').fill('Asks for my bank password');
  await visitor.getByRole('button', { name: 'Send report' }).click();
  await expect(visitor.getByRole('status')).toHaveText('Thank you. An Instance Admin will look at it.');

  await signInPage(page, instanceAdmin.email, instanceAdmin.password);
  await openUserMenu(page);
  await page.getByRole('link', { name: 'Instance Admin' }).click();
  const report = page.getByRole('table', { name: 'Reports' }).getByRole('row').filter({ hasText: code });
  await expect(report).toContainText('https://evil.example/login');
  await expect(report).toContainText('Asks for my bank password');
  await expect(report).toContainText(user.slug);

  const owner = report.getByRole('listitem').filter({ hasText: user.email });
  await expect(owner).toContainText('Owner');
  await owner.getByRole('button', { name: 'Disable' }).click();
  await confirmDialog(page);
  await expect(owner).toContainText('Disabled');

  await report.getByRole('button', { name: 'Block Link' }).click();
  await expect(page.getByRole('dialog')).toContainText(`Block ${shortUrl}?`);
  await confirmDialog(page);
  await expect(report).toHaveCount(0);
  await expect(page.getByRole('table', { name: 'Blocked Links' })).toContainText(code);
  expect((await page.request.get(shortUrl, { maxRedirects: 0 })).status()).toBe(403);
});

test('an Instance Admin dismisses a report, and its Link keeps redirecting', async ({ page, user }) => {
  const code = `fine-${Date.now()}`;
  const shortUrl = `${baseURL}/${code}`;
  await user.api('POST', `/workspaces/${user.slug}/links`, { code, target: 'https://fine.example/' });
  const sent = await page.request.post('/api/reports', { data: { short_url: shortUrl, reason: 'I do not like it' } });
  expect(sent.status()).toBe(204);

  await signInPage(page, instanceAdmin.email, instanceAdmin.password);
  await page.goto('/app/admin');
  const report = page.getByRole('table', { name: 'Reports' }).getByRole('row').filter({ hasText: code });
  await report.getByRole('button', { name: 'Dismiss' }).click();
  await expect(report).toHaveCount(0);
  expect((await page.request.get(shortUrl, { maxRedirects: 0 })).status()).toBe(302);
});
