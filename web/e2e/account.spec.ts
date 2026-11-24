import { baseURL } from '../playwright.config';
import {
  test,
  expect,
  confirmDialog,
  inviteUrl,
  newEmail,
  openUserMenu,
  signInPage,
  signOut,
  switcher,
  userMenu,
} from './fixtures';

test('a new User accepts an invite and lands signed in', async ({ page, request }, testInfo) => {
  const email = newEmail(testInfo.workerIndex);
  await page.goto(await inviteUrl(request, email));

  await page.getByLabel('Choose a password').fill('short');
  await page.getByRole('button', { name: 'Accept invite' }).click();
  await expect(page.getByRole('alert')).toHaveText('The password must be at least 8 characters.');

  await page.getByLabel('Choose a password').fill('my new password');
  await page.getByRole('button', { name: 'Accept invite' }).click();
  await expect(userMenu(page)).toBeVisible();
  // In their personal Workspace.
  await expect(switcher(page)).toContainText(email);
  await expect(page).toHaveURL(`${baseURL}/app/`);

  await page.reload();
  await expect(userMenu(page)).toBeVisible();
});

test('a used invite link says so', async ({ page, request }, testInfo) => {
  const url = await inviteUrl(request, newEmail(testInfo.workerIndex));
  await page.goto(url);
  await page.getByLabel('Choose a password').fill('my new password');
  await page.getByRole('button', { name: 'Accept invite' }).click();
  await signOut(page);
  // Navigating before sign-out finishes would cancel it.
  await expect(page.getByRole('button', { name: 'Sign in' })).toBeVisible();

  await page.goto(url);
  await page.getByLabel('Choose a password').fill('my new password');
  await page.getByRole('button', { name: 'Accept invite' }).click();
  await expect(page.getByRole('alert')).toHaveText('This invite link has already been used.');
});

test('a User changes their password and signs in with the new one', async ({ page, user }) => {
  await signInPage(page, user.email, user.password);
  await openUserMenu(page);
  await page.getByRole('link', { name: 'Account' }).click();
  await page.getByLabel('Current password').fill('not the password');
  await page.getByLabel('New password').fill('a brand new password');
  await page.getByRole('button', { name: 'Change password' }).click();
  await expect(page.getByRole('alert')).toHaveText('The current password is wrong.');

  await page.getByLabel('Current password').fill(user.password);
  await page.getByRole('button', { name: 'Change password' }).click();
  await expect(page.getByRole('status')).toHaveText('Password changed.');

  await signOut(page);
  // Navigating before sign-out finishes would cancel it, leaving no sign-in form to fill.
  await expect(page.getByRole('button', { name: 'Sign in' })).toBeVisible();
  await signInPage(page, user.email, 'a brand new password');
});

test('a User deletes themself, after which they cannot sign in', async ({ page, user }) => {
  await signInPage(page, user.email, user.password);
  await openUserMenu(page);
  await page.getByRole('link', { name: 'Account' }).click();
  const password = page.getByLabel('Password', { exact: true });

  await password.fill('not the password');
  await page.getByRole('button', { name: 'Delete account' }).click();
  await expect(page.getByRole('dialog')).toContainText('cannot be undone');
  await confirmDialog(page);
  await expect(page.getByRole('alert')).toHaveText('The current password is wrong.');

  await password.fill(user.password);
  await page.getByRole('button', { name: 'Delete account' }).click();
  await confirmDialog(page);
  await expect(page.getByRole('button', { name: 'Sign in' })).toBeVisible();
  await expect(page).toHaveURL(`${baseURL}/app/`);

  await page.getByLabel('Email').fill(user.email);
  await page.getByLabel('Password').fill(user.password);
  await page.getByRole('button', { name: 'Sign in' }).click();
  await expect(page.getByRole('alert')).toHaveText('Wrong email or password.');
});
