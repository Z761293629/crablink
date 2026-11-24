import { useEffect, useState, type FormEvent } from 'react';
import { useTranslation } from 'react-i18next';
import { api, type BlockedLink, type DomainStatus, type ManagedDomain, type ManagedUser } from './api';
import { errorMessage } from './i18n';
import { useConfirm } from './ui';

const fetchAll = () =>
  Promise.all([
    api<ManagedUser[]>('GET', '/admin/users'),
    api<ManagedDomain[]>('GET', '/admin/domains'),
    api<BlockedLink[]>('GET', '/admin/blocked-links'),
  ]);

/** Instance administration: Users, invites, custom Domains and Blocked Links. The API refuses anyone but an Instance Admin. */
export function InstanceAdmin({ me }: { /** The signed-in User's email. */ me: string }) {
  const { t } = useTranslation();
  const [dialog, ask] = useConfirm();
  const [users, setUsers] = useState<ManagedUser[]>();
  const [domains, setDomains] = useState<ManagedDomain[]>();
  const [blocked, setBlocked] = useState<BlockedLink[]>();
  // A just reset password, shown once next to its User.
  const [reset, setReset] = useState<{ id: number; password: string }>();
  const [invite, setInvite] = useState<string>();
  const [error, setError] = useState<unknown>();

  function show([u, d, b]: Awaited<ReturnType<typeof fetchAll>>) {
    setUsers(u);
    setDomains(d);
    setBlocked(b);
  }

  const load = async () => show(await fetchAll());

  useEffect(() => {
    fetchAll().then(show, setError);
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

  // Taking something away is asked first: a User signed out, rights revoked, an approved Domain's Short URLs dead.
  const disable = async (u: ManagedUser) =>
    (await ask(t('admin.users.confirmDisable', { email: u.email }), t('admin.users.confirmDisableAction'))) &&
    updateUser(u.id, { disabled: true });
  const revoke = async (u: ManagedUser) =>
    (await ask(t('admin.users.confirmRevoke', { email: u.email }), t('admin.users.confirmRevokeAction'))) &&
    updateUser(u.id, { instance_admin: false });
  const reject = async (d: ManagedDomain) =>
    (d.status !== 'approved' ||
      (await ask(t('admin.domains.confirmReject', { domain: d.domain }), t('admin.domains.confirmRejectAction')))) &&
    setStatus(d.domain, 'rejected');

  const unblock = (b: BlockedLink) =>
    act(() => api('DELETE', `/admin/blocked-links?short_url=${encodeURIComponent(b.short_url)}`));

  async function block(e: FormEvent<HTMLFormElement>) {
    e.preventDefault();
    const form = e.currentTarget;
    const shortUrl = (new FormData(form).get('short_url') as string).trim();
    if (!(await ask(t('admin.blocked.confirmBlock', { url: shortUrl }), t('admin.blocked.confirmBlockAction')))) return;
    await act(async () => {
      await api('POST', '/admin/blocked-links', { short_url: shortUrl });
      form.reset();
    });
  }

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
                      {u.email === me && <span className="micro"> · {t('admin.users.you')}</span>}
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
                      {/* Never offered on one's own row: an Instance Admin can't lock themself out. */}
                      {u.email !== me && (
                        <>
                          <button
                            className="small"
                            onClick={() => (u.disabled ? updateUser(u.id, { disabled: false }) : disable(u))}
                          >
                            {u.disabled ? t('admin.users.enable') : t('admin.users.disable')}
                          </button>
                          <button
                            className="small"
                            onClick={() => (u.instance_admin ? revoke(u) : updateUser(u.id, { instance_admin: true }))}
                          >
                            {u.instance_admin ? t('admin.users.revokeAdmin') : t('admin.users.makeAdmin')}
                          </button>
                        </>
                      )}
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
                          <button className="small" onClick={() => reject(d)}>
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

        <section className="board-sec" aria-labelledby="blocked-title">
          <div className="board-title">
            <h2 id="blocked-title">{t('admin.blocked.title')}</h2>
            <span className="board-domain">{blocked?.length}</span>
          </div>
          {blocked?.length === 0 ? (
            <p className="empty">{t('admin.blocked.empty')}</p>
          ) : (
            <div className="table">
              <table className="gate" aria-labelledby="blocked-title">
                <thead>
                  <tr>
                    <th>{t('links.shortUrl')}</th>
                    <th>{t('links.target')}</th>
                    <th>{t('admin.domains.workspace')}</th>
                    <th>
                      <span className="vh">{t('links.actions')}</span>
                    </th>
                  </tr>
                </thead>
                <tbody>
                  {blocked?.map((b) => (
                    <tr key={b.short_url}>
                      <td className="mono primary">{b.short_url.replace(/^https?:\/\//, '')}</td>
                      <td className="mono">{b.target}</td>
                      <td className="mono">{b.workspace_slug}</td>
                      <td className="actions">
                        <button className="small" onClick={() => unblock(b)}>
                          {t('admin.blocked.unblock')}
                        </button>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </section>
      </div>

      <div className="coupons">
        <form className="pass ticket" aria-labelledby="block-title" onSubmit={block}>
          <div className="ticket-body">
            <h2 id="block-title">{t('admin.blocked.blockTitle')}</h2>
            <label className="field">
              <span className="micro">{t('links.shortUrl')}</span>
              <input name="short_url" className="mono" placeholder="crabl.ink/aZ3k9" required />
            </label>
          </div>
          <div className="tear">
            <button type="submit" className="danger">
              {t('admin.blocked.block')}
            </button>
          </div>
        </form>

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
      {dialog}
    </div>
  );
}
