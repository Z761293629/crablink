import { useCallback, useEffect, useState, type FormEvent } from 'react';
import { useTranslation } from 'react-i18next';
import { api, type Domain, type ImportedRow, type Link, type LinkPage } from './api';
import { clicksHref } from './Clicks';
import { errorMessage } from './i18n';

/** A Workspace's Links: create, filter by Tag, search, list a page at a time, edit, delete. */
export function Links({ slug }: { slug: string }) {
  const { t } = useTranslation();
  const [links, setLinks] = useState<Link[]>();
  const [next, setNext] = useState<string | null>(null);
  const [error, setError] = useState<unknown>();
  const [tags, setTags] = useState<string[]>([]);
  const [tag, setTag] = useState('');
  const [q, setQ] = useState('');
  /** Bumped to fetch the list again, e.g. after an import. */
  const [reloads, setReloads] = useState(0);
  const base = `/workspaces/${encodeURIComponent(slug)}/links`;
  const filter = new URLSearchParams(Object.entries({ tag, q }).filter(([, v]) => v));

  const loadTags = useCallback(
    () => api<string[]>('GET', `/workspaces/${encodeURIComponent(slug)}/tags`).then(setTags, setError),
    [slug],
  );
  useEffect(() => void loadTags(), [loadTags]);

  const query = filter.toString();
  useEffect(() => {
    // Typing in the search box fires a request per keystroke: only the latest may land.
    let latest = true;
    api<LinkPage>('GET', `${base}?${query}`).then((page) => {
      if (!latest) return;
      setLinks(page.links);
      setNext(page.next_cursor);
    }, setError);
    return () => {
      latest = false;
    };
  }, [base, query, reloads]);

  async function loadMore() {
    try {
      filter.set('cursor', next!);
      const page = await api<LinkPage>('GET', `${base}?${filter}`);
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
      <CreateLink
        slug={slug}
        base={base}
        onCreated={(link) => {
          setLinks([link, ...links]);
          void loadTags();
        }}
      />
      <CsvTools
        base={base}
        onImported={() => {
          setReloads((n) => n + 1);
          void loadTags();
        }}
      />
      <div className="link-filter">
        <label>
          {t('links.search')}
          <input type="search" value={q} onChange={(e) => setQ(e.target.value)} />
        </label>
        <label>
          {t('links.tag')}
          {/* Named explicitly: a wrapping label would also take in the selected option's text. */}
          <select value={tag} onChange={(e) => setTag(e.target.value)} aria-label={t('links.tag')}>
            <option value="">{t('links.allTags')}</option>
            {tags.map((name) => (
              <option key={name}>{name}</option>
            ))}
          </select>
        </label>
      </div>
      {links.length === 0 ? (
        <p>{t(query ? 'links.noMatch' : 'links.empty')}</p>
      ) : (
        <div className="table">
          <table>
            <thead>
              <tr>
                <th>{t('links.shortUrl')}</th>
                <th>{t('links.target')}</th>
                <th>{t('links.tags')}</th>
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
                  onChanged={(changed) => {
                    setLinks(links.map((l) => (l === link ? changed : l)));
                    void loadTags();
                  }}
                  onDeleted={() => {
                    setLinks(links.filter((l) => l !== link));
                    void loadTags();
                  }}
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
          tags: tagList(data.get('tags') as string),
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
        {t('links.tags')}
        <input name="tags" placeholder={t('links.tagsHint')} />
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

/** Export the Workspace's Links as CSV, or import a CSV file and show each row's result. */
function CsvTools({ base, onImported }: { base: string; onImported: () => void }) {
  const { t } = useTranslation();
  const [rows, setRows] = useState<ImportedRow[]>();
  const [error, setError] = useState<unknown>();

  async function upload(input: HTMLInputElement) {
    const file = input.files?.[0];
    if (!file) return;
    setError(undefined);
    try {
      setRows(await api<ImportedRow[]>('POST', `${base}.csv`, file));
      onImported();
    } catch (err) {
      setError(err);
    }
    input.value = '';
  }

  return (
    <div className="csv-tools">
      <a href={`/api${base}.csv`} download="links.csv">
        {t('links.exportCsv')}
      </a>
      <label>
        {t('links.importCsv')}
        <input type="file" accept=".csv,text/csv" onChange={(e) => void upload(e.currentTarget)} />
      </label>
      {error !== undefined && <p role="alert">{errorMessage(t, error)}</p>}
      {rows && (
        <div className="table">
          <table aria-label={t('links.importCsv')}>
            <thead>
              <tr>
                <th>{t('links.line')}</th>
                <th>{t('links.result')}</th>
              </tr>
            </thead>
            <tbody>
              {rows.map((row) => (
                <tr key={row.line}>
                  <td>{row.line}</td>
                  <td>
                    {row.short_url ? (
                      <a href={row.short_url}>{row.short_url}</a>
                    ) : (
                      t(`errors.${row.skipped!.kind}`, { defaultValue: row.skipped!.error })
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
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
          tags: tagList(data.get('tags') as string),
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
          <td>
            <input
              form={form}
              name="tags"
              defaultValue={link.tags.join(', ')}
              placeholder={t('links.tagsHint')}
              aria-label={t('links.tags')}
            />
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
          <td>{link.tags.join(', ')}</td>
          <td>{link.click_count}</td>
          <td>{link.expires_at ? date.format(new Date(link.expires_at)) : t('links.never')}</td>
          <td className="actions" key="viewing">
            <button onClick={copy}>{t(copied ? 'links.copied' : 'links.copy')}</button>
            <a href={clicksHref(link)}>{t('clicks.clicks')}</a>
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

/** Comma-separated Tag names as typed (a Tag never has a comma), blanks dropped. */
function tagList(value: string): string[] {
  return value
    .split(',')
    .map((name) => name.trim())
    .filter(Boolean);
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
