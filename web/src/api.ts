// The JSON API (crates/api), called from the same origin with the session cookie.

export interface User {
  email: string;
  instance_admin: boolean;
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
}

export interface LinkPage {
  links: Link[];
  next_cursor: string | null;
}

export type DomainStatus = 'pending' | 'approved' | 'rejected';

/** A Domain a Workspace may use: the Default Domain or one of its custom Domains. */
export interface Domain {
  host: string;
  status: DomainStatus;
  default: boolean;
}

/** A custom Domain as an Instance Admin sees it, with the slug of the Workspace that has it. */
export interface ManagedDomain {
  host: string;
  status: DomainStatus;
  workspace: string;
}

export interface ManagedUser {
  id: number;
  email: string;
  instance_admin: boolean;
  disabled: boolean;
}

/** An error response: `kind` is stable, `message` is the server's English text. */
export class ApiError extends Error {
  constructor(
    readonly status: number,
    readonly kind: string,
    message: string,
  ) {
    super(message);
  }
}

/** Calls `/api{path}`; resolves to the JSON body, or `undefined` for 204. */
export async function api<T>(method: string, path: string, body?: unknown): Promise<T> {
  const res = await fetch(`/api${path}`, {
    method,
    headers: body === undefined ? {} : { 'content-type': 'application/json' },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  if (!res.ok) {
    const err = await res.json().catch(() => ({}));
    throw new ApiError(res.status, err.kind ?? 'unknown', err.error ?? res.statusText);
  }
  return res.status === 204 ? (undefined as T) : res.json();
}
