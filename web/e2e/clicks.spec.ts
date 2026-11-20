import { baseURL } from '../playwright.config';
import { test, expect, openUserMenu, signInPage } from './fixtures';

const FIREFOX_UA = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:130.0) Gecko/20100101 Firefox/130.0';

test("following a Short URL shows in the Link's chart and the Workspace's, in either language", async ({
  page,
  user,
}) => {
  const code = `clicks-${Date.now()}`;
  const shortUrl = `${baseURL}/${code}`;
  await user.api('POST', `/workspaces/${user.slug}/links`, { target: 'https://example.com/clicks', code });
  const followed = await page.request.get(shortUrl, {
    maxRedirects: 0,
    headers: { 'user-agent': FIREFOX_UA, referer: 'https://news.ycombinator.com/item?id=1' },
  });
  expect(followed.status()).toBe(302);
  await signInPage(page, user.email, user.password);

  await page.getByRole('row').filter({ hasText: shortUrl }).getByRole('button').first().click();
  await page.getByRole('region', { name: 'Selected Link' }).getByRole('link', { name: /View Clicks/ }).click();
  await expect(page.getByRole('heading', { name: `Clicks on ${new URL(baseURL).host}/${code}` })).toBeVisible();
  await expect(page.locator('.total')).toHaveText('1 Click');
  await expect(page.getByRole('table', { name: 'Referrers' })).toContainText('news.ycombinator.com');
  await expect(page.getByRole('table', { name: 'Devices' })).toContainText('Desktop');
  await expect(page.getByRole('table', { name: 'Browsers' })).toContainText('Firefox');
  await page.getByLabel('Time range').selectOption({ label: 'Last 24 hours' });
  await expect(page.locator('.total')).toHaveText('1 Click');

  await page.getByRole('link', { name: 'Workspace Clicks' }).click();
  await expect(page.getByRole('heading', { name: 'Workspace Clicks' })).toBeVisible();
  await expect(page.locator('.total')).toHaveText('1 Click');
  await expect(page.getByRole('row').filter({ hasText: shortUrl })).toContainText('1');

  await openUserMenu(page);
  await page.getByRole('radio', { name: '简体中文' }).check();
  await expect(page.getByRole('heading', { name: '工作区点击' })).toBeVisible();
  await expect(page.locator('.total')).toHaveText('1 次点击');
});
