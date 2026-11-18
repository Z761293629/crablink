CREATE TABLE users (
    id             BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    email          TEXT NOT NULL,
    password_hash  TEXT NOT NULL,
    instance_admin BOOLEAN NOT NULL DEFAULT FALSE,
    disabled       BOOLEAN NOT NULL DEFAULT FALSE
);

-- Emails are unique ignoring case; sign-in looks them up the same way.
CREATE UNIQUE INDEX users_email_key ON users (lower(email));

CREATE TABLE sessions (
    -- SHA-256 of the cookie's random token; the token itself is never stored.
    token_hash BYTEA PRIMARY KEY,
    user_id    BIGINT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    expires_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX sessions_user_id ON sessions (user_id);
