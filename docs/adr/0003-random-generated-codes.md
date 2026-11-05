# Generated Codes are random, not hashed from the Target

A Generated Code is 7 random base62 characters. Uniqueness is enforced by a unique constraint on Code in the database, never by checking first: an insert that hits the constraint simply draws a new Code and retries. This works unchanged across any number of server instances sharing one database, with no counter or coordination.

## Considered Options

- **Hash of the Target**: gives free deduplication, but a Link's Target can change (so the Code stops matching its hash) and two people shortening the same URL must get separate Links. Truncated hashes collide anyway, so it saves no code.
- **Base62 of an auto-increment ID**: shortest Codes, but enumerable, and needs a central counter once there are multiple instances.
