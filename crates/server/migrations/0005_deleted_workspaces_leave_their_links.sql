-- Deleting a Workspace leaves its Links as Deleted Links with no Workspace, so
-- their Codes stay taken (ADR-0002). Its memberships and API Keys go with it.
ALTER TABLE links
    ALTER COLUMN workspace_id DROP NOT NULL,
    DROP CONSTRAINT links_workspace_id_fkey,
    ADD CONSTRAINT links_workspace_id_fkey
        FOREIGN KEY (workspace_id) REFERENCES workspaces (id) ON DELETE SET NULL;
