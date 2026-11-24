// Test fixtures: each test gets its own fresh Users, set up through the API the way
// the web UI and an Instance Admin would. Import `test` and `expect` from here.

import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join } from 'node:path';
import { test as base, expect, type APIRequestContext, type Page } from '@playwright/test';
import { baseURL, instanceAdmin, mailDir } from '../playwright.config';

/** A User with their personal Workspace, and a session for seeding data through the API. */
export interface User {
  email: string;
  password: string;
  /** Their personal Workspace's slug. */
  slug: string;
  /** Calls `/api{path}` as this User, expecting success; resolves to the JSON body. */
  api: (method: string, path: string, body?: unknown) => Promise<unknown>;
}

let nextUser = 0;

/** A fresh email no User has yet. */
export function newEmail(workerIndex: number) {
  return `user-${workerIndex}-${Date.now()}-${nextUser++}@example.com`;
}

async function newUser(request: APIRequestContext, workerIndex: number): Promise<User> {
  const email = newEmail(workerIndex);
  const password = 'e2e user password';
  const token = new URL(await inviteUrl(request, email)).hash.slice(1);
  const accepted = await request.post('/api/invites/accept', {
    headers: { origin: baseURL },
    data: { token, password },
  });
  expect(accepted.status(), await accepted.text()).toBe(200);
  const cookie = sessionCookie(accepted.headers());
  const [workspace] = (await call(request, cookie, 'GET', '/workspaces')) as { slug: string }[];
  return {
    email,
    password,
    slug: workspace.slug,
    api: (method, path, body) => call(request, cookie, method, path, body),
  };
}

/** `user` is the one the test drives; `other` is a second User, e.g. to add as a Member. */
export const test = base.extend<{ user: User; other: User }>({
  user: async ({ request }, use, testInfo) => use(await newUser(request, testInfo.workerIndex)),
  other: async ({ request }, use, testInfo) => use(await newUser(request, testInfo.workerIndex)),
});

export { expect };

/** An invite link for `email`, made by the Instance Admin. */
export async function inviteUrl(request: APIRequestContext, email: string) {
  const adminCookie = await signInApi(request, instanceAdmin.email, instanceAdmin.password);
  const { url } = (await call(request, adminCookie, 'POST', '/admin/invites', { email })) as { url: string };
  return url;
}

/** Signs `page` in through the web UI's sign-in form. */
export async function signInPage(page: Page, email: string, password: string) {
  await page.goto('/app/');
  await page.getByLabel('Email').fill(email);
  await page.getByLabel('Password').fill(password);
  await page.getByRole('button', { name: 'Sign in' }).click();
  await expect(userMenu(page)).toBeVisible();
}

/** The signed-in User's menu in the header: their Workspaces, then Account, Language, Instance Admin, Sign out. */
export const userMenu = (page: Page) => page.getByRole('button', { name: /^(Account menu|账户菜单):/ });

export async function openUserMenu(page: Page) {
  await userMenu(page).click();
}

export async function signOut(page: Page) {
  await openUserMenu(page);
  await page.getByRole('button', { name: 'Sign out' }).click();
}

/** The header's one menu, which also switches Workspaces and names the current one. */
export const switcher = userMenu;

/** Opens the switcher; resolves to its list of Workspaces. */
export async function openWorkspaceMenu(page: Page) {
  await switcher(page).click();
  return page.getByRole('list', { name: 'Workspace' }).getByRole('button');
}

export async function switchWorkspace(page: Page, name: string) {
  await openWorkspaceMenu(page);
  await page.getByRole('list', { name: 'Workspace' }).getByRole('button', { name, exact: true }).click();
}

/** Opens one of the current Workspace's pages from the switcher. */
export async function openWorkspacePage(page: Page, name: 'Domains' | 'Members & API Keys' | 'New Workspace') {
  await openWorkspaceMenu(page);
  await page.getByRole('link', { name }).click();
}

/** Chooses the action (not Cancel) in the in-app "Are you sure?" dialog. */
export async function confirmDialog(page: Page) {
  await page.getByRole('dialog').getByRole('button').last().click();
}

/** The link in the newest email to `email`, waiting for the server to send it. */
export async function emailedLink(email: string): Promise<string> {
  let link: string | undefined;
  await expect
    .poll(() => (link = newestEmailTo(email)?.match(/https?:\/\/\S+/)?.[0]), { message: `an email to ${email}` })
    .toBeDefined();
  return link!;
}

/** The newest email to `email` in the SMTP sink's directory, its body decoded. */
function newestEmailTo(email: string): string | undefined {
  const emails = readdirSync(mailDir)
    .map((name) => join(mailDir, name))
    .sort((a, b) => statSync(b).mtimeMs - statSync(a).mtimeMs)
    .map((path) => readFileSync(path, 'utf8'))
    .filter((raw) => new RegExp(`^To: .*${email.replace(/[.+]/g, '\\$&')}`, 'mi').test(raw));
  return emails[0] && body(emails[0]);
}

/** The text of a raw single-part email: lettre picks 7bit, quoted-printable or base64. */
function body(raw: string): string {
  const split = raw.indexOf('\n\n');
  const [head, text] = [raw.slice(0, split), raw.slice(split + 2)];
  if (/^Content-Transfer-Encoding: base64/im.test(head)) return Buffer.from(text, 'base64').toString('utf8');
  if (!/^Content-Transfer-Encoding: quoted-printable/im.test(head)) return text;
  const unwrapped = text.replace(/=\n/g, '');
  const bytes: number[] = [];
  for (let i = 0; i < unwrapped.length; i++) {
    if (unwrapped[i] === '=') {
      bytes.push(parseInt(unwrapped.slice(i + 1, i + 3), 16));
      i += 2;
    } else bytes.push(unwrapped.charCodeAt(i));
  }
  return Buffer.from(bytes).toString('utf8');
}

async function signInApi(request: APIRequestContext, email: string, password: string) {
  const res = await request.post('/api/session', { headers: { origin: baseURL }, data: { email, password } });
  expect(res.status(), await res.text()).toBe(200);
  return sessionCookie(res.headers());
}

// The session cookie is `Secure`; sending it by hand doesn't rely on the API client
// keeping a Secure cookie over plain-HTTP localhost.
function sessionCookie(headers: Record<string, string>) {
  return headers['set-cookie'].split(';')[0];
}

async function call(request: APIRequestContext, cookie: string, method: string, path: string, body?: unknown) {
  const res = await request.fetch(`/api${path}`, {
    method,
    headers: { cookie, origin: baseURL },
    data: body,
  });
  expect(res.ok(), `${method} ${path}: ${await res.text()}`).toBe(true);
  return res.status() === 204 ? undefined : res.json();
}
