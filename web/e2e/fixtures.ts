// Test fixtures: each test gets its own fresh User, set up through the API the way
// the web UI and an Instance Admin would. Import `test` and `expect` from here.

import { test as base, expect, type APIRequestContext, type Page } from '@playwright/test';
import { admin, baseURL } from '../playwright.config';

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

export const test = base.extend<{ user: User }>({
  user: async ({ request }, use, testInfo) => {
    const email = `user-${testInfo.workerIndex}-${Date.now()}-${nextUser++}@example.com`;
    const password = 'e2e user password';
    const adminCookie = await signIn(request, admin.email, admin.password);
    const { url } = (await call(request, adminCookie, 'POST', '/admin/invites', { email })) as { url: string };
    const token = new URL(url).hash.slice(1);
    const accepted = await request.post('/api/invites/accept', {
      headers: { origin: baseURL },
      data: { token, password },
    });
    expect(accepted.status(), await accepted.text()).toBe(200);
    const cookie = sessionCookie(accepted.headers());
    const [workspace] = (await call(request, cookie, 'GET', '/workspaces')) as { slug: string }[];
    await use({
      email,
      password,
      slug: workspace.slug,
      api: (method, path, body) => call(request, cookie, method, path, body),
    });
  },
});

export { expect };

/** Signs `user` in through the sign-in page. */
export async function signInPage(page: Page, user: User) {
  await page.goto('/app/');
  await page.getByLabel('Email').fill(user.email);
  await page.getByLabel('Password').fill(user.password);
  await page.getByRole('button', { name: 'Sign in' }).click();
  await expect(page.getByRole('button', { name: 'Sign out' })).toBeVisible();
}

async function signIn(request: APIRequestContext, email: string, password: string) {
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
