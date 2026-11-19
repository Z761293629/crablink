-- One row per human redirect. Never the visitor's IP or full User-Agent (ADR-0006).
-- `links.click_count` is bumped in the same statement that inserts a row, so it
-- stays the number of a Link's Clicks.
CREATE TABLE clicks (
    id            BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    link_id       BIGINT NOT NULL REFERENCES links (id),
    at            TIMESTAMPTZ NOT NULL,
    referrer_host TEXT,
    -- ISO 3166-1 alpha-2.
    country       TEXT,
    device        TEXT NOT NULL CHECK (device IN ('desktop', 'mobile', 'tablet')),
    browser       TEXT
);

CREATE INDEX clicks_link_id_at ON clicks (link_id, at);
