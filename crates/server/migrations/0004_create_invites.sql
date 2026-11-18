CREATE TABLE invites (
    -- SHA-256 of the invite link's random token; the token itself is never stored.
    token_hash   BYTEA PRIMARY KEY,
    -- The new User's email.
    email        TEXT NOT NULL,
    -- The Workspace the new User joins as a Member, if any.
    workspace_id BIGINT REFERENCES workspaces (id) ON DELETE CASCADE,
    expires_at   TIMESTAMPTZ NOT NULL,
    -- Set when accepted: an invite works once.
    used_at      TIMESTAMPTZ
);
