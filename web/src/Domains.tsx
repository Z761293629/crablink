import { useEffect, useState, type FormEvent } from 'react';
import { useTranslation } from 'react-i18next';
import { api, type Domain, type Membership } from './api';
import { errorMessage } from './i18n';
import { PageError, useConfirm } from './ui';

/** The Default Domain and the custom Domains of `workspace`; Owners add and remove them. */
export function Domains({ workspace }: { workspace: Membership }) {
  const { t } = useTranslation();
  const [domains, setDomains] = useState<Domain[]>();
  const [error, setError] = useState<unknown>();
  const [tries, setTries] = useState(0);
  const [dialog, ask] = useConfirm();

  const load = async (slug: string) => setDomains(await api<Domain[]>('GET', `/workspaces/${slug}/domains`));

  useEffect(() => {
    api<Domain[]>('GET', `/workspaces/${workspace.slug}/domains`).then(setDomains, setError);
  }, [workspace.slug, tries]);

  async function add(e: FormEvent<HTMLFormElement>) {
    e.preventDefault();
    const form = e.currentTarget;
    try {
      await api('POST', `/workspaces/${workspace.slug}/domains`, { domain: new FormData(form).get('domain') });
      form.reset();
      setError(undefined);
      await load(workspace.slug);
    } catch (err) {
      setError(err);
    }
  }

  async function remove(domain: string) {
    if (!(await ask(t('domains.confirmRemove', { domain }), t('domains.confirmRemoveAction')))) return;
    try {
      await api('DELETE', `/workspaces/${workspace.slug}/domains/${encodeURIComponent(domain)}`);
      await load(workspace.slug);
    } catch (err) {
      setError(err);
    }
  }

  if (!domains)
    return error === undefined ? null : (
      <PageError
        error={error}
        onRetry={() => {
          setError(undefined);
          setTries((n) => n + 1);
        }}
      />
    );
  const owner = workspace.role === 'owner';
  // DNS points at a host name, never a port.
  const target = domains.find((d) => d.default)?.domain.replace(/:\d+$/, '');
  return (
    // A gate board: each Domain a row, its status in the mono column.
    <div className="counter">
      <div className="board-head">
        <div className="board-title">
          <h1>{t('domains.title')}</h1>
          <span className="board-domain">{domains.length}</span>
        </div>
      </div>
      <p className="lede">{t('domains.dns', { host: target })}</p>
      {error !== undefined && <p role="alert">{errorMessage(t, error)}</p>}
      <div className="table">
        <table className="gate">
          <thead>
            <tr>
              <th>{t('domains.domain')}</th>
              <th>{t('domains.status')}</th>
              <th>{t('domains.dnsRecord')}</th>
              {owner && (
                <th>
                  <span className="vh">{t('links.actions')}</span>
                </th>
              )}
            </tr>
          </thead>
          <tbody>
            {domains.map((d) => (
              <tr key={d.domain}>
                <td className="mono primary">{d.domain}</td>
                <td>
                  <span className={`state ${d.default ? 'default' : d.status}`}>
                    {d.default ? t('domains.default') : t(`domains.statuses.${d.status}`)}
                  </span>
                </td>
                <td className="mono">{d.default ? '' : `CNAME ${target}`}</td>
                {owner && (
                  <td className="actions">
                    {!d.default && (
                      <button className="danger small" onClick={() => remove(d.domain)}>
                        {t('domains.remove')}
                      </button>
                    )}
                  </td>
                )}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      {/* A blank row at the foot of the board, to fill in. */}
      {owner && (
        <form className="row add gate-add" onSubmit={add}>
          <label className="field">
            <span className="micro">{t('domains.domain')}</span>
            <input name="domain" className="mono" placeholder="go.example.com" required />
          </label>
          <button type="submit" className="primary">
            {t('domains.add')}
          </button>
        </form>
      )}
      {dialog}
    </div>
  );
}
