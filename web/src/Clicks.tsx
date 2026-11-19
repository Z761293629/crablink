// `/app/clicks?workspace={slug}[&code=&domain=]`: one Link's Clicks with breakdowns, or
// the whole Workspace's with its top Links.

import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { api, type ClickBreakdown, type ClickBucket, type ClickStats, type WorkspaceClicks } from './api';
import { errorMessage } from './i18n';

/** The time ranges on offer: `n` buckets of `interval`, ending with the current one. */
const RANGES = { '24h': ['hour', 24], '7d': ['day', 7], '30d': ['day', 30], '90d': ['day', 90] } as const;
type Range = keyof typeof RANGES;

/** The page's address for a Workspace's Clicks, or one of its Links' when given. */
export function clicksHref(slug: string, link?: { code: string; domain: string }): string {
  const q = new URLSearchParams({ workspace: slug });
  if (link) {
    q.set('code', link.code);
    q.set('domain', link.domain);
  }
  return `/app/clicks?${q}`;
}

export function ClicksPage() {
  const q = new URLSearchParams(location.search);
  const slug = q.get('workspace') ?? '';
  const code = q.get('code');
  const domain = q.get('domain') ?? '';
  return code ? <LinkClicks slug={slug} code={code} domain={domain} /> : <WorkspaceClicksView slug={slug} />;
}

/** `from`, `to` and `interval` of `range`, in whole UTC buckets. */
function query(range: Range): string {
  const [interval, n] = RANGES[range];
  const ms = interval === 'hour' ? 3_600_000 : 86_400_000;
  const to = Math.floor(Date.now() / ms) * ms + ms;
  const from = to - n * ms;
  return new URLSearchParams({
    from: new Date(from).toISOString(),
    to: new Date(to).toISOString(),
    interval,
  }).toString();
}

/** Loads `path?{range}` whenever the range changes; `undefined` while loading. */
function useClicks<T>(path: string, range: Range, extra = '') {
  const [data, setData] = useState<T>();
  const [error, setError] = useState<unknown>();
  useEffect(() => {
    api<T>('GET', `${path}?${query(range)}${extra}`).then(setData, setError);
  }, [path, range, extra]);
  return { data, error };
}

function RangePicker({ range, onChange }: { range: Range; onChange: (r: Range) => void }) {
  const { t } = useTranslation();
  return (
    <label className="inline">
      {t('clicks.range')}
      <select value={range} onChange={(e) => onChange(e.target.value as Range)}>
        {Object.keys(RANGES).map((r) => (
          <option key={r} value={r}>
            {t(`clicks.ranges.${r}`)}
          </option>
        ))}
      </select>
    </label>
  );
}

function LinkClicks({ slug, code, domain }: { slug: string; code: string; domain: string }) {
  const { t } = useTranslation();
  const [range, setRange] = useState<Range>('30d');
  const path = `/workspaces/${encodeURIComponent(slug)}/links/${encodeURIComponent(code)}/clicks`;
  const { data, error } = useClicks<ClickStats>(path, range, `&domain=${encodeURIComponent(domain)}`);
  const countryName = useCountryName();
  return (
    <>
      <p>
        <a href={clicksHref(slug)}>{t('clicks.workspaceTitle')}</a>
      </p>
      <h1>{t('clicks.linkTitle', { url: `${domain}/${code}` })}</h1>
      <RangePicker range={range} onChange={setRange} />
      {error !== undefined ? (
        <p role="alert">{errorMessage(t, error)}</p>
      ) : (
        data && (
          <>
            <SeriesChart series={data.series} hourly={RANGES[range][0] === 'hour'} />
            <div className="breakdowns">
              <Breakdown title={t('clicks.referrers')} rows={data.referrers} label={(v) => v ?? t('clicks.direct')} />
              <Breakdown title={t('clicks.countries')} rows={data.countries} label={countryName} />
              <Breakdown
                title={t('clicks.devices')}
                rows={data.devices}
                label={(v) => (v ? t(`clicks.device.${v}`, { defaultValue: v }) : t('clicks.unknown'))}
              />
              <Breakdown title={t('clicks.browsers')} rows={data.browsers} label={(v) => v ?? t('clicks.unknown')} />
            </div>
          </>
        )
      )}
    </>
  );
}

function WorkspaceClicksView({ slug }: { slug: string }) {
  const { t } = useTranslation();
  const [range, setRange] = useState<Range>('30d');
  const { data, error } = useClicks<WorkspaceClicks>(`/workspaces/${encodeURIComponent(slug)}/clicks`, range);
  return (
    <>
      <h1>{t('clicks.workspaceTitle')}</h1>
      <RangePicker range={range} onChange={setRange} />
      {error !== undefined ? (
        <p role="alert">{errorMessage(t, error)}</p>
      ) : (
        data && (
          <>
            <SeriesChart series={data.series} hourly={RANGES[range][0] === 'hour'} />
            <h2>{t('clicks.topLinks')}</h2>
            {data.top_links.length === 0 ? (
              <p>{t('clicks.none')}</p>
            ) : (
              <div className="table">
                <table>
                  <thead>
                    <tr>
                      <th>{t('links.shortUrl')}</th>
                      <th>{t('clicks.clicks')}</th>
                    </tr>
                  </thead>
                  <tbody>
                    {data.top_links.map((l) => (
                      <tr key={l.short_url}>
                        <td>
                          <a href={clicksHref(slug, l)}>{l.short_url}</a>
                        </td>
                        <td>{l.count}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}
          </>
        )
      )}
    </>
  );
}

/** The country's name in the UI's language, from its ISO code. */
function useCountryName() {
  const { t, i18n } = useTranslation();
  const names = new Intl.DisplayNames([i18n.language], { type: 'region' });
  return (code: string | null) => (code ? (names.of(code) ?? code) : t('clicks.unknown'));
}

const W = 640;
const H = 160;
const PAD = 4;

/** Clicks per bucket as bars, with the total; each bar's tooltip gives its bucket and count. */
function SeriesChart({ series, hourly }: { series: ClickBucket[]; hourly: boolean }) {
  const { t, i18n } = useTranslation();
  // Day buckets are UTC days, so they are named in UTC; hours in local time.
  const date = new Intl.DateTimeFormat(
    i18n.language,
    hourly ? { hour: 'numeric', minute: '2-digit' } : { month: 'short', day: 'numeric', timeZone: 'UTC' },
  );
  const total = series.reduce((sum, b) => sum + b.count, 0);
  const max = Math.max(1, ...series.map((b) => b.count));
  const slot = W / series.length;
  const bar = Math.max(1, slot - 2);
  return (
    <figure className="chart">
      <figcaption>{t('clicks.total', { count: total })}</figcaption>
      <svg viewBox={`0 0 ${W} ${H}`} preserveAspectRatio="none" role="img" aria-label={t('clicks.overTime')}>
        <line x1={0} x2={W} y1={H - 0.5} y2={H - 0.5} className="axis" vectorEffect="non-scaling-stroke" />
        {series.map((b, i) => {
          const h = (b.count / max) * (H - PAD);
          return (
            <g key={b.time}>
              <title>{`${date.format(new Date(b.time))}: ${t('clicks.total', { count: b.count })}`}</title>
              {/* A full-height hit target, so an empty bucket still has a tooltip. */}
              <rect x={i * slot} y={0} width={slot} height={H} fill="transparent" />
              {b.count > 0 && <rect x={i * slot + 1} y={H - h} width={bar} height={h} rx={Math.min(2, bar / 2)} />}
            </g>
          );
        })}
      </svg>
      <div className="chart-axis">
        <span>{series.length > 0 && date.format(new Date(series[0].time))}</span>
        <span>{t('clicks.max', { value: max })}</span>
        <span>{series.length > 0 && date.format(new Date(series[series.length - 1].time))}</span>
      </div>
    </figure>
  );
}

/** One breakdown as a table with a bar per row, most Clicks first. */
function Breakdown({
  title,
  rows,
  label,
}: {
  title: string;
  rows: ClickBreakdown[];
  label: (value: string | null) => string;
}) {
  const { t } = useTranslation();
  const max = Math.max(1, ...rows.map((r) => r.count));
  return (
    <section>
      <h2>{title}</h2>
      {rows.length === 0 ? (
        <p>{t('clicks.none')}</p>
      ) : (
        <table aria-label={title}>
          <tbody>
            {rows.map((r) => (
              <tr key={r.value ?? ''}>
                <td>
                  {label(r.value)}
                  <div className="bar" style={{ width: `${(r.count / max) * 100}%` }} />
                </td>
                <td className="count">{r.count}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </section>
  );
}
