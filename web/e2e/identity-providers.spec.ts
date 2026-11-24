import { test, expect, newEmail, userMenu } from './fixtures';

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
