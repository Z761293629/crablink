import { useEffect, useState, type FormEvent } from 'react';
import { useTranslation } from 'react-i18next';
import { api, type DomainStatus, type ManagedDomain, type ManagedUser } from './api';
import { errorMessage } from './i18n';

const fetchAll = () =>
  Promise.all([api<ManagedUser[]>('GET', '/admin/users'), api<ManagedDomain[]>('GET', '/admin/domains')]);

/** Instance administration: Users, invites and custom Domains. The API refuses anyone but an Instance Admin. */
export function Admin() {
  const { t } = useTranslation();
  const [users, setUsers] = useState<ManagedUser[]>();
  const [domains, setDomains] = useState<ManagedDomain[]>();
  // A just reset password, shown once next to its User.
  const [reset, setReset] = useState<{ id: number; password: string }>();
  const [invite, setInvite] = useState<string>();
  const [error, setError] = useState<unknown>();

  async function load() {
    const [u, d] = await fetchAll();
    setUsers(u);
    setDomains(d);
  }

  useEffect(() => {
    fetchAll().then(([u, d]) => {
      setUsers(u);
      setDomains(d);
    }, setError);
  }, []);

  // Runs an action, then reloads; shows its error, if any.
  async function act(action: () => Promise<unknown>) {
    try {
      await action();
      setError(undefined);
      await load();
    } catch (err) {
      setError(err);
    }
  }

  const updateUser = (id: number, change: Partial<ManagedUser>) => act(() => api('PATCH', `/admin/users/${id}`, change));
  const resetPassword = (id: number) =>
    act(async () => setReset({ id, ...(await api<{ password: string }>('POST', `/admin/users/${id}/password`)) }));
  const setStatus = (host: string, status: DomainStatus) =>
    act(() => api('PATCH', `/admin/domains/${encodeURIComponent(host)}`, { status }));

  async function createInvite(e: FormEvent<HTMLFormElement>) {
    e.preventDefault();
    const form = new FormData(e.currentTarget);
    const workspace = (form.get('workspace') as string).trim();
    await act(async () =>
      setInvite(
        (await api<{ url: string }>('POST', '/admin/invites', { email: form.get('email'), workspace: workspace || null }))
          .url,
      ),
    );
  }

  return (
    <>
      <h1>{t('admin.title')}</h1>
      {error !== undefined && <p role="alert">{errorMessage(t, error)}</p>}

      <h2>{t('admin.invite.title')}</h2>
      <form className="inline" onSubmit={createInvite}>
        <label>
          {t('admin.invite.email')} <input name="email" type="email" required />
        </label>
        <label>
          {t('admin.invite.workspace')} <input name="workspace" placeholder={t('admin.invite.workspaceHint')} />
        </label>
        <button type="submit">{t('admin.invite.create')}</button>
      </form>
      {invite && (
        <label className="inline">
          {t('admin.invite.link')} <input readOnly value={invite} onFocus={(e) => e.target.select()} />
        </label>
      )}

      <h2>{t('admin.domains.title')}</h2>
      {domains?.length === 0 ? (
        <p>{t('admin.domains.empty')}</p>
      ) : (
        <div className="table">
          <table>
            <thead>
              <tr>
                <th>{t('domains.host')}</th>
                <th>{t('admin.domains.workspace')}</th>
                <th>{t('domains.status')}</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {domains?.map((d) => (
                <tr key={d.host}>
                  <td>{d.host}</td>
                  <td>{d.workspace}</td>
                  <td>{t(`domains.statuses.${d.status}`)}</td>
                  <td>
                    {d.status !== 'approved' && (
                      <button onClick={() => setStatus(d.host, 'approved')}>{t('admin.domains.approve')}</button>
                    )}
                    {d.status !== 'rejected' && (
                      <button onClick={() => setStatus(d.host, 'rejected')}>{t('admin.domains.reject')}</button>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      <h2>{t('admin.users.title')}</h2>
      <div className="table">
        <table>
          <thead>
            <tr>
              <th>{t('admin.users.email')}</th>
              <th>{t('admin.users.roles')}</th>
              <th />
            </tr>
          </thead>
          <tbody>
            {users?.map((u) => (
              <tr key={u.id}>
                <td>{u.email}</td>
                <td>
                  {[u.instance_admin && t('admin.users.instanceAdmin'), u.disabled && t('admin.users.disabled')]
                    .filter(Boolean)
                    .join(', ')}
                </td>
                <td>
                  <button onClick={() => updateUser(u.id, { disabled: !u.disabled })}>
                    {u.disabled ? t('admin.users.enable') : t('admin.users.disable')}
                  </button>
                  <button onClick={() => updateUser(u.id, { instance_admin: !u.instance_admin })}>
                    {u.instance_admin ? t('admin.users.revokeAdmin') : t('admin.users.makeAdmin')}
                  </button>
                  <button onClick={() => resetPassword(u.id)}>{t('admin.users.resetPassword')}</button>
                  {reset?.id === u.id && (
                    <label>
                      {' '}
                      {t('admin.users.newPassword')} <output>{reset.password}</output>
                    </label>
                  )}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </>
  );
}
