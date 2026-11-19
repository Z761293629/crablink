import { useCallback, useEffect, useState, type FormEvent } from 'react';
import { useTranslation } from 'react-i18next';
import { api, type Link, type LinkPage, type Membership, type User } from './api';
import { AcceptInvite, ChangePassword } from './Account';
import { errorMessage, languages, setLanguage } from './i18n';
import { ManageWorkspace } from './Workspace';

type View = 'links' | 'workspace' | 'account';

/** The token of an invite link, `/app/invite#<token>`. */
function inviteToken() {
  return location.pathname.endsWith('/invite') ? location.hash.slice(1) : null;
}

export function App() {
  const { t } = useTranslation();
  // undefined while `/api/me` is in flight; null when signed out.
  const [user, setUser] = useState<User | null>();
  const [invite, setInvite] = useState(inviteToken);
  const [workspaces, setWorkspaces] = useState<Membership[]>();
  const [slug, setSlug] = useState<string>();
  const [view, setView] = useState<View>('links');
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

  function accepted(user: User) {
    history.replaceState(null, '', '/app/');
    setInvite(null);
    signedInAs(user);
  }

  async function signOut() {
    await api('DELETE', '/session');
    setUser(null);
    setWorkspaces(undefined);
    setView('links');
  }

  const workspace = workspaces?.find((w) => w.slug === slug);
  const signedIn = user && !invite;
  let page;
  if (invite) page = <AcceptInvite token={invite} onAccepted={accepted} />;
  else if (user === null) page = <SignIn onSignedIn={signedInAs} />;
  else if (error !== undefined) page = <p role="alert">{errorMessage(t, error)}</p>;
  else if (!user || !workspaces) page = null;
  else if (view === 'account') page = <ChangePassword />;
  else if (view === 'workspace')
    page = <ManageWorkspace key={slug} workspace={workspace} email={user.email} onChanged={refresh} />;
  else if (workspace) page = <Links key={slug} workspace={workspace} />;
  else page = <p>{t('workspace.none')}</p>;

  return (
    <>
      <header>
        <strong>crablink</strong>
        {signedIn && workspaces && (
          <>
            {workspaces.length > 0 && (
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
            <nav>
              {(['links', 'workspace', 'account'] as const).map((v) => (
                <button key={v} aria-current={view === v ? 'page' : undefined} onClick={() => setView(v)}>
                  {t(`nav.${v}`)}
                </button>
              ))}
            </nav>
          </>
        )}
        <LanguageSwitcher />
        {signedIn && (
          <>
            <span className="email">{user.email}</span>
            <button onClick={signOut}>{t('signOut')}</button>
          </>
        )}
      </header>
      <main>{page}</main>
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

/** The Links of `workspace`. */
function Links({ workspace }: { workspace: Membership }) {
  const { t, i18n } = useTranslation();
  const [links, setLinks] = useState<Link[]>();
  const [error, setError] = useState<unknown>();

  useEffect(() => {
    api<LinkPage>('GET', `/workspaces/${workspace.slug}/links`).then((page) => setLinks(page.links), setError);
  }, [workspace.slug]);

  if (error !== undefined) return <p role="alert">{errorMessage(t, error)}</p>;
  if (!links) return null;
  const date = new Intl.DateTimeFormat(i18n.language, { dateStyle: 'medium', timeStyle: 'short' });
  return (
    <>
      <h1>{workspace.name}</h1>
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
