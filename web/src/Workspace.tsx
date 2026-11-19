// Managing a Workspace: renaming, deleting and leaving it, its members and its API
// Keys (Owners only, as the API enforces), and creating a new one.

import { useCallback, useEffect, useState, type FormEvent } from 'react';
import { useTranslation } from 'react-i18next';
import { api, type ApiKey, type Membership, type NewApiKey, type WorkspaceUser } from './api';
import { errorMessage } from './i18n';
import { useLocationKey } from './ui';

/** Runs an action, showing its error in the page's one alert. */
type Run = (action: () => Promise<unknown>) => Promise<void>;

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
  const [error, setError] = useState<unknown>();
  const run: Run = useCallback(async (action) => {
    setError(undefined);
    try {
      await action();
    } catch (err) {
      setError(err);
      throw err;
    }
  }, []);
  // An error is already shown; the form keeps what was typed.
  const attempt = (action: () => Promise<unknown>) => run(action).then(
    () => true,
    () => false,
  );

  // `#new`, from New Workspace in the switcher: straight to naming it.
  const where = useLocationKey();
  useEffect(() => {
    if (where.endsWith('#new')) document.querySelector<HTMLInputElement>('#new input')?.focus();
  }, [where]);

  const path = workspace && `/workspaces/${workspace.slug}`;
  const owner = workspace?.role === 'owner';

  return (
    <>
      {workspace && (
        <div className="page-head">
          <div className="title">
            <h1>{workspace.name}</h1>
            <span className="stamp">{t(`roles.${workspace.role}`)}</span>
          </div>
        </div>
      )}
      {error !== undefined && <p role="alert">{errorMessage(t, error)}</p>}
      {workspace && owner && (
        <>
          <form
            className="sheet row"
            onSubmit={onSubmit(({ name }) => attempt(async () => {
              await api('PATCH', path!, { name });
              await onChanged();
            }))}
          >
            <label className="field">
              <span className="micro">{t('workspace.name')}</span>
              <input name="name" defaultValue={workspace.name} required maxLength={100} />
            </label>
            <button type="submit" className="primary">
              {t('workspace.rename')}
            </button>
          </form>
          <Members path={path!} email={email} run={run} attempt={attempt} onChanged={onChanged} />
          <ApiKeys path={path!} run={run} attempt={attempt} />
        </>
      )}
      {workspace && (
        <p className="sheet leave">
          <button
            onClick={() =>
              confirm(t('workspace.confirmLeave', { name: workspace.name })) &&
              attempt(async () => {
                await api('DELETE', `${path}/members/${encodeURIComponent(email)}`);
                await onChanged();
              })
            }
          >
            {t('workspace.leave')}
          </button>
          {owner && (
            <button
              className="danger"
              onClick={() =>
                confirm(t('workspace.confirmDelete', { name: workspace.name })) &&
                attempt(async () => {
                  await api('DELETE', path!);
                  await onChanged();
                })
              }
            >
              {t('workspace.delete')}
            </button>
          )}
        </p>
      )}
      {!workspace && <p className="sheet empty">{t('workspace.none')}</p>}
      <section className="sheet" id="new">
        <h2>{t('workspace.create')}</h2>
        <form
          className="row"
        onSubmit={onSubmit(({ name }) => attempt(async () => {
          const created = await api<Membership>('POST', '/workspaces', { name });
          await onChanged(created.slug);
        }))}
      >
        <label className="field">
          <span className="micro">{t('workspace.newName')}</span>
          <input name="name" required maxLength={100} />
        </label>
        <button type="submit" className="primary">
          {t('workspace.create')}
        </button>
        </form>
      </section>
    </>
  );
}

interface SectionProps {
  /** `/workspaces/{slug}`. */
  path: string;
  run: Run;
  attempt: (action: () => Promise<unknown>) => Promise<boolean>;
}

function Members({
  path,
  email,
  run,
  attempt,
  onChanged,
}: SectionProps & { email: string; onChanged: () => Promise<void> }) {
  const { t } = useTranslation();
  const [members, setMembers] = useState<WorkspaceUser[]>([]);
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
    });

  return (
    <section className="sheet">
      <h2>{t('members.title')}</h2>
      <div className="table">
        <table aria-label={t('members.title')}>
          <thead>
            <tr>
              <th>{t('members.email')}</th>
              <th>{t('members.role')}</th>
              <th />
            </tr>
          </thead>
          <tbody>
            {members.map((m) => (
              <tr key={m.email}>
                <td>{m.email}</td>
                <td>
                  <span className="stamp">{t(`roles.${m.role}`)}</span>
                </td>
                <td className="actions">
                  <button className="small" onClick={() => change(m.email, m.role === 'owner' ? 'member' : 'owner')}>
                    {t(m.role === 'owner' ? 'members.makeMember' : 'members.makeOwner')}
                  </button>
                  {m.email !== email && (
                    <button
                      className="danger small"
                      onClick={() => confirm(t('members.confirmRemove', { email: m.email })) && change(m.email)}
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
        className="row"
        onSubmit={onSubmit(({ email }) => attempt(async () => {
          await api('POST', `${path}/members`, { email });
          await load();
        }))}
      >
        <label className="field">
          <span className="micro">{t('members.addEmail')}</span>
          <input name="email" type="email" required />
        </label>
        <button type="submit" className="primary">
          {t('members.add')}
        </button>
      </form>
    </section>
  );
}

function ApiKeys({ path, run, attempt }: SectionProps) {
  const { t, i18n } = useTranslation();
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
    <section className="sheet">
      <h2>{t('apiKeys.title')}</h2>
      {keys.length === 0 ? (
        <p className="empty">{t('apiKeys.empty')}</p>
      ) : (
        <div className="table">
          <table aria-label={t('apiKeys.title')}>
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
                  <td>{key.name}</td>
                  <td>
                    <code>{key.prefix}…</code>
                  </td>
                  <td className="mono">{date.format(new Date(key.created_at))}</td>
                  <td className="mono">{key.last_used_at ? date.format(new Date(key.last_used_at)) : t('apiKeys.never')}</td>
                  <td className="actions">
                    <button
                      className="danger small"
                      onClick={() =>
                        confirm(t('apiKeys.confirmRevoke', { name: key.name })) &&
                        attempt(async () => {
                          await api('DELETE', `${path}/api-keys/${key.id}`);
                          if (created?.id === key.id) setCreated(undefined);
                          await load();
                        })
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
              <input readOnly value={created.secret} onFocus={(e) => e.target.select()} />
            </label>
            <button className="primary" onClick={() => navigator.clipboard.writeText(created.secret)}>
              {t('apiKeys.copy')}
            </button>
          </div>
        </div>
      )}
      <form
        className="row"
        onSubmit={onSubmit(({ name }) => attempt(async () => {
          setCreated(await api<NewApiKey>('POST', `${path}/api-keys`, { name }));
          await load();
        }))}
      >
        <label className="field">
          <span className="micro">{t('apiKeys.newName')}</span>
          <input name="name" required maxLength={100} />
        </label>
        <button type="submit" className="primary">
          {t('apiKeys.create')}
        </button>
      </form>
    </section>
  );
}
