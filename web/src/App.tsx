import { useEffect, useState, type FormEvent } from 'react';
import { useTranslation } from 'react-i18next';
import { api, type Link, type LinkPage, type Membership, type User } from './api';
import { errorMessage, languages, setLanguage } from './i18n';

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
      <main>{user === undefined ? null : user ? <Links /> : <SignIn onSignedIn={setUser} />}</main>
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

/** The Links of the User's first Workspace, their personal one until they join others. */
function Links() {
  const { t, i18n } = useTranslation();
  const [workspace, setWorkspace] = useState<Membership>();
  const [links, setLinks] = useState<Link[]>();
  const [error, setError] = useState<unknown>();

  useEffect(() => {
    (async () => {
      const [first] = await api<Membership[]>('GET', '/workspaces');
      if (!first) return setLinks([]);
      setWorkspace(first);
      setLinks((await api<LinkPage>('GET', `/workspaces/${first.slug}/links`)).links);
    })().catch(setError);
  }, []);

  if (error !== undefined) return <p role="alert">{errorMessage(t, error)}</p>;
  if (!links) return null;
  const date = new Intl.DateTimeFormat(i18n.language, { dateStyle: 'medium', timeStyle: 'short' });
  return (
    <>
      <h1>{workspace?.name}</h1>
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
