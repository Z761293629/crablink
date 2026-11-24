import { readFile } from 'node:fs/promises';
import { baseURL } from '../playwright.config';
import type { Locator, Page } from '@playwright/test';
import { test, expect, confirmDialog, openUserMenu, signInPage, switchWorkspace } from './fixtures';

/** The pass of the Link picked on the board. */
const selectedPass = (page: Page) => page.getByRole('region', { name: 'Selected Link' });

/** Picks a Link's row on the board, opening its pass. */
const pick = (row: Locator) => row.getByRole('button').first().click();

/** Chooses Delete from a row's More menu, which is named after the Link's Code. */
async function deleteFrom(row: Locator, code: string) {
  await row.getByRole('button', { name: `More: ${code}` }).click();
  await row.page().getByRole('menuitem', { name: 'Delete' }).click();
}

/** Opens the New Link pass's More options: Custom Code, Tags, Expiry. */
const moreOptions = (page: Page) => page.getByText('More options').click();

/** Opens the Board's More menu and chooses `action`. */
async function boardMenu(page: Page, action: 'Export CSV' | 'Import CSV') {
  await page.getByRole('button', { name: 'CSV import and export' }).click();
  await page.getByRole('menuitem', { name: action }).click();
}

test('a User creates a Link with a Custom Code, changes its Target from the pass, follows it and deletes it', async ({
  page,
  user,
}) => {
  const code = `e2e-${Date.now()}`;
  const shortUrl = `${baseURL}/${code}`;
  await signInPage(page, user.email, user.password);

  await page.getByRole('button', { name: 'New Link' }).click();
  await page.getByLabel('Target').fill('https://example.com/first');
  await moreOptions(page);
  await page.getByLabel('Custom Code').fill(code);
  await page.getByRole('button', { name: 'Create' }).click();
  const row = page.getByRole('row').filter({ hasText: shortUrl });
  await expect(row).toContainText('https://example.com/first');
  // The new Link is the selected pass.
  const pass = selectedPass(page);
  await expect(pass).toContainText(code);

  // Changing the Target is on the pass, saying the Short URL stays.
  const change = pass.getByRole('button', { name: 'Change Target' });
  await expect(change).toHaveAccessibleDescription(/Printed QR codes keep working/);
  await change.click();
  await pass.getByLabel('Target').fill('https://example.com/second');
  await pass.getByLabel('Expiry').fill('2030-01-02T03:04');
  await pass.getByRole('button', { name: 'Save' }).click();
  // A new Target repoints every printed copy, so it says from where to where first.
  await expect(page.getByRole('dialog')).toContainText('https://example.com/first');
  await expect(page.getByRole('dialog')).toContainText('https://example.com/second');
  await confirmDialog(page);
  await expect(page.getByText(`${shortUrl} now points to https://example.com/second`)).toBeVisible();
  await expect(row).toContainText('https://example.com/second');
  // Saved: focus is back on Change Target.
  await expect(change).toBeFocused();
  // The row shows the Expiry's day; the pass carries its time.
  await expect(row).toContainText('Jan 2, 2030');
  await expect(pass).toContainText('3:04 AM');

  // Followed without leaving for example.com.
  const followed = await page.request.get(shortUrl, { maxRedirects: 0 });
  expect(followed.status()).toBe(302);
  expect(followed.headers().location).toBe('https://example.com/second');

  // The confirmation says what deleting costs; Escape backs out.
  await deleteFrom(row, code);
  const dialog = page.getByRole('dialog');
  await expect(dialog).toContainText(`can never be used again on ${new URL(baseURL).host}`);
  await page.keyboard.press('Escape');
  await expect(dialog).toHaveCount(0);
  await expect(row).toHaveCount(1);

  await deleteFrom(row, code);
  await confirmDialog(page);
  await expect(row).toHaveCount(0);
  expect((await page.request.get(shortUrl, { maxRedirects: 0 })).status()).toBe(410);
});

test('taking a Code that is already taken says so, in either language', async ({ page, user }) => {
  const code = `taken-${Date.now()}`;
  await user.api('POST', `/workspaces/${user.slug}/links`, { target: 'https://example.com/first', code });
  await signInPage(page, user.email, user.password);

  await page.getByRole('button', { name: 'New Link' }).click();
  await page.getByLabel('Target').fill('https://example.com/second');
  await moreOptions(page);
  await page.getByLabel('Custom Code').fill(code);
  await page.getByRole('button', { name: 'Create' }).click();
  await expect(page.getByRole('alert')).toHaveText('That Code is already taken.');

  await openUserMenu(page);
  await page.getByRole('radio', { name: '简体中文' }).check();
  await page.keyboard.press('Escape');
  await page.getByRole('button', { name: '创建', exact: true }).click();
  await expect(page.getByRole('alert')).toHaveText('该短码已被占用。');
});

test("switching Workspace shows that Workspace's Links", async ({ page, user }) => {
  const shared = `Shared ${Date.now()}`;
  await user.api('POST', `/workspaces/${user.slug}/links`, { target: 'https://example.com/personal' });
  const { slug } = (await user.api('POST', '/workspaces', { name: shared })) as { slug: string };
  await user.api('POST', `/workspaces/${slug}/links`, { target: 'https://example.com/shared' });
  await signInPage(page, user.email, user.password);

  const links = page.getByRole('table');
  await switchWorkspace(page, user.email);
  await expect(links).toContainText('https://example.com/personal');
  await expect(links).not.toContainText('https://example.com/shared');

  await switchWorkspace(page, shared);
  await expect(links).toContainText('https://example.com/shared');
  await expect(links).not.toContainText('https://example.com/personal');
});

test("a Link's QR code downloads as PNG or SVG from its pass, and its Short URL copies in one click", async ({
  page,
  context,
  user,
}) => {
  const code = `qr-${Date.now()}`;
  const other = `${code}-row`;
  await user.api('POST', `/workspaces/${user.slug}/links`, { target: 'https://example.com/qr-row', code: other });
  await user.api('POST', `/workspaces/${user.slug}/links`, { target: 'https://example.com/qr', code });
  await context.grantPermissions(['clipboard-read', 'clipboard-write']);
  await signInPage(page, user.email, user.password);
  await pick(page.getByRole('row').filter({ hasText: `${baseURL}/${code}` }));
  const pass = selectedPass(page);
  await expect(pass.getByRole('img', { name: `QR code of ${new URL(baseURL).host}/${code}` })).toBeVisible();

  await pass.getByRole('button', { name: 'Copy' }).click();
  await expect(pass.getByRole('button', { name: 'Copied' })).toBeVisible();
  expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(`${baseURL}/${code}`);

  for (const [format, name] of [
    ['png', 'Download QR'],
    ['svg', 'Download QR as SVG'],
  ]) {
    const download = page.waitForEvent('download');
    await pass.getByRole('link', { name, exact: true }).click();
    const file = await download;
    expect(file.suggestedFilename()).toBe(`${code}.${format}`);
    expect((await file.failure()) ?? null).toBeNull();
  }

  // Beside the pass at this width a row keeps Copy and QR in its More menu; another row's QR is a PNG too.
  await expect(page.getByRole('link', { name: `Download QR of ${code} (PNG)` })).toHaveCount(0);
  const otherRow = page.getByRole('row').filter({ hasText: `${baseURL}/${other}` });
  await otherRow.getByRole('button', { name: `More: ${other}` }).click();
  const download = page.waitForEvent('download');
  await page.getByRole('menuitem', { name: 'Download QR' }).click();
  expect((await download).suggestedFilename()).toBe(`${other}.png`);

  // On a screen with room, the row carries its own Copy and QR.
  await page.setViewportSize({ width: 1600, height: 900 });
  const wide = page.waitForEvent('download');
  await page.getByRole('link', { name: `Download QR of ${other} (PNG)` }).click();
  expect((await wide).suggestedFilename()).toBe(`${other}.png`);
});

test('when the clipboard refuses, the Short URL shows selected to copy by hand', async ({ page, user }) => {
  const code = `manual-${Date.now()}`;
  await user.api('POST', `/workspaces/${user.slug}/links`, { target: 'https://example.com/manual', code });
  await page.addInitScript(() => {
    navigator.clipboard.writeText = () => Promise.reject(new DOMException('Denied', 'NotAllowedError'));
  });
  await signInPage(page, user.email, user.password);

  await selectedPass(page).getByRole('button', { name: 'Copy' }).click();
  await expect(page.getByText('press and hold to copy')).toBeVisible();
  const manual = selectedPass(page).getByRole('textbox', { name: 'Short URL' });
  await expect(manual).toHaveValue(`${baseURL}/${code}`);
  await expect(manual).toBeFocused();
});

test('a User tags two Links, filters by one Tag and searches by part of a Target', async ({ page, user }) => {
  const code = `autumn-${Date.now()}`;
  await user.api('POST', `/workspaces/${user.slug}/links`, { target: 'https://example.com/autumn-sale', code });
  await user.api('POST', `/workspaces/${user.slug}/links`, { target: 'https://example.com/untagged' });
  await signInPage(page, user.email, user.password);

  await page.getByRole('button', { name: 'New Link' }).click();
  await page.getByLabel('Target').fill('https://example.com/spring-sale');
  await moreOptions(page);
  await page.getByLabel('Tags').fill('spring, launch');
  await page.getByRole('button', { name: 'Create' }).click();
  const spring = page.getByRole('row').filter({ hasText: 'https://example.com/spring-sale' });
  await expect(spring).toContainText('launch');
  await expect(spring).toContainText('spring');

  const autumn = page.getByRole('row').filter({ hasText: `${baseURL}/${code}` });
  await pick(autumn);
  const pass = selectedPass(page);
  await pass.getByRole('button', { name: 'Change Target' }).click();
  await pass.getByLabel('Tags').fill('autumn');
  await pass.getByRole('button', { name: 'Save' }).click();
  // Its Code says "autumn" too, so wait for the Tag itself: the save must land before the board is filtered.
  await expect(autumn.getByTitle('autumn', { exact: true })).toBeVisible();

  const rows = page.getByRole('table').getByRole('row');
  await page.getByLabel('Tag', { exact: true }).selectOption('spring');
  await expect(rows).toHaveCount(1 + 1);
  await expect(rows.nth(1)).toContainText('https://example.com/spring-sale');
  // The filter is in the address, so it survives a reload.
  await expect(page).toHaveURL(/[?&]tag=spring/);
  await page.reload();
  await expect(page.getByLabel('Tag', { exact: true })).toHaveValue('spring');
  await expect(rows).toHaveCount(1 + 1);

  await page.getByLabel('Tag', { exact: true }).selectOption('');
  await page.getByLabel('Search').fill('AUTUMN-SA');
  await expect(rows).toHaveCount(1 + 1);
  await expect(rows.nth(1)).toContainText('https://example.com/autumn-sale');
  await expect(page).toHaveURL(/[?&]q=AUTUMN-SA/);
  await page.reload();
  await expect(page.getByLabel('Search')).toHaveValue('AUTUMN-SA');
  await expect(rows).toHaveCount(1 + 1);
});

test('more Links load a page at a time', async ({ page, user }) => {
  for (let i = 0; i < 51; i++) {
    await user.api('POST', `/workspaces/${user.slug}/links`, { target: `https://example.com/${i}` });
  }
  await signInPage(page, user.email, user.password);

  const rows = page.getByRole('table').getByRole('row');
  await expect(rows).toHaveCount(1 + 50);
  await expect(rows.nth(1)).toContainText('https://example.com/50');
  await page.getByRole('button', { name: 'Load more' }).click();
  await expect(rows).toHaveCount(1 + 51);
  await expect(rows.last()).toContainText('https://example.com/0');
  await expect(page.getByRole('button', { name: 'Load more' })).toBeHidden();
});

test('a User exports Links as CSV, then imports a file and sees each row’s result', async ({ page, user }) => {
  const taken = `csv-${Date.now()}`;
  const fresh = `${taken}-new`;
  await user.api('POST', `/workspaces/${user.slug}/links`, {
    target: 'https://example.com/exported',
    code: taken,
    tags: ['launch'],
  });
  await signInPage(page, user.email, user.password);

  const downloading = page.waitForEvent('download');
  await boardMenu(page, 'Export CSV');
  const download = await downloading;
  expect(download.suggestedFilename()).toBe('links.csv');
  const exported = await readFile((await download.path())!, 'utf8');
  expect(exported).toContain(`${taken},${new URL(baseURL).host},https://example.com/exported,,launch,0`);

  await page.getByLabel('Import CSV').setInputFiles({
    name: 'links.csv',
    mimeType: 'text/csv',
    buffer: Buffer.from(
      'Code,Domain,Target,Expiry,Tags,Click Count\n' +
        `${fresh},,https://example.com/imported,,,\n` +
        `${taken},,https://example.com/again,,,\n`,
    ),
  });
  const results = page.getByRole('table', { name: 'Import CSV' }).getByRole('row');
  await expect(results.nth(1)).toContainText(`${baseURL}/${fresh}`);
  await expect(results.nth(2)).toContainText('That Code is already taken.');
  await expect(page.getByRole('row').filter({ hasText: 'https://example.com/imported' })).toContainText(fresh);
});

test('a Workspace with no Links shows a blank pass that previews the New Link, then issues it', async ({
  page,
  user,
}) => {
  const code = `first-${Date.now()}`;
  await signInPage(page, user.email, user.password);

  const blank = page.getByRole('region', { name: 'New Link' });
  await expect(blank).toContainText('Generated');
  await page.getByLabel('Target').fill('https://example.com/preview');
  await expect(blank).toContainText('example.com');
  await moreOptions(page);
  await page.getByLabel('Custom Code').fill(code);
  await expect(blank).toContainText(`${new URL(baseURL).host}/${code}`);
  await page.getByLabel('Target').press('Enter');

  await expect(blank).toHaveCount(0);
  await expect(selectedPass(page)).toContainText(code);
  await expect(page.getByRole('row').filter({ hasText: `${baseURL}/${code}` })).toContainText(
    'https://example.com/preview',
  );
});

test('a search that matches nothing says so on the board and the pass, and clearing it brings the Links back', async ({
  page,
  user,
}) => {
  await user.api('POST', `/workspaces/${user.slug}/links`, { target: 'https://example.com/kept' });
  await signInPage(page, user.email, user.password);
  const pass = selectedPass(page);
  await expect(pass).toContainText('https://example.com/kept');

  await page.getByLabel('Search').fill('nothing-matches-this');
  await expect(page.getByRole('table')).toHaveCount(0);
  await expect(pass).toContainText('No Links match.');
  await expect(pass.getByRole('button', { name: 'Copy' })).toHaveCount(0);

  await page.getByRole('button', { name: 'Clear filters' }).click();
  await expect(page.getByLabel('Search')).toHaveValue('');
  await expect(page.getByRole('row').filter({ hasText: 'https://example.com/kept' })).toHaveCount(1);
  await expect(pass).toContainText('https://example.com/kept');
});
