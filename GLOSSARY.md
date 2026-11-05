# crablink

A short-link service: maps a short path on our domain to a long URL and redirects visitors to it.

## Language

**Link**:
The mapping from a Code to a Target. Its Target can change after creation; the Code cannot.
_Avoid_: Short link, URL, mapping, redirect

**Code**:
The identifier in the path of a Short URL, e.g. `aZ3k9` in `crab.link/aZ3k9`. Either a **Generated Code** (chosen by the service) or a **Custom Code** (chosen by whoever creates the Link). Case-sensitive, and never reused once taken, even after its Link is deleted.
_Avoid_: Slug, key, alias, hash, short id

**Target**:
The URL a Link sends visitors to.
_Avoid_: Destination, long URL, original URL

**Short URL**:
The service's base URL joined with a Code. Derived for display, never stored.
_Avoid_: Short link, link

**Expiry**:
An optional moment after which a Link stops redirecting.
_Avoid_: TTL, deadline

**Deleted Link**:
A Link that has been removed. It no longer redirects, but still holds its Code so the Code can never point anywhere else.
_Avoid_: Removed, archived

**Click Count**:
The number of times a Link has redirected a visitor.
_Avoid_: Hits, views, visits, PV
