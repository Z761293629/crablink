// A User's own password: changing it, and choosing it when accepting an invite.

import { useState, type FormEvent } from 'react';
import { useTranslation } from 'react-i18next';
import { api } from './api';
import { errorMessage } from './i18n';
import { tearHalf, tearOff } from './ui';

export function ChangePassword() {
  const { t } = useTranslation();
  const [result, setResult] = useState<unknown>();

  async function submit(e: FormEvent<HTMLFormElement>) {
    e.preventDefault();
    const form = e.currentTarget;
    const fields = new FormData(form);
    setResult(undefined);
    try {
      await api('POST', '/me/password', {
        current_password: fields.get('current'),
        new_password: fields.get('new'),
      });
      form.reset();
      setResult('changed');
    } catch (err) {
      setResult(err);
    }
  }

  return (
    <form className="ticket" onSubmit={submit}>
      <div className="ticket-body">
        <h1>{t('account.title')}</h1>
        <label className="field">
          <span className="micro">{t('account.currentPassword')}</span>
          <input name="current" type="password" autoComplete="current-password" required />
        </label>
        <label className="field">
          <span className="micro">{t('account.newPassword')}</span>
          <input name="new" type="password" autoComplete="new-password" required />
        </label>
        {result === 'changed' ? (
          <p role="status">{t('account.changed')}</p>
        ) : (
          result !== undefined && <p role="alert">{errorMessage(t, result)}</p>
        )}
      </div>
      <div className="tear">
        <button type="submit" className="primary">
          {t('account.change')}
        </button>
      </div>
    </form>
  );
}

/** `/app/invite#<token>`: the invited person chooses a password and is signed in. */
export function AcceptInvite() {
  const { t } = useTranslation();
  const [error, setError] = useState<unknown>();

  async function submit(e: FormEvent<HTMLFormElement>) {
    e.preventDefault();
    const ticket = e.currentTarget;
    // The last error stays until the answer replaces it: cleared now, the button would jump up and back.
    const password = new FormData(ticket).get('password');
    try {
      await api('POST', '/invites/accept', { token: location.hash.slice(1), password });
      // Signed in: the foot is torn off along the perforation, then the app opens.
      await tearOff(ticket);
      location.assign('/app/');
    } catch (err) {
      setError(err);
      tearHalf(ticket);
    }
  }

  return (
    <form className="ticket" onSubmit={submit}>
      <div className="ticket-body">
        <h1>{t('invite.title')}</h1>
        <label className="field">
          <span className="micro">{t('invite.password')}</span>
          <input name="password" type="password" autoComplete="new-password" required />
        </label>
        {error !== undefined && <p role="alert">{errorMessage(t, error)}</p>}
      </div>
      <div className="tear">
        <button type="submit" className="primary">
          {t('invite.submit')}
        </button>
      </div>
    </form>
  );
}
