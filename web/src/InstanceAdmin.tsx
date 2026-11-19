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
    <>
      <div className="page-head">
        <h1>{t('admin.title')}</h1>
      </div>
      {error !== undefined && <p role="alert">{errorMessage(t, error)}</p>}

      <section className="sheet">
        <h2>{t('admin.invite.title')}</h2>
        <form className="row" onSubmit={createInvite}>
          <label className="field">
            <span className="micro">{t('admin.invite.email')}</span>
            <input name="email" type="email" required />
          </label>
          <label className="field">
            <span className="micro">{t('admin.invite.workspace')}</span>
            <input name="workspace" className="mono" placeholder={t('admin.invite.workspaceHint')} />
          </label>
          <button type="submit" className="primary">
            {t('admin.invite.create')}
          </button>
        </form>
        {invite && (
          <div className="secret invite-link">
            <label className="field">
              <span className="micro">{t('admin.invite.link')}</span>
              <input name="invite" readOnly value={invite} onFocus={(e) => e.target.select()} />
            </label>
          </div>
        )}
      </section>

      <section className="sheet">
        <h2>{t('admin.domains.title')}</h2>
        {domains?.length === 0 ? (
          <p className="empty">{t('admin.domains.empty')}</p>
        ) : (
          <div className="table">
            <table>
              <thead>
                <tr>
                  <th>{t('domains.domain')}</th>
                  <th>{t('admin.domains.workspace')}</th>
                  <th>{t('domains.status')}</th>
                  <th />
                </tr>
              </thead>
              <tbody>
                {domains?.map((d) => (
                  <tr key={d.domain}>
                    <td className="mono">{d.domain}</td>
                    <td className="mono">{d.workspace_slug}</td>
                    <td>
                      <span className={`stamp ${d.status}`}>{t(`domains.statuses.${d.status}`)}</span>
                    </td>
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

      <section className="sheet">
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
                  <td className="mono">
                    {u.email}
                    {reset?.id === u.id && (
                      <label className="reset">
                        <span className="micro">{t('admin.users.newPassword')}</span> <output>{reset.password}</output>
                      </label>
                    )}
                  </td>
                  <td>
                    <span className="stamps">
                      {u.instance_admin && <span className="stamp">{t('admin.users.instanceAdmin')}</span>}
                      {u.disabled && <span className="stamp rejected">{t('admin.users.disabled')}</span>}
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
    </>
  );
}
