-- A visitor's report of a Short URL. Never the visitor's IP (ADR-0006). Open until
-- an Instance Admin dismisses it or blocks its Link.
CREATE TABLE reports (
    id          BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    link_id     BIGINT NOT NULL REFERENCES links (id),
    reason      TEXT NOT NULL,
    reported_at TIMESTAMPTZ NOT NULL,
    closed_at   TIMESTAMPTZ
);

CREATE INDEX reports_open ON reports (reported_at) WHERE closed_at IS NULL;
