// `/app/clicks`: the switcher's Workspace's Clicks with its top Links, or with
// `?code=&domain=` one of its Links' Clicks with breakdowns.

import { useEffect, useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { api, linkPath, type ClickBreakdown, type ClickBucket, type ClickStats, type WorkspaceClicks } from './api';
import { Flap, PageError } from './ui';

/** The time ranges on offer: `n` buckets of `interval`, ending with the current one. */
const RANGES = { '24h': ['hour', 24], '7d': ['day', 7], '30d': ['day', 30], '90d': ['day', 90] } as const;
type Range = keyof typeof RANGES;

/** The address of a Link's Clicks page. */
export function clicksHref(link: { code: string; domain: string }): string {
  return `/app/clicks?${new URLSearchParams({ code: link.code, domain: link.domain })}`;
}

export function Clicks({ slug }: { slug: string }) {
  const q = new URLSearchParams(location.search);
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

/** GETs `path`, which may have a query already, over `range`. */
function useClicks<T>(path: string, range: Range) {
  const [data, setData] = useState<T>();
  const [error, setError] = useState<unknown>();
  const [tries, setTries] = useState(0);
  useEffect(() => {
    const sep = path.includes('?') ? '&' : '?';
    api<T>('GET', `${path}${sep}${query(range)}`).then(setData, setError);
  }, [path, range, tries]);
  const retry = () => {
    setError(undefined);
    setTries((n) => n + 1);
  };
  return { data, error, retry };
}

function RangePicker({ range, onChange }: { range: Range; onChange: (r: Range) => void }) {
  const { t } = useTranslation();
  return (
    <label className="field">
      <span className="micro">{t('clicks.range')}</span>
      <select name="range" value={range} onChange={(e) => onChange(e.target.value as Range)}>
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
  const { data, error, retry } = useClicks<ClickStats>(linkPath(slug, { code, domain }, '/clicks'), range);
  const countryName = useCountryName();
  return (
    <>
      <div className="page-head">
        <div>
          <a className="back" href="/app/clicks">
            {t('clicks.workspaceTitle')}
          </a>
          <h1>{t('clicks.linkTitle', { url: `${domain}/${code}` })}</h1>
        </div>
        <RangePicker range={range} onChange={setRange} />
      </div>
      {error !== undefined ? (
        <PageError error={error} onRetry={retry} />
      ) : (
        data && (
          // The back of the Link's pass: Clicks over time, then each breakdown as a segment.
          <ClicksPass>
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
          </ClicksPass>
        )
      )}
    </>
  );
}

function WorkspaceClicksView({ slug }: { slug: string }) {
  const { t, i18n } = useTranslation();
  const count = new Intl.NumberFormat(i18n.language);
  const [range, setRange] = useState<Range>('30d');
  const { data, error, retry } = useClicks<WorkspaceClicks>(`/workspaces/${encodeURIComponent(slug)}/clicks`, range);
  return (
    <>
      <div className="page-head">
        <h1>{t('clicks.workspaceTitle')}</h1>
        <RangePicker range={range} onChange={setRange} />
      </div>
      {error !== undefined ? (
        <PageError error={error} onRetry={retry} />
      ) : (
        data && (
          <>
            <ClicksPass>
              <SeriesChart series={data.series} hourly={RANGES[range][0] === 'hour'} />
            </ClicksPass>
            <section className="sheet">
              <h2>{t('clicks.topLinks')}</h2>
              {data.top_links.length === 0 ? (
                <p className="empty">{t('clicks.none')}</p>
              ) : (
                <div className="table">
                  <table aria-label={t('clicks.topLinks')}>
                    <thead>
                      <tr>
                        <th>{t('links.shortUrl')}</th>
                        <th className="num">{t('clicks.clicks')}</th>
                      </tr>
                    </thead>
                    <tbody>
                      {data.top_links.map((l) => (
                        <tr key={l.short_url}>
                          <td className="mono">
                            <a className="short-link" href={clicksHref(l)}>
                              <span className="vh">{l.short_url.slice(0, l.short_url.indexOf('://') + 3)}</span>
                              <span className="host">{l.domain}/</span>
                              <span className="code">{l.code}</span>
                            </a>
                          </td>
                          <td className="num">
                            <Flap text={count.format(l.count)} />
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              )}
            </section>
          </>
        )
      )}
    </>
  );
}

/** Clicks on a light pass, the same paper as the Links page's. */
function ClicksPass({ children }: { children: ReactNode }) {
  return (
    <div className="pass">
      <div className="pass-card clicks-card">{children}</div>
    </div>
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
  const number = new Intl.NumberFormat(i18n.language);
  const total = series.reduce((sum, b) => sum + b.count, 0);
  const peak = Math.max(0, ...series.map((b) => b.count));
  const max = Math.max(1, peak);
  const slot = W / Math.max(1, series.length);
  // Bars narrower than their slot, so a lone busy day reads as one day among the others.
  const bar = Math.max(1, slot * 0.6);
  return (
    <figure className="chart">
      <figcaption>{t('clicks.total', { count: total })}</figcaption>
      <div className="seg-head">
        <span className="micro">{t('clicks.peak')}</span>
        <p className="value">
          <Flap text={number.format(peak)} />
        </p>
      </div>
      <div className="seg-head">
        <span className="micro">{t('clicks.eachBar')}</span>
        <p className="value">{t(`clicks.per.${hourly ? 'hour' : 'day'}`)}</p>
      </div>
      <svg viewBox={`0 0 ${W} ${H}`} preserveAspectRatio="none" role="img" aria-label={t('clicks.overTime')}>
        <line x1={0} x2={W} y1={H - 0.5} y2={H - 0.5} className="axis" vectorEffect="non-scaling-stroke" />
        {/* Hairline at the peak, so the bars read against a scale. */}
        <line x1={0} x2={W} y1={PAD} y2={PAD} className="peak" vectorEffect="non-scaling-stroke" />
        {series.map((b, i) => {
          const h = (b.count / max) * (H - PAD);
          return (
            <g key={b.time}>
              <title>{`${date.format(new Date(b.time))}: ${t('clicks.total', { count: b.count })}`}</title>
              {/* A full-height hit target, so an empty bucket still has a tooltip. */}
              <rect x={i * slot} y={0} width={slot} height={H} fill="transparent" />
              {b.count > 0 ? (
                <rect x={i * slot + (slot - bar) / 2} y={H - h} width={bar} height={h} />
              ) : (
                // An empty bucket still shows: a stub on the axis.
                <rect className="none" x={i * slot + (slot - bar) / 2} y={H - 3} width={bar} height={3} />
              )}
            </g>
          );
        })}
      </svg>
      <div className="chart-axis micro">
        <span>{series.length > 0 && date.format(new Date(series[0].time))}</span>
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
  const { t, i18n } = useTranslation();
  const max = Math.max(1, ...rows.map((r) => r.count));
  const number = new Intl.NumberFormat(i18n.language);
  return (
    <section>
      <h2 className="micro">{title}</h2>
      {rows.length === 0 ? (
        <p className="empty">{t('clicks.none')}</p>
      ) : (
        <table aria-label={title}>
          <tbody>
            {rows.map((r) => (
              <tr key={r.value ?? ''}>
                <td>
                  {label(r.value)}
                  <div className="bar" style={{ width: `${(r.count / max) * 100}%` }} />
                </td>
                <td className="num">
                  <Flap text={number.format(r.count)} />
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </section>
  );
}
