import { useEffect, useState, type FormEvent } from 'react';
import { useTranslation } from 'react-i18next';
import { Admin } from './Admin';
import { api, type Membership, type User } from './api';
import { ClicksPage, clicksHref } from './Clicks';
import { Domains } from './Domains';
import { Links } from './Links';
import { errorMessage, languages, setLanguage } from './i18n';

// ponytail: pages are plain links that reload the SPA; add a router when a page keeps state worth not reloading.
const page = location.pathname.replace(/^\/app\/?/, '').replace(/\/$/, '');

export function App() {
  const { t } = useTranslation();
  // undefined while `/api/me` is in flight; null when signed out.
  const [user, setUser] = useState<User | null>();

  useEffect(() => {
    api<{ user?: User }>('GET', '/me').then(
      (me) => setUser(me.user ?? null),
      () => setUser(null),
    );
  }, []);

  async function signOut() {
    await api('DELETE', '/session');
    setUser(null);
  }

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
          <a href="/app/">{t('nav.links')}</a>
          <a href="/app/domains">{t('nav.domains')}</a>
          {user.instance_admin && <a href="/app/admin">{t('nav.admin')}</a>}
        </nav>
      )}
      <main>
        {page === 'invite' ? (
          <AcceptInvite />
        ) : user === undefined ? null : !user ? (
          <SignIn onSignedIn={setUser} />
        ) : page === 'clicks' ? (
          <ClicksPage />
        ) : page === 'domains' ? (
          <Domains />
        ) : page === 'admin' && user.instance_admin ? (
          <Admin />
        ) : (
          <Workspaces />
        )}
      </main>
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
    <form className="sign-in" onSubmit={submit}>
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

/** `/app/invite#{token}`: a new User picks a password and is signed in. */
function AcceptInvite() {
  const { t } = useTranslation();
  const [error, setError] = useState<unknown>();

  async function submit(e: FormEvent<HTMLFormElement>) {
    e.preventDefault();
    const password = new FormData(e.currentTarget).get('password');
    try {
      await api('POST', '/invites/accept', { token: location.hash.slice(1), password });
      location.assign('/app/');
    } catch (err) {
      setError(err);
    }
  }

  return (
    <form className="sign-in" onSubmit={submit}>
      <h1>{t('invite.title')}</h1>
      <label>
        {t('invite.password')}
        <input name="password" type="password" autoComplete="new-password" minLength={8} required />
      </label>
      {error !== undefined && <p role="alert">{errorMessage(t, error)}</p>}
      <button type="submit">{t('invite.submit')}</button>
    </form>
  );
}

/** The User's Workspaces, one at a time: the first, their personal one until they join others, until they pick another. */
function Workspaces() {
  const { t } = useTranslation();
  const [workspaces, setWorkspaces] = useState<Membership[]>();
  const [slug, setSlug] = useState<string>();
  const [error, setError] = useState<unknown>();

  useEffect(() => {
    api<Membership[]>('GET', '/workspaces').then(setWorkspaces, setError);
  }, []);

  if (error !== undefined) return <p role="alert">{errorMessage(t, error)}</p>;
  if (!workspaces) return null;
  const current = slug ?? workspaces[0]?.slug;
  if (!current) return <p>{t('links.empty')}</p>;
  return (
    <>
      <label className="workspace">
        {t('workspace')}{' '}
        <select value={current} onChange={(e) => setSlug(e.target.value)}>
          {workspaces.map((w) => (
            <option key={w.slug} value={w.slug}>
              {w.name}
            </option>
          ))}
        </select>
      </label>
      <h1>{t('links.title')}</h1>
      <p>
        <a href={clicksHref(current)}>{t('clicks.workspaceTitle')}</a>
      </p>
      <Links key={current} slug={current} />
    </>
  );
}
