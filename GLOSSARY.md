# crablink

A short-link service: maps a short path on a Domain to a long URL and redirects visitors to it.

## Language

**Link**:
The mapping from a Code on a Domain to a Target, owned by one Workspace. Its Target can change after creation; its Code and Domain cannot.
_Avoid_: Short link, URL, mapping, redirect

**Code**:
The identifier in the path of a Short URL, e.g. `aZ3k9` in `crabl.ink/aZ3k9`. Either a **Generated Code** (chosen by the service) or a **Custom Code** (chosen by whoever creates the Link). Case-sensitive, unique within its Domain, and never reused on that Domain once taken, even after its Link is deleted.
_Avoid_: Slug (that names a Workspace), key, alias, hash, short id

**Reserved Code**:
A Code the instance keeps off-limits as a Custom Code on the Default Domain, such as a well-known brand name, so nobody can take it first.
_Avoid_: Blocked code, banned word, blacklist

**Target**:
The URL a Link sends visitors to.
_Avoid_: Destination, long URL, original URL

**Short URL**:
A Domain joined with a Code, e.g. `crabl.ink/aZ3k9`. Derived for display, never stored.
_Avoid_: Short link, link

**Tag**:
A label a Workspace puts on its Links to filter and group them. A Link can have many Tags; Tags never change where a Link redirects.
_Avoid_: Label, folder, category, group

**Expiry**:
An optional moment after which a Link stops redirecting.
_Avoid_: TTL, deadline

**Deleted Link**:
A Link that has been removed, or whose Workspace was deleted, leaving it with no Workspace. It no longer redirects, but still holds its Code on its Domain so that Short URL can never point anywhere else.
_Avoid_: Removed, archived

**Blocked Link**:
A Link an Instance Admin has stopped for abuse. Visitors see why instead of being redirected, and its Workspace still sees it but can no longer change it; only an Instance Admin can unblock it. Its Workspace can still delete it, which makes it a Deleted Link.
_Avoid_: Disabled, suspended, banned

**Report**:
A visitor's note, with a reason, that a Short URL is used for abuse. It stays open until an Instance Admin blocks its Link or dismisses the Report.
_Avoid_: Complaint, flag, abuse ticket

**Click**:
One redirect of a human visitor through a Link, with when it happened and where it came from (Referrer, country, device, browser). Bots and link-preview fetchers are redirected but never make a Click.
_Avoid_: Hit, view, visit, event

**Click Count**:
The number of Clicks a Link has had.
_Avoid_: Hits, views, visits, PV

**Domain**:
A host name that Short URLs live on, e.g. `crabl.ink` or `go.example.com`. Each Code belongs to exactly one Domain. A custom Domain belongs to one Workspace.
_Avoid_: Host, base URL, hostname

**Default Domain**:
The instance's own Domain, shared by every Workspace: its Codes go to whichever Workspace takes them first.
_Avoid_: Base URL, main domain, shared domain

**Workspace**:
The owner of Links: a group of Users who manage the same Links together, each as its Owner or a Member. Someone working alone has a Workspace of one.
_Avoid_: Team, organization, account, project

**Workspace slug**:
The name of a Workspace in URLs, e.g. `marketing-team`: unique on the instance, derived from the Workspace's name when it is created, and kept when it is renamed. Always "Workspace slug", so it is never mistaken for a Code.
_Avoid_: Bare "slug", Workspace id, handle

**Personal Workspace**:
The Workspace a User gets on their first sign-in, with them as its Owner. Otherwise an ordinary Workspace.
_Avoid_: Default workspace, home workspace

**User**:
A person who signs in to manage Links, through one or more Workspaces they belong to.
_Avoid_: Account, operator

**Verified email**:
A User's email once the instance knows it is theirs: through an emailed link, an Invite, an Instance Admin, or an Identity Provider that vouches for it. Until then the User can create neither Links nor API Keys.
_Avoid_: Confirmed email, validated email

**Owner**:
A User who manages who is in a Workspace, and its Domains and API Keys, as well as its Links.
_Avoid_: Admin, manager

**Member**:
A User who manages a Workspace's Links but not the Workspace itself.
_Avoid_: Collaborator, editor

**Instance Admin**:
A User who runs the instance: invites and manages Users and approves Domains. Separate from any Workspace role.
_Avoid_: Admin, superuser, root, operator

**Invite**:
A one-time link an Instance Admin sends a new User to set their password, optionally joining a Workspace as a Member. It expires unused after a while.
_Avoid_: Invitation, sign-up link, registration

**Open sign-up**:
An instance switch, off by default, that lets anyone become a User without an Invite: with an email and a password, or through any Identity Provider.
_Avoid_: Registration, public sign-up, self-service sign-up

**Session**:
A User's sign-in in a browser, lasting until they sign out or it expires, unless a password change or their being disabled ends it first.
_Avoid_: Login, cookie, token

**Identity Provider**:
A service outside the instance that Users can sign in through instead of a password, e.g. Google or a company's Keycloak. An Instance Admin sets up which ones the instance trusts.
_Avoid_: IdP, SSO provider, OIDC provider, auth provider, connector

**Identity**:
A User as one Identity Provider knows them, by its issuer and subject. A User can have several; signing in through an Identity Provider finds the User by their Identity, or else by Verified email.
_Avoid_: Linked account, external account, connection

**API Key**:
A secret that lets a program manage one Workspace's Links. Belongs to the Workspace, not to the User who made it.
_Avoid_: Token, access token
