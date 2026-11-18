CREATE TABLE workspaces (
    id   BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    name TEXT NOT NULL,
    slug TEXT NOT NULL UNIQUE
);

CREATE TABLE memberships (
    user_id      BIGINT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    workspace_id BIGINT NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
    role         TEXT NOT NULL CHECK (role IN ('owner', 'member')),
    PRIMARY KEY (user_id, workspace_id)
);

CREATE INDEX memberships_workspace_id ON memberships (workspace_id);

-- Set on a User's first sign-in, which is when their personal Workspace is created.
ALTER TABLE users ADD COLUMN signed_in BOOLEAN NOT NULL DEFAULT FALSE;

CREATE TABLE domains (
    id         BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    -- With the port if it isn't the scheme's default, e.g. `127.0.0.1:8080`.
    host       TEXT NOT NULL UNIQUE,
    is_default BOOLEAN NOT NULL DEFAULT FALSE
);

-- At most one Default Domain.
CREATE UNIQUE INDEX domains_one_default ON domains (is_default) WHERE is_default;

-- 0.4 starts empty (ADR-0007), so `links` has no rows to fill these in for.
ALTER TABLE links
    ADD COLUMN domain_id    BIGINT NOT NULL REFERENCES domains (id),
    ADD COLUMN workspace_id BIGINT NOT NULL REFERENCES workspaces (id),
    DROP CONSTRAINT links_code_key,
    -- A Code is unique within its Domain, Deleted Links included (ADR-0002, ADR-0005).
    ADD CONSTRAINT links_domain_id_code_key UNIQUE (domain_id, code);

CREATE INDEX links_workspace_id_id ON links (workspace_id, id);

CREATE TABLE api_keys (
    id           BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    workspace_id BIGINT NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
    name         TEXT NOT NULL,
    -- The secret's first characters, shown so people can tell keys apart.
    prefix       TEXT NOT NULL,
    -- SHA-256 of the secret; the secret itself is shown once and never stored.
    secret_hash  BYTEA NOT NULL UNIQUE,
    -- Who made it; the key outlives them.
    created_by   BIGINT REFERENCES users (id) ON DELETE SET NULL,
    created_at   TIMESTAMPTZ NOT NULL,
    last_used_at TIMESTAMPTZ,
    revoked_at   TIMESTAMPTZ
);

CREATE INDEX api_keys_workspace_id ON api_keys (workspace_id);
