-- A Blocked Link: stopped for abuse by an Instance Admin until one unblocks it.
ALTER TABLE links ADD COLUMN blocked_at TIMESTAMPTZ;
