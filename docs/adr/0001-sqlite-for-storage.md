# SQLite for storage, Postgres later if we go public

crablink starts as a single self-hosted instance with one operator, so we store Links in SQLite (via `sqlx`): one file, zero ops, backup is a file copy. We may later open it to the public, which would need multiple writers and instances; we accept a migration to Postgres at that point instead of running Postgres now. To keep that migration cheap, SQL stays within the subset both databases support.

## Considered Options

- **Postgres now**: no future migration, but an extra service to run for a project that may never need it.
- **Redis / KV**: fastest redirects, but Click Count, Deleted Links and listing need relational queries.
