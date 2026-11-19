# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users

- **Primary (web UI): non-developers on a team** — operations, marketing and admin colleagues who will never install a CLI. They create Links, change a Target after a poster is printed, download a QR code, and check how a Link is doing in its Clicks. Usage is occasional, often on a phone, in English or Simplified Chinese.
- **Owners and Instance Admins** use the same web UI for management: members, API Keys, custom Domains, invites, Users.
- **Developers** mainly use the `crablink` CLI, its TUI and the JSON API with an API Key; the web UI is not built around them.

## Product Purpose

crablink is a self-hosted short-link service a team runs for itself: one server binary, one Postgres, a CLI and an embedded web UI. Phase A (spec #47) is a team product that can be self-hosted and used daily without asking its author anything; a public sign-up service is a later phase B. Success: a colleague turns a long URL into a Short URL on the team's own Domain, shares it or prints its QR code, and later sees who clicked, without help.

## Positioning

- **Codes are never reused** on a Domain, even after a Link is deleted, so nothing printed ever points somewhere new (ADR-0002).
- **A Link's Target can change** after creation while its Short URL stays the same.
- **Self-hosted and private**: the team owns the instance and the data; Clicks never store an IP (ADR-0006).
- Workspaces own Links on Domains, so a team keeps its Links when someone leaves (ADR-0005).

## Operating Context

- A team self-hosts with `docker compose` (server + Postgres) behind a reverse proxy (Caddy) that terminates TLS (ADR-0007, ADR-0009).
- The web UI is served by the same server at `/app`, same origin, signed in with a session cookie (ADR-0008).
- Typical moments: making a link for a campaign or poster, copying a Short URL into a chat, downloading a QR code for print, glancing at Clicks on a phone.

## Capabilities and Constraints

- Domain terms come from `GLOSSARY.md` (Link, Code, Target, Short URL, Domain, Workspace, Owner, Member, Instance Admin, API Key, Click, Tag, Expiry) and are used in UI copy too.
- Web UI covers everything the CLI does with Links plus Workspace, member, Domain and API Key management, Click charts, Tags, search and CSV.
- UI languages: English and Simplified Chinese, default from the browser, switchable and remembered per browser; API errors are shown by translating their `kind`.
- Must work at phone width and follow the system light or dark theme.
- Stack is fixed: React + TypeScript + Vite in `web/`, embedded into the Rust server.

## Brand Commitments

Binding (user confirmed): `logo/GUIDELINES.md`.

- Logo files in `logo/masters/` used as-is (horizontal preferred; symbol for favicon and small spaces; `*-reversed` on dark), with 2X clear space and minimum sizes; never stretched, recoloured, outlined, shadowed or re-typeset.
- Brand colours: Crab Red `#E0412A`, Shell Ink `#17181C`. Crab Red is never used for small body text.
- Body type Inter or the system sans-serif; code in JetBrains Mono.

## Evidence on Hand

- Logo masters and exports: `logo/masters/`, `logo/export/`; brand guide `logo/GUIDELINES.md`.
- Product decisions: `docs/adr/`, `GLOSSARY.md`, spec #47 and its tickets.
- No testimonials, customer names, usage numbers or benchmarks exist; do not invent any.

## Product Principles

1. **Never surprise someone holding an old Short URL**: Codes stay taken; deletions and Domain removals are explicit and confirmed.
2. **One step for the common case**: a Link with a Generated Code on the Default Domain is a single action; everything else is optional.
3. **Same rules everywhere**: the web UI, CLI and API share validation and error `kind`s, so behaviour never differs by surface.
4. **Private by default**: show what helps a Link's owner, never data the product doesn't keep (no IPs, no unique visitors).

## Accessibility & Inclusion

- Target: WCAG 2.2 AA (user confirmed).
- Bilingual English / Simplified Chinese UI, phone width, light and dark themes.
