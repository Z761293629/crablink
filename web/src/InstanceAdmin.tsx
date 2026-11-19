import { useEffect, useState, type FormEvent } from 'react';
import { useTranslation } from 'react-i18next';
import { api, type DomainStatus, type ManagedDomain, type ManagedUser } from './api';
import { errorMessage } from './i18n';

const fetchAll = () =>
  Promise.all([api<ManagedUser[]>('GET', '/admin/users'), api<ManagedDomain[]>('GET', '/admin/domains')]);

/** Instance administration: Users, invites and custom Domains. The API refuses anyone but an Instance Admin. */
export function InstanceAdmin() {
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
  const setStatus = (domain: string, status: DomainStatus) =>
    act(() => api('PATCH', `/admin/domains/${encodeURIComponent(domain)}`, { status }));

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
    // Gate boards of Users and custom Domains, with the invite as a coupon beside them.
    <div className="desk">
      <div className="desk-board">
        <div className="board-head">
          <h1>{t('admin.title')}</h1>
        </div>
        {error !== undefined && <p role="alert">{errorMessage(t, error)}</p>}

        <section className="board-sec" aria-labelledby="users-title">
          <div className="board-title">
            <h2 id="users-title">{t('admin.users.title')}</h2>
            <span className="board-domain">{users?.length}</span>
          </div>
          <div className="table">
            <table className="gate" aria-labelledby="users-title">
              <thead>
                <tr>
                  <th>{t('admin.users.email')}</th>
                  <th>{t('domains.status')}</th>
                  <th>
                    <span className="vh">{t('links.actions')}</span>
                  </th>
                </tr>
              </thead>
              <tbody>
                {users?.map((u) => (
                  <tr key={u.id}>
                    <td className="mono primary">
                      {u.email}
                      {reset?.id === u.id && (
                        <label className="reset">
                          <span className="micro">{t('admin.users.newPassword')}</span> <output>{reset.password}</output>
                        </label>
                      )}
                    </td>
                    <td>
                      <span className="states">
                        {u.disabled && <span className="state rejected">{t('admin.users.disabled')}</span>}
                        {u.instance_admin && <span className="state default">{t('admin.users.instanceAdmin')}</span>}
                        {!u.disabled && !u.instance_admin && <span className="state">{t('admin.users.active')}</span>}
                      </span>
                    </td>
                    <td className="actions">
                      <button className="small" onClick={() => updateUser(u.id, { disabled: !u.disabled })}>
                        {u.disabled ? t('admin.users.enable') : t('admin.users.disable')}
                      </button>
                      <button className="small" onClick={() => updateUser(u.id, { instance_admin: !u.instance_admin })}>
                        {u.instance_admin ? t('admin.users.revokeAdmin') : t('admin.users.makeAdmin')}
                      </button>
                      <button className="small" onClick={() => resetPassword(u.id)}>
                        {t('admin.users.resetPassword')}
                      </button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </section>

        <section className="board-sec" aria-labelledby="custom-title">
          <div className="board-title">
            <h2 id="custom-title">{t('admin.domains.title')}</h2>
            <span className="board-domain">{domains?.length}</span>
          </div>
          {domains?.length === 0 ? (
            <p className="empty">{t('admin.domains.empty')}</p>
          ) : (
            <div className="table">
              <table className="gate" aria-labelledby="custom-title">
                <thead>
                  <tr>
                    <th>{t('domains.domain')}</th>
                    <th>{t('domains.status')}</th>
                    <th>{t('admin.domains.workspace')}</th>
                    <th>
                      <span className="vh">{t('links.actions')}</span>
                    </th>
                  </tr>
                </thead>
                <tbody>
                  {domains?.map((d) => (
                    <tr key={d.domain}>
                      <td className="mono primary">{d.domain}</td>
                      <td>
                        <span className={`state ${d.status}`}>{t(`domains.statuses.${d.status}`)}</span>
                      </td>
                      <td className="mono">{d.workspace_slug}</td>
                      <td className="actions">
                        {d.status !== 'approved' && (
                          <button className="small primary" onClick={() => setStatus(d.domain, 'approved')}>
                            {t('admin.domains.approve')}
                          </button>
                        )}
                        {d.status !== 'rejected' && (
                          <button className="small" onClick={() => setStatus(d.domain, 'rejected')}>
                            {t('admin.domains.reject')}
                          </button>
                        )}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </section>
      </div>

      <form className="pass ticket" aria-labelledby="invite-title" onSubmit={createInvite}>
        <div className="ticket-body">
          <h2 id="invite-title">{t('admin.invite.title')}</h2>
          <label className="field">
            <span className="micro">{t('admin.invite.email')}</span>
            <input name="email" type="email" required />
          </label>
          <label className="field">
            <span className="micro">{t('admin.invite.workspace')}</span>
            <input name="workspace" className="mono" placeholder={t('admin.invite.workspaceHint')} />
          </label>
        </div>
        <div className="tear">
          <button type="submit" className="primary">
            {t('admin.invite.create')}
          </button>
          {invite && (
            <label className="field">
              <span className="micro">{t('admin.invite.link')}</span>
              <input name="invite" readOnly value={invite} onFocus={(e) => e.target.select()} />
            </label>
          )}
        </div>
      </form>
    </div>
  );
}
