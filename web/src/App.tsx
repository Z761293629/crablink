import { useCallback, useEffect, useState, type FormEvent } from 'react';
import { useTranslation } from 'react-i18next';
import { AcceptInvite, ChangePassword } from './Account';
import { Admin } from './Admin';
import { api, type Membership, type User } from './api';
import { Domains } from './Domains';
import { Links } from './Links';
import { errorMessage, languages, setLanguage } from './i18n';
import { ManageWorkspace } from './Workspace';

// ponytail: pages are plain links that reload the SPA; add a router when a page keeps state worth not reloading.
const page = location.pathname.replace(/^\/app\/?/, '').replace(/\/$/, '');

/** The pages about one Workspace, the one the switcher picks. */
const workspacePages = ['', 'domains', 'workspace'];

// The switcher's pick, kept across the reload a page link makes.
const pickKey = 'crablink.workspace';
function savedPick() {
  try {
    return localStorage.getItem(pickKey) ?? undefined;
  } catch {
    return undefined;
  }
}

export function App() {
  const { t } = useTranslation();
  // undefined while `/api/me` is in flight; null when signed out.
  const [user, setUser] = useState<User | null>();
  const [workspaces, setWorkspaces] = useState<Membership[]>();
  const [slug, setSlug] = useState(savedPick);
  const [error, setError] = useState<unknown>();

  /** Reloads the User's Workspaces, keeping the current one, or `prefer`, if still theirs. */
  const refresh = useCallback(async (prefer?: string) => {
    const list = await api<Membership[]>('GET', '/workspaces');
    setWorkspaces(list);
    setSlug((current) => list.find((w) => w.slug === (prefer ?? current))?.slug ?? list[0]?.slug);
  }, []);

  const signedInAs = useCallback(
    (user: User) => {
      setUser(user);
      refresh().catch(setError);
    },
    [refresh],
  );

  useEffect(() => {
    api<{ user?: User }>('GET', '/me').then(
      (me) => (me.user ? signedInAs(me.user) : setUser(null)),
      () => setUser(null),
    );
  }, [signedInAs]);

  useEffect(() => {
    try {
      if (slug) localStorage.setItem(pickKey, slug);
    } catch {
      // Storage is blocked: the pick lasts until the next page.
    }
  }, [slug]);

  async function signOut() {
    await api('DELETE', '/session');
    setUser(null);
    setWorkspaces(undefined);
  }

  const workspace = workspaces?.find((w) => w.slug === slug);
  let main;
  if (page === 'invite') main = <AcceptInvite />;
  else if (user === undefined) main = null;
  else if (!user) main = <SignIn onSignedIn={signedInAs} />;
  else if (error !== undefined) main = <p role="alert">{errorMessage(t, error)}</p>;
  else if (page === 'admin' && user.instance_admin) main = <Admin />;
  else if (page === 'account') main = <ChangePassword />;
  else if (!workspaces) main = null;
  else if (page === 'workspace')
    main = <ManageWorkspace key={slug} workspace={workspace} email={user.email} onChanged={refresh} />;
  else if (!workspace) main = <p>{t('workspace.none')}</p>;
  else if (page === 'domains') main = <Domains key={slug} workspace={workspace} />;
  else
    main = (
      <>
        <h1>{t('links.title')}</h1>
        <Links key={slug} slug={workspace.slug} />
      </>
    );

  return (
    <>
      <header>
        <strong>crablink</strong>
        <LanguageSwitcher />
        {user && (
          <>
            <span className="email">{user.email}</span>
            <button onClick={signOut}>{t('signOut')}</button>
          </>
        )}
      </header>
      {user && (
        <nav>
          {workspacePages.includes(page) && workspaces && workspaces.length > 0 && (
            <span>
              <label htmlFor="workspace">{t('nav.switcher')}</label>{' '}
              <select id="workspace" value={slug} onChange={(e) => setSlug(e.target.value)}>
                {workspaces.map((w) => (
                  <option key={w.slug} value={w.slug}>
                    {w.name}
                  </option>
                ))}
              </select>
            </span>
          )}
          <a href="/app/">{t('nav.links')}</a>
          <a href="/app/domains">{t('nav.domains')}</a>
          <a href="/app/workspace">{t('nav.workspace')}</a>
          <a href="/app/account">{t('nav.account')}</a>
          {user.instance_admin && <a href="/app/admin">{t('nav.admin')}</a>}
        </nav>
      )}
      <main>{main}</main>
    </>
  );
}

function LanguageSwitcher() {
  const { t, i18n } = useTranslation();
  return (
    <label>
      {t('language')}{' '}
      <select value={i18n.language} onChange={(e) => setLanguage(e.target.value)}>
        {Object.entries(languages).map(([lng, name]) => (
          <option key={lng} value={lng}>
            {name}
          </option>
        ))}
      </select>
    </label>
  );
}

function SignIn({ onSignedIn }: { onSignedIn: (user: User) => void }) {
  const { t } = useTranslation();
  const [error, setError] = useState<unknown>();

  async function submit(e: FormEvent<HTMLFormElement>) {
    e.preventDefault();
    const form = new FormData(e.currentTarget);
    try {
      onSignedIn(await api<User>('POST', '/session', { email: form.get('email'), password: form.get('password') }));
    } catch (err) {
      setError(err);
    }
  }

  return (
    <form className="narrow" onSubmit={submit}>
      <h1>{t('signIn.title')}</h1>
      <label>
        {t('signIn.email')}
        <input name="email" type="email" autoComplete="username" required />
      </label>
      <label>
        {t('signIn.password')}
        <input name="password" type="password" autoComplete="current-password" required />
      </label>
      {error !== undefined && <p role="alert">{errorMessage(t, error)}</p>}
      <button type="submit">{t('signIn.submit')}</button>
    </form>
  );
}
