import { baseURL } from '../playwright.config';
import { test, expect, inviteUrl, newEmail, signInPage } from './fixtures';

test('a new User accepts an invite and lands signed in', async ({ page, request }, testInfo) => {
  const email = newEmail(testInfo.workerIndex);
  await page.goto(await inviteUrl(request, email));

  await page.getByLabel('Choose a password').fill('short');
  await page.getByRole('button', { name: 'Accept invite' }).click();
  await expect(page.getByRole('alert')).toHaveText('The password must be at least 8 characters.');

  await page.getByLabel('Choose a password').fill('my new password');
  await page.getByRole('button', { name: 'Accept invite' }).click();
  await expect(page.getByRole('button', { name: 'Sign out' })).toBeVisible();
  // In their personal Workspace.
  await expect(page.getByLabel('Workspace', { exact: true }).locator('option:checked')).toHaveText(email);
  expect(page.url()).toBe(`${baseURL}/app/`);

  await page.reload();
  await expect(page.getByRole('button', { name: 'Sign out' })).toBeVisible();
});

test('a used invite link says so', async ({ page, request }, testInfo) => {
  const url = await inviteUrl(request, newEmail(testInfo.workerIndex));
  await page.goto(url);
  await page.getByLabel('Choose a password').fill('my new password');
  await page.getByRole('button', { name: 'Accept invite' }).click();
  await page.getByRole('button', { name: 'Sign out' }).click();

  await page.goto(url);
  await page.getByLabel('Choose a password').fill('my new password');
  await page.getByRole('button', { name: 'Accept invite' }).click();
  await expect(page.getByRole('alert')).toHaveText('This invite link has already been used.');
});

test('a User changes their password and signs in with the new one', async ({ page, user }) => {
  await signInPage(page, user.email, user.password);
  await page.getByRole('link', { name: 'Account' }).click();
  await page.getByLabel('Current password').fill('not the password');
  await page.getByLabel('New password').fill('a brand new password');
  await page.getByRole('button', { name: 'Change password' }).click();
  await expect(page.getByRole('alert')).toHaveText('The current password is wrong.');

  await page.getByLabel('Current password').fill(user.password);
  await page.getByRole('button', { name: 'Change password' }).click();
  await expect(page.getByRole('status')).toHaveText('Password changed.');

  await page.getByRole('button', { name: 'Sign out' }).click();
  await signInPage(page, user.email, 'a brand new password');
});
