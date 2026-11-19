import {
  useEffect,
  useId,
  useLayoutEffect,
  useRef,
  useState,
  type CSSProperties,
  type FormEvent,
  type ReactNode,
} from 'react';
import { useTranslation } from 'react-i18next';
import { api, linkPath, type Domain, type ImportedRow, type Link, type LinkPage } from './api';
import { clicksHref, LinkBoard } from './Clicks';
import { errorMessage } from './i18n';
import {
  Chevron,
  Flap,
  MoreMenu,
  navigate,
  PageError,
  Plane,
  reducedMotion,
  useConfirm,
  useLocationKey,
  useMedia,
  useToast,
  WIDE,
} from './ui';

type Fresh = 'created' | 'updated';
/** The Tags a board row shows before a "+N" chip. */
const ROW_TAGS = 2;
/** From this width up a board row has room for Copy and QR beside its More menu; below it, down to WIDE, they join the menu. */
const ROOMY = '(min-width: 90rem)';
const EASE_OUT = 'cubic-bezier(0.16, 1, 0.3, 1)';

/**
 * A Workspace's Links as a board, and the selected one as a pass: create, filter by
 * Tag, search, list a page at a time, edit, delete, copy, download its QR code.
 */
export function Links({ slug, pick }: { slug: string; /** The Link in the URL, if any. */ pick?: { domain: string; code: string } }) {
  const { t, i18n } = useTranslation();
  const wide = useMedia(WIDE);
  const roomy = useMedia(ROOMY);
  // Beside the open pass and short of room: a row's Copy and QR move into its More menu, leaving the Target its column.
  const compact = wide && !roomy;
  const [toast, showToast] = useToast();
  const [dialog, ask] = useConfirm();
  const search = useRef<HTMLInputElement>(null);
  // The search and the Tag filter live in the query, so a filtered board can be shared and reloaded.
  useLocationKey();
  const params = new URLSearchParams(location.search);
  const q = params.get('q') ?? '';
  const tag = params.get('tag') ?? '';
  const [links, setLinks] = useState<Link[]>();
  const [next, setNext] = useState<string | null>(null);
  const [error, setError] = useState<unknown>();
  const [tags, setTags] = useState<string[]>([]);
  const [domains, setDomains] = useState<Domain[]>([]);
  /** Bumped to fetch the list again, e.g. after an import or to retry. */
  const [reloads, setReloads] = useState(0);
  /** The request whose page is on the board; another one in flight means loading. */
  const [landed, setLanded] = useState<string>();
  /** The Short URL whose pass is open for editing. */
  const [editing, setEditing] = useState<string>();
  /** Links just issued or changed, marked until picked. */
  const [fresh, setFresh] = useState<Record<string, Fresh>>({});
  /** The blank pass for a New Link is open. */
  const [issuing, setIssuing] = useState(false);
  /** The Short URL that couldn't be copied: its pass shows it selected, to copy by hand. */
  const [manual, setManual] = useState<string>();
  const [imported, setImported] = useState<{ rows?: ImportedRow[]; error?: unknown }>();
  const base = `/workspaces/${encodeURIComponent(slug)}/links`;
  const filter = new URLSearchParams(Object.entries({ tag, q }).filter(([, v]) => v));
  const query = filter.toString();
  const loadKey = `${query}#${reloads}`;

  /** Bumped to fetch the Tags again, after a change that may add or drop one. */
  const [tagLoads, setTagLoads] = useState(0);
  const loadTags = () => setTagLoads((n) => n + 1);
  useEffect(() => {
    api<string[]>('GET', `/workspaces/${encodeURIComponent(slug)}/tags`).then(setTags, setError);
  }, [slug, reloads, tagLoads]);

  useEffect(() => {
    api<Domain[]>('GET', `/workspaces/${encodeURIComponent(slug)}/domains`).then(setDomains, setError);
  }, [slug, reloads]);

  useEffect(() => {
    // Typing in the search box fires a request per keystroke: only the latest may land.
    let latest = true;
    api<LinkPage>('GET', `${base}?${query}`).then((page) => {
      if (!latest) return;
      setLinks(page.links);
      setNext(page.next_cursor);
      setLanded(`${query}#${reloads}`);
    }, setError);
    return () => {
      latest = false;
    };
  }, [base, query, reloads]);

  // Rows move rather than pop: each row's place before a change, to slide it from there.
  const rows = useRef(new Map<string, HTMLTableRowElement>());
  const pending = useRef<{ from?: Map<string, number>; enter?: string }>({});
  const beforeChange = (enter?: string) => {
    if (reducedMotion()) return;
    const from = new Map<string, number>();
    rows.current.forEach((el, key) => from.set(key, el.getBoundingClientRect().top));
    pending.current = { from, enter };
  };
  useLayoutEffect(() => {
    const { from, enter } = pending.current;
    pending.current = {};
    if (!from) return;
    rows.current.forEach((el, key) => {
      if (key === enter) {
        el.animate([{ transform: 'translateX(-1.5rem)', opacity: 0 }, { transform: 'none', opacity: 1 }], {
          duration: 380,
          easing: EASE_OUT,
        });
        return;
      }
      const top = from.get(key);
      const dy = top === undefined ? 0 : top - el.getBoundingClientRect().top;
      if (dy) el.animate([{ transform: `translateY(${dy}px)` }, { transform: 'none' }], { duration: 360, easing: EASE_OUT });
    });
  }, [links]);

  // Shortcuts, never while typing in a field: N a New Link, / the search box, Esc back to the pass.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const el = e.target as HTMLElement;
      if (e.metaKey || e.ctrlKey || e.altKey || el.isContentEditable) return;
      if (['INPUT', 'SELECT', 'TEXTAREA'].includes(el.tagName) || el.closest('[role=menu]')) return;
      if (e.key === 'n') setIssuing(true);
      else if (e.key === '/') search.current?.focus();
      else if (e.key === 'Escape') {
        setIssuing(false);
        setEditing(undefined);
        return;
      } else return;
      e.preventDefault();
    };
    addEventListener('keydown', onKey);
    return () => removeEventListener('keydown', onKey);
  }, []);

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

  // Nothing picked, or the pick filtered away: the newest live Link on the board, so the pass is never blank.
  const selected =
    links?.find((l) => l.domain === pick?.domain && l.code === pick?.code) ??
    (wide ? (links?.find((l) => !isExpired(l)) ?? links?.[0]) : undefined);

  const mark = (key: string, how?: Fresh) =>
    setFresh((f) => {
      const rest = Object.fromEntries(Object.entries(f).filter(([k]) => k !== key));
      return how ? { ...rest, [key]: how } : rest;
    });

  /** Puts `link`'s pass in the URL, or none, keeping the filters. */
  const go = (link?: Link, replace = false) =>
    navigate((link ? linkHref(link) : '/app/') + location.search, { replace, keepScroll: true });

  function setFilter(key: 'q' | 'tag', value: string) {
    const next = new URLSearchParams(location.search);
    if (value) next.set(key, value);
    else next.delete(key);
    const query = next.toString();
    navigate(location.pathname + (query ? `?${query}` : ''), { replace: true, keepScroll: true });
  }

  function choose(link: Link) {
    setIssuing(false);
    setEditing(undefined);
    setManual(undefined);
    mark(link.short_url);
    // On a phone the pass opens under its row, and a second tap folds it away.
    go(!wide && selected === link ? undefined : link);
  }

  /** Opens `link`'s pass for editing. */
  function edit(link: Link) {
    setIssuing(false);
    setManual(undefined);
    mark(link.short_url);
    go(link);
    setEditing(link.short_url);
  }

  function created(link: Link) {
    beforeChange(link.short_url);
    setLinks([link, ...(links ?? [])]);
    go(link);
    mark(link.short_url, 'created');
    setIssuing(false);
    showToast(t('links.announce.created', { url: link.short_url }));
    loadTags();
  }

  function changed(old: Link, link: Link) {
    beforeChange(link.short_url);
    setLinks(links!.map((l) => (l === old ? link : l)));
    mark(link.short_url, 'updated');
    setEditing(undefined);
    showToast(
      old.target === link.target
        ? t('links.announce.updated', { url: link.short_url })
        : t('links.announce.retargeted', { url: link.short_url, target: link.target }),
    );
    loadTags();
  }

  const copied = (link: Link) => showToast(t('links.announce.copied', { url: link.short_url }));

  /** The clipboard refused: open the Link's pass with its Short URL selected, to copy by hand. */
  function copyFailed(link: Link) {
    setIssuing(false);
    setEditing(undefined);
    go(link);
    setManual(link.short_url);
    showToast(t('links.copyFailed'));
  }

  async function remove(link: Link) {
    const message = t('links.confirmDelete', { url: link.short_url, code: link.code, domain: link.domain });
    if (!(await ask(message, t('links.confirmDeleteAction')))) return;
    try {
      await api('DELETE', linkPath(slug, link));
    } catch (err) {
      showToast(errorMessage(t, err));
      return;
    }
    const el = rows.current.get(link.short_url);
    // The row boards: it drops and fades off the board before the rest close up.
    if (el && !reducedMotion()) {
      await el.animate(
        [
          { transform: 'none', opacity: 1 },
          { transform: 'translateY(0.75rem) scaleY(0.85)', opacity: 0 },
        ],
        { duration: 240, easing: 'cubic-bezier(0.7, 0, 0.84, 0)', fill: 'forwards' },
      ).finished;
    }
    const i = links!.indexOf(link);
    const after = links![i + 1] ?? links![i - 1];
    beforeChange();
    setLinks((ls) => ls!.filter((l) => l.short_url !== link.short_url));
    // Beside the board the next Link's pass takes its place; on a phone the board just closes up, so a
    // second Delete never lands on a Link that slid under the thumb.
    if (selected === link) go(wide ? after : undefined, true);
    else if (wide) go(selected, true);
    mark(link.short_url);
    showToast(t('links.announce.deleted', { url: link.short_url }));
    loadTags();
  }

  const clearFilters = () => navigate(location.pathname, { replace: true, keepScroll: true });

  if (error !== undefined) {
    return (
      <div className="links">
        <div className="board-col">
          <PageError
            error={error}
            onRetry={() => {
              setError(undefined);
              setReloads((n) => n + 1);
            }}
          />
        </div>
      </div>
    );
  }

  const loading = landed !== loadKey;
  const noLinks = links?.length === 0 && !query;
  const editor = links !== undefined && (issuing || noLinks) && (
    <IssuePass
      base={base}
      domains={domains}
      focus={issuing || wide}
      canCancel={!noLinks}
      onCancel={() => setIssuing(false)}
      onCreated={created}
    />
  );
  const pass = (link: Link) => (
    <LinkPass
      key={link.short_url}
      slug={slug}
      link={link}
      editing={editing === link.short_url}
      manualCopy={manual === link.short_url}
      reveal={!wide}
      confirm={ask}
      onEditing={(on) => setEditing(on ? link.short_url : undefined)}
      onChanged={changed}
      onCopied={copied}
      onCopyFailed={copyFailed}
    />
  );
  // With one usable Domain, rows show just /Code and the Domain sits once in the board's head.
  const usable = usableDomains(domains);
  const onlyDomain = usable.length === 1 ? usable[0].domain : undefined;
  const count = new Intl.NumberFormat(i18n.language);
  // A row shows the Expiry's day only; the pass carries the time.
  const date = new Intl.DateTimeFormat(i18n.language, { dateStyle: 'medium' });

  return (
    <div className="links">
      <section className="board-col" aria-labelledby="links-title">
        <div className="board-head">
          <div className="board-title">
            <h1 id="links-title">{t('links.title')}</h1>
            {onlyDomain && <span className="board-domain">{onlyDomain}</span>}
          </div>
          {/* While the blank pass is open, its Create is the one Crab Red action: this shows pressed. */}
          <button
            className={editor ? undefined : 'accent'}
            aria-pressed={!!editor}
            aria-keyshortcuts="N"
            title={t('links.shortcutNew')}
            onClick={() => {
              setEditing(undefined);
              setIssuing(!(editor && !noLinks));
            }}
          >
            {t('links.newLink')}
          </button>
          <CsvTools
            base={base}
            onImported={(result) => {
              setImported(result);
              if (result.rows) {
                setReloads((n) => n + 1);
                loadTags();
              }
            }}
          />
        </div>
        {!wide && editor}
        <div className="filters">
          <label>
            <span className="vh">{t('links.search')}</span>
            <input
              ref={search}
              type="search"
              name="q"
              value={q}
              aria-keyshortcuts="/"
              placeholder={t('links.searchHint')}
              onChange={(e) => setFilter('q', e.target.value)}
            />
          </label>
          {/* Named explicitly: a wrapping label would also take in the selected option's text. */}
          <select name="tag" value={tag} onChange={(e) => setFilter('tag', e.target.value)} aria-label={t('links.tag')}>
            <option value="">{t('links.allTags')}</option>
            {/* A Tag in the URL that no Link carries any more still shows as chosen, rather than "All Tags" over an empty board. */}
            {(tag && !tags.includes(tag) ? [tag, ...tags] : tags).map((name) => (
              <option key={name}>{name}</option>
            ))}
          </select>
        </div>
        {imported && <ImportResult {...imported} onClose={() => setImported(undefined)} />}
        <div className={`flightpath${loading ? ' on' : ''}`} aria-hidden="true" />
        {loading && <p className="vh">{t('links.loading')}</p>}
        {links &&
          (links.length === 0 ? (
            <div className="board-empty">
              <p>{t(query ? 'links.noMatch' : 'links.empty')}</p>
              {query && (
                <button className="small" onClick={clearFilters}>
                  {t('links.clearFilters')}
                </button>
              )}
            </div>
          ) : (
            <table className={compact ? 'board compact' : 'board'} aria-labelledby="links-title">
              <colgroup>
                <col className="c-short" />
                <col />
                <col className="c-count" />
                <col className="c-acts" />
              </colgroup>
              <thead>
                <tr>
                  <th>{t('links.shortUrl')}</th>
                  <th>{t('links.target')}</th>
                  <th className="num">{t('links.clickCount')}</th>
                  <th>
                    <span className="vh">{t('links.actions')}</span>
                  </th>
                </tr>
              </thead>
              <tbody>
                {links.map((link) => {
                  const expired = isExpired(link);
                  // While a New Link is being issued, no pass on the board is open.
                  const isSelected = link === selected && !editor;
                  const how = fresh[link.short_url];
                  const [scheme, host] = splitShortUrl(link);
                  return [
                    <tr
                      key={link.short_url}
                      ref={(el) => {
                        if (el) rows.current.set(link.short_url, el);
                        else rows.current.delete(link.short_url);
                      }}
                      className={[isSelected && 'is-selected', how && 'is-fresh'].filter(Boolean).join(' ')}
                    >
                      <td className="c-short">
                        <button className="pick" aria-pressed={isSelected} onClick={() => choose(link)}>
                          <span className="vh">{scheme}</span>
                          <span className={link.domain === onlyDomain ? 'vh' : 'host'}>{host.slice(0, -1)}</span>
                          <span className="host">/</span>
                          <span className="code">{link.code}</span>
                        </button>
                      </td>
                      <td className="c-target">
                        <span className="target">{link.target}</span>
                        <span className="meta">
                          {how && <span className="stamp fresh">{t(`links.fresh.${how}`)}</span>}
                          {/* A row keeps one line of meta: two Tags at most, shrinking to fit, then +N; the pass shows them all. */}
                          {link.tags.slice(0, ROW_TAGS).map((name) => (
                            <span key={name} className="tag" title={name}>
                              {name}
                            </span>
                          ))}
                          {link.tags.length > ROW_TAGS && (
                            <span className="tag rest" title={link.tags.slice(ROW_TAGS).join(', ')}>
                              +{link.tags.length - ROW_TAGS}
                              <span className="vh">: {link.tags.slice(ROW_TAGS).join(', ')}</span>
                            </span>
                          )}
                          {expired ? (
                            <span className="stamp expired">{t('links.expired')}</span>
                          ) : (
                            link.expires_at && (
                              <span className="when">
                                {t('links.expiry')} <Flap text={date.format(new Date(link.expires_at))} />
                              </span>
                            )
                          )}
                        </span>
                      </td>
                      <td className="num">
                        <Flap text={count.format(link.click_count)} />
                      </td>
                      <td className="c-acts">
                        <span className="row-acts">
                          {!compact && <CopyButton link={link} onCopied={copied} onFailed={copyFailed} />}
                          {!compact && !expired && (
                            <a
                              className="button small"
                              href={qrHref(slug, link, 'png')}
                              download={`${link.code}.png`}
                              aria-label={t('links.downloadQrOf', { code: link.code })}
                            >
                              {t('links.qrShort')}
                            </a>
                          )}
                          <MoreMenu
                            label={t('links.moreActions', { code: link.code })}
                            items={[
                              ...(compact
                                ? [
                                    { label: t('links.copy'), onSelect: () => void copy(link, copied, copyFailed) },
                                    ...(expired
                                      ? []
                                      : [{ label: t('links.downloadQr'), onSelect: () => download(qrHref(slug, link, 'png'), `${link.code}.png`) }]),
                                  ]
                                : []),
                              { label: t('links.edit'), onSelect: () => edit(link) },
                              { label: t('links.delete'), danger: true, onSelect: () => void remove(link) },
                            ]}
                          />
                        </span>
                      </td>
                    </tr>,
                    !wide && isSelected && (
                      <tr key={`${link.short_url} pass`} className="detail">
                        <td colSpan={4}>{pass(link)}</td>
                      </tr>
                    ),
                  ];
                })}
              </tbody>
            </table>
          ))}
        {next && (
          <button className="more" onClick={loadMore}>
            {t('links.more')}
          </button>
        )}
      </section>
      {wide && links && (
        <div className="pane">
          <div className="pane-inner">
            {editor ||
              (selected ? (
                <>
                  {pass(selected)}
                  <LinkBoard key={`board ${selected.short_url}`} slug={slug} link={selected} />
                </>
              ) : (
                <section className="pass" aria-label={t('links.selected')}>
                  <div className="pane-empty">
                    <p>{t('links.noMatch')}</p>
                  </div>
                </section>
              ))}
          </div>
        </div>
      )}
      {toast}
      {dialog}
    </div>
  );
}

/** Copies `link`'s Short URL, then says how that went. */
async function copy(link: Link, onCopied: (link: Link) => void, onFailed: (link: Link) => void) {
  try {
    await navigator.clipboard.writeText(link.short_url);
  } catch {
    // No clipboard here (e.g. plain http), or it refused.
    onFailed(link);
    return;
  }
  onCopied(link);
}

/** Saves the file at `href` as `name`, as a click on a download link would. */
function download(href: string, name: string) {
  const a = document.createElement('a');
  a.href = href;
  a.download = name;
  a.click();
}

/** Copies a Link's Short URL; for a moment it says so, with the plane. */
function CopyButton({
  link,
  onCopied,
  onFailed,
  className = 'small',
}: {
  link: Link;
  onCopied: (link: Link) => void;
  /** No clipboard here (e.g. plain http), or it refused. */
  onFailed: (link: Link) => void;
  className?: string;
}) {
  const { t } = useTranslation();
  const [copied, setCopied] = useState(false);
  useEffect(() => {
    if (!copied) return;
    const timer = setTimeout(() => setCopied(false), 1200);
    return () => clearTimeout(timer);
  }, [copied]);
  return (
    // Both labels share one cell, the idle one hidden, so the button is always as wide as the wider.
    <button
      className={`${className} copy`}
      onClick={() =>
        copy(
          link,
          (l) => {
            setCopied(true);
            onCopied(l);
          },
          onFailed,
        )
      }
    >
      <span className={copied ? 'off' : undefined}>{t('links.copy')}</span>
      <span className={copied ? undefined : 'off'}>
        {/* Mounted afresh on each copy, so the plane flies in every time. */}
        <Plane key={String(copied)} className="plane-sm" />
        {t('links.copied')}
      </span>
    </button>
  );
}

/** The blank pass a New Link is created from: the Target first, the rest under More options; the stub previews the Short URL. */
function IssuePass({
  base,
  domains,
  focus,
  canCancel,
  onCancel,
  onCreated,
}: {
  base: string;
  domains: Domain[];
  focus: boolean;
  canCancel: boolean;
  onCancel: () => void;
  onCreated: (link: Link) => void;
}) {
  const { t } = useTranslation();
  const id = useId();
  const [error, setError] = useState<unknown>();
  const [draft, setDraft] = useState<Record<string, string>>({});
  const [tearing, setTearing] = useState(false);
  const usable = usableDomains(domains);
  const domain = draft.domain || usable[0]?.domain || '';
  const to = hostOf(draft.target ?? '');

  async function submit(e: FormEvent<HTMLFormElement>) {
    e.preventDefault();
    const form = e.currentTarget;
    const data = new FormData(form);
    setError(undefined);
    try {
      const link = await api<Link>('POST', base, {
        target: data.get('target'),
        code: data.get('code') || undefined,
        domain: data.get('domain') || undefined,
        expires_at: fromInput(data.get('expires_at') as string) ?? undefined,
        tags: tagList(data.get('tags') as string),
      });
      // The stub tears off along the perforation, then the Link lands on the board.
      if (!reducedMotion()) {
        setTearing(true);
        await new Promise((done) => setTimeout(done, 280));
        setTearing(false);
      }
      form.reset();
      setDraft({});
      onCreated(link);
    } catch (err) {
      setError(err);
    }
  }

  return (
    <section className={`pass${tearing ? ' tearing' : ''}`} aria-label={t('links.newLink')}>
      <form
        className="pass-card print"
        onSubmit={submit}
        onInput={(e) => setDraft(Object.fromEntries(new FormData(e.currentTarget)) as Record<string, string>)}
        onKeyDown={(e) => {
          if (e.key === 'Escape' && canCancel) onCancel();
        }}
      >
        <div className="pass-main">
          <Route
            from={domain}
            code={draft.code || t('links.generatedShort')}
            codeIsPlaceholder={!draft.code}
            to={to}
          />
          <label className="field target-field">
            <span className="micro">{t('links.target')}</span>
            <input name="target" type="url" required placeholder={t('links.targetHint')} autoFocus={focus} />
          </label>
          {/* With one usable Domain there is nothing to choose. */}
          {usable.length > 1 && (
            <label className="field domain-field">
              <span className="micro">{t('links.domain')}</span>
              <select name="domain">
                {usable.map((d) => (
                  <option key={d.domain}>{d.domain}</option>
                ))}
              </select>
            </label>
          )}
          <details className="options">
            <summary>
              {t('links.moreOptions')}
              <Chevron />
            </summary>
            <div className="segs">
              <label className="seg l field">
                <span className="micro">{t('links.customCode')}</span>
                <input name="code" className="mono" placeholder={t('links.codeHint')} />
              </label>
              <label className="seg r field">
                <span className="micro">{t('links.tags')}</span>
                <input name="tags" placeholder={t('links.tagsHint')} />
              </label>
              <div className="seg wide field">
                <label className="micro" htmlFor={`${id}expiry`}>
                  {t('links.expiry')}
                </label>
                <input id={`${id}expiry`} name="expires_at" type="datetime-local" aria-describedby={`${id}hint`} />
                <p id={`${id}hint`} className="hint">
                  {t('links.expiryHint')}
                </p>
              </div>
            </div>
          </details>
          {error !== undefined && <p role="alert">{errorMessage(t, error)}</p>}
        </div>
        <div className="perf" aria-hidden="true" />
        <div className="stub">
          <div className="qr blank">{t('links.qrPending')}</div>
          <ShortLine text={`${domain}/${draft.code || t('links.generatedShort')}`} />
          <div className="acts">
            <button type="submit" className="accent">
              {t('links.issue')}
            </button>
            {canCancel && (
              <button type="button" className="small" onClick={onCancel}>
                {t('links.cancel')}
              </button>
            )}
          </div>
        </div>
      </form>
    </section>
  );
}

/** From the Short URL's Domain and Code to the Target's host, with the plane flying the route once. */
function Route({ from, code, codeIsPlaceholder, to }: { from: string; code: string; codeIsPlaceholder?: boolean; to: string }) {
  const { t } = useTranslation();
  return (
    <div className="route">
      <p className="from">
        <span className="micro">{from} /</span>
        {/* A long Code shrinks to fit the column, then breaks after its hyphens and underscores. */}
        {codeIsPlaceholder ? (
          // No Code yet: a blank of the Generated Code's seven slots, to be filled.
          <span className="code placeholder">
            <span className="vh">{code}</span>
            {Array.from({ length: 7 }, (_, i) => (
              <span key={i} className="slot" aria-hidden="true" />
            ))}
          </span>
        ) : (
          <span className="code" style={{ '--ems': monoEms(code) } as CSSProperties} title={code}>
            {breakable(code)}
          </span>
        )}
      </p>
      <div className="flight" aria-hidden="true">
        <Plane className="plane" />
      </div>
      {/* No Target yet: the route ends at the plane rather than at a label with nothing under it. */}
      {to && (
        <p className="to">
          <span className="micro">{t('links.target')}</span>
          <span className="host">{to}</span>
        </p>
      )}
    </div>
  );
}

/** The width of `text` in ems of the mono face: a CJK character is a full em, the rest 0.6. */
const monoEms = (text: string) => [...text].reduce((n, ch) => n + (ch.codePointAt(0)! >= 0x2e80 ? 1 : 0.6), 0);

/** `text` with a line-break chance after each slash, hyphen and underscore, and nowhere else. */
const breakable = (text: string) => text.split(/(?<=[/_-])/).flatMap((part, i) => (i ? [<wbr key={i} />, part] : part));

/** The stub's Short URL in mono, set smaller to fit the stub, and only then broken: after the Domain's slash first, then at hyphens. */
function ShortLine({ text, href }: { text: string; href?: string }) {
  const slash = text.indexOf('/') + 1;
  const body = (
    <>
      {breakable(text.slice(0, slash))}
      <span className="code">{breakable(text.slice(slash))}</span>
    </>
  );
  return (
    <p className="short" style={{ '--ems': monoEms(text) } as CSSProperties} title={text}>
      {href ? <a href={href}>{body}</a> : body}
    </p>
  );
}

/** The selected Link as a pass: its segments, Change Target, the stub with its QR code to download. */
function LinkPass({
  slug,
  link,
  editing,
  manualCopy,
  reveal,
  confirm,
  onEditing,
  onChanged,
  onCopied,
  onCopyFailed,
}: {
  slug: string;
  link: Link;
  editing: boolean;
  /** Copying failed: show the Short URL selected, to copy by hand. */
  manualCopy: boolean;
  /** Scroll the stub (QR code and actions) into view when the pass opens, as under a row on a phone. */
  reveal: boolean;
  /** Asks "Are you sure?"; resolves to whether `action` was chosen. */
  confirm: (message: ReactNode, action: string) => Promise<boolean>;
  onEditing: (on: boolean) => void;
  /** With the Link as it was and as it is now. */
  onChanged: (old: Link, link: Link) => void;
  onCopied: (link: Link) => void;
  onCopyFailed: (link: Link) => void;
}) {
  const { t, i18n } = useTranslation();
  const id = useId();
  const [error, setError] = useState<unknown>();
  const stub = useRef<HTMLDivElement>(null);
  const manual = useRef<HTMLInputElement>(null);
  const change = useRef<HTMLButtonElement>(null);
  const wasEditing = useRef(editing);

  useEffect(() => {
    if (reveal) stub.current?.scrollIntoView({ block: 'nearest', behavior: reducedMotion() ? 'auto' : 'smooth' });
  }, [reveal]);
  useEffect(() => {
    if (manualCopy) manual.current?.select();
  }, [manualCopy]);
  // Saved or cancelled: focus back where editing began.
  useEffect(() => {
    if (wasEditing.current && !editing) change.current?.focus();
    wasEditing.current = editing;
  }, [editing]);

  async function save(e: FormEvent<HTMLFormElement>) {
    e.preventDefault();
    const data = new FormData(e.currentTarget);
    const target = data.get('target') as string;
    // A new Target repoints every printed copy of the Short URL: say from where to where before it does.
    const retarget = (
      <>
        <p>{t('links.confirmRetarget', { url: short })}</p>
        <dl className="retarget">
          <dt className="micro">{t('links.retargetFrom')}</dt>
          <dd className="from">{link.target}</dd>
          <dt className="micro">{t('links.retargetTo')}</dt>
          <dd className="to">{target}</dd>
        </dl>
        <p className="hint">{t('links.confirmRetargetNote')}</p>
      </>
    );
    if (target !== link.target && !(await confirm(retarget, t('links.confirmRetargetAction')))) return;
    try {
      onChanged(
        link,
        await api<Link>('PATCH', linkPath(slug, link), {
          target,
          expires_at: fromInput(data.get('expires_at') as string),
          tags: tagList(data.get('tags') as string),
        }),
      );
      setError(undefined);
    } catch (err) {
      setError(err);
    }
  }

  // A QR code of an expired Link would only mislead, so the server refuses one.
  const expired = isExpired(link);
  const form = `${id}edit`;
  // Short, so a date and time fit one line of a half-width segment.
  const date = new Intl.DateTimeFormat(i18n.language, { dateStyle: 'short', timeStyle: 'short' });
  const short = link.short_url.replace(/^https?:\/\//, '');
  return (
    <section className="pass" aria-label={t('links.selected')}>
      <div className="pass-card print">
        <div className="pass-main">
          <Route from={link.domain} code={link.code} to={hostOf(link.target)} />
          {editing ? (
            <form
              id={form}
              className="segs"
              onSubmit={save}
              onKeyDown={(e) => {
                if (e.key === 'Escape') onEditing(false);
              }}
            >
              <div className="seg wide field">
                <label className="micro" htmlFor={`${id}target`}>
                  {t('links.target')}
                </label>
                <input
                  id={`${id}target`}
                  name="target"
                  type="url"
                  required
                  defaultValue={link.target}
                  className="mono"
                  aria-describedby={`${id}same`}
                  autoFocus
                />
                <p id={`${id}same`} className="hint">
                  {t('links.changeTargetHint')}
                </p>
              </div>
              {/* Full width each, so the Expiry's date and time both show. */}
              <label className="seg wide field">
                <span className="micro">{t('links.expiry')}</span>
                <input name="expires_at" type="datetime-local" defaultValue={toInput(link.expires_at)} />
              </label>
              <label className="seg wide field">
                <span className="micro">{t('links.tags')}</span>
                <input name="tags" defaultValue={link.tags.join(', ')} placeholder={t('links.tagsHint')} />
              </label>
            </form>
          ) : (
            <dl className="segs">
              <div className="seg l">
                <dt className="micro">{t('links.clickCount')}</dt>
                <dd className="big">
                  <a className="to-clicks" href={clicksHref(link)}>
                    <Flap text={new Intl.NumberFormat(i18n.language).format(link.click_count)} />
                    <span className="micro">{t('links.viewClicks')}</span>
                  </a>
                </dd>
              </div>
              <div className="seg r">
                <dt className="micro">{t('links.expiry')}</dt>
                <dd>
                  {link.expires_at ? date.format(new Date(link.expires_at)) : t('links.never')}
                  {expired && (
                    <>
                      {' '}
                      <span className="stamp expired">{t('links.expired')}</span>
                    </>
                  )}
                </dd>
              </div>
              <div className="seg l">
                <dt className="micro">{t('links.created')}</dt>
                <dd>{date.format(new Date(link.created_at))}</dd>
              </div>
              <div className="seg r">
                <dt className="micro">{t('links.tags')}</dt>
                <dd className={link.tags.length ? 'prose tags' : 'prose'}>
                  {link.tags.length
                    ? link.tags.map((name) => (
                        <span key={name} className="tag" title={name}>
                          {name}
                        </span>
                      ))
                    : t('links.noTags')}
                </dd>
              </div>
              <div className="seg wide">
                <dt className="micro">{t('links.target')}</dt>
                <dd>
                  <a href={link.target} rel="noreferrer">
                    {link.target}
                  </a>
                </dd>
                <dd className="change">
                  <button ref={change} className="small" aria-describedby={`${id}same`} onClick={() => onEditing(true)}>
                    {t('links.changeTarget')}
                  </button>
                  <span id={`${id}same`} className="hint">
                    {t('links.changeTargetHint')}
                  </span>
                </dd>
              </div>
            </dl>
          )}
          {error !== undefined && <p role="alert">{errorMessage(t, error)}</p>}
        </div>
        <div className="perf" aria-hidden="true" />
        <div className="stub" ref={stub}>
          {expired ? (
            <div className="qr blank">{t('links.qrExpired')}</div>
          ) : (
            <span className="qr">
              <img src={qrHref(slug, link, 'svg')} alt={t('links.qrAlt', { url: short })} width={124} height={124} />
            </span>
          )}
          {manualCopy ? (
            <input
              ref={manual}
              className="manual"
              name="short_url"
              readOnly
              value={link.short_url}
              aria-label={t('links.shortUrl')}
              onFocus={(e) => e.target.select()}
            />
          ) : (
            <ShortLine text={short} href={link.short_url} />
          )}
          {editing ? (
            // While editing, the stub ends the form: Save and Cancel in place of the QR's actions.
            <div className="acts">
              <button form={form} type="submit" className="primary">
                {t('links.save')}
              </button>
              <button className="small" onClick={() => onEditing(false)}>
                {t('links.cancel')}
              </button>
            </div>
          ) : (
          <div className="acts">
            {/* The QR code for a poster is the pass's main job; PNG prints anywhere, SVG for designers. */}
            {!expired && (
              <a className="button primary" href={qrHref(slug, link, 'png')} download={`${link.code}.png`}>
                {t('links.downloadQr')}
              </a>
            )}
            <div className="acts-row">
              <CopyButton
                link={link}
                onCopied={onCopied}
                onFailed={onCopyFailed}
                className={expired ? 'primary small' : 'small'}
              />
              {!expired && (
                <a
                  className="button small"
                  href={qrHref(slug, link, 'svg')}
                  download={`${link.code}.svg`}
                  aria-label={t('links.downloadQrAs', { format: 'SVG' })}
                >
                  SVG
                </a>
              )}
            </div>
          </div>
          )}
        </div>
      </div>
    </section>
  );
}

/** The board's More menu: export the Workspace's Links as CSV, or import a CSV file. */
function CsvTools({
  base,
  onImported,
}: {
  base: string;
  onImported: (result: { rows?: ImportedRow[]; error?: unknown }) => void;
}) {
  const { t } = useTranslation();
  const file = useRef<HTMLInputElement>(null);

  async function upload(input: HTMLInputElement) {
    const csv = input.files?.[0];
    if (!csv) return;
    try {
      onImported({ rows: await api<ImportedRow[]>('POST', `${base}.csv`, csv) });
    } catch (error) {
      onImported({ error });
    }
    input.value = '';
  }

  function download() {
    const a = document.createElement('a');
    a.href = `/api${base}.csv`;
    a.download = 'links.csv';
    a.click();
  }

  return (
    <>
      <MoreMenu
        label={t('links.boardActions')}
        items={[
          { label: t('links.exportCsv'), onSelect: download },
          { label: t('links.importCsv'), onSelect: () => file.current?.click() },
        ]}
      />
      <input
        ref={file}
        className="vh"
        type="file"
        name="csv"
        tabIndex={-1}
        accept=".csv,text/csv"
        aria-label={t('links.importCsv')}
        onChange={(e) => void upload(e.currentTarget)}
      />
    </>
  );
}

/** What a CSV import did, row by row. */
function ImportResult({ rows, error, onClose }: { rows?: ImportedRow[]; error?: unknown; onClose: () => void }) {
  const { t } = useTranslation();
  return (
    <div className="import">
      {error !== undefined && <p role="alert">{errorMessage(t, error)}</p>}
      {rows && (
        <table aria-label={t('links.importCsv')}>
          <thead>
            <tr>
              <th className="num">{t('links.line')}</th>
              <th>{t('links.result')}</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((row) => (
              <tr key={row.line}>
                <td className="num">{row.line}</td>
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
      )}
      <button className="small" onClick={onClose}>
        {t('close')}
      </button>
    </div>
  );
}

/** The Domains a Link can be created on: the Default Domain first, then the approved custom ones. */
const usableDomains = (domains: Domain[]) =>
  domains.filter((d) => d.default || d.status === 'approved').sort((a, b) => +b.default - +a.default);

/** Where a Link's pass lives: `/app/links/{domain}/{code}`. */
const linkHref = (link: Link) => `/app/links/${encodeURIComponent(link.domain)}/${encodeURIComponent(link.code)}`;

const qrHref = (slug: string, link: Link, format: string) => `/api${linkPath(slug, link, '/qr')}&format=${format}`;

const isExpired = (link: Link) => link.expires_at !== null && new Date(link.expires_at) <= new Date();

/** A Short URL as its scheme, its host with the slash, and (the rest) its Code. */
function splitShortUrl(link: Link): [string, string] {
  const url = link.short_url;
  const start = url.indexOf('://') + 3;
  return [url.slice(0, start), url.slice(start, url.length - link.code.length)];
}

/** A URL's host, or '' while it isn't one yet. */
function hostOf(url: string): string {
  try {
    return new URL(url).host;
  } catch {
    return '';
  }
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
