import { test, expect, userMenu } from './fixtures';

test('switching to Simplified Chinese translates the page, and a reload keeps it', async ({ page, user }) => {
  await page.goto('/app/');
  await expect(page.getByRole('heading', { name: 'Sign in' })).toBeVisible();

  await page.getByLabel('Language').selectOption('zh-CN');
  await expect(page.getByRole('heading', { name: '登录' })).toBeVisible();
  await page.reload();
  await expect(page.getByRole('heading', { name: '登录' })).toBeVisible();

  // API errors too, translated from their `kind`.
  await page.getByLabel('邮箱').fill(user.email);
  await page.getByLabel('密码').fill('not the password');
  await page.getByRole('button', { name: '登录' }).click();
  await expect(page.getByRole('alert')).toHaveText('邮箱或密码错误。');

  await page.getByLabel('密码').fill(user.password);
  await page.getByRole('button', { name: '登录' }).click();
  await userMenu(page).click();
  await expect(page.getByRole('button', { name: '退出登录' })).toBeVisible();
});
