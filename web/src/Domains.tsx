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
      await api('POST', `/workspaces/${workspace.slug}/domains`, { host: new FormData(form).get('host') });
      form.reset();
      setError(undefined);
      await load(workspace.slug);
    } catch (err) {
      setError(err);
    }
  }

  async function remove(host: string) {
    if (!confirm(t('domains.confirmRemove', { host }))) return;
    try {
      await api('DELETE', `/workspaces/${workspace.slug}/domains/${encodeURIComponent(host)}`);
      await load(workspace.slug);
    } catch (err) {
      setError(err);
    }
  }

  if (!domains) return error === undefined ? null : <p role="alert">{errorMessage(t, error)}</p>;
  const owner = workspace.role === 'owner';
  // DNS points at a host name, never a port.
  const target = domains.find((d) => d.default)?.host.replace(/:\d+$/, '');
  return (
    <>
      <h1>{t('domains.title')}</h1>
      <p>{t('domains.dns', { host: target })}</p>
      {owner && (
        <form className="inline" onSubmit={add}>
          <label>
            {t('domains.host')} <input name="host" placeholder="go.example.com" required />
          </label>
          <button type="submit">{t('domains.add')}</button>
        </form>
      )}
      {error !== undefined && <p role="alert">{errorMessage(t, error)}</p>}
      <div className="table">
        <table>
          <thead>
            <tr>
              <th>{t('domains.host')}</th>
              <th>{t('domains.status')}</th>
              <th>{t('domains.dnsRecord')}</th>
              {owner && <th />}
            </tr>
          </thead>
          <tbody>
            {domains.map((d) => (
              <tr key={d.host}>
                <td>{d.host}</td>
                <td>{d.default ? t('domains.default') : t(`domains.statuses.${d.status}`)}</td>
                <td>{d.default ? '' : `CNAME ${target}`}</td>
                {owner && (
                  <td>{!d.default && <button onClick={() => remove(d.host)}>{t('domains.remove')}</button>}</td>
                )}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </>
  );
}
