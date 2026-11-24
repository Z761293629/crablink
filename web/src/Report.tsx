// `/report` (served at `/app/report`): any visitor, signed in or not, reports a Short URL.

import { useState, type FormEvent } from 'react';
import { useTranslation } from 'react-i18next';
import { api } from './api';
import { errorMessage } from './i18n';

export function ReportShortUrl() {
  const { t } = useTranslation();
  const [result, setResult] = useState<unknown>();

  async function submit(e: FormEvent<HTMLFormElement>) {
    e.preventDefault();
    const form = e.currentTarget;
    const fields = new FormData(form);
    try {
      await api('POST', '/reports', { short_url: fields.get('short_url'), reason: fields.get('reason') });
      form.reset();
      setResult('sent');
    } catch (err) {
      setResult(err);
    }
  }

  return (
    <form className="ticket" onSubmit={submit}>
      <div className="ticket-body">
        <h1>{t('report.title')}</h1>
        <p>{t('report.intro')}</p>
        <label className="field">
          <span className="micro">{t('links.shortUrl')}</span>
          <input
            name="short_url"
            className="mono"
            placeholder="crabl.ink/aZ3k9"
            defaultValue={new URLSearchParams(location.search).get('short_url') ?? undefined}
            required
          />
        </label>
        <label className="field">
          <span className="micro">{t('report.reason')}</span>
          <textarea name="reason" placeholder={t('report.reasonHint')} maxLength={1000} required />
        </label>
        {result === 'sent' ? (
          <p role="status">{t('report.sent')}</p>
        ) : (
          result !== undefined && <p role="alert">{errorMessage(t, result)}</p>
        )}
      </div>
      <div className="tear">
        <button type="submit" className="primary">
          {t('report.submit')}
        </button>
      </div>
    </form>
  );
}
