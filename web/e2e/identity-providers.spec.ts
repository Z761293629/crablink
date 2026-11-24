import { instanceAdmin } from '../playwright.config';
import { test, expect, newEmail, openWorkspaceMenu, signInPage, switcher, userMenu } from './fixtures';

// examples/e2e.rs sets up the harness's fake Identity Provider as "Fake ID": its
// sign-in page asks for the email to sign in as, and vouches for it as verified.

test('a User signs in through an Identity Provider', async ({ page, user }) => {
  await page.goto('/app/');
  await page.getByRole('link', { name: 'Sign in with Fake ID' }).click();
  await page.getByLabel('Email').fill(user.email.toUpperCase());
  await page.getByRole('button', { name: 'Continue' }).click();

  await expect(userMenu(page)).toHaveAccessibleName(new RegExp(`^Account menu: ${user.email},`));
  await expect(page).toHaveURL(/\/app\/$/);
});

test('someone with no User is told to ask for an Invite', async ({ page }, testInfo) => {
  await page.goto('/app/');
  await page.getByRole('link', { name: 'Sign in with Fake ID' }).click();
  await page.getByLabel('Email').fill(newEmail(testInfo.workerIndex));
  await page.getByRole('button', { name: 'Continue' }).click();

  await expect(page.getByRole('alert')).toHaveText(
    'No one here has your email yet: ask an Instance Admin for an Invite.',
  );
  // The password form is still there.
  await expect(page.getByLabel('Password')).toBeVisible();
});

test('someone on an allowed email domain gets a User without a password', async ({ page, browser }, testInfo) => {
  const email = `idp-${testInfo.workerIndex}-${Date.now()}@idp.example.com`;
  await page.goto('/app/');
  await page.getByRole('link', { name: 'Sign in with Fake ID' }).click();
  await page.getByLabel('Email').fill(email);
  await page.getByRole('button', { name: 'Continue' }).click();

  await expect(switcher(page)).toContainText(email);
  await openWorkspaceMenu(page);
  await expect(page.getByRole('button', { name: 'Sign out' })).toBeVisible();
  // There is no password to change.
  await expect(page.getByRole('link', { name: 'Account' })).toHaveCount(0);

  const admin = await (await browser.newContext()).newPage();
  await signInPage(admin, instanceAdmin.email, instanceAdmin.password);
  await admin.goto('/app/admin');
  const row = admin.getByRole('row').filter({ hasText: email });
  await expect(row).toContainText('No password');
  await expect(row).toContainText('Fake ID');
});
