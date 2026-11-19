import { baseURL } from '../playwright.config';
import { test, expect, signInPage } from './fixtures';

test('a User creates a Link with a Custom Code, edits its Target, follows it and deletes it', async ({
  page,
  user,
}) => {
  const code = `e2e-${Date.now()}`;
  const shortUrl = `${baseURL}/${code}`;
  await signInPage(page, user.email, user.password);

  await page.getByLabel('Target').fill('https://example.com/first');
  await page.getByLabel('Custom Code').fill(code);
  await page.getByRole('button', { name: 'Create' }).click();
  const row = page.getByRole('row').filter({ hasText: shortUrl });
  await expect(row).toContainText('https://example.com/first');

  await row.getByRole('button', { name: 'Edit' }).click();
  await row.getByLabel('Target').fill('https://example.com/second');
  await row.getByLabel('Expiry').fill('2030-01-02T03:04');
  await row.getByRole('button', { name: 'Save' }).click();
  await expect(row).toContainText('https://example.com/second');
  await expect(row).toContainText('Jan 2, 2030, 3:04 AM');

  // Followed without leaving for example.com.
  const followed = await page.request.get(shortUrl, { maxRedirects: 0 });
  expect(followed.status()).toBe(302);
  expect(followed.headers().location).toBe('https://example.com/second');

  page.once('dialog', (dialog) => dialog.dismiss());
  await row.getByRole('button', { name: 'Delete' }).click();
  await expect(row).toHaveCount(1);

  page.once('dialog', (dialog) => dialog.accept());
  await row.getByRole('button', { name: 'Delete' }).click();
  await expect(row).toHaveCount(0);
  expect((await page.request.get(shortUrl, { maxRedirects: 0 })).status()).toBe(410);
});

test('taking a Code that is already taken says so, in either language', async ({ page, user }) => {
  const code = `taken-${Date.now()}`;
  await user.api('POST', `/workspaces/${user.slug}/links`, { target: 'https://example.com/first', code });
  await signInPage(page, user.email, user.password);

  await page.getByLabel('Target').fill('https://example.com/second');
  await page.getByLabel('Custom Code').fill(code);
  await page.getByRole('button', { name: 'Create' }).click();
  await expect(page.getByRole('alert')).toHaveText('That Code is already taken.');

  await page.getByLabel('Language').selectOption('zh-CN');
  await page.getByRole('button', { name: '创建' }).click();
  await expect(page.getByRole('alert')).toHaveText('该短码已被占用。');
});

test("switching Workspace shows that Workspace's Links", async ({ page, user }) => {
  const team = `Team ${Date.now()}`;
  await user.api('POST', `/workspaces/${user.slug}/links`, { target: 'https://example.com/personal' });
  const { slug } = (await user.api('POST', '/workspaces', { name: team })) as { slug: string };
  await user.api('POST', `/workspaces/${slug}/links`, { target: 'https://example.com/team' });
  await signInPage(page, user.email, user.password);

  const links = page.getByRole('table');
  await page.getByLabel('Workspace').selectOption({ label: user.email });
  await expect(links).toContainText('https://example.com/personal');
  await expect(links).not.toContainText('https://example.com/team');

  await page.getByLabel('Workspace').selectOption({ label: team });
  await expect(links).toContainText('https://example.com/team');
  await expect(links).not.toContainText('https://example.com/personal');
});

test("a Link's Short URL copies in one click, and its QR code downloads as SVG or PNG", async ({
  page,
  context,
  user,
}) => {
  const code = `qr-${Date.now()}`;
  await user.api('POST', `/workspaces/${user.slug}/links`, { target: 'https://example.com/qr', code });
  await context.grantPermissions(['clipboard-read', 'clipboard-write']);
  await signInPage(page, user.email, user.password);
  const row = page.getByRole('row').filter({ hasText: `${baseURL}/${code}` });

  await row.getByRole('button', { name: 'Copy' }).click();
  await expect(row.getByRole('button', { name: 'Copied' })).toBeVisible();
  expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(`${baseURL}/${code}`);

  for (const format of ['svg', 'png']) {
    const download = page.waitForEvent('download');
    await row.getByRole('link', { name: `QR code (${format.toUpperCase()})` }).click();
    const file = await download;
    expect(file.suggestedFilename()).toBe(`${code}.${format}`);
    expect((await file.failure()) ?? null).toBeNull();
  }
});

test('a User tags two Links, filters by one Tag and searches by part of a Target', async ({ page, user }) => {
  const code = `autumn-${Date.now()}`;
  await user.api('POST', `/workspaces/${user.slug}/links`, { target: 'https://example.com/autumn-sale', code });
  await user.api('POST', `/workspaces/${user.slug}/links`, { target: 'https://example.com/untagged' });
  await signInPage(page, user.email, user.password);

  await page.getByLabel('Target').fill('https://example.com/spring-sale');
  await page.getByLabel('Tags').fill('spring, launch');
  await page.getByRole('button', { name: 'Create' }).click();
  const spring = page.getByRole('row').filter({ hasText: 'https://example.com/spring-sale' });
  await expect(spring).toContainText('launch');
  await expect(spring).toContainText('spring');

  const autumn = page.getByRole('row').filter({ hasText: `${baseURL}/${code}` });
  await autumn.getByRole('button', { name: 'Edit' }).click();
  await autumn.getByLabel('Tags').fill('autumn');
  await autumn.getByRole('button', { name: 'Save' }).click();
  await expect(autumn).toContainText('autumn');

  const rows = page.getByRole('table').getByRole('row');
  await page.getByLabel('Tag', { exact: true }).selectOption('spring');
  await expect(rows).toHaveCount(1 + 1);
  await expect(rows.nth(1)).toContainText('https://example.com/spring-sale');

  await page.getByLabel('Tag', { exact: true }).selectOption('');
  await page.getByLabel('Search').fill('AUTUMN-SA');
  await expect(rows).toHaveCount(1 + 1);
  await expect(rows.nth(1)).toContainText('https://example.com/autumn-sale');
});

test('more Links load a page at a time', async ({ page, user }) => {
  for (let i = 0; i < 51; i++) {
    await user.api('POST', `/workspaces/${user.slug}/links`, { target: `https://example.com/${i}` });
  }
  await signInPage(page, user.email, user.password);

  const rows = page.getByRole('table').getByRole('row');
  await expect(rows).toHaveCount(1 + 50);
  await expect(rows.nth(1)).toContainText('https://example.com/50');
  await page.getByRole('button', { name: 'Load more' }).click();
  await expect(rows).toHaveCount(1 + 51);
  await expect(rows.last()).toContainText('https://example.com/0');
  await expect(page.getByRole('button', { name: 'Load more' })).toBeHidden();
});
