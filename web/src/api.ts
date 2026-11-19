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

export interface ManagedUser {
  id: number;
  email: string;
  instance_admin: boolean;
  disabled: boolean;
}

/** The signed-in User's first Workspace by name: their personal one until they join others. */
export async function firstWorkspace(): Promise<Membership | undefined> {
  return (await api<Membership[]>('GET', '/workspaces'))[0];
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
