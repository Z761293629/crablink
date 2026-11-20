// Managing a Workspace: renaming, deleting and leaving it, its members and its API
// Keys (Owners only, as the API enforces), and creating a new one.

import { useCallback, useEffect, useRef, useState, type FormEvent, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { api, type ApiKey, type Membership, type NewApiKey, type WorkspaceUser } from './api';
import { errorMessage } from './i18n';
import { useConfirm, useLocationKey } from './ui';

/** Where an action's error shows: in the form that ran it, or over the page. */
type At = 'page' | 'rename' | 'members' | 'keys' | 'new';

/** Runs an action, showing its error at `at`. */
type Run = (action: () => Promise<unknown>, at?: At) => Promise<void>;

/** The error of the action run at `at`, if that is where the last one failed. */
type Alert = (at: At) => ReactNode;

/** Asks "Are you sure?" in the page's dialog; resolves to whether `action` was chosen. */
type Ask = (message: string, action: string) => Promise<boolean>;

/** Calls `submit` with the form's fields, then clears the form if it succeeded. */
function onSubmit(submit: (fields: Record<string, string>) => Promise<boolean | void>) {
  return async (e: FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    const form = e.currentTarget;
    const fields = Object.fromEntries(new FormData(form)) as Record<string, string>;
    if ((await submit(fields)) !== false) form.reset();
  };
}

export function ManageWorkspace({
  workspace,
  email,
  onChanged,
}: {
  /** None when the User is in no Workspace. */
  workspace?: Membership;
  /** The signed-in User's. */
  email: string;
  /** Reloads the User's Workspaces, switching to `prefer` if given. */
  onChanged: (prefer?: string) => Promise<void>;
}) {
  const { t } = useTranslation();
  const [error, setError] = useState<{ at: At; error: unknown }>();
  const [dialog, ask] = useConfirm();
  const run: Run = useCallback(async (action, at = 'page') => {
    setError(undefined);
    try {
      await action();
    } catch (err) {
      setError({ at, error: err });
      throw err;
    }
  }, []);
  // An error is already shown; the form keeps what was typed.
  const attempt = (action: () => Promise<unknown>, at?: At) => run(action, at).then(
    () => true,
    () => false,
  );
  const alert: Alert = (at) => error?.at === at && <p role="alert">{errorMessage(t, error.error)}</p>;

  // `#new`, from New Workspace in the switcher: straight to naming it.
  const where = useLocationKey();
  useEffect(() => {
    if (where.endsWith('#new')) document.querySelector<HTMLInputElement>('#new input')?.focus();
  }, [where]);

  const path = workspace && `/workspaces/${workspace.slug}`;
  const owner = workspace?.role === 'owner';

  const leave = async () =>
    (await ask(t('workspace.confirmLeave', { name: workspace!.name }), t('workspace.leave'))) &&
    attempt(async () => {
      await api('DELETE', `${path}/members/${encodeURIComponent(email)}`);
      await onChanged();
    });
  const remove = async () =>
    (await ask(t('workspace.confirmDelete', { name: workspace!.name }), t('workspace.delete'))) &&
    attempt(async () => {
      await api('DELETE', path!);
      await onChanged();
    });

  return (
    <div className="counter">
      {alert('page')}
      {workspace ? (
        // The Workspace as a pass: its name as the Code, the role and members as segments,
        // and on the stub its API Keys, then leaving and deleting it.
        <section className="pass ws" aria-labelledby="ws-name">
          <div className="ticket wide">
            <div className="ticket-body">
              <div className="route">
                <div className="from">
                  <span className="micro">{t('nav.switcher')}</span>
                  <h1 id="ws-name" className="code">
                    {workspace.name}
                  </h1>
                </div>
              </div>
              <div className="segs">
                <div className={owner ? 'seg l' : 'seg wide'}>
                  <p className="micro">{t('members.role')}</p>
                  <p className="val">{t(`roles.${workspace.role}`)}</p>
                </div>
                {owner && (
                  <form
                    className="seg r field"
                    onSubmit={onSubmit(({ name }) =>
                      attempt(async () => {
                        await api('PATCH', path!, { name });
                        await onChanged();
                      }, 'rename'),
                    )}
                  >
                    <label className="micro" htmlFor="ws-rename">
                      {t('workspace.name')}
                    </label>
                    <span className="inline">
                      <input id="ws-rename" name="name" defaultValue={workspace.name} required maxLength={100} />
                      <button type="submit">{t('workspace.rename')}</button>
                    </span>
                    {alert('rename')}
                  </form>
                )}
                {owner && (
                  <Members
                    path={path!}
                    email={email}
                    run={run}
                    attempt={attempt}
                    ask={ask}
                    alert={alert}
                    onChanged={onChanged}
                  />
                )}
              </div>
            </div>
            <div className="tear">
              {owner && <ApiKeys path={path!} run={run} attempt={attempt} ask={ask} alert={alert} />}
              <p className="leave">
                <button onClick={leave}>{t('workspace.leave')}</button>
                {owner && (
                  <button className="danger" onClick={remove}>
                    {t('workspace.delete')}
                  </button>
                )}
              </p>
            </div>
          </div>
        </section>
      ) : (
        <p className="empty">{t('workspace.none')}</p>
      )}
      <form
        id="new"
        className="pass ticket"
        aria-labelledby="new-ws"
        onSubmit={onSubmit(({ name }) =>
          attempt(async () => {
            const created = await api<Membership>('POST', '/workspaces', { name });
            await onChanged(created.slug);
          }, 'new'),
        )}
      >
        <div className="ticket-body">
          <h2 id="new-ws">{t('nav.newWorkspace')}</h2>
          <label className="field">
            <span className="micro">{t('workspace.newName')}</span>
            <input name="name" required maxLength={100} />
          </label>
          {alert('new')}
        </div>
        <div className="tear">
          <button type="submit" className="primary">
            {t('workspace.create')}
          </button>
        </div>
      </form>
      {dialog}
    </div>
  );
}

interface SectionProps {
  /** `/workspaces/{slug}`. */
  path: string;
  run: Run;
  attempt: (action: () => Promise<unknown>, at?: At) => Promise<boolean>;
  ask: Ask;
  alert: Alert;
}

function Members({
  path,
  email,
  run,
  attempt,
  ask,
  alert,
  onChanged,
}: SectionProps & { email: string; onChanged: () => Promise<void> }) {
  const { t } = useTranslation();
  // None until loaded, so the count never flashes 0.
  const [members, setMembers] = useState<WorkspaceUser[]>();
  const load = useCallback(
    () => run(async () => setMembers(await api<WorkspaceUser[]>('GET', `${path}/members`))),
    [path, run],
  );
  useEffect(() => {
    load().catch(() => {});
  }, [load]);

  /** Changes `member`'s role, or removes them. An Owner demoting themself reloads their Workspaces instead, as they can no longer list members. */
  const change = (member: string, role?: WorkspaceUser['role']) =>
    attempt(async () => {
      const url = `${path}/members/${encodeURIComponent(member)}`;
      await (role ? api('PATCH', url, { role }) : api('DELETE', url));
      await (member === email ? onChanged() : load());
    }, 'members');

  // A Workspace keeps an Owner: the only one isn't offered Make Member.
  const owners = members?.filter((m) => m.role === 'owner').length ?? 0;
  return (
    <section className="seg wide">
      <h2 className="micro">
        {t('members.title')}
        {members && ` · ${members.length}`}
      </h2>
      <div className="table">
        <table className="gate" aria-label={t('members.title')}>
          <thead>
            <tr>
              <th>{t('members.email')}</th>
              <th>{t('members.role')}</th>
              <th />
            </tr>
          </thead>
          <tbody>
            {members?.map((m) => (
              <tr key={m.email}>
                <td className="mono">{m.email}</td>
                <td>
                  <span className="state">{t(`roles.${m.role}`)}</span>
                </td>
                <td className="actions">
                  {!(m.role === 'owner' && owners === 1) && (
                    <button className="small" onClick={() => change(m.email, m.role === 'owner' ? 'member' : 'owner')}>
                      {t(m.role === 'owner' ? 'members.makeMember' : 'members.makeOwner')}
                    </button>
                  )}
                  {m.email !== email && (
                    <button
                      className="danger small"
                      onClick={async () =>
                        (await ask(t('members.confirmRemove', { email: m.email }), t('members.confirmRemoveAction'))) &&
                        change(m.email)
                      }
                    >
                      {t('members.remove')}
                    </button>
                  )}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <form
        className="row add"
        onSubmit={onSubmit(({ email }) => attempt(async () => {
          await api('POST', `${path}/members`, { email });
          await load();
        }, 'members'))}
      >
        <label className="field">
          <span className="micro">{t('members.addEmail')}</span>
          <input name="email" type="email" required />
        </label>
        <button type="submit">
          {t('members.add')}
        </button>
      </form>
      {alert('members')}
    </section>
  );
}

function ApiKeys({ path, run, attempt, ask, alert }: SectionProps) {
  const { t, i18n } = useTranslation();
  const secret = useRef<HTMLInputElement>(null);
  const [copyFailed, setCopyFailed] = useState(false);
  const [keys, setKeys] = useState<ApiKey[]>([]);
  // The key just created: its secret is shown this once.
  const [created, setCreated] = useState<NewApiKey>();
  const load = useCallback(
    () => run(async () => setKeys(await api<ApiKey[]>('GET', `${path}/api-keys`))),
    [path, run],
  );
  useEffect(() => {
    load().catch(() => {});
  }, [load]);

  const date = new Intl.DateTimeFormat(i18n.language, { dateStyle: 'medium', timeStyle: 'short' });
  return (
    <section className="keys">
      <h2 className="micro">{t('apiKeys.title')}</h2>
      {keys.length === 0 ? (
        <p className="empty">{t('apiKeys.empty')}</p>
      ) : (
        <div className="table">
          <table className="gate entries" aria-label={t('apiKeys.title')}>
            <thead>
              <tr>
                <th>{t('apiKeys.name')}</th>
                <th>{t('apiKeys.prefix')}</th>
                <th>{t('apiKeys.createdAt')}</th>
                <th>{t('apiKeys.lastUsed')}</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {keys.map((key) => (
                <tr key={key.id}>
                  <td className="name">{key.name}</td>
                  <td>
                    <code>{key.prefix}…</code>
                  </td>
                  <td className="mono">{date.format(new Date(key.created_at))}</td>
                  <td className="mono">{key.last_used_at ? date.format(new Date(key.last_used_at)) : t('apiKeys.never')}</td>
                  <td className="actions">
                    <button
                      className="danger small"
                      onClick={async () =>
                        (await ask(t('apiKeys.confirmRevoke', { name: key.name }), t('apiKeys.confirmRevokeAction'))) &&
                        attempt(async () => {
                          await api('DELETE', `${path}/api-keys/${key.id}`);
                          if (created?.id === key.id) setCreated(undefined);
                          await load();
                        }, 'keys')
                      }
                    >
                      {t('apiKeys.revoke')}
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
      {created && (
        <div className="secret">
          <p>{t('apiKeys.secretOnce', { name: created.name })}</p>
          <div className="row">
            <label className="field">
              <span className="micro">{t('apiKeys.secret')}</span>
              <input ref={secret} name="secret" readOnly value={created.secret} onFocus={(e) => e.target.select()} />
            </label>
            <button
              className="primary"
              onClick={async () => {
                try {
                  await navigator.clipboard.writeText(created.secret);
                  setCopyFailed(false);
                } catch {
                  // No clipboard here: leave the secret selected to copy by hand.
                  secret.current?.select();
                  setCopyFailed(true);
                }
              }}
            >
              {t('apiKeys.copy')}
            </button>
          </div>
          {copyFailed && <p role="status">{t('apiKeys.copyFailed')}</p>}
        </div>
      )}
      <form
        className="row add"
        onSubmit={onSubmit(({ name }) => attempt(async () => {
          setCreated(await api<NewApiKey>('POST', `${path}/api-keys`, { name }));
          await load();
        }, 'keys'))}
      >
        <label className="field">
          <span className="micro">{t('apiKeys.newName')}</span>
          <input name="name" required maxLength={100} />
        </label>
        <button type="submit">
          {t('apiKeys.create')}
        </button>
      </form>
      {alert('keys')}
    </section>
  );
}
