import { instanceAdmin } from '../playwright.config';
import { test, expect, confirmDialog, openUserMenu, openWorkspaceMenu, signInPage, userMenu } from './fixtures';

test('an Instance Admin creates an invite into a Workspace, and a new User accepts it', async ({ page, browser, user }) => {
  const email = `invited-${Date.now()}@example.com`;
  await signInPage(page, instanceAdmin.email, instanceAdmin.password);
  await openUserMenu(page);
  await page.getByRole('link', { name: 'Instance Admin' }).click();
  await page.getByLabel('Email').fill(email);
  await page.getByLabel('Workspace').fill(user.slug);
  await page.getByRole('button', { name: 'Create invite link' }).click();
  const url = await page.getByLabel('Invite link').inputValue();

  const invited = await (await browser.newContext()).newPage();
  await invited.goto(url);
  await invited.getByLabel('Choose a password').fill('invited password');
  await invited.getByRole('button', { name: 'Accept invite' }).click();
  await expect(userMenu(invited)).toBeVisible();
  await expect(invited.getByRole('banner')).toContainText(email);

  const members = (await user.api('GET', `/workspaces/${user.slug}/members`)) as { email: string; role: string }[];
  expect(members).toContainEqual({ email, role: 'member' });
});

test('a User who isn’t an Instance Admin sees no admin navigation', async ({ page, user }) => {
  await signInPage(page, user.email, user.password);
  // One menu holds the Workspace's pages and the account's.
  await openWorkspaceMenu(page);
  await expect(page.getByRole('link', { name: 'Domains' })).toBeVisible();
  await expect(page.getByRole('link', { name: 'Account' })).toBeVisible();
  await expect(page.getByRole('link', { name: 'Instance Admin' })).toHaveCount(0);

  // Nor the admin page itself.
  await page.goto('/app/admin');
  await openWorkspaceMenu(page);
  await expect(page.getByRole('link', { name: 'Domains' })).toBeVisible();
  await expect(page.getByRole('heading', { name: 'Instance administration' })).toHaveCount(0);
});

test('an Instance Admin resets a password, then disables the User', async ({ page, browser, user }) => {
  await signInPage(page, instanceAdmin.email, instanceAdmin.password);
  await openUserMenu(page);
  await page.getByRole('link', { name: 'Instance Admin' }).click();
  const row = page.getByRole('row').filter({ hasText: user.email });
  await row.getByRole('button', { name: 'Reset password' }).click();
  const password = await row.getByLabel('New password').textContent();

  const userPage = await (await browser.newContext()).newPage();
  await signInPage(userPage, user.email, password!);

  await row.getByRole('button', { name: 'Disable' }).click();
  await expect(page.getByRole('dialog')).toContainText(`Disable ${user.email}?`);
  await confirmDialog(page);
  await expect(row).toContainText('Disabled');
  await userPage.reload();
  await expect(userPage.getByRole('button', { name: 'Sign in' })).toBeVisible();
});

test('an Instance Admin is never offered to disable or revoke themselves', async ({ page }) => {
  await signInPage(page, instanceAdmin.email, instanceAdmin.password);
  await openUserMenu(page);
  await page.getByRole('link', { name: 'Instance Admin' }).click();
  const row = page.getByRole('row').filter({ hasText: instanceAdmin.email });
  await expect(row).toContainText('you');
  await expect(row.getByRole('button', { name: 'Reset password' })).toBeVisible();
  await expect(row.getByRole('button', { name: 'Revoke Instance Admin' })).toHaveCount(0);
  await expect(row.getByRole('button', { name: 'Disable' })).toHaveCount(0);
});
