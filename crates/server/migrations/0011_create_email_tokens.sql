-- One-time links the instance emails to a User, such as a password reset link.
CREATE TABLE email_tokens (
    -- SHA-256 of the link's random token; the token itself is never stored.
    token_hash BYTEA PRIMARY KEY,
    user_id    BIGINT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    -- What the link does: 'password_reset'.
    purpose    TEXT NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX email_tokens_user_id ON email_tokens (user_id);
