import type { Locator, Page } from '@playwright/test';
import { test, expect, signInPage } from './fixtures';

const HOST = 'www.example-very-long-subdomain-for-marketing-campaigns.example.co.uk';
const TARGET = `https://${HOST}/landing/2026/spring/limited-edition-collection?utm_source=wechat&utm_medium=poster&utm_campaign=spring-sale-2026-shanghai-metro-line-2&utm_content=variant-b&ref=abcdef1234567890`.padEnd(
  300,
  'x',
);
const TAGS = [
  'campaign',
  'poster',
  'spring-2026-shanghai-metro-line-2-lightbox-wave',
  '二零二六年春季上海地铁二号线全线灯箱海报投放活动第二批次',
  '🎉🔥活动',
];

/** A 64-character Code: the longest there is. */
const longCode = (tail: string) => `${Date.now()}-${tail}-spring-2026-shanghai-metro-line-two-lightbox-poster`.slice(0, 64);

/** Nothing scrolls sideways: not `el`, not the page, and no part of `el` sticks out of it (to either side). */
async function fits(page: Page, el: Locator) {
  const escaped = await el.evaluate((box) => {
    const outer = box.getBoundingClientRect();
    return [...box.querySelectorAll<HTMLElement>('*:not(.vh)')]
      .filter((e) => {
        const r = e.getBoundingClientRect();
        return r.width > 0 && (r.left < outer.left - 1 || r.right > outer.right + 1);
      })
      .map((e) => e.outerHTML.slice(0, 80));
  });
  expect(escaped).toEqual([]);
  const [scroll, client] = await el.evaluate((e) => [e.scrollWidth, e.clientWidth]);
  expect(scroll).toBeLessThanOrEqual(client);
  const root = await page.evaluate(() => [document.documentElement.scrollWidth, document.documentElement.clientWidth]);
  expect(root[0]).toBeLessThanOrEqual(root[1]);
}

test('a Link with a 64-character Code, a long Target and long Tags fits the board and its pass on desktop and phone', async ({
  page,
  context,
  user,
}) => {
  await context.grantPermissions(['clipboard-read', 'clipboard-write']);
  const code = longCode('a');
  const link = (await user.api('POST', `/workspaces/${user.slug}/links`, { target: TARGET, code, tags: TAGS })) as {
    domain: string;
    short_url: string;
  };
  const shortText = link.short_url.replace(/^https?:\/\//, '');
  await signInPage(page, user.email, user.password);

  for (const viewport of [
    { width: 1440, height: 900 },
    { width: 390, height: 844 },
  ]) {
    await page.setViewportSize(viewport);
    await page.goto(`/app/links/${encodeURIComponent(link.domain)}/${code}`);
    const pass = page.getByRole('region', { name: 'Selected Link' });
    // The whole Short URL is on the pass, never cut short: it is what gets copied and printed.
    await expect(pass.locator('.stub .short')).toHaveText(shortText);
    await expect(pass.locator('.route .code')).toHaveText(code);
    await fits(page, pass);

    // The row shows two Tags on one line, then the rest as one chip naming them.
    const row = page.getByRole('row').filter({ hasText: code }).first();
    await expect(row.locator('.tag')).toHaveCount(3);
    await expect(row.locator('.tag').last()).toHaveAttribute('title', TAGS.slice(2).join(', '));
    const meta = await row.locator('.meta').boundingBox();
    const chip = await row.locator('.tag').first().boundingBox();
    expect(meta!.height).toBeLessThan(chip!.height * 1.5);
    await fits(page, page.locator('.board'));

    // Copy says Copied in the same box: nothing beside it moves.
    const copy = row.getByRole('button', { name: 'Copy' });
    const before = (await copy.boundingBox())!.width;
    await copy.click();
    await expect(row.getByRole('button', { name: 'Copied' })).toBeVisible();
    expect((await row.getByRole('button', { name: 'Copied' }).boundingBox())!.width).toBe(before);

    // Changing the Target keeps the pass within its box.
    await pass.getByRole('button', { name: 'Change Target' }).click();
    await expect(pass.getByLabel('Target')).toHaveValue(TARGET);
    await fits(page, pass);
    await page.keyboard.press('Escape');

    // The blank pass previews a long New Link while it is typed.
    await page.getByRole('button', { name: 'New Link' }).click();
    const blank = page.getByRole('region', { name: 'New Link' });
    await blank.getByLabel('Target').fill(TARGET);
    await blank.getByText('More options').click();
    const typed = longCode('b');
    await blank.getByLabel('Custom Code').fill(typed);
    await blank.getByLabel('Tags').fill(TAGS.join(', '));
    await expect(blank.locator('.stub .short')).toHaveText(`${link.domain}/${typed}`);
    await expect(blank.locator('.route .host')).toHaveText(HOST);
    await fits(page, blank);
    await page.keyboard.press('Escape');
  }
});

test('a Tag longer than 64 characters is refused, saying why', async ({ page, user }) => {
  await signInPage(page, user.email, user.password);
  await page.getByRole('button', { name: 'New Link' }).click();
  const blank = page.getByRole('region', { name: 'New Link' });
  await blank.getByLabel('Target').fill('https://example.com/long-tag');
  await blank.getByText('More options').click();
  await blank.getByLabel('Tags').fill('长'.repeat(65));
  await blank.getByRole('button', { name: 'Create' }).click();
  await expect(blank.getByRole('alert')).toHaveText('A Tag must be 1-64 characters, with no comma.');
});
