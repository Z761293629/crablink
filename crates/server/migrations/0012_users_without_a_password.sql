-- A User an Identity Provider created has no password: they sign in only through
-- Identity Providers, and password sign-in fails for them like a wrong password.
ALTER TABLE users ALTER COLUMN password_hash DROP NOT NULL;
