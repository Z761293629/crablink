import { useEffect, useState, type FormEvent } from 'react';
import { useTranslation } from 'react-i18next';
import { api, type Domain, type Membership } from './api';
import { errorMessage } from './i18n';

/** The Default Domain and the custom Domains of `workspace`; Owners add and remove them. */
export function Domains({ workspace }: { workspace: Membership }) {
  const { t } = useTranslation();
  const [domains, setDomains] = useState<Domain[]>();
  const [error, setError] = useState<unknown>();

  const load = async (slug: string) => setDomains(await api<Domain[]>('GET', `/workspaces/${slug}/domains`));

  useEffect(() => {
    api<Domain[]>('GET', `/workspaces/${workspace.slug}/domains`).then(setDomains, setError);
  }, [workspace.slug]);

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
    if (!confirm(t('domains.confirmRemove', { domain }))) return;
    try {
      await api('DELETE', `/workspaces/${workspace.slug}/domains/${encodeURIComponent(domain)}`);
      await load(workspace.slug);
    } catch (err) {
      setError(err);
    }
  }

  if (!domains) return error === undefined ? null : <p role="alert">{errorMessage(t, error)}</p>;
  const owner = workspace.role === 'owner';
  // DNS points at a host name, never a port.
  const target = domains.find((d) => d.default)?.domain.replace(/:\d+$/, '');
  return (
    <>
      <div className="page-head">
        <h1>{t('domains.title')}</h1>
        <p className="lede">{t('domains.dns', { host: target })}</p>
      </div>
      {error !== undefined && <p role="alert">{errorMessage(t, error)}</p>}
      <section className="sheet">
        {owner && (
          <form className="row" onSubmit={add}>
            <label className="field">
              <span className="micro">{t('domains.domain')}</span>
              <input name="domain" className="mono" placeholder="go.example.com" required />
            </label>
            <button type="submit" className="primary">
              {t('domains.add')}
            </button>
          </form>
        )}
        <div className="table">
          <table>
            <thead>
              <tr>
                <th>{t('domains.domain')}</th>
                <th>{t('domains.status')}</th>
                <th>{t('domains.dnsRecord')}</th>
                {owner && <th />}
              </tr>
            </thead>
            <tbody>
              {domains.map((d) => (
                <tr key={d.domain}>
                  <td className="mono">{d.domain}</td>
                  <td>
                    <span className={`stamp ${d.default ? 'default' : d.status}`}>
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
      </section>
    </>
  );
}
