// The JSON API (crates/api), called from the same origin with the session cookie.

export interface User {
  email: string;
  instance_admin: boolean;
  /** False for a User an Identity Provider created, who signs in only through one. */
  has_password: boolean;
  /** False for a User who signed up and hasn't verified their email: they can't create Links or API Keys yet. */
  email_verified: boolean;
}

/** An Identity Provider the sign-in page offers; signing in through it starts at `/auth/{id}`. */
export interface IdentityProvider {
  id: string;
  name: string;
}

export interface Membership {
  slug: string;
  name: string;
  role: 'owner' | 'member';
}

export interface WorkspaceUser {
  email: string;
  role: Membership['role'];
}

export interface ApiKey {
  id: number;
  name: string;
  /** The secret's first characters, to tell keys apart. */
  prefix: string;
  created_at: string;
  last_used_at: string | null;
}

/** Only in the response that creates the key: the secret is never shown again. */
export interface NewApiKey extends ApiKey {
  secret: string;
}

export interface Link {
  code: string;
  domain: string;
  short_url: string;
  target: string;
  created_at: string;
  updated_at: string;
  expires_at: string | null;
  click_count: number;
  /** Sorted. */
  tags: string[];
  /** A Blocked Link: stopped for abuse by an Instance Admin, it can only be deleted. */
  blocked: boolean;
}

/** A Blocked Link as an Instance Admin sees it. */
export interface BlockedLink {
  short_url: string;
  target: string;
  workspace_slug: string;
  blocked_at: string;
}

/** An open report of a Short URL, with the Owners and Members of its Link's Workspace. */
export interface Report {
  id: number;
  short_url: string;
  target: string;
  workspace_slug: string;
  reason: string;
  reported_at: string;
  workspace_users: { id: number; email: string; role: Membership['role']; disabled: boolean }[];
}

export interface LinkPage {
  links: Link[];
  next_cursor: string | null;
}

export type DomainStatus = 'pending' | 'approved' | 'rejected';

/** A Domain a Workspace may use: the Default Domain or one of its custom Domains. */
export interface Domain {
  domain: string;
  status: DomainStatus;
  default: boolean;
}

/** A custom Domain as an Instance Admin sees it, with the Workspace slug of the Workspace that has it. */
export interface ManagedDomain {
  domain: string;
  status: DomainStatus;
  workspace_slug: string;
}

/** The Clicks in the bucket starting at `time`. */
export interface ClickBucket {
  time: string;
  count: number;
}

/** The Clicks sharing one `value` (null where none or unknown). */
export interface ClickBreakdown {
  value: string | null;
  count: number;
}

export interface ClickStats {
  series: ClickBucket[];
  referrers: ClickBreakdown[];
  countries: ClickBreakdown[];
  devices: ClickBreakdown[];
  browsers: ClickBreakdown[];
}

export interface WorkspaceClicks {
  series: ClickBucket[];
  top_links: { code: string; domain: string; short_url: string; count: number }[];
}

/** What the instance offers before sign-in, from `GET /api/instance`. */
export interface Instance {
  /** Whether Users can reset a forgotten password by email. */
  password_reset: boolean;
  /** Whether anyone can sign up with an email and a password. */
  sign_up: boolean;
  /** The terms of service and privacy policy the sign-up form links, if any. */
  terms_url: string | null;
  privacy_url: string | null;
}

export interface ManagedUser {
  id: number;
  email: string;
  instance_admin: boolean;
  disabled: boolean;
  has_password: boolean;
  /** The names of the Identity Providers the User has signed in through. */
  identity_providers: string[];
}

/**
 * The API path of a Link of Workspace `slug`, or of its `sub` resource (e.g. `/qr`),
 * addressed by its Domain; it ends in `?domain=…`, so add more query with `&`.
 */
export function linkPath(slug: string, link: { code: string; domain: string }, sub = ''): string {
  const code = encodeURIComponent(link.code);
  return `/workspaces/${encodeURIComponent(slug)}/links/${code}${sub}?domain=${encodeURIComponent(link.domain)}`;
}

/**
 * An error response: `kind` is stable, `message` is the server's English text, and
 * `retryAfter` the seconds a `rate_limited` caller must wait.
 */
export class ApiError extends Error {
  constructor(
    readonly status: number,
    readonly kind: string,
    message: string,
    readonly retryAfter?: number,
  ) {
    super(message);
  }
}

/** One row of a CSV import: the created Link's Short URL, or why the row was skipped. */
export interface ImportedRow {
  /** The row's line in the file, the header being line 1. */
  line: number;
  short_url?: string;
  skipped?: { kind: string; error: string };
}

/**
 * Calls `/api{path}`; resolves to the JSON body, or `undefined` for 204. A `Blob`
 * body (a chosen file) is sent as is, anything else as JSON.
 */
export async function api<T>(method: string, path: string, body?: unknown): Promise<T> {
  const raw = body instanceof Blob;
  // The UI's language, not the browser's: emails the request sends follow it.
  const headers: Record<string, string> = { 'accept-language': document.documentElement.lang || 'en' };
  if (body !== undefined && !raw) headers['content-type'] = 'application/json';
  const res = await fetch(`/api${path}`, {
    method,
    headers,
    body: body === undefined ? undefined : raw ? body : JSON.stringify(body),
  });
  if (!res.ok) {
    const err = await res.json().catch(() => ({}));
    const retryAfter = Number(res.headers.get('retry-after')) || undefined;
    throw new ApiError(res.status, err.kind ?? 'unknown', err.error ?? res.statusText, retryAfter);
  }
  return res.status === 204 ? (undefined as T) : res.json();
}
