-- A custom Domain belongs to the Workspace that added it. Removing it (or deleting
-- that Workspace) leaves the row with no Workspace, so its Deleted Links keep their
-- Codes taken if any Workspace adds the host again (ADR-0005).
ALTER TABLE domains
    ADD COLUMN workspace_id BIGINT REFERENCES workspaces (id) ON DELETE SET NULL,
    -- Only an approved custom Domain redirects, takes Links and gets a certificate
    -- (ADR-0009). The Default Domain is always approved.
    ADD COLUMN status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'approved', 'rejected'));

CREATE INDEX domains_workspace_id ON domains (workspace_id);
