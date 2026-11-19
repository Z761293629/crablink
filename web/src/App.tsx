import { useEffect, useState, type FormEvent } from 'react';
import { useTranslation } from 'react-i18next';
import { Admin } from './Admin';
import { api, firstWorkspace, type Domain, type Link, type LinkPage, type Membership, type User } from './api';
import { Domains } from './Domains';
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
        ) : page === 'domains' ? (
          <Domains />
        ) : page === 'admin' && user.instance_admin ? (
          <Admin />
        ) : (
          <Links />
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

/** The Links of the User's first Workspace, their personal one until they join others. */
function Links() {
  const { t, i18n } = useTranslation();
  const [workspace, setWorkspace] = useState<Membership>();
  const [links, setLinks] = useState<Link[]>();
  const [domains, setDomains] = useState<Domain[]>([]);
  const [error, setError] = useState<unknown>();

  useEffect(() => {
    (async () => {
      const first = await firstWorkspace();
      if (!first) return setLinks([]);
      setWorkspace(first);
      setLinks((await api<LinkPage>('GET', `/workspaces/${first.slug}/links`)).links);
      setDomains(await api<Domain[]>('GET', `/workspaces/${first.slug}/domains`));
    })().catch(setError);
  }, []);

  async function create(e: FormEvent<HTMLFormElement>) {
    e.preventDefault();
    const form = e.currentTarget;
    const data = new FormData(form);
    try {
      const link = await api<Link>('POST', `/workspaces/${workspace!.slug}/links`, {
        target: data.get('target'),
        code: data.get('code') || null,
        domain: data.get('domain'),
      });
      setLinks([link, ...(links ?? [])]);
      setError(undefined);
      form.reset();
    } catch (err) {
      setError(err);
    }
  }

  if (!links) return error === undefined ? null : <p role="alert">{errorMessage(t, error)}</p>;
  const date = new Intl.DateTimeFormat(i18n.language, { dateStyle: 'medium', timeStyle: 'short' });
  return (
    <>
      <h1>{workspace?.name}</h1>
      {workspace && (
        <form className="inline" onSubmit={create}>
          <label>
            {t('links.target')} <input name="target" type="url" placeholder="https://" required />
          </label>
          <label>
            {t('links.code')} <input name="code" placeholder={t('links.codeHint')} />
          </label>
          <label>
            {t('links.domain')}{' '}
            <select name="domain">
              {domains
                .filter((d) => d.default || d.status === 'approved')
                .map((d) => (
                  <option key={d.host}>{d.host}</option>
                ))}
            </select>
          </label>
          <button type="submit">{t('links.create')}</button>
        </form>
      )}
      {error !== undefined && <p role="alert">{errorMessage(t, error)}</p>}
      {links.length === 0 ? (
        <p>{t('links.empty')}</p>
      ) : (
        <div className="table">
          <table>
            <thead>
              <tr>
                <th>{t('links.shortUrl')}</th>
                <th>{t('links.target')}</th>
                <th>{t('links.clickCount')}</th>
                <th>{t('links.expiry')}</th>
              </tr>
            </thead>
            <tbody>
              {links.map((link) => (
                <tr key={link.short_url}>
                  <td>
                    <a href={link.short_url}>{link.short_url}</a>
                  </td>
                  <td className="target">{link.target}</td>
                  <td>{link.click_count}</td>
                  <td>{link.expires_at ? date.format(new Date(link.expires_at)) : t('links.never')}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </>
  );
}
