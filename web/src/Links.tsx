import { useEffect, useState, type FormEvent } from 'react';
import { useTranslation } from 'react-i18next';
import { api, type Domain, type Link, type LinkPage } from './api';
import { errorMessage } from './i18n';

/** A Workspace's Links: create, list a page at a time, edit, delete. */
export function Links({ slug }: { slug: string }) {
  const { t } = useTranslation();
  const [links, setLinks] = useState<Link[]>();
  const [next, setNext] = useState<string | null>(null);
  const [error, setError] = useState<unknown>();
  const base = `/workspaces/${encodeURIComponent(slug)}/links`;

  useEffect(() => {
    api<LinkPage>('GET', base).then((page) => {
      setLinks(page.links);
      setNext(page.next_cursor);
    }, setError);
  }, [base]);

  async function loadMore() {
    try {
      const page = await api<LinkPage>('GET', `${base}?cursor=${encodeURIComponent(next!)}`);
      setLinks([...links!, ...page.links]);
      setNext(page.next_cursor);
    } catch (err) {
      setError(err);
    }
  }

  if (error !== undefined) return <p role="alert">{errorMessage(t, error)}</p>;
  if (!links) return null;
  return (
    <>
      <CreateLink slug={slug} base={base} onCreated={(link) => setLinks([link, ...links])} />
      {links.length === 0 ? (
        <p>{t('links.empty')}</p>
      ) : (
        <div className="table">
          <table>
            <thead>
              <tr>
                <th>{t('links.shortUrl')}</th>
                <th>{t('links.target')}</th>
                <th>{t('links.clickCount')}</th>
                <th>{t('links.expiry')}</th>
                <th>{t('links.actions')}</th>
              </tr>
            </thead>
            <tbody>
              {links.map((link) => (
                <LinkRow
                  key={link.short_url}
                  base={base}
                  link={link}
                  onChanged={(changed) => setLinks(links.map((l) => (l === link ? changed : l)))}
                  onDeleted={() => setLinks(links.filter((l) => l !== link))}
                />
              ))}
            </tbody>
          </table>
        </div>
      )}
      {next && <button onClick={loadMore}>{t('links.more')}</button>}
    </>
  );
}

function CreateLink({ slug, base, onCreated }: { slug: string; base: string; onCreated: (link: Link) => void }) {
  const { t } = useTranslation();
  const [error, setError] = useState<unknown>();
  const [domains, setDomains] = useState<Domain[]>([]);

  useEffect(() => {
    api<Domain[]>('GET', `/workspaces/${encodeURIComponent(slug)}/domains`).then(setDomains, setError);
  }, [slug]);

  async function submit(e: FormEvent<HTMLFormElement>) {
    e.preventDefault();
    const form = e.currentTarget;
    const data = new FormData(form);
    setError(undefined);
    try {
      onCreated(
        await api<Link>('POST', base, {
          target: data.get('target'),
          code: data.get('code') || undefined,
          domain: data.get('domain') || undefined,
          expires_at: fromInput(data.get('expires_at') as string) ?? undefined,
        }),
      );
      form.reset();
    } catch (err) {
      setError(err);
    }
  }

  return (
    <form className="create-link" onSubmit={submit}>
      <label>
        {t('links.target')}
        <input name="target" type="url" required placeholder="https://" />
      </label>
      <label>
        {t('links.customCode')}
        <input name="code" placeholder={t('links.generated')} />
      </label>
      <label>
        {t('links.expiry')}
        <input name="expires_at" type="datetime-local" />
      </label>
      <label>
        {t('links.domain')}
        <select name="domain">
          {domains
            .filter((d) => d.default || d.status === 'approved')
            .map((d) => (
              <option key={d.host}>{d.host}</option>
            ))}
        </select>
      </label>
      <button type="submit">{t('links.create')}</button>
      {error !== undefined && <p role="alert">{errorMessage(t, error)}</p>}
    </form>
  );
}

function LinkRow({
  base,
  link,
  onChanged,
  onDeleted,
}: {
  base: string;
  link: Link;
  onChanged: (link: Link) => void;
  onDeleted: () => void;
}) {
  const { t, i18n } = useTranslation();
  const [editing, setEditing] = useState(false);
  const [copied, setCopied] = useState(false);
  const [error, setError] = useState<unknown>();
  const path = `${base}/${encodeURIComponent(link.code)}?domain=${encodeURIComponent(link.domain)}`;

  async function save(e: FormEvent<HTMLFormElement>) {
    e.preventDefault();
    const data = new FormData(e.currentTarget);
    try {
      onChanged(
        await api<Link>('PATCH', path, {
          target: data.get('target'),
          expires_at: fromInput(data.get('expires_at') as string),
        }),
      );
      setEditing(false);
      setError(undefined);
    } catch (err) {
      setError(err);
    }
  }

  async function remove() {
    if (!confirm(t('links.confirmDelete', { url: link.short_url }))) return;
    try {
      await api('DELETE', path);
      onDeleted();
    } catch (err) {
      setError(err);
    }
  }

  async function copy() {
    try {
      await navigator.clipboard.writeText(link.short_url);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch (err) {
      setError(err);
    }
  }

  // A QR code of an expired Link would only mislead, so the server refuses one.
  const expired = link.expires_at !== null && new Date(link.expires_at) <= new Date();
  const form = `edit-${link.short_url}`;
  const date = new Intl.DateTimeFormat(i18n.language, { dateStyle: 'medium', timeStyle: 'short' });
  return (
    <tr>
      <td>
        <a href={link.short_url}>{link.short_url}</a>
        {error !== undefined && <p role="alert">{errorMessage(t, error)}</p>}
      </td>
      {editing ? (
        <>
          <td className="target">
            <form id={form} onSubmit={save} />
            <input form={form} name="target" type="url" required defaultValue={link.target} aria-label={t('links.target')} />
          </td>
          <td>{link.click_count}</td>
          <td>
            <input
              form={form}
              name="expires_at"
              type="datetime-local"
              defaultValue={toInput(link.expires_at)}
              aria-label={t('links.expiry')}
            />
          </td>
          {/* Its own key: reusing the Edit button's node would submit on that click. */}
          <td className="actions" key="editing">
            <button form={form} type="submit">
              {t('links.save')}
            </button>
            <button onClick={() => setEditing(false)}>{t('links.cancel')}</button>
          </td>
        </>
      ) : (
        <>
          <td className="target">{link.target}</td>
          <td>{link.click_count}</td>
          <td>{link.expires_at ? date.format(new Date(link.expires_at)) : t('links.never')}</td>
          <td className="actions" key="viewing">
            <button onClick={copy}>{t(copied ? 'links.copied' : 'links.copy')}</button>
            {!expired &&
              ['svg', 'png'].map((format) => (
                <a
                  key={format}
                  href={`/api${base}/${encodeURIComponent(link.code)}/qr?format=${format}&domain=${encodeURIComponent(link.domain)}`}
                  download={`${link.code}.${format}`}
                >
                  {t('links.qr', { format: format.toUpperCase() })}
                </a>
              ))}
            <button onClick={() => setEditing(true)}>{t('links.edit')}</button>
            <button onClick={remove}>{t('links.delete')}</button>
          </td>
        </>
      )}
    </tr>
  );
}

/** A `datetime-local` input's value, in this browser's time zone, as an ISO instant; null when empty. */
function fromInput(value: string): string | null {
  return value ? new Date(value).toISOString() : null;
}

/** An ISO instant as a `datetime-local` value in this browser's time zone. */
function toInput(iso: string | null): string {
  if (!iso) return '';
  const d = new Date(iso);
  return new Date(d.getTime() - d.getTimezoneOffset() * 60_000).toISOString().slice(0, 16);
}
