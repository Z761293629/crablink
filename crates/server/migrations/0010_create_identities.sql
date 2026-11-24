-- A User's identities at Identity Providers: who an ID token's issuer and subject
-- name. One identity belongs to at most one User; a User may have many.
CREATE TABLE identities (
    issuer  TEXT NOT NULL,
    subject TEXT NOT NULL,
    user_id BIGINT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    PRIMARY KEY (issuer, subject)
);
