// A User managing themself: signing up and verifying the email, changing their password,
// deleting themself, resetting a forgotten password by email, and choosing one
// when accepting an invite.

import { useEffect, useState, type FormEvent } from 'react';
import { Trans, useTranslation } from 'react-i18next';
import { api, type Instance } from './api';
import { errorMessage } from './i18n';
import { tearHalf, tearOff, useConfirm } from './ui';

/** `/app/account`: changing the password, for Users who have one, and deleting the User. */
export function Account({ hasPassword, onDeleted }: { hasPassword: boolean; onDeleted: () => void }) {
  return (
    <>
      {hasPassword && <ChangePassword />}
      <DeleteAccount hasPassword={hasPassword} onDeleted={onDeleted} />
    </>
  );
}

/**
 * Deletes the account after an "it cannot be undone"; a User with a password confirms with it,
 * one without must have just signed in through an Identity Provider.
 */
function DeleteAccount({ hasPassword, onDeleted }: { hasPassword: boolean; onDeleted: () => void }) {
  const { t } = useTranslation();
  const [dialog, ask] = useConfirm();
  const [error, setError] = useState<unknown>();

  async function submit(e: FormEvent<HTMLFormElement>) {
    e.preventDefault();
    const password = new FormData(e.currentTarget).get('password') ?? undefined;
    if (!(await ask(t('account.confirmDelete'), t('account.delete')))) return;
    try {
      await api('DELETE', '/me', { password });
      onDeleted();
    } catch (err) {
      setError(err);
    }
  }

  return (
    <>
      <form className="ticket" aria-labelledby="delete-account" onSubmit={submit}>
        <div className="ticket-body">
          <h2 id="delete-account">{t('account.delete')}</h2>
          <p>{t(hasPassword ? 'account.deleteLede' : 'account.deleteLedeNoPassword')}</p>
          {hasPassword && (
            <label className="field">
              <span className="micro">{t('account.password')}</span>
              <input name="password" type="password" autoComplete="current-password" required />
            </label>
          )}
          {error !== undefined && <p role="alert">{errorMessage(t, error)}</p>}
        </div>
        <div className="tear">
          <button type="submit" className="danger">
            {t('account.delete')}
          </button>
        </div>
      </form>
      {dialog}
    </>
  );
}

function ChangePassword() {
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

/** `/app/forgot-password`: asks for a password reset link by email. The answer never says whether a User has the email. */
export function ForgotPassword() {
  const { t } = useTranslation();
  const [result, setResult] = useState<unknown>();

  async function submit(e: FormEvent<HTMLFormElement>) {
    e.preventDefault();
    const email = new FormData(e.currentTarget).get('email');
    try {
      await api('POST', '/password-resets', { email });
      setResult('sent');
    } catch (err) {
      setResult(err);
    }
  }

  return (
    <form className="ticket" onSubmit={submit}>
      <div className="ticket-body">
        <h1>{t('forgotPassword.title')}</h1>
        <label className="field">
          <span className="micro">{t('signIn.email')}</span>
          <input name="email" type="email" autoComplete="username" required />
        </label>
        {result === 'sent' ? (
          <p role="status">{t('forgotPassword.sent')}</p>
        ) : (
          result !== undefined && <p role="alert">{errorMessage(t, result)}</p>
        )}
        <p>
          <a href="/app/">{t('forgotPassword.back')}</a>
        </p>
      </div>
      <div className="tear">
        <button type="submit" className="primary">
          {t('forgotPassword.submit')}
        </button>
      </div>
    </form>
  );
}

/** `/app/reset-password#<token>`: the emailed link; choosing a new password signs the User in. */
export function ResetPassword() {
  const { t } = useTranslation();
  const [error, setError] = useState<unknown>();

  async function submit(e: FormEvent<HTMLFormElement>) {
    e.preventDefault();
    const ticket = e.currentTarget;
    const password = new FormData(ticket).get('password');
    try {
      await api('POST', '/password-resets/use', { token: location.hash.slice(1), password });
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
        <h1>{t('resetPassword.title')}</h1>
        <label className="field">
          <span className="micro">{t('account.newPassword')}</span>
          <input name="password" type="password" autoComplete="new-password" required />
        </label>
        {error !== undefined && <p role="alert">{errorMessage(t, error)}</p>}
      </div>
      <div className="tear">
        <button type="submit" className="primary">
          {t('resetPassword.submit')}
        </button>
      </div>
    </form>
  );
}

/** `/app/sign-up`: anyone signs up with an email and a password, on an instance with open sign-up. */
export function SignUp() {
  const { t } = useTranslation();
  const [error, setError] = useState<unknown>();
  const [instance, setInstance] = useState<Instance>();

  useEffect(() => {
    // Without it, the form still signs up; it just links no terms.
    api<Instance>('GET', '/instance').then(setInstance, () => {});
  }, []);

  async function submit(e: FormEvent<HTMLFormElement>) {
    e.preventDefault();
    const ticket = e.currentTarget;
    const form = new FormData(ticket);
    try {
      await api('POST', '/sign-ups', { email: form.get('email'), password: form.get('password') });
      await tearOff(ticket);
      location.assign('/app/');
    } catch (err) {
      setError(err);
      tearHalf(ticket);
    }
  }

  const { terms_url: terms, privacy_url: privacy } = instance ?? {};
  const agree = terms && privacy ? 'agreeBoth' : terms ? 'agreeTerms' : privacy ? 'agreePrivacy' : undefined;
  return (
    <form className="ticket" onSubmit={submit}>
      <div className="ticket-body">
        <h1>{t('signUp.title')}</h1>
        <label className="field">
          <span className="micro">{t('signIn.email')}</span>
          <input name="email" type="email" autoComplete="username" required />
        </label>
        <label className="field">
          <span className="micro">{t('signIn.password')}</span>
          <input name="password" type="password" autoComplete="new-password" minLength={8} required />
        </label>
        {agree && (
          <p>
            <Trans
              i18nKey={`signUp.${agree}`}
              components={{
                terms: <a href={terms ?? undefined} target="_blank" rel="noreferrer" />,
                privacy: <a href={privacy ?? undefined} target="_blank" rel="noreferrer" />,
              }}
            />
          </p>
        )}
        {error !== undefined && <p role="alert">{errorMessage(t, error)}</p>}
        <p>
          <a href="/app/">{t('forgotPassword.back')}</a>
        </p>
      </div>
      <div className="tear">
        <button type="submit" className="primary">
          {t('signUp.submit')}
        </button>
      </div>
    </form>
  );
}

/** `/app/verify-email#<token>`: the emailed link; following it lets the User create Links and API Keys. */
export function VerifyEmail() {
  const { t } = useTranslation();
  const [error, setError] = useState<unknown>();

  async function submit(e: FormEvent<HTMLFormElement>) {
    e.preventDefault();
    const ticket = e.currentTarget;
    try {
      await api('POST', '/email-verifications/use', { token: location.hash.slice(1) });
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
        <h1>{t('verifyEmail.title')}</h1>
        {error !== undefined && <p role="alert">{errorMessage(t, error)}</p>}
      </div>
      <div className="tear">
        <button type="submit" className="primary">
          {t('verifyEmail.submit')}
        </button>
      </div>
    </form>
  );
}

/** Across the top while the signed-in User's email isn't verified: why they can't create yet, and the link again. */
export function VerifyBanner({ email }: { email: string }) {
  const { t } = useTranslation();
  const [result, setResult] = useState<unknown>();

  async function resend() {
    try {
      await api('POST', '/email-verifications');
      setResult('sent');
    } catch (err) {
      setResult(err);
    }
  }

  return (
    <aside className="verify-banner" aria-label={t('verifyEmail.title')}>
      <p>{t('verifyEmail.banner', { email })}</p>
      {result === 'sent' ? (
        <p role="status">{t('verifyEmail.resent')}</p>
      ) : (
        <>
          {result !== undefined && <p role="alert">{errorMessage(t, result)}</p>}
          <button type="button" onClick={resend}>
            {t('verifyEmail.resend')}
          </button>
        </>
      )}
    </aside>
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
