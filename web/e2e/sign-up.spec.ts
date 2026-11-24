import { baseURL } from '../playwright.config';
import { test, expect, emailedLink, newEmail, userMenu } from './fixtures';

// playwright.config.ts turns open sign-up on, with terms of service and a privacy policy.

test('someone signs up, verifies their email and creates a Link', async ({ page }, testInfo) => {
  const email = newEmail(testInfo.workerIndex);
  await page.goto('/app/');
  await page.getByRole('link', { name: 'Sign up' }).click();
  await expect(page.getByRole('link', { name: 'terms of service' })).toHaveAttribute('href', `${baseURL}/terms`);
  await expect(page.getByRole('link', { name: 'privacy policy' })).toHaveAttribute('href', `${baseURL}/privacy`);
  await page.getByLabel('Email').fill(email);
  await page.getByLabel('Password').fill('my sign-up password');
  await page.getByRole('button', { name: 'Sign up' }).click();

  // Signed in, but nothing to create until the email is verified.
  await expect(userMenu(page)).toBeVisible();
  const banner = page.getByRole('complementary', { name: 'Verify your email' });
  await expect(banner).toContainText(email);
  await page.getByRole('button', { name: 'New Link' }).click();
  await page.getByLabel('Target').fill('https://example.com/unverified');
  await page.getByRole('button', { name: 'Create' }).click();
  await expect(page.getByRole('alert')).toHaveText('Verify your email first: open the link we emailed you.');

  await banner.getByRole('button', { name: 'Email me the link again' }).click();
  await expect(banner.getByRole('status')).toBeVisible();

  await page.goto(await emailedLink(email, `${baseURL}/app/verify-email#`));
  await page.getByRole('button', { name: 'Verify email' }).click();
  await expect(userMenu(page)).toBeVisible();
  await expect(banner).toHaveCount(0);

  const code = `signup-${Date.now()}`;
  await page.getByRole('button', { name: 'New Link' }).click();
  await page.getByLabel('Target').fill('https://example.com/verified');
  await page.getByText('More options').click();
  await page.getByLabel('Custom Code').fill(code);
  await page.getByRole('button', { name: 'Create' }).click();
  await expect(page.getByRole('row').filter({ hasText: `${baseURL}/${code}` })).toBeVisible();
});
