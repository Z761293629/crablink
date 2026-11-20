import { useEffect, useId, useState, type FormEvent } from 'react';
import { useTranslation } from 'react-i18next';
import { api, type Domain, type Membership } from './api';
import { errorMessage } from './i18n';
import { PageError, useConfirm } from './ui';

/** The Default Domain and the custom Domains of `workspace`; Owners add and remove them. */
export function Domains({ workspace }: { workspace: Membership }) {
  const { t } = useTranslation();
  const [domains, setDomains] = useState<Domain[]>();
  const [error, setError] = useState<unknown>();
  // Adding a Domain fails beside its field, not over the board.
  const [addError, setAddError] = useState<unknown>();
  const errorId = useId();
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
      setAddError(undefined);
      await load(workspace.slug);
    } catch (err) {
      setAddError(err);
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
    // A gate board of Domains, each its status in the mono column and the CNAME to point; for an Owner
    // the add-a-Domain coupon beside it.
    <div className={owner ? 'desk' : 'counter'}>
      <section className="desk-board" aria-labelledby="domains-title">
        <div className="board-head">
          <div className="board-title">
            <h1 id="domains-title">{t('domains.title')}</h1>
            <span className="board-domain">{domains.length}</span>
          </div>
        </div>
        {!owner && <p className="lede">{t('domains.dns', { host: target })}</p>}
        {error !== undefined && <p role="alert">{errorMessage(t, error)}</p>}
        <div className="table">
          <table className="gate" aria-labelledby="domains-title">
            <thead>
              <tr>
                <th>{t('domains.domain')}</th>
                <th>{t('domains.status')}</th>
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
                  <td className="mono primary">
                    {d.domain}
                    {!d.default && (
                      <span className="dns">
                        <span className="micro">CNAME</span> {target}
                      </span>
                    )}
                  </td>
                  <td>
                    <span className={`state ${d.default ? 'default' : d.status}`}>
                      {d.default ? t('domains.default') : t(`domains.statuses.${d.status}`)}
                    </span>
                  </td>
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
      {owner && (
        <form className="pass ticket" onSubmit={add}>
          <div className="ticket-body">
            <h2>{t('domains.addTitle')}</h2>
            <label className="field">
              <span className="micro">{t('domains.domain')}</span>
              <input
                name="domain"
                className="mono"
                placeholder="go.example.com"
                required
                aria-invalid={addError !== undefined || undefined}
                aria-describedby={addError !== undefined ? errorId : undefined}
              />
            </label>
            {addError !== undefined && (
              <p id={errorId} role="alert">
                {errorMessage(t, addError)}
              </p>
            )}
            <p className="hint">{t('domains.dns', { host: target })}</p>
          </div>
          <div className="tear">
            <button type="submit" className="primary">
              {t('domains.add')}
            </button>
          </div>
        </form>
      )}
      {dialog}
    </div>
  );
}
