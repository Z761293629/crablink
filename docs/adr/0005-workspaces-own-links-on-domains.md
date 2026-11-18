# Links belong to a Workspace and live on a Domain

crablink is growing from one operator with one token into a product others self-host for their teams, and later a public service. So a Link is owned by a Workspace (a group of Users), not by a User, and its Code lives on a Domain instead of on the instance's single base URL. A Code is unique within its Domain, and ADR-0002 now holds per Domain: a Code is never reused on the Domain it was taken on.

## Considered Options

- **Links owned by a User**: simplest, but a team can't share and hand over Links when people leave. A solo user is just a Workspace of one.
- **One Domain per instance (keep `CRABLINK_BASE_URL`)**: no change to Codes, but custom Domains are table stakes for a short-link product, and bolting them on later means rekeying every Code.

## Consequences

- The same Code can exist on two Domains and point to different Targets.
- The Default Domain is shared by every Workspace, first come first served; a custom Domain belongs to one Workspace.
- Removing a custom Domain from a Workspace leaves its Links as Deleted Links on that Domain. If any Workspace adds the Domain again, those Codes stay taken.
- Deleting a Workspace turns its Links into Deleted Links that belong to no Workspace; their Codes stay taken on their Domains (ADR-0002).
