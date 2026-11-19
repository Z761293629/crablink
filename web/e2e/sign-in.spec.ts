import { baseURL } from '../playwright.config';
import { test, expect } from './fixtures';

test('a User signs in, sees their Links and signs out', async ({ page, user }) => {
  const code = `e2e-${Date.now()}`;
  await user.api('POST', `/workspaces/${user.slug}/links`, {
    target: 'https://example.com/expiring',
    code,
    expires_at: '2030-01-02T03:04:00Z',
  });
  await user.api('POST', `/workspaces/${user.slug}/links`, { target: 'https://example.com/forever' });

  await page.goto('/app/');
  await page.getByLabel('Email').fill(user.email);
  await page.getByLabel('Password').fill(user.password);
  await page.getByRole('button', { name: 'Sign in' }).click();

  const links = page.getByRole('table');
  await expect(links.getByRole('columnheader')).toHaveText(['Short URL', 'Target', 'Click Count', 'Expiry']);
  const expiring = links.getByRole('row').filter({ hasText: 'https://example.com/expiring' });
  await expect(expiring.getByRole('cell')).toHaveText([
    `${baseURL}/${code}`,
    'https://example.com/expiring',
    '0',
    'Jan 2, 2030, 3:04 AM',
  ]);
  const forever = links.getByRole('row').filter({ hasText: 'https://example.com/forever' });
  await expect(forever.getByRole('cell').last()).toHaveText('Never');

  await page.getByRole('button', { name: 'Sign out' }).click();
  await expect(page.getByRole('button', { name: 'Sign in' })).toBeVisible();
  await page.reload();
  await expect(page.getByRole('button', { name: 'Sign in' })).toBeVisible();
});

test('a wrong password is refused', async ({ page, user }) => {
  await page.goto('/app/');
  await page.getByLabel('Email').fill(user.email);
  await page.getByLabel('Password').fill('not the password');
  await page.getByRole('button', { name: 'Sign in' }).click();

  await expect(page.getByRole('alert')).toHaveText('Wrong email or password.');
});
