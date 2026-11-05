-- Timestamps are bound from Rust (chrono DateTime<Utc>), never defaulted in SQL,
-- so every stored value has the same text format and compares correctly.
CREATE TABLE links (
    id          INTEGER PRIMARY KEY,
    code        TEXT NOT NULL UNIQUE,
    target      TEXT NOT NULL,
    created_at  TIMESTAMP NOT NULL,
    updated_at  TIMESTAMP NOT NULL,
    expires_at  TIMESTAMP,
    deleted_at  TIMESTAMP,
    click_count BIGINT NOT NULL DEFAULT 0
);
