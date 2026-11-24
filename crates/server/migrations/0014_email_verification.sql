-- When a User's email was shown to be theirs. Only open sign-up creates a User
-- without it; Users created by Invite, by an Instance Admin or through an Identity
-- Provider, and every User before open sign-up existed, count as verified.
ALTER TABLE users ADD COLUMN email_verified_at TIMESTAMPTZ;
UPDATE users SET email_verified_at = now();
