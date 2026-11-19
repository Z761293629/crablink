import { useCallback, useEffect, useId, useState, type FormEvent } from 'react';
import { useTranslation } from 'react-i18next';
import { AcceptInvite, ChangePassword } from './Account';
import { api, type Membership, type User } from './api';
import { Clicks } from './Clicks';
import { Domains } from './Domains';
import { InstanceAdmin } from './InstanceAdmin';
import { Links } from './Links';
import { errorMessage, languages, setLanguage } from './i18n';
import { Chevron, interceptLinks, navigate, PageError, useLocationKey, usePopover } from './ui';
import { ManageWorkspace } from './Workspace';
import logoReversed from './assets/crablink-horizontal-reversed.svg';

// The switcher's pick, kept across visits.
const pickKey = 'crablink.workspace';
function savedPick() {
  try {
    return localStorage.getItem(pickKey) ?? undefined;
  } catch {
    return undefined;
  }
}

/** `/app/{page}/{rest…}`, decoded; `links` for the Links page at `/app/`. */
function route(): [string, string[]] {
  const [page = 'links', ...rest] = location.pathname
    .replace(/^\/app\/?/, '')
    .split('/')
    .filter(Boolean)
    .map(decodeURIComponent);
  return [page, rest];
}

export function App() {
  const { t } = useTranslation();
  useLocationKey();
  const [page, rest] = route();
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
      // Storage is blocked: the pick lasts until the next visit.
    }
  }, [slug]);

  useEffect(() => {
    document.addEventListener('click', interceptLinks);
    return () => document.removeEventListener('click', interceptLinks);
  }, []);

  async function signOut() {
    await api('DELETE', '/session');
    setUser(null);
    setWorkspaces(undefined);
  }

  function switchTo(next: string) {
    setSlug(next);
    // A picked Link belongs to the Workspace it was picked in.
    if (page === 'links' && rest.length) navigate('/app/', { replace: true });
  }

  const workspace = workspaces?.find((w) => w.slug === slug);
  let main;
  let board = false;
  if (page === 'invite') main = <AcceptInvite />;
  else if (user === undefined) main = null;
  else if (!user) main = <SignIn onSignedIn={signedInAs} />;
  else if (error !== undefined)
    main = (
      <PageError
        error={error}
        onRetry={() => {
          setError(undefined);
          refresh().catch(setError);
        }}
      />
    );
  else if (page === 'admin' && user.instance_admin) main = <InstanceAdmin me={user.email} />;
  else if (page === 'account') main = <ChangePassword />;
  else if (page === 'settings') main = <Settings user={user} workspace={workspace} onSignOut={signOut} />;
  else if (!workspaces) main = null;
  else if (page === 'workspace')
    main = <ManageWorkspace key={slug} workspace={workspace} email={user.email} onChanged={refresh} />;
  else if (!workspace) main = <p className="sheet empty">{t('workspace.none')}</p>;
  else if (page === 'clicks') main = <Clicks key={slug} slug={workspace.slug} />;
  else if (page === 'domains') main = <Domains key={slug} workspace={workspace} />;
  else {
    board = true;
    const [domain, code] = rest;
    main = <Links key={slug} slug={workspace.slug} pick={code ? { domain, code } : undefined} />;
  }

  // The pages the phone's Settings tab leads to: there it stays lit, with a way back to the sheet.
  const inSettings = ['admin', 'account', 'workspace', 'domains'].includes(page) && !!user;
  const tabs: [string, string, boolean][] = [
    ['/app/', 'nav.links', page === 'links'],
    ['/app/clicks', 'nav.clicks', page === 'clicks'],
  ];

  return (
    <>
      <header className="band">
        {/* The band is Shell Ink in both themes, so it takes the reversed lockup (logo/GUIDELINES.md). */}
        <a className="logo" href="/app/">
          <img src={logoReversed} alt="crablink" width={123} height={36} />
        </a>
        {user && workspaces && workspaces.length > 0 && (
          <WorkspaceSwitcher workspaces={workspaces} current={workspace} onSwitch={switchTo} />
        )}
        {user && (
          <nav className="tabs" aria-label={t('nav.label')}>
            {tabs.map(([href, key, current]) => (
              <a key={href} href={href} aria-current={current ? 'page' : undefined}>
                {t(key)}
              </a>
            ))}
          </nav>
        )}
        <div className="tools">
          {user ? <UserMenu user={user} onSignOut={signOut} /> : <LanguageSwitcher />}
        </div>
      </header>
      <main className={board ? 'flush' : undefined}>
        {inSettings && (
          <a className="back to-settings" href="/app/settings">
            {t('nav.backToSettings')}
          </a>
        )}
        {main}
      </main>
      {user && (
        <nav className="tabbar" aria-label={t('nav.label')}>
          {[...tabs, ['/app/settings', 'nav.settings', page === 'settings'] as const].map(([href, key, current]) => (
            <a
              key={href}
              href={href}
              aria-current={current ? 'page' : href === '/app/settings' && inSettings ? 'true' : undefined}
            >
              {t(key)}
            </a>
          ))}
        </nav>
      )}
    </>
  );
}

/** The current Workspace and the User's role in it; opens onto the other Workspaces and its management pages. */
function WorkspaceSwitcher({
  workspaces,
  current,
  onSwitch,
}: {
  workspaces: Membership[];
  current?: Membership;
  onSwitch: (slug: string) => void;
}) {
  const { t } = useTranslation();
  const { open, setOpen, toggle, ref } = usePopover<HTMLDivElement>();
  const id = useId();
  return (
    <div className="menu-wrap ws" ref={ref}>
      <button className="band-button" aria-expanded={open} aria-controls={open ? id : undefined} onClick={toggle}>
        <span className="vh">{t('nav.switcher')}: </span>
        <span className="ws-name">{current?.name ?? t('nav.switcher')}</span>
        {current && <span className="stamp">{t(`roles.${current.role}`)}</span>}
        <Chevron />
      </button>
      {open && (
        <div id={id} className="popover">
          <ul aria-label={t('nav.switcher')}>
            {workspaces.map((w) => (
              <li key={w.slug}>
                <button
                  className="item"
                  aria-current={w.slug === current?.slug}
                  onClick={() => {
                    onSwitch(w.slug);
                    setOpen(false);
                  }}
                >
                  {w.name}
                </button>
              </li>
            ))}
          </ul>
          <WorkspaceLinks current={current} />
        </div>
      )}
    </div>
  );
}

/** The current Workspace's pages, and starting a new one. */
function WorkspaceLinks({ current }: { current?: Membership }) {
  const { t } = useTranslation();
  return (
    <ul className="links-list">
      {current && (
        <>
          <li>
            <a className="item" href="/app/domains">
              {t('nav.domains')}
            </a>
          </li>
          <li>
            <a className="item" href="/app/workspace">
              {t('nav.workspace')}
            </a>
          </li>
        </>
      )}
      <li>
        <a className="item" href="/app/workspace#new">
          {t('nav.newWorkspace')}
        </a>
      </li>
    </ul>
  );
}

/** The signed-in User: their Account, the language, Instance Admin for Instance Admins, signing out. */
function UserMenu({ user, onSignOut }: { user: User; onSignOut: () => void }) {
  const { t } = useTranslation();
  const { open, toggle, ref } = usePopover<HTMLDivElement>();
  const id = useId();
  return (
    <div className="menu-wrap user" ref={ref}>
      <button className="band-button" aria-expanded={open} aria-controls={open ? id : undefined} onClick={toggle}>
        <span className="vh">{t('nav.userMenu')}: </span>
        <span className="avatar" aria-hidden="true">
          {user.email[0].toUpperCase()}
        </span>
        {/* Only the avatar shows: the switcher beside it often names a Workspace after this same email. */}
        <span className="vh">{user.email}</span>
        <Chevron />
      </button>
      {open && (
        <div id={id} className="popover end">
          <p className="menu-who">{user.email}</p>
          <AccountItems user={user} onSignOut={onSignOut} />
        </div>
      )}
    </div>
  );
}

function AccountItems({ user, onSignOut }: { user: User; onSignOut: () => void }) {
  const { t } = useTranslation();
  return (
    <>
      <ul className="links-list">
        <li>
          <a className="item" href="/app/account">
            {t('nav.account')}
          </a>
        </li>
        {user.instance_admin && (
          <li>
            <a className="item" href="/app/admin">
              {t('nav.admin')}
            </a>
          </li>
        )}
      </ul>
      <LanguageToggle />
      <button className="item" onClick={onSignOut}>
        {t('signOut')}
      </button>
    </>
  );
}

/** On a phone: the Workspace's and the account's pages, which the header menus hold on a wider screen. */
function Settings({ user, workspace, onSignOut }: { user: User; workspace?: Membership; onSignOut: () => void }) {
  const { t } = useTranslation();
  return (
    <>
      <div className="page-head">
        <h1>{t('nav.settings')}</h1>
      </div>
      {workspace && (
        <section className="sheet settings">
          <h2>
            {workspace.name} <span className="stamp">{t(`roles.${workspace.role}`)}</span>
          </h2>
          <WorkspaceLinks current={workspace} />
        </section>
      )}
      <section className="sheet settings">
        <h2 className="mono">{user.email}</h2>
        <AccountItems user={user} onSignOut={onSignOut} />
      </section>
    </>
  );
}

/** The language as a select, alone in the signed-out band. */
function LanguageSwitcher() {
  const { t, i18n } = useTranslation();
  return (
    <label>
      <span className="vh">{t('language')}</span>
      <select name="language" value={i18n.language} onChange={(e) => setLanguage(e.target.value)}>
        {Object.entries(languages).map(([lng, name]) => (
          <option key={lng} value={lng}>
            {name}
          </option>
        ))}
      </select>
    </label>
  );
}

/** The language as one click between its two segments, inside the account menu: no dropdown in a dropdown. */
function LanguageToggle() {
  const { t, i18n } = useTranslation();
  return (
    <fieldset className="menu-field lang-toggle">
      <legend className="micro">{t('language')}</legend>
      <div className="segmented">
        {Object.entries(languages).map(([lng, name]) => (
          <label key={lng} lang={lng}>
            <input
              type="radio"
              name="language"
              value={lng}
              checked={i18n.language === lng}
              onChange={() => setLanguage(lng)}
            />
            {name}
          </label>
        ))}
      </div>
    </fieldset>
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
    <form className="ticket" onSubmit={submit}>
      <div className="ticket-body">
        <h1>{t('signIn.title')}</h1>
        <label className="field">
          <span className="micro">{t('signIn.email')}</span>
          <input name="email" type="email" autoComplete="username" required />
        </label>
        <label className="field">
          <span className="micro">{t('signIn.password')}</span>
          <input name="password" type="password" autoComplete="current-password" required />
        </label>
        {error !== undefined && <p role="alert">{errorMessage(t, error)}</p>}
      </div>
      <div className="tear">
        <button type="submit" className="primary">
          {t('signIn.submit')}
        </button>
      </div>
    </form>
  );
}
