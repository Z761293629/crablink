# crablink

A short-link service: maps a short path on a Domain to a long URL and redirects visitors to it.

## Language

**Link**:
The mapping from a Code on a Domain to a Target, owned by one Workspace. Its Target can change after creation; its Code and Domain cannot.
_Avoid_: Short link, URL, mapping, redirect

**Code**:
The identifier in the path of a Short URL, e.g. `aZ3k9` in `crab.link/aZ3k9`. Either a **Generated Code** (chosen by the service) or a **Custom Code** (chosen by whoever creates the Link). Case-sensitive, unique within its Domain, and never reused on that Domain once taken, even after its Link is deleted.
_Avoid_: Slug, key, alias, hash, short id

**Target**:
The URL a Link sends visitors to.
_Avoid_: Destination, long URL, original URL

**Short URL**:
A Domain joined with a Code, e.g. `crab.link/aZ3k9`. Derived for display, never stored.
_Avoid_: Short link, link

**Tag**:
A label a Workspace puts on its Links to filter and group them. A Link can have many Tags; Tags never change where a Link redirects.
_Avoid_: Label, folder, category, group

**Expiry**:
An optional moment after which a Link stops redirecting.
_Avoid_: TTL, deadline

**Deleted Link**:
A Link that has been removed. It no longer redirects, but still holds its Code on its Domain so that Short URL can never point anywhere else.
_Avoid_: Removed, archived

**Click**:
One redirect of a human visitor through a Link, with when it happened and where it came from (Referrer, country, device, browser). Bots and link-preview fetchers are redirected but never make a Click.
_Avoid_: Hit, view, visit, event

**Click Count**:
The number of Clicks a Link has had.
_Avoid_: Hits, views, visits, PV

**Domain**:
A host name that Short URLs live on, e.g. `crab.link` or `go.example.com`. Each Code belongs to exactly one Domain. A custom Domain belongs to one Workspace.
_Avoid_: Host, base URL, hostname

**Default Domain**:
The instance's own Domain, shared by every Workspace: its Codes go to whichever Workspace takes them first.
_Avoid_: Base URL, main domain, shared domain

**Workspace**:
The owner of Links: a group of Users who manage the same Links together. Someone working alone has a Workspace with one member.
_Avoid_: Team, organization, account, project

**User**:
A person who signs in to manage Links, through one or more Workspaces they belong to.
_Avoid_: Account, operator

**Owner**:
A User who manages a Workspace's members, Domains and API Keys, as well as its Links.
_Avoid_: Admin, manager

**Member**:
A User who manages a Workspace's Links but not the Workspace itself.
_Avoid_: Collaborator, editor

**Instance Admin**:
A User who runs the instance: creates Users and approves Domains. Separate from any Workspace role.
_Avoid_: Admin, superuser, root, operator

**API Key**:
A secret that lets a program manage one Workspace's Links. Belongs to the Workspace, not to the User who made it.
_Avoid_: Token, access token
