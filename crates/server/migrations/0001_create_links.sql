CREATE TABLE links (
    id          BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    code        TEXT NOT NULL UNIQUE,
    target      TEXT NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL,
    updated_at  TIMESTAMPTZ NOT NULL,
    expires_at  TIMESTAMPTZ,
    deleted_at  TIMESTAMPTZ,
    click_count BIGINT NOT NULL DEFAULT 0
);
