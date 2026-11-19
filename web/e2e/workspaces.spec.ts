import { test, expect, openWorkspaceMenu, openWorkspacePage, signInPage, switcher, switchWorkspace } from './fixtures';

test.beforeEach(({ page }) => {
  // Accepts the "Are you sure?" of leaving, removing, revoking and deleting.
  page.on('dialog', (dialog) => dialog.accept());
});

test('an Owner creates a Workspace, adds a Member, promotes them, then leaves', async ({ page, user, other }) => {
  await signInPage(page, user.email, user.password);
  await openWorkspacePage(page, 'Members & API Keys');
  await page.getByLabel('New Workspace name').fill('Marketing team');
  await page.getByRole('button', { name: 'Create Workspace' }).click();
  await expect(page.getByRole('heading', { name: 'Marketing team' })).toBeVisible();
  // The new Workspace is the current one.
  await expect(switcher(page)).toContainText('Marketing team');
  const created = (await user.api('GET', '/workspaces')) as { slug: string; name: string }[];
  const slug = created.find((w) => w.name === 'Marketing team')!.slug;
  expect(slug).toMatch(/^marketing-team/);

  await page.getByLabel("Member's email").fill(other.email);
  await page.getByRole('button', { name: 'Add member' }).click();
  const members = page.getByRole('table', { name: 'Members' });
  const added = members.getByRole('row').filter({ hasText: other.email });
  await expect(added.getByRole('cell').nth(1)).toHaveText('Member');

  // The last Owner can't leave.
  await page.getByRole('button', { name: 'Leave Workspace' }).click();
  await expect(page.getByRole('alert')).toHaveText('A Workspace needs an Owner: make someone else Owner first.');

  await added.getByRole('button', { name: 'Make Owner' }).click();
  await expect(added.getByRole('cell').nth(1)).toHaveText('Owner');
  await page.getByRole('button', { name: 'Leave Workspace' }).click();

  await expect(await openWorkspaceMenu(page)).toHaveText([user.email]);
  expect(await other.api('GET', '/workspaces')).toContainEqual({ slug, name: 'Marketing team', role: 'owner' });
});

test('an Owner renames a Workspace, demotes and removes a Member, then deletes it', async ({ page, user, other }) => {
  const { slug } = (await user.api('POST', '/workspaces', { name: 'Old name' })) as { slug: string };
  await user.api('POST', `/workspaces/${slug}/members`, { email: other.email });
  await user.api('PATCH', `/workspaces/${slug}/members/${other.email}`, { role: 'owner' });

  await signInPage(page, user.email, user.password);
  await switchWorkspace(page, 'Old name');
  await openWorkspacePage(page, 'Members & API Keys');
  await page.getByLabel('Name', { exact: true }).fill('New name');
  await page.getByRole('button', { name: 'Rename' }).click();
  await expect(page.getByRole('heading', { name: 'New name' })).toBeVisible();

  const row = page.getByRole('table', { name: 'Members' }).getByRole('row').filter({ hasText: other.email });
  await row.getByRole('button', { name: 'Make Member' }).click();
  await expect(row.getByRole('cell').nth(1)).toHaveText('Member');
  await row.getByRole('button', { name: 'Remove' }).click();
  await expect(row).toHaveCount(0);
  expect(await other.api('GET', '/workspaces')).toHaveLength(1);

  await page.getByRole('button', { name: 'Delete Workspace' }).click();
  await expect(await openWorkspaceMenu(page)).toHaveText([user.email]);
  expect(await user.api('GET', '/workspaces')).toHaveLength(1);
});

test('an Owner creates an API Key, a program uses it, and revoking it stops it working', async ({
  page,
  context,
  request,
  user,
}) => {
  await context.grantPermissions(['clipboard-read', 'clipboard-write']);
  await signInPage(page, user.email, user.password);
  await openWorkspacePage(page, 'Members & API Keys');
  await page.getByLabel('API Key name').fill('deploy script');
  await page.getByRole('button', { name: 'Create API Key' }).click();

  const secret = await page.getByLabel('Secret').inputValue();
  expect(secret).toMatch(/^crab_./);
  await page.getByRole('button', { name: 'Copy' }).click();
  expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(secret);

  const listLinks = () =>
    request.get(`/api/workspaces/${user.slug}/links`, { headers: { authorization: `Bearer ${secret}` } });
  expect((await listLinks()).status()).toBe(200);

  const row = page.getByRole('table', { name: 'API Keys' }).getByRole('row').filter({ hasText: 'deploy script' });
  await expect(row.getByRole('cell').nth(1)).toHaveText(`${secret.slice(0, 12)}…`);
  await row.getByRole('button', { name: 'Revoke' }).click();
  await expect(row).toHaveCount(0);
  await expect(page.getByLabel('Secret')).toHaveCount(0);
  expect((await listLinks()).status()).toBe(401);
});

test('an Owner demotes themself, then as a Member sees no Owner tools and can leave', async ({
  page,
  user,
  other,
}) => {
  const { slug } = (await other.api('POST', '/workspaces', { name: 'Their team' })) as { slug: string };
  await other.api('POST', `/workspaces/${slug}/members`, { email: user.email });
  await other.api('PATCH', `/workspaces/${slug}/members/${user.email}`, { role: 'owner' });

  await signInPage(page, user.email, user.password);
  await switchWorkspace(page, 'Their team');
  await openWorkspacePage(page, 'Members & API Keys');
  await expect(page.getByRole('heading', { name: 'Their team' })).toBeVisible();
  const self = page.getByRole('table', { name: 'Members' }).getByRole('row').filter({ hasText: user.email });
  await self.getByRole('button', { name: 'Make Member' }).click();

  await expect(page.getByRole('table')).toHaveCount(0);
  await expect(page.getByRole('alert')).toHaveCount(0);
  await expect(page.getByRole('button', { name: 'Delete Workspace' })).toHaveCount(0);

  await page.getByRole('button', { name: 'Leave Workspace' }).click();
  await expect(await openWorkspaceMenu(page)).toHaveText([user.email]);
});
