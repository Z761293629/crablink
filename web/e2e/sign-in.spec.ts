import { baseURL } from '../playwright.config';
import { test, expect, signOut } from './fixtures';

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
  await expect(links.getByRole('columnheader')).toHaveText(['Short URL', 'Target', 'Click Count', 'Actions']);
  const expiring = links.getByRole('row').filter({ hasText: 'https://example.com/expiring' });
  // The Target cell carries the Tags and the Expiry beneath the Target.
  await expect(expiring.getByRole('cell')).toHaveText([
    `${baseURL}/${code}`,
    /^https:\/\/example\.com\/expiring\s*Expiry Jan 2, 2030$/,
    '0',
    /Copy/,
  ]);
  const forever = links.getByRole('row').filter({ hasText: 'https://example.com/forever' });
  await forever.getByRole('button').first().click();
  const expiry = page
    .getByRole('region', { name: 'Selected Link' })
    .locator('dl > div', { has: page.getByRole('term').filter({ hasText: /^Expiry$/ }) })
    .getByRole('definition');
  await expect(expiry).toHaveText('Never');

  await signOut(page);
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
