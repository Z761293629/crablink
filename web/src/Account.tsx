// A User's own password: changing it, and choosing it when accepting an invite.

import { useState, type FormEvent } from 'react';
import { useTranslation } from 'react-i18next';
import { api } from './api';
import { errorMessage } from './i18n';

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
    <form className="narrow" onSubmit={submit}>
      <h1>{t('account.title')}</h1>
      <label>
        {t('account.currentPassword')}
        <input name="current" type="password" autoComplete="current-password" required />
      </label>
      <label>
        {t('account.newPassword')}
        <input name="new" type="password" autoComplete="new-password" required />
      </label>
      {result === 'changed' ? (
        <p role="status">{t('account.changed')}</p>
      ) : (
        result !== undefined && <p role="alert">{errorMessage(t, result)}</p>
      )}
      <button type="submit">{t('account.change')}</button>
    </form>
  );
}

/** `/app/invite#<token>`: the invited person chooses a password and is signed in. */
export function AcceptInvite() {
  const { t } = useTranslation();
  const [error, setError] = useState<unknown>();

  async function submit(e: FormEvent<HTMLFormElement>) {
    e.preventDefault();
    const password = new FormData(e.currentTarget).get('password');
    try {
      await api('POST', '/invites/accept', { token: location.hash.slice(1), password });
      location.assign('/app/');
    } catch (err) {
      setError(err);
    }
  }

  return (
    <form className="narrow" onSubmit={submit}>
      <h1>{t('invite.title')}</h1>
      <label>
        {t('invite.password')}
        <input name="password" type="password" autoComplete="new-password" required />
      </label>
      {error !== undefined && <p role="alert">{errorMessage(t, error)}</p>}
      <button type="submit">{t('invite.submit')}</button>
    </form>
  );
}
