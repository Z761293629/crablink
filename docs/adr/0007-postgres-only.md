# Postgres is the only database

Supersedes ADR-0001. crablink is becoming a product that teams self-host and that will later run as a public service, so we move to Postgres now and drop SQLite rather than supporting both. One backend means every query, migration and test is written once; Click reporting in particular needs time bucketing, which SQLite and Postgres spell differently.

## Considered Options

- **Both, SQLite by default and Postgres optional**: the nicest self-hosting story, but every future feature pays double: two migration sets, a CI matrix, and queries limited to the shared subset or written twice.
- **Stay on SQLite until we need several server instances**: cheapest today, but the public service needs Postgres anyway, and moving later means migrating far more data and code.

## Consequences

- Self-hosting goes from `docker run` with one file to `docker compose` with a Postgres service; "back up by copying a file" is gone.
- There is no upgrade path for data: an instance moving off SQLite starts with an empty Postgres database.
