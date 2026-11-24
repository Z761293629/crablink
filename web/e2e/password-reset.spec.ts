import { baseURL } from '../playwright.config';
import { test, expect, emailedLink, newEmail, signInPage, signOut, userMenu } from './fixtures';

test('a User who forgot their password resets it through the emailed link', async ({ page, user }) => {
  await page.goto('/app/');
  await page.getByRole('link', { name: 'Forgot password?' }).click();
  await page.getByLabel('Email').fill(user.email);
  await page.getByRole('button', { name: 'Email me a reset link' }).click();
  await expect(page.getByRole('status')).toContainText('If a User has this email');

  const link = await emailedLink(user.email, `${baseURL}/app/reset-password#`);
  await page.goto(link);
  await page.getByLabel('New password').fill('my reset password');
  await page.getByRole('button', { name: 'Set new password' }).click();
  await expect(userMenu(page)).toBeVisible();

  await signOut(page);
  await expect(page.getByRole('button', { name: 'Sign in' })).toBeVisible();
  await signInPage(page, user.email, 'my reset password');

  // The link worked once.
  await page.goto(link);
  await page.getByLabel('New password').fill('yet another password');
  await page.getByRole('button', { name: 'Set new password' }).click();
  await expect(page.getByRole('alert')).toHaveText('This reset link has expired or was already used. Ask for a new one.');
});

test('asking for a reset link for an unknown email answers the same', async ({ page }, testInfo) => {
  await page.goto('/app/forgot-password');
  await page.getByLabel('Email').fill(newEmail(testInfo.workerIndex));
  await page.getByRole('button', { name: 'Email me a reset link' }).click();
  await expect(page.getByRole('status')).toContainText('If a User has this email');
});
