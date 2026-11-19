-- A Tag belongs to one Workspace, and goes with it.
CREATE TABLE tags (
    id           BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    workspace_id BIGINT NOT NULL REFERENCES workspaces (id) ON DELETE CASCADE,
    name         TEXT NOT NULL,
    UNIQUE (workspace_id, name)
);

CREATE TABLE link_tags (
    link_id BIGINT NOT NULL REFERENCES links (id),
    tag_id  BIGINT NOT NULL REFERENCES tags (id) ON DELETE CASCADE,
    PRIMARY KEY (link_id, tag_id)
);

CREATE INDEX link_tags_tag_id ON link_tags (tag_id);
