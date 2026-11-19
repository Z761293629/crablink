// Test fixtures: each test gets its own fresh Users, set up through the API the way
// the web UI and an Instance Admin would. Import `test` and `expect` from here.

import { test as base, expect, type APIRequestContext, type Page } from '@playwright/test';
import { baseURL, instanceAdmin } from '../playwright.config';

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
  await expect(page.getByRole('button', { name: 'Sign out' })).toBeVisible();
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
